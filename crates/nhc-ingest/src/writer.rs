//! Database writer — persists parsed advisories to PostgreSQL + PostGIS.
//!
//! All writes are idempotent (INSERT … ON CONFLICT DO NOTHING).
//!
//! NOTE: These use `sqlx::query` (dynamic) so the crate compiles without a
//! live database.  In production, convert to `sqlx::query!` macros with
//! `SQLX_OFFLINE=true` and a generated `sqlx-data.json` for compile-time
//! query verification.

use sqlx::PgPool;
use tracing::{info, instrument};

use nhc_types::{Advisory, ForecastAdvisory, ForecastDiscussion, PublicAdvisory, WindProbAdvisory};

// ── Public entry point ────────────────────────────────────────────────────────

/// Persist a parsed advisory to the database.
#[instrument(skip(pool, advisory), fields(
    storm_id  = %advisory.wmo().storm_id,
    adv_num   = %advisory.advisory_number(),
    product   = %advisory.wmo().product_type,
))]
pub async fn write_advisory(pool: &PgPool, advisory: &Advisory) -> anyhow::Result<()> {
    match advisory {
        Advisory::Fstadv(a) => write_fstadv(pool, a).await,
        Advisory::Public(a) => write_public(pool, a).await,
        Advisory::Discus(a) => write_discus(pool, a).await,
        Advisory::Wndprb(a) => write_wndprb(pool, a).await,
    }
}

// ── fstadv ────────────────────────────────────────────────────────────────────

