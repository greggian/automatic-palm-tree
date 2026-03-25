# CLAUDE.md — NHC Forecast Parser

## Build & validate

```sh
cargo build --release          # full workspace
cargo test -p nhc-parser       # parser unit tests
cargo run -p nhc-validate --release   # parse all files in nhc_data/, expect 0 failures
```

## Project structure

| Crate | Role |
|---|---|
| `nhc-types` | Shared structs and enums (`Advisory`, `StormStatus`, etc.) |
| `nhc-parser` | PEG grammars + Rust extraction for all four product types |
| `nhc-validate` | CLI — walks `nhc_data/` and reports parse pass/fail |
| `nhc-download` | CLI — fetches advisories from NHC web archive |
| `nhc-ingest` | Polling service that writes parsed advisories to Postgres |

## Grammar architecture

pest has no native `@include` — grammars are composed via `build.rs`:

```
grammars/common.pest          shared rules (WHITESPACE, tokens, envelope lines)
grammars/fstadv_rules.pest    fstadv-specific rules
grammars/discus_rules.pest    discus-specific rules
grammars/public_rules.pest    public-specific rules
grammars/wndprb_rules.pest    wndprb-specific rules
         ↓  build.rs concatenates common.pest + *_rules.pest
grammars/fstadv.pest          generated (gitignored)
grammars/discus.pest          generated (gitignored)
grammars/public.pest          generated (gitignored)
grammars/wndprb.pest          generated (gitignored)
```

**When editing grammar rules:** edit `common.pest` or `*_rules.pest`, never the generated `.pest` files.

## Key conventions

- **Uppercase normalisation:** `parse_advisory()` in `lib.rs` calls `.to_uppercase()` on all input before grammar matching. Grammars assume ALL-CAPS text.
- **pest implicit WHITESPACE:** The `WHITESPACE` rule (`" " | "\t"`) is silently consumed between tokens in non-atomic rules. Atomic rules (`@{...}`) do not consume implicit whitespace.
- **Product detection:** AWIPS ID in the ZCZC line determines product type — `TCM` → fstadv, `TCP` → public, `TCD` → discus, `PWS` → wndprb.
- **Originators:** Three AWIPS prefixes exist — `MIA` (NHC Miami), `HFO` (CPHC Honolulu), `NFD` (Weather Prediction Center). Both grammar and Rust (`envelope.rs`) must handle all three.

## Known edge cases (2025 season)

These are already handled in the grammars/parsers but are non-obvious:

- **CCA/CCB corrections** on ZCZC and WMO header lines
- **CORRECTED** note line after issued time (public, fstadv)
- **ISSUED BY** line between NWS line and issued time
- **SPECIAL** variants: SPECIAL FORECAST/ADVISORY, SPECIAL DISCUSSION, SPECIAL WIND SPEED PROBABILITIES
- **EXTENDED OUTLOOK** blocks in fstadv between forecast blocks
- **STATIONARY** movement (public) — no cardinal/degrees/speed fields
- **Multiple "ABOUT" distance lines** between LOCATION and MAXIMUM SUSTAINED WINDS (public)
- **Two-line headers** in wndprb (storm name on one line, product title on next)
- **Missing NNNN** trailer (truncated corrected advisories)
- **REMNANTS OF / SUBTROPICAL STORM** status variants
- **Dissipated/merged storms** in fstadv — forecast positions replaced with `"..."` instead of lat/lon
