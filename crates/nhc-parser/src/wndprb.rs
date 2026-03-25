//! Parser for NHC Wind Speed Probabilities (wndprb / PWS) products.
//!
//! Grammar spec: `grammars/wndprb.pest`
//!
//! The 2025 format uses a flat table with entries of the form:
//!   LOCATION_NAME  KT  OP  OP(CP)  OP(CP)  ...
//! where OP is onset probability and CP is cumulative probability.
//! We store CP values (and OP for the first 12-hr window where CP is absent).

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
            Rule::body => {
                tables = parse_entries_from_body(pair.as_str());
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

// ── Body parsing for the flat OP(CP) probability table ───────────────────────

/// Parse all probability entries from the body text and group by KT threshold.
fn parse_entries_from_body(body: &str) -> Vec<WindProbTable> {
    // Collect (kt, entry) pairs in order, then group by kt preserving order.
    let mut by_kt: std::collections::BTreeMap<u8, Vec<WindProbEntry>> =
        std::collections::BTreeMap::new();

    for raw_line in body.lines() {
        let line = raw_line.trim_end();
        if line.is_empty() { continue; }

        for kt in [34u8, 50, 64] {
            if let Some((location, probs)) = try_parse_entry_line(line, kt) {
                by_kt.entry(kt).or_default().push(WindProbEntry {
                    location,
                    lat: 0.0,
                    lon: 0.0,
                    probs,
                });
                break;
            }
        }
    }

    by_kt.into_iter()
        .map(|(kt, entries)| WindProbTable { threshold_kt: kt, entries })
        .collect()
}

/// Try to parse a line of the form "LOCATION_NAME  KT  OP  OP(CP)  ...".
/// Returns `None` if the line doesn't look like a probability entry.
fn try_parse_entry_line(line: &str, kt: u8) -> Option<(String, [u8; 7])> {
    // Look for " KT  " (space + 2-digit threshold + at least two spaces).
    // This avoids matching "34 KT" in the intro text (followed by " KT ").
    let pattern = format!(" {}  ", kt);
    let kt_pos = line.find(&pattern)?;

    let location = line[..kt_pos].trim();
    // Location must start with an alphanumeric character (filters out "..." intro lines).
    if location.is_empty()
        || !location.chars().next().map(|c| c.is_ascii_alphanumeric()).unwrap_or(false)
    {
        return None;
    }

    let prob_str = &line[kt_pos + pattern.len()..];
    let probs = parse_prob_tokens(prob_str.trim());
    Some((location.to_string(), probs))
}

/// Parse probability tokens from a string of the form:
///   `X   X( X)   X( X)   X( X)   2( 2)   1( 3)   X( 3)`
///
/// The first token is OP-only (no cumulative); subsequent tokens are `OP(CP)`.
/// We store CP for windows 2–7 and OP for window 1.  `X` → 0.
fn parse_prob_tokens(s: &str) -> [u8; 7] {
    let mut probs = [0u8; 7];
    let tokens: Vec<&str> = s.split_whitespace().collect();
    let mut out_idx = 0;
    let mut tok_idx = 0;

    while tok_idx < tokens.len() && out_idx < 7 {
        let t = tokens[tok_idx];
        if t.ends_with('(') {
            // "OP(" — next token is "CP)"
            tok_idx += 1;
            if tok_idx < tokens.len() {
                let cp = tokens[tok_idx].trim_end_matches(')');
                probs[out_idx] = parse_prob_val(cp);
                out_idx += 1;
            }
        } else if t.contains('(') {
            // "OP(CP)" all in one token (rare, defensive)
            if let Some(start) = t.find('(') {
                let cp = t[start + 1..].trim_end_matches(')');
                probs[out_idx] = parse_prob_val(cp);
                out_idx += 1;
            }
        } else {
            // Plain OP value (first window has no cumulative)
            probs[out_idx] = parse_prob_val(t);
            out_idx += 1;
        }
        tok_idx += 1;
    }

    probs
}

fn parse_prob_val(s: &str) -> u8 {
    if s == "X" { 0 } else { s.parse().unwrap_or(0) }
}
