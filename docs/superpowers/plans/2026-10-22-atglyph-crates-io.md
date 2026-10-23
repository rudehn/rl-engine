# Publishing as `atglyph` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The engine is renamed `atglyph` and is ready to publish: pushing a `v*` tag would publish all ten crates to crates.io, and the starter template plays a dungeon, a player, goblins, combat, a potion and gear on its first run.

**Architecture:** A re-runnable Python script does the mechanical rename, proven by unchanged fingerprints, and the seed prefix moves in a commit of its own.
Then Bevy and `rand` are re-exported, each crate gets its publishing metadata and front page, CI packages every crate, the release workflow checks semver and publishes in dependency order, and the template gains items.

**Tech Stack:** Rust 2024, Bevy 0.19, Cargo workspace publishing (`cargo package --workspace`, cargo 1.90 or later), `cargo-semver-checks`, Python 3 for scripts, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-10-22-atglyph-crates-io-design.md`

## Global Constraints

- Crate names and facade paths, exactly:

  | Today | Crate | Directory | Facade path |
  |---|---|---|---|
  | `rl-engine` | `atglyph` | `crates/atglyph` | `atglyph` |
  | `rl-core` | `atglyph-core` | `crates/atglyph-core` | `atglyph::base` |
  | `rl-grid` | `atglyph-grid` | `crates/atglyph-grid` | `atglyph::grid` |
  | `rl-mapgen` | `atglyph-mapgen` | `crates/atglyph-mapgen` | `atglyph::mapgen` |
  | `rl-rules` | `atglyph-rules` | `crates/atglyph-rules` | `atglyph::rules` |
  | `rl-save` | `atglyph-save` | `crates/atglyph-save` | `atglyph::save` |
  | `rl-bevy` | `atglyph-bevy` | `crates/atglyph-bevy` | `atglyph::play` |
  | `rl-render` | `atglyph-render` | `crates/atglyph-render` | `atglyph::render` |
  | `rl-ui` | `atglyph-ui` | `crates/atglyph-ui` | `atglyph::ui` |
  | `rl-tty` | `atglyph-tty` | `crates/atglyph-tty` | `atglyph::tty` |

- Usable on their own: `atglyph-core`, `atglyph-grid`, `atglyph-rules`, `atglyph-mapgen`. Internal: `atglyph-bevy`, `atglyph-render`, `atglyph-ui`, `atglyph-save`, `atglyph-tty`.
- The repository URL `https://github.com/rudehn/rl-engine` and the guide's address `https://rudehn.github.io/rl-engine/` do not change in this project; Nate renames the repository at the release.
- History is not rewritten: `docs/superpowers/**`, `docs/reviews/**`, `.planning/**`, `docs/PLAN.md`, `examples/undermere/SOAK.md`, and `CHANGELOG.md` from its `## 0.3.0` heading down.
- The workspace version stays `0.3.0`; the release project bumps it to `0.4.0`.
- Every crate's versions move together; an internal dependency carries `version = "<workspace version>"`.
- A game lists `atglyph` and `bevy` (at the version `atglyph` re-exports, `0.19`), and no `rand`; `serde` and `ron` stay the game's own.
- Engine streams are named under `atglyph.` after Task 2.
- The template's new keys: `d` drinks a potion, `w` wears the first wearable thing in the bag; `g` and `,` pick up, through `PlayerKeysPlugin::pick_up`.
- Nothing is published, no repository is renamed and no tag is pushed by the builder.
- Every commit runs `scripts/gate.sh` and writes its `CHANGELOG.md` line under `Unreleased` with a `To change:` line; Tasks 1, 2 and the branch end run `scripts/gate.sh --all`.
- Prose uses the plain dash, never an em dash, one sentence per line in Markdown.

## Review Focus

1. A generated game whose `bevy` line drifts from the engine's Bevy builds two Bevys and fails with "expected `App`, found `App`": `check-template.sh` must refuse a template whose `bevy` version differs from the workspace's (Task 7).
2. A release whose publishing stops partway, from crates.io's rate limit on new crates or a network failure, must finish when the workflow is re-run, skipping what is already up (Task 6, `plan_publish` test).
3. A commit from another branch that lands old names after the rename merges: `rename-to-atglyph.py --check` in CI must name every leftover (Task 1).
4. The first release, when no crate has a published baseline, must not fail the semver check (Task 6, `semver_targets` test).
5. A crate that reads a file outside its own directory at compile time builds in the workspace and fails on crates.io: `cargo package --workspace` in CI must catch it (Task 4).

