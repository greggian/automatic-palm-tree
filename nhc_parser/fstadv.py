"""
Parser for NHC Forecast/Advisory (fstadv) products.

All values are in knots and nautical miles as issued.

Grammar (simplified BNF):

  fstadv     ::= status_line advisory_line issued_line body
  status_line ::= <STATUS> ("FORECAST/ADVISORY" | "SPECIAL") "NUMBER" <N>
  body        ::= position movement pressure winds radii* prev_pos forecast* next_adv
"""
from __future__ import annotations

import re
from datetime import datetime, timezone
from typing import Optional

from .ast_types import (
    ForecastAdvisory,
    ForecastPoint,
    StormStatus,
    WindRadii,
    WmoHeader,
)
from .envelope import parse_envelope

# ---------------------------------------------------------------------------
# Token patterns
# ---------------------------------------------------------------------------

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

# "TROPICAL STORM FOUR-E FORECAST/ADVISORY NUMBER   1"
# "HURRICANE DALILA SPECIAL ADVISORY NUMBER   8"
# "TROPICAL STORM DALILA FORECAST/ADVISORY NUMBER  14"
_HEADER_RE = re.compile(
    r"^(?P<status>POTENTIAL TROP CYCLONE|TROPICAL STORM|HURRICANE|TROPICAL CYCLONE"
    r"|POST-TROPICAL CYCLONE|POST-TROPICAL|REMNANT LOW)\s+"
    r"(?P<name>[A-Z0-9\-]+)\s+"
    r"(?:FORECAST/ADVISORY|SPECIAL ADVISORY|ADVISORY)\s+NUMBER\s+"
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

# "CENTER LOCATED NEAR 13.5N  98.5W AT 09/1800Z"
_POSITION_RE = re.compile(
    r"CENTER LOCATED NEAR\s+"
    r"(?P<lat>\d+\.?\d*)(?P<ns>[NS])\s+"
    r"(?P<lon>\d+\.?\d*)(?P<ew>[EW])\s+"
    r"AT\s+(?P<time>\d{2}/\d{4}Z)",
    re.MULTILINE,
)

# "POSITION ACCURATE WITHIN  60 NM"
_ACCURACY_RE = re.compile(
    r"POSITION ACCURATE WITHIN\s+(\d+)\s+NM",
    re.MULTILINE,
)

# "PRESENT MOVEMENT TOWARD THE WNW OR 285 DEGREES AT   8 KT"
# "PRESENT MOVEMENT TOWARD THE NORTH-NORTHWEST OR 335 DEGREES AT   5 KT"
_MOVEMENT_RE = re.compile(
    r"PRESENT MOVEMENT TOWARD THE\s+"
    r"(?P<cardinal>[A-Z\-]+)\s+OR\s+"
    r"(?P<deg>\d+)\s+DEGREES AT\s+(?P<kt>\d+)\s+KT",
    re.MULTILINE,
)

# "ESTIMATED MINIMUM CENTRAL PRESSURE  999 MB"
_PRESSURE_RE = re.compile(
    r"ESTIMATED MINIMUM CENTRAL PRESSURE\s+(?P<mb>\d+)\s+MB",
    re.MULTILINE,
)

# "MAX SUSTAINED WINDS  40 KT WITH GUSTS TO  50 KT"
_WINDS_RE = re.compile(
    r"MAX SUSTAINED WINDS\s+(?P<max>\d+)\s+KT\s+WITH\s+GUSTS\s+TO\s+(?P<gust>\d+)\s+KT",
    re.MULTILINE,
)

# " 34 KT.........120NE 120SE  90SW  90NW."
# " 64 KT...........0NE   0SE   0SW   0NW."
_RADII_RE = re.compile(
    r"^\s*(?P<thresh>34|50|64)\s+KT[. ]+(?P<ne>\d+)NE\s+(?P<se>\d+)SE\s+(?P<sw>\d+)SW\s+(?P<nw>\d+)NW",
    re.MULTILINE,
)

# "AT 09/1200Z CENTER WAS LOCATED NEAR 13.5N  97.5W"
_PREV_POS_RE = re.compile(
    r"AT\s+(?P<time>\d{2}/\d{4}Z)\s+CENTER WAS LOCATED NEAR\s+"
    r"(?P<lat>\d+\.?\d*)[NS]\s+"
    r"(?P<lon>\d+\.?\d*)(?P<ew>[EW])",
    re.MULTILINE,
)

# "FORECAST VALID 10/0600Z 14.2N 100.4W"
# "OUTLOOK VALID 14/1800Z 17.0N 112.0W"
# "FORECAST VALID 11/1800Z...DISSIPATED"
_FCST_VALID_RE = re.compile(
    r"^(?P<kind>FORECAST|OUTLOOK)\s+VALID\s+(?P<time>\d{2}/\d{4}Z)"
    r"(?:\s+(?P<lat>\d+\.?\d*)[NS]\s+(?P<lon>\d+\.?\d*)(?P<ew>[EW])"
    r"|\s*\.\.\.\s*(?P<dis>DISSIPATED))",
    re.MULTILINE,
)

# "MAX WIND  45 KT...GUSTS  55 KT."  (in forecast block)
_FCST_WIND_RE = re.compile(
    r"MAX WIND\s+(?P<max>\d+)\s+KT.*?GUSTS\s+(?P<gust>\d+)\s+KT",
    re.MULTILINE,
)

# Forecast radii: same pattern as current
_FCST_RADII_RE = _RADII_RE

# "NEXT ADVISORY AT 10/0000Z"
# "NEXT SPECIAL ADVISORY AT 10/0000Z"
_NEXT_ADV_RE = re.compile(
    r"NEXT\s+(?:SPECIAL\s+)?ADVISORY\s+AT\s+(\d{2}/\d{4}Z)",
    re.MULTILINE,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _parse_status(s: str) -> StormStatus:
    s = s.strip().upper()
    if s in _STATUS_STRINGS:
        return _STATUS_STRINGS[s]
    # Partial match
    for key, val in _STATUS_STRINGS.items():
        if key in s:
            return val
    raise ValueError(f"Unknown storm status: {s!r}")


def _signed_lon(lon_str: str, ew: str) -> float:
    lon = float(lon_str)
    return -lon if ew.upper() == "W" else lon


def _signed_lat(lat_str: str, ns: str) -> float:
    lat = float(lat_str)
    return -lat if ns.upper() == "S" else lat


def _parse_issued(m: re.Match) -> datetime:
    hour = int(m.group("hhmm")[:2])
    minute = int(m.group("hhmm")[2:])
    month = _MONTHS[m.group("mon").upper()]
    day = int(m.group("day"))
    year = int(m.group("year"))
    return datetime(year, month, day, hour, minute, tzinfo=timezone.utc)


# ---------------------------------------------------------------------------
# Forecast block parser
# ---------------------------------------------------------------------------

def _parse_forecast_blocks(body: str) -> list[ForecastPoint]:
    """
    Extract all FORECAST VALID / OUTLOOK VALID blocks from body text.
    Each block starts at the FORECAST/OUTLOOK line and continues until
    the next such line or the end of the body.
    """
    points: list[ForecastPoint] = []

    # Split body into lines for easier multi-line block scanning
    lines = body.splitlines()
    n = len(lines)

    i = 0
    while i < n:
        line = lines[i]
        m = _FCST_VALID_RE.match(line)
        if not m:
            i += 1
            continue

        kind = m.group("kind")
        time_str = m.group("time")
        is_outlook = kind == "OUTLOOK"

        if m.group("dis"):
            points.append(ForecastPoint(
                valid_time=time_str,
                is_outlook=is_outlook,
                status=StormStatus.DISSIPATED,
                lat=None,
                lon=None,
                max_wind_kt=None,
                gust_kt=None,
            ))
            i += 1
            continue

        lat = float(m.group("lat"))
        if "S" in line[m.start("lat"):m.end("lat") + 1]:
            lat = -lat
        lon = _signed_lon(m.group("lon"), m.group("ew"))

        # Scan next few lines for MAX WIND and radii
        block_lines = []
        j = i + 1
        while j < n and j < i + 10:
            l2 = lines[j]
            # Stop if new FORECAST/OUTLOOK block or NEXT ADVISORY line
            if _FCST_VALID_RE.match(l2) or "NEXT ADVISORY" in l2 or "NEXT SPECIAL" in l2:
                break
            block_lines.append(l2)
            j += 1

        block_text = "\n".join(block_lines)

        mw = _FCST_WIND_RE.search(block_text)
        max_wind = int(mw.group("max")) if mw else None
        gust = int(mw.group("gust")) if mw else None

        # Status for forecast point – infer from wind speed
        # (The fstadv product doesn't always label the forecast-point status)
        if max_wind is None:
            status = StormStatus.DISSIPATED
        elif max_wind >= 64:
            status = StormStatus.HURRICANE
        elif max_wind >= 34:
            status = StormStatus.TROPICAL_STORM
        else:
            status = StormStatus.TROPICAL_STORM  # sub-TS but still listed

        radii: list[WindRadii] = []
        for rm in _FCST_RADII_RE.finditer(block_text):
            radii.append(WindRadii(
                threshold_kt=int(rm.group("thresh")),
                ne_nm=int(rm.group("ne")),
                se_nm=int(rm.group("se")),
                sw_nm=int(rm.group("sw")),
                nw_nm=int(rm.group("nw")),
            ))

        points.append(ForecastPoint(
            valid_time=time_str,
            is_outlook=is_outlook,
            status=status,
            lat=lat,
            lon=lon,
            max_wind_kt=max_wind,
            gust_kt=gust,
            wind_radii=radii,
        ))
        i = j

    return points


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------


def parse_fstadv(raw: str) -> ForecastAdvisory:
    """
    Parse a raw fstadv product string into a :class:`ForecastAdvisory` AST node.

    Parameters
    ----------
    raw:
        Full product text including ZCZC header and NNNN trailer,
        or just the text inside the ``<pre>`` block from the NHC archive page.
    """
    wmo, body, forecaster = parse_envelope(raw)

    full_text = raw.replace("\r\n", "\n").replace("\r", "\n")

    # --- Header line ---
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

    # --- Current position ---
    m_pos = _POSITION_RE.search(body)
    if not m_pos:
        raise ValueError("Could not find CENTER LOCATED NEAR line")
    lat_raw = m_pos.group("lat")
    lon_raw = m_pos.group("lon")
    lat = float(lat_raw) * (-1 if m_pos.group("ns") == "S" else 1)
    lon = _signed_lon(lon_raw, m_pos.group("ew"))

    # --- Accuracy ---
    m_acc = _ACCURACY_RE.search(body)
    accuracy_nm = int(m_acc.group(1)) if m_acc else 0

    # --- Movement ---
    m_mov = _MOVEMENT_RE.search(body)
    if not m_mov:
        raise ValueError("Could not find PRESENT MOVEMENT line")
    movement_cardinal = m_mov.group("cardinal")
    movement_degrees = int(m_mov.group("deg"))
    movement_kt = int(m_mov.group("kt"))

    # --- Pressure ---
    m_pres = _PRESSURE_RE.search(body)
    if not m_pres:
        raise ValueError("Could not find ESTIMATED MINIMUM CENTRAL PRESSURE")
    pressure_mb = int(m_pres.group("mb"))

    # --- Max winds ---
    m_winds = _WINDS_RE.search(body)
    if not m_winds:
        raise ValueError("Could not find MAX SUSTAINED WINDS line")
    max_wind_kt = int(m_winds.group("max"))
    gust_kt = int(m_winds.group("gust"))

    # --- Previous position ---
    m_prev = _PREV_POS_RE.search(body)
    prev_position: Optional[tuple[str, float, float]] = None
    if m_prev:
        prev_lat = float(m_prev.group("lat"))
        prev_lon = _signed_lon(m_prev.group("lon"), m_prev.group("ew"))
        prev_position = (m_prev.group("time"), prev_lat, prev_lon)

    # --- Forecast points ---
    forecast_points = _parse_forecast_blocks(body)

    # --- Next advisory ---
    m_next = _NEXT_ADV_RE.search(body)
    next_advisory_utc = m_next.group(1) if m_next else None

    return ForecastAdvisory(
        wmo=wmo,
        advisory_number=advisory_number,
        issued_utc=issued_utc,
        storm_name=storm_name,
        status=status,
        lat=lat,
        lon=lon,
        position_accuracy_nm=accuracy_nm,
        movement_degrees=movement_degrees,
        movement_cardinal=movement_cardinal,
        movement_kt=movement_kt,
        pressure_mb=pressure_mb,
        max_wind_kt=max_wind_kt,
        gust_kt=gust_kt,
        prev_position=prev_position,
        forecast_points=forecast_points,
        next_advisory_utc=next_advisory_utc,
        forecaster=forecaster,
    )
