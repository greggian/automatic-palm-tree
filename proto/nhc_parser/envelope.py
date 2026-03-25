"""
WMO / AWIPS envelope parser.

Every NHC product is wrapped in:

    ZCZC <AWIPS_ID> ALL
    TTAA00 KNHC DDHHMM

    <body>

    $$
    Forecaster <NAME>
    NNNN

This module strips that framing and returns (WmoHeader, body_text, forecaster).
"""
from __future__ import annotations

import re
from typing import Optional

from .ast_types import WmoHeader

# AWIPS-ID → product type code
_PRODUCT_RE = re.compile(
    r"^ZCZC\s+(?P<awips_id>\S+)\s+ALL",
    re.MULTILINE,
)
_WMO_HDR_RE = re.compile(
    r"^TTAA00\s+(?P<originator>\S+)\s+\d{6}",
    re.MULTILINE,
)
_FORECASTER_RE = re.compile(
    r"^(?:FORECASTER|Forecaster)\s+([A-Z][A-Z\-]+)",
    re.MULTILINE | re.IGNORECASE,
)

# AWIPS product code → (product_type, basin_pattern)
# e.g. MIATCMEP4  -> TCM, EP, storm 4
_AWIPS_RE = re.compile(
    r"^MIA"
    r"(?P<ptype>TCM|TCP|TCD|PWS)"
    r"(?P<basin>EP|AL|CP)"
    r"(?P<num>\d+)$",
    re.IGNORECASE,
)


def _parse_awips(awips_id: str) -> tuple[str, str, int]:
    """Return (product_type, basin, storm_number) from AWIPS ID."""
    m = _AWIPS_RE.match(awips_id.upper())
    if not m:
        raise ValueError(f"Unrecognised AWIPS ID: {awips_id!r}")
    return m.group("ptype").upper(), m.group("basin").upper(), int(m.group("num"))


def parse_envelope(raw: str) -> tuple[WmoHeader, str, str]:
    """
    Parse WMO/AWIPS envelope from *raw* product text.

    Returns
    -------
    (WmoHeader, body, forecaster)
        body    – text between the WMO header and the ``$$`` sentinel
        forecaster – name string, e.g. ``"LATTO"``
    """
    # Normalise line endings
    text = raw.replace("\r\n", "\n").replace("\r", "\n")

    # --- ZCZC line ---
    m_zczc = _PRODUCT_RE.search(text)
    if not m_zczc:
        raise ValueError("No ZCZC sentinel found in product text")
    awips_id = m_zczc.group("awips_id")
    product_type, basin, storm_number = _parse_awips(awips_id)

    # --- WMO header line ---
    m_wmo = _WMO_HDR_RE.search(text)
    if not m_wmo:
        raise ValueError("No TTAA00 WMO header found")
    originator = m_wmo.group("originator")

    # --- Derive storm_id: need year from body ---
    # Storm ID like EP042025 is usually in the first non-blank body line.
    # We'll build it from basin + storm_number + year extracted later.

    # --- Body: everything between WMO header line and $$ ---
    wmo_end = m_wmo.end()
    body_start = text.find("\n", wmo_end) + 1
    body_end = text.find("\n$$")
    if body_end == -1:
        # Try without newline prefix
        body_end = text.find("$$")
    body = text[body_start:body_end].strip()

    # --- Forecaster ---
    m_fc = _FORECASTER_RE.search(text[body_end:])
    if not m_fc:
        # Try in full text
        m_fc = _FORECASTER_RE.search(text)
    forecaster = m_fc.group(1).upper() if m_fc else "UNKNOWN"

    # --- Storm ID (extract year from body) ---
    year_m = re.search(
        r"\b(?:EP|AL|CP)0*" + str(storm_number) + r"(\d{4})\b",
        body,
        re.IGNORECASE,
    )
    if year_m:
        year = int(year_m.group(1))
    else:
        # Fall back: look for 4-digit year in first 200 chars of body
        year_fallback = re.search(r"\b(20\d{2})\b", body[:200])
        year = int(year_fallback.group(1)) if year_fallback else 0

    storm_id = f"{basin}{storm_number:02d}{year}"

    wmo = WmoHeader(
        awips_id=awips_id.upper(),
        product_type=product_type,
        originator=originator.upper(),
        storm_id=storm_id,
        basin=basin,
        storm_number=storm_number,
        year=year,
    )
    return wmo, body, forecaster
