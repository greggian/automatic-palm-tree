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
                issued_utc = Some(parse_issued_line(&pair)?);
                for p in pair.into_inner() {
                    if p.as_rule() == Rule::issued_year {
                        issued_year = p.as_str().parse().unwrap_or(0);
                    }
                }
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
