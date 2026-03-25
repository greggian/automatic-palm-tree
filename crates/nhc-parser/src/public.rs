//! Parser for NHC Public Advisory (public / TCP) products.
//!
//! Grammar spec: `grammars/public.pest`
//!
//! Values are in **mph and km/h** as issued (not knots).

use chrono::{DateTime, Utc};
use pest::Parser;
use pest_derive::Parser;

use nhc_types::{PublicAdvisory, StormStatus};

use crate::envelope::{
    build_wmo_header, extract_awips_id, extract_forecaster,
    parse_lat_token, parse_lon_token, parse_month_abbr, parse_status,
};

/// Scan text for a `DD/HHmmZ` time reference and return the first match.
fn find_time_ref(text: &str) -> Option<String> {
    for part in text.split_whitespace() {
        let p = part.trim_end_matches('.');
        if p.len() == 8 && p.contains('/') && p.ends_with('Z') {
            let sides: Vec<&str> = p.splitn(2, '/').collect();
            if sides.len() == 2
                && sides[0].len() == 2
                && sides[0].chars().all(|c| c.is_ascii_digit())
                && sides[1].len() == 5
                && sides[1][..4].chars().all(|c| c.is_ascii_digit())
            {
                return Some(p.to_string());
            }
        }
    }
    None
}

/// Parse an issued-time line that may be UTC or local time.
fn parse_issued_from_str(s: &str) -> (chrono::DateTime<chrono::Utc>, u16) {
    use chrono::{TimeZone, Utc};
    let upper = s.trim().to_uppercase();
    let parts: Vec<&str> = upper.split_whitespace().collect();

    let year: i32 = parts.iter()
        .filter_map(|p| p.parse().ok())
        .find(|&y: &i32| y >= 2000 && y <= 2100)
        .unwrap_or(0);

    let month: u32 = parts.iter()
        .find_map(|p| parse_month_abbr(p).ok())
        .unwrap_or(1);

    let day: u32 = parts.iter()
        .filter_map(|p| p.parse::<u32>().ok())
        .find(|&d| d >= 1 && d <= 31)
        .unwrap_or(1);

    let hhmm_str = parts.iter()
        .find(|p| p.len() == 4 && p.chars().all(|c| c.is_ascii_digit()));
    let (hour, min) = if let Some(t) = hhmm_str {
        (t[..2].parse::<u32>().unwrap_or(0), t[2..].parse::<u32>().unwrap_or(0))
    } else {
        (0, 0)
    };

    let dt = Utc.with_ymd_and_hms(year, month, day, hour, min, 0)
        .single()
        .unwrap_or_else(|| Utc.with_ymd_and_hms(year.max(1970), 1, 1, 0, 0, 0).unwrap());

    (dt, year as u16)
}
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
                let (dt, yr) = parse_issued_from_str(pair.as_str());
                issued_utc = Some(dt);
                issued_year = yr;
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
                if let Some(first) = inner.next() {
                    if first.as_rule() == Rule::cardinal {
                        movement_cardinal = first.as_str().to_string();
                        movement_degrees  = inner.next().unwrap().as_str().parse().unwrap_or(0);
                        movement_mph      = inner.next().unwrap().as_str().parse().unwrap_or(0);
                        movement_kmh      = inner.next().unwrap().as_str().parse().unwrap_or(0);
                    }
                    // else: STATIONARY — all movement fields stay at defaults (0)
                }
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
                next_advisory_utc = find_time_ref(pair.as_str());
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
