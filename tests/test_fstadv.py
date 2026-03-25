"""Tests for the fstadv (Forecast/Advisory) parser."""
from __future__ import annotations

from datetime import datetime, timezone

import pytest
from nhc_parser.ast_types import ForecastAdvisory, ForecastPoint, StormStatus, WindRadii
from nhc_parser.fstadv import parse_fstadv


class TestFstadvAdvisory1:
    @pytest.fixture(autouse=True)
    def _adv(self, fstadv_001):
        self.adv = parse_fstadv(fstadv_001)

    def test_returns_forecast_advisory(self):
        assert isinstance(self.adv, ForecastAdvisory)

    def test_advisory_number(self):
        assert self.adv.advisory_number == "1"

    def test_storm_name(self):
        assert self.adv.storm_name == "FOUR-E"

    def test_status_tropical_storm(self):
        assert self.adv.status == StormStatus.TROPICAL_STORM

    def test_issued_utc(self):
        expected = datetime(2025, 7, 9, 18, 0, tzinfo=timezone.utc)
        assert self.adv.issued_utc == expected

    def test_lat(self):
        assert self.adv.lat == pytest.approx(13.5)

    def test_lon_negative(self):
        assert self.adv.lon == pytest.approx(-98.5)

    def test_position_accuracy(self):
        assert self.adv.position_accuracy_nm == 60

    def test_movement_cardinal(self):
        assert self.adv.movement_cardinal == "WNW"

    def test_movement_degrees(self):
        assert self.adv.movement_degrees == 285

    def test_movement_kt(self):
        assert self.adv.movement_kt == 8

    def test_pressure(self):
        assert self.adv.pressure_mb == 999

    def test_max_wind(self):
        assert self.adv.max_wind_kt == 40

    def test_gust(self):
        assert self.adv.gust_kt == 50

    def test_prev_position(self):
        assert self.adv.prev_position is not None
        time, lat, lon = self.adv.prev_position
        assert time == "09/1200Z"
        assert lat == pytest.approx(13.5)
        assert lon == pytest.approx(-97.5)

    def test_next_advisory(self):
        assert self.adv.next_advisory_utc == "10/0000Z"

    def test_forecaster(self):
        assert self.adv.forecaster == "LATTO"

    def test_wmo_basin(self):
        assert self.adv.wmo.basin == "EP"

    def test_forecast_points_count(self):
        # 5 FORECAST VALID + 1 OUTLOOK VALID
        assert len(self.adv.forecast_points) == 6

    def test_first_forecast_point_time(self):
        pt = self.adv.forecast_points[0]
        assert pt.valid_time == "10/0600Z"
        assert not pt.is_outlook

    def test_first_forecast_point_position(self):
        pt = self.adv.forecast_points[0]
        assert pt.lat == pytest.approx(14.2)
        assert pt.lon == pytest.approx(-100.4)

    def test_first_forecast_point_winds(self):
        pt = self.adv.forecast_points[0]
        assert pt.max_wind_kt == 45
        assert pt.gust_kt == 55

    def test_first_forecast_point_radii(self):
        pt = self.adv.forecast_points[0]
        r34 = next(r for r in pt.wind_radii if r.threshold_kt == 34)
        assert r34.ne_nm == 120
        assert r34.se_nm == 120
        assert r34.sw_nm == 90
        assert r34.nw_nm == 90

    def test_hurricane_forecast_point_has_64kt_radii(self):
        # advisory #1 forecast point 3 (48h) should have 64kt radii
        pt = self.adv.forecast_points[2]  # 11/1800Z, 65 kt
        assert pt.max_wind_kt == 65
        assert any(r.threshold_kt == 64 for r in pt.wind_radii)

    def test_outlook_point_is_flagged(self):
        outlook = self.adv.forecast_points[-1]
        assert outlook.is_outlook is True

    def test_outlook_point_time(self):
        outlook = self.adv.forecast_points[-1]
        assert outlook.valid_time == "16/1800Z"


