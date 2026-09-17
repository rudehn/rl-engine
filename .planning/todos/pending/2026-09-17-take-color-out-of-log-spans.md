---
created: 2026-09-17T04:17:22.126Z
title: Take Color out of log spans
area: rl-ui
files:
  - crates/rl-ui/src/log.rs:20-32
  - crates/rl-ui/src/narrate.rs:587
  - crates/rl-ui/src/narrate.rs:595
---

## Problem

`Span` in the log view carries `pub color: Color`, filled from a game's `Glyph::fg` in `collect_narration`.
CLAUDE.md says a view is plain data with no `Color`, and nothing documents this as a deliberate exception.
Because the colour is baked in when the line is collected, recolouring the `Palette` leaves old log lines in the old colours.
Untracked before the 2026-09-17 architecture review.

## Solution

Decide between a `ToneId` (or an entity-appearance key the presenter resolves at draw time) and a documented exception.
The rule and the recolour behaviour both argue for resolving the colour in the presenter.
