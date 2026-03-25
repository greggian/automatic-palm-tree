//! Integration tests for all four NHC advisory parsers.
//!
//! Test data lives in `proto/test_data/` (mock advisories created from the
//! known NHC format; replace with real fetched data once network access is
//! available by running `python proto/scripts/fetch_test_data.py`).

use std::path::PathBuf;

use nhc_parser::{
    discus::parse as parse_discus,
    fstadv::parse as parse_fstadv,
    public::parse as parse_public,
    wndprb::parse as parse_wndprb,
    parse_advisory,
};
use nhc_types::{Advisory, Basin, ProductType, StormStatus};
use chrono::{DateTime, TimeZone, Utc};

fn test_data(name: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("proto/test_data");
    std::fs::read_to_string(root.join(name))
        .unwrap_or_else(|_| panic!("test_data/{name} not found"))
}

// ─────────────────────────────────────────────────────────────────────────────
// Envelope / dispatch
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_advisory_dispatches_fstadv() {
    let raw = test_data("ep042025.fstadv.001.txt");
    let adv = parse_advisory(&raw).expect("parse failed");
    assert!(matches!(adv, Advisory::Fstadv(_)));
}

#[test]
fn parse_advisory_dispatches_public() {
    let raw = test_data("ep042025.public.001.txt");
    let adv = parse_advisory(&raw).expect("parse failed");
    assert!(matches!(adv, Advisory::Public(_)));
}

#[test]
fn parse_advisory_dispatches_discus() {
    let raw = test_data("ep042025.discus.001.txt");
    let adv = parse_advisory(&raw).expect("parse failed");
    assert!(matches!(adv, Advisory::Discus(_)));
}

#[test]
fn parse_advisory_dispatches_wndprb() {
    let raw = test_data("ep042025.wndprb.001.txt");
    let adv = parse_advisory(&raw).expect("parse failed");
    assert!(matches!(adv, Advisory::Wndprb(_)));
}

// ─────────────────────────────────────────────────────────────────────────────
// WmoHeader
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn wmo_header_fstadv_001() {
    let raw = test_data("ep042025.fstadv.001.txt");
    let adv = parse_fstadv(&raw).expect("parse failed");
    let wmo = &adv.wmo;
    assert_eq!(wmo.awips_id, "MIATCMEP4");
    assert_eq!(wmo.product_type, ProductType::Tcm);
    assert_eq!(wmo.basin, Basin::Ep);
    assert_eq!(wmo.storm_number, 4);
    assert_eq!(wmo.year, 2025);
    assert_eq!(wmo.storm_id, "EP042025");
    assert_eq!(wmo.originator, "KNHC");
}

#[test]
fn wmo_header_public_001() {
    let raw = test_data("ep042025.public.001.txt");
    let adv = parse_public(&raw).expect("parse failed");
    assert_eq!(adv.wmo.product_type, ProductType::Tcp);
    assert_eq!(adv.wmo.awips_id, "MIATCPEP4");
}

#[test]
fn wmo_header_discus_001() {
    let raw = test_data("ep042025.discus.001.txt");
    let adv = parse_discus(&raw).expect("parse failed");
    assert_eq!(adv.wmo.product_type, ProductType::Tcd);
    assert_eq!(adv.wmo.awips_id, "MIATCDEP4");
}

#[test]
fn wmo_header_wndprb_001() {
    let raw = test_data("ep042025.wndprb.001.txt");
    let adv = parse_wndprb(&raw).expect("parse failed");
    assert_eq!(adv.wmo.product_type, ProductType::Pws);
    assert_eq!(adv.wmo.awips_id, "MIAPWSEP4");
}

// ─────────────────────────────────────────────────────────────────────────────
// fstadv — Advisory 1 (TS forming)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn fstadv_001_advisory_number() {
    let raw = test_data("ep042025.fstadv.001.txt");
    let adv = parse_fstadv(&raw).expect("parse failed");
    assert_eq!(adv.advisory_number, "1");
}

#[test]
fn fstadv_001_storm_name() {
    let raw = test_data("ep042025.fstadv.001.txt");
    let adv = parse_fstadv(&raw).expect("parse failed");
    assert_eq!(adv.storm_name, "FOUR-E");
}

#[test]
fn fstadv_001_status_tropical_storm() {
    let raw = test_data("ep042025.fstadv.001.txt");
    let adv = parse_fstadv(&raw).expect("parse failed");
    assert_eq!(adv.status, StormStatus::TropicalStorm);
}

#[test]
fn fstadv_001_issued_utc() {
    let raw = test_data("ep042025.fstadv.001.txt");
    let adv = parse_fstadv(&raw).expect("parse failed");
    let expected: DateTime<Utc> = Utc.with_ymd_and_hms(2025, 7, 9, 18, 0, 0).unwrap();
    assert_eq!(adv.issued_utc, expected);
}

