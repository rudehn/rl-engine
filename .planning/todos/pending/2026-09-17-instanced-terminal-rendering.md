---
created: 2026-09-17T04:17:22.126Z
title: Instanced terminal rendering
area: rl-render
files:
  - crates/rl-render/src/terminal.rs:190-225
---

## Problem

The terminal spawns one `Sprite` and one `Text2d` entity per cell (6,400 entities at 80x40), each queried and drawn individually.
It is the known scaling ceiling for terminal size, and `docs/PLAN.md` sections 2 and 6 rule out one entity per cell.
Tracked in `docs/OVERVIEW.md`, "Not built yet".

## Solution

One mesh with per-instance glyph and colour data, fed from the same diff the current presenter uses.
Land the render bench (see the per-frame bench todo) first so the gain is measured.