async fn write_fstadv(pool: &PgPool, a: &ForecastAdvisory) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO fstadv_advisory (
            storm_id, basin, storm_number, year,
            advisory_number, issued_utc,
            storm_name, status,
            lat, lon,
            position_accuracy_nm,
            movement_degrees, movement_cardinal, movement_kt,
            pressure_mb, max_wind_kt, gust_kt,
            prev_time, prev_lat, prev_lon,
            next_advisory_utc, forecaster
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22)
        ON CONFLICT (storm_id, advisory_number, issued_utc) DO NOTHING
        "#,
    )
    .bind(&a.wmo.storm_id)
    .bind(a.wmo.basin.to_string())
    .bind(a.wmo.storm_number as i16)
    .bind(a.wmo.year as i16)
    .bind(&a.advisory_number)
    .bind(a.issued_utc)
    .bind(&a.storm_name)
    .bind(a.status.to_string())
    .bind(a.lat as f64)
    .bind(a.lon as f64)
    .bind(a.position_accuracy_nm as i32)
    .bind(a.movement_degrees as i32)
    .bind(&a.movement_cardinal)
    .bind(a.movement_kt as i16)
    .bind(a.pressure_mb as i32)
    .bind(a.max_wind_kt as i32)
    .bind(a.gust_kt as i32)
    .bind(a.prev_position.as_ref().map(|(t, _, _)| t.clone()))
    .bind(a.prev_position.as_ref().map(|(_, lat, _)| *lat as f64))
    .bind(a.prev_position.as_ref().map(|(_, _, lon)| *lon as f64))
    .bind(a.next_advisory_utc.as_deref())
    .bind(&a.forecaster)
    .execute(pool)
    .await?;

    for r in &a.wind_radii {
        sqlx::query(
            r#"
            INSERT INTO wind_radii (
                storm_id, advisory_number, issued_utc,
                threshold_kt, ne_nm, se_nm, sw_nm, nw_nm
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(&a.wmo.storm_id)
        .bind(&a.advisory_number)
        .bind(a.issued_utc)
        .bind(r.threshold_kt as i16)
        .bind(r.ne_nm as i32)
        .bind(r.se_nm as i32)
        .bind(r.sw_nm as i32)
        .bind(r.nw_nm as i32)
        .execute(pool)
        .await?;
    }

    for fp in &a.forecast_points {
        sqlx::query(
            r#"
            INSERT INTO forecast_point (
                storm_id, advisory_number, issued_utc,
                valid_time, is_outlook, status,
                lat, lon, max_wind_kt, gust_kt
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(&a.wmo.storm_id)
        .bind(&a.advisory_number)
        .bind(a.issued_utc)
        .bind(&fp.valid_time)
        .bind(fp.is_outlook)
        .bind(fp.status.to_string())
        .bind(fp.lat.map(|v| v as f64))
        .bind(fp.lon.map(|v| v as f64))
        .bind(fp.max_wind_kt.map(|v| v as i32))
        .bind(fp.gust_kt.map(|v| v as i32))
        .execute(pool)
        .await?;
    }

    info!("wrote fstadv advisory {}", a.advisory_number);
    Ok(())
}

// ── public ────────────────────────────────────────────────────────────────────

async fn write_public(pool: &PgPool, a: &PublicAdvisory) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO public_advisory (
            storm_id, advisory_number, issued_utc,
            storm_name, status,
            lat, lon,
            movement_degrees, movement_cardinal, movement_mph, movement_kmh,
            pressure_mb, max_wind_mph, max_wind_kmh, gust_mph, gust_kmh,
            headline, next_advisory_utc, forecaster
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19)
        ON CONFLICT (storm_id, advisory_number, issued_utc) DO NOTHING
        "#,
    )
    .bind(&a.wmo.storm_id)
    .bind(&a.advisory_number)
    .bind(a.issued_utc)
    .bind(&a.storm_name)
    .bind(a.status.to_string())
    .bind(a.lat as f64)
    .bind(a.lon as f64)
    .bind(a.movement_degrees as i32)
    .bind(&a.movement_cardinal)
    .bind(a.movement_mph as i32)
    .bind(a.movement_kmh as i32)
    .bind(a.pressure_mb as i32)
    .bind(a.max_wind_mph as i32)
    .bind(a.max_wind_kmh as i32)
    .bind(a.gust_mph as i32)
    .bind(a.gust_kmh as i32)
    .bind(a.headline.as_deref())
    .bind(a.next_advisory_utc.as_deref())
    .bind(&a.forecaster)
    .execute(pool)
    .await?;

    info!("wrote public advisory {}", a.advisory_number);
    Ok(())
}

// ── discus ────────────────────────────────────────────────────────────────────

async fn write_discus(pool: &PgPool, a: &ForecastDiscussion) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO discus_advisory (
            storm_id, advisory_number, issued_utc,
            storm_name, status, body, forecaster
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7)
        ON CONFLICT (storm_id, advisory_number, issued_utc) DO NOTHING
        "#,
    )
    .bind(&a.wmo.storm_id)
    .bind(&a.advisory_number)
    .bind(a.issued_utc)
    .bind(&a.storm_name)
    .bind(a.status.to_string())
    .bind(&a.body)
    .bind(&a.forecaster)
    .execute(pool)
    .await?;

    info!("wrote discus advisory {}", a.advisory_number);
    Ok(())
}

// ── wndprb ────────────────────────────────────────────────────────────────────

async fn write_wndprb(pool: &PgPool, a: &WindProbAdvisory) -> anyhow::Result<()> {
    for table in &a.tables {
        for entry in &table.entries {
            sqlx::query(
                r#"
                INSERT INTO wind_prob_entry (
                    storm_id, advisory_number, issued_utc,
                    threshold_kt, location, lat, lon,
                    p12, p24, p36, p48, p72, p96, p120
                )
                VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
                ON CONFLICT DO NOTHING
                "#,
            )
            .bind(&a.wmo.storm_id)
            .bind(&a.advisory_number)
            .bind(a.issued_utc)
            .bind(table.threshold_kt as i16)
            .bind(&entry.location)
            .bind(entry.lat as f64)
            .bind(entry.lon as f64)
            .bind(entry.probs[0] as i16)
            .bind(entry.probs[1] as i16)
            .bind(entry.probs[2] as i16)
            .bind(entry.probs[3] as i16)
            .bind(entry.probs[4] as i16)
            .bind(entry.probs[5] as i16)
            .bind(entry.probs[6] as i16)
            .execute(pool)
            .await?;
        }
    }

    info!("wrote wndprb advisory {}", a.advisory_number);
    Ok(())
}
