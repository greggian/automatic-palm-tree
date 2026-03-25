"""Tests for the shared WMO/AWIPS envelope parser."""
from __future__ import annotations

import pytest
from nhc_parser.envelope import parse_envelope
from nhc_parser.ast_types import WmoHeader


class TestEnvelopeBasics:
    def test_fstadv_awips(self, fstadv_001):
        wmo, body, forecaster = parse_envelope(fstadv_001)
        assert wmo.awips_id == "MIATCMEP4"
        assert wmo.product_type == "TCM"
        assert wmo.basin == "EP"
        assert wmo.storm_number == 4
        assert wmo.year == 2025
        assert wmo.storm_id == "EP042025"
        assert wmo.originator == "KNHC"

    def test_public_awips(self, public_001):
        wmo, _, _ = parse_envelope(public_001)
        assert wmo.awips_id == "MIATCPEP4"
        assert wmo.product_type == "TCP"

    def test_discus_awips(self, discus_001):
        wmo, _, _ = parse_envelope(discus_001)
        assert wmo.awips_id == "MIATCDEP4"
        assert wmo.product_type == "TCD"

    def test_wndprb_awips(self, wndprb_001):
        wmo, _, _ = parse_envelope(wndprb_001)
        assert wmo.awips_id == "MIAPWSEP4"
        assert wmo.product_type == "PWS"

    def test_forecaster_extracted(self, fstadv_001):
        _, _, forecaster = parse_envelope(fstadv_001)
        assert forecaster == "LATTO"

    def test_body_excludes_zczc(self, fstadv_001):
        _, body, _ = parse_envelope(fstadv_001)
        assert "ZCZC" not in body

    def test_body_excludes_nnnn(self, fstadv_001):
        _, body, _ = parse_envelope(fstadv_001)
        assert "NNNN" not in body

    def test_body_excludes_sentinel(self, fstadv_001):
        _, body, _ = parse_envelope(fstadv_001)
        assert "$$" not in body

    def test_invalid_awips_raises(self):
        raw = "ZCZC XXXZZZZZ ALL\nTTAA00 KNHC 091800\n\nbody\n\n$$\nNNNN\n"
        with pytest.raises(ValueError, match="Unrecognised AWIPS ID"):
            parse_envelope(raw)

    def test_missing_zczc_raises(self):
        raw = "TTAA00 KNHC 091800\n\nbody\n$$\nNNNN"
        with pytest.raises(ValueError, match="No ZCZC sentinel"):
            parse_envelope(raw)

    def test_advisory_8_forecaster(self, fstadv_008):
        _, _, forecaster = parse_envelope(fstadv_008)
        assert forecaster == "BERG"

    def test_advisory_14_forecaster(self, fstadv_014):
        _, _, forecaster = parse_envelope(fstadv_014)
        assert forecaster == "ROBERTS"
