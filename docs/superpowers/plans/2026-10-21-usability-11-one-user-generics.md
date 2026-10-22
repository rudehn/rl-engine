# Usability 11: Remove What Has One User Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `Pass` and `Chain` lose the context parameter only `BaseContext` fills, facet keys are plain `&'static str`, and the unused `Charts` goes.

**Architecture:** Three mechanical commits the compiler drives; no behaviour changes.

**Tech Stack:** Rust 2024, Bevy 0.19.

**Spec:** `docs/reviews/engine_usability.md` section 3.6; index `docs/superpowers/plans/2026-10-21-engine-usability.md`.

## Global Constraints

- The index's "Rules for every branch" bind this branch: worktree `scripts/worktree.sh usability-11-one-user-generics`, after branch 10 merges.
- No fingerprint moves.
- `BaseContext` keeps its name; renames are branch 15's.
- Plain dash, never an em dash.

## Review Focus

1.
  A save written before `Charts` went still loads: `EngineSave` ignores the unknown `charts` field; pinned by `a_save_that_still_carries_charts_loads`.
2.
  The README's toolkit sample and `rl-engine`'s prelude doc-test name `BuildContext` and `Chain<BaseContext>`: both are fixed and run (`cargo test -p rl-engine --doc`).
3.
  The nearby panel's facet filter keeps matching after keys become strings: pinned by the existing nearby tests.
4.
  `Explore` and `Uncharted` stay in `rl_rules::ai::tactics` for a game that pushes the sense itself; their docs stop saying the engine pushes it.
5.
  A game that declared facet keys from data at run time (none in this repository) would need `&'static str`: noted in the CHANGELOG line.

---

### Task 1: `Pass` without a context parameter

**Files:**
- Modify: `crates/rl-mapgen/src/context.rs:15-170` (the trait goes; its methods become `BaseContext`'s own), `chain.rs:52` and the `Chain` type
- Modify: every `impl Pass<BaseContext>` and `C: BuildContext` (52 impls; `rg -n 'BuildContext|Pass<' crates examples templates`)
- Modify: `README.md:152-156` (the toolkit sample), `examples/terminal/src/lib.rs:14`, `crates/rl-engine/src/lib.rs:198` (the prelude doc-test)

**Interfaces:**
- Changes: `pub trait Pass: Send + Sync { fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError>; .. }` with the rest of today's methods; `pub struct Chain`; `Chain::run(&self, ctx: &mut BaseContext, seed: impl Into<RunSeed>)`.

- [ ] **Step 1:** Delete the trait and the parameter; follow the compiler.
- [ ] **Step 2:** `cargo test -p rl-engine --doc`, `scripts/gate.sh`, which plays the fingerprints.
  Expected: PASS, unchanged.
- [ ] **Step 3:** Commit.
  CHANGELOG: "`BuildContext` is gone: `Pass` and `Chain` work on `BaseContext`, its one implementation. To change: `impl Pass<BaseContext> for X` becomes `impl Pass for X`, `Chain<BaseContext>` becomes `Chain`, and an import of `BuildContext` goes."

### Task 2: Facet keys are strings

**Files:**
- Modify: `crates/rl-ui/src/facet.rs` (`Facets`, `FacetKey`, `FacetId` go), `crates/rl-ui/src/view/nearby.rs:147` (the filter), `crates/rl-ui/src/lib.rs`
- Modify: the 28 `ResMut<Facets>` sites (`rg -n 'Facets' crates examples templates`)

**Interfaces:**
- Changes: `pub struct Facet { pub key: &'static str, pub text: String, pub tone: ToneId }`; `Facet::new(key: &'static str, text: impl Into<String>) -> Facet`; `toned` unchanged.

- [ ] **Step 1:** Convert; `facets.facet("wielding", text)` becomes `Facet::new("wielding", text)`.
- [ ] **Step 2:** `cargo nextest run -p rl-ui -p undermere -p foundry -p tutorial`, `scripts/gate.sh`.
  Expected: PASS.
- [ ] **Step 3:** Commit.
  CHANGELOG: "Facet keys are plain `&'static str`; the `Facets` resource is gone. To change: drop `ResMut<Facets>` and write `Facet::new(\"key\", text)`."

### Task 3: `Charts` goes

**Files:**
- Delete: `crates/rl-bevy/src/charts.rs`
- Modify: `crates/rl-bevy/src/lib.rs:66,145`, `crates/rl-bevy/src/minds.rs:1057-1062`, `crates/rl-bevy/src/knowledge.rs:22`
- Modify: `crates/rl-save/src/engine.rs:50,179,200` (the `charts` field and its capture and restore)
- Modify: `crates/rl-rules/src/ai/tactics.rs:614-640` (the docs of `Uncharted` and `Explore`)
- Test: `crates/rl-save/src/engine.rs` test module

- [ ] **Step 1: Write** `a_save_that_still_carries_charts_loads`: an `EngineSave` RON with a `charts` entry deserializes.
  Run it.
  Expected: PASS before and after.
- [ ] **Step 2:** Delete the module and every use.
- [ ] **Step 3:** `cargo nextest run -p rl-bevy -p rl-save`, the fingerprints, `scripts/gate.sh`.
  Expected: PASS, unchanged.
- [ ] **Step 4:** Commit.
  CHANGELOG: "`Charts`, `Charting` and their save are gone; nothing used them, and `Knowledge` is what a place remembers. To change: nothing; a game that marked an actor `Charting` pushes `Uncharted` itself for `Explore`."

### Task 4: Branch end

- [ ] `scripts/gate.sh --all`, rebase, re-gate, merge.
