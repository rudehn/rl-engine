# Usability 04: Play Begins on Its Own Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A run's start no longer sets `EngineState::Playing`; the engine begins play once the start has built a `WorldMap` and nothing holds it.

**Architecture:** `begin_run` is the one door every run comes through (startup, a title's New Game, a restart).
After it runs `NewRun`, it sets `NextState(Playing)` when a `WorldMap` exists and no `HoldStart` stands, in the same frame the games set it today, so no timing moves.

**Tech Stack:** Rust 2024, Bevy 0.19.

**Spec:** `docs/reviews/engine_usability.md` section 1.1 (the third of its fixes); index `docs/superpowers/plans/2026-10-21-engine-usability.md`.

## Global Constraints

- The index's "Rules for every branch" bind this branch: worktree `scripts/worktree.sh usability-04-play-begins`, after branch 01 merges (both edit the tutorial binaries).
- No fingerprint moves: the state is set in the same frame as before.
- Setting `EngineState::Playing` by hand stays legal; engine tests that do so are left alone.
- Plain dash, never an em dash.

## Review Focus

1.
  A game with a title: `HoldStart` present at startup means no run and no play until `begin_run`; pinned by `a_held_start_keeps_play_from_beginning_until_the_title_begins_the_run`.
2.
  A restart: `begin_pending_run` reaches `begin_run`, so play begins again; pinned by `a_restart_begins_play_again_on_its_own`.
3.
  A continued run whose save cannot be read: Undermere's `resume` falls back to `begin_afresh`, still inside `NewRun`, so play still begins; Undermere's continue tests cover it.
4.
  A start that builds no world: play stays idle, and the existing 120-frame warning still fires for a `WorldMap` inserted outside `NewRun`.
5.
  A start that sets `Playing` itself and also builds a world: setting it twice is harmless; pinned by the unchanged engine tests.

---

### Task 1: `begin_run` begins play

**Files:**
- Modify: `crates/rl-bevy/src/title.rs:44-47` (`begin_run`)
- Modify: `crates/rl-bevy/src/plugin.rs:844` (the requirements panic's wording), `:848-862` (the warning's wording)
- Test: `crates/rl-bevy/src/title.rs` test module

**Interfaces:**
- Changes: `pub fn begin_run(world: &mut World)` sets `NextState<EngineState>` to `Playing` after `run_schedule(NewRun)` when `world.contains_resource::<WorldMap>() && !world.contains_resource::<HoldStart>()`.

- [ ] **Step 1: Write the failing tests:**
  - `a_run_whose_start_builds_a_world_begins_play_on_its_own`: a `NewRun` system inserts `WorldMap::new(TileRegistry::standard().tables())` and nothing else; after two updates the state is `Playing`.
  - `a_start_that_builds_no_world_leaves_play_idle`: an empty `NewRun`; after two updates the state is `Idle`.
  - `a_held_start_keeps_play_from_beginning_until_the_title_begins_the_run`: `HoldStart` inserted; two updates leave `Idle`; `begin_run` through `World::run_system_once` or a queued command, two updates, `Playing`.
  - `a_start_that_holds_the_start_again_keeps_play_idle`: `NewRun` inserts a `WorldMap` and `HoldStart`; `Idle`.
  - `a_restart_begins_play_again_on_its_own`: play begun, a `Restart` written, three updates, `Playing` again.
- [ ] **Step 2:** Run `cargo test -p rl-bevy title`.
  Expected: the first, third and fifth FAIL.
- [ ] **Step 3:** Implement.
  The requirements panic reads "Insert each before play begins."; the warning reads "a WorldMap was inserted outside NewRun and EngineState is still Idle after 120 frames; play begins on its own only after a run's start".
- [ ] **Step 4:** Run `cargo nextest run -p rl-bevy`.
  Expected: PASS.
- [ ] **Step 5:** `scripts/gate.sh`, commit.

### Task 2: The starts stop setting the state

**Files:**
- Modify: the start system of every tutorial binary (`examples/tutorial/src/bin/step0*.rs`), `templates/starter/src/main.rs:196`, `examples/foundry/src/run.rs:150,225`, `examples/undermere/src/run.rs:426,474`

- [ ] **Step 1:** Delete each `set(EngineState::Playing)` and the `NextState` parameter or `Begin` field it leaves unused.
- [ ] **Step 2:** Run `cargo nextest run --workspace -E 'package(tutorial) | package(undermere) | package(foundry) | test(/fingerprint/)'` and `scripts/check-template.sh`.
  Expected: PASS, fingerprints unchanged.
- [ ] **Step 3:** `rg -n 'set\(EngineState::Playing\)' examples templates` finds only tests.
- [ ] **Step 4:** `scripts/gate.sh`, commit.
  CHANGELOG: "Play begins on its own once a run's start, in `NewRun`, has built a `WorldMap` and no `HoldStart` holds it. To change: nothing; a start may delete its `next.set(EngineState::Playing)` and the `NextState` parameter, and setting it by hand still works."

### Task 3: Branch end

- [ ] `scripts/gate.sh --all`, rebase, re-gate, merge.
