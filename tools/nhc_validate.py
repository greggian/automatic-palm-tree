#!/usr/bin/env python3
"""
nhc_validate.py — Parse every downloaded NHC advisory and report results.

Scans a directory tree for advisory text files (named
<storm_id>.<product>.<NNN>.txt), attempts to parse each one using the Python
prototype parser, and prints a summary of successes and failures.

USAGE
-----
  python tools/nhc_validate.py                    # scans nhc_data/
  python tools/nhc_validate.py data/              # custom directory
  python tools/nhc_validate.py --product fstadv   # one product type only
  python tools/nhc_validate.py -v                 # show passing files too
  python tools/nhc_validate.py --stop-on-error    # bail on first failure
"""
from __future__ import annotations

import argparse
import sys
import traceback
from dataclasses import dataclass, field
from pathlib import Path

# Add proto/ to the path so we can import the Python parser.
_REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(_REPO_ROOT / "proto"))

try:
    from nhc_parser.fstadv import parse_fstadv
    from nhc_parser.public  import parse_public
    from nhc_parser.discus  import parse_discus
    from nhc_parser.wndprb  import parse_wndprb
except ImportError as e:
    sys.exit(
        f"Cannot import nhc_parser: {e}\n"
        f"Make sure proto/nhc_parser/ exists and dependencies are installed."
    )

# Map product name → parse function
_PARSERS = {
    "fstadv": parse_fstadv,
    "public":  parse_public,
    "discus":  parse_discus,
    "wndprb":  parse_wndprb,
}

# File name pattern: <storm_id>.<product>.<NNN>.txt
# storm_id example: ep042025
_VALID_PRODUCTS = set(_PARSERS)


# ---------------------------------------------------------------------------
# Result types
# ---------------------------------------------------------------------------

@dataclass
class FileResult:
    path: Path
    product: str
    ok: bool
    error: str = ""
    tb: str = ""


@dataclass
class Summary:
    results: list[FileResult] = field(default_factory=list)

    @property
    def ok(self) -> list[FileResult]:
        return [r for r in self.results if r.ok]

    @property
    def failed(self) -> list[FileResult]:
        return [r for r in self.results if not r.ok]

    def by_product(self, prod: str) -> list[FileResult]:
        return [r for r in self.results if r.product == prod]


# ---------------------------------------------------------------------------
# File discovery
# ---------------------------------------------------------------------------

def _infer_product(path: Path) -> str | None:
    """
    Infer the product type from the filename.
    Expects  <anything>.<product>.<NNN>.txt
    """
    parts = path.stem.split(".")   # stem strips .txt
    # stem of "ep042025.fstadv.001" → ["ep042025", "fstadv", "001"]
    for part in parts:
        if part.lower() in _VALID_PRODUCTS:
            return part.lower()
    return None


def discover_files(root: Path, products: set[str]) -> list[tuple[Path, str]]:
    """Return (path, product) pairs for all matching .txt files under root."""
    found = []
    for path in sorted(root.rglob("*.txt")):
        product = _infer_product(path)
        if product is None:
            continue
        if product not in products:
            continue
        found.append((path, product))
    return found


# ---------------------------------------------------------------------------
# Parsing
# ---------------------------------------------------------------------------

def validate_file(path: Path, product: str) -> FileResult:
    parse = _PARSERS[product]
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
        parse(text)
        return FileResult(path=path, product=product, ok=True)
    except Exception as exc:
        tb = traceback.format_exc()
        return FileResult(
            path=path,
            product=product,
            ok=False,
            error=str(exc),
            tb=tb,
        )


# ---------------------------------------------------------------------------
# Reporting
# ---------------------------------------------------------------------------

# ANSI colours (disabled on Windows or when not a tty)
_USE_COLOUR = sys.stdout.isatty() and sys.platform != "win32"
_GRN = "\033[32m" if _USE_COLOUR else ""
_RED = "\033[31m" if _USE_COLOUR else ""
_YLW = "\033[33m" if _USE_COLOUR else ""
_RST = "\033[0m"  if _USE_COLOUR else ""


