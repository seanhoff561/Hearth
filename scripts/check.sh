#!/usr/bin/env bash
# Full local CI: formatting, lints (warnings are errors), the test suite and the content lint.
# Usage: scripts/check.sh [--quick]   (--quick skips the release-mode statistical tests)
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

echo "==> cargo fmt --check"
cargo fmt --all -- --check

echo "==> cargo clippy (deny warnings)"
cargo clippy --workspace --all-targets -- -D warnings

echo "==> cargo test (the dev-opt profile: optimized, no LTO)"
cargo test --profile dev-opt --workspace

echo "==> hearth content lint"
cargo run -q -p hearth -- content lint | tail -n 1

echo "==> no todo!/unimplemented! in shipped code"
if grep -rnE '\b(todo|unimplemented)!\(' crates tools --include='*.rs'; then
  echo "found todo!/unimplemented!" >&2
  exit 1
fi

echo "==> lean check (fast)"
scripts/lean-check.sh

echo "All checks passed."
