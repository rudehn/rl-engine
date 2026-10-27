# Lean Process Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Merging one small commit into `main` takes one minute-long gate instead of two gates, two test tiers and a budget check, and the hang in `scripts/slot` is gone.

**Architecture:** Tests run under plain `cargo test`, which ran all 1,975 tests in 9 s (23 s with every seed) against nextest's 26 s, so nextest, the test tiers, per-test processor-time accounting and the budgets all go.
`scripts/slot` keeps wrapping compiles only, rewritten so it never calls `fork()`.
The CHANGELOG moves to Keep a Changelog 1.1.0 with one line per change, so the release page is the changelog section verbatim and `scripts/release-notes.sh` goes.
AGENTS.md keeps its rules and loses its history.
Last, `atglyph::bevy` is removed, through the new process.

**Tech Stack:** Rust workspace (Bevy 0.19), bash, Python 3.9 (`/usr/bin/python3`), GitHub Actions.

**Spec:** this conversation's agreed list, restated here:
1. One test tier; every gate runs every test.
2. One gate, about a minute warm: fmt, clippy, `cargo test --workspace` (doc-tests included), `check-tiers.sh`, `check-names.sh`, `check-prelude.py`. The wasm build stays in CI only.
3. Tests stop going through `slot`; nextest, `test-times.py`, the budgets and the times history go.
4. `slot` stops hanging in macOS's `fork()` (`_asl_mt_shim_fork_child` → `objc_release`, seen 2026-10-26 holding slot 3 for 10 minutes).
5. AGENTS.md keeps every rule, drops measurements, dates and history.
6. CHANGELOG follows https://keepachangelog.com/en/1.1.0/, one line per change; `release-notes.sh` goes.
7. `rename-to-atglyph.py`, its test, and `check-systems-style.sh` go.
8. `atglyph::bevy` is removed; the README note about depending on Bevy directly (uncommitted on `main` now) lands with it.

## Global Constraints