#[test]
fn fstadv_001_lat() {
    let raw = test_data("ep042025.fstadv.001.txt");
    let adv = parse_fstadv(&raw).expect("parse failed");
    assert!((adv.lat - 13.5).abs() < 0.01, "lat={}", adv.lat);
}

#[test]
fn fstadv_001_lon_negative() {
    let raw = test_data("ep042025.fstadv.001.txt");
    let adv = parse_fstadv(&raw).expect("parse failed");
    assert!((adv.lon - (-98.5)).abs() < 0.01, "lon={}", adv.lon);
}

#[test]
fn fstadv_001_position_accuracy() {
    let raw = test_data("ep042025.fstadv.001.txt");
    let adv = parse_fstadv(&raw).expect("parse failed");
    assert_eq!(adv.position_accuracy_nm, 60);
}

#[test]
fn fstadv_001_movement() {
    let raw = test_data("ep042025.fstadv.001.txt");
    let adv = parse_fstadv(&raw).expect("parse failed");
    assert_eq!(adv.movement_cardinal, "WNW");
    assert_eq!(adv.movement_degrees, 285);
    assert_eq!(adv.movement_kt, 8);
}

#[test]
fn fstadv_001_pressure() {
    let raw = test_data("ep042025.fstadv.001.txt");
    let adv = parse_fstadv(&raw).expect("parse failed");
    assert_eq!(adv.pressure_mb, 999);
}

#[test]
fn fstadv_001_winds() {
    let raw = test_data("ep042025.fstadv.001.txt");
    let adv = parse_fstadv(&raw).expect("parse failed");
    assert_eq!(adv.max_wind_kt, 40);
    assert_eq!(adv.gust_kt, 50);
}

#[test]
fn fstadv_001_prev_position() {
    let raw = test_data("ep042025.fstadv.001.txt");
    let adv = parse_fstadv(&raw).expect("parse failed");
    let (t, lat, lon) = adv.prev_position.expect("no prev position");
    assert_eq!(t, "09/1200Z");
    assert!((lat - 13.5).abs() < 0.01);
    assert!((lon - (-97.5)).abs() < 0.01);
}

#[test]
fn fstadv_001_next_advisory() {
    let raw = test_data("ep042025.fstadv.001.txt");
    let adv = parse_fstadv(&raw).expect("parse failed");
    assert_eq!(adv.next_advisory_utc.as_deref(), Some("10/0000Z"));
}

#[test]
fn fstadv_001_forecaster() {
    let raw = test_data("ep042025.fstadv.001.txt");
    let adv = parse_fstadv(&raw).expect("parse failed");
    assert_eq!(adv.forecaster, "LATTO");
}

#[test]
fn fstadv_001_forecast_points_count() {
    let raw = test_data("ep042025.fstadv.001.txt");
    let adv = parse_fstadv(&raw).expect("parse failed");
    // 5 FORECAST VALID + 1 OUTLOOK VALID
    assert_eq!(adv.forecast_points.len(), 6);
}

#[test]
fn fstadv_001_first_forecast_point() {
    let raw = test_data("ep042025.fstadv.001.txt");
    let adv = parse_fstadv(&raw).expect("parse failed");
    let pt = &adv.forecast_points[0];
    assert_eq!(pt.valid_time, "10/0600Z");
    assert!(!pt.is_outlook);
    assert!((pt.lat.unwrap() - 14.2).abs() < 0.01);
    assert!((pt.lon.unwrap() - (-100.4)).abs() < 0.01);
    assert_eq!(pt.max_wind_kt, Some(45));
    assert_eq!(pt.gust_kt, Some(55));
}

#[test]
fn fstadv_001_first_forecast_34kt_radii() {
    let raw = test_data("ep042025.fstadv.001.txt");
    let adv = parse_fstadv(&raw).expect("parse failed");
    let pt = &adv.forecast_points[0];
    let r34 = pt.wind_radii.iter().find(|r| r.threshold_kt == 34)
        .expect("no 34kt radii");
    assert_eq!(r34.ne_nm, 120);
    assert_eq!(r34.se_nm, 120);
    assert_eq!(r34.sw_nm, 90);
    assert_eq!(r34.nw_nm, 90);
}

#[test]
fn fstadv_001_hurricane_forecast_has_64kt_radii() {
    // forecast point index 2 = 11/1800Z at 65kt → hurricane
    let raw = test_data("ep042025.fstadv.001.txt");
    let adv = parse_fstadv(&raw).expect("parse failed");
    let pt = &adv.forecast_points[2];
    assert_eq!(pt.max_wind_kt, Some(65));
    assert!(pt.wind_radii.iter().any(|r| r.threshold_kt == 64));
}

