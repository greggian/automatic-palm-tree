//! Shared envelope helpers — detect product type, extract forecaster name,
//! build WmoHeader.  These are used by all four product parsers.

use nhc_types::{Basin, ProductType, WmoHeader};

use crate::error::ParseError;

/// Detect the product type from the AWIPS ID in the ZCZC line without doing a
/// full parse.  Called by [`crate::parse_advisory`] for dispatch.
pub fn detect_product_type(raw: &str) -> Result<ProductType, ParseError> {
    let awips_id = extract_awips_id(raw)?;
    product_type_from_awips(&awips_id)
}

/// Extract the AWIPS ID string from the ZCZC line.
pub fn extract_awips_id(raw: &str) -> Result<String, ParseError> {
    for line in raw.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("ZCZC ") {
            // ZCZC <AWIPS_ID> ALL
            if let Some(id) = rest.split_whitespace().next() {
                return Ok(id.to_uppercase());
            }
        }
    }
    Err(ParseError::NoZczc)
}

/// Parse AWIPS ID into (ProductType, Basin, storm_number).
pub fn parse_awips(awips_id: &str) -> Result<(ProductType, Basin, u8), ParseError> {
    let id = awips_id.to_uppercase();
    // MIA = NHC Miami, HFO = CPHC Honolulu, NFD = Weather Prediction Center
    let rest = id.strip_prefix("MIA")
        .or_else(|| id.strip_prefix("HFO"))
        .or_else(|| id.strip_prefix("NFD"))
        .ok_or_else(|| ParseError::UnknownAwips(awips_id.to_string()))?;

    let product_type = if let Some(r) = rest.strip_prefix("TCM") {
        ("TCM", r, ProductType::Tcm)
    } else if let Some(r) = rest.strip_prefix("TCP") {
        ("TCP", r, ProductType::Tcp)
    } else if let Some(r) = rest.strip_prefix("TCD") {
        ("TCD", r, ProductType::Tcd)
    } else if let Some(r) = rest.strip_prefix("PWS") {
        ("PWS", r, ProductType::Pws)
    } else {
        return Err(ParseError::UnknownAwips(awips_id.to_string()));
    };

    let (_, suffix, pt) = product_type;
    let (basin, num_str) = if let Some(r) = suffix.strip_prefix("EP") {
        (Basin::Ep, r)
    } else if let Some(r) = suffix.strip_prefix("AL") {
        (Basin::Al, r)
    } else if let Some(r) = suffix.strip_prefix("AT") {
        // Wind probability products use "AT" for Atlantic instead of "AL"
        (Basin::Al, r)
    } else if let Some(r) = suffix.strip_prefix("CP") {
        (Basin::Cp, r)
    } else {
        return Err(ParseError::UnknownAwips(awips_id.to_string()));
    };

    let storm_number: u8 = num_str.parse().map_err(|_| {
        ParseError::UnknownAwips(awips_id.to_string())
    })?;

    Ok((pt, basin, storm_number))
}

pub fn product_type_from_awips(awips_id: &str) -> Result<ProductType, ParseError> {
    Ok(parse_awips(awips_id)?.0)
}

/// Build a [`WmoHeader`] from parsed envelope fields.
pub fn build_wmo_header(
    awips_id: &str,
    originator: &str,
    storm_number_from_body: Option<u8>,
    year_from_body: Option<u16>,
) -> Result<WmoHeader, ParseError> {
    let (product_type, basin, storm_number_awips) = parse_awips(awips_id)?;
    let storm_number = storm_number_from_body.unwrap_or(storm_number_awips);
    let year = year_from_body.unwrap_or(0);
    let basin_str = match basin {
        Basin::Ep => "EP",
        Basin::Al => "AL",
        Basin::Cp => "CP",
    };
    let storm_id = format!("{}{:02}{}", basin_str, storm_number, year);

    Ok(WmoHeader {
        awips_id: awips_id.to_uppercase(),
        product_type,
        originator: originator.to_uppercase(),
        storm_id,
        basin,
        storm_number,
        year,
    })
}

