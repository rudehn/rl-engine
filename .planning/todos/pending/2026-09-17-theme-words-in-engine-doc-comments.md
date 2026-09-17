---
created: 2026-09-17T04:17:22.126Z
title: Theme words in engine doc comments
area: docs
files:
  - crates/rl-rules/src/affix.rs
  - crates/rl-bevy/src/combat.rs:18
  - crates/rl-bevy/src/items.rs:10
  - crates/rl-bevy/src/ability.rs:189
  - crates/rl-save/src/morgue.rs:39
  - crates/rl-ui/src/narrate.rs:644
---

## Problem

Engine doc comments use "cutlass", "sword", "potion", "wand" and "goblin" as worked examples.
CLAUDE.md says no fantasy, sci-fi or pirate vocabulary in types, docs or constants.
Every instance illustrates what a game might name rather than naming an engine concept, so the severity is low, but a literal reading of the rule is broken.

## Solution

Swap the examples for neutral ids in the style the registries use, keeping a worked example so the affix and ability docs still read well.
