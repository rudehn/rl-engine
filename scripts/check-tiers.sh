#!/usr/bin/env bash
# The tier boundaries, read from each workspace member's
# `[package.metadata.rl-engine] tier`. Conventions did not hold these lines
# in the repos rl-engine was extracted from; the build does.
#
#   scripts/check-tiers.sh          every member declares a tier, no crate
#                                   depends on a higher tier, and tiers 0 and
#                                   1 never reach Bevy
#   scripts/check-tiers.sh --wasm   build tiers 0 and 1 for wasm32, and
#                                   rl-save, whose browser storage and
#                                   unload bridge exist only there
#
# Tiers: 0 core, 1 no Bevy, 2 the Bevy layer, 3 the facade and the games.
# The crate lists come from `cargo metadata`, never from this file, so a new
# or renamed crate cannot slip past. Any failing cargo command fails the
# check: a check that cannot run has not passed.
set -euo pipefail
cd "$(dirname "$0")/.."

# The highest tier that must never reach Bevy.
bevy_free_max=1
max_tier=3

metadata=$(cargo metadata --format-version 1 --no-deps)

# One "name tier" line per workspace member; "-" where the tier is missing
# or not an integer from 0 to max_tier.
members=$(jq -r --argjson max "$max_tier" '
  .packages[]
  | (.metadata["rl-engine"].tier // null) as $t
  | "\(.name) \(if ($t | type) == "number" and $t == ($t | floor) and $t >= 0 and $t <= $max then $t else "-" end)"
' <<<"$metadata")

bevy_free=()
while read -r name tier; do
  if [[ "$tier" != "-" && "$tier" -le "$bevy_free_max" ]]; then
    bevy_free+=("$name")
  fi
done <<<"$members"
if [[ ${#bevy_free[@]} -eq 0 ]]; then
  echo "FAIL: no workspace member declares tier 0 or 1; is [package.metadata.rl-engine] tier set?" >&2
  exit 1
fi

if [[ "${1:-}" == "--wasm" ]]; then
  packages=()
  for crate in "${bevy_free[@]}"; do packages+=(-p "$crate"); done
  # rl-save is tier 2, but its web backend and unload bridge are compiled
  # only for wasm32, so a native build never sees a mistake in them.
  exec cargo check "${packages[@]}" -p rl-save --target wasm32-unknown-unknown
elif [[ $# -gt 0 ]]; then
  echo "usage: scripts/check-tiers.sh [--wasm]" >&2
  exit 2
fi

status=0

while read -r name tier; do
  if [[ "$tier" == "-" ]]; then
    echo "FAIL: $name declares no tier; add [package.metadata.rl-engine] tier = 0 to $max_tier to its Cargo.toml" >&2
    status=1
  fi
done <<<"$members"

# Normal and build dependencies only: a dev-dependency never reaches a
# consumer.
upward=$(jq -r '
  (.packages | map({key: .name, value: .metadata["rl-engine"].tier}) | from_entries) as $tier
  | .packages[]
  | .name as $crate
  | select(($tier[$crate] | type) == "number")
  | .dependencies[]
  | select(.kind == null or .kind == "build")
  | select(($tier[.name] | type) == "number" and $tier[.name] > $tier[$crate])
  | "\($crate) (tier \($tier[$crate])) depends on \(.name) (tier \($tier[.name]))"
' <<<"$metadata")
if [[ -n "$upward" ]]; then
  while read -r line; do echo "FAIL: $line" >&2; done <<<"$upward"
  status=1
else
  echo "ok: no crate depends on a higher tier"
fi

# Every target, so a Bevy dependency behind a cfg cannot hide; build
# dependencies too, since they compile Bevy all the same.
for crate in "${bevy_free[@]}"; do
  if ! tree=$(cargo tree -p "$crate" -e normal,build --target all --prefix none); then
    echo "FAIL: cargo tree failed for $crate" >&2
    status=1
  elif reached=$(grep -m1 -E '^bevy' <<<"$tree"); then
    echo "FAIL: $crate depends on $reached; cargo tree -p $crate -i ${reached%% *} shows the path" >&2
    status=1
  else
    echo "ok: $crate is bevy-free"
  fi
done

exit $status
