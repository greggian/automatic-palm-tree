"""Typed AST nodes for all four NHC advisory product types."""
from __future__ import annotations

from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from typing import Optional


# ---------------------------------------------------------------------------
# Shared enumerations
# ---------------------------------------------------------------------------


class StormStatus(Enum):
    POTENTIAL_TROPICAL_CYCLONE = "POTENTIAL TROP CYCLONE"
    TROPICAL_CYCLONE = "TROPICAL CYCLONE"
    TROPICAL_STORM = "TROPICAL STORM"
    HURRICANE = "HURRICANE"
    POST_TROPICAL = "POST-TROPICAL"
    REMNANT_LOW = "REMNANT LOW"
    DISSIPATED = "DISSIPATED"


# ---------------------------------------------------------------------------
# Shared envelope node
# ---------------------------------------------------------------------------


@dataclass
class WmoHeader:
    awips_id: str        # e.g. "MIATCMEP4"
    product_type: str    # "TCM", "TCP", "TCD", "PWS"
    originator: str      # "KNHC"
    storm_id: str        # "EP042025"
    basin: str           # "EP", "AL", "CP"
    storm_number: int    # 4
    year: int            # 2025


# ---------------------------------------------------------------------------
# fstadv – Forecast/Advisory
# ---------------------------------------------------------------------------


@dataclass
class WindRadii:
    threshold_kt: int   # 34, 50, or 64
    ne_nm: int
    se_nm: int
    sw_nm: int
    nw_nm: int


@dataclass
class ForecastPoint:
    valid_time: str              # "13/0600Z"  (DD/HHmmZ)
    is_outlook: bool             # True for day 4-5 "OUTLOOK VALID" lines
    status: StormStatus
    lat: Optional[float]         # None if DISSIPATED
    lon: Optional[float]         # always stored as negative (west)
    max_wind_kt: Optional[int]
    gust_kt: Optional[int]
    wind_radii: list[WindRadii] = field(default_factory=list)


@dataclass
class ForecastAdvisory:
    wmo: WmoHeader
    advisory_number: str         # "1", "2", "12A"
    issued_utc: datetime
    storm_name: str              # "FOUR-E", "DALILA"
    status: StormStatus
    lat: float
    lon: float                   # stored as negative float for W
    position_accuracy_nm: int
    movement_degrees: int
    movement_cardinal: str       # "WNW", "NNE", etc.
    movement_kt: int
    pressure_mb: int
    max_wind_kt: int
    gust_kt: int
    prev_position: Optional[tuple[str, float, float]]  # (time, lat, lon)
    forecast_points: list[ForecastPoint]
    next_advisory_utc: Optional[str]
    forecaster: str


# ---------------------------------------------------------------------------
# public – Public Advisory
# ---------------------------------------------------------------------------


@dataclass
class PublicAdvisory:
    wmo: WmoHeader
    advisory_number: str         # "1", "2", "12A"
    issued_utc: datetime
    storm_name: str
    status: StormStatus
    lat: float
    lon: float                   # negative for W
    movement_degrees: int
    movement_cardinal: str
    movement_mph: int            # public uses mph
    movement_kmh: int
    pressure_mb: int
    max_wind_mph: int
    max_wind_kmh: int
    gust_mph: int
    gust_kmh: int
    headline: Optional[str]      # free-text summary paragraph
    next_advisory_utc: Optional[str]
    forecaster: str


# ---------------------------------------------------------------------------
# discus – Forecast Discussion
# ---------------------------------------------------------------------------


@dataclass
class ForecastDiscussion:
    wmo: WmoHeader
    advisory_number: str
    issued_utc: datetime
    storm_name: str
    status: StormStatus
    body: str                    # full prose body (between header and $$)
    forecaster: str


# ---------------------------------------------------------------------------
# wndprb – Wind Speed Probabilities
# ---------------------------------------------------------------------------


@dataclass
class WindProbEntry:
    """One row in the wind probability table (one location)."""
    location: str                # e.g. "PORT OF SAN JOSE  GU  14.0N  90.8W"
    lat: float
    lon: float                   # negative for W
    # Cumulative probabilities keyed by threshold (34/50/64 kt) then window
    # windows: "12HR", "24HR", "36HR", "48HR", "72HR", "96HR", "120HR"
    probs: dict[int, dict[str, int]]  # probs[threshold_kt][window] = pct


@dataclass
class WindProbTable:
    threshold_kt: int
    entries: list[WindProbEntry]


@dataclass
class WindProbAdvisory:
    wmo: WmoHeader
    advisory_number: str
    issued_utc: datetime
    storm_name: str
    status: StormStatus
    tables: list[WindProbTable]
    forecaster: str
