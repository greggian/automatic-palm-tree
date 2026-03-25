//! NHC advisory poller.
//!
//! Polls the NHC RSS feed as a change signal and fetches individual advisory
//! HTML pages when new products are detected.  Falls back to direct URL
//! construction when the RSS feed is unavailable.

use std::collections::HashSet;
use std::time::Duration;

use anyhow::Context;
use reqwest::Client;
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::config::Config;

/// Archive page URL pattern:
/// `https://www.nhc.noaa.gov/archive/{year}/{storm_id}/{storm_id}.{product}.{num}.shtml`
const ARCHIVE_BASE: &str = "https://www.nhc.noaa.gov/archive";

/// Known product suffixes in the order they are typically issued.
const PRODUCTS: [&str; 4] = ["fstadv", "public", "discus", "wndprb"];

// ── Discovery entry from RSS / archive index ─────────────────────────────────

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct AdvisoryRef {
    pub storm_id: String,   // e.g. "ep042025"
    pub product: String,    // e.g. "fstadv"
    pub number: String,     // e.g. "001"
    pub url: String,
}

impl AdvisoryRef {
    pub fn build(storm_id: &str, product: &str, number: &str) -> Self {
        let year = &storm_id[storm_id.len() - 4..];
        let basin_num = &storm_id[..storm_id.len() - 4]; // "ep04"
        let url = format!(
            "{ARCHIVE_BASE}/{year}/{basin_num}/{storm_id}.{product}.{number}.shtml"
        );
        AdvisoryRef {
            storm_id: storm_id.to_string(),
            product: product.to_string(),
            number: number.to_string(),
            url,
        }
    }
}

// ── Poller ────────────────────────────────────────────────────────────────────

pub struct Poller {
    client: Client,
    seen: HashSet<String>,
}

impl Poller {
    pub fn new() -> anyhow::Result<Self> {
        let client = Client::builder()
            .user_agent("nhc-ingest/0.1 (+https://github.com/greggian/automatic-palm-tree)")
            .timeout(Duration::from_secs(30))
            .gzip(true)
            .build()
            .context("failed to build HTTP client")?;
        Ok(Poller { client, seen: HashSet::new() })
    }

    /// Fetch and return the raw text of an advisory, stripping the HTML wrapper.
    pub async fn fetch_advisory(&self, url: &str) -> anyhow::Result<String> {
        let html = self
            .client
            .get(url)
            .send()
            .await
            .with_context(|| format!("GET {url}"))?
            .error_for_status()
            .with_context(|| format!("HTTP error for {url}"))?
            .text()
            .await
            .context("reading response body")?;
        extract_pre_text(&html).with_context(|| format!("no <pre> block in {url}"))
    }

    /// Single poll cycle: check RSS feed and return newly-discovered refs.
    pub async fn poll_rss(&mut self, rss_url: &str) -> Vec<AdvisoryRef> {
        match self.fetch_rss(rss_url).await {
            Ok(refs) => {
                let new: Vec<_> = refs
                    .into_iter()
                    .filter(|r| self.seen.insert(r.url.clone()))
                    .collect();
                if !new.is_empty() {
                    info!(count = new.len(), "discovered new advisories");
                }
                new
            }
            Err(e) => {
                warn!("RSS poll failed: {e:#}");
                Vec::new()
            }
        }
    }

    async fn fetch_rss(&self, url: &str) -> anyhow::Result<Vec<AdvisoryRef>> {
        let text = self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        Ok(parse_rss_links(&text))
    }
}

// ── RSS link extraction ───────────────────────────────────────────────────────

/// Extract advisory archive links from the NHC RSS feed XML.
///
/// The RSS items contain `<link>` elements pointing to archive pages like:
/// `https://www.nhc.noaa.gov/archive/2025/ep04/ep042025.fstadv.001.shtml`
fn parse_rss_links(xml: &str) -> Vec<AdvisoryRef> {
    let mut refs = Vec::new();
    for line in xml.lines() {
        let t = line.trim();
        // Look for <link> elements (RSS 2.0 style)
        if let Some(inner) = t.strip_prefix("<link>").and_then(|s| s.strip_suffix("</link>")) {
            if let Some(r) = ref_from_url(inner) {
                refs.push(r);
            }
        }
        // Also handle Atom-style <link href="..." />
        if t.contains("href=") {
            for part in t.split('"') {
                if part.contains("archive/") && part.ends_with(".shtml") {
                    if let Some(r) = ref_from_url(part) {
                        refs.push(r);
                    }
                }
            }
        }
    }
    refs
}

/// Parse an NHC archive URL into an `AdvisoryRef`.
///
/// URL shape: `.../archive/{year}/{basin_num}/{storm_id}.{product}.{num}.shtml`
fn ref_from_url(url: &str) -> Option<AdvisoryRef> {
    let filename = url.split('/').last()?;
    // "ep042025.fstadv.001.shtml"
    let without_ext = filename.strip_suffix(".shtml")?;
    let parts: Vec<_> = without_ext.splitn(3, '.').collect();
    if parts.len() != 3 { return None; }
    let storm_id = parts[0];
    let product  = parts[1];
    let number   = parts[2];
    if !PRODUCTS.contains(&product) { return None; }
    Some(AdvisoryRef::build(storm_id, product, number))
}

// ── HTML pre-block extractor ──────────────────────────────────────────────────

/// Strip the HTML wrapper and return the text inside the `<pre>` block.
pub fn extract_pre_text(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start = lower.find("<pre")? + 4; // skip "<pre"
    let content_start = html[start..].find('>')? + start + 1;
    let end = lower[content_start..].find("</pre>")? + content_start;
    let raw = &html[content_start..end];
    // Strip any remaining HTML tags
    let stripped = strip_tags(raw);
    Some(stripped.trim().to_string())
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

// ── Continuous polling loop ───────────────────────────────────────────────────

/// Run the polling loop, calling `on_advisory` for each raw product text.
///
/// This function runs indefinitely; cancel via the runtime.
pub async fn run_poll_loop<F, Fut>(
    config: &Config,
    on_advisory: F,
) -> anyhow::Result<()>
where
    F: Fn(AdvisoryRef, String) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<()>>,
{
    let mut poller = Poller::new()?;
    loop {
        let new_refs = poller.poll_rss(&config.rss_url).await;
        for r in new_refs {
            match poller.fetch_advisory(&r.url).await {
                Ok(raw) => {
                    if let Err(e) = on_advisory(r.clone(), raw).await {
                        error!(url = %r.url, "advisory processing failed: {e:#}");
                    }
                }
                Err(e) => {
                    error!(url = %r.url, "fetch failed: {e:#}");
                }
            }
        }
        sleep(Duration::from_secs(config.poll_interval_secs)).await;
    }
}
