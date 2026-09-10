#!/usr/bin/env bash
# Tier 0 and tier 1 crates must never depend on Bevy. Conventions did not
# hold this line in the repos rl-engine was extracted from; the build does.
set -euo pipefail
cd "$(dirname "$0")/.."
tier1=(rl-core rl-grid rl-mapgen rl-world rl-test-support)
status=0
for crate in "${tier1[@]}"; do
  if cargo tree -p "$crate" -e normal --prefix none 2>/dev/null | grep -qE '^bevy'; then
    echo "FAIL: $crate depends on bevy" >&2
    status=1
  else
    echo "ok: $crate is bevy-free"
  fi
done
exit $status
