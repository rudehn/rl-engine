---
created: 2026-09-17T04:17:22.126Z
title: Route Corsair's draws through Seed::stream
area: examples
files:
  - examples/corsair/src/items.rs:327
  - examples/corsair/src/statuses.rs:25
---

## Problem

Corsair draws from `ResMut<CombatRng>` in two places: `drop_loot` in `items.rs` and a system in `statuses.rs`.
The randomness rule says a game's own draws come from `Seed::stream`; the engine's `CombatRng` stream belongs to `CombatPlugin`.
`docs/TODO.md` section 4, "Streams out of the prelude", names only `items.rs`, so a fix pass that follows it leaves `statuses.rs` as precedent for a third.

## Solution

Remove `CombatRng` and `AbilityRng` from the preludes so the misuse stops compiling, then move both Corsair systems to their own `Seed::stream` streams.
Update the TODO.md item to name both files.
