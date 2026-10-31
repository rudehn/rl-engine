#!/usr/bin/env bash
# The gate a commit runs before it is made, and a branch again after a rebase:
# fmt, the name and prelude checks, clippy, every test with the doc-tests, the
# tier boundary and the guide's links into the code. CI runs the same, and plays every seed, builds the wasm
# crates and checks the docs. ATGLYPH_FULL_SEEDS=1 is passed through.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

step() {
    echo "gate: $*"
    "$@"
}

step cargo fmt --all --check
step scripts/check-names.sh
step scripts/check-prelude.py
step cargo clippy --workspace --all-targets -- -D warnings
step cargo test --workspace
step scripts/check-tiers.sh
step scripts/check-guide.sh
echo "gate: passed"
