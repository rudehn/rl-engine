---
created: 2026-09-17T04:17:22.126Z
title: Split Heist's main.rs and test its stealth
area: examples
files:
  - examples/heist/src/main.rs
---

## Problem

`examples/heist/src/main.rs` is 1,076 lines holding input, floor population (`populate_floor`, about 98 lines), narration wiring and content loading, the same shape `docs/TODO.md` section 5 flags for Delve.
Heist postdates that review, so it is untracked.
The mechanic Heist exists to demonstrate - sound reaching `Aware`, a notice shared with everyone in earshot, shading a lantern - has no test, and `crates/rl-bevy/tests/fingerprint.rs` does not cover Heist's content.
A refactor of `StealthPlugin` or `LightingPlugin` could break the one example that uses them together without failing anything.

## Solution

Split into input, narration and content modules the way Corsair is, alongside the Delve split.
Add tests for the stealth-and-light interplay, ideally as properties over a seed range.