---

### Task 1: The mechanical rename, by a re-runnable script

**Files:**
- Create: `scripts/rename-to-atglyph.py`
- Create: `scripts/test_rename_to_atglyph.py`
- Modify: `scripts/check-systems.py` (a `--current` flag)
- Modify: `.github/workflows/ci.yml` (one step)
- Modify: `docs/PLAN.md` (one note line under its title, by hand)
- Modify: `docs/TODO.md` (the script's removal after the release)
- Modify: every living file the script rewrites, and `git mv` of every crate directory

**Interfaces:**
- Produces: `rewrite(text: str, facade_user: bool) -> str`, `classify(path: pathlib.Path) -> Literal["skip", "facade_user", "engine"]`, `SHORT: dict[str, str]` (crate suffix to facade module, from the table above), `rename_urls(text: str) -> str`.
- Produces: the CLI `python3 scripts/rename-to-atglyph.py` (rewrite and move), `--check` (exit 1 listing each leftover `path:line`), `--urls` (only the repository and guide URLs, for the release).
- Produces: `python3 scripts/check-systems.py --current`, printing the name of every system page whose fingerprint matches its files, one per line.

The rewrite rules, applied in this order:
1. `rl_engine::rl_<c>` becomes `atglyph::<SHORT[c]>`, everywhere.
2. In a facade-user file, a bare `\brl_<c>\b` becomes `atglyph::<SHORT[c]>`.
3. `\brl_engine\b` becomes `atglyph`.
4. `\brl_<c>\b` becomes `atglyph_<c>`.
5. `\brl-engine\b` becomes `atglyph`.
6. `\brl-<c>\b` becomes `atglyph-<c>`.
7. `\bRL_(FULL_SEEDS|RECORD|REPLAY|CAPTURE(_[A-Z]+)?)\b` becomes `ATGLYPH_\1`.
8. In `crates/atglyph/src/lib.rs` only, `pub use atglyph_<c>;` becomes `pub use atglyph_<c> as <SHORT[c]>;`.

The URLs `github.com/rudehn/rl-engine` and `rudehn.github.io/rl-engine` are masked before the rules and restored after; `--urls` rewrites only them, to `github.com/rudehn/atglyph` and `rudehn.github.io/atglyph`.
`<c>` is one of `core grid mapgen rules save bevy render ui tty`.

`classify` returns `skip` for `.git`, `target`, the history paths in Global Constraints, `Cargo.lock`, symbolic links (`CLAUDE.md` is one), and any file that is not UTF-8.
It returns `facade_user` for `examples/foundry`, `examples/undermere`, `examples/tutorial`, `templates/` and `docs/guide/`, and for `README.md`.
Everything else is `engine`, `examples/terminal` included, since it depends on the sub-crates directly.
`CHANGELOG.md` is rewritten only above its first `\n## 0.` heading.

- [ ] **Step 1: Write the failing tests** in `scripts/test_rename_to_atglyph.py`, with `unittest`:

```python
def test_a_facade_path_takes_its_short_name(self):
    self.assertEqual(rewrite("use rl_engine::rl_bevy::TurnSet;", facade_user=True), "use atglyph::play::TurnSet;")
    self.assertEqual(rewrite("rl_engine::rl_core::Point", facade_user=False), "atglyph::base::Point")

def test_a_bare_crate_path_in_a_game_goes_through_the_facade(self):
    self.assertEqual(rewrite("`rl_mapgen::Dress`", facade_user=True), "`atglyph::mapgen::Dress`")

def test_a_bare_crate_path_in_the_engine_keeps_its_crate(self):
    self.assertEqual(rewrite("use rl_rules::Registry;", facade_user=False), "use atglyph_rules::Registry;")

def test_crate_names_paths_and_the_metadata_key_move(self):
    self.assertEqual(rewrite("[package.metadata.rl-engine]", facade_user=False), "[package.metadata.atglyph]")
    self.assertEqual(rewrite("crates/rl-bevy/src/noise.rs", facade_user=True), "crates/atglyph-bevy/src/noise.rs")
    self.assertEqual(rewrite('rl-engine.workspace = true', facade_user=True), 'atglyph.workspace = true')

def test_the_repository_and_guide_urls_stay_until_the_release(self):
    text = "https://github.com/rudehn/rl-engine/blob/main/x and https://rudehn.github.io/rl-engine/"
    self.assertEqual(rewrite(text, facade_user=True), text)
    self.assertEqual(rename_urls(text), "https://github.com/rudehn/atglyph/blob/main/x and https://rudehn.github.io/atglyph/")

def test_seed_domains_and_other_words_are_left_alone(self):
    for text in ['b"rl.loot"', '("rl.combat", COMBAT)', "ctrl_ui", "rl_ui_layout", "curl-core", "rl-test-times.tsv"]:
        self.assertEqual(rewrite(text, facade_user=False), text)

def test_environment_variables_move(self):
    self.assertEqual(rewrite("RL_FULL_SEEDS=1", facade_user=False), "ATGLYPH_FULL_SEEDS=1")
    self.assertEqual(rewrite("RL_CAPTURE_AT", facade_user=False), "ATGLYPH_CAPTURE_AT")

def test_a_rewrite_is_idempotent(self):
    once = rewrite("use rl_engine::rl_ui::LogPanel; rl-core", facade_user=True)
    self.assertEqual(rewrite(once, facade_user=True), once)

def test_history_and_links_are_skipped(self):
    for path in ["docs/reviews/a.md", "docs/superpowers/plans/x.md", ".planning/y.md", "docs/PLAN.md", "examples/undermere/SOAK.md", "Cargo.lock"]:
        self.assertEqual(classify(pathlib.Path(path)), "skip")
    self.assertEqual(classify(pathlib.Path("examples/terminal/src/main.rs")), "engine")
    self.assertEqual(classify(pathlib.Path("docs/guide/src/systems/noise.md")), "facade_user")
```

- [ ] **Step 2: Run them to see them fail**

Run: `python3 -m unittest scripts/test_rename_to_atglyph.py -v`
Expected: FAIL, `rename-to-atglyph` cannot be imported (load it with `importlib` from the hyphenated path).

- [ ] **Step 3: Implement `scripts/rename-to-atglyph.py`** with the interfaces and rules above.
The CLI walks `git ls-files` so untracked build output is never touched, rewrites each file by its class, then `git mv`s each `crates/rl-<c>` to `crates/atglyph-<c>` and `crates/rl-engine` to `crates/atglyph` when the old directory exists, and finally runs `cargo update --workspace` to rename the lockfile's entries without upgrading anything.
A top-of-file comment says what it is for, that it is run again after every rebase until `0.4.0` is released, and that `--urls` is the release's step.

- [ ] **Step 4: Run the tests to see them pass**

Run: `python3 -m unittest scripts/test_rename_to_atglyph.py -v`
Expected: PASS, all nine.

- [ ] **Step 5: Add `--current` to `scripts/check-systems.py`**, printing each page whose stored fingerprint equals `fingerprint(files)`.
Run: `python3 scripts/check-systems.py --current > "$TMPDIR/current-pages"` and keep the file for Step 7.

- [ ] **Step 6: Run the rename and read the result**

Run: `python3 scripts/rename-to-atglyph.py && python3 scripts/rename-to-atglyph.py --check && git status --short | wc -l`
Expected: `--check` prints nothing and exits 0; several hundred paths changed.
Add one line under `docs/PLAN.md`'s title by hand: "The engine was renamed `atglyph` at 0.4.0; this plan, written before, uses its old name."
Read `crates/atglyph/src/lib.rs`: the sub-crates are `pub use atglyph_bevy as play;` and so on, and the prelude still globs each sub-crate's prelude.

- [ ] **Step 7: Re-confirm the pages that were current**

Run: `while read -r page; do python3 scripts/check-systems.py --bless "$page"; done < "$TMPDIR/current-pages"`
Expected: each blessed page passes again; a page that was stale before stays stale, for the release's documentation pass.

- [ ] **Step 8: Prove it changed no play**

Run: `scripts/gate.sh --all && scripts/check-tiers.sh && scripts/check-tiers.sh --wasm && scripts/check-template.sh && scripts/check-overview.sh && scripts/check-guide.sh`
Expected: all pass, with no fingerprint test edited. A fingerprint that moves means the script rewrote something it should not have; fix the rule, never the fingerprint.

- [ ] **Step 9: Run it again to prove it is re-runnable**

Run: `python3 scripts/rename-to-atglyph.py && git status --short`
Expected: no change beyond Step 6's.

- [ ] **Step 10: Guard against leftovers**

Add a CI step after `check-tiers.sh` in `.github/workflows/ci.yml`: `python3 scripts/rename-to-atglyph.py --check`.
Add to `docs/TODO.md`: remove `scripts/rename-to-atglyph.py`, its test and its CI step after the `0.4.0` release has run `--urls`.

- [ ] **Step 11: Commit**

CHANGELOG line: the engine is `atglyph`; `To change:` the dependency `rl-engine = ...` becomes `atglyph = ...`, `rl_engine::` becomes `atglyph::`, `rl_engine::rl_<c>::` becomes `atglyph::<short>::` with the table of short names, and `RL_RECORD`, `RL_REPLAY` and `RL_CAPTURE*` become `ATGLYPH_*`.

```bash
git add -A
git commit -m "the engine is atglyph: every crate, path, metadata key and environment variable renamed by scripts/rename-to-atglyph.py, re-runnable after a rebase, with the sub-crates under short facade names (atglyph::base, atglyph::play, atglyph::grid and the rest), history left as written, and every fingerprint unchanged"
```

On every later rebase onto `main` before merging: rebase, take `main`'s side of any conflict in a renamed file, run the script again, re-run Step 8.

---

### Task 2: Engine streams are named under `atglyph.`

**Files:**
- Modify: `crates/atglyph-core/src/seed.rs` (`ENGINE_PREFIX`, its assertion message, its doc examples and tests)
- Modify: `crates/atglyph-bevy/src/seed.rs` (every stream name in the table at about line 109)
- Modify: every other `"rl.` or `b"rl.` literal in `crates/` and `examples/` that names an engine domain (`grep -rn '"rl\.' crates examples`)
- Modify: `crates/atglyph-bevy/tests/fingerprint.rs`, `examples/foundry/tests/fingerprint.rs`, `examples/undermere/tests/fingerprint.rs`, the effect stream's tripwire in `crates/atglyph-bevy/src/effects/mod.rs`
- Modify: every test pinned to a seed that no longer shows its case

**Interfaces:**
- Produces: `ENGINE_PREFIX: &[u8] = b"atglyph."`; a game's domain may not start with `atglyph.`.

- [ ] **Step 1: Change the test first**: in `crates/atglyph-core/src/seed.rs`, the existing assertion becomes `assert_eq!(SeedDomain::engine(b"place"), SeedDomain(fnv1a(b"atglyph.place") | 1));`, and the reserved-prefix test uses `b"atglyph.loot"`.

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p atglyph-core seed`
Expected: FAIL on the `engine` domain assertion.

- [ ] **Step 3: Set `ENGINE_PREFIX = b"atglyph."`**, rename every engine stream from `rl.<name>` to `atglyph.<name>`, and say `atglyph.` in the assertion message and doc examples.

- [ ] **Step 4: Run it to see it pass**

Run: `cargo test -p atglyph-core seed`
Expected: PASS.

- [ ] **Step 5: Re-baseline once**

Run: `scripts/gate.sh --all`
Expected: the four tripwires fail with their new numbers, and some seed-pinned tests lose their case.
Write each tripwire's new number; for each seed-pinned test, take the first seed that shows its case again, as commit `c2043536` did; never weaken an assertion.
Run `scripts/gate.sh --all` again until it passes.

- [ ] **Step 6: Commit**

CHANGELOG line: engine streams are named under `atglyph.`, so every seed grows and plays differently and a recording made on 0.3 replays differently; `To change:` re-baseline a game's fingerprints and seed-pinned tests, and a game domain starting `atglyph.` is refused.

```bash
git add -A
git commit -m "engine streams are named under atglyph., the rename's one re-baseline: <each tripwire and why it moved, and each re-picked seed>"
```

---

### Task 3: `atglyph` re-exports Bevy and `rand`

**Files:**
- Modify: `crates/atglyph/src/lib.rs` (two re-exports, a crate-docs paragraph)
- Modify: `examples/foundry/Cargo.toml`, `examples/undermere/Cargo.toml`, `examples/tutorial/Cargo.toml` (drop `rand`)
- Modify: every `use rand` and `rand::` in those games and `templates/starter/src/main.rs`
- Modify: `templates/starter/Cargo.toml.liquid` (drop `rand`, comment the `bevy` line)

**Interfaces:**
- Produces: `atglyph::bevy` (Bevy 0.19) and `atglyph::rand` (rand 0.9, with `std` and `std_rng`).

- [ ] **Step 1: Write the failing doc-test** at the top of `crates/atglyph/src/lib.rs`'s docs, which compiles only when both re-exports exist:

```rust
use atglyph::rand::{Rng, SeedableRng, rngs::StdRng};
let mut rng = StdRng::seed_from_u64(7);
let roll: i32 = rng.random_range(1..=6);
assert!((1..=6).contains(&roll));
let _app = atglyph::bevy::app::App::new();
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p atglyph --doc`
Expected: FAIL, no `rand` or `bevy` in `atglyph`.

- [ ] **Step 3: Add `pub use bevy;` and `pub use rand;`** beside the sub-crate re-exports, and a paragraph in the crate docs saying a game still lists `bevy` at the same version, because Bevy's derive macros look for it in the game's own Cargo.toml, and lists no `rand`.

- [ ] **Step 4: Move the games and the template onto `atglyph::rand`**: drop each `rand` dependency line, and write `use atglyph::rand::...` wherever `use rand::...` was.
The template's `bevy` line gains the comment: "Must be the Bevy `atglyph` re-exports, or the game builds two Bevys whose types do not match."

- [ ] **Step 5: Run the gate**

Run: `cargo test -p atglyph --doc && scripts/gate.sh && scripts/check-template.sh`
Expected: PASS, and `cargo tree -p undermere -i rand@0.9` shows one `rand`.

- [ ] **Step 6: Commit**

CHANGELOG line: `atglyph::bevy` and `atglyph::rand`; `To change:` drop the game's `rand` line and write `atglyph::rand`; keep `bevy` at `0.19`.

```bash
git add -A
git commit -m "atglyph re-exports bevy and rand, the games and the template draw from atglyph::rand with no rand of their own, and a game keeps its bevy line at the same version since bevy's derives read it"
```

---

### Task 4: Every crate packages for crates.io

**Files:**
- Modify: `Cargo.toml` (`version` on each internal entry of `[workspace.dependencies]`)
- Modify: each `crates/*/Cargo.toml` (`readme`, `documentation`, and `[package.metadata.docs.rs]` where needed)
- Create: `crates/atglyph-<c>/README.md` for each of the nine sub-crates
- Create: `scripts/check-publish.sh`
- Modify: `.github/workflows/ci.yml` (one step)

**Interfaces:**
- Produces: `scripts/check-publish.sh`, which exits 1 naming each problem, and 0 after `cargo package --workspace` succeeds.

`check-publish.sh` checks, from `cargo metadata`, for every member that is not `publish = false`: `description`, `license`, `repository`, `readme`, `keywords` and `categories` are set; the `readme` file exists; and every dependency on another workspace member carries a version requirement equal to the workspace version.
It then runs `cargo package --workspace`, which builds every crate from its packaged files alone, against a local registry for the crates not yet on crates.io.

- [ ] **Step 1: Write `scripts/check-publish.sh` and run it to see it fail**

Run: `scripts/check-publish.sh`
Expected: FAIL, naming nine crates with no `readme` and every internal dependency with no version.

- [ ] **Step 2: Add `version = "0.3.0"` to each internal entry** of `[workspace.dependencies]`, and a comment that the release's version bump changes them with the workspace version.

- [ ] **Step 3: Write the nine READMEs.**
A usable-on-its-own crate's README says what it is in one paragraph, that it needs no Bevy, and links to `atglyph` for the whole engine.
An internal crate's README is two sentences: it is part of `atglyph`, and a game depends on `atglyph` and reaches it as `atglyph::<short>`.
Each manifest gains `readme = "README.md"` and `documentation = "https://docs.rs/<crate>"`.

- [ ] **Step 4: Run it to see it pass**

Run: `scripts/check-publish.sh`
Expected: PASS, with ten `Packaged` lines and a verified build of each.
A crate that fails verification is fixed in the crate, never excluded.

- [ ] **Step 5: Add the CI step** `scripts/check-publish.sh` after `scripts/check-template.sh`.

- [ ] **Step 6: Commit**

CHANGELOG line: every crate packages for crates.io; `To change:` nothing.

```bash
git add -A
git commit -m "every crate packages for crates.io: internal dependencies carry the workspace version, each crate has its readme, and scripts/check-publish.sh packages and verifies the workspace in CI"
```

---

### Task 5: Each crate's front page

**Files:**
- Modify: `crates/atglyph/src/lib.rs` (crate docs open)
- Modify: `crates/atglyph-core/src/lib.rs`, `crates/atglyph-grid/src/lib.rs`, `crates/atglyph-rules/src/lib.rs`, `crates/atglyph-mapgen/src/lib.rs` (crate docs open, a Bevy-free example each)
- Modify: `crates/atglyph-bevy/src/lib.rs`, `crates/atglyph-render/src/lib.rs`, `crates/atglyph-ui/src/lib.rs`, `crates/atglyph-save/src/lib.rs`, `crates/atglyph-tty/src/lib.rs` (one opening paragraph)
- Modify: `README.md` (the template command first, then the demos)

- [ ] **Step 1: `atglyph`'s crate docs open with the template command** `cargo generate --git https://github.com/rudehn/rl-engine --tag v0.3.0 templates/starter --name my-game`, one sentence on what the generated game plays, and links to the guide (`https://rudehn.github.io/rl-engine/`) and to the four usable-on-their-own crates.
`README.md` opens the same way, and links the playable tutorial demos the guide already builds.

- [ ] **Step 2: Each usable-on-its-own crate's docs open** with what it is, that it builds with no Bevy and for wasm, and one doc-tested example using only that crate and its own dependencies:
  - `atglyph-core`: `RunSeed(7).rng(SeedDomain::new(b"demo"), 0)` as the stream, and a `DiceRoll` rolled on it, asserting the roll is in its range.
  - `atglyph-grid`: `fov` from a point on a small walled grid, asserting a cell behind a wall is unseen.
  - `atglyph-rules`: a `Registry` built with `Registry::from_defs` and looked up by name.
  - `atglyph-mapgen`: a `Chain` run on a `PlaceSeed`, asserting the map has a walkable cell.
  Each example's assertions are what it proves; they are doc-tests, so they run.

- [ ] **Step 3: Each internal crate's docs open** with "Part of `atglyph`: a game depends on `atglyph` and reaches this crate as `atglyph::<short>`."

- [ ] **Step 4: Run the doc-tests and the docs build**

Run: `cargo test --workspace --doc && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
Expected: PASS.

- [ ] **Step 5: Commit**

CHANGELOG line: none needed beyond Task 1's; `To change:` nothing.

```bash
git add -A
git commit -m "each crate's front page says whether to use it alone or through atglyph, the four bevy-free crates each open with an example that needs no bevy, and atglyph and the readme open with the template command"
```

---

### Task 6: The release workflow checks semver and publishes

**Files:**
- Create: `scripts/publish.py`
- Create: `scripts/test_publish.py`
- Modify: `.github/workflows/release.yml` (two jobs)
- Create: `docs/RELEASING.md`
- Modify: `docs/README.md` (link `RELEASING.md`)

**Interfaces:**
- Produces: `publish_order(metadata: dict) -> list[str]`, every publishable member in dependency order, ties broken by name.
- Produces: `plan_publish(order: list[str], version: str, published: dict[str, set[str]]) -> list[str]`, the crates in `order` whose `version` is not already in `published[crate]`.
- Produces: `semver_targets(order: list[str], published: dict[str, set[str]]) -> tuple[list[str], list[str]]`, the crates with a published version to check against, and those skipped for having none.
- Produces: the CLI `python3 scripts/publish.py --plan` (print the order and what would publish), `--semver` (run `cargo semver-checks -p <crate>` for each target, print a notice per skipped crate), `--publish` (publish each planned crate with `cargo publish -p <crate>`, and on crates.io's rate-limit response wait the time it names, or ten minutes, and retry that crate).
- `published` is read from `https://crates.io/api/v1/crates/<name>` with a User-Agent of `atglyph-release (https://github.com/rudehn/rl-engine)`; a 404 means no versions.

- [ ] **Step 1: Write the failing tests** in `scripts/test_publish.py`:

```python
def test_the_order_puts_a_crate_after_everything_it_depends_on(self):
    order = publish_order(METADATA)  # a made-up metadata of atglyph-core, atglyph-grid -> core, atglyph -> both, and one publish = false game
    self.assertEqual(order, ["atglyph-core", "atglyph-grid", "atglyph"])

def test_a_rerun_skips_what_is_already_published(self):
    order = ["atglyph-core", "atglyph-grid", "atglyph"]
    self.assertEqual(plan_publish(order, "0.4.0", {"atglyph-core": {"0.4.0"}, "atglyph-grid": {"0.3.9"}}), ["atglyph-grid", "atglyph"])

def test_the_first_release_has_nothing_to_check_semver_against(self):
    self.assertEqual(semver_targets(["atglyph-core", "atglyph"], {}), ([], ["atglyph-core", "atglyph"]))
    self.assertEqual(semver_targets(["atglyph-core", "atglyph"], {"atglyph-core": {"0.4.0"}}), (["atglyph-core"], ["atglyph"]))
```

- [ ] **Step 2: Run them to see them fail**

Run: `python3 -m unittest scripts/test_publish.py -v`
Expected: FAIL, no `publish` module.

- [ ] **Step 3: Implement `scripts/publish.py`** with the interfaces above, reading `cargo metadata --format-version 1 --no-deps`.

- [ ] **Step 4: Run them to see them pass, then the plan on the real workspace**

Run: `python3 -m unittest scripts/test_publish.py -v && python3 scripts/publish.py --plan`
Expected: PASS; the plan starts with `atglyph-core`, ends with `atglyph`, lists ten crates, and says all ten would publish.

- [ ] **Step 5: Add two jobs to `release.yml`**, both `needs: check`:
  - `semver`: installs `cargo-semver-checks` with `cargo install --locked cargo-semver-checks`, runs `python3 scripts/publish.py --semver`.
  - `publish`: `needs: [check, semver]`, `permissions: { contents: read, id-token: write }`; gets a token with `rust-lang/crates-io-auth-action@v1` when the repository secret `CARGO_REGISTRY_TOKEN` is empty, and uses the secret otherwise (crates.io trusts a publisher only for a crate that already exists, so the first release uses the secret); then runs `python3 scripts/publish.py --publish`.
  The `page` job gains `needs: publish`, so a release page never announces crates that are not up.

- [ ] **Step 6: Write `docs/RELEASING.md`**: the release, in order, with each step that needs Nate marked as his.
  1. The documentation pass, as `AGENTS.md` describes it.
  2. Bump the workspace version, the internal dependency versions and the template's pin together.
  3. Run `python3 scripts/publish.py --semver` and `scripts/check-publish.sh` by hand.
  4. Nate, the first release only: create the crates.io account, make an API token scoped to publishing new crates, and store it as the `CARGO_REGISTRY_TOKEN` secret.
  5. Nate: push the tag. A run that stops partway is re-run from the Actions page and publishes what is left.
  6. Nate, the first release only: add the GitHub workflow as each crate's trusted publisher on crates.io, then delete the secret.
  7. Nate, at `0.4.0`: rename the repository to `rudehn/atglyph` in GitHub's settings, then run `python3 scripts/rename-to-atglyph.py --urls`, commit, and let Pages republish.
  8. Check every crate's docs.rs build.
  9. Nate: the listings and announcements (Bevy Assets, arewegameyet.rs, RogueBasin, r/roguelikedev Sharing Saturday, r/rust, This Week in Rust).

- [ ] **Step 7: Check the workflow parses**

Run: `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/release.yml'))"` and, if `actionlint` is installed, `actionlint .github/workflows/release.yml`.
Expected: no errors.

- [ ] **Step 8: Commit**

CHANGELOG line: none, the release's own machinery; `To change:` nothing.

```bash
git add -A
git commit -m "a tag publishes every crate in dependency order after the semver check, a run cut short by the rate limit finishes on a re-run, and docs/RELEASING.md lists the release's steps and which are Nate's"
```

---

### Task 7: The template's first run has items, on a crates.io version

**Files:**
- Modify: `templates/starter/src/main.rs`
- Modify: `templates/starter/README.md`
- Modify: `templates/starter/Cargo.toml.liquid`
- Modify: `scripts/check-template.sh`
- Modify: `scripts/release-notes.sh` (the dependency line it prints)

**Interfaces:**
- Consumes: `ItemsPlugin`, `Item`, `Inventory`, `Equipped`, `Wearable`, `Armor`, `MeleeAttack`, `GearScore`, `Intent<UseItem>`, `Intent<Equip>`, `ItemEvent::Used`, `PlayerKeysPlugin::pick_up` and the registry's equipment slots, all through `atglyph::prelude`; Undermere and tutorial step 4 show each in use.
- Produces: in `main.rs`, `const POTIONS: usize = 3`, `const POTION_HEALS: i32 = 6`, one dagger (main hand, a `MeleeAttack` stronger than the player's fists) and one leather coat (body, `Armor(2)`) placed by `populate` on open cells; `struct Potion(i32)`; a `drink` system in `TurnSet::React` like the tutorial's `eat`.

- [ ] **Step 1: Write the failing tests** in the template's `mod tests`, using its `started(seed)` helper and setting state up rather than playing to it:

```rust
#[test]
fn drinking_a_potion_mends_your_wounds() {
    let (mut app, me) = started(7);
    // give the player a potion and take 10 health off
    press(&mut app, KeyCode::KeyD);
    // health is back up by POTION_HEALS, never past max, and the potion is gone from the bag
}

#[test]
fn a_coat_picked_up_and_worn_takes_its_share_of_each_blow() {
    let (mut app, me) = started(7);
    // lay the coat on the player's cell
    press(&mut app, KeyCode::KeyG);
    press(&mut app, KeyCode::KeyW);
    // the player's Equipped holds the coat, and a goblin's blow lands for less than with no coat
}
```

and extend `every_seed_builds_a_floor_you_can_stand_on` to assert each seed lays out `POTIONS` potions, one dagger and one coat, each on a walkable cell.

- [ ] **Step 2: Run them to see them fail**

Run: `scripts/check-template.sh`
Expected: FAIL, no `KeyD`, `KeyW` controls and no items on the floor.

- [ ] **Step 3: Add the items**: `ItemsPlugin` beside the template's plugins; the player spawned with an `Inventory` and `Equipped` over the slots the template registers (`main hand`, `body`); the keys `d` ("drink a potion") and `w` ("wear what you carry") declared with `add_control` under `Act`, so `?` lists them; `PlayerKeysPlugin::pick_up("Act", "pick up what is here")`; `populate` lays out the items; `drink` heals and despawns.
The constants sit with the others at the top of `main.rs`, each with a one-line comment as theirs have.

- [ ] **Step 4: Depend on a version**: `templates/starter/Cargo.toml.liquid` gets `atglyph = "0.3.0"` in place of the git dependency, with the comment "The release this template was written against; read the engine's CHANGELOG.md for what moved.", and `scripts/release-notes.sh` prints `atglyph = "$version"`.
`scripts/check-template.sh` checks the pin equals the workspace version in place of the tag, swaps `^atglyph = .*` for the path, and fails with "templates/starter pins bevy <x>, but the engine is on bevy <y>" when the template's `bevy` version differs from `[workspace.dependencies]`'s.

- [ ] **Step 5: Run them to see them pass**

Run: `scripts/check-template.sh`
Expected: PASS, the new tests among the template's own.
Then set the template's `bevy` to `"0.18"` and run it again: expected FAIL with the Bevy message; set it back.

- [ ] **Step 6: Play it** with the template rendered as `check-template.sh` renders it: `cargo run -- --seed 7` in that directory.
Find a potion and the coat, pick both up, drink while hurt, wear the coat, and check the log narrates each and the `?` screen lists `d` and `w`.
Anything that looks off on screen is fixed here.

- [ ] **Step 7: Update `templates/starter/README.md`**: an "Items" bullet in "What is here", `g`, `d` and `w` in the keys table, and "Where to go next" linking chapters of the published guide (`https://rudehn.github.io/rl-engine/`) rather than its source folder.

- [ ] **Step 8: Commit**

CHANGELOG line: the template's first run has potions, a dagger and a coat, and depends on `atglyph` by version; `To change:` nothing for a game; a game made from the template may do the same.

```bash
git add -A
git commit -m "the template's first run has three potions to drink on d, a dagger and a coat to pick up and wear on w, depends on atglyph by crates.io version, and check-template refuses a bevy line that differs from the engine's"
```

---

### Branch end

- [ ] Rebase onto `main`, run `python3 scripts/rename-to-atglyph.py` again, and run `scripts/gate.sh --all`, `scripts/check-publish.sh`, `scripts/check-template.sh` and `python3 scripts/rename-to-atglyph.py --check`.
- [ ] Read what landed on `main` since the base and re-gate any combination with it.
- [ ] Merge, then remove the worktree and delete the branch.
