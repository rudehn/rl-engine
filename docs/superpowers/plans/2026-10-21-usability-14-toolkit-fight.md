# Usability 14: The Toolkit Example Has a Fight Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `examples/terminal` shows the rules the README sells: a rat with a turn order, a brain and blows, each a plain tested function; and the README's toolkit sample compiles from the tier-1 crates alone.

**Architecture:** The example stays a loop the game owns: `rl_core::TurnQueue` deals turns, `rl_rules::ai::Brain` with `MeleeAdjacent` and `Hunt` decides for each rat from a `Snapshot`, and `rl_rules::damage::apply_hit` lands blows both ways.
No Bevy.

**Tech Stack:** Rust 2024, crossterm, `rl-core`, `rl-grid`, `rl-mapgen`, `rl-rules`, rand.

**Spec:** `docs/reviews/engine_usability.md` section 2.6 and "The toolkit hello world"; index `docs/superpowers/plans/2026-10-21-engine-usability.md`.

## Global Constraints

- The index's "Rules for every branch" bind this branch: worktree `scripts/worktree.sh usability-14-toolkit-fight`, after branch 11 merges (it removed `BuildContext` from the same files).
- `examples/terminal` depends on no tier-2 crate and no Bevy; `cargo tree -p terminal -i bevy` finds nothing.
- Randomness from `RunSeed::rng(DOMAIN, index)` with a named domain constant; no generator from a constant.
- Every test is a plain `#[test]` with no `App`.
- Plain dash, never an em dash.

## Review Focus

1.
  A rat out of sight must not hunt: its snapshot holds no enemy unless the player is in its view; pinned by `a_rat_that_cannot_see_you_does_not_close_in`.
2.
  The player at zero health: the loop says so and stops taking steps; pinned by `a_walker_struck_to_nothing_takes_no_more_steps`.
3.
  Two rats wanting one cell: `can_step` refuses a cell another rat stands on; pinned by `two_rats_never_stand_on_one_cell`.
4.
  A dead rat's place in the queue: `pop_due`'s `is_alive` drops it; pinned by `a_rat_struck_to_nothing_is_gone_and_never_acts_again`.
5.
  The README sample run as the `rl-engine` doc-test must still compile with crate paths: `cargo test -p rl-engine --doc`.

---

### Task 1: Rats that hunt and bite

**Files:**
- Modify: `examples/terminal/Cargo.toml` (`rl-rules`, `rand`)
- Modify: `examples/terminal/src/lib.rs` (the model, the rules calls, the tests), `examples/terminal/src/main.rs` (the loop deals turns, draws `r` and health)

**Interfaces:**
- Produces: `pub enum Who { You, Rat(usize) }` (`Copy`, `Eq`, `Ord`); `pub struct Rat { pub at: Point, pub health: Vitals }`.
- Changes: `Walk` gains `health: Vitals`, `rats: Vec<Rat>`, `queue: TurnQueue<Who>` and `log: Vec<String>`; `generate(seed)` puts three rats on floor cells drawn from `RunSeed(seed).rng(RATS, 0)`, with `const RATS: SeedDomain = SeedDomain::new(b"terminal.rats")`.
- Changes: `pub fn step(walk: &mut Walk, dir: Direction) -> bool`: into a rat is a blow of 2 with `apply_hit`; otherwise a move as now.
- Produces: `pub fn rats_act(walk: &mut Walk)`: pops due rats until `Who::You`; each builds `Snapshot::alone(me)` with the player as its one enemy when in its view (`fov` from its cell), decides with `Brain::new().then(MeleeAdjacent).then(Hunt)` and `NoFields`, and applies `Attack` as a blow of 1 on the player or `Step` as a move.

- [ ] **Step 1: Write the failing tests:** `a_step_into_a_rat_strikes_it_and_does_not_move`, `a_rat_beside_you_bites_you_on_its_turn`, `a_rat_that_sees_you_closes_in`, `a_rat_that_cannot_see_you_does_not_close_in`, `two_rats_never_stand_on_one_cell`, `a_rat_struck_to_nothing_is_gone_and_never_acts_again`, `a_walker_struck_to_nothing_takes_no_more_steps`; each sets rats and the walker where it needs them rather than searching seeds.
- [ ] **Step 2:** `cargo test -p terminal`.
  Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test -p terminal`; play it by hand with `cargo run -p terminal`.
  Expected: PASS, rats hunt and bite, the shell is restored on quit.
- [ ] **Step 5:** `scripts/gate.sh`, commit.
  No CHANGELOG line: an example.

### Task 2: The README's toolkit sample uses crate paths

**Files:**
- Modify: `README.md:145-175` (the `toml` block lists `rl-core`, `rl-grid`, `rl-mapgen`; the sample's `use` lines name `rl_core`, `rl_grid`, `rl_mapgen`)

- [ ] **Step 1:** Rewrite the imports and the dependency block.
- [ ] **Step 2:** `cargo test -p rl-engine --doc`.
  Expected: PASS (`ReadmeDoctests` runs the sample; doc-tests see `rl-engine`'s dependencies by their own names).
- [ ] **Step 3:** Commit.
  No CHANGELOG line.

### Task 3: Branch end

- [ ] `scripts/gate.sh --all`, rebase, re-gate, merge.
