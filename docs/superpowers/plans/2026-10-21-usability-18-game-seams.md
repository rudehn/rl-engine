# Usability 18: The Seams a Game Stands On Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Undermere and Foundry stand only on the engine's public, stable API: continuing a run is the engine's, a game reads its launch arguments through one engine type, and `rl_engine::testing` is a stated, stable surface.

**Architecture:** Three seams, one commit each.
Continuing: `begin_run` runs a new `PrepareRun` schedule on every run, then `NewRun` on a fresh one, or, when `ContinueRun` stands, a `ContinueSaved` schedule that `SavePlugin` fills with the load, reseed, prepare, restore and fall-back the games write by hand today, ending in a `RunResumed` schedule for the game's own steps.
Launching: `LaunchArgs` takes the recording's or the command line's `--seed` out of the arguments and hands the game the rest, so no game reads `replay::seed` or `replay::args`.
Testing: `rl_engine::testing` re-exports an explicit list instead of two globs; `rl_bevy::testing` and `rl_ui::testing` stay public for the engine crates' shared fixtures but are `#[doc(hidden)]` and outside semver.

**Tech Stack:** Rust 2024, Bevy 0.19.

**Spec:** `docs/TODO.md` item 30; the index's public-API rule; index `docs/superpowers/plans/2026-10-21-engine-usability.md`.

## Global Constraints

- The index's "Rules for every branch" bind this branch: worktree `scripts/worktree.sh usability-18-game-seams`, after branch 15 merges.
- No fingerprint moves: a fresh run builds the same world in the same order; a fingerprint that moves is a bug here.
- Saves load unchanged: the save's shape does not change, only who calls `load_run` and `restore`.
- No game names `clear_run`, `load_run`, `replay::seed`, `replay::args`, `rl_bevy::testing` or `rl_ui::testing` after this branch.
- Plain dash, never an em dash.

## Review Focus

1.
  A continued run must be prepared on the saved seed, not the launch's: `ContinueSaved` inserts `Seed(saved.engine.seed)` before `PrepareRun`; pinned by `a_continued_run_is_prepared_on_its_saved_seed_then_restored_then_resumed`.
2.
  A restore needs a `WorldMap` (`rl-save/src/engine.rs:198` takes it mutably): the `WorldMap` that branch 06 builds from `PlaceRules::tiles` is built right after `PrepareRun`, before `NewRun` or the restore.
3.
  A save that restores in part leaves neither run: torn down with `clear_run` and begun afresh, as both games do today (`undermere/src/run.rs:460-469`, `foundry/src/run.rs:186-195`); pinned by `a_save_that_restores_without_one_player_is_torn_down_and_begun_afresh`.
4.
  `ContinueRun` in a game with no `SavePlugin`: `ContinueSaved` is empty, the marker is still there after it, and `begin_run` warns once and begins afresh; pinned by `a_continue_with_nothing_to_continue_it_begins_afresh`.
5.
  A restart after a continued run begins fresh: `ContinueSaved` removes `ContinueRun` first; pinned by `a_restart_after_a_continued_run_begins_fresh`.
6.
  A replay must run on its recorded seed whatever the command line says: `LaunchArgs` prefers the recording's; pinned by `a_recording_s_seed_wins_over_the_command_line`.
7.
  A name a game's tests use dropping out of `rl_engine::testing`: every game test still compiles against the explicit list, and `scripts/check-template.sh` builds the template outside the workspace.

---

### Task 1: Continuing a run is the engine's

**Files:**
- Modify: `crates/rl-bevy/src/plugin.rs:99` (beside `NewRun`), `:254-256` (`init_schedule`), `crates/rl-bevy/src/title.rs:44-47` (`begin_run`), `crates/rl-bevy/src/lib.rs` (re-exports, prelude)
- Modify: `crates/rl-save/src/run.rs:1058` (`SavePlugin::build`), beside `load_run` at `:1214`
- Modify: `examples/undermere/src/run.rs:62` (`prepare`), `:393-427` (`start`), `:439-481` (`resume`, `begin_afresh`), `examples/undermere/src/plugin.rs:223-227`
- Modify: `examples/foundry/src/run.rs:39` (`prepare`), `:134-151` (`start`), `:162-222` (`resume`), `examples/foundry/src/plugin.rs:44-47,195`
- Test: `crates/rl-bevy/src/title.rs` and `crates/rl-save/src/run.rs` test modules

