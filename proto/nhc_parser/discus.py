"""
Parser for NHC Forecast Discussion (discus / TCD) products.

The discussion is largely free-form prose with a structured tail
containing the forecast positions table.
"""
from __future__ import annotations

import re
from datetime import datetime, timezone

from .ast_types import ForecastDiscussion, StormStatus, WmoHeader
from .envelope import parse_envelope

# ---------------------------------------------------------------------------
# Token patterns
# ---------------------------------------------------------------------------

_HEADER_RE = re.compile(
    r"^(?P<status>POTENTIAL TROP CYCLONE|TROPICAL STORM|HURRICANE|"
    r"TROPICAL CYCLONE|POST-TROPICAL CYCLONE|POST-TROPICAL|REMNANT LOW)\s+"
    r"(?P<name>[A-Z0-9\-]+)\s+"
    r"(?:FORECAST\s+)?DISCUSSION\s+NUMBER\s+"
    r"(?P<num>\d+[A-Z]?)",
    re.MULTILINE,
)

_ISSUED_RE = re.compile(
    r"(?P<hhmm>\d{4})\s+UTC\s+\w{3}\s+(?P<mon>\w{3})\s+(?P<day>\d{1,2})\s+(?P<year>\d{4})",
    re.MULTILINE,
)

_MONTHS = {
    "JAN": 1, "FEB": 2, "MAR": 3, "APR": 4, "MAY": 5, "JUN": 6,
    "JUL": 7, "AUG": 8, "SEP": 9, "OCT": 10, "NOV": 11, "DEC": 12,
}

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


def parse_discus(raw: str) -> ForecastDiscussion:
    """
    Parse a raw forecast discussion product into a :class:`ForecastDiscussion`.

    The full prose body (everything between the NWS office line and ``$$``)
    is preserved verbatim in ``ForecastDiscussion.body``.
    """
    wmo, body, forecaster = parse_envelope(raw)

    full_text = raw.replace("\r\n", "\n").replace("\r", "\n")

    # --- Header ---
    m_hdr = _HEADER_RE.search(full_text)
    if not m_hdr:
        raise ValueError("Could not find discussion header line")
    status = _parse_status(m_hdr.group("status"))
    storm_name = m_hdr.group("name")
    advisory_number = m_hdr.group("num")

    # --- Issued time ---
    m_iss = _ISSUED_RE.search(body)
    if not m_iss:
        raise ValueError("Could not find issued UTC time")
    issued_utc = _parse_issued(m_iss)

    # The body is preserved as-is after stripping the issue time line.
    # Find the line after the issued time and use everything from there.
    iss_end = m_iss.end()
    prose_start = body.find("\n", iss_end)
    prose = body[prose_start:].strip() if prose_start != -1 else body

    return ForecastDiscussion(
        wmo=wmo,
        advisory_number=advisory_number,
        issued_utc=issued_utc,
        storm_name=storm_name,
        status=status,
        body=prose,
        forecaster=forecaster,
    )
