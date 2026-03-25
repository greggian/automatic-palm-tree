"""Tests for the public advisory (TCP) parser."""
from __future__ import annotations

from datetime import datetime, timezone

import pytest
from nhc_parser.ast_types import PublicAdvisory, StormStatus
from nhc_parser.public import parse_public


class TestPublicAdvisory1:
    @pytest.fixture(autouse=True)
    def _adv(self, public_001):
        self.adv = parse_public(public_001)

    def test_returns_public_advisory(self):
        assert isinstance(self.adv, PublicAdvisory)

    def test_advisory_number(self):
        assert self.adv.advisory_number == "1"

    def test_storm_name(self):
        assert self.adv.storm_name == "FOUR-E"

    def test_status(self):
        assert self.adv.status == StormStatus.TROPICAL_STORM

    def test_issued_utc(self):
        expected = datetime(2025, 7, 9, 18, 0, tzinfo=timezone.utc)
        assert self.adv.issued_utc == expected

    def test_lat(self):
        assert self.adv.lat == pytest.approx(13.5)

    def test_lon_negative(self):
        assert self.adv.lon == pytest.approx(-98.5)

    def test_max_wind_mph(self):
        assert self.adv.max_wind_mph == 45

    def test_max_wind_kmh(self):
        assert self.adv.max_wind_kmh == 75

    def test_gust_mph(self):
        assert self.adv.gust_mph == 55

    def test_gust_kmh(self):
        assert self.adv.gust_kmh == 89

    def test_movement_cardinal(self):
        assert self.adv.movement_cardinal == "WNW"

    def test_movement_degrees(self):
        assert self.adv.movement_degrees == 285

    def test_movement_mph(self):
        assert self.adv.movement_mph == 9

    def test_movement_kmh(self):
        assert self.adv.movement_kmh == 15

    def test_pressure(self):
        assert self.adv.pressure_mb == 999

    def test_headline_present(self):
        assert self.adv.headline is not None
        assert "FOUR-E" in self.adv.headline

    def test_next_advisory(self):
        assert self.adv.next_advisory_utc == "10/0000Z"

    def test_forecaster(self):
        assert self.adv.forecaster == "LATTO"

    def test_wmo_product_type(self):
        assert self.adv.wmo.product_type == "TCP"


class TestPublicAdvisory8:
    @pytest.fixture(autouse=True)
    def _adv(self, public_008):
        self.adv = parse_public(public_008)

    def test_status_hurricane(self):
        assert self.adv.status == StormStatus.HURRICANE

    def test_storm_name(self):
        assert self.adv.storm_name == "DALILA"

    def test_advisory_number(self):
        assert self.adv.advisory_number == "8"

    def test_max_wind_mph(self):
        assert self.adv.max_wind_mph == 90

    def test_max_wind_kmh(self):
        assert self.adv.max_wind_kmh == 150

    def test_gust_mph(self):
        assert self.adv.gust_mph == 110

    def test_gust_kmh(self):
        assert self.adv.gust_kmh == 175

    def test_pressure_lower(self):
        assert self.adv.pressure_mb == 973

    def test_lat(self):
        assert self.adv.lat == pytest.approx(15.8)

    def test_lon(self):
        assert self.adv.lon == pytest.approx(-107.2)

    def test_headline_contains_hurricane(self):
        assert self.adv.headline is not None
        assert "DALILA" in self.adv.headline

    def test_forecaster(self):
        assert self.adv.forecaster == "BERG"

    def test_issued_utc(self):
        expected = datetime(2025, 7, 12, 18, 0, tzinfo=timezone.utc)
        assert self.adv.issued_utc == expected


class TestPublicAdvisory14:
    @pytest.fixture(autouse=True)
    def _adv(self, public_014):
        self.adv = parse_public(public_014)

    def test_status(self):
        assert self.adv.status == StormStatus.TROPICAL_STORM

    def test_advisory_number(self):
        assert self.adv.advisory_number == "14"

    def test_max_wind_mph(self):
        assert self.adv.max_wind_mph == 40

    def test_pressure_above_1000(self):
        assert self.adv.pressure_mb == 1003

    def test_forecaster(self):
        assert self.adv.forecaster == "ROBERTS"

    def test_no_watches_warnings_no_headline_crash(self):
        # Headline may be present even when no warnings; just ensure no exception
        _ = self.adv.headline
