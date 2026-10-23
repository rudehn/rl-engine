# Usability 16: The Place Model in Tier 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** What a place is, and how a game builds one, is plain data in `atglyph-mapgen`, so a terminal game or a tool builds the same places as the framework without Bevy or the rules crate.

**Architecture:** `MapId`, `Spot`, `Lock`, `PlaceBuild`, `PlaceRules`, `Ladder`, `LadderEnd`, `Climb` and `StairLook` move from `crates/atglyph-bevy/src/places.rs` to a new `atglyph_mapgen::place`, beside the `BaseContext` that `PlaceBuild::from_context` reads.
`MapId` loses its unused `Component` derive; `Lock::key` holds the raw tag id `Join::Locked` already carries, since `atglyph-mapgen` cannot name `atglyph_rules::TagId`; `Ladder`'s stairs come back as plain data that `atglyph-bevy` turns into its `Stair` and `Transition` components.
`atglyph_bevy::places` re-exports the moved types, so a framework game's paths (`atglyph::ecs::places::MapId`) do not change.
`Stair`, `Transition`, `Destination`, `Arrive` and `OnMap` are components or their data and stay in `atglyph-bevy`.
Everything the moved types name is already below `atglyph-mapgen` or in it: `PlaceSeed` (core), `Light`, `Rgb` and `TileTables` (grid), `BuildError` and `Secret` (mapgen).
The rename did none of this; it is all still to do at `86f356ed`.

**Tech Stack:** Rust 2024, serde (in `atglyph-mapgen` since branch 10).

**Spec:** `docs/reviews/engine_usability.md` section 2.2; index `docs/superpowers/plans/2026-10-21-engine-usability.md`.

## Global Constraints

- The index's "Rules for every branch" bind this branch: worktree `scripts/worktree.sh usability-16-place-model`, after branch 15 merges.
- `atglyph-mapgen` stays Bevy-free, wasm-clean and free of `atglyph-rules`: `scripts/check-tiers.sh --wasm`, and `cargo tree -p atglyph-mapgen -e normal` names neither.
- `atglyph-mapgen` is published as usable on its own, so its dependency lines, its readme and `scripts/check-template.sh`'s guard of them do not change, and `scripts/check-publish.sh` still packages it.
- The change is breaking (`Lock::key`'s type, `MapId` no longer a `Component`); the CHANGELOG line says so.
- No fingerprint moves; saves load unchanged, since the serde shapes do not change.
- A re-export is not a second declaration: `scripts/check-names.sh` still passes.
- Plain dash, never an em dash.

## Review Focus

1.
  A save holding `MapId`, `Spot`, `Lock` or `StairLook`: the same serde derives, pinned by atglyph-save's round-trip tests and both games' continue tests.
  `Lock::key` as `Option<u32>` reads and writes as `Option<TagId>` did, since `Id` serializes as its raw `u32` (`atglyph-core/src/id.rs:71-80`).
2.
  Stairs laid on a place's first build: the same cells, looks and destinations as before, pinned by the new tier-1 test `a_ladder_lays_an_up_stair_at_the_entry_and_a_down_stair_at_the_exit` and the existing `App` stair tests.
3.
  `MapId` used as a component anywhere: none is (`rg 'Query<.*MapId>|With<MapId>'` finds nothing); dropping the derive is safe.
4.
  A tool building a place without Bevy: pinned by a doc-test on `atglyph_mapgen::place::PlaceRules` that builds one from a chain with only `atglyph_core`, `atglyph_grid` and `atglyph_mapgen` in scope.
5.
  `PlaceBuild::from_context` reading branch 15's `SecretCell`: unchanged behaviour, its tests move with it.

---

### Task 1: Move the place model

**Files:**
- Create: `crates/atglyph-mapgen/src/place.rs`
- Modify: `crates/atglyph-mapgen/src/lib.rs` (module, root re-exports, prelude, and a sentence on the front page naming `place`)
- Modify: `crates/atglyph-bevy/src/places.rs:55-383` (the moved types go; `pub use atglyph_mapgen::place::{..}` in their place; `PlaceBuild::from_context` stops wrapping `Join::Locked`'s tag at `:202`), `crates/atglyph-bevy/src/lib.rs`
- Modify: every reader of `Lock::key`, which wraps it in `TagId::from_raw`: `crates/atglyph-bevy/src/doors.rs:110`, and the `Lock` literals at `doors.rs:393`, `world.rs:550` and `examples/undermere/src/vaults/tests.rs:503,522`

**Interfaces:**
- Produces: `atglyph_mapgen::place::{MapId, Spot, Lock, PlaceBuild, PlaceRules, Ladder, LadderEnd, Climb, StairLook, LadderStair}` with today's fields and methods except `Lock { at: Point, key: Option<u32> }`, `PlaceRules` as branch 06 left it (`build(&self, MapId, PlaceSeed)`, `tiles()`, `ladder()`).
- Produces: `pub struct LadderStair { pub at: Point, pub climb: Climb, pub look: StairLook, pub to: MapId }`; `Ladder::stairs(&self, map: MapId, entry: Point, exit: Option<Point>) -> Vec<LadderStair>` is `pub`.
- Changes: `Ladder::stairs` is `pub(crate)` today and returns `(Point, Stair, Transition)`; `above` and `below` stay crate-private, and `floor_of` stays test-only.
- Changes: `atglyph-bevy`'s warp turns each `LadderStair` into `(Position, OnMap, Transition, Stair, Name)` where it spawns them today (`places.rs:694-701`).

- [ ] **Step 1:** Move the `PlaceBuild::from_context` and `Ladder` tests from `places.rs` that build no `App` into `place.rs`, and write `a_ladder_lays_an_up_stair_at_the_entry_and_a_down_stair_at_the_exit` there.
- [ ] **Step 2:** Move the types; `cargo test -p atglyph-mapgen place`.
  Expected: PASS.
- [ ] **Step 3: Write** the doc-test on `PlaceRules`: a two-room chain built into a `PlaceBuild` with only `atglyph_core`, `atglyph_grid` and `atglyph_mapgen` in scope.
- [ ] **Step 4:** `scripts/check-tiers.sh --wasm` and `scripts/gate.sh`, which plays atglyph-save's round trips, both games' continue tests and the fingerprints.
  Expected: PASS, fingerprints unchanged.
- [ ] **Step 5:** Commit.
  CHANGELOG: "The place model is tier 1, in `atglyph::mapgen::place`: `MapId`, `PlaceBuild`, `PlaceRules`, `Ladder` and the rest, still reachable from `atglyph::ecs::places`; `Lock::key` is the raw tag id and `MapId` is no longer a `Component`. To change: breaking: a reader of `Lock::key` wraps it in `atglyph::rules::TagId::from_raw`; a toolkit game builds places with `atglyph_mapgen::place::PlaceRules`."

### Task 2: Branch end

- [ ] `scripts/gate.sh --all`, rebase, re-gate, merge.