**Interfaces:**
- Produces in `rl_bevy::plugin`: `#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)] pub struct PrepareRun;` (what every run stands on, fresh or continued, run once its seed is set), `pub struct RunResumed;` (a game's steps after a continued run is restored), and `pub struct ContinueSaved;` (filled by `rl-save`; a game never adds to it); `CorePlugin` inits all three.
- Produces: `pub fn begin_afresh(world: &mut World)` in `rl_bevy::title`: runs `PrepareRun`, builds the `WorldMap` from `PlaceRulesRes` when none stands (branch 06's step, moved here), then runs `NewRun`.
- Changes: `begin_run(world)` removes `HoldStart`; with `ContinueRun` present it runs `ContinueSaved`, and if `ContinueRun` is still present afterwards it removes it, warns that no plugin continues a run, and calls `begin_afresh`; without it, `begin_afresh`; then branch 04's play check.
- Produces in `rl_save::run`: `pub(crate) fn continue_saved_run(world: &mut World)`, added by `SavePlugin` to `ContinueSaved`: removes `ContinueRun`; `load_run`; on `Ok(None)` or `Err`, logs and calls `begin_afresh`; inserts `Seed(saved.engine.seed)`; runs `PrepareRun`; builds the `WorldMap` as `begin_afresh` does; `saved.restore(world)`; unless that is `Ok` and exactly one `Player` stands, logs, calls `clear_run` and `begin_afresh`; otherwise runs `RunResumed`.
- Changes in each game: `prepare` becomes a `PrepareRun` system (`fn prepare(mut commands: Commands, seed: Res<Seed>, registries: Res<Registries>)`); `start` loses its `ContinueRun` check and its `prepare` call; `resume` and `begin_afresh` go; what `resume` did after the restore becomes `RunResumed` systems.
- Undermere's `RunResumed` systems: the "Continuing on floor N" line through branch 09's `Log`, which stamps the restored turn, and `corpses::dress_the_dead` (today `.after(resume)` in `NewRun`).
- Foundry's `RunResumed` systems: the upgrades given again and the waiting pick offered again (`run.rs:196-216`), then the "Continuing on deck N" line; `mission::start` moves to `PrepareRun`, since it ran before the restore on both paths.

- [ ] **Step 1: Write the failing tests** in `title.rs`: `a_fresh_run_prepares_before_it_starts` (a `PrepareRun` and a `NewRun` system push to a `Vec`; the order is prepare, start), `a_continue_with_nothing_to_continue_it_begins_afresh` (`ContinueRun` inserted, no `SavePlugin`: the fresh systems ran and `ContinueRun` is gone).
- [ ] **Step 2: Write the failing tests** in `rl-save/src/run.rs`, each a small app with `SavePlugin` and a memory backend: `a_continued_run_is_prepared_on_its_saved_seed_then_restored_then_resumed` (a saved run on seed 9, launched on seed 1: `PrepareRun` sees seed 9, `NewRun` never runs, `RunResumed` runs after the player is back), `a_continue_with_no_readable_save_begins_a_fresh_run`, `a_save_that_restores_without_one_player_is_torn_down_and_begun_afresh`, `a_restart_after_a_continued_run_begins_fresh`.
- [ ] **Step 3:** `cargo test -p rl-bevy title` and `cargo test -p rl-save continue`.
  Expected: FAIL to compile.
- [ ] **Step 4:** Implement the engine half; then move both games as the Interfaces say.
- [ ] **Step 5:** `rg -n 'ContinueRun|clear_run|load_run|run_system_once\(start' examples` finds only tests.
- [ ] **Step 6:** `scripts/gate.sh`, which plays both games' continue tests (`testing::continued`) and the fingerprints.
  Expected: PASS, fingerprints unchanged.
- [ ] **Step 7:** Commit.
  CHANGELOG: "Continuing a run is the engine's: `SavePlugin` loads the save, sets its seed, runs the new `PrepareRun` schedule, restores, and falls back to a fresh run, and a game's own steps after a restore go in `RunResumed`. To change: move what every run stands on out of the `NewRun` start into a `PrepareRun` system; delete the `ContinueRun` check in the start and the hand-written `resume`; put its after-restore steps in `RunResumed`."

### Task 2: A game reads its launch through `LaunchArgs`

**Files:**
- Modify: `crates/rl-bevy/src/seed.rs:43-73` (`Seed::from_args`, `seed_argument`), `crates/rl-bevy/src/replay.rs:113-128` (`args`, `seed`), `crates/rl-bevy/src/lib.rs`
- Modify: `examples/undermere/src/main.rs:352-363`, `examples/foundry/src/main.rs:203-222`
- Test: `crates/rl-bevy/src/seed.rs` test module

**Interfaces:**
- Produces: `#[derive(Debug, Clone, PartialEq, Eq)] pub struct LaunchArgs { pub seed: Option<RunSeed>, pub rest: Vec<String> }` in `rl_bevy::seed`.
- Produces: `#[cfg(not(target_arch = "wasm32"))] pub fn LaunchArgs::from_env() -> LaunchArgs`: the recording's arguments and seed when replaying, the command line's otherwise; `--seed N` is taken out of `rest`; panics naming the argument when `N` is not a whole number, as `Seed::from_args` does.
- Produces: `pub fn LaunchArgs::parse(args: Vec<String>, recorded: Option<RunSeed>) -> Result<LaunchArgs, String>`, the pure half, tested without an `App`; the recording's seed wins over `--seed`.
- Produces: `pub fn LaunchArgs::seed_or_fresh(&self) -> Seed`.
- Changes: `Seed::from_args()` is `LaunchArgs::from_env().seed_or_fresh()`; `seed_argument` goes into `parse`.
- Changes: `replay::seed` becomes `pub(crate)`; `replay::args` stays `pub` for `rl-ui`'s recorder (`rl-ui/src/replay.rs:74`), its docs saying a game reads `LaunchArgs`.
- Changes in each game's `main`: `let launch = LaunchArgs::from_env();` matches on `launch.rest`; the print modes (`--floor N`, `--deck N`) take `launch.seed` and print the usage when it is `None`, as a missing `--seed` does today; the run inserts `launch.seed_or_fresh()`.

- [ ] **Step 1: Write the failing tests:** `a_seed_on_the_command_line_is_taken_out_of_the_game_s_arguments`, `a_recording_s_seed_wins_over_the_command_line`, `a_seed_that_is_not_a_number_is_refused_naming_it`, `with_no_seed_the_arguments_are_left_as_they_were`; `the_seed_argument_is_read_or_refused_by_name` (`seed.rs:173`) folds into them.
- [ ] **Step 2:** `cargo test -p rl-bevy seed`.
  Expected: FAIL to compile.
- [ ] **Step 3:** Implement; move both games' `main`.
- [ ] **Step 4:** `rg -n 'replay::(seed|args)' examples templates` finds nothing.
  Run `cargo run -p undermere -- --floor 3 --seed 7` and `cargo run -p foundry -- --deck 2 --seed 7` by hand; each prints its floor as before.
- [ ] **Step 5:** `scripts/gate.sh`, commit.
  CHANGELOG: "`LaunchArgs::from_env()` gives a game its seed and the rest of its arguments, a replay's when replaying, so a game's own flags replay with it. To change: a `main` that read `replay::seed()` and `replay::args()` reads `LaunchArgs::from_env()`; `replay::seed` is no longer public."

### Task 3: The stable testing surface

**Files:**
- Modify: `crates/rl-engine/src/testing.rs:1-17` (the globs become an explicit list; the module docs state what is stable)
- Modify: `crates/rl-bevy/src/lib.rs:54`, `crates/rl-ui/src/lib.rs:142` (`#[doc(hidden)] pub mod testing`), and the module docs of `crates/rl-bevy/src/testing.rs:1-24` and `crates/rl-ui/src/testing.rs`
- Modify: `crates/rl-mapgen/src/testing.rs` docs (stable, for toolkit games)
- Modify: every game import of `rl_bevy::testing` (`rg -n 'rl_bevy::testing|rl_ui::testing' examples templates`: the template, Undermere, Foundry and tutorial steps 4, 5 and 6)

**Interfaces:**
- `rl_engine::testing` re-exports, by name and nothing else: `windowless`, `headless`, `continued`; `KeyScriptPlugin`, `KeyScript`, `press`, `settle`, `pass_turns`, `let_time_pass`; `player`, `here`, `at`, `name_of`, `pack`; `stand_on`, `go_through`, `take_up`, `put_down`, `give`, `equip`, `unequip`, `kill`, `lay`, `lay_tile`, `wall_off`; `row`, `written`, `said`; `QUICK`, `seeds`, `first_seeds`, `par_seeds`; and the module `ambiguity` whole.
- Engine fixtures stay out of it: `TEST_SEED`, `ground`, `ground_with`, `TwoSides` and `two_sides` (branch 15's name), and `headless_app` (in `rl_bevy::testing` since branch 13), which `rl-ui`, `rl-render` and `rl-save` share and no game uses.
- `rl_engine::testing`'s module docs say: these names are the engine's API and change only with a version, as any other; `rl_bevy::testing` and `rl_ui::testing` are the engine crates' shared fixtures, hidden and outside semver.
- `rl_mapgen::testing` stays public and stable, its docs saying so, since a toolkit game's tests use it without `rl-engine`.

- [ ] **Step 1:** Make the list; `#[doc(hidden)]` the two crate modules; write the docs.
- [ ] **Step 2:** Move every game import to `rl_engine::testing`.
  `rg -n 'rl_bevy::testing|rl_ui::testing' examples templates` finds nothing.
- [ ] **Step 3:** `cargo doc -p rl-engine --no-deps` with warnings denied shows every listed name under `rl_engine::testing`; `scripts/check-template.sh`; `scripts/gate.sh`.
  Expected: PASS.
- [ ] **Step 4:** Commit.
  CHANGELOG: "`rl_engine::testing` is the stable testing API, an explicit list of names; `rl_bevy::testing` and `rl_ui::testing` are the engine's own fixtures, hidden and outside semver. To change: a test that imported from `rl_bevy::testing` or `rl_ui::testing` imports the same name from `rl_engine::testing`; `TEST_SEED`, `ground`, `two_sides` and `headless_app` are no longer offered to games, and `windowless` stands an app up."

### Task 4: Branch end

- [ ] Remove TODO 30 from `docs/TODO.md`; a line in `docs/PLAN.md`'s progress log.
- [ ] `scripts/gate.sh --all`, rebase, re-gate, merge.
