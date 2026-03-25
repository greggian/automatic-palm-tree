# NHC Forecast Parser

Rust workspace for downloading and parsing National Hurricane Center (NHC)
tropical cyclone advisories.

## Prerequisites

### Rust toolchain

Rust **1.80 or later** is required (`LazyLock` stabilised in 1.80).

```sh
# Install via rustup (https://rustup.rs)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Or update an existing installation
rustup update stable
```

Verify:

```sh
rustc --version   # rustc 1.80.0 or newer
cargo --version
```

### OpenSSL (Linux / macOS)

`reqwest` (used by `nhc-download`) links against the system OpenSSL.

| Platform | Install |
|---|---|
| Ubuntu / Debian | `sudo apt install pkg-config libssl-dev` |
| Fedora / RHEL | `sudo dnf install openssl-devel` |
| macOS (Homebrew) | `brew install openssl` |
| Windows | Use [`rustls`](https://github.com/rustls/rustls) feature or install via [vcpkg](https://github.com/microsoft/vcpkg) |

> **Note:** If you prefer to avoid the OpenSSL dependency entirely, switch
> the `reqwest` workspace dependency to use the `rustls-tls` feature instead
> of the default native-tls.

### PostgreSQL (nhc-ingest only)

`nhc-ingest` uses `sqlx` to write parsed advisories to a PostgreSQL database.
A running Postgres instance is only needed if you run `nhc-ingest`; the
download and validate tools have no database dependency.

```sh
# Ubuntu / Debian
sudo apt install postgresql

# macOS (Homebrew)
brew install postgresql@16
```

---

## Building

Build everything in the workspace:

```sh
cargo build --release
```

Or build individual crates:

```sh
cargo build -p nhc-download --release
cargo build -p nhc-validate --release
cargo build -p nhc-ingest   --release
```

Release binaries land in `target/release/`.

### Run the test suite

```sh
cargo test
# or just the parser tests
cargo test -p nhc-parser
```

---

## Dependency overview

| Crate | Key dependencies |
|---|---|
| `nhc-parser` | `pest` / `pest_derive` (PEG grammar), `chrono` |
| `nhc-download` | `tokio` (async runtime), `reqwest` (HTTP + TLS), `clap` (CLI), `regex` |
| `nhc-validate` | `nhc-parser`, `walkdir`, `clap` |
| `nhc-ingest` | `nhc-parser`, `nhc-types`, `tokio`, `reqwest`, `sqlx` (Postgres), `tracing` |

All dependency versions are pinned in `Cargo.lock`. The workspace-level
`Cargo.toml` defines shared version constraints under `[workspace.dependencies]`.

---

## Workspace crates

| Crate | Description |
|---|---|
| `nhc-types` | Shared data types (structs for advisories, forecasts, etc.) |
| `nhc-parser` | PEG grammar parsers for all four advisory products |
| `nhc-ingest` | Polling ingest service |
| `nhc-download` | CLI tool — download advisories from the NHC archive |
| `nhc-validate` | CLI tool — parse a directory of advisories and report results |

## Advisory products

| Product | AWIPS ID | Description |
|---|---|---|
| `fstadv` | `TCM` | Forecast/advisory — position, intensity, forecast track |
| `public` | `TCP` | Public advisory — plain-language synopsis |
| `discus` | `TCD` | Discussion — forecaster narrative |
| `wndprb` | `PWS` | Wind speed probabilities — probabilities by location |

Basins: **EP** (Eastern Pacific), **AL** (Atlantic), **CP** (Central Pacific).

---

## nhc-download

Download advisories from the [NHC public archive](https://www.nhc.noaa.gov/archive/).
Files are saved as `<output>/<year>/<storm_id>.<product>.<NNN>.txt`.

### Build

```sh
cargo build -p nhc-download --release
# binary at target/release/nhc-download
```

### Usage

```
nhc-download --storm  BASIN NUM YEAR  [options]
nhc-download --year   YEAR            [options]
nhc-download --all                    [options]
```

Exactly one of `--storm`, `--year`, or `--all` is required.

### Options

| Flag | Default | Description |
|---|---|---|
| `--storm BASIN NUM YEAR` | — | Download one storm, e.g. `--storm EP 4 2025` |
| `--year YEAR` | — | Download all storms in a calendar year |
| `--all` | — | Download every storm in the archive (2008 – present) |
| `--basin BASIN[,…]` | all | Filter by basin: `EP`, `AL`, `CP` |
| `--product PRODUCT[,…]` | all | Filter by product: `fstadv`, `public`, `discus`, `wndprb` |
| `--output DIR` | `nhc_data/` | Destination directory |
| `--concurrency N` | `4` | Parallel download workers |
| `--skip-existing` | off | Skip files already on disk |

### Examples

```sh
# One storm — EP storm #4 of 2025, all products
nhc-download --storm EP 4 2025

# All Atlantic storms in 2024, forecast advisories only
nhc-download --year 2024 --basin AL --product fstadv,public

# Full archive, 8 parallel workers, skip already-downloaded files
nhc-download --all --concurrency 8 --skip-existing --output /data/nhc

# 2023 EP + AL, wind probabilities only
nhc-download --year 2023 --basin EP,AL --product wndprb
```

### Output layout

```
nhc_data/
  2024/
    al092024.fstadv.001.txt
    al092024.fstadv.002.txt
    al092024.public.001.txt
    ...
  2025/
    ep042025.fstadv.001.txt
    ...
```

---

## nhc-validate

Walk a directory of downloaded advisories, parse each one with the Rust
`nhc-parser`, and print a pass/fail summary. Exits with code `1` if any
file fails to parse.

### Build

```sh
cargo build -p nhc-validate --release
# binary at target/release/nhc-validate
```

### Usage

```
nhc-validate [DIR] [options]
```

`DIR` defaults to `nhc_data/`.

### Options

| Flag | Default | Description |
|---|---|---|
| `DIR` | `nhc_data/` | Directory to scan (recursive) |
| `--product PRODUCT` | all | Only validate one product type |
| `--stop-on-error` | off | Stop after the first parse failure |
| `-v, --verbose` | off | Show passing files in addition to failures |

### Examples

```sh
# Validate everything in nhc_data/
nhc-validate

# Validate a different directory
nhc-validate /data/nhc

# Only check forecast advisories
nhc-validate --product fstadv

# Show all results, stop on first failure
nhc-validate --verbose --stop-on-error

# Combine: specific dir, one product, verbose
nhc-validate nhc_data/2024 --product discus --verbose
```

### Sample output

```
Scanning 48 file(s) under nhc_data/ …

  FAIL  2024/al092024.fstadv.003.txt
        --> 47:1
        |
     47 | ZCZC TCPAT3 ...

────────────────────────────────────────────────────────────
  Total : 48
  Passed: 47
  Failed: 1

  fstadv    18 files   17 ok  1 fail
  public    12 files   12 ok
  discus    12 files   12 ok
  wndprb     6 files    6 ok
────────────────────────────────────────────────────────────
```

---

## Typical workflow

```sh
# 1. Download one season
nhc-download --year 2024 --output nhc_data

# 2. Validate the downloads
nhc-validate nhc_data --verbose

# 3. Iterate on grammar fixes, re-validate
cargo test -p nhc-parser
nhc-validate nhc_data --product fstadv
```
