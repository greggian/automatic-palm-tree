"""Tests for the wind speed probability (PWS) parser."""
from __future__ import annotations

from datetime import datetime, timezone

import pytest
from nhc_parser.ast_types import StormStatus, WindProbAdvisory, WindProbEntry, WindProbTable
from nhc_parser.wndprb import parse_wndprb


class TestWndprbAdvisory1:
    @pytest.fixture(autouse=True)
    def _adv(self, wndprb_001):
        self.adv = parse_wndprb(wndprb_001)

    def test_returns_wind_prob_advisory(self):
        assert isinstance(self.adv, WindProbAdvisory)

    def test_advisory_number(self):
        assert self.adv.advisory_number == "1"

    def test_storm_name(self):
        assert self.adv.storm_name == "FOUR-E"

    def test_status(self):
        assert self.adv.status == StormStatus.TROPICAL_STORM

    def test_issued_utc(self):
        expected = datetime(2025, 7, 9, 18, 0, tzinfo=timezone.utc)
        assert self.adv.issued_utc == expected

    def test_three_tables(self):
        assert len(self.adv.tables) == 3

    def test_table_thresholds(self):
        thresholds = {t.threshold_kt for t in self.adv.tables}
        assert thresholds == {34, 50, 64}

    def test_34kt_table_has_entries(self):
        t34 = next(t for t in self.adv.tables if t.threshold_kt == 34)
        assert len(t34.entries) >= 3

    def test_acapulco_in_34kt(self):
        t34 = next(t for t in self.adv.tables if t.threshold_kt == 34)
        names = [e.location for e in t34.entries]
        assert any("ACAPULCO" in n for n in names)

    def test_acapulco_24hr_prob(self):
        t34 = next(t for t in self.adv.tables if t.threshold_kt == 34)
        acapulco = next(e for e in t34.entries if "ACAPULCO" in e.location)
        assert acapulco.probs[34]["24HR"] == 50

    def test_x_becomes_zero(self):
        t34 = next(t for t in self.adv.tables if t.threshold_kt == 34)
        puerto_angel = next(e for e in t34.entries if "PUERTO ANGEL" in e.location)
        # 12HR is "X" → 0
        assert puerto_angel.probs[34]["12HR"] == 0

    def test_location_lat_lon(self):
        t34 = next(t for t in self.adv.tables if t.threshold_kt == 34)
        acapulco = next(e for e in t34.entries if "ACAPULCO" in e.location)
        assert acapulco.lat == pytest.approx(16.9)
        assert acapulco.lon == pytest.approx(-99.9)

    def test_64kt_table_has_fewer_locations(self):
        t34 = next(t for t in self.adv.tables if t.threshold_kt == 34)
        t64 = next(t for t in self.adv.tables if t.threshold_kt == 64)
        assert len(t64.entries) <= len(t34.entries)

    def test_forecaster(self):
        assert self.adv.forecaster == "LATTO"

    def test_wmo_product_type(self):
        assert self.adv.wmo.product_type == "PWS"


class TestWndprbAdvisory8:
    @pytest.fixture(autouse=True)
    def _adv(self, wndprb_008):
        self.adv = parse_wndprb(wndprb_008)

    def test_status_hurricane(self):
        assert self.adv.status == StormStatus.HURRICANE

    def test_storm_name(self):
        assert self.adv.storm_name == "DALILA"

    def test_advisory_number(self):
        assert self.adv.advisory_number == "8"

    def test_three_tables(self):
        assert len(self.adv.tables) == 3

    def test_64kt_table_has_entries(self):
        t64 = next(t for t in self.adv.tables if t.threshold_kt == 64)
        assert len(t64.entries) >= 1

    def test_manzanillo_34kt_48hr(self):
        t34 = next(t for t in self.adv.tables if t.threshold_kt == 34)
        manz = next(e for e in t34.entries if "MANZANILLO" in e.location)
        assert manz.probs[34]["48HR"] == 65

    def test_zihuatanejo_lat(self):
        t34 = next(t for t in self.adv.tables if t.threshold_kt == 34)
        zihua = next(e for e in t34.entries if "ZIHUATANEJO" in e.location)
        assert zihua.lat == pytest.approx(17.6)
        assert zihua.lon == pytest.approx(-101.5)

    def test_forecaster(self):
        assert self.adv.forecaster == "BERG"

    def test_issued_utc(self):
        expected = datetime(2025, 7, 12, 18, 0, tzinfo=timezone.utc)
        assert self.adv.issued_utc == expected


class TestWndprbAdvisory14:
    @pytest.fixture(autouse=True)
    def _adv(self, wndprb_014):
        self.adv = parse_wndprb(wndprb_014)

    def test_status(self):
        assert self.adv.status == StormStatus.TROPICAL_STORM

    def test_advisory_number(self):
        assert self.adv.advisory_number == "14"

    def test_three_tables_present(self):
        assert len(self.adv.tables) == 3

    def test_all_probs_are_zero_or_low(self):
        """Advisory 14 is a weakening system far from land; all probs should be 0."""
        for table in self.adv.tables:
            for entry in table.entries:
                for window, pct in entry.probs[table.threshold_kt].items():
                    assert pct == 0, f"{entry.location} {table.threshold_kt}kt {window}={pct}"

    def test_forecaster(self):
        assert self.adv.forecaster == "ROBERTS"
