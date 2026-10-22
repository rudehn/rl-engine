# Usability 13: Ambiguity Tests Through the Schedule, and Engine Systems Private Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Foundry's ordering test names only Foundry's systems, the engine checks its own systems' conflicts in its own test, and engine system functions no game calls become `pub(crate)`.

**Architecture:** Foundry already compares apps with and without its plugin (`ambiguity::added`); what still names about 45 engine systems is its allow list and a `headless_without_foundry` that leaves out engine plugins `FoundryPlugin` adds itself.
Those plugins move to a `plugin::add_engine` that `FoundryPlugin::build` and `headless_without_foundry` both call, as Undermere's do (`examples/undermere/src/plugin.rs:63,108`), and the engine-against-engine allowances move to an rl-bevy test.
A game describes itself only through the engine's public API, so its allowances name its own system and the data (`Allowed::with_any`) or the sets (`Allowed::among`), never an engine function; engine-against-engine pairs stay exact, by `TypeId`, in the engine's own tests.

**Tech Stack:** Rust 2024, Bevy 0.19.

**Spec:** `docs/reviews/engine_usability.md` section 3.4 and the index's correction of it; `docs/TODO.md` items 11 and 28; index `docs/superpowers/plans/2026-10-21-engine-usability.md`.

## Global Constraints

- The index's "Rules for every branch" bind this branch: worktree `scripts/worktree.sh usability-13-ambiguity`, after branch 12 merges.
- Every allowance keeps a reason; a game's `with_any` reason must hold for any engine system on that data, and where it holds only for one, the game orders its system against that system's set instead.
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
  An allowance that matched nothing after the move: `assert_ordered` fails on an unused entry (`testing/ambiguity.rs`, the `stale` assertion), and the pair it covered fails as unexplained.
4.
  A system made `pub(crate)` that the guide quotes by path: `scripts/check-guide.sh` still passes.
5.
  The `Acting` allowance admitting any pair: narrowed to systems in `TurnSet::Resolve` and `TurnSet::Sweep`, pinned by `an_allowance_among_sets_admits_no_pair_outside_them`.
6.
  A game reaching engine internals: after this branch neither game's ambiguity test names an engine function, and Undermere's needs no change beyond `among`, since it already allows by its own system and the data.

---

### Task 1: The engine checks its own conflicts

**Files:**
- Create: `crates/rl-bevy/src/ambiguity.rs` (`#[cfg(test)]`, declared in `lib.rs`)

- [ ] **Step 1: Write** `every_conflicting_pair_of_the_engines_systems_is_ordered_or_allowed_for_a_stated_reason`: `headless_app()` with every rl-bevy plugin, generic ones with the test makers rl-bevy's own tests use, never a list taken from a game; `ours` is `ambiguity::added` against a bare `headless_app()`.
- [ ] **Step 2:** Run it.
  Expected: FAIL, listing the engine pairs.
- [ ] **Step 3:** Move each engine-against-engine allowance from `examples/foundry/src/plugin/ambiguity.rs` here with its reason, an exact `Allowed::pair` of in-crate functions; add an allowance with a reason for any other pair, or order the pair where an order is right.
  A moved pair with an `rl-save` or `rl-ui` system (`refresh_stash`, `save_on_arrival`, `flush_on_exit`, `forget_save` today) goes to the same test in that crate, which adds `rl-bevy`'s plugins and allows it by its own system and the data, since the `rl-bevy` function is `pub(crate)` by Task 4.
- [ ] **Step 4:** Run it.
  Expected: PASS.
  `scripts/gate.sh`, commit.

### Task 2: An allowance among sets

**Files:**
- Modify: `crates/rl-bevy/src/testing/ambiguity.rs:96-139`

**Interfaces:**
- Produces: `pub fn Allowed::among(sets: Vec<InternedSystemSet>, on: Vec<ComponentId>, why: &'static str) -> Allowed`: a pair is admitted only when both systems sit in one of `sets`.
- Changes: the module docs' claim that the workspace builds without Bevy's `debug` feature (`ambiguity.rs:19-21`) is corrected; nothing matches on a system's name.