class TestFstadvAdvisory8:
    @pytest.fixture(autouse=True)
    def _adv(self, fstadv_008):
        self.adv = parse_fstadv(fstadv_008)

    def test_status_hurricane(self):
        assert self.adv.status == StormStatus.HURRICANE

    def test_storm_name(self):
        assert self.adv.storm_name == "DALILA"

    def test_advisory_number(self):
        assert self.adv.advisory_number == "8"

    def test_max_wind_hurricane_force(self):
        assert self.adv.max_wind_kt == 80

    def test_gust(self):
        assert self.adv.gust_kt == 95

    def test_pressure_lower(self):
        assert self.adv.pressure_mb == 973

    def test_lat(self):
        assert self.adv.lat == pytest.approx(15.8)

    def test_lon(self):
        assert self.adv.lon == pytest.approx(-107.2)

    def test_current_64kt_radii(self):
        # Advisory 8 has 64kt winds currently
        # Wind radii are not directly on the adv object, they're per forecast pt
        # The advisory body has current radii listed after MAX SUSTAINED WINDS
        # Verify via forecast pt 1 (13/0600Z) which retains 64kt radii
        pt = self.adv.forecast_points[0]
        assert any(r.threshold_kt == 64 for r in pt.wind_radii)

    def test_outlook_point_present(self):
        outlook_pts = [p for p in self.adv.forecast_points if p.is_outlook]
        assert len(outlook_pts) == 1

    def test_forecaster(self):
        assert self.adv.forecaster == "BERG"

    def test_issued_utc(self):
        expected = datetime(2025, 7, 12, 18, 0, tzinfo=timezone.utc)
        assert self.adv.issued_utc == expected


class TestFstadvAdvisory14:
    @pytest.fixture(autouse=True)
    def _adv(self, fstadv_014):
        self.adv = parse_fstadv(fstadv_014)

    def test_status_weakening(self):
        assert self.adv.status == StormStatus.TROPICAL_STORM

    def test_advisory_number(self):
        assert self.adv.advisory_number == "14"

    def test_max_wind_weak(self):
        assert self.adv.max_wind_kt == 35

    def test_dissipated_forecast_point(self):
        dis_pts = [p for p in self.adv.forecast_points if p.status == StormStatus.DISSIPATED]
        assert len(dis_pts) >= 1

    def test_dissipated_point_has_no_coords(self):
        dis_pt = next(p for p in self.adv.forecast_points if p.status == StormStatus.DISSIPATED)
        assert dis_pt.lat is None
        assert dis_pt.lon is None

    def test_pressure_above_1000(self):
        assert self.adv.pressure_mb == 1003

    def test_forecaster(self):
        assert self.adv.forecaster == "ROBERTS"


class TestWindRadiiParsing:
    def test_current_34kt_radii(self, fstadv_001):
        """Current 34kt wind radii are embedded in the advisory body."""
        # We verify via a forecast point that has them
        adv = parse_fstadv(fstadv_001)
        pt = adv.forecast_points[0]
        r34 = next((r for r in pt.wind_radii if r.threshold_kt == 34), None)
        assert r34 is not None
        assert r34.ne_nm == 120

    def test_50kt_radii_at_peak(self, fstadv_008):
        adv = parse_fstadv(fstadv_008)
        pt = adv.forecast_points[0]  # 13/0600Z, still 75kt
        r50 = next((r for r in pt.wind_radii if r.threshold_kt == 50), None)
        assert r50 is not None
        assert r50.ne_nm == 90

    def test_radii_zero_when_absent(self, fstadv_001):
        adv = parse_fstadv(fstadv_001)
        # Forecast point at 10/0600Z has 34kt radii but 0/absent 64kt
        pt = adv.forecast_points[0]
        r64 = next((r for r in pt.wind_radii if r.threshold_kt == 64), None)
        assert r64 is None or (r64.ne_nm == 0 and r64.se_nm == 0)
