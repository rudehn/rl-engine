---
created: 2026-09-17T04:17:22.126Z
title: Bench the per-frame loop
area: rl-bevy
files:
  - crates/rl-grid/benches/grid.rs
  - crates/rl-bevy/src/plugin.rs
  - crates/rl-bevy/src/minds.rs
  - crates/rl-render/src/terminal.rs
---

## Problem

The only criterion benches are in `crates/rl-grid/benches/grid.rs` (fov, astar, dijkstra, regions, light, tile_field).
The `Turn` schedule, the minds `PerceiveSet` pipeline and the terminal draw pass have none, so an O(n^2) regression in, for example, the `Thinking` snapshot would show up only as a felt frame drop.
`docs/PLAN.md` section 3.9 also promises benches for the choke map, each builder, the world chain at three sizes and the render sweep.

## Solution

Headless criterion benches on a realistic map with a realistic actor count: a turn pass, a minds decision pass, and a terminal draw/diff pass.
