//! Typed data model for all four NHC tropical cyclone advisory products.
//!
//! This crate has no I/O dependencies — it contains only pure value types that
//! can be constructed by the parser, stored in the database, or serialised to
//! JSON without any coupling to either.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// Shared envelope types
// ─────────────────────────────────────────────────────────────────────────────

/// WMO/AWIPS product envelope — present in every advisory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WmoHeader {
    /// Full AWIPS ID, e.g. `"MIATCMEP4"`.
    pub awips_id: String,
    /// Parsed product type.
    pub product_type: ProductType,
    /// Originating office, e.g. `"KNHC"`.
    pub originator: String,
    /// Composite storm identifier, e.g. `"EP042025"`.
    pub storm_id: String,
    /// Oceanic basin.
    pub basin: Basin,
    /// Season number within basin (1–30).
    pub storm_number: u8,
    /// Four-digit year.
    pub year: u16,
}

/// NHC product type encoded in the AWIPS ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProductType {
    /// `TCM` — Forecast/Advisory (`fstadv`)
    Tcm,
    /// `TCP` — Public Advisory (`public`)
    Tcp,
    /// `TCD` — Forecast Discussion (`discus`)
    Tcd,
    /// `PWS` — Wind Speed Probabilities (`wndprb`)
    Pws,
}

/// Tropical cyclone basin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Basin {
    /// Eastern North Pacific
    Ep,
    /// North Atlantic
    Al,
    /// Central North Pacific
    Cp,
}

/// Operational status of the tropical cyclone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StormStatus {
    PotentialTropicalCyclone,
    TropicalCyclone,
    TropicalStorm,
    Hurricane,
    PostTropical,
    RemnantLow,
    Dissipated,
}

// ─────────────────────────────────────────────────────────────────────────────
// fstadv — Forecast/Advisory  (all values in knots and nautical miles)
// ─────────────────────────────────────────────────────────────────────────────

/// Wind radii for one threshold at the current position or a forecast point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindRadii {
    /// Wind threshold in knots (34, 50, or 64).
    pub threshold_kt: u8,
    pub ne_nm: u16,
    pub se_nm: u16,
    pub sw_nm: u16,
    pub nw_nm: u16,
}

/// One track/intensity point in the fstadv forecast.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForecastPoint {
    /// `"DD/HHmmZ"` string as issued, e.g. `"13/0600Z"`.
    pub valid_time: String,
    /// `true` for day-4/5 "OUTLOOK VALID" lines.
    pub is_outlook: bool,
    pub status: StormStatus,
    /// `None` when the forecast is DISSIPATED.
    pub lat: Option<f32>,
    /// Always negative (stored as west-negative).  `None` when DISSIPATED.
    pub lon: Option<f32>,
    pub max_wind_kt: Option<u16>,
    pub gust_kt: Option<u16>,
    pub wind_radii: Vec<WindRadii>,
}

/// Parsed Forecast/Advisory (fstadv / TCM) product.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForecastAdvisory {
    pub wmo: WmoHeader,
    /// Advisory number string, e.g. `"1"`, `"12A"`.
    pub advisory_number: String,
    pub issued_utc: DateTime<Utc>,
    pub storm_name: String,
    pub status: StormStatus,
    /// Current centre latitude (positive N).
    pub lat: f32,
    /// Current centre longitude (west-negative).
    pub lon: f32,
    pub position_accuracy_nm: u16,
    pub movement_degrees: u16,
    pub movement_cardinal: String,
    pub movement_kt: u8,
    pub pressure_mb: u16,
    pub max_wind_kt: u16,
    pub gust_kt: u16,
    /// Current 34/50/64-kt wind radii at the advisory time.
    pub wind_radii: Vec<WindRadii>,
    /// `(time_str, lat, lon)` of the previous 6-hourly position.
    pub prev_position: Option<(String, f32, f32)>,
    pub forecast_points: Vec<ForecastPoint>,
    /// `"DD/HHmmZ"` of the next scheduled advisory.
    pub next_advisory_utc: Option<String>,
    pub forecaster: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// public — Public Advisory (values in mph and km/h)
// ─────────────────────────────────────────────────────────────────────────────

/// Parsed Public Advisory (public / TCP) product.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PublicAdvisory {
    pub wmo: WmoHeader,
    pub advisory_number: String,
    pub issued_utc: DateTime<Utc>,
    pub storm_name: String,
    pub status: StormStatus,
    pub lat: f32,
    pub lon: f32,
    pub movement_degrees: u16,
    pub movement_cardinal: String,
    pub movement_mph: u16,
    pub movement_kmh: u16,
    pub pressure_mb: u16,
    pub max_wind_mph: u16,
    pub max_wind_kmh: u16,
    pub gust_mph: u16,
    pub gust_kmh: u16,
    /// Optional headline extracted from `"...TEXT..."` line.
    pub headline: Option<String>,
    pub next_advisory_utc: Option<String>,
    pub forecaster: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// discus — Forecast Discussion
// ─────────────────────────────────────────────────────────────────────────────

