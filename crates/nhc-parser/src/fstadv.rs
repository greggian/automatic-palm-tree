//! Parser for NHC Forecast/Advisory (fstadv / TCM) products.
//!
//! Grammar spec: `grammars/fstadv.pest`
//!
//! All numeric values are in **knots and nautical miles** as issued.

use chrono::{DateTime, TimeZone, Utc};
use pest::Parser;
use pest_derive::Parser;

use nhc_types::{ForecastAdvisory, ForecastPoint, StormStatus, WindRadii};

use crate::envelope::{
    build_wmo_header, extract_awips_id, extract_forecaster,
    parse_lat_token, parse_lon_token, parse_month_abbr, parse_status,
};
use crate::error::ParseError;

// ── Pest parser ───────────────────────────────────────────────────────────────

#[derive(Parser)]
#[grammar = "grammars/fstadv.pest"]
pub struct FstadvParser;

// ── Public entry point ────────────────────────────────────────────────────────

/// Parse a raw fstadv product string into a [`ForecastAdvisory`].
pub fn parse(raw: &str) -> Result<ForecastAdvisory, ParseError> {
    // Normalise line endings
    let text = raw.replace("\r\n", "\n").replace('\r', "\n");

    // ── Envelope ─────────────────────────────────────────────────────────────
    let awips_id = extract_awips_id(&text)?;
    let originator = extract_originator(&text);
    let forecaster = extract_forecaster(&text);

    // ── Run the pest grammar ──────────────────────────────────────────────────
    let pairs = FstadvParser::parse(Rule::fstadv, &text)
        .map_err(|e| ParseError::Grammar {
            product: "fstadv",
            detail: e.to_string(),
        })?;

    // ── Walk the pair tree ────────────────────────────────────────────────────
    let mut advisory_number = String::new();
    let mut issued_utc: Option<DateTime<Utc>> = None;
    let mut storm_name = String::new();
    let mut status = StormStatus::TropicalStorm;
    let mut lat = 0.0f32;
    let mut lon = 0.0f32;
    let mut position_accuracy_nm = 0u16;
    let mut movement_degrees = 0u16;
    let mut movement_cardinal = String::new();
    let mut movement_kt = 0u8;
    let mut pressure_mb = 0u16;
    let mut max_wind_kt = 0u16;
    let mut gust_kt = 0u16;
    let mut wind_radii: Vec<WindRadii> = Vec::new();
    let mut prev_position: Option<(String, f32, f32)> = None;
    let mut forecast_points: Vec<ForecastPoint> = Vec::new();
    let mut next_advisory_utc: Option<String> = None;
    let mut issued_year = 0u16;

    for pair in pairs.into_iter().next().unwrap().into_inner() {
        match pair.as_rule() {
            Rule::header_line => {
                let mut inner = pair.into_inner();
                status = parse_status(inner.next().unwrap().as_str())?;
                storm_name = inner.next().unwrap().as_str().to_string();
                // Skip the advisory type keyword(s)
                for p in inner {
                    if p.as_rule() == Rule::advisory_num {
                        advisory_number = p.as_str().to_string();
                        break;
                    }
                }
            }
            Rule::issued_line => {
                issued_utc = Some(parse_issued_line(&pair)?);
                // Extract year separately
                for p in pair.into_inner() {
                    if p.as_rule() == Rule::issued_year {
                        issued_year = p.as_str().parse().unwrap_or(0);
                    }
                }
            }
            Rule::position_line => {
                let mut inner = pair.into_inner();
                // Optional status prefix
                let first = inner.next().unwrap();
                let (lat_tok, lon_tok) = if first.as_rule() == Rule::status {
                    let lt = inner.next().unwrap();
                    let ln = inner.next().unwrap();
                    (lt, ln)
                } else if first.as_rule() == Rule::lat {
                    let ln = inner.next().unwrap();
                    (first, ln)
                } else {
                    continue;
                };
                lat = parse_lat_token(lat_tok.as_str())?;
                lon = parse_lon_token(lon_tok.as_str())?;
            }
            Rule::accuracy_line => {
                let n = pair.into_inner().next().unwrap().as_str();
                position_accuracy_nm = n.parse().unwrap_or(0);
            }
            Rule::movement_line => {
                let mut inner = pair.into_inner();
                movement_cardinal = inner.next().unwrap().as_str().to_string();
                movement_degrees = inner.next().unwrap().as_str().parse().unwrap_or(0);
                movement_kt = inner.next().unwrap().as_str().parse().unwrap_or(0);
            }
            Rule::pressure_line => {
                let n = pair.into_inner().next().unwrap().as_str();
                pressure_mb = n.parse().unwrap_or(0);
            }
            Rule::wind_line => {
                let mut inner = pair.into_inner();
                max_wind_kt = inner.next().unwrap().as_str().parse().unwrap_or(0);
                gust_kt = inner.next().unwrap().as_str().parse().unwrap_or(0);
            }
            Rule::radii_line => {
                wind_radii.push(parse_radii_line(pair)?);
            }
            Rule::prev_position_line => {
                let mut inner = pair.into_inner();
                let time = inner.next().unwrap().as_str().to_string();
                let plat = parse_lat_token(inner.next().unwrap().as_str())?;
                let plon = parse_lon_token(inner.next().unwrap().as_str())?;
                prev_position = Some((time, plat, plon));
            }
            Rule::forecast_block => {
                forecast_points.push(parse_forecast_block(pair)?);
            }
            Rule::next_advisory_line => {
                let t = pair.into_inner().next().unwrap().as_str().to_string();
                next_advisory_utc = Some(t);
            }
            _ => {}
        }
    }

    // Build WMO header (year comes from issued line)
    let wmo = build_wmo_header(&awips_id, &originator, None, Some(issued_year))?;

    let issued_utc = issued_utc.ok_or(ParseError::MissingField { field: "issued_utc" })?;

    Ok(ForecastAdvisory {
        wmo,
        advisory_number,
        issued_utc,
        storm_name,
        status,
        lat,
        lon,
        position_accuracy_nm,
        movement_degrees,
        movement_cardinal,
        movement_kt,
        pressure_mb,
        max_wind_kt,
        gust_kt,
        wind_radii,
        prev_position,
        forecast_points,
        next_advisory_utc,
        forecaster,
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn extract_originator(text: &str) -> String {
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("TTAA00 ") {
            if let Some(orig) = rest.split_whitespace().next() {
                return orig.to_uppercase();
            }
        }
    }
    "KNHC".to_string()
}

fn parse_issued_line(pair: &pest::iterators::Pair<Rule>) -> Result<DateTime<Utc>, ParseError> {
    let mut hhmm = 0u32;
    let mut month = 0u32;
    let mut day = 0u32;
    let mut year = 0i32;

    for p in pair.clone().into_inner() {
        match p.as_rule() {
            Rule::issued_hhmm => {
                let s = p.as_str();
                hhmm = s[..2].parse::<u32>().unwrap_or(0) * 100
                    + s[2..4].parse::<u32>().unwrap_or(0);
            }
            Rule::month_abbr => month = parse_month_abbr(p.as_str())?,
            Rule::issued_day  => day  = p.as_str().trim().parse().unwrap_or(0),
            Rule::issued_year => year = p.as_str().trim().parse().unwrap_or(0),
            _ => {}
        }
    }

    let hour = hhmm / 100;
    let min  = hhmm % 100;
    Utc.with_ymd_and_hms(year, month, day, hour, min, 0)
        .single()
        .ok_or(ParseError::InvalidValue {
            field: "issued_utc",
            value: format!("{year}-{month}-{day} {hour}:{min}"),
        })
}

fn parse_radii_line(pair: pest::iterators::Pair<Rule>) -> Result<WindRadii, ParseError> {
    let mut threshold_kt = 0u8;
    let mut ne = 0u16;
    let mut se = 0u16;
    let mut sw = 0u16;
    let mut nw = 0u16;

    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::radii_threshold => threshold_kt = p.as_str().parse().unwrap_or(0),
            Rule::radii_quad_ne   => ne = p.into_inner().next().unwrap().as_str().parse().unwrap_or(0),
            Rule::radii_quad_se   => se = p.into_inner().next().unwrap().as_str().parse().unwrap_or(0),
            Rule::radii_quad_sw   => sw = p.into_inner().next().unwrap().as_str().parse().unwrap_or(0),
            Rule::radii_quad_nw   => nw = p.into_inner().next().unwrap().as_str().parse().unwrap_or(0),
            _ => {}
        }
    }
    Ok(WindRadii { threshold_kt, ne_nm: ne, se_nm: se, sw_nm: sw, nw_nm: nw })
}

