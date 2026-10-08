#!/usr/bin/env bash
# Lean check (Amendment Q §8.4): reports what makes the project heavier and fails on growth past
# the thresholds below. Raising a threshold needs a DECISIONS.md entry saying why.
# Usage: scripts/lean-check.sh [--full]   (--full also times a clean build and the test suite,
# and lists duplicate dependency versions; audits run it so)
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

# Thresholds (Audit 0's baseline, 2026-10-08).
MAX_DEAD_ALLOWS=12        # allow(dead_code) / allow(unused…) in crates and tools
MAX_LONG_FILES=8          # Rust files over 2,000 lines
MAX_PROGRESS_KB=25
MAX_DECISIONS_KB=40
MAX_PLAN_KB=20
MAX_BLOB_KB=1024          # any one tracked file
MAX_REPO_TRACKED_MB=40    # all tracked files together

fail=0
warn() { echo "  FAIL: $*"; fail=1; }

echo "==> dead-code allowances"
n=$(grep -rnE 'allow\((dead_code|unused[a-z_]*)' crates tools --include='*.rs' | wc -l)
echo "  $n (max $MAX_DEAD_ALLOWS)"
[ "$n" -le "$MAX_DEAD_ALLOWS" ] || warn "dead-code allowances grew to $n"

echo "==> Rust files over 2,000 lines"
long=$(git ls-files '*.rs' | xargs wc -l | awk '$2 != "total" && $1 > 2000 {print $1, $2}' | sort -rn)
echo "$long" | sed 's/^/  /'
n=$(echo "$long" | grep -c . || true)
echo "  $n (max $MAX_LONG_FILES)"
[ "$n" -le "$MAX_LONG_FILES" ] || warn "$n files over 2,000 lines"

echo "==> restart files"
for f in PROGRESS.md:$MAX_PROGRESS_KB DECISIONS.md:$MAX_DECISIONS_KB PLAN.md:$MAX_PLAN_KB; do
  file=${f%%:*}; max=${f##*:}
  kb=$(( $(wc -c < "$file") / 1024 ))
  echo "  $file ${kb} KB (max $max)"
  [ "$kb" -le "$max" ] || warn "$file is ${kb} KB"
done

echo "==> repository"
total=0
big=""
while read -r size path; do
  total=$((total + size))
  [ "$size" -gt $((MAX_BLOB_KB * 1024)) ] && big+="  $((size / 1024)) KB $path"$'\n'
done < <(git ls-files -z | xargs -0 stat -c '%s %n' 2>/dev/null)
echo "  tracked: $((total / 1048576)) MB (max $MAX_REPO_TRACKED_MB)"
[ "$((total / 1048576))" -le "$MAX_REPO_TRACKED_MB" ] || warn "tracked files total $((total / 1048576)) MB"
if [ -n "$big" ]; then printf '%s' "$big"; warn "files over $MAX_BLOB_KB KB"; fi

echo "==> unused dependencies (declared but never named in the crate's sources)"
for toml in crates/*/Cargo.toml tools/*/Cargo.toml; do
  [ -f "$toml" ] || continue
  dir=$(dirname "$toml")
  deps=$(awk '/^\[(dev-)?dependencies\]/{on=1; next} /^\[/{on=0} on && /^[a-zA-Z0-9_-]+(\.workspace)? *=/{sub(/\.workspace.*/, "", $1); print $1}' "$toml")
  for d in $deps; do
    name=${d//-/_}
    if ! grep -rqE "\b$name\b" "$dir/src" "$dir/tests" "$dir/benches" "$dir/examples" "$dir/build.rs" 2>/dev/null; then
      warn "$toml: $d is not used"
    fi
  done
done

echo "==> shipped assets without a license entry"
if [ -f ASSETS_LICENSES.md ]; then
  for a in $(git ls-files 'assets/**' 'data/**/*.png' 'data/**/*.jpg' 'data/**/*.ogg' 'data/**/*.ttf' 'data/**/*.otf' 2>/dev/null); do
    grep -qF "$(basename "$a")" ASSETS_LICENSES.md || warn "$a has no entry in ASSETS_LICENSES.md"
  done
else
  echo "  (no ASSETS_LICENSES.md: no third-party assets shipped)"
fi

if [ "${1:-}" = "--full" ]; then
  echo "==> duplicate dependency versions"
  cargo tree -d --workspace -e normal --depth 0 2>/dev/null | grep -E '^[a-z]' | sort -u | sed 's/^/  /'
  echo "==> clean release build time"
  cargo clean -q --release -p hearth
  s=$(date +%s); cargo build -q --release -p hearth; echo "  hearth: $(( $(date +%s) - s )) s"
  echo "==> test suite time (release)"
  s=$(date +%s); cargo test -q --release --workspace >/dev/null; echo "  $(( $(date +%s) - s )) s"
fi

if [ "$fail" -ne 0 ]; then
  echo "lean check: over a threshold (see FAIL lines)" >&2
  exit 1
fi
echo "lean check: within thresholds."
