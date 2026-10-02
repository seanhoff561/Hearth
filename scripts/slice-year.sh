#!/usr/bin/env bash
# The vertical slice's year (V2-9): the year bot from a loincloth in spring through a whole
# year, its log in bench-out/slice/year.log and copies of the world at its moments in
# bench-out/slice/<moment>/ (render them with `hearth --screenshot "save=bench-out/slice/<moment>/,..."`).
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
cargo test -p hearth --test slice_year -- --ignored --nocapture "$@"
