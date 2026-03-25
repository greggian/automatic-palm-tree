//! nhc-download — Download NHC tropical cyclone advisories from the public archive.
//!
//! ARCHIVE STRUCTURE
//! -----------------
//!   https://www.nhc.noaa.gov/archive/
//!       → year links:  2024/, 2023/, …
//!   https://www.nhc.noaa.gov/archive/<year>/
//!       → storm links: ep04/, al12/, cp01/, …
//!   https://www.nhc.noaa.gov/archive/<year>/<basin><num>/
//!       → advisory files: <storm_id>.<product>.<NNN>.shtml
//!   Advisory text lives in the first <pre>…</pre> of each .shtml page.
//!
//! EXAMPLES
//! --------
//!   nhc-download --storm EP 4 2025
//!   nhc-download --year 2024 --basin AL --product fstadv,public
//!   nhc-download --all --concurrency 8 --skip-existing --output /data/nhc

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::LazyLock;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use clap::{ArgGroup, Parser};
use regex::Regex;
use reqwest::Client;
use tokio::sync::Semaphore;
use tokio::time::sleep;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const BASE_URL: &str = "https://www.nhc.noaa.gov/archive";
const ALL_PRODUCTS: &[&str] = &["fstadv", "public", "discus", "wndprb"];
const ALL_BASINS: &[&str] = &["ep", "al", "cp"];
/// Oldest year that reliably has all four product types.
const MIN_YEAR: u32 = 2008;

// ---------------------------------------------------------------------------
// Regex patterns (compiled once)
// ---------------------------------------------------------------------------

static RE_PRE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?si)<pre[^>]*>(.*?)</pre>").unwrap()
});
static RE_TAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<[^>]+>").unwrap()
});
static RE_YEAR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"href="(\d{4})/""#).unwrap()
});
/// Matches storm-name page links on year index, e.g. href="ANDREA.shtml" or href="ANDREA.shtml?"
static RE_STORM_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"href="([A-Z]{3,}(?:-[A-Z]+)*)\.shtml\??"#).unwrap()
});
/// Extracts basin dir from absolute advisory paths on storm name pages,
/// e.g. /archive/2025/al01/al012025.fstadv.001.shtml → "al01"
static RE_BASIN_DIR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"/archive/\d+/([a-z]{2}\d{2})/"#).unwrap()
});
/// Matches advisory file links — may be absolute (/archive/YYYY/XXNN/...) or relative.
static RE_FILE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)href="(?:/archive/\d+/[a-z]{2}\d{2}/)?([a-z]{2}\d{2}\d{4})\.(fstadv|public|discus|wndprb)\.(\d{3})\.shtml""#,
    )
    .unwrap()
});

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(
    name = "nhc-download",
    about = "Download NHC tropical cyclone advisories from the public archive",
    group(ArgGroup::new("mode").required(true).args(["storm", "year", "all"]))
)]
struct Cli {
    /// Download one storm: --storm EP 4 2025
    #[arg(long, num_args = 3, value_names = ["BASIN", "NUM", "YEAR"])]
    storm: Option<Vec<String>>,

    /// Download all storms in YEAR
    #[arg(long, value_name = "YEAR")]
    year: Option<u32>,

    /// Download every storm in the archive
    #[arg(long)]
    all: bool,

    /// Basin filter — comma-separated: EP,AL,CP  (default: all)
    #[arg(long, value_name = "BASIN")]
    basin: Option<String>,

    /// Product filter — comma-separated: fstadv,public,discus,wndprb  (default: all)
    #[arg(long, value_name = "PRODUCT")]
    product: Option<String>,

    /// Output directory  (default: nhc_data/)
    #[arg(long, default_value = "nhc_data", value_name = "DIR")]
    output: PathBuf,

    /// Parallel download workers  (default: 4)
    #[arg(long, default_value_t = 4, value_name = "N")]
    concurrency: usize,

    /// Skip files that already exist on disk
    #[arg(long)]
    skip_existing: bool,
}

// ---------------------------------------------------------------------------
// HTTP helpers
// ---------------------------------------------------------------------------

