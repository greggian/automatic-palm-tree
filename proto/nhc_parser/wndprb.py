"""
Parser for NHC Wind Speed Probability (wndprb / PWS) products.

The wndprb product contains fixed-width columnar tables of cumulative
wind probability by location, grouped by wind threshold (34/50/64 kt).

Table structure
---------------
Each threshold section is introduced by a header line:

    PROBABILITIES OF 34 KT WINDS

followed by a column-header pair:

                         FROM  12 HR 24 HR 36 HR 48 HR 72 HR 96 HR 120HR
                         NOW   END   END   END   END   END   END    END
    LOCATION             PROB  PROB  PROB  PROB  PROB  PROB  PROB   PROB

then rows, two lines per location:

    ACAPULCO         MX  16.9N  99.9W
                           X    35    50    50    30    10     5     X

"X" denotes < 1 % (stored as 0).
"""
from __future__ import annotations

import re
from datetime import datetime, timezone
from typing import Optional

from .ast_types import (
    WindProbAdvisory,
    WindProbEntry,
    WindProbTable,
    StormStatus,
    WmoHeader,
)
from .envelope import parse_envelope

# ---------------------------------------------------------------------------
# Token patterns
# ---------------------------------------------------------------------------

_HEADER_RE = re.compile(
    r"^(?P<status>POTENTIAL TROP CYCLONE|TROPICAL STORM|HURRICANE|"
    r"TROPICAL CYCLONE|POST-TROPICAL CYCLONE|POST-TROPICAL|REMNANT LOW)\s+"
    r"(?P<name>[A-Z0-9\-]+)\s+"
    r"WIND SPEED PROBABILITIES\s+NUMBER\s+"
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

# Section header: "PROBABILITIES OF 34 KT WINDS"
_SECTION_RE = re.compile(
    r"PROBABILITIES OF (?P<thresh>34|50|64) KT WINDS",
    re.MULTILINE,
)

# Location line: "ACAPULCO         MX  16.9N  99.9W"
#                "LAZARO CARDENAS  MX  17.9N 102.2W"
# Two-letter country code always present in NHC wndprb products.
_LOC_RE = re.compile(
    r"^(?P<name>[A-Z][A-Z0-9 \.\-]{1,24}?)\s{2,}"
    r"(?P<cc>[A-Z]{2})\s+"
    r"(?P<lat>\d+\.?\d*)(?P<ns>[NS])\s+"
    r"(?P<lon>\d+\.?\d*)(?P<ew>[EW])",
)

# Probability row: space-padded integers or "X"
_PROB_TOKEN_RE = re.compile(r"\b(?P<val>\d+|X)\b")

# Column labels used to identify the window order
_WINDOWS = ("12HR", "24HR", "36HR", "48HR", "72HR", "96HR", "120HR")


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


def _parse_prob_line(line: str) -> Optional[list[int]]:
    """
    Extract probability values from a data row.

    Returns a list of 7 ints (one per window) or None if not a data row.
    "X" → 0.
    """
    tokens = [t for t in line.split() if re.fullmatch(r"\d+|X", t)]
    if len(tokens) < 7:
        return None
    return [0 if t == "X" else int(t) for t in tokens[:7]]


def _parse_section(lines: list[str], threshold_kt: int) -> list[WindProbEntry]:
    """
    Parse location/probability pairs from a section of lines belonging to one
    wind threshold.  Returns a list of WindProbEntry.
    """
    entries: list[WindProbEntry] = []
    i = 0
    while i < len(lines):
        line = lines[i]
        m_loc = _LOC_RE.match(line.rstrip())
        if m_loc:
            loc_name = m_loc.group("name").strip()
            if m_loc.group("cc"):
                loc_name = f"{loc_name} {m_loc.group('cc')}"
            lat = float(m_loc.group("lat")) * (-1 if m_loc.group("ns") == "S" else 1)
            lon = _signed_lon(m_loc.group("lon"), m_loc.group("ew"))

            # Next non-blank line should be the probability row
            j = i + 1
            probs_list: Optional[list[int]] = None
            while j < len(lines):
                probs_list = _parse_prob_line(lines[j])
                if probs_list is not None:
                    break
                if lines[j].strip() and not lines[j].strip().startswith("PROB"):
                    break
                j += 1

            if probs_list:
                probs_dict = {w: probs_list[k] for k, w in enumerate(_WINDOWS)}
                entries.append(WindProbEntry(
                    location=loc_name,
                    lat=lat,
                    lon=lon,
                    probs={threshold_kt: probs_dict},
                ))
                i = j + 1
                continue
        i += 1
    return entries


def parse_wndprb(raw: str) -> WindProbAdvisory:
    """
    Parse a raw wndprb product string into a :class:`WindProbAdvisory`.
    """
    wmo, body, forecaster = parse_envelope(raw)

    full_text = raw.replace("\r\n", "\n").replace("\r", "\n")

    # --- Header ---
    m_hdr = _HEADER_RE.search(full_text)
    if not m_hdr:
        raise ValueError("Could not find wndprb header line")
    status = _parse_status(m_hdr.group("status"))
    storm_name = m_hdr.group("name")
    advisory_number = m_hdr.group("num")

    # --- Issued time ---
    m_iss = _ISSUED_RE.search(body)
    if not m_iss:
        raise ValueError("Could not find issued UTC time")
    issued_utc = _parse_issued(m_iss)

    # --- Parse probability sections ---
    body_lines = body.splitlines()
    # Find section boundaries
    section_bounds: list[tuple[int, int]] = []  # (start_line, threshold)
    for i, line in enumerate(body_lines):
        ms = _SECTION_RE.search(line)
        if ms:
            section_bounds.append((i, int(ms.group("thresh"))))

    tables: list[WindProbTable] = []
    for idx, (start, thresh) in enumerate(section_bounds):
        # Section runs until next section header or end of body
        end = section_bounds[idx + 1][0] if idx + 1 < len(section_bounds) else len(body_lines)
        section_lines = body_lines[start + 1:end]
        entries = _parse_section(section_lines, thresh)
        tables.append(WindProbTable(threshold_kt=thresh, entries=entries))

    return WindProbAdvisory(
        wmo=wmo,
        advisory_number=advisory_number,
        issued_utc=issued_utc,
        storm_name=storm_name,
        status=status,
        tables=tables,
        forecaster=forecaster,
    )