- [ ] **Step 1: Write the failing tests** `an_allowance_among_sets_admits_a_pair_inside_them` and `an_allowance_among_sets_admits_no_pair_outside_them`, each on a small app of two test systems.
- [ ] **Step 2:** Implement from the schedule graph's set membership.
- [ ] **Step 3:** The `Acting` entries in Foundry's and Undermere's tests and the engine's become `Allowed::among(vec![TurnSet::Resolve.intern(), TurnSet::Sweep.intern()], ..)`.
- [ ] **Step 4:** `cargo nextest run -p rl-bevy -p foundry -p undermere -E 'test(/ambigu|conflicting/)'`, `scripts/gate.sh`.
  Expected: PASS.
  Commit.
  CHANGELOG: "`Allowed::among` admits a conflicting pair only inside the named system sets. To change: nothing."

### Task 3: Foundry names only its own systems

**Files:**
- Modify: `examples/foundry/src/plugin.rs:55,64,69,107` (the engine plugins, `PropsPlugin`, `RemainsPlugin`, `LootPlugin`, `PrefabPlugin` and `AmmoPlugin`, move to `pub fn add_engine(app: &mut App)`, which `FoundryPlugin::build` calls first)
- Modify: `examples/foundry/src/testing/mod.rs:66` (`headless_without_foundry` calls `add_engine`)
- Modify: `examples/foundry/src/plugin/ambiguity.rs`

- [ ] **Step 1:** Move the plugins; run the Foundry ordering test.
  Expected: FAIL on the entries that no longer match.
- [ ] **Step 2:** `names()` lists only Foundry's systems; each `pair(ours, engine_fn, ..)` becomes `Allowed::with_any(ours, on(..), reason)` with its reason restated for any engine system on that data, or Foundry's system is ordered against the engine's set where the reason holds for one system only; the engine-against-engine ones are gone (Task 1 holds them).
- [ ] **Step 3:** `rg -n 'id\(rl_engine' examples/*/src/plugin/ambiguity.rs` finds nothing, Foundry's or Undermere's: no engine function is named.
- [ ] **Step 4:** `scripts/gate.sh`, which plays Foundry's tests and fingerprint.
  Expected: PASS, unchanged.
  Commit.

### Task 4: Engine systems are crate-private

**Files:**
- Modify: system functions in `crates/rl-bevy/src`, `crates/rl-ui/src`, `crates/rl-save/src` registered with `add_systems` and declared `pub fn`

- [ ] **Step 1:** Make each such function `pub(crate)`; build the workspace; restore `pub` only on functions that are part of the API a game calls (`begin_run`, `clear_run`, `revive` and the like), never on one a game names only in a test or an ordering, and list them in the commit.
- [ ] **Step 2:** `rl_bevy::plugin::headless_app`, a test helper, moves to `rl_bevy::testing`, the test-support API; the tutorial's tests reach it through `rl_engine::testing::windowless` (branch 06 moves them).
- [ ] **Step 3:** Drop each re-export at a crate root or prelude that named a now-private function.
- [ ] **Step 4:** `scripts/check-guide.sh`, `cargo test --doc --workspace`, `scripts/gate.sh`.
  Expected: PASS.
- [ ] **Step 5:** Commit with the count made private.
  CHANGELOG: "Engine systems are no longer public; a game orders its systems by the engine's sets, never by an engine function. To change: a game that named an engine system function orders by the set it runs in, and an ambiguity allowance names its own system and the data with `Allowed::with_any`; `rl_bevy::plugin::headless_app` is `rl_bevy::testing::headless_app`."

### Task 5: Branch end

- [ ] Remove TODO 11 and 28 from `docs/TODO.md`; a line each in `docs/PLAN.md`'s progress log, the one for 28 saying a game allows a pair with an engine system by that system's Bevy name, not by the data alone as the TODO proposed, so each allowance stays one pair.
- [ ] `scripts/gate.sh --all`, rebase, re-gate, merge.