/// Parse a signed latitude from a decimal string + N/S suffix character.
pub fn parse_lat(decimal: &str, ns: char) -> Result<f32, ParseError> {
    let v: f32 = decimal.parse().map_err(|_| ParseError::InvalidValue {
        field: "lat",
        value: decimal.to_string(),
    })?;
    Ok(if ns == 'S' { -v } else { v })
}

/// Parse a signed longitude from a decimal string + E/W suffix character.
pub fn parse_lon(decimal: &str, ew: char) -> Result<f32, ParseError> {
    let v: f32 = decimal.parse().map_err(|_| ParseError::InvalidValue {
        field: "lon",
        value: decimal.to_string(),
    })?;
    Ok(if ew == 'W' { -v } else { v })
}

/// Parse a `lat` token like `"13.5N"` into a signed float.
pub fn parse_lat_token(s: &str) -> Result<f32, ParseError> {
    let ns = s.chars().last().ok_or(ParseError::MissingField { field: "lat" })?;
    parse_lat(s.trim_end_matches(|c: char| c.is_alphabetic()), ns)
}

/// Parse a `lon` token like `"98.5W"` into a signed float.
pub fn parse_lon_token(s: &str) -> Result<f32, ParseError> {
    let ew = s.chars().last().ok_or(ParseError::MissingField { field: "lon" })?;
    parse_lon(s.trim_end_matches(|c: char| c.is_alphabetic()), ew)
}

/// Parse storm status string as found in product text.
pub fn parse_status(s: &str) -> Result<nhc_types::StormStatus, ParseError> {
    use nhc_types::StormStatus;
    match s.trim() {
        "POTENTIAL TROPICAL CYCLONE" | "POTENTIAL TROP CYCLONE" => Ok(StormStatus::PotentialTropicalCyclone),
        "SUBTROPICAL STORM"       => Ok(StormStatus::SubtropicalStorm),
        "TROPICAL DEPRESSION"     => Ok(StormStatus::TropicalDepression),
        "TROPICAL CYCLONE"        => Ok(StormStatus::TropicalCyclone),
        "TROPICAL STORM"          => Ok(StormStatus::TropicalStorm),
        "HURRICANE"               => Ok(StormStatus::Hurricane),
        "POST-TROPICAL CYCLONE" | "POST-TROPICAL" => Ok(StormStatus::PostTropical),
        "REMNANT LOW" | "REMNANTS OF" => Ok(StormStatus::RemnantLow),
        "DISSIPATED"              => Ok(StormStatus::Dissipated),
        other => Err(ParseError::UnknownStatus(other.to_string())),
    }
}

/// Extract forecaster name from lines after `$$`.
pub fn extract_forecaster(raw: &str) -> String {
    let mut after_body = false;
    for line in raw.lines() {
        let t = line.trim();
        if t == "$$" {
            after_body = true;
            continue;
        }
        if after_body {
            let up = t.to_uppercase();
            if let Some(rest) = up.strip_prefix("FORECASTER ") {
                return rest.trim().to_string();
            }
        }
    }
    "UNKNOWN".to_string()
}

// Month abbreviation → month number
pub fn parse_month_abbr(m: &str) -> Result<u32, ParseError> {
    match m.to_uppercase().as_str() {
        "JAN" => Ok(1),  "FEB" => Ok(2),  "MAR" => Ok(3),
        "APR" => Ok(4),  "MAY" => Ok(5),  "JUN" => Ok(6),
        "JUL" => Ok(7),  "AUG" => Ok(8),  "SEP" => Ok(9),
        "OCT" => Ok(10), "NOV" => Ok(11), "DEC" => Ok(12),
        other => Err(ParseError::InvalidValue {
            field: "month",
            value: other.to_string(),
        }),
    }
}