/// Parsed Forecast Discussion (discus / TCD) product.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForecastDiscussion {
    pub wmo: WmoHeader,
    pub advisory_number: String,
    pub issued_utc: DateTime<Utc>,
    pub storm_name: String,
    pub status: StormStatus,
    /// Full prose body preserved verbatim.
    pub body: String,
    pub forecaster: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// wndprb — Wind Speed Probabilities
// ─────────────────────────────────────────────────────────────────────────────

/// Time-window labels used as keys in probability tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProbWindow {
    Hr12,
    Hr24,
    Hr36,
    Hr48,
    Hr72,
    Hr96,
    Hr120,
}

impl ProbWindow {
    pub const ALL: [ProbWindow; 7] = [
        ProbWindow::Hr12,
        ProbWindow::Hr24,
        ProbWindow::Hr36,
        ProbWindow::Hr48,
        ProbWindow::Hr72,
        ProbWindow::Hr96,
        ProbWindow::Hr120,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ProbWindow::Hr12  => "12HR",
            ProbWindow::Hr24  => "24HR",
            ProbWindow::Hr36  => "36HR",
            ProbWindow::Hr48  => "48HR",
            ProbWindow::Hr72  => "72HR",
            ProbWindow::Hr96  => "96HR",
            ProbWindow::Hr120 => "120HR",
        }
    }
}

/// Probabilities for one location across all windows, for one threshold.
/// Values are integer percentages; `0` encodes the `"X"` (< 1 %) token.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindProbEntry {
    /// Location name as issued, e.g. `"ACAPULCO MX"`.
    pub location: String,
    pub lat: f32,
    /// West-negative longitude.
    pub lon: f32,
    /// Cumulative probability (%) per window; `0` == `"X"` (< 1 %).
    pub probs: [u8; 7],
}

/// One threshold section (34 / 50 / 64 kt) of a wndprb product.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindProbTable {
    /// Wind threshold in knots.
    pub threshold_kt: u8,
    pub entries: Vec<WindProbEntry>,
}

/// Parsed Wind Speed Probabilities (wndprb / PWS) product.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindProbAdvisory {
    pub wmo: WmoHeader,
    pub advisory_number: String,
    pub issued_utc: DateTime<Utc>,
    pub storm_name: String,
    pub status: StormStatus,
    /// Three tables: 34, 50, and 64 kt (in that order).
    pub tables: Vec<WindProbTable>,
    pub forecaster: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Unified enum for dispatch
// ─────────────────────────────────────────────────────────────────────────────

/// Any parsed NHC advisory product.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Advisory {
    Fstadv(ForecastAdvisory),
    Public(PublicAdvisory),
    Discus(ForecastDiscussion),
    Wndprb(WindProbAdvisory),
}

impl Advisory {
    pub fn wmo(&self) -> &WmoHeader {
        match self {
            Advisory::Fstadv(a) => &a.wmo,
            Advisory::Public(a)  => &a.wmo,
            Advisory::Discus(a)  => &a.wmo,
            Advisory::Wndprb(a)  => &a.wmo,
        }
    }

    pub fn advisory_number(&self) -> &str {
        match self {
            Advisory::Fstadv(a) => &a.advisory_number,
            Advisory::Public(a)  => &a.advisory_number,
            Advisory::Discus(a)  => &a.advisory_number,
            Advisory::Wndprb(a)  => &a.advisory_number,
        }
    }

    pub fn issued_utc(&self) -> DateTime<Utc> {
        match self {
            Advisory::Fstadv(a) => a.issued_utc,
            Advisory::Public(a)  => a.issued_utc,
            Advisory::Discus(a)  => a.issued_utc,
            Advisory::Wndprb(a)  => a.issued_utc,
        }
    }

    pub fn storm_name(&self) -> &str {
        match self {
            Advisory::Fstadv(a) => &a.storm_name,
            Advisory::Public(a)  => &a.storm_name,
            Advisory::Discus(a)  => &a.storm_name,
            Advisory::Wndprb(a)  => &a.storm_name,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Display impls (used by DB serialisation layer)
// ─────────────────────────────────────────────────────────────────────────────

impl std::fmt::Display for Basin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self { Basin::Ep => write!(f, "EP"), Basin::Al => write!(f, "AL"), Basin::Cp => write!(f, "CP") }
    }
}
impl std::fmt::Display for StormStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            StormStatus::PotentialTropicalCyclone => "POTENTIAL TROP CYCLONE",
            StormStatus::TropicalCyclone => "TROPICAL CYCLONE",
            StormStatus::TropicalStorm => "TROPICAL STORM",
            StormStatus::Hurricane => "HURRICANE",
            StormStatus::PostTropical => "POST-TROPICAL",
            StormStatus::RemnantLow => "REMNANT LOW",
            StormStatus::Dissipated => "DISSIPATED",
        };
        write!(f, "{s}")
    }
}
impl std::fmt::Display for ProductType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self { ProductType::Tcm => write!(f, "TCM"), ProductType::Tcp => write!(f, "TCP"), ProductType::Tcd => write!(f, "TCD"), ProductType::Pws => write!(f, "PWS") }
    }
}
