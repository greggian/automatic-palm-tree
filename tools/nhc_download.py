#!/usr/bin/env python3
"""
nhc_download.py — Download NHC tropical cyclone advisories from the archive.

ARCHIVE STRUCTURE
-----------------
  https://www.nhc.noaa.gov/archive/
      → year links: 2024/, 2023/, …

  https://www.nhc.noaa.gov/archive/<year>/
      → storm links: ep04/, al12/, cp01/, …

  https://www.nhc.noaa.gov/archive/<year>/<basin><num>/
      → advisory files: <storm_id>.<product>.<NNN>.shtml

  File content lives inside the first <pre>…</pre> block of each .shtml page.

USAGE
-----
  # one storm
  python tools/nhc_download.py --storm EP 4 2025

  # all storms in a year
  python tools/nhc_download.py --year 2025

  # everything in the archive
  python tools/nhc_download.py --all

  # filters and options
  python tools/nhc_download.py --year 2025 --basin EP --product fstadv,public
  python tools/nhc_download.py --year 2025 --output data/ --concurrency 8 --skip-existing
"""
from __future__ import annotations

import argparse
import re
import sys
import time
import urllib.request
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from threading import Lock
from typing import Iterator

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

BASE_URL = "https://www.nhc.noaa.gov/archive"
ALL_PRODUCTS = ("fstadv", "public", "discus", "wndprb")
ALL_BASINS = ("ep", "al", "cp")

# Minimum year in the NHC archive that has the four standard product types.
MIN_YEAR = 2008

_USER_AGENT = (
    "Mozilla/5.0 (compatible; nhc-archive-fetcher/1.0; "
    "+https://github.com/greggian/automatic-palm-tree)"
)

# ---------------------------------------------------------------------------
# HTTP helpers
# ---------------------------------------------------------------------------

def _fetch(url: str, retries: int = 4, backoff: float = 2.0) -> str:
    """Fetch *url* and return decoded text; retry with exponential back-off."""
    req = urllib.request.Request(url, headers={"User-Agent": _USER_AGENT})
    last: Exception | None = None
    for attempt in range(retries):
        try:
            with urllib.request.urlopen(req, timeout=30) as r:
                return r.read().decode("utf-8", errors="replace")
        except Exception as exc:
            last = exc
            if attempt < retries - 1:
                wait = backoff * (2 ** attempt)
                print(f"  [retry {attempt+1}] {exc} — sleeping {wait:.0f}s",
                      file=sys.stderr)
                time.sleep(wait)
    raise RuntimeError(f"Failed to fetch {url}: {last}") from last


_PRE_RE  = re.compile(r"<pre[^>]*>(.*?)</pre>",  re.DOTALL | re.IGNORECASE)
_TAG_RE  = re.compile(r"<[^>]+>")
_ENT_RE  = re.compile(r"&([a-z]+|#\d+);")
_ENTITIES = {"amp": "&", "lt": "<", "gt": ">", "quot": '"', "nbsp": " "}


def _unescape(text: str) -> str:
    def _sub(m: re.Match) -> str:
        name = m.group(1)
        if name.startswith("#"):
            return chr(int(name[1:]))
        return _ENTITIES.get(name, m.group(0))
    return _ENT_RE.sub(_sub, text)


def _extract_pre(html: str) -> str:
    """Return stripped text from the first <pre> block."""
    m = _PRE_RE.search(html)
    if not m:
        raise ValueError("no <pre> block found")
    return _unescape(_TAG_RE.sub("", m.group(1))).strip()


# ---------------------------------------------------------------------------
# Archive discovery
# ---------------------------------------------------------------------------

_YEAR_RE  = re.compile(r'href="(\d{4})/"')
_STORM_RE = re.compile(r'href="([a-z]{2}\d{2})/"', re.IGNORECASE)
_FILE_RE  = re.compile(
    r'href="([a-z]{2}\d{2}\d{4})\.(fstadv|public|discus|wndprb)\.(\d{3})\.shtml"',
    re.IGNORECASE,
)


