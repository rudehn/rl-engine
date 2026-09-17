---
created: 2026-09-17T04:17:22.126Z
title: Make movement profiles change pathing costs
area: rl-bevy
files:
  - crates/rl-bevy/src/minds.rs:127-146
  - crates/rl-bevy/src/minds.rs:99
---

## Problem

`FlowFields::ensure` keys its cache by `MovementProfile` (`FieldKey`, line 99) but builds and rescans every `DijkstraMap` with `PathRules::default()`.
A swimmer and a walker therefore path over the same map, and the sailing profile the plan's river section promised is not wired.
No test would catch the bug or prove a fix, so it stays latent until a game defines two profiles.
Already listed in `docs/TODO.md` section 3, "Movement profiles that change costs"; the architecture review (`.planning/codebase/CONCERNS.md`) rates the missing test high.

## Solution

Write the property test first: two profiles with different walkability over a seed range must produce different fields where the masks differ.
Then give `TileProps` a per-profile walkability mask and have the flood and rescan read it.
