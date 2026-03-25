//! Parser for NHC Forecast Discussion (discus / TCD) products.
//!
//! Grammar spec: `grammars/discus.pest`
//!
//! The discussion body is captured verbatim; only the envelope and
//! header/issued-time anchor lines are fully parsed.

use chrono::{DateTime, TimeZone, Utc};
use pest::Parser;
use pest_derive::Parser;

use nhc_types::{ForecastDiscussion, StormStatus};

use crate::envelope::{
    build_wmo_header, extract_awips_id, extract_forecaster,
    parse_month_abbr, parse_status,
};
use crate::error::ParseError;

#[derive(Parser)]
#[grammar = "grammars/discus.pest"]
pub struct DiscusParser;

/// Parse a raw forecast discussion product string into a [`ForecastDiscussion`].
pub fn parse(raw: &str) -> Result<ForecastDiscussion, ParseError> {
    let text = raw.replace("\r\n", "\n").replace('\r', "\n");

    let awips_id   = extract_awips_id(&text)?;
    let originator = extract_originator(&text);
    let forecaster = extract_forecaster(&text);

    let pairs = DiscusParser::parse(Rule::discus, &text)
        .map_err(|e| ParseError::Grammar {
            product: "discus",
            detail: e.to_string(),
        })?;

    let mut advisory_number = String::new();
    let mut issued_utc: Option<DateTime<Utc>> = None;
    let mut storm_name = String::new();
    let mut status = StormStatus::TropicalStorm;
    let mut body = String::new();
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
            Rule::body => {
                body = pair.as_str().trim().to_string();
            }
            _ => {}
        }
    }

    let wmo = build_wmo_header(&awips_id, &originator, None, Some(issued_year))?;
    let issued_utc = issued_utc.ok_or(ParseError::MissingField { field: "issued_utc" })?;

    Ok(ForecastDiscussion {
        wmo,
        advisory_number,
        issued_utc,
        storm_name,
        status,
        body,
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

/// Parse an issued-time line that may be UTC ("1800 UTC WED JUL 09 2025")
/// or local time ("1100 AM AST TUE JUN 24 2025").
/// Returns (DateTime<Utc>, year).  For local-time lines the UTC hour is
/// approximate (we use the nominal local hour as-is).
fn parse_issued_from_str(s: &str) -> (DateTime<Utc>, u16) {
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
