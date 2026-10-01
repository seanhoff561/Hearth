#!/usr/bin/env bash
# Performance regression gate, run at the end of every milestone (docs/perf-audit.md, D59).
#
# Builds this working tree and the baseline commit named in perf/baseline (in a git worktree
# under bench-out/gate), runs the quick benchmark scenes on both, alternating, and compares the
# medians: if this tree's average FPS or 1 % lows fall more than 5 % below the baseline's in
# any scene, the gate fails. Fix the regression, or justify it in DECISIONS.md and move the
# baseline with --accept (on a clean, committed tree; then commit perf/baseline).
#
# Usage: scripts/perf-gate.sh [--rounds N] [--frames N] [--gate PCT] [--accept]
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

ROUNDS=3
FRAMES=600
GATE=5
ACCEPT=0
while [ $# -gt 0 ]; do
  case "$1" in
    --rounds) ROUNDS=$2; shift 2 ;;
    --frames) FRAMES=$2; shift 2 ;;
    --gate) GATE=$2; shift 2 ;;
    --accept) ACCEPT=1; shift ;;
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

ROOT=$(pwd)
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
  "$@" bench --scenes quick --frames "$FRAMES" --report none
}
for i in $(seq 1 "$ROUNDS"); do
  echo "==> round $i of $ROUNDS"
  (cd "$WT" && bench "$OLD" --cache "$OUT/cache-base" --json "$OUT/base-$i.json")
  bench "$NEW" --cache "$OUT/cache-new" --json "$OUT/new-$i.json"
done

list() {
  local files=()
  for i in $(seq 1 "$ROUNDS"); do files+=("$OUT/$1-$i.json"); done
  (IFS=,; echo "${files[*]}")
}
echo "==> verdict"
"$NEW" bench --judge --baseline "$(list base)" --candidate "$(list new)" --gate "$GATE"
