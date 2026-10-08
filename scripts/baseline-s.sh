#!/usr/bin/env bash
# Baseline-S (Amendment S §12.1, docs/spec/amendment-s-smooth-world.md): the numbers the smooth
# world is held to, measured on the machine this runs on before any of it reaches the game.
#
# With a graphics card it runs the benchmark scenes at the High and Low
# presets at 1440p (frame time p50, p99 and worst, GPU time per pass, triangles and draws,
# video memory), then the near terrain on the CPU (meshing speed, memory per surface cube,
# payload and edit sizes) and the smooth-mesher prototypes (`bench smooth`). Results are
# appended to BENCHMARKS.md and kept in bench-out/baseline-s/. With --cpu-only (a machine
# without a GPU, such as the cloud builder), only the terrain and prototype parts.
#
# Usage: scripts/baseline-s.sh [--cpu-only] [--frames N] [--size WxH]
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

CPU_ONLY=0
FRAMES=600
SIZE=2560x1440
while [ $# -gt 0 ]; do
  case "$1" in
    --cpu-only) CPU_ONLY=1; shift ;;
    --frames) FRAMES=$2; shift 2 ;;
    --size) SIZE=$2; shift 2 ;;
    -h|--help) sed -n '2,12p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

OUT=bench-out/baseline-s
mkdir -p "$OUT"
cargo build --release -p hearth -p bench
COMMIT=$(git rev-parse --short HEAD)

if [ "$CPU_ONLY" = 0 ]; then
  for preset in high low; do
    echo "==> the benchmark scenes at preset $preset, $SIZE"
    target/release/hearth bench --preset "$preset" --size "$SIZE" --frames "$FRAMES" \
      --label "Baseline-S, preset $preset, $COMMIT" --json "$OUT/$preset.json"
  done
fi
echo "==> the near terrain on the CPU"
target/release/hearth bench --terrain-only --label "Baseline-S, $COMMIT"
echo "==> the smooth-mesher prototypes"
target/release/bench smooth --out "$OUT/smooth"
echo "Baseline-S appended to BENCHMARKS.md; images and reports in $OUT"