- Plain dash `-`, never an em dash, in every file and commit message.
- Markdown: one full sentence per physical line.
- Commit messages: a subject line under 72 characters, and at most a few lines of body. No `Co-Authored-By` line (Nate's global instructions override the harness attribution).
- All work in one worktree made with `scripts/worktree.sh lean-process`, at `.claude/worktrees/lean-process`; never switch the main checkout's branch.
- Before starting, move the uncommitted README edit off `main`: `git -C <main> diff README.md > <scratchpad>/readme-note.patch && git -C <main> checkout README.md`, and apply the patch in Task 6.
This plan, also uncommitted on `main`, moves the same way and is the branch's first commit.
- `ATGLYPH_FULL_SEEDS=1` and `seeds(full)` stay as they are; CI keeps setting it.
- `docs/superpowers/`, `docs/reviews/` and `.planning/` are historical records: leave their mentions of nextest and the old scripts alone.

## Review Focus

1. Tests sharing process state now that a crate's tests run as threads in one process (same file paths, environment variables, globals): the suite must pass five runs in a row. Pinned in Task 1, Step 5.
2. A killed compile must free its slot, and a long-lived child (the sccache server) must not inherit the lock. Pinned in Task 2, Step 4.
3. The loader paths on macOS: `/usr/bin/python3` is a protected binary, so `DYLD_*` is stripped as it was for perl, and the `RL_`/`NEXTEST_` re-export must still reach rustc. Pinned in Task 2, Step 5 (a build that links Bevy's dylib and runs a test).
4. The release page drawn from the wrong section: `0.4.1` must not match `0.4.10`, and a missing section must fail the job, not publish an empty page. Pinned in Task 3, Step 4.
5. `check-overview.sh` reads the layout section of AGENTS.md; a rewrite that drops a design doc from it fails the release. Pinned in Task 5, Step 3.

---

### Task 1: One tier, `cargo test`, one gate

**Files:**
- Delete: `.config/nextest.toml`, `scripts/test-times.py`, and `$(git rev-parse --git-common-dir)/rl-test-times.tsv`
- Modify: `scripts/gate.sh` (whole file), `.github/workflows/ci.yml:68-84`
- Modify: `scripts/slot:66-85`, the block recording a test's processor time under `NEXTEST_RUN_ID` (removed; Task 2 rewrites the file anyway, but this task must leave the tree working)

- [ ] **Step 1: Reproduce the hang end to end, before nextest goes.**
Run the suite the way the gate did, ten times, each under a timeout:

```bash
for i in $(seq 10); do timeout 180 cargo nextest run --workspace --profile full >/dev/null 2>&1; echo "run $i rc=$?"; done
```

Expected: some run ends `rc=124`. While it hangs, `ps -A -o pid,pcpu,etime,command | grep scripts/slot` shows a perl process at 100% CPU, and `sample <pid> 1` shows `Perl_pp_fork` → `_asl_mt_shim_fork_child` → `objc_release`.
Write down how many of the ten runs hung; Task 2 needs the number.
If none hang, record that, and Task 2's stress loop is the reproduction.

- [ ] **Step 2: Rewrite `scripts/gate.sh` as one gate.**
Keep `set -euo pipefail`, the `cd` to the toplevel, and `step()`.
The body is exactly these steps, in this order, and no arguments:

```bash
step cargo fmt --all --check
step scripts/check-names.sh
step scripts/check-prelude.py
step cargo clippy --workspace --all-targets -- -D warnings
step cargo test --workspace
step scripts/check-tiers.sh
echo "gate: passed"
```

The header comment is three or four lines: what the gate runs, that CI also plays every seed and builds wasm, and that `ATGLYPH_FULL_SEEDS=1` passes through.
Nothing takes `--all` any more: find each caller with `grep -rn "gate.sh --all" --exclude-dir=target --exclude-dir=docs/superpowers .` and change it to `scripts/gate.sh` (AGENTS.md is Task 5's).

- [ ] **Step 3: Delete nextest's config, the budget script and the history file.**
`git rm .config/nextest.toml scripts/test-times.py`; `rm "$(git rev-parse --git-common-dir)/rl-test-times.tsv"`; delete the `NEXTEST_RUN_ID` block from `scripts/slot`.
If `.config/` is then empty, it goes too.

- [ ] **Step 4: Change CI's test steps.**
In `.github/workflows/ci.yml`, replace the `install-action` nextest step, the `tests` step, the `test times` step and the `doc-tests` step with one step:

```yaml
      - name: tests
        run: cargo test --workspace --no-fail-fast
        env:
          ATGLYPH_FULL_SEEDS: 1
```

- [ ] **Step 5: Prove the suite is stable as threads.**
Run: `for i in 1 2 3 4 5; do cargo test --workspace -q 2>&1 | grep -E "^test result: FAILED|panicked" ; done; echo done`
Expected: only `done`. Then `ATGLYPH_FULL_SEEDS=1 cargo test --workspace -q` passes once.
A failure here is a test that shared state with another; fix the test, not the runner.

- [ ] **Step 6: Run the gate and time it.**
Run: `time scripts/gate.sh`
Expected: `gate: passed`, and under 90 s with a warm build.

- [ ] **Step 7: Commit.**
`git commit -am "tests run under cargo test in one tier, and the gate is one gate"` with the deletions staged.

### Task 2: `slot` without `fork()`

**Files:**
- Modify: `scripts/slot` (whole file, rewritten in Python)
- Unchanged callers: `scripts/rustc-wrapper:56` (`exec "$scripts/slot", @command`)

**Interfaces:**
- Consumes: nothing from Task 1 beyond the removed `NEXTEST_RUN_ID` block.
- Produces: `scripts/slot <command> [args...]`, same interface, same exit status and signal behaviour, same slot files `/tmp/rl-build-slots/0..N-1`, same `RL_BUILD_SLOTS` override.

- [ ] **Step 1: Write the stress loop that reproduces the hang against the current perl `slot`.**
Save it in the scratchpad as `slot-stress.sh`: 20,000 runs of `scripts/slot /usr/bin/true` through `xargs -P 16`, each under `timeout 10`, printing the count of exits other than 0.

```bash
seq 20000 | xargs -P 16 -I{} sh -c 'timeout 10 scripts/slot /usr/bin/true || echo hung' | grep -c hung
```

- [ ] **Step 2: Run it against the perl `slot`.**
Expected: a count above 0 reproduces the hang in isolation.
If it is 0, raise to 100,000 runs once; if still 0, Task 1 Step 1's count is the evidence, and the stack trace is the cause.

- [ ] **Step 3: Rewrite `scripts/slot` in Python.**
Shebang `#!/usr/bin/python3 -IS` (one token: macOS passes everything after the interpreter as a single argument).
Keep the header comment's substance in fewer lines: why slots, why a lock file and not a jobserver, why the command must not inherit the lock, and, new, why `posix_spawn`: macOS runs `fork()`'s child handlers, and one of them (`_asl_mt_shim_fork_child`) was seen spinning forever in a forked perl, holding its slot.
Behaviour, in order:
- slots from `RL_BUILD_SLOTS`, else `os.cpu_count()`, else 8; directory `/tmp/rl-build-slots`, made if missing;
- loop over `0..slots-1`, `os.open(path, os.O_RDWR | os.O_CREAT)` then `fcntl.flock(fd, LOCK_EX | LOCK_NB)`; on `BlockingIOError` close and try the next; sleep 0.05 s after a full pass without one;
- copy every `NEXTEST_DYLD_*`, `NEXTEST_LD_*`, `RL_DYLD_*`, `RL_LD_*` variable to its unprefixed name unless that is already set (drop the `NEXTEST_` half if nothing sets it any more: `grep -rn NEXTEST_ scripts`);
- `pid = os.posix_spawnp(argv[0], argv, env)`; Python opens fds non-inheritable by default (PEP 446), so the lock never reaches the command; ignore `SIGINT`, `SIGTERM`, `SIGHUP` after spawning;
- `os.waitpid(pid, 0)`, close the lock, then mirror the status: on a signal restore its default handler and `os.kill(os.getpid(), sig)`; else exit with the code;
- usage error to stderr and exit 2 with no arguments; a command that cannot be spawned prints `slot: cannot run <cmd>: <err>` and exits 127.

- [ ] **Step 4: Check the lock's two edges.**
Run with `RL_BUILD_SLOTS=1`:
`scripts/slot sleep 30 & sleep 1; kill -9 $(pgrep -n sleep); wait; RL_BUILD_SLOTS=1 timeout 5 scripts/slot /usr/bin/true && echo freed`
Expected: `freed`.
Then `RL_BUILD_SLOTS=1 scripts/slot sh -c 'sleep 60 &' ; RL_BUILD_SLOTS=1 timeout 5 scripts/slot /usr/bin/true && echo not-inherited`
Expected: `not-inherited` (the backgrounded child outlives the command, as the sccache server does, and does not hold the slot).
`scripts/slot sh -c 'exit 3'; echo $?` prints `3`.

- [ ] **Step 5: Run the stress loop against the new `slot`, then a build that links Bevy.**
Run Step 1's loop at the count that reproduced the hang (or 100,000).
Expected: `0`.
Then `touch crates/atglyph-bevy/src/lib.rs && scripts/gate.sh`
Expected: `gate: passed`; the rebuild of `atglyph-bevy` and its dependents went through the new wrapper, and the tests found Bevy's dylib.

- [ ] **Step 6: Commit.**
`git commit -am "slot spawns its command with posix_spawn, since a forked perl hung in macOS's fork handlers"`

### Task 3: Keep a Changelog, and no `release-notes.sh`

**Files:**
- Modify: `CHANGELOG.md` (header, every version heading, `Unreleased`, link references at the end)
- Delete: `scripts/release-notes.sh`
- Modify: `.github/workflows/release.yml:111-127`, `docs/RELEASING.md:14`, `docs/OVERVIEW.md:339`, `docs/PLAN.md:209`

- [ ] **Step 1: Rewrite the changelog's head and headings.**
The top is Keep a Changelog's preamble, adapted: the title, one line that the format is Keep a Changelog 1.1.0 and the project follows Semantic Versioning, and one line that a game depends on a release by its git tag.
Headings: `## [Unreleased]`, `## [0.4.0] - 2026-10-24`, `## [0.3.0] - 2026-09-23`, `## [0.2.0] - 2026-09-16`, `## [0.1.0] - 2026-09-14`.
At the end, the link references:

```markdown
[Unreleased]: https://github.com/rudehn/rl-engine/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/rudehn/rl-engine/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/rudehn/rl-engine/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/rudehn/rl-engine/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/rudehn/rl-engine/releases/tag/v0.1.0
```

- [ ] **Step 1b: Rewrite each released section short.**
Read only each entry's first sentence, never the whole file: `awk '/^## /{print} /^- /{print substr($0, 1, 120)}' CHANGELOG.md` (about 500 lines), plus 0.4.0's summary above "Every change, newest first:".
Write each release as 10 to 25 lines under `### Added`, `### Changed`, `### Removed` (and `### Fixed` where it applies), one line per change, keeping what a game author would look for: new systems and plugins, breaking renames and moved paths, removals.
Small fixes, internal refactors and docs-only changes are left out.
Each section's last line is `Full detail: [CHANGELOG.md at v<x>](https://github.com/rudehn/rl-engine/blob/v<x>/CHANGELOG.md).`, since the old text lives on at its tag.
Expected: the file drops from 349 KB to about 10 KB, and `grep -c "" CHANGELOG.md` is under 150.

- [ ] **Step 2: Rewrite `Unreleased` in the format.**
Groups only as used, in the spec's order: `### Added`, `### Changed`, `### Deprecated`, `### Removed`, `### Fixed`, `### Security`.
One line per change; a breaking one starts `**Breaking:**` and says in the same line what a game changes.
The existing entry becomes one line under `### Changed`: Undermere and Foundry moved to their own repositories (linked), and a game changes nothing.
This branch adds no other line until Task 6.

- [ ] **Step 3: Replace the script in the release workflow.**
`git rm scripts/release-notes.sh`.
In `release.yml`, the first line of the `run:` block becomes:

```bash
          awk -v v="${GITHUB_REF_NAME#v}" '/^## \[/ { f = index($0, "## [" v "]") == 1; next } /^\[[^]]+\]: / { f = 0 } f' CHANGELOG.md | sed '/./,$!d' > notes.md
          test -s notes.md || { echo "CHANGELOG.md has no section for $GITHUB_REF_NAME" >&2; exit 1; }
```

The comment above the step says the page is the changelog's section, verbatim.

- [ ] **Step 4: Check the extraction.**
Run the awk from Step 3 locally with `v=0.4.0`, `v=0.1.0`, and `v=0.4.1`.
Expected: `0.4.0` prints its section from its first line to the line before `## [0.3.0]`; `0.1.0` stops before the link references; `0.4.1` prints nothing, so `test -s` fails.

- [ ] **Step 5: Fix the three docs that name the script.**
`docs/RELEASING.md:14` drops `scripts/release-notes.sh` from the list; `docs/OVERVIEW.md:339` and `docs/PLAN.md:209` say the release page is the tag's `CHANGELOG.md` section.

- [ ] **Step 6: Commit.**
`git commit -am "the changelog follows keep a changelog, and the release page is its section"`

### Task 4: Dead scripts

**Files:**
- Delete: `scripts/rename-to-atglyph.py`, `scripts/test_rename_to_atglyph.py`, `scripts/check-systems-style.sh`
- Modify: `.github/workflows/ci.yml:64-66` (the "system pages keep to the template" step)

- [ ] **Step 1: Find every caller.**
Run: `grep -rn "rename-to-atglyph\|test_rename_to_atglyph\|check-systems-style" --exclude-dir=target --exclude-dir=.git --exclude-dir=docs/superpowers --exclude-dir=docs/reviews --exclude-dir=.planning .`
Expected: only AGENTS.md (Task 5 owns it), `ci.yml`, and the files themselves.

- [ ] **Step 2: Delete the three scripts and the CI step.**

- [ ] **Step 3: Commit.**
`git commit -am "the one-time rename scripts and the system pages' prose budget go"`

### Task 5: AGENTS.md keeps the rules and drops the history

**Files:**
- Modify: `AGENTS.md` (whole file; `CLAUDE.md` is a symlink to it)

**Interfaces:**
- Consumes: the gate from Task 1, the changelog rule from Task 3, the deletions in Task 4.

- [ ] **Step 1: Rewrite it.**
Same sections in the same order.
Every rule stays; what goes is measurements, dates, and the story of how a rule came to be ("195 of 197", "until 2026-09-22", `noise.md` against `fields.md`, `registries.md is the page that found this`).
Target: under 100 lines, from 159.
What changes in substance:
- the release paragraph: docs are brought up to the code once, before a tag; a commit runs `scripts/gate.sh` and writes its changelog line; the five files and when each changes stay as a list of one line each;
- "Writing fast tests" becomes "Tests": one tier, `cargo test -p <crate> <filter>` while iterating, `scripts/gate.sh` before a commit and again after a rebase onto a `main` that moved; keep the four-kinds table and the cheapest-fix list in one line each; a test past two seconds is made cheaper; the budgets, profiles and `test-times.py` paragraphs go;
- new, in the build-enforced section: a `CHANGELOG.md` line follows Keep a Changelog, one line per change under its group in `Unreleased`, a breaking one marked `**Breaking:**` with what a game changes;
- new: a commit message is a subject line and at most a short body;
- "Writing a system page": drop the eighty-line budget and its history, and the `check-systems-style.sh` instruction; keep the six parts, the include rule and the manifest rule (one sentence for why the manifest lists every claimed file);
- the layout block drops `check-systems-style.sh`, `test-times.py`, and `slot`'s "records each test's processor time"; it keeps every `docs/design/*.md` name.

- [ ] **Step 2: Check the prose rules.**
Run: `grep -n "—" AGENTS.md; grep -nE "\. [A-Z]" AGENTS.md | head`
Expected: no em dash; any second sentence on one line is split.

- [ ] **Step 3: Run the overview check against the new layout section.**
Run: `scripts/check-overview.sh`
Expected: passes, so every design doc is still listed.

- [ ] **Step 4: Commit.**
`git commit -am "agents.md keeps its rules and drops their history"`

### Task 6: Remove `atglyph::bevy`

**Files:**
- Modify: `crates/atglyph/src/lib.rs:50,61,73`, `docs/OVERVIEW.md:332`, `CHANGELOG.md` (`Unreleased`), `README.md` (the note from `main`)

- [ ] **Step 1: Apply the README note.**
`git apply <scratchpad>/readme-note.patch`.

- [ ] **Step 2: Remove the re-export and its doc.**
Delete `pub use bevy;` at `crates/atglyph/src/lib.rs:73`.
The module doc at line 50 names only `atglyph::rand`, and says a game lists `bevy` itself because Bevy's derive macros find it by reading the game's `Cargo.toml`.
The doc-test line at 61 becomes `let _app = bevy::app::App::new();`, which compiles since doc-tests see the crate's own dependencies.
`docs/OVERVIEW.md:332` drops `atglyph::bevy`.

- [ ] **Step 3: Check nothing else names it.**
Run: `grep -rn "atglyph::bevy" crates examples docs/guide README.md`
Expected: no output.
Run: `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
Expected: passes, so no doc link pointed at the re-export.

- [ ] **Step 4: The changelog line.**
Under `### Removed` in `Unreleased`: `- **Breaking:** \`atglyph::bevy\`; name \`bevy::\` from the game's own \`bevy = "0.19"\` dependency, which Bevy's derive macros need anyway.`

- [ ] **Step 5: Gate and commit.**
Run: `scripts/gate.sh`, expected `gate: passed`.
`git commit -am "atglyph no longer re-exports bevy, which a game depends on directly"`

### Task 7: Merge

- [ ] **Step 1: Read what landed on `main` since the branch left it.**
Run: `git log --oneline $(git merge-base main HEAD)..main`
If empty, skip to Step 3.

- [ ] **Step 2: Rebase and gate again.**
`git rebase main && scripts/gate.sh`, expected `gate: passed`.

- [ ] **Step 3: Fast-forward `main` and clean up.**
From the main checkout: `git merge --ff-only lean-process`, then `git worktree remove .claude/worktrees/lean-process && git branch -d lean-process`.
Do not push unless Nate asks.

- [ ] **Step 4: Update the memories that describe the old process.**
In `~/.claude/projects/-Users-nathanrude-Development-rl-engine/memory/`: rewrite `test-runs.md` (one tier, `cargo test`, one gate) and `build-slots.md` (slots wrap compiles only, Python `posix_spawn`, no nextest), and update their lines in `MEMORY.md`.
