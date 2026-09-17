---
created: 2026-09-17T04:17:22.126Z
title: Sanitize save slot names
area: rl-save
files:
  - crates/rl-save/src/backend.rs
---

## Problem

The file backend joins a save-slot name straight into a filesystem path beside the executable.
Every current caller passes an engine-generated or compile-time name, so it is not exploitable today, but a game that lets a player type a save name could write outside the save directory.

## Solution

Validate slot names in the backend (a restricted character set, no separators or `..`) and return an error, with a test.
