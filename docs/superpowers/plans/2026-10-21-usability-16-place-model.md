# Usability 16: The Place Model in Tier 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** What a place is, and how a game builds one, is plain data in `rl-rules`, so a terminal game or a tool builds the same places as the framework without Bevy.

**Architecture:** `MapId`, `Spot`, `Lock`, `PlaceBuild`, `PlaceRules`, `Ladder`, `LadderEnd`, `Climb` and `StairLook` move from `crates/rl-bevy/src/places.rs` to a new `rl_rules::place`.
`MapId` loses its unused `Component` derive; `Ladder`'s stairs come back as plain data that `rl-bevy` turns into its `Stair` and `Transition` components.
`rl_bevy::places` re-exports the moved types, so a framework game's paths do not change.

**Tech Stack:** Rust 2024, serde.

**Spec:** `docs/reviews/engine_usability.md` section 2.2; index `docs/superpowers/plans/2026-10-21-engine-usability.md`.

## Global Constraints

- The index's "Rules for every branch" bind this branch: worktree `scripts/worktree.sh usability-16-place-model`, after branch 15 merges.
- `rl-rules` stays Bevy-free and wasm-clean: `scripts/check-tiers.sh --wasm`.
- No fingerprint moves; saves load unchanged, since the serde shapes do not change.
- A re-export is not a second declaration: `scripts/check-names.sh` still passes.
- Plain dash, never an em dash.

## Review Focus

1.
  A save holding `MapId`, `Spot`, `Lock` or `StairLook`: the same serde derives, pinned by rl-save's round-trip tests and both games' continue tests.
2.
  Stairs laid on a place's first build: the same cells, looks and destinations as before, pinned by the new tier-1 test `a_ladder_lays_an_up_stair_at_the_entry_and_a_down_stair_at_the_exit` and the existing `App` stair tests.
3.
  `MapId` used as a component anywhere: none is (`rg 'Query<.*MapId>|With<MapId>'` finds nothing); dropping the derive is safe.
4.
  A tool building a place without Bevy: pinned by a doc-test on `rl_rules::place::PlaceRules` that builds one from a chain.
5.
  `PlaceBuild::from_context` reading branch 15's `SecretCell`: unchanged behaviour, its tests move with it.

---

### Task 1: Move the place model

**Files:**
- Create: `crates/rl-rules/src/place.rs`
- Modify: `crates/rl-rules/src/lib.rs` (module, root re-exports, module map line)
- Modify: `crates/rl-bevy/src/places.rs:48-330` (the moved types go; `pub use rl_rules::place::{..}` in their place), `crates/rl-bevy/src/lib.rs`

**Interfaces:**
- Produces: `rl_rules::place::{MapId, Spot, Lock, PlaceBuild, PlaceRules, Ladder, LadderEnd, Climb, StairLook, LadderStair}` with today's fields and methods, `PlaceRules` as branch 06 left it (`build(&self, MapId, PlaceSeed)`, `tiles()`, `ladder()`).
- Produces: `pub struct LadderStair { pub at: Point, pub climb: Climb, pub look: StairLook, pub to: MapId }`; `Ladder::stairs(&self, map: MapId, entry: Point, exit: Option<Point>) -> Vec<LadderStair>` is `pub`.
- Changes: `rl-bevy`'s warp turns each `LadderStair` into `(Position, OnMap, Transition, Stair, Name)` where it spawns them today (`places.rs:625-631`).

- [ ] **Step 1:** Move the `PlaceBuild::from_context` and `Ladder` tests from `places.rs` that build no `App` into `place.rs`, and write `a_ladder_lays_an_up_stair_at_the_entry_and_a_down_stair_at_the_exit` there.
- [ ] **Step 2:** Move the types; `cargo test -p rl-rules place`.
  Expected: PASS.
- [ ] **Step 3: Write** the doc-test on `PlaceRules`: a two-room chain built into a `PlaceBuild` with only tier-1 crates in scope.
- [ ] **Step 4:** `cargo nextest run --workspace -E 'package(rl-bevy) | package(rl-save) | test(/fingerprint|continue|resume/)'`, `scripts/check-tiers.sh --wasm`, `scripts/gate.sh`.
  Expected: PASS, fingerprints unchanged.
- [ ] **Step 5:** Commit.
  CHANGELOG: "The place model is tier 1, in `rl_rules::place`: `MapId`, `PlaceBuild`, `PlaceRules`, `Ladder` and the rest, still reachable from `rl_bevy::places`. To change: nothing for a framework game; a toolkit game builds places with `rl_rules::place::PlaceRules`."

### Task 2: Branch end

- [ ] `scripts/gate.sh --all`, rebase, re-gate, merge.