#[test]
fn fstadv_001_outlook_point_is_flagged() {
    let raw = test_data("ep042025.fstadv.001.txt");
    let adv = parse_fstadv(&raw).expect("parse failed");
    let outlook = adv.forecast_points.last().unwrap();
    assert!(outlook.is_outlook);
    assert_eq!(outlook.valid_time, "16/1800Z");
}

// ─────────────────────────────────────────────────────────────────────────────
// fstadv — Advisory 8 (Hurricane at peak)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn fstadv_008_status_hurricane() {
    let raw = test_data("ep042025.fstadv.008.txt");
    let adv = parse_fstadv(&raw).expect("parse failed");
    assert_eq!(adv.status, StormStatus::Hurricane);
    assert_eq!(adv.storm_name, "DALILA");
    assert_eq!(adv.advisory_number, "8");
    assert_eq!(adv.max_wind_kt, 80);
    assert_eq!(adv.gust_kt, 95);
    assert_eq!(adv.pressure_mb, 973);
}

#[test]
fn fstadv_008_position() {
    let raw = test_data("ep042025.fstadv.008.txt");
    let adv = parse_fstadv(&raw).expect("parse failed");
    assert!((adv.lat - 15.8).abs() < 0.01);
    assert!((adv.lon - (-107.2)).abs() < 0.01);
}

#[test]
fn fstadv_008_forecaster() {
    let raw = test_data("ep042025.fstadv.008.txt");
    let adv = parse_fstadv(&raw).expect("parse failed");
    assert_eq!(adv.forecaster, "BERG");
}

// ─────────────────────────────────────────────────────────────────────────────
// fstadv — Advisory 14 (weakening/dissipating)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn fstadv_014_dissipated_forecast_point() {
    let raw = test_data("ep042025.fstadv.014.txt");
    let adv = parse_fstadv(&raw).expect("parse failed");
    let dis = adv.forecast_points.iter()
        .find(|p| p.status == StormStatus::Dissipated)
        .expect("no dissipated point");
    assert!(dis.lat.is_none());
    assert!(dis.lon.is_none());
}

#[test]
fn fstadv_014_pressure_above_1000() {
    let raw = test_data("ep042025.fstadv.014.txt");
    let adv = parse_fstadv(&raw).expect("parse failed");
    assert_eq!(adv.pressure_mb, 1003);
}

// ─────────────────────────────────────────────────────────────────────────────
// public — Advisory 1
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn public_001_fields() {
    let raw = test_data("ep042025.public.001.txt");
    let adv = parse_public(&raw).expect("parse failed");
    assert_eq!(adv.advisory_number, "1");
    assert_eq!(adv.storm_name, "FOUR-E");
    assert_eq!(adv.status, StormStatus::TropicalStorm);
    assert_eq!(adv.max_wind_mph, 45);
    assert_eq!(adv.max_wind_kmh, 75);
    assert_eq!(adv.gust_mph, 55);
    assert_eq!(adv.gust_kmh, 89);
    assert_eq!(adv.pressure_mb, 999);
    assert_eq!(adv.movement_cardinal, "WNW");
    assert_eq!(adv.movement_degrees, 285);
    assert_eq!(adv.movement_mph, 9);
    assert_eq!(adv.movement_kmh, 15);
    assert_eq!(adv.next_advisory_utc.as_deref(), Some("10/0000Z"));
    assert_eq!(adv.forecaster, "LATTO");
}

#[test]
fn public_001_position() {
    let raw = test_data("ep042025.public.001.txt");
    let adv = parse_public(&raw).expect("parse failed");
    assert!((adv.lat - 13.5).abs() < 0.01);
    assert!((adv.lon - (-98.5)).abs() < 0.01);
}

#[test]
fn public_001_headline() {
    let raw = test_data("ep042025.public.001.txt");
    let adv = parse_public(&raw).expect("parse failed");
    let hl = adv.headline.expect("no headline");
    assert!(hl.contains("FOUR-E") || hl.contains("TROPICAL STORM"));
}

// ─────────────────────────────────────────────────────────────────────────────
// public — Advisory 8 (Hurricane)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn public_008_hurricane() {
    let raw = test_data("ep042025.public.008.txt");
    let adv = parse_public(&raw).expect("parse failed");
    assert_eq!(adv.status, StormStatus::Hurricane);
    assert_eq!(adv.max_wind_mph, 90);
    assert_eq!(adv.max_wind_kmh, 150);
    assert_eq!(adv.gust_mph, 110);
    assert_eq!(adv.gust_kmh, 175);
    assert_eq!(adv.pressure_mb, 973);
    assert_eq!(adv.forecaster, "BERG");
}

