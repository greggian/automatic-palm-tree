//! Parser for NHC Wind Speed Probabilities (wndprb / PWS) products.
//!
//! Grammar spec: `grammars/wndprb.pest`
//!
//! Probability values are integer percentages; `"X"` encodes < 1 % and is
//! stored as `0`.

use chrono::{DateTime, TimeZone, Utc};
use pest::Parser;
use pest_derive::Parser;

use nhc_types::{StormStatus, WindProbAdvisory, WindProbEntry, WindProbTable};

use crate::envelope::{
    build_wmo_header, extract_awips_id, extract_forecaster,
    parse_month_abbr, parse_status,
};
use crate::error::ParseError;

#[derive(Parser)]
#[grammar = "grammars/wndprb.pest"]
pub struct WndprbParser;

/// Parse a raw wndprb product string into a [`WindProbAdvisory`].
pub fn parse(raw: &str) -> Result<WindProbAdvisory, ParseError> {
    let text = raw.replace("\r\n", "\n").replace('\r', "\n");

    let awips_id   = extract_awips_id(&text)?;
    let originator = extract_originator(&text);
    let forecaster = extract_forecaster(&text);

    let pairs = WndprbParser::parse(Rule::wndprb, &text)
        .map_err(|e| ParseError::Grammar {
            product: "wndprb",
            detail: e.to_string(),
        })?;

    let mut advisory_number = String::new();
    let mut issued_utc: Option<DateTime<Utc>> = None;
    let mut storm_name = String::new();
    let mut status = StormStatus::TropicalStorm;
    let mut tables: Vec<WindProbTable> = Vec::new();
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
            Rule::prob_section => {
                tables.push(parse_prob_section(pair)?);
            }
            _ => {}
        }
    }

    let wmo = build_wmo_header(&awips_id, &originator, None, Some(issued_year))?;
    let issued_utc = issued_utc.ok_or(ParseError::MissingField { field: "issued_utc" })?;

    Ok(WindProbAdvisory {
        wmo,
        advisory_number,
        issued_utc,
        storm_name,
        status,
        tables,
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

fn parse_prob_section(
    pair: pest::iterators::Pair<Rule>,
) -> Result<WindProbTable, ParseError> {
    let mut threshold_kt = 0u8;
    let mut entries: Vec<WindProbEntry> = Vec::new();

    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::section_header => {
                for sp in p.into_inner() {
                    if sp.as_rule() == Rule::threshold_kt {
                        threshold_kt = sp.as_str().parse().unwrap_or(0);
                    }
                }
            }
            Rule::prob_row => {
                entries.push(parse_prob_row(p, threshold_kt)?);
            }
            _ => {}
        }
    }

    Ok(WindProbTable { threshold_kt, entries })
}

fn parse_prob_row(
    pair: pest::iterators::Pair<Rule>,
    _threshold_kt: u8,
) -> Result<WindProbEntry, ParseError> {
    let mut location = String::new();
    let mut lat = 0.0f32;
    let mut lon = 0.0f32;
    let mut probs = [0u8; 7];

    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::loc_line => {
                // loc_line children: loc_text, decimal, lat_dir, decimal, lon_dir
                // loc_text = "ACAPULCO         MX" (name words + 2-letter CC, possibly padded)
                let mut inner = p.into_inner();
                let raw_text = inner.next().unwrap().as_str(); // loc_text
                let lat_str  = inner.next().unwrap().as_str(); // decimal
                let lat_ns   = inner.next().unwrap().as_str(); // lat_dir "N"|"S"
                let lon_str  = inner.next().unwrap().as_str(); // decimal
                let lon_ew   = inner.next().unwrap().as_str(); // lon_dir "E"|"W"

                // Split loc_text by whitespace; last token is the 2-letter CC,
                // everything before is the name.
                let parts: Vec<&str> = raw_text.split_whitespace().collect();
                if parts.len() >= 2 {
                    let cc   = *parts.last().unwrap();
                    let name = parts[..parts.len() - 1].join(" ");
                    location = format!("{} {}", name, cc);
                } else {
                    location = raw_text.trim().to_string();
                }

                let lat_v: f32 = lat_str.parse().unwrap_or(0.0);
                let lon_v: f32 = lon_str.parse().unwrap_or(0.0);
                lat = if lat_ns == "S" { -lat_v } else { lat_v };
                lon = if lon_ew == "W" { -lon_v } else { lon_v };
            }
            Rule::prob_values_line => {
                let mut i = 0;
                for vp in p.into_inner() {
                    if i >= 7 { break; }
                    probs[i] = if vp.as_str() == "X" {
                        0
                    } else {
                        vp.as_str().parse().unwrap_or(0)
                    };
                    i += 1;
                }
            }
            _ => {}
        }
    }

    Ok(WindProbEntry {
        location,
        lat,
        lon,
        probs,
    })
}
