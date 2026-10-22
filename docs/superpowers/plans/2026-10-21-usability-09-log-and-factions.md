# Usability 09: The Log Stamps the Turn and Factions Are Named Once Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A log line is stamped with the turn by the engine, and factions are named once.
Step 1's `PlayerKeysPlugin`, the review's third first-step item, moved to branch 06, which rewrites those lines.

**Architecture:** Two commits.
`Log` is a system param over `MessageLog` and `Turns`; `MessageLog` keeps one `push_at` for exclusive systems and tests.
`Factions` keeps only the relations a game named and answers the rest by rule (a side is allied with itself, neutral to others), so it needs no registry and `CombatRules::new()` takes nothing.

**Tech Stack:** Rust 2024, Bevy 0.19.

**Spec:** `docs/reviews/engine_usability.md` sections 1.6 and 1.7; index `docs/superpowers/plans/2026-10-21-engine-usability.md` ("Review claims found wrong", 1.6).

## Global Constraints

- The index's "Rules for every branch" bind this branch: worktree `scripts/worktree.sh usability-09-log-and-factions`, after branch 07 merges.
- No fingerprint moves: the same relations and the same turn numbers.
- No `HashMap` in `Factions`: a `BTreeMap`, ordered and small.
- Plain dash, never an em dash.

## Review Focus

1.
  A line pushed at a run's start reads turn 0, as the template's `0` did: `Turns` is reset before `NewRun`; pinned by `a_line_pushed_through_the_log_carries_the_turn_it_was_pushed_on`.
2.
  A relation named one way only stays one way: `hunts` is asymmetric; pinned by `relations_default_and_can_be_asymmetric`, kept.
3.
  A faction id from no registry at all, as a toolkit game makes them: allied with itself, neutral to others, never a panic; pinned by `a_side_never_named_is_allied_with_itself_and_neutral_to_others`.
4.
  A continued run's log: Undermere's `resume` stamps the saved turn with `push_at`.

---

### Task 1: The log stamps its own turn

**Files:**
- Modify: `crates/rl-ui/src/log.rs:90-129`
- Modify: every caller passing a turn: `rg -n '\.(push|info|muted|notice)\(' examples templates crates/rl-ui` (about 55)

**Interfaces:**
- Produces: `#[derive(SystemParam)] pub struct Log<'w> { log: ResMut<'w, MessageLog>, turns: Res<'w, Turns> }` with `push(&mut self, text: impl Into<String>, tone: ToneId)`, `info(text)`, `muted(text)`, `notice(text)`, each stamping `turns.turn_number()`.
- Changes: `MessageLog::push` becomes `push_at(&mut self, text: impl Into<String>, tone: ToneId, turn: u32)`; `MessageLog::info`, `muted` and `notice` go.

- [ ] **Step 1: Write the failing test** `a_line_pushed_through_the_log_carries_the_turn_it_was_pushed_on` in `log.rs`: pass three turns, push through a system taking `Log`, read the entry's turn.
- [ ] **Step 2:** `cargo test -p rl-ui log`.
  Expected: FAIL to compile.
- [ ] **Step 3:** Implement; systems take `Log`, exclusive systems and tests call `push_at`.
- [ ] **Step 4:** `scripts/gate.sh`, which plays the fingerprints.
  Expected: PASS, unchanged.
- [ ] **Step 5:** Commit.
  CHANGELOG: "A system writes to the log through the `Log` param, which stamps the current turn: `log.notice(text)`. To change: `ResMut<MessageLog>` becomes `Log`, and the turn argument goes; an exclusive system or a test calls `MessageLog::push_at(text, tone, turn)`."

### Task 2: Factions are named once

**Files:**
- Modify: `crates/rl-rules/src/faction.rs:41-101` (`Factions`)
- Modify: `crates/rl-bevy/src/combat.rs:293-330` (`CombatRules::new`, `Default`), its doc example at `:270-284`
- Modify: the 25 `CombatRules::new(&..)` sites (`rg -n 'CombatRules::new\('`)

**Interfaces:**
- Changes: `#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)] pub struct Factions { named: BTreeMap<(FactionId, FactionId), Relation> }`; `Factions::new() -> Self`; `set`, `set_mutual` and `relation` keep their signatures; `relation(a, b)` is the named one, else `Allied` when `a == b`, else `Neutral`.
- Changes: `CombatRules::new() -> Self` and `impl Default for CombatRules`.

- [ ] **Step 1: Write the failing test** `a_side_never_named_is_allied_with_itself_and_neutral_to_others` in `faction.rs`, with ids from `Id::from_raw` and no registry.
- [ ] **Step 2:** `cargo test -p rl-rules faction`.
  Expected: FAIL to compile.
- [ ] **Step 3:** Implement; move the 25 sites; a `Registry<FactionDef>` argument that was only for `CombatRules` goes.
- [ ] **Step 4:** `cargo test -p rl-rules faction`, then `scripts/gate.sh`, which plays the fingerprints.
  Expected: PASS, fingerprints unchanged.
- [ ] **Step 5:** Commit.
  CHANGELOG: "`CombatRules::new()` and `Factions::new()` take no registry: relations a game never names are worked out (a side is allied with itself and neutral to the rest). To change: drop the argument, `CombatRules::new(&sides)` becomes `CombatRules::new()`."

### Task 3: Branch end

- [ ] `scripts/gate.sh --all`, rebase, re-gate, merge.
