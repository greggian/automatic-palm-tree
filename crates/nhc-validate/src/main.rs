//! nhc-validate — Parse every downloaded NHC advisory and report results.
//!
//! Walks a directory tree, finds advisory `.txt` files named
//! `<storm_id>.<product>.<NNN>.txt`, parses each with the Rust nhc-parser,
//! and prints a coloured pass/fail summary broken down by product type.
//!
//! EXAMPLES
//! --------
//!   nhc-validate
//!   nhc-validate nhc_data/
//!   nhc-validate nhc_data/ --product fstadv
//!   nhc-validate nhc_data/ --verbose
//!   nhc-validate nhc_data/ --stop-on-error

use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Parser;
use nhc_parser::parse_advisory;
use walkdir::WalkDir;

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(
    name = "nhc-validate",
    about = "Parse all downloaded NHC advisory files and report pass/fail"
)]
struct Cli {
    /// Directory to scan  (default: nhc_data/)
    #[arg(default_value = "nhc_data", value_name = "DIR")]
    directory: PathBuf,

    /// Only validate this product type: fstadv, public, discus, wndprb
    #[arg(long, value_name = "PRODUCT")]
    product: Option<String>,

    /// Stop after the first parse failure
    #[arg(long)]
    stop_on_error: bool,

    /// Show passing files as well as failures
    #[arg(short, long)]
    verbose: bool,
}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

const ALL_PRODUCTS: &[&str] = &["fstadv", "public", "discus", "wndprb"];

struct FileResult {
    path: PathBuf,
    product: String,
    ok: bool,
    error: String,
}

// ---------------------------------------------------------------------------
// ANSI colours — disabled when stdout is not a terminal
// ---------------------------------------------------------------------------

struct Colours {
    grn: &'static str,
    red: &'static str,
    ylw: &'static str,
    dim: &'static str,
    rst: &'static str,
}

impl Colours {
    fn new() -> Self {
        if std::io::stdout().is_terminal() {
            Self { grn: "\x1b[32m", red: "\x1b[31m", ylw: "\x1b[33m", dim: "\x1b[2m", rst: "\x1b[0m" }
        } else {
            Self { grn: "", red: "", ylw: "", dim: "", rst: "" }
        }
    }
}

// ---------------------------------------------------------------------------
// File discovery
// ---------------------------------------------------------------------------

/// Infer product type from filename stem (e.g. `ep042025.fstadv.001` → `"fstadv"`).
fn infer_product(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_string_lossy();
    for part in stem.split('.') {
        if ALL_PRODUCTS.contains(&part.to_lowercase().as_str()) {
            return Some(part.to_lowercase());
        }
    }
    None
}

fn discover_files(root: &Path, product_filter: Option<&str>) -> Vec<(PathBuf, String)> {
    WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |x| x == "txt"))
        .filter_map(|e| {
            let path = e.into_path();
            let product = infer_product(&path)?;
            if let Some(filter) = product_filter {
                if product != filter {
                    return None;
                }
            }
            Some((path, product))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

fn validate_file(path: &Path) -> FileResult {
    let product = infer_product(path).unwrap_or_default();
    match std::fs::read_to_string(path) {
        Err(e) => FileResult { path: path.to_path_buf(), product, ok: false, error: e.to_string() },
        Ok(text) => match parse_advisory(&text) {
            Ok(_) => FileResult { path: path.to_path_buf(), product, ok: true, error: String::new() },
            Err(e) => FileResult { path: path.to_path_buf(), product, ok: false, error: format!("{e:?}") },
        },
    }
}

// ---------------------------------------------------------------------------
// Reporting
// ---------------------------------------------------------------------------

fn rel(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn print_result(r: &FileResult, root: &Path, c: &Colours, verbose: bool) {
    let path_str = rel(&r.path, root);
    if r.ok {
        if verbose {
            println!("  {}PASS{}  {}{}{}", c.grn, c.rst, c.dim, path_str, c.rst);
        }
    } else {
        println!("  {}FAIL{}  {}", c.red, c.rst, path_str);
        // First line of error, indented
        let first = r.error.lines().next().unwrap_or(&r.error);
        println!("        {}{}{}", c.ylw, first, c.rst);
    }
}

fn print_summary(results: &[FileResult], c: &Colours) {
    let total  = results.len();
    let passed = results.iter().filter(|r| r.ok).count();
    let failed = total - passed;

    println!();
    println!("{}", "─".repeat(60));
    println!("  Total : {total}");
    println!("  {}Passed{}: {passed}", c.grn, c.rst);
    if failed > 0 {
        println!("  {}Failed{}: {failed}", c.red, c.rst);
    }
    println!();

    for prod in ALL_PRODUCTS {
        let rows: Vec<_> = results.iter().filter(|r| r.product == *prod).collect();
        if rows.is_empty() { continue; }
        let p = rows.iter().filter(|r| r.ok).count();
        let f = rows.len() - p;
        let ok_str = format!("{}{} ok{}", c.grn, p, c.rst);
        let fail_str = if f > 0 { format!("  {}{} fail{}", c.red, f, c.rst) } else { String::new() };
        println!("  {:<8} {:>5} files   {}{}", prod, rows.len(), ok_str, fail_str);
    }
    println!("{}", "─".repeat(60));
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    let cli = Cli::parse();
    let c = Colours::new();

    let root = &cli.directory;
    if !root.exists() {
        eprintln!("{}error{}: directory not found: {}", c.red, c.rst, root.display());
        std::process::exit(2);
    }

    // Validate product filter if given
    let product_filter = match cli.product.as_deref() {
        None => None,
        Some(p) => {
            let pl = p.to_lowercase();
            if !ALL_PRODUCTS.contains(&pl.as_str()) {
                eprintln!(
                    "{}error{}: unknown product '{}'. valid: {}",
                    c.red, c.rst, p,
                    ALL_PRODUCTS.join(", ")
                );
                std::process::exit(2);
            }
            Some(pl)
        }
    };

    let files = discover_files(root, product_filter.as_deref());
    if files.is_empty() {
        println!("No advisory .txt files found under {}", root.display());
        return Ok(());
    }

    println!("Scanning {} file(s) under {} …\n", files.len(), root.display());

    let mut results: Vec<FileResult> = Vec::new();
    let mut any_failed = false;

    for (path, _product) in files {
        let r = validate_file(&path);
        let failed = !r.ok;
        print_result(&r, root, &c, cli.verbose);
        results.push(r);

        if failed {
            any_failed = true;
            if cli.stop_on_error {
                println!("\n{}Stopped on first error.{}", c.red, c.rst);
                break;
            }
        }
    }

    print_summary(&results, &c);

    if any_failed {
        std::process::exit(1);
    }
    Ok(())
}
