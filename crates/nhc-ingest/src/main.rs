//! NHC advisory ingestion service.
//!
//! Polls NHC for new advisories, parses them into typed AST nodes, and writes
//! them to PostgreSQL.
//!
//! # Configuration (environment variables)
//!
//! | Variable                  | Default                                  |
//! |---------------------------|------------------------------------------|
//! | `NHC__DATABASE_URL`       | _(required)_                             |
//! | `NHC__POLL_INTERVAL_SECS` | `300`                                    |
//! | `NHC__RSS_URL`            | `https://www.nhc.noaa.gov/nhc_at1.xml`   |
//! | `NHC__FETCH_CONCURRENCY`  | `4`                                      |

use std::sync::Arc;

use anyhow::Context;
use sqlx::postgres::PgPoolOptions;
use tracing::{error, info};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod config;
mod poller;
mod writer;

use config::Config;
use nhc_parser::parse_advisory;
use poller::{run_poll_loop, AdvisoryRef};
use writer::write_advisory;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── Observability ─────────────────────────────────────────────────────────
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(fmt::layer())
        .init();

    // ── Config ────────────────────────────────────────────────────────────────
    let config = Config::from_env().context("loading config")?;
    info!("starting nhc-ingest");

    // ── Database pool ─────────────────────────────────────────────────────────
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(&config.database_url)
        .await
        .context("connecting to database")?;

    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .context("running migrations")?;

    info!("database connected and migrations applied");

    let pool = Arc::new(pool);

    // ── Poll loop ─────────────────────────────────────────────────────────────
    run_poll_loop(&config, move |r: AdvisoryRef, raw: String| {
        let pool = Arc::clone(&pool);
        async move {
            let span = tracing::info_span!(
                "process_advisory",
                storm_id = %r.storm_id,
                product  = %r.product,
                number   = %r.number,
            );
            let _guard = span.enter();

            let advisory = parse_advisory(&raw).map_err(|e| {
                error!("parse error: {e}");
                anyhow::anyhow!("parse failed: {e}")
            })?;

            write_advisory(&pool, &advisory).await?;
            Ok(())
        }
    })
    .await
}
