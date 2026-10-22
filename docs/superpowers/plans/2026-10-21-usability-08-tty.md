# Usability 08: A Framework Game Plays in a Terminal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A game built on the framework runs in a text terminal, locally or over SSH, with `TtyRoguelikePlugins` in place of `RoguelikePlugins`, and each SSH user saves apart.

**Architecture:** A new tier-2 crate, `rl-tty`, owns the terminal: it holds the same `rl_render::Terminal` cell buffer every panel already draws into, writes the cells that changed with crossterm, and turns key presses into `ButtonInput<KeyCode>`.
`rl-engine` gains `TtyRoguelikePlugins`, `MinimalPlugins` and the engine's plugins with no window, desktop only.
The GPU crates are still compiled (index, "Deferred"); they are never added.

**Tech Stack:** Rust 2024, Bevy 0.19, crossterm 0.29, unicode-width.

**Spec:** `docs/reviews/engine_usability.md` section 2.5; index `docs/superpowers/plans/2026-10-21-engine-usability.md`.

## Global Constraints

- The index's "Rules for every branch" bind this branch: worktree `scripts/worktree.sh usability-08-tty`, after branch 06 merges.
- `rl-tty` declares `tier = 2` and `#![deny(missing_docs)]`; no theme words.
- `rl-engine` depends on `rl-tty` only off wasm (`[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`), so the wasm demos still build.
- Pure logic is a plain function tested without an `App`; the plugin's `App` test proves the calls.
- No fingerprint moves.
- Plain dash, never an em dash.

## Review Focus

1.
  A panic or an early exit must hand back a usable shell: the raw-mode guard restores on drop and a panic hook restores before the message prints; pinned by `the_guard_restores_the_terminal_when_dropped` against a writer.
2.
  Terminals send no key releases: a key pressed in one frame is released in the next, so `just_pressed` fires once per press and autorepeat walks; pinned by `a_key_pressed_is_released_the_frame_after`.
3.
  A glyph two columns wide shifts every cell after it: drawn as `?` with one warning; pinned by `a_glyph_two_columns_wide_is_drawn_as_a_question_mark`.
4.
  A resize: the next frame redraws every cell; pinned by `after_a_resize_every_cell_is_written_again`.
5.
  A user name with a separator or `..` over SSH must not reach outside the save directory: refused like a slot, as an error rather than a panic, since a user name is input and not the game's constant; pinned by `a_user_name_that_is_not_a_plain_name_is_refused`.

---

### Task 1: Each user saves apart

**Files:**
- Modify: `crates/rl-save/src/backend.rs:249` (beside `Saves::platform_default`)

**Interfaces:**
- Produces: `#[cfg(not(target_arch = "wasm32"))] pub fn Saves::per_user(name: &str, user: &str) -> Result<Saves, SaveError>`, saving in `saves/<name>/users/<user>` beside the executable; `SaveError::Slot` when either is not a plain name (`check_slot`).
- A game inserts what it returns before adding `SavePlugin`, which keeps a `Saves` it finds (`run.rs`, `SavePlugin::build`); no `SavePlugin` method is added.

- [ ] **Step 1: Write the failing tests** `each_user_saves_in_a_directory_of_their_own` and `a_user_name_that_is_not_a_plain_name_is_refused`, with a temporary directory as `backend.rs:428` does.
- [ ] **Step 2:** `cargo test -p rl-save user`.
  Expected: FAIL to compile.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run them.
  Expected: PASS.
  `scripts/gate.sh`, commit.
  CHANGELOG: "`Saves::per_user(name, user)` keeps each user's saves apart, for a game many people play over SSH: insert it before `SavePlugin`. To change: nothing."

### Task 2: The terminal's pure parts

**Files:**
- Create: `crates/rl-tty/Cargo.toml`, `crates/rl-tty/src/lib.rs`, `crates/rl-tty/src/keys.rs`, `crates/rl-tty/src/paint.rs`
- Modify: `Cargo.toml` (workspace member, `crossterm` and `unicode-width` workspace dependencies; `examples/terminal` moves to the workspace `crossterm`)
- Modify: `crates/rl-render/src/terminal.rs` (a `pub fn cells(&self) -> &[Cell]` if no public read of every cell exists)

**Interfaces:**
- Produces: `pub fn key_code(event: &crossterm::event::KeyEvent) -> Option<KeyCode>`: letters, digits, punctuation the engine binds, arrows, the numpad as digits, Enter, Escape, Tab, Backspace; `None` for the rest.
- Produces: `pub fn changes(last: Option<&[Cell]>, now: &Terminal) -> Vec<(u16, u16, Cell)>`: every cell when `last` is `None`, else those that differ.
- Produces: `pub fn shown(c: char) -> char`: `c` when it is one column wide, else `?`.

