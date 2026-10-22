# Usability 13: Ambiguity Tests Through the Schedule, and Engine Systems Private Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Foundry's ordering test names only Foundry's systems, the engine checks its own systems' conflicts in its own test, and engine system functions no game calls become `pub(crate)`.

**Architecture:** Foundry already compares apps with and without its plugin (`ambiguity::added`); what still names about 45 engine systems is its allow list and a `headless_without_foundry` that leaves out engine plugins `FoundryPlugin` adds itself.
Those plugins move to a `plugin::add_engine` that `FoundryPlugin::build` and `headless_without_foundry` both call, as Undermere's do (`examples/undermere/src/plugin.rs:63,108`), and the engine-against-engine allowances move to an rl-bevy test.

**Tech Stack:** Rust 2024, Bevy 0.19.

**Spec:** `docs/reviews/engine_usability.md` section 3.4 and the index's correction of it; `docs/TODO.md` items 11 and 28; index `docs/superpowers/plans/2026-10-21-engine-usability.md`.

## Global Constraints

- The index's "Rules for every branch" bind this branch: worktree `scripts/worktree.sh usability-13-ambiguity`, after branch 12 merges.
- Every allowance keeps a reason; none is widened beyond what its reason covers.
- A system function a game or another engine crate calls outside tests stays `pub`.
- No fingerprint moves: the plugins Foundry adds do not change, only where the call sits.
  If moving them moves Foundry's fingerprint, some unordered pair depends on the order systems were added: order that pair, never re-baseline.
- Plain dash, never an em dash.

## Review Focus

1.
  An engine-against-engine conflict that Foundry's test used to catch must still be caught: each such allowance moves to the rl-bevy test, pinned by that test failing when one is deleted.
2.
  Moving `FoundryPlugin`'s engine plugins into `add_engine` must keep the binary's plugins: `FoundryPlugin::build` calls `add_engine(app)` first, so `main.rs` does not change.
3.
  An allowance that matched nothing after the move: `assert_ordered` already fails on an unused entry.
4.
  A system made `pub(crate)` that the guide quotes by path: `scripts/check-guide.sh` still passes.
5.
  The `Acting` allowance admitting any pair: narrowed to systems in `TurnSet::Resolve` and `TurnSet::Sweep`, pinned by `an_allowance_among_sets_admits_no_pair_outside_them`.

---

### Task 1: The engine checks its own conflicts

**Files:**
- Create: `crates/rl-bevy/src/ambiguity.rs` (`#[cfg(test)]`, declared in `lib.rs`)

- [ ] **Step 1: Write** `every_conflicting_pair_of_the_engines_systems_is_ordered_or_allowed_for_a_stated_reason`: `headless_app()` with every rl-bevy plugin Foundry or Undermere adds, generic ones with the test makers rl-bevy's own tests use; `ours` is `ambiguity::added` against a bare `headless_app()`.
- [ ] **Step 2:** Run it.
  Expected: FAIL, listing the engine pairs.
- [ ] **Step 3:** Move each engine-against-engine allowance from `examples/foundry/src/plugin/ambiguity.rs` here with its reason, naming the functions in-crate; add an allowance with a reason for any other pair, or order the pair where an order is right.
- [ ] **Step 4:** Run it.
  Expected: PASS.
  `scripts/gate.sh`, commit.

### Task 2: Foundry names only its own systems

**Files:**
- Modify: `examples/foundry/src/plugin.rs:55,64,69,107` (the engine plugins, `PropsPlugin`, `RemainsPlugin`, `LootPlugin`, `PrefabPlugin` and `AmmoPlugin`, move to `pub fn add_engine(app: &mut App)`, which `FoundryPlugin::build` calls first)
- Modify: `examples/foundry/src/testing/mod.rs:66` (`headless_without_foundry` calls `add_engine`)
- Modify: `examples/foundry/src/plugin/ambiguity.rs`

- [ ] **Step 1:** Move the plugins; run the Foundry ordering test.
  Expected: FAIL on the entries that no longer match.
- [ ] **Step 2:** `names()` lists only Foundry's systems; each `pair(ours, engine_fn, ..)` becomes `Allowed::with_any(ours, on(..), reason)`; the engine-against-engine ones are gone (Task 1 holds them).
- [ ] **Step 3:** `rg -n 'rl_bevy::|rl_ui::|rl_save::' examples/foundry/src/plugin/ambiguity.rs` names no system function.
- [ ] **Step 4:** `cargo nextest run -p foundry`, fingerprints, `scripts/gate.sh`.
  Expected: PASS, unchanged.
  Commit.

### Task 3: An allowance among sets

**Files:**
- Modify: `crates/rl-bevy/src/testing/ambiguity.rs:96-139`

**Interfaces:**
- Produces: `pub fn Allowed::among(sets: Vec<InternedSystemSet>, on: Vec<ComponentId>, why: &'static str) -> Allowed`: a pair is admitted only when both systems sit in one of `sets`.

- [ ] **Step 1: Write the failing tests** `an_allowance_among_sets_admits_a_pair_inside_them` and `an_allowance_among_sets_admits_no_pair_outside_them`.
- [ ] **Step 2:** Implement from the schedule graph's set membership.
- [ ] **Step 3:** The `Acting` entries in Foundry's and Undermere's tests and the engine's become `Allowed::among(vec![TurnSet::Resolve.intern(), TurnSet::Sweep.intern()], ..)`.
- [ ] **Step 4:** `cargo nextest run -p rl-bevy -p foundry -p undermere -E 'test(/ambigu|conflicting/)'`, `scripts/gate.sh`.
  Expected: PASS.
  Commit.
  CHANGELOG: "`Allowed::among` admits a conflicting pair only inside the named system sets. To change: nothing."

### Task 4: Engine systems are crate-private

**Files:**
- Modify: system functions in `crates/rl-bevy/src`, `crates/rl-ui/src`, `crates/rl-save/src` registered with `add_systems` and declared `pub fn`

- [ ] **Step 1:** Make each such function `pub(crate)`; build the workspace; restore `pub` only on those a game or another crate calls outside tests (`begin_run`, `clear_run`, `revive` and the like), and list them in the commit.
- [ ] **Step 2:** Drop each re-export at a crate root or prelude that named a now-private function.
- [ ] **Step 3:** `scripts/check-guide.sh`, `cargo test --doc --workspace`, `scripts/gate.sh`.
  Expected: PASS.
- [ ] **Step 4:** Commit with the count made private.
  CHANGELOG: "Engine systems are no longer public; a game orders its systems by the engine's sets, never by an engine function. To change: a game that named an engine system function, in an ordering or a test, names the set it runs in or the data it shares."

### Task 5: Branch end

- [ ] Remove TODO 11 and 28 from `docs/TODO.md`; a line each in `docs/PLAN.md`'s progress log.
- [ ] `scripts/gate.sh --all`, rebase, re-gate, merge.