fn parse_forecast_block(
    pair: pest::iterators::Pair<Rule>,
) -> Result<ForecastPoint, ParseError> {
    let mut valid_time = String::new();
    let mut is_outlook = false;
    let mut fp_status = StormStatus::TropicalStorm;
    let mut fp_lat: Option<f32> = None;
    let mut fp_lon: Option<f32> = None;
    let mut max_wind_kt: Option<u16> = None;
    let mut gust_kt: Option<u16> = None;
    let mut wind_radii: Vec<WindRadii> = Vec::new();
    let mut dissipated = false;

    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::forecast_valid_line => {
                let mut inner = p.into_inner();
                valid_time = inner.next().unwrap().as_str().to_string();
                let next = inner.next();
                if let Some(pos) = next {
                    if pos.as_rule() == Rule::lat {
                        fp_lat = Some(parse_lat_token(pos.as_str())?);
                        fp_lon = Some(parse_lon_token(inner.next().unwrap().as_str())?);
                    } else {
                        // DISSIPATED
                        dissipated = true;
                    }
                } else {
                    dissipated = true;
                }
            }
            Rule::outlook_valid_line => {
                is_outlook = true;
                let mut inner = p.into_inner();
                valid_time = inner.next().unwrap().as_str().to_string();
                let next = inner.next();
                if let Some(pos) = next {
                    if pos.as_rule() == Rule::lat {
                        fp_lat = Some(parse_lat_token(pos.as_str())?);
                        fp_lon = Some(parse_lon_token(inner.next().unwrap().as_str())?);
                    } else {
                        dissipated = true;
                    }
                } else {
                    dissipated = true;
                }
            }
            Rule::forecast_wind_line => {
                let mut inner = p.into_inner();
                max_wind_kt = Some(inner.next().unwrap().as_str().parse().unwrap_or(0));
                gust_kt = Some(inner.next().unwrap().as_str().parse().unwrap_or(0));
            }
            Rule::forecast_radii_line => {
                wind_radii.push(parse_forecast_radii_line(p)?);
            }
            _ => {}
        }
    }

    if dissipated {
        fp_status = StormStatus::Dissipated;
    } else if let Some(w) = max_wind_kt {
        fp_status = if w >= 64 {
            StormStatus::Hurricane
        } else {
            StormStatus::TropicalStorm
        };
    }

    Ok(ForecastPoint {
        valid_time,
        is_outlook,
        status: fp_status,
        lat: fp_lat,
        lon: fp_lon,
        max_wind_kt,
        gust_kt,
        wind_radii,
    })
}

fn parse_forecast_radii_line(
    pair: pest::iterators::Pair<Rule>,
) -> Result<WindRadii, ParseError> {
    let mut threshold_kt = 0u8;
    let mut ne = 0u16;
    let mut se = 0u16;
    let mut sw = 0u16;
    let mut nw = 0u16;

    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::radii_threshold => threshold_kt = p.as_str().parse().unwrap_or(0),
            Rule::radii_quad_ne   => ne = p.into_inner().next().unwrap().as_str().parse().unwrap_or(0),
            Rule::radii_quad_se   => se = p.into_inner().next().unwrap().as_str().parse().unwrap_or(0),
            Rule::radii_quad_sw   => sw = p.into_inner().next().unwrap().as_str().parse().unwrap_or(0),
            Rule::radii_quad_nw   => nw = p.into_inner().next().unwrap().as_str().parse().unwrap_or(0),
            _ => {}
        }
    }
    Ok(WindRadii { threshold_kt, ne_nm: ne, se_nm: se, sw_nm: sw, nw_nm: nw })
}
