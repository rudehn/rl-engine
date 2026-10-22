# Usability 05: Tile Light in Tier 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reading a tile's light from the tiles file needs no renderer.

**Architecture:** `light_tiles` moves from `rl-render` to `rl-rules`, which already has `ron`, `serde` and `ContentError`.
The review's other half, lean Bevy features, is deferred with the split of `rl-render`'s window half (index, "Deferred"); this branch only files it in `docs/TODO.md`.

**Tech Stack:** Rust 2024, serde, ron.

**Spec:** `docs/reviews/engine_usability.md` section 2.3; index `docs/superpowers/plans/2026-10-21-engine-usability.md`, "Deferred or changed".

## Global Constraints

- The index's "Rules for every branch" bind this branch: worktree `scripts/worktree.sh usability-05-tile-light`.
- No fingerprint moves.
- `rl-rules` stays Bevy-free and builds for wasm: `scripts/check-tiers.sh --wasm`.
- Plain dash, never an em dash.

## Review Focus

1.
  A tiles file whose light names a tile that is not registered: `rl_rules` reports it by name, as `rl-render` did; the existing test moves with the function.
2.
  A look file with lights still loads in `TileAppearance::load`, which ignores the light: pinned by the existing `rl-render` test.

---

### Task 1: `light_tiles` in `rl-rules`

**Files:**
- Create: `crates/rl-rules/src/tiles.rs`
- Modify: `crates/rl-rules/src/lib.rs` (module, root re-export, module-map line)
- Modify: `crates/rl-render/src/looks.rs:16-21,104-117` (remove the function, keep the `light` field ignored, fix the module docs and the doc example at `:50`)
- Modify: `examples/undermere/src/dungeon.rs:455`, `examples/foundry/src/decks.rs:187`
- Test: move `a_look_files_lights_land_on_the_registry_and_its_looks_are_unchanged` (`looks.rs:212`) to `tiles.rs` as `a_tiles_files_lights_land_on_the_registry`, and add `a_light_on_a_tile_never_registered_is_reported_by_name`.

**Interfaces:**
- Produces: `pub fn rl_rules::tiles::light_tiles(text: &str, tiles: &mut TileRegistry) -> Result<(), ContentError>`, same behaviour as today; it deserializes only `tile` and `light` and ignores every other field.

- [ ] **Step 1:** Write the two tests in `tiles.rs`; run `cargo test -p rl-rules tiles`.
  Expected: FAIL to compile.
- [ ] **Step 2:** Move the function; the two games call `rl_engine::rl_rules::tiles::light_tiles`.
- [ ] **Step 3:** Run `cargo test -p rl-rules tiles` and `cargo test -p rl-render looks`.
  Expected: PASS.
- [ ] **Step 4:** Add to `docs/TODO.md` section 9: "Lean Bevy features and the window half of `rl-render`: `terminal.rs` draws with sprites and text, so `rl-ui`, `rl-engine` and any terminal front end compile the GPU stack. Split the cell buffer from the window drawing into its own crate, then let `rl-bevy`, `rl-save` and the cell-buffer crate declare only the Bevy features they use; until then lean features free no build and make a `-p rl-bevy` build compile a second Bevy."
- [ ] **Step 5:** `scripts/gate.sh`, commit.
  CHANGELOG: "`light_tiles` moved from `rl_render::looks` to `rl_rules::tiles`, so a game reads its tiles' light without a renderer. To change: the path."

### Task 2: Branch end

- [ ] `scripts/gate.sh --all`, rebase, re-gate, merge.
