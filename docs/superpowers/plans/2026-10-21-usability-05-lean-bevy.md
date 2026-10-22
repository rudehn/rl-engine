# Usability 05: Lean Bevy Features and Tile Glow in Tier 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reading a tile's light from the tiles file needs no renderer, and `rl-bevy` and `rl-save` compile without a window or a GPU.

**Architecture:** `light_tiles` moves from `rl-render` to `rl-rules`, which already has `ron`, `serde` and `ContentError`.
The workspace's `bevy` dependency declares no features, and each crate adds only the ones it uses; `rl-engine`, `rl-render` and `rl-ui` keep the window set.
No cargo feature is added to any crate.

**Tech Stack:** Rust 2024, Bevy 0.19, cargo.

**Spec:** `docs/reviews/engine_usability.md` sections 2.3 and 2.4; index `docs/superpowers/plans/2026-10-21-engine-usability.md`, "Deferred or changed".

## Global Constraints

- The index's "Rules for every branch" bind this branch: worktree `scripts/worktree.sh usability-05-lean-bevy`.
- No fingerprint moves.
- Tier 0 and 1 stay Bevy-free and build for wasm: `scripts/check-tiers.sh --wasm`.
- The playable wasm demos still build: `scripts/build-demos.sh step01_walking`.
- Plain dash, never an em dash.

## Review Focus

1.
  A feature dropped from the workspace that a binary needs at run time, such as `x11`, `webgl2` or `default_font`: `rl-engine` declares the full window set, so every game gets it through `rl-engine`; a desktop run of `cargo run -p tutorial --bin step01_walking` and the wasm demo build prove it.
2.
  `rl-ui`'s `WindowFocused` (`crates/rl-ui/src/controls.rs:387`): `rl-ui` keeps `bevy_window`.
3.
  A tiles file whose light names a tile that is not registered: `rl_rules` reports it by name, as `rl-render` did; the existing test moves with the function.
4.
  A look file with lights still loads in `TileAppearance::load`, which ignores the light: pinned by the existing `rl-render` test.
5.
  A later change re-adding a window feature to `rl-bevy` is caught by `scripts/check-tiers.sh`.

---

### Task 1: `light_tiles` in `rl-rules`

**Files:**
- Create: `crates/rl-rules/src/tiles.rs`
- Modify: `crates/rl-rules/src/lib.rs` (module and root re-export)
- Modify: `crates/rl-render/src/looks.rs:16-21,104-117` (remove the function, keep the `light` field ignored, fix the module docs)
- Modify: `examples/undermere/src/dungeon.rs:455`, `examples/foundry/src/decks.rs:187`
- Test: move `a_look_files_lights_land_on_the_registry_and_its_looks_are_unchanged` (`looks.rs:212`) to `tiles.rs` as `a_tiles_files_lights_land_on_the_registry`, and add `a_light_on_a_tile_never_registered_is_reported_by_name`.

**Interfaces:**
- Produces: `pub fn rl_rules::tiles::light_tiles(text: &str, tiles: &mut TileRegistry) -> Result<(), ContentError>`, same behaviour as today; it deserializes only `tile` and `light` and ignores every other field.

- [ ] **Step 1:** Write the two tests in `tiles.rs`; run `cargo test -p rl-rules tiles`.
  Expected: FAIL to compile.
- [ ] **Step 2:** Move the function; the two games call `rl_engine::rl_rules::tiles::light_tiles`.
- [ ] **Step 3:** Run `cargo test -p rl-rules tiles` and `cargo test -p rl-render looks`.
  Expected: PASS.
- [ ] **Step 4:** `scripts/gate.sh`, commit.
  CHANGELOG: "`light_tiles` moved from `rl_render::looks` to `rl_rules::tiles`, so a game reads its tiles' light without a renderer. To change: the path."

### Task 2: Each crate declares its own Bevy features

**Files:**
- Modify: `Cargo.toml:40-61` (the workspace `bevy` entry: `default-features = false`, no features)
- Modify: `crates/{rl-bevy,rl-save,rl-render,rl-ui,rl-engine}/Cargo.toml`
- Modify: `scripts/check-tiers.sh` (one more check)
- Modify: `docs/TODO.md` (the deferred window split)

- [ ] **Step 1:** Add to `scripts/check-tiers.sh`: for `rl-bevy` and `rl-save`, `cargo tree -p <crate> -e normal -i winit` and `-i wgpu` find nothing; the check fails naming the crate otherwise.
  Run it.
  Expected: FAIL for both.
- [ ] **Step 2:** Move the workspace's feature list to `rl-engine`'s `bevy` line unchanged; give `rl-render` and `rl-ui` what they need to build (start from the window set and remove until `cargo check -p <crate>` fails); give `rl-bevy` and `rl-save` the ECS set (start from `bevy_state`, `bevy_log`, `std`, `multi_threaded`, `async_executor`, `serialize`, `debug`, and add only what `cargo check -p` asks for, such as input).
- [ ] **Step 3:** Run `scripts/check-tiers.sh`, `scripts/check-tiers.sh --wasm`, `scripts/build-demos.sh step01_walking`, `scripts/check-template.sh`.
  Expected: PASS.
- [ ] **Step 4:** Add to `docs/TODO.md` section 9: "The window half of `rl-render`: `terminal.rs` draws with sprites and text, so `rl-ui`, and any terminal front end, compiles the GPU stack; split the drawing from the cell buffer into its own crate if a terminal build's compile time is measured to matter."
- [ ] **Step 5:** `scripts/gate.sh`, commit.
  CHANGELOG: "`rl-bevy` and `rl-save` no longer compile Bevy's window, renderer or text. To change: a game that depends on `rl-bevy` alone and opens a window adds the Bevy features it uses to its own `bevy` line; a game on `rl-engine` changes nothing."

### Task 3: Branch end

- [ ] `scripts/gate.sh --all`, rebase, re-gate, merge.
