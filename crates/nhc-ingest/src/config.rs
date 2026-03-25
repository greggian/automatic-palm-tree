//! Service configuration loaded from environment variables or config file.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    /// PostgreSQL connection URL, e.g.
    /// `postgres://user:pass@localhost:5432/nhc`
    pub database_url: String,

    /// How often to poll NHC for new advisories (seconds).
    #[serde(default = "default_poll_interval_secs")]
    pub poll_interval_secs: u64,

    /// NHC RSS feed URL for change detection.
    #[serde(default = "default_rss_url")]
    pub rss_url: String,

    /// Number of concurrent advisory fetch tasks.
    #[serde(default = "default_fetch_concurrency")]
    pub fetch_concurrency: usize,
}

fn default_poll_interval_secs() -> u64 { 300 }
fn default_rss_url() -> String {
    "https://www.nhc.noaa.gov/nhc_at1.xml".to_string()
}
fn default_fetch_concurrency() -> usize { 4 }

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let cfg = config::Config::builder()
            .add_source(config::Environment::with_prefix("NHC").separator("__"))
            .build()?;
        Ok(cfg.try_deserialize()?)
    }
}
