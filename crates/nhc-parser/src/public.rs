//! Parser for NHC Public Advisory (public / TCP) products.
//!
//! Grammar spec: `grammars/public.pest`
//!
//! Values are in **mph and km/h** as issued (not knots).

use chrono::{DateTime, TimeZone, Utc};
use pest::Parser;
use pest_derive::Parser;

use nhc_types::{PublicAdvisory, StormStatus};

use crate::envelope::{
    build_wmo_header, extract_awips_id, extract_forecaster,
    parse_lat_token, parse_lon_token, parse_month_abbr, parse_status,
};
use crate::error::ParseError;

#[derive(Parser)]
#[grammar = "grammars/public.pest"]
pub struct PublicParser;

/// Parse a raw public advisory product string into a [`PublicAdvisory`].
pub fn parse(raw: &str) -> Result<PublicAdvisory, ParseError> {
    let text = raw.replace("\r\n", "\n").replace('\r', "\n");

    let awips_id  = extract_awips_id(&text)?;
    let originator = extract_originator(&text);
    let forecaster = extract_forecaster(&text);

    let pairs = PublicParser::parse(Rule::public_adv, &text)
        .map_err(|e| ParseError::Grammar {
            product: "public",
            detail: e.to_string(),
        })?;

    let mut advisory_number = String::new();
    let mut issued_utc: Option<DateTime<Utc>> = None;
    let mut storm_name = String::new();
    let mut status = StormStatus::TropicalStorm;
    let mut lat = 0.0f32;
    let mut lon = 0.0f32;
    let mut max_wind_mph = 0u16;
    let mut max_wind_kmh = 0u16;
    let mut gust_mph = 0u16;
    let mut gust_kmh = 0u16;
    let mut movement_degrees = 0u16;
    let mut movement_cardinal = String::new();
    let mut movement_mph = 0u16;
    let mut movement_kmh = 0u16;
    let mut pressure_mb = 0u16;
    let mut headline: Option<String> = None;
    let mut next_advisory_utc: Option<String> = None;
    let mut issued_year = 0u16;

    for pair in pairs.into_iter().next().unwrap().into_inner() {
        match pair.as_rule() {
            Rule::header_line => {
                let mut inner = pair.into_inner();
                status = parse_status(inner.next().unwrap().as_str())?;
                storm_name = inner.next().unwrap().as_str().to_string();
                for p in inner {
                    if p.as_rule() == Rule::advisory_num {
                        advisory_number = p.as_str().to_string();
                        break;
                    }
                }
            }
            Rule::issued_line => {
                issued_utc = Some(parse_issued_line(&pair)?);
                for p in pair.into_inner() {
                    if p.as_rule() == Rule::issued_year {
                        issued_year = p.as_str().parse().unwrap_or(0);
                    }
                }
            }
            Rule::headline => {
                let text_pair = pair.into_inner().next();
                if let Some(tp) = text_pair {
                    headline = Some(tp.as_str().to_string());
                }
            }
            Rule::location_line => {
                let mut inner = pair.into_inner();
                lat = parse_lat_token(inner.next().unwrap().as_str())?;
                lon = parse_lon_token(inner.next().unwrap().as_str())?;
            }
            Rule::wind_line => {
                let mut inner = pair.into_inner();
                max_wind_mph = inner.next().unwrap().as_str().parse().unwrap_or(0);
                max_wind_kmh = inner.next().unwrap().as_str().parse().unwrap_or(0);
            }
            Rule::movement_line => {
                let mut inner = pair.into_inner();
                movement_cardinal = inner.next().unwrap().as_str().to_string();
                movement_degrees  = inner.next().unwrap().as_str().parse().unwrap_or(0);
                movement_mph      = inner.next().unwrap().as_str().parse().unwrap_or(0);
                movement_kmh      = inner.next().unwrap().as_str().parse().unwrap_or(0);
            }
            Rule::pressure_line => {
                let mut inner = pair.into_inner();
                pressure_mb = inner.next().unwrap().as_str().parse().unwrap_or(0);
            }
            Rule::body_prose => {
                // Scan for gust line inside the prose
                if let Some((mph, kmh)) = extract_gusts(pair.as_str()) {
                    gust_mph = mph;
                    gust_kmh = kmh;
                }
            }
            Rule::next_advisory_section => {
                for p in pair.into_inner() {
                    if p.as_rule() == Rule::next_advisory_line {
                        for inner_p in p.into_inner() {
                            if inner_p.as_rule() == Rule::time_ref {
                                next_advisory_utc = Some(inner_p.as_str().to_string());
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let wmo = build_wmo_header(&awips_id, &originator, None, Some(issued_year))?;
    let issued_utc = issued_utc.ok_or(ParseError::MissingField { field: "issued_utc" })?;

    Ok(PublicAdvisory {
        wmo,
        advisory_number,
        issued_utc,
        storm_name,
        status,
        lat,
        lon,
        movement_degrees,
        movement_cardinal,
        movement_mph,
        movement_kmh,
        pressure_mb,
        max_wind_mph,
        max_wind_kmh,
        gust_mph,
        gust_kmh,
        headline,
        next_advisory_utc,
        forecaster,
    })
}

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

/// Scan body prose for "WITH GUSTS TO N MPH (M KM/H)" pattern.
/// The value may appear on the next line after "TO" in actual advisories.
fn extract_gusts(prose: &str) -> Option<(u16, u16)> {
    let upper = prose.to_uppercase();
    let marker = "WITH GUSTS TO";
    let start = upper.find(marker)?;
    let rest = &prose[start + marker.len()..];
    let mut parts = rest.split_whitespace();
    let mph_str = parts.next()?;
    let mph: u16 = mph_str.parse().ok()?;
    // skip "MPH" and "("
    let _ = parts.next(); // "MPH"
    let kmh_part = parts.next()?; // "(89" or "89"
    let kmh_str = kmh_part.trim_start_matches('(');
    let kmh: u16 = kmh_str.parse().ok()?;
    Some((mph, kmh))
}
