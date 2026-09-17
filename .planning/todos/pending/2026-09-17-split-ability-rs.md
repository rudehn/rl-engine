---
created: 2026-09-17T04:17:22.126Z
title: Split rl-bevy ability.rs
area: rl-bevy
files:
  - crates/rl-bevy/src/ability.rs
---

## Problem

`crates/rl-bevy/src/ability.rs` is 1,717 lines, up from the 1,562 `docs/TODO.md` section 4 measured on 2026-09-15, and still growing.
It holds state components, the effect registry, the gate, payment, `Offered`, the `Known` refresh, airborne landings and cue emission.

## Solution

As TODO.md section 4 describes: state, registry and resolver submodules, and named `SystemParam`s in place of the `Spender` and `Bearing` tuple aliases.
Update the measured size in TODO.md or do the split.