fn build_client() -> Result<Client> {
    Client::builder()
        .user_agent(
            "nhc-archive-fetcher/1.0 (+https://github.com/greggian/automatic-palm-tree)",
        )
        .timeout(Duration::from_secs(30))
        .gzip(true)
        .build()
        .context("build HTTP client")
}

/// Fetch a URL, retrying up to 4 times with exponential back-off.
async fn fetch(client: &Client, url: &str) -> Result<String> {
    let mut delay = Duration::from_secs(2);
    let mut last_err: Option<anyhow::Error> = None;

    for attempt in 0..4u32 {
        match client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => {
                return resp.text().await.context("read response body");
            }
            Ok(resp) => {
                last_err = Some(anyhow!("HTTP {}", resp.status()));
            }
            Err(e) => {
                last_err = Some(e.into());
            }
        }
        if attempt < 3 {
            eprintln!(
                "  [retry {}/3] {} — sleeping {}s",
                attempt + 1,
                last_err.as_ref().unwrap(),
                delay.as_secs()
            );
            sleep(delay).await;
            delay *= 2;
        }
    }
    Err(last_err.unwrap())
}

/// Extract and clean the first `<pre>` block from an HTML page.
fn extract_pre(html: &str) -> Result<String> {
    let cap = RE_PRE
        .captures(html)
        .ok_or_else(|| anyhow!("no <pre> block found"))?;
    let inner = cap.get(1).unwrap().as_str();
    // Strip any residual HTML tags inside the pre block.
    let stripped = RE_TAG.replace_all(inner, "");
    // Decode common HTML entities.
    let decoded = stripped
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&nbsp;", " ");
    Ok(decoded.trim().to_string())
}

// ---------------------------------------------------------------------------
// Archive discovery
// ---------------------------------------------------------------------------

/// Return all years present in the archive (newest first, MIN_YEAR and above).
async fn discover_years(client: &Client) -> Result<Vec<u32>> {
    let html = fetch(client, &format!("{BASE_URL}/")).await?;
    let mut years: Vec<u32> = RE_YEAR
        .captures_iter(&html)
        .filter_map(|c| c[1].parse::<u32>().ok())
        .filter(|&y| y >= MIN_YEAR)
        .collect();
    years.sort_unstable_by(|a, b| b.cmp(a));
    Ok(years)
}

/// Return `(storm_name, basin_dir, storm_id)` tuples for every storm in `year`.
///
/// The NHC year index lists storms by name (e.g. `href="ANDREA.shtml"`).
/// Each named storm page contains absolute advisory links from which we
/// extract the numbered basin directory (e.g. `al01`).
async fn discover_storms(
    client: &Client,
    year: u32,
    basins: &HashSet<String>,
) -> Result<Vec<(String, String, String)>> {
    let html = fetch(client, &format!("{BASE_URL}/{year}/")).await?;

    let storm_names: Vec<String> = RE_STORM_NAME
        .captures_iter(&html)
        .map(|c| c[1].to_string())
        .collect();

    let mut seen = std::collections::HashSet::new();
    let mut storms = Vec::new();

    for name in storm_names {
        let storm_url = format!("{BASE_URL}/{year}/{name}.shtml");
        let storm_html = match fetch(client, &storm_url).await {
            Ok(h) => h,
            Err(e) => {
                eprintln!("  Warning: could not fetch storm page {name}: {e}");
                continue;
            }
        };
        if let Some(cap) = RE_BASIN_DIR.captures(&storm_html) {
            let bd = cap[1].to_lowercase();
            if basins.contains(&bd[..2]) && seen.insert(bd.clone()) {
                let storm_id = format!("{bd}{year}");
                storms.push((name, bd, storm_id));
            }
        }
    }

    Ok(storms)
}

// ---------------------------------------------------------------------------
// Download jobs
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Job {
    year: u32,
    basin_dir: String,
    storm_id: String,
    product: String,
    adv_num: String,
}