def discover_years() -> list[int]:
    """Return list of years present in the NHC archive, newest first."""
    html = _fetch(f"{BASE_URL}/")
    years = [int(y) for y in _YEAR_RE.findall(html) if int(y) >= MIN_YEAR]
    years.sort(reverse=True)
    return years


def discover_storms(year: int, basins: set[str]) -> list[tuple[str, str]]:
    """
    Return list of (basin_dir, storm_id) pairs for *year*.
    basin_dir e.g. "ep04", storm_id e.g. "ep042025".
    """
    html = _fetch(f"{BASE_URL}/{year}/")
    storms = []
    for bd in _STORM_RE.findall(html):
        bd = bd.lower()
        basin = bd[:2]
        if basin not in basins:
            continue
        storm_id = f"{bd}{year}"
        storms.append((bd, storm_id))
    return storms


def discover_advisories(
    year: int,
    basin_dir: str,
    storm_id: str,
    products: set[str],
) -> list[tuple[str, str, str]]:
    """
    Return list of (storm_id, product, advisory_num) tuples.
    advisory_num is zero-padded to 3 digits, e.g. "001".
    """
    html = _fetch(f"{BASE_URL}/{year}/{basin_dir}/")
    results = []
    for sid, prod, num in _FILE_RE.findall(html):
        if sid.lower() != storm_id.lower():
            continue
        if prod.lower() not in products:
            continue
        results.append((storm_id.lower(), prod.lower(), num))
    return results


# ---------------------------------------------------------------------------
# Download worker
# ---------------------------------------------------------------------------

_print_lock = Lock()


def _download_one(
    year: int,
    basin_dir: str,
    storm_id: str,
    product: str,
    adv_num: str,
    out_dir: Path,
    skip_existing: bool,
) -> tuple[str, bool, str]:
    """
    Download one advisory file.

    Returns (filename, success, message).
    """
    filename = f"{storm_id}.{product}.{adv_num}.txt"
    out_path = out_dir / str(year) / filename

    if skip_existing and out_path.exists():
        return filename, True, "skipped (exists)"

    url = (
        f"{BASE_URL}/{year}/{basin_dir}/"
        f"{storm_id}.{product}.{adv_num}.shtml"
    )
    try:
        html = _fetch(url)
        text = _extract_pre(html)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(text + "\n", encoding="utf-8")
        return filename, True, f"OK ({len(text):,} chars)"
    except Exception as exc:
        return filename, False, str(exc)


# ---------------------------------------------------------------------------
# High-level download jobs
# ---------------------------------------------------------------------------

def _jobs_for_storm(
    year: int,
    basin_dir: str,
    storm_id: str,
    products: set[str],
) -> list[tuple]:
    """Discover all advisory files for one storm and return job tuples."""
    try:
        advisories = discover_advisories(year, basin_dir, storm_id, products)
    except Exception as exc:
        print(f"  Warning: could not index {storm_id}: {exc}", file=sys.stderr)
        return []
    return [
        (year, basin_dir, sid, prod, num)
        for sid, prod, num in advisories
    ]


def run_downloads(
    jobs: list[tuple],
    out_dir: Path,
    concurrency: int,
    skip_existing: bool,
) -> tuple[int, int]:
    """Execute download jobs in parallel. Returns (ok, errors)."""
    ok = errors = 0

    def _work(job: tuple):
        year, bd, sid, prod, num = job
        return _download_one(year, bd, sid, prod, num, out_dir, skip_existing)

    with ThreadPoolExecutor(max_workers=concurrency) as pool:
        futures = {pool.submit(_work, j): j for j in jobs}
        for future in as_completed(futures):
            filename, success, msg = future.result()
            with _print_lock:
                status = "OK  " if success else "ERR "
                print(f"  {status} {filename}  {msg}")
            if success:
                ok += 1
            else:
                errors += 1

    return ok, errors


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def _parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="Download NHC tropical cyclone advisories from the archive.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )

    mode = p.add_mutually_exclusive_group(required=True)
    mode.add_argument(
        "--storm",
        nargs=3,
        metavar=("BASIN", "NUM", "YEAR"),
        help="Download one storm, e.g. --storm EP 4 2025",
    )
    mode.add_argument(
        "--year",
        type=int,
        metavar="YEAR",
        help="Download all storms in YEAR",
    )
    mode.add_argument(
        "--all",
        action="store_true",
        help="Download every storm in the archive",
    )

    p.add_argument(
        "--basin",
        metavar="BASIN",
        help="Comma-separated basin filter: EP, AL, CP (default: all)",
    )
    p.add_argument(
        "--product",
        metavar="PRODUCT",
        help="Comma-separated product filter: fstadv,public,discus,wndprb (default: all)",
    )
    p.add_argument(
        "--output",
        metavar="DIR",
        default="nhc_data",
        help="Output directory (default: nhc_data/)",
    )
    p.add_argument(
        "--concurrency",
        type=int,
        default=4,
        metavar="N",
        help="Parallel download workers (default: 4)",
    )
    p.add_argument(
        "--skip-existing",
        action="store_true",
        help="Skip files that already exist on disk",
    )
    return p.parse_args()


