"""
Parser for NHC Public Advisory (public / TCP) products.

Public advisories use mph and km/h (not knots) and include
a human-readable prose section.
"""
from __future__ import annotations

import re
from datetime import datetime, timezone
from typing import Optional

from .ast_types import PublicAdvisory, StormStatus, WmoHeader
from .envelope import parse_envelope

# ---------------------------------------------------------------------------
# Token patterns
# ---------------------------------------------------------------------------

# "TROPICAL STORM FOUR-E PUBLIC ADVISORY NUMBER   1"
# "HURRICANE DALILA PUBLIC ADVISORY NUMBER   8"
# "TROPICAL STORM DALILA PUBLIC ADVISORY NUMBER  14"
# Also handles intermediate "...A" advisories
_HEADER_RE = re.compile(
    r"^(?P<status>POTENTIAL TROP CYCLONE|TROPICAL STORM|HURRICANE|"
    r"TROPICAL CYCLONE|POST-TROPICAL CYCLONE|POST-TROPICAL|REMNANT LOW)\s+"
    r"(?P<name>[A-Z0-9\-]+)\s+"
    r"(?:PUBLIC\s+)?(?:SPECIAL\s+)?ADVISORY\s+NUMBER\s+"
    r"(?P<num>\d+[A-Z]?)",
    re.MULTILINE,
)

# "1800 UTC WED JUL 09 2025"
_ISSUED_RE = re.compile(
    r"(?P<hhmm>\d{4})\s+UTC\s+\w{3}\s+(?P<mon>\w{3})\s+(?P<day>\d{1,2})\s+(?P<year>\d{4})",
    re.MULTILINE,
)

_MONTHS = {
    "JAN": 1, "FEB": 2, "MAR": 3, "APR": 4, "MAY": 5, "JUN": 6,
    "JUL": 7, "AUG": 8, "SEP": 9, "OCT": 10, "NOV": 11, "DEC": 12,
}

# Public summary block uses "LOCATION...13.5N 98.5W"
_LOCATION_RE = re.compile(
    r"LOCATION\.\.\.\s*(?P<lat>\d+\.?\d*)(?P<ns>[NS])\s+(?P<lon>\d+\.?\d*)(?P<ew>[EW])",
    re.MULTILINE,
)

# "MAXIMUM SUSTAINED WINDS...45 MPH...75 KM/H"
# "MAXIMUM SUSTAINED WINDS...90 MPH...150 KM/H"
_WINDS_RE = re.compile(
    r"MAXIMUM SUSTAINED WINDS\s*\.\.\.\s*"
    r"(?P<mph>\d+)\s*MPH\s*\.\.\.\s*(?P<kmh>\d+)\s*KM/H",
    re.MULTILINE | re.IGNORECASE,
)

# "PRESENT MOVEMENT...WNW OR 285 DEGREES AT 9 MPH...15 KM/H"
_MOVEMENT_RE = re.compile(
    r"PRESENT MOVEMENT\s*\.\.\.\s*"
    r"(?P<cardinal>[A-Z\-]+)\s+OR\s+(?P<deg>\d+)\s+DEGREES\s+AT\s+"
    r"(?P<mph>\d+)\s+MPH\s*\.\.\.\s*(?P<kmh>\d+)\s+KM/H",
    re.MULTILINE | re.IGNORECASE,
)

# "MINIMUM CENTRAL PRESSURE...999 MB...29.50 INCHES"
_PRESSURE_RE = re.compile(
    r"MINIMUM CENTRAL PRESSURE\s*\.\.\.\s*(?P<mb>\d+)\s*MB",
    re.MULTILINE | re.IGNORECASE,
)

# Gusts come from the discussion paragraph:
# "MAXIMUM SUSTAINED WINDS ARE NEAR 45 MPH (75 KM/H) WITH HIGHER GUSTS"
# or "... WITH GUSTS TO 55 MPH (89 KM/H)"
_GUST_RE = re.compile(
    r"WITH\s+GUSTS\s+(?:TO\s+)?(?P<mph>\d+)\s+MPH\s*[\(\.](?P<kmh>\d+)\s*KM/H",
    re.MULTILINE | re.IGNORECASE,
)

# Headline: the "...SOME TEXT..." line right after the issue time
_HEADLINE_RE = re.compile(
    r"^\.\.\.([A-Z0-9 '\-,/]+)\.\.\.",
    re.MULTILINE,
)

# "NEXT COMPLETE ADVISORY AT 10/0000Z"
_NEXT_ADV_RE = re.compile(
    r"NEXT\s+(?:COMPLETE\s+|SPECIAL\s+)?ADVISORY\s+AT\s+(\d{2}/\d{4}Z)",
    re.MULTILINE | re.IGNORECASE,
)