/// Discover all advisory files for one storm via its name page.
///
/// The storm name page (e.g. `HELENE.shtml`) lists all product types with
/// absolute links. The basin directory pages only have fstadv and public.
async fn jobs_for_storm(
    client: &Client,
    year: u32,
    storm_name: &str,
    basin_dir: &str,
    storm_id: &str,
    products: &HashSet<String>,
) -> Vec<Job> {
    let url = format!("{BASE_URL}/{year}/{storm_name}.shtml");
    let html = match fetch(client, &url).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("  Warning: could not index {storm_id}: {e}");
            return vec![];
        }
    };

    RE_FILE
        .captures_iter(&html)
        .filter(|c| c[1].to_lowercase() == storm_id.to_lowercase())
        .filter(|c| products.contains(&c[2].to_lowercase()))
        .map(|c| Job {
            year,
            basin_dir: basin_dir.to_string(),
            storm_id: c[1].to_lowercase(),
            product: c[2].to_lowercase(),
            adv_num: c[3].to_string(),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Download one file
// ---------------------------------------------------------------------------

struct DownloadResult {
    filename: String,
    ok: bool,
    msg: String,
}

async fn download_one(
    client: &Client,
    job: Job,
    out_dir: &PathBuf,
    skip_existing: bool,
) -> DownloadResult {
    let filename = format!("{}.{}.{}.txt", job.storm_id, job.product, job.adv_num);
    let out_path = out_dir.join(job.year.to_string()).join(&filename);

    if skip_existing && out_path.exists() {
        return DownloadResult { filename, ok: true, msg: "skipped (exists)".into() };
    }

    let url = format!(
        "{BASE_URL}/{}/{}/{}.{}.{}.shtml",
        job.year, job.basin_dir, job.storm_id, job.product, job.adv_num
    );

    match fetch(client, &url).await {
        Err(e) => DownloadResult { filename, ok: false, msg: e.to_string() },
        Ok(html) => match extract_pre(&html) {
            Err(e) => DownloadResult { filename, ok: false, msg: e.to_string() },
            Ok(text) => {
                if let Some(parent) = out_path.parent() {
                    if let Err(e) = tokio::fs::create_dir_all(parent).await {
                        return DownloadResult { filename, ok: false, msg: e.to_string() };
                    }
                }
                let content = format!("{text}\n");
                match tokio::fs::write(&out_path, content.as_bytes()).await {
                    Ok(_) => DownloadResult {
                        filename,
                        ok: true,
                        msg: format!("OK ({} chars)", text.len()),
                    },
                    Err(e) => DownloadResult { filename, ok: false, msg: e.to_string() },
                }
            }
        },
    }
}

// ---------------------------------------------------------------------------
// Parallel runner
// ---------------------------------------------------------------------------

async fn run_downloads(
    client: Arc<Client>,
    jobs: Vec<Job>,
    out_dir: Arc<PathBuf>,
    concurrency: usize,
    skip_existing: bool,
) -> (usize, usize) {
    let sem = Arc::new(Semaphore::new(concurrency));
    let mut handles = Vec::with_capacity(jobs.len());

    for job in jobs {
        let permit = sem.clone().acquire_owned().await.unwrap();
        let c = client.clone();
        let d = out_dir.clone();
        handles.push(tokio::spawn(async move {
            let _permit = permit;
            download_one(&c, job, &d, skip_existing).await
        }));
    }

    let mut ok = 0usize;
    let mut errors = 0usize;
    for handle in handles {
        let r = handle.await.unwrap();
        let status = if r.ok { "OK " } else { "ERR" };
        println!("  {status}  {}  {}", r.filename, r.msg);
        if r.ok { ok += 1; } else { errors += 1; }
    }
    (ok, errors)
}

// ---------------------------------------------------------------------------
// Argument validation helpers
// ---------------------------------------------------------------------------

fn resolve_basins(arg: Option<&str>) -> Result<HashSet<String>> {
    match arg {
        None => Ok(ALL_BASINS.iter().map(|s| s.to_string()).collect()),
        Some(s) => {
            let set: HashSet<String> = s.split(',').map(|b| b.trim().to_lowercase()).collect();
            let bad: Vec<_> = set.iter().filter(|b| !ALL_BASINS.contains(&b.as_str())).collect();
            if !bad.is_empty() {
                bail!("unknown basin(s): {}. valid: EP, AL, CP", bad.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
            }
            Ok(set)
        }
    }
}

fn resolve_products(arg: Option<&str>) -> Result<HashSet<String>> {
    match arg {
        None => Ok(ALL_PRODUCTS.iter().map(|s| s.to_string()).collect()),
        Some(s) => {
            let set: HashSet<String> = s.split(',').map(|p| p.trim().to_lowercase()).collect();
            let bad: Vec<_> = set.iter().filter(|p| !ALL_PRODUCTS.contains(&p.as_str())).collect();
            if !bad.is_empty() {
                bail!("unknown product(s): {}. valid: fstadv, public, discus, wndprb", bad.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
            }
            Ok(set)
        }
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let basins   = resolve_basins(cli.basin.as_deref())?;
    let products = resolve_products(cli.product.as_deref())?;
    let out_dir  = Arc::new(cli.output);
    let client   = Arc::new(build_client()?);

    // ── Collect all download jobs ────────────────────────────────────────────

    let mut all_jobs: Vec<Job> = Vec::new();

    if let Some(parts) = cli.storm {
        // --storm BASIN NUM YEAR
        let basin = parts[0].to_lowercase();
        if !ALL_BASINS.contains(&basin.as_str()) {
            bail!("unknown basin '{}'. valid: EP, AL, CP", parts[0]);
        }
        let num: u32 = parts[1].parse().context("--storm NUM must be an integer")?;
        let year: u32 = parts[2].parse().context("--storm YEAR must be an integer")?;
        let basin_dir = format!("{basin}{num:02}");
        let storm_id  = format!("{basin_dir}{year}");

        // Discover the storm name so we can index from the name page
        // (basin directory pages only list fstadv and public).
        println!("Discovering storm name for {storm_id} …");
        let all_storms = discover_storms(&client, year, &basins).await?;
        let storm_name = all_storms.iter()
            .find(|(_, bd, _)| bd == &basin_dir)
            .map(|(name, _, _)| name.clone());

        if let Some(name) = storm_name {
            println!("  Found: {name}");
            println!("  Indexing {storm_id} …");
            all_jobs.extend(
                jobs_for_storm(&client, year, &name, &basin_dir, &storm_id, &products).await,
            );
        } else {
            bail!("storm {storm_id} not found in the {year} archive");
        }

    } else if let Some(year) = cli.year {
        // --year YEAR
        println!("Discovering storms in {year} …");
        let storms = discover_storms(&client, year, &basins).await?;
        println!("  Found {} storm(s): {}", storms.len(),
            storms.iter().map(|(_, _, s)| s.as_str()).collect::<Vec<_>>().join(", "));
        for (name, bd, sid) in &storms {
            println!("  Indexing {sid} …");
            all_jobs.extend(jobs_for_storm(&client, year, name, bd, sid, &products).await);
        }

    } else {
        // --all
        println!("Discovering available years …");
        let years = discover_years(&client).await?;
        println!("  Found {} year(s): {}", years.len(),
            years.iter().map(|y| y.to_string()).collect::<Vec<_>>().join(", "));
        for year in years {
            println!("  Discovering storms in {year} …");
            let storms = discover_storms(&client, year, &basins).await?;
            println!("    {} storm(s)", storms.len());
            for (name, bd, sid) in &storms {
                all_jobs.extend(jobs_for_storm(&client, year, name, bd, sid, &products).await);
            }
        }
    }

    if all_jobs.is_empty() {
        println!("No advisory files found.");
        return Ok(());
    }

    // ── Download ─────────────────────────────────────────────────────────────

    println!(
        "\nDownloading {} file(s) → {}  (concurrency={}) …\n",
        all_jobs.len(),
        out_dir.display(),
        cli.concurrency,
    );

    let (ok, errors) =
        run_downloads(client, all_jobs, out_dir, cli.concurrency, cli.skip_existing).await;

    println!("\nDone: {ok} downloaded, {errors} errors.");
    if errors > 0 {
        std::process::exit(1);
    }
    Ok(())
}
