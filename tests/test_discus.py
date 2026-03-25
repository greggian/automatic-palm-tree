"""Tests for the forecast discussion (TCD) parser."""
from __future__ import annotations

from datetime import datetime, timezone

import pytest
from nhc_parser.ast_types import ForecastDiscussion, StormStatus
from nhc_parser.discus import parse_discus


class TestDiscusAdvisory1:
    @pytest.fixture(autouse=True)
    def _adv(self, discus_001):
        self.adv = parse_discus(discus_001)

    def test_returns_forecast_discussion(self):
        assert isinstance(self.adv, ForecastDiscussion)

    def test_advisory_number(self):
        assert self.adv.advisory_number == "1"

    def test_storm_name(self):
        assert self.adv.storm_name == "FOUR-E"

    def test_status(self):
        assert self.adv.status == StormStatus.TROPICAL_STORM

    def test_issued_utc(self):
        expected = datetime(2025, 7, 9, 18, 0, tzinfo=timezone.utc)
        assert self.adv.issued_utc == expected

    def test_body_is_nonempty(self):
        assert len(self.adv.body) > 100

    def test_body_contains_prose(self):
        assert "vigorous" in self.adv.body.lower() or "organization" in self.adv.body.lower()

    def test_body_contains_forecast_table(self):
        assert "FORECAST POSITIONS" in self.adv.body.upper()

    def test_body_does_not_contain_sentinel(self):
        assert "$$" not in self.adv.body
        assert "NNNN" not in self.adv.body

    def test_forecaster(self):
        assert self.adv.forecaster == "LATTO"

    def test_wmo_product_type(self):
        assert self.adv.wmo.product_type == "TCD"


class TestDiscusAdvisory8:
    @pytest.fixture(autouse=True)
    def _adv(self, discus_008):
        self.adv = parse_discus(discus_008)

    def test_status_hurricane(self):
        assert self.adv.status == StormStatus.HURRICANE

    def test_storm_name(self):
        assert self.adv.storm_name == "DALILA"

    def test_advisory_number(self):
        assert self.adv.advisory_number == "8"

    def test_body_mentions_80kt(self):
        assert "80 kt" in self.adv.body.lower()

    def test_forecaster(self):
        assert self.adv.forecaster == "BERG"

    def test_issued_utc(self):
        expected = datetime(2025, 7, 12, 18, 0, tzinfo=timezone.utc)
        assert self.adv.issued_utc == expected


class TestDiscusAdvisory14:
    @pytest.fixture(autouse=True)
    def _adv(self, discus_014):
        self.adv = parse_discus(discus_014)

    def test_status(self):
        assert self.adv.status == StormStatus.TROPICAL_STORM

    def test_advisory_number(self):
        assert self.adv.advisory_number == "14"

    def test_body_mentions_dissipated(self):
        assert "dissipate" in self.adv.body.lower()

    def test_forecaster(self):
        assert self.adv.forecaster == "ROBERTS"

    def test_body_contains_forecast_positions(self):
        assert "FORECAST POSITIONS" in self.adv.body.upper()
