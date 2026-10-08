#!/usr/bin/env bash
# The soak suite (Amendment Q §5.5): the long runs kept out of the default suite — the bots'
# acceptance runs, fifty years of each biome's animals, the coasts' generation, a year from a
# loincloth. Audits run it; so does any change to what those runs cover. About an hour on four
# cores. Usage: scripts/soak.sh [FILTER]   (only the soak tests whose names contain FILTER)
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
# Tools that are ignored for other reasons are skipped: the fixture writer rewrites a fixture,
# the surveys only print what lies about a spawn. The year from a loincloth waits to be cut to
# Earth's clock (PLAN.md, "From E3").
cargo test --profile dev-opt --workspace --no-fail-fast -- --ignored \
  --skip write_placeholders_fixture --skip what_lies_about_the_spawn \
  --skip fresh_water_about_the_spawn --skip a_year_from_a_loincloth ${1:+"$1"}
