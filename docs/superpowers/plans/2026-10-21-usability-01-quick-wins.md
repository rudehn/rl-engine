# Usability 01: Quick Wins Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Do the review's rank-1 items: drop the redundant `DamageStages`, make the tutorial's numbering match the guide, add a `familiar` test helper, and keep test timings across worktrees.

**Architecture:** Four independent commits; no engine API changes except removals of redundant setup in examples.

**Tech Stack:** Rust 2024, Bevy 0.19, cargo-nextest, Python 3 for `scripts/test-times.py`.

**Spec:** `docs/reviews/engine_usability.md` sections 1.5, 1.8, 4.5, 4.7; index `docs/superpowers/plans/2026-10-21-engine-usability.md`.

## Global Constraints

- The index's "Rules for every branch" bind this branch: worktree `scripts/worktree.sh usability-01-quick-wins`, `scripts/gate.sh` per commit, `scripts/gate.sh --all` at the end.
- No fingerprint moves.
- Plain dash, never an em dash; each Markdown sentence on its own line.

## Review Focus

1.
  A `DamageStages` removed from an app with no `CombatPlugin` leaves no stages at all: only games that add `CombatPlugin` lose the line; the two engine test inserts (`crates/rl-bevy/src/testing.rs:122` in `two_sides`, `crates/rl-bevy/src/status.rs:668`) stay, since `two_sides` serves apps with and without it.
2.
  A renamed tutorial binary breaks a guide include silently until the release: the same commit fixes every path, and `scripts/check-guide.sh` runs on it.
3.
  `familiar` skipping the real path: the helper sets the count one short and plays one turn, so `grow_familiar` still makes the thing known and says so.
4.
  The timing file written inside a worktree that is later removed: it lives in the git common directory, shared by every worktree.
5.
  Two gates appending at once: each run appends whole lines in one `write` call.

---

### Task 1: The default `DamageStages` is not written out

**Files:**
- Modify: `examples/tutorial/src/bin/step03_blows.rs:131`, `step04_things.rs:135`, `step05_knack.rs:150`, `step06_descent.rs:191`, `step08_content.rs:266`, `step09_shove.rs:279`, `step10_panels.rs:344`
- Modify: `templates/starter/src/main.rs:219`
- Modify: `CHANGELOG.md`

- [ ] **Step 1:** Delete each `commands.insert_resource(DamageStages(vec![Box::new(SubtractArmor)]));` above, and any import it leaves unused.
- [ ] **Step 2:** Leave Undermere's and Foundry's lists, which differ from the default, and the two engine test inserts.
- [ ] **Step 3:** Run `cargo nextest run -p tutorial` and `scripts/check-template.sh`.
  Expected: PASS.
- [ ] **Step 4:** Commit with the CHANGELOG line: "The tutorial and the template no longer insert `DamageStages`, whose default is already `[SubtractArmor]`. To change: nothing; a game may delete the same line."

### Task 2: Chapter N runs `stepNN`

**Files:**
- Rename: `examples/tutorial/src/bin/step08_content.rs` to `step07_content.rs`, `step09_shove.rs` to `step08_shove.rs`, `step10_panels.rs` to `step09_panels.rs`
- Modify: every file `rg -l --hidden -g '!.git' 'step0[89]_|step10_' -g '!docs/superpowers/**' -g '!docs/reviews/**'` names: guide chapters 07 to 09, `docs/guide/src/systems/{controls,rendering,narration,panels,registries}.md`, `docs/PLAN.md`, `AGENTS.md`, `.planning/codebase/{STRUCTURE,ARCHITECTURE}.md`, and the three binaries' own `cargo run` lines
- Modify: `README.md:105`

- [ ] **Step 1:** `git mv` the three files, highest first, so no name is taken twice.
- [ ] **Step 2:** Replace `step10_panels` with `step09_panels`, then `step09_shove` with `step08_shove`, then `step08_content` with `step07_content`, in that order, in every file listed.
- [ ] **Step 3:** `README.md:105` reads "nine steps that each add one thing, the first six playable in the browser".
- [ ] **Step 4:** Run `scripts/check-guide.sh` and `rg --hidden -g '!.git' 'step0[89]_content|step09_shove|step10_' -g '!docs/superpowers/**' -g '!docs/reviews/**'`.
  Expected: the check passes and the search finds nothing.
- [ ] **Step 5:** Run `scripts/gate.sh` and commit.
  CHANGELOG: "The tutorial's binaries are numbered as the guide's chapters: `step07_content`, `step08_shove`, `step09_panels`. To change: a script that runs one by name uses the new name."

### Task 3: `familiar`, a worn thing made known without playing 200 turns

**Files:**
- Modify: `examples/undermere/src/testing/mod.rs` (the helper)
- Modify: `examples/undermere/src/seeming.rs:409`, `examples/undermere/src/enchant.rs:311`, `examples/undermere/src/armory.rs:1653`

**Interfaces:**
- Produces: `pub fn familiar(app: &mut App, thing: Entity)` in `undermere::testing`: sets `Unfamiliar.worn` to `FAMILIAR_AFTER - 1` and runs `pass_turns(app, 1)`.
  It panics naming the thing if the thing is not `Unfamiliar` or not worn.

- [ ] **Step 1:** Write the helper; `examples/undermere/src/main.rs:1139` already sets the count the same way and is the model.
- [ ] **Step 2:** Replace `pass_turns(&mut app, FAMILIAR_AFTER)` with `familiar(&mut app, thing)` at the three sites, keeping every assertion.
- [ ] **Step 3:** Keep `lore.rs`'s `armor_found_is_called_unidentified_until_it_has_been_worn_two_hundred_turns`, which plays the turns on purpose.
- [ ] **Step 4:** Run `cargo nextest run -p undermere -E 'test(/familiar|unfamiliar|seeming|enchant|armory/)'`.
  Expected: PASS, each changed test under 1 s of processor time in `scripts/test-times.py --top 10`.
- [ ] **Step 5:** `scripts/gate.sh`, commit.
  No CHANGELOG line: nothing public changed.

### Task 4: Test timings outlive the worktree

**Files:**
- Modify: `scripts/test-times.py` (`main`: the per-package totals at lines 138-143 move out of `if args.top:` so they exist on every run)
- Modify: `AGENTS.md`, "Writing fast tests", one sentence naming the file

**Interfaces:**
- Produces: `$(git rev-parse --git-common-dir)/rl-test-times.tsv`, tab-separated, header `when\tbranch\tcommit\tprofile\tpackage\ttests\tcpu_s\twall_s`, one line per package per run, appended in one write.

- [ ] **Step 1:** Add `def keep(rows, profile)` writing the header when the file is new and appending the rows; `when` is UTC ISO 8601 to the second; branch and commit come from `git rev-parse --abbrev-ref HEAD` and `git rev-parse --short HEAD`.
- [ ] **Step 2:** Call it after the report on every run that read a junit file, except `--profile ci`, whose checkout is thrown away; a failure to write warns and never fails the check.
- [ ] **Step 3:** Run `scripts/gate.sh` twice.
  Expected: the file gains one set of package lines per run, and its path resolves to the main checkout's `.git`.
- [ ] **Step 4:** Commit.
  No CHANGELOG line.

### Task 5: Branch end

- [ ] Run `scripts/gate.sh --all`, rebase on `main`, re-gate, merge, remove the worktree.
