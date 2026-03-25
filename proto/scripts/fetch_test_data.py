#!/usr/bin/env python3
"""
Fetch real NHC advisory text from the NHC archive and save to test_data/.

Usage
-----
    python scripts/fetch_test_data.py

This requires network access to www.nhc.noaa.gov.  Run it from outside
any sandbox environment that blocks that host.

The script fetches advisory numbers 1, 8, and 14 for all four product types
for Tropical Storm / Hurricane Dalila (EP04 2025).  Adjust STORM_ID,
ADVISORIES, and PRODUCTS as needed for other storms.
"""
from __future__ import annotations

import re
import sys
import time
import urllib.request
from pathlib import Path

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

STORM_ID = "ep042025"          # lowercase, matches archive path
BASIN = "ep04"                 # sub-directory under /archive/<year>/
YEAR = "2025"
ADVISORIES = ["001", "008", "014"]
PRODUCTS = ["fstadv", "public", "discus", "wndprb"]
BASE_URL = f"https://www.nhc.noaa.gov/archive/{YEAR}/{BASIN}/"
OUT_DIR = Path(__file__).parent.parent / "test_data"

# ---------------------------------------------------------------------------
# HTML stripper
# ---------------------------------------------------------------------------

_PRE_RE = re.compile(r"<pre[^>]*>(.*?)</pre>", re.DOTALL | re.IGNORECASE)
_TAG_RE = re.compile(r"<[^>]+>")


def extract_pre(html: str) -> str:
    """Return the text inside the first <pre> block, HTML-stripped."""
    m = _PRE_RE.search(html)
    if not m:
        raise ValueError("No <pre> block found in HTML")
    return _TAG_RE.sub("", m.group(1)).strip()


# ---------------------------------------------------------------------------
# Fetcher
# ---------------------------------------------------------------------------

def fetch_url(url: str, retries: int = 3, backoff: float = 2.0) -> str:
    headers = {
        "User-Agent": (
            "Mozilla/5.0 (compatible; nhc-parser-fetcher/1.0; "
            "+https://github.com/greggian/automatic-palm-tree)"
        )
    }
    req = urllib.request.Request(url, headers=headers)
    last_exc: Exception | None = None
    for attempt in range(retries):
        try:
            with urllib.request.urlopen(req, timeout=30) as resp:
                return resp.read().decode("utf-8", errors="replace")
        except Exception as exc:
            last_exc = exc
            if attempt < retries - 1:
                wait = backoff * (2 ** attempt)
                print(f"  Retry in {wait:.0f}s ({exc})", file=sys.stderr)
                time.sleep(wait)
    raise RuntimeError(f"Failed to fetch {url}: {last_exc}") from last_exc


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    total = 0
    errors = 0

    for adv_num in ADVISORIES:
        for product in PRODUCTS:
            filename = f"{STORM_ID}.{product}.{adv_num}.txt"
            out_path = OUT_DIR / filename
            url = f"{BASE_URL}{STORM_ID}.{product}.{adv_num}.shtml"

            print(f"Fetching {url} ...", end=" ", flush=True)
            try:
                html = fetch_url(url)
                text = extract_pre(html)
                out_path.write_text(text + "\n", encoding="utf-8")
                print(f"OK ({len(text)} chars) -> {out_path.name}")
                total += 1
            except Exception as exc:
                print(f"ERROR: {exc}", file=sys.stderr)
                errors += 1

    print(f"\nDone: {total} files saved, {errors} errors.")
    if errors:
        sys.exit(1)


if __name__ == "__main__":
    main()