def _rel(path: Path, root: Path) -> str:
    try:
        return str(path.relative_to(root))
    except ValueError:
        return str(path)


def print_result(result: FileResult, root: Path, verbose: bool) -> None:
    rel = _rel(result.path, root)
    if result.ok:
        if verbose:
            print(f"  {_GRN}PASS{_RST}  {rel}")
    else:
        print(f"  {_RED}FAIL{_RST}  {rel}")
        # Show first line of the error (concise).
        first_line = result.error.split("\n")[0]
        print(f"        {_YLW}{first_line}{_RST}")


def print_summary(summary: Summary, verbose: bool) -> None:
    total  = len(summary.results)
    passed = len(summary.ok)
    failed = len(summary.failed)

    print()
    print("─" * 60)
    print(f"  Total : {total}")
    print(f"  {_GRN}Passed{_RST}: {passed}")
    if failed:
        print(f"  {_RED}Failed{_RST}: {failed}")

    # Per-product breakdown
    print()
    for prod in ("fstadv", "public", "discus", "wndprb"):
        rows = summary.by_product(prod)
        if not rows:
            continue
        p = sum(1 for r in rows if r.ok)
        f = len(rows) - p
        bar = f"{_GRN}{p} ok{_RST}"
        if f:
            bar += f"  {_RED}{f} fail{_RST}"
        print(f"  {prod:<8} {len(rows):>5} files   {bar}")

    if failed and not verbose:
        print()
        print(f"Re-run with -v to see which files passed.")
        print()
        print(f"{_RED}Failed files:{_RST}")
        for r in summary.failed:
            print(f"  {r.path}")
            print(f"    {_YLW}{r.error.split(chr(10))[0]}{_RST}")

    print("─" * 60)


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def _parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="Parse all downloaded NHC advisory files and report errors.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    p.add_argument(
        "directory",
        nargs="?",
        default="nhc_data",
        metavar="DIR",
        help="Root directory to scan (default: nhc_data/)",
    )
    p.add_argument(
        "--product",
        metavar="PRODUCT",
        help="Only validate this product type (fstadv, public, discus, wndprb)",
    )
    p.add_argument(
        "--stop-on-error",
        action="store_true",
        help="Stop after the first parse failure",
    )
    p.add_argument(
        "-v", "--verbose",
        action="store_true",
        help="Show passing files as well as failures",
    )
    p.add_argument(
        "--tb",
        action="store_true",
        help="Print full traceback for each failure",
    )
    return p.parse_args()


def main() -> None:
    args = _parse_args()

    root = Path(args.directory)
    if not root.exists():
        sys.exit(f"Directory not found: {root}")

    if args.product:
        prod = args.product.lower()
        if prod not in _VALID_PRODUCTS:
            sys.exit(
                f"Unknown product '{args.product}'. "
                f"Choose from: {', '.join(sorted(_VALID_PRODUCTS))}"
            )
        products = {prod}
    else:
        products = _VALID_PRODUCTS

    # Discover files
    files = discover_files(root, products)
    if not files:
        print(f"No advisory .txt files found under {root}/")
        sys.exit(0)

    print(f"Scanning {len(files)} file(s) under {root}/ …\n")

    summary = Summary()

    for path, product in files:
        result = validate_file(path, product)
        summary.results.append(result)
        print_result(result, root, args.verbose)

        if args.tb and not result.ok:
            print()
            print(result.tb)

        if args.stop_on_error and not result.ok:
            print(f"\n{_RED}Stopped on first error.{_RST}")
            print_summary(summary, args.verbose)
            sys.exit(1)

    print_summary(summary, args.verbose)

    if summary.failed:
        sys.exit(1)


if __name__ == "__main__":
    main()