// ─────────────────────────────────────────────────────────────────────────────
// discus
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn discus_001_fields() {
    let raw = test_data("ep042025.discus.001.txt");
    let adv = parse_discus(&raw).expect("parse failed");
    assert_eq!(adv.advisory_number, "1");
    assert_eq!(adv.storm_name, "FOUR-E");
    assert_eq!(adv.status, StormStatus::TropicalStorm);
    assert!(!adv.body.is_empty());
    assert!(!adv.body.contains("$$"));
    assert!(!adv.body.contains("NNNN"));
    assert!(adv.body.to_uppercase().contains("FORECAST POSITIONS"));
    assert_eq!(adv.forecaster, "LATTO");
}

#[test]
fn discus_008_hurricane() {
    let raw = test_data("ep042025.discus.008.txt");
    let adv = parse_discus(&raw).expect("parse failed");
    assert_eq!(adv.status, StormStatus::Hurricane);
    assert!(adv.body.contains("80 kt") || adv.body.contains("80 KT"));
    assert_eq!(adv.forecaster, "BERG");
}

#[test]
fn discus_014_mentions_dissipate() {
    let raw = test_data("ep042025.discus.014.txt");
    let adv = parse_discus(&raw).expect("parse failed");
    assert!(adv.body.to_lowercase().contains("dissipat"));
}

// ─────────────────────────────────────────────────────────────────────────────
// wndprb
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn wndprb_001_three_tables() {
    let raw = test_data("ep042025.wndprb.001.txt");
    let adv = parse_wndprb(&raw).expect("parse failed");
    assert_eq!(adv.tables.len(), 3);
    let thresholds: Vec<u8> = adv.tables.iter().map(|t| t.threshold_kt).collect();
    assert!(thresholds.contains(&34));
    assert!(thresholds.contains(&50));
    assert!(thresholds.contains(&64));
}

#[test]
fn wndprb_001_acapulco_24hr() {
    let raw = test_data("ep042025.wndprb.001.txt");
    let adv = parse_wndprb(&raw).expect("parse failed");
    let t34 = adv.tables.iter().find(|t| t.threshold_kt == 34).unwrap();
    let entry = t34.entries.iter().find(|e| e.location.contains("ACAPULCO")).unwrap();
    // 24HR window is index 1
    assert_eq!(entry.probs[1], 50);
}

#[test]
fn wndprb_001_x_encodes_as_zero() {
    let raw = test_data("ep042025.wndprb.001.txt");
    let adv = parse_wndprb(&raw).expect("parse failed");
    let t34 = adv.tables.iter().find(|t| t.threshold_kt == 34).unwrap();
    let entry = t34.entries.iter().find(|e| e.location.contains("PUERTO ANGEL")).unwrap();
    // 12HR is "X" → 0
    assert_eq!(entry.probs[0], 0);
}

#[test]
fn wndprb_001_location_coordinates() {
    let raw = test_data("ep042025.wndprb.001.txt");
    let adv = parse_wndprb(&raw).expect("parse failed");
    let t34 = adv.tables.iter().find(|t| t.threshold_kt == 34).unwrap();
    let entry = t34.entries.iter().find(|e| e.location.contains("ACAPULCO")).unwrap();
    assert!((entry.lat - 16.9).abs() < 0.01, "lat={}", entry.lat);
    assert!((entry.lon - (-99.9)).abs() < 0.01, "lon={}", entry.lon);
}

#[test]
fn wndprb_001_64kt_has_fewer_locations() {
    let raw = test_data("ep042025.wndprb.001.txt");
    let adv = parse_wndprb(&raw).expect("parse failed");
    let t34 = adv.tables.iter().find(|t| t.threshold_kt == 34).unwrap();
    let t64 = adv.tables.iter().find(|t| t.threshold_kt == 64).unwrap();
    assert!(t64.entries.len() <= t34.entries.len());
}

#[test]
fn wndprb_008_hurricane_tables() {
    let raw = test_data("ep042025.wndprb.008.txt");
    let adv = parse_wndprb(&raw).expect("parse failed");
    assert_eq!(adv.status, StormStatus::Hurricane);
    assert_eq!(adv.tables.len(), 3);
    let t64 = adv.tables.iter().find(|t| t.threshold_kt == 64).unwrap();
    assert!(!t64.entries.is_empty());
}

#[test]
fn wndprb_014_all_zeros() {
    let raw = test_data("ep042025.wndprb.014.txt");
    let adv = parse_wndprb(&raw).expect("parse failed");
    for table in &adv.tables {
        for entry in &table.entries {
            assert!(entry.probs.iter().all(|&p| p == 0),
                "expected all zeros for {}", entry.location);
        }
    }
}
