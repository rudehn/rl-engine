---
created: 2026-09-17T04:17:22.126Z
title: Require OnMap on Position
area: rl-bevy
files:
  - crates/rl-bevy/src/turn.rs
  - crates/rl-bevy/src/minds.rs
  - crates/rl-bevy/src/places.rs
  - crates/rl-bevy/src/items.rs
---

## Problem

About 21 call sites across engine crates and the example games write `unwrap_or(MapId::SURFACE)` on an optional `OnMap`, and tutorial step 1 has to explain why a delve's first floor is map one.
Tracked in `docs/TODO.md` section 4, "`OnMap` as a required component"; the review found it spreading since.

## Solution

Make `OnMap` a required component of `Position` so the `Option` and every fallback disappear.
