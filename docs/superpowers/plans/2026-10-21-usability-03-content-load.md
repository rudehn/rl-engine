# Usability 03: Undermere's Test Apps Stop Re-Checking Its Content Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Measure what reading and checking Undermere's content costs each test `App`, and, if the cross-file checks are a real share, check them once in one test instead of in every app.

**Architecture:** A by-hand timing test first; the change in Task 2 is made only if the numbers clear the bar it sets.
The checks stay on in the binary; a test app opts out through a resource whose absence means "check everything".

**Tech Stack:** Rust 2024, Bevy 0.19, cargo-nextest, `scripts/test-times.py`.

**Spec:** `docs/reviews/engine_usability.md` section 4.2; index `docs/superpowers/plans/2026-10-21-engine-usability.md`.

## Global Constraints

- The index's "Rules for every branch" bind this branch: worktree `scripts/worktree.sh usability-03-content-load`, `scripts/gate.sh` per commit, `scripts/gate.sh --all` at the end.
- No fingerprint moves; the fingerprint test uses whatever `testing::headless` does.
- `#[ignore]` only with its reason in the attribute, for the by-hand timing.
- Plain dash, never an em dash.

## Review Focus

1.
  A content file broken in a way only a cross-file check catches must still fail the fast tier: one fast test runs every check (Task 2, Step 1).
2.
  The binary must never skip a check: `ContentChecks` absent means every check runs, and `main.rs` never inserts it.
3.
  A continued run (`run::resume`) must load the same content a fresh one does, checked or not: both go through `prepare`.
4.
  Timing taken while other agents build is noise: the timing test reports the median of five and is run on a quiet machine or twice.
5.
  A test that sets up its own app without `testing::headless` keeps full checks, which is correct and only slower.

---

### Task 1: Measure

**Files:**
- Test: `examples/undermere/src/content.rs` test module

- [ ] **Step 1: Write** `#[test] #[ignore = "by hand: times what each test app spends loading and checking content"] fn what_loading_and_checking_the_content_costs_each_test_app()`, printing the median of five runs of each: `crate::content::registries()`; `Undermere::new(RunSeed(0))`; inside it, the four cross-file checks (`biomes::check_favored`, `check_pieces`, `check_pool`, `dressing::check_districts`) timed on their own; and `crate::testing::headless(RunSeed(0))` with its first `update()`.
- [ ] **Step 2:** Run `cargo test -p undermere what_loading_and_checking -- --ignored --nocapture`, twice.
- [ ] **Step 3:** Record the medians in the commit message and in `docs/PLAN.md`'s progress log.
- [ ] **Step 4:** Commit.
- [ ] **Step 5: Decide.** Go on to Task 2 only if the four checks cost at least 0.03 s per app, which is about 8 s across Undermere's 252 fast `App` tests.
  Otherwise skip to Task 3 and say so in the progress log.

### Task 2: Test apps skip the cross-file checks

**Files:**
- Modify: `examples/undermere/src/dungeon.rs:492-509` (`Undermere::new`)
- Modify: `examples/undermere/src/run.rs:62` (`prepare`) and its callers `start` and `resume`
- Modify: `examples/undermere/src/testing/mod.rs:40` (`headless`)
- Test: `examples/undermere/src/content.rs`

**Interfaces:**
- Produces: `#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)] pub enum ContentChecks { Every, Skip }` in `undermere::content`; absent means `Every`.
- Produces: `pub fn Undermere::load(seed: RunSeed, checks: ContentChecks) -> Undermere`; `Undermere::new(seed)` is `load(seed, ContentChecks::Every)`.
- Changes: `prepare(commands, seed, registries, checks: ContentChecks)`; `start` and `resume` read `Option<Res<ContentChecks>>`.

- [ ] **Step 1: Write the test** `every_cross_file_check_passes_on_the_shipped_content`, calling `Undermere::load(RunSeed(0), ContentChecks::Every)`; it must stay in the fast tier.
- [ ] **Step 2:** Run it.
  Expected: PASS (it is the check the apps run today).
- [ ] **Step 3:** Implement; `testing::headless` and `headless_without_undermere` insert `ContentChecks::Skip`.
- [ ] **Step 4:** Break one cross-reference in `assets/biomes.ron` by hand and run `cargo nextest run -p undermere -E 'test(every_cross_file_check)'`.
  Expected: FAIL naming the file.
  Restore it.
- [ ] **Step 5:** Run `scripts/gate.sh`; compare Undermere's fast-tier processor time with Task 1's baseline run.
  Commit with both numbers.

### Task 3: Branch end

- [ ] `scripts/gate.sh --all`, rebase, re-gate, merge.
- [ ] No CHANGELOG line: nothing in the engine changed.
