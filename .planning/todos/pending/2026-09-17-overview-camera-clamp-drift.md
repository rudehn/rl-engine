---
created: 2026-09-17T04:17:22.126Z
title: Note the camera clamp in OVERVIEW.md
area: docs
files:
  - docs/OVERVIEW.md
  - crates/rl-render/src/map_view.rs
---

## Problem

`docs/OVERVIEW.md` is current as of `3ff1c58`.
Since then `MapView::clamp_to` landed and `follow_player` keeps the camera inside map bounds, a behaviour change every game sees on screen, and the overview does not mention it.
Its "Last updated" stamp also predates the 2026-09-16 work it already describes.

## Solution

One line in the rl-render section and a corrected stamp.