_STATUS_STRINGS = {
    "POTENTIAL TROP CYCLONE": StormStatus.POTENTIAL_TROPICAL_CYCLONE,
    "TROPICAL STORM": StormStatus.TROPICAL_STORM,
    "HURRICANE": StormStatus.HURRICANE,
    "TROPICAL CYCLONE": StormStatus.TROPICAL_CYCLONE,
    "POST-TROPICAL CYCLONE": StormStatus.POST_TROPICAL,
    "POST-TROPICAL": StormStatus.POST_TROPICAL,
    "REMNANT LOW": StormStatus.REMNANT_LOW,
    "DISSIPATED": StormStatus.DISSIPATED,
}


def _parse_status(s: str) -> StormStatus:
    s = s.strip().upper()
    if s in _STATUS_STRINGS:
        return _STATUS_STRINGS[s]
    for key, val in _STATUS_STRINGS.items():
        if key in s:
            return val
    raise ValueError(f"Unknown storm status: {s!r}")


def _parse_issued(m: re.Match) -> datetime:
    hour = int(m.group("hhmm")[:2])
    minute = int(m.group("hhmm")[2:])
    month = _MONTHS[m.group("mon").upper()]
    day = int(m.group("day"))
    year = int(m.group("year"))
    return datetime(year, month, day, hour, minute, tzinfo=timezone.utc)


def _signed_lon(lon_str: str, ew: str) -> float:
    lon = float(lon_str)
    return -lon if ew.upper() == "W" else lon


def parse_public(raw: str) -> PublicAdvisory:
    """
    Parse a raw public advisory product string into a :class:`PublicAdvisory`.
    """
    wmo, body, forecaster = parse_envelope(raw)

    full_text = raw.replace("\r\n", "\n").replace("\r", "\n")

    # --- Header ---
    m_hdr = _HEADER_RE.search(full_text)
    if not m_hdr:
        raise ValueError("Could not find advisory header line")
    status = _parse_status(m_hdr.group("status"))
    storm_name = m_hdr.group("name")
    advisory_number = m_hdr.group("num")

    # --- Issued time ---
    m_iss = _ISSUED_RE.search(body)
    if not m_iss:
        raise ValueError("Could not find issued UTC time")
    issued_utc = _parse_issued(m_iss)

    # --- Headline ---
    headline: Optional[str] = None
    m_hl = _HEADLINE_RE.search(body)
    if m_hl:
        headline = m_hl.group(1).strip()

    # --- Location ---
    m_loc = _LOCATION_RE.search(body)
    if not m_loc:
        raise ValueError("Could not find LOCATION line in public advisory")
    lat = float(m_loc.group("lat")) * (-1 if m_loc.group("ns") == "S" else 1)
    lon = _signed_lon(m_loc.group("lon"), m_loc.group("ew"))

    # --- Max winds ---
    m_winds = _WINDS_RE.search(body)
    if not m_winds:
        raise ValueError("Could not find MAXIMUM SUSTAINED WINDS line")
    max_wind_mph = int(m_winds.group("mph"))
    max_wind_kmh = int(m_winds.group("kmh"))

    # --- Gusts ---
    m_gust = _GUST_RE.search(body)
    gust_mph = int(m_gust.group("mph")) if m_gust else 0
    gust_kmh = int(m_gust.group("kmh")) if m_gust else 0

    # --- Movement ---
    m_mov = _MOVEMENT_RE.search(body)
    if not m_mov:
        raise ValueError("Could not find PRESENT MOVEMENT line")
    movement_cardinal = m_mov.group("cardinal")
    movement_degrees = int(m_mov.group("deg"))
    movement_mph = int(m_mov.group("mph"))
    movement_kmh = int(m_mov.group("kmh"))

    # --- Pressure ---
    m_pres = _PRESSURE_RE.search(body)
    if not m_pres:
        raise ValueError("Could not find MINIMUM CENTRAL PRESSURE")
    pressure_mb = int(m_pres.group("mb"))

    # --- Next advisory ---
    m_next = _NEXT_ADV_RE.search(body)
    next_advisory_utc = m_next.group(1) if m_next else None

    return PublicAdvisory(
        wmo=wmo,
        advisory_number=advisory_number,
        issued_utc=issued_utc,
        storm_name=storm_name,
        status=status,
        lat=lat,
        lon=lon,
        movement_degrees=movement_degrees,
        movement_cardinal=movement_cardinal,
        movement_mph=movement_mph,
        movement_kmh=movement_kmh,
        pressure_mb=pressure_mb,
        max_wind_mph=max_wind_mph,
        max_wind_kmh=max_wind_kmh,
        gust_mph=gust_mph,
        gust_kmh=gust_kmh,
        headline=headline,
        next_advisory_utc=next_advisory_utc,
        forecaster=forecaster,
    )
