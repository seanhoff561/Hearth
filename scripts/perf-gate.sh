#!/usr/bin/env bash
# Performance regression gate, run at the end of every milestone (docs/perf-audit.md, D59).
#
# Builds this working tree and the baseline commit named in perf/baseline (in a git worktree
# under bench-out/gate), runs the quick benchmark scenes on both, alternating (and which goes
# first alternating by round), and compares the medians: if this tree's average FPS or 1 %
# lows fall more than 5 % below the baseline's in any scene, the gate fails. Fix the
# regression, or justify it in DECISIONS.md and move the baseline with --accept (on a clean,
# committed tree; then commit perf/baseline).
#
# The Earth-scale entries (E4.1 §5) run beside them: `hearth bench globe`, `creator` and `load`
# on both builds, judged by `hearth bench earth-judge` (the globe's map, hover and click; the
# creator's build and a slider's lag; Play to control for a new world and its save, the whole
# render distance, peak memory, the main thread's frames while loading: each median no more than
# the gate worse beyond a small floor; and no tile of the finest level built for a coarse
# caller). --no-earth leaves them out (they take some minutes a round).
#
# Usage: scripts/perf-gate.sh [--rounds N] [--frames N] [--gate PCT] [--no-earth] [--accept]
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

ROUNDS=3
FRAMES=600
GATE=5
ACCEPT=0
EARTH=1
while [ $# -gt 0 ]; do
  case "$1" in
    --rounds) ROUNDS=$2; shift 2 ;;
    --frames) FRAMES=$2; shift 2 ;;
    --gate) GATE=$2; shift 2 ;;
    --accept) ACCEPT=1; shift ;;
    --no-earth) EARTH=0; shift ;;
    -h|--help) sed -n '2,10p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

if [ "$ACCEPT" = 1 ]; then
  if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
    echo "commit first: the baseline is a commit" >&2
    exit 2
  fi
  mkdir -p perf
  git rev-parse HEAD > perf/baseline
  echo "baseline is now $(cat perf/baseline); commit perf/baseline"
  exit 0
fi

BASE=$(cat perf/baseline 2>/dev/null || true)
if [ -z "$BASE" ]; then
  echo "no baseline: run scripts/perf-gate.sh --accept on a committed tree" >&2
  exit 2
fi

# Paths as the benchmark binaries take them (Windows paths under Git Bash).
native() {
  if command -v cygpath >/dev/null 2>&1; then cygpath -m "$1"; else echo "$1"; fi
}
ROOT=$(native "$(pwd)")
OUT="$ROOT/bench-out/gate"
WT="$OUT/base-src"
mkdir -p "$OUT"
rm -f "$OUT"/base-*.json "$OUT"/new-*.json

echo "==> building this tree"
cargo build --release -p hearth
NEW="$ROOT/target/release/hearth"

echo "==> building the baseline $BASE"
if [ -d "$WT" ]; then
  git -C "$WT" checkout -q --detach "$BASE"
else
  git worktree add -q --detach "$WT" "$BASE"
fi
# The baseline runs from its own tree: its data packs, its world cache, its commit id.
(cd "$WT" && CARGO_TARGET_DIR="$OUT/base-target" cargo build --release -p hearth)
OLD="$OUT/base-target/release/hearth"

bench() {
  local exe=$1
  shift
  "$exe" bench --scenes quick --frames "$FRAMES" --report none "$@"
}
run_base() {
  if ! (cd "$WT" && bench "$OLD" --cache "$OUT/cache-base" --json "$OUT/base-$1.json") \
    > "$OUT/base-$1.log" 2>&1; then
    tail -n 5 "$OUT/base-$1.log"
    exit 2
  fi
}
run_new() {
  if ! bench "$NEW" --cache "$OUT/cache-new" --json "$OUT/new-$1.json" \
    > "$OUT/new-$1.log" 2>&1; then
    tail -n 5 "$OUT/new-$1.log"
    exit 2
  fi
}
# A baseline from before the Earth-scale benchmarks cannot run them: the gate then judges the
# scenes alone (accept a newer baseline to gate them too).
if [ "$EARTH" = 1 ] && [ ! -f "$WT/crates/hearth/src/bench_earth.rs" ]; then
  echo "note: the baseline has no Earth-scale benchmarks; they are not judged"
  EARTH=0
fi
# The Earth-scale benchmarks of one build (E4.1 §5), each writing its numbers.
earth() {
  local side=$1 exe=$2 dir=$3 i=$4
  local what
  for what in globe creator load; do
    if ! (cd "$dir" && "$exe" bench "$what" --seed 7 --json "$OUT/$side-$what-$i.json") \
      > "$OUT/$side-$what-$i.log" 2>&1; then
      tail -n 5 "$OUT/$side-$what-$i.log"
      exit 2
    fi
  done
}
# Which build runs first alternates by round: the second of a pair can run slower (D74).
for i in $(seq 1 "$ROUNDS"); do
  echo "==> round $i of $ROUNDS (logs in bench-out/gate)"
  if [ $((i % 2)) = 1 ]; then
    run_base "$i"
    run_new "$i"
    if [ "$EARTH" = 1 ]; then
      earth base "$OLD" "$WT" "$i"
      earth new "$NEW" "$ROOT" "$i"
    fi
  else
    run_new "$i"
    run_base "$i"
    if [ "$EARTH" = 1 ]; then
      earth new "$NEW" "$ROOT" "$i"
      earth base "$OLD" "$WT" "$i"
    fi
  fi
done

list() {
  local files=()
  for i in $(seq 1 "$ROUNDS"); do files+=("$OUT/$1-$i.json"); done
  (IFS=,; echo "${files[*]}")
}
echo "==> verdict"
VERDICT=0
"$NEW" bench --judge --baseline "$(list base)" --candidate "$(list new)" --gate "$GATE" || VERDICT=$?
if [ "$EARTH" = 1 ]; then
  earth_list() {
    local files=() what
    for i in $(seq 1 "$ROUNDS"); do
      for what in globe creator load; do files+=("$OUT/$1-$what-$i.json"); done
    done
    (IFS=,; echo "${files[*]}")
  }
  echo "==> the Earth-scale verdict"
  "$NEW" bench earth-judge --baseline "$(earth_list base)" --candidate "$(earth_list new)" \
    --gate "$GATE" || VERDICT=$?
fi
exit "$VERDICT"