def _resolve_basins(basin_arg: str | None) -> set[str]:
    if basin_arg is None:
        return set(ALL_BASINS)
    result = {b.strip().lower() for b in basin_arg.split(",")}
    bad = result - set(ALL_BASINS)
    if bad:
        sys.exit(f"Unknown basin(s): {', '.join(bad)}. Choose from: EP, AL, CP")
    return result


def _resolve_products(product_arg: str | None) -> set[str]:
    if product_arg is None:
        return set(ALL_PRODUCTS)
    result = {p.strip().lower() for p in product_arg.split(",")}
    bad = result - set(ALL_PRODUCTS)
    if bad:
        sys.exit(f"Unknown product(s): {', '.join(bad)}. Choose from: {', '.join(ALL_PRODUCTS)}")
    return result


def main() -> None:
    args = _parse_args()
    basins   = _resolve_basins(args.basin)
    products = _resolve_products(args.product)
    out_dir  = Path(args.output)

    # ── Collect jobs ──────────────────────────────────────────────────────────

    all_jobs: list[tuple] = []

    if args.storm:
        basin_raw, num_raw, year_raw = args.storm
        basin = basin_raw.lower()
        if basin not in ALL_BASINS:
            sys.exit(f"Unknown basin '{basin_raw}'. Choose from: EP, AL, CP")
        try:
            num  = int(num_raw)
            year = int(year_raw)
        except ValueError:
            sys.exit("--storm BASIN NUM YEAR: NUM and YEAR must be integers")
        basin_dir = f"{basin}{num:02d}"
        storm_id  = f"{basin_dir}{year}"
        print(f"Indexing {storm_id} …")
        all_jobs = _jobs_for_storm(year, basin_dir, storm_id, products)

    elif args.year:
        year = args.year
        print(f"Discovering storms in {year} …")
        storms = discover_storms(year, basins)
        print(f"  Found {len(storms)} storm(s): {', '.join(s for _, s in storms)}")
        for bd, sid in storms:
            print(f"  Indexing {sid} …")
            all_jobs += _jobs_for_storm(year, bd, sid, products)

    else:  # --all
        print("Discovering available years …")
        years = discover_years()
        print(f"  Found {len(years)} year(s): {', '.join(str(y) for y in years)}")
        for year in years:
            print(f"  Discovering storms in {year} …")
            storms = discover_storms(year, basins)
            print(f"    {len(storms)} storm(s)")
            for bd, sid in storms:
                all_jobs += _jobs_for_storm(year, bd, sid, products)

    if not all_jobs:
        print("No advisory files found.")
        return

    # ── Download ──────────────────────────────────────────────────────────────

    print(f"\nDownloading {len(all_jobs)} file(s) → {out_dir}/  "
          f"(concurrency={args.concurrency}) …\n")

    ok, errors = run_downloads(all_jobs, out_dir, args.concurrency, args.skip_existing)

    print(f"\nDone: {ok} downloaded, {errors} errors.")
    if errors:
        sys.exit(1)


if __name__ == "__main__":
    main()