- [ ] **Step 1: Write the failing tests:** `vi_letters_arrows_and_the_numpad_become_their_key_codes`, `a_key_the_engine_never_binds_becomes_nothing`, `only_the_cells_that_changed_are_written`, `with_nothing_written_before_every_cell_is`, `a_glyph_two_columns_wide_is_drawn_as_a_question_mark`.
- [ ] **Step 2:** `cargo test -p rl-tty`.
  Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test -p rl-tty`, `scripts/check-tiers.sh`.
  Expected: PASS.
  `scripts/gate.sh`, commit.

### Task 3: `TtyPlugin`

**Files:**
- Create: `crates/rl-tty/src/plugin.rs`
- Test: same file

**Interfaces:**
- Produces: `pub struct TtyPlugin`, `TtyPlugin::new(cols: i32, rows: i32)` for stdout in raw mode, and `TtyPlugin::to_writer(cols, rows, out: impl Write + Send + Sync + 'static)` for tests, with no raw mode and no polling.
- `build` inserts `Terminal::new(cols, rows, Vec2::ONE)`, `rl_render::pointer::Pointer::default()`, and adds: in `Startup`, raw mode, the alternate screen and a hidden cursor behind a guard resource that restores on drop; in `PreUpdate` after Bevy's input systems, release last frame's keys, then poll crossterm with a zero timeout and press each `key_code`; in `PostUpdate`, write `changes` with `MoveTo`, colours and `shown`, then flush.

- [ ] **Step 1: Write the failing tests** with `to_writer` over a shared `Vec<u8>`: `a_frame_writes_only_the_cells_that_changed_since_the_last`, `after_a_resize_every_cell_is_written_again` (send the resize through the plugin's event seam), `a_key_pressed_is_released_the_frame_after`, `the_guard_restores_the_terminal_when_dropped`.
- [ ] **Step 2:** Run them.
  Expected: FAIL.
- [ ] **Step 3:** Implement; keep each system under the clippy limits by splitting gather, pure call, and write.
- [ ] **Step 4:** Run them.
  Expected: PASS.
  `scripts/gate.sh`, commit.

### Task 4: `TtyRoguelikePlugins`, and the template plays in a terminal

**Files:**
- Modify: `crates/rl-engine/Cargo.toml`, `crates/rl-engine/src/lib.rs:77-175` (beside `RoguelikePlugins`), its prelude
- Modify: `templates/starter/src/main.rs` (`main` picks the front end), `README.md:83`

**Interfaces:**
- Produces: `#[cfg(not(target_arch = "wasm32"))] pub struct TtyRoguelikePlugins` with `new(cols, rows)`, `map(Rect)` and `across(i32)` as `RoguelikePlugins` has; its group is `MinimalPlugins` with `ScheduleRunnerPlugin::run_loop` at 30 frames a second, `StatesPlugin`, `InputPlugin`, `TtyPlugin`, `CorePlugin`, `FovPlugin`, `MapViewPlugin`, `ParticlesPlugin`, `UiPlugin`, `ReplayPlugin::from_env()`; no `LogPlugin`, whose lines would land on the game.

- [ ] **Step 1: Write the failing test** `the_terminal_group_builds_an_app_that_plays_a_turn` in `rl-engine`, with `TtyRoguelikePlugins` and `TtyPlugin::to_writer` swapped in through `PluginGroupBuilder::set`, a tiny place and a player; one wait passes a turn.
- [ ] **Step 2:** Run it.
  Expected: FAIL.
- [ ] **Step 3:** Implement.
  The template's `main` adds `TtyRoguelikePlugins` when `--tty` is among the arguments and `RoguelikePlugins` otherwise.
- [ ] **Step 4:** Run the test; run `cargo run` on the generated template with `--tty` by hand and walk, quit, and see the shell restored; `scripts/check-template.sh`; `scripts/build-demos.sh step01_walking`.
  Expected: PASS.
- [ ] **Step 5:** `README.md:83` says the terminal front end exists and names `TtyRoguelikePlugins`.
- [ ] **Step 6:** `scripts/gate.sh`, commit.
  CHANGELOG: "`TtyRoguelikePlugins` plays a game in a text terminal, locally or over SSH, with the same panels; the template runs that way with `--tty`. To change: nothing."

### Task 5: Branch end

- [ ] `scripts/gate.sh --all`, rebase, re-gate, merge.
