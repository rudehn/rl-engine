# Testing Patterns

**Analysis Date:** 2026-09-17

## Test Framework

**Runner:**
- Standard `cargo test`, no custom test harness.
- No `proptest` or `quickcheck` dependency anywhere in the workspace (`Cargo.toml`, `crates/*/Cargo.toml` all checked) - "property-over-seed-range" testing is done by hand: a loop over a small fixed range of seeds, not a property-testing crate.

**Assertion Library:**
- Built-in `assert!`, `assert_eq!`, `assert_ne!` only. No `pretty_assertions` or similar.
- Assertion messages are written as sentences explaining what failed and why it matters, not just restating the values: `assert!(hurt > 0, "something was struck, so the fingerprint is of a fight and not of a frozen field");` (`crates/rl-bevy/tests/fingerprint.rs:102`).

**Run Commands:**
```bash
cargo test --workspace                 # everything: unit, integration, doc-tests
cargo test --workspace --no-run        # compile-only check, fast feedback
cargo test -p rl-grid                  # one crate
cargo doc --workspace --no-deps        # compiles and runs doc-tests as part of doc generation
cargo bench --workspace --no-run       # benches compile (CI does not run them)
```
- No coverage tool is configured (no `cargo-tarpaulin`/`cargo-llvm-cov` config found).

## Test File Organization

**Location:**
- Overwhelmingly co-located: `#[cfg(test)] mod tests { ... }` at the bottom of the source file it tests. This is true in every crate checked (`crates/rl-core/src/turn.rs`, `crates/rl-core/src/seed.rs`, `crates/rl-grid/src/dijkstra.rs`, `crates/rl-grid/src/fov.rs`, etc.).
- One crate has a top-level `tests/` integration directory: `crates/rl-bevy/tests/`, containing:
  - `crates/rl-bevy/tests/fingerprint.rs` - the fingerprint tripwire (see below).
  - `crates/rl-bevy/tests/genres.rs` - a cross-cutting content-loading test that spans five RON asset sets.
  - `crates/rl-bevy/tests/genres/*.ron` - the five fixture files `genres.rs` loads (`fantasy.ron`, `pirates.ron`, `scifi.ron`, `medieval.ron`, `crime.ron`).
- These live in `tests/` rather than co-located because they exercise the whole `App` end-to-end across multiple plugins, not one module's internals.

**Naming:**
- Test functions read as a full sentence describing the property being verified, never `test_foo`:
  - `astar_and_dijkstra_agree_on_every_reachable_cost` (`crates/rl-grid/src/dijkstra.rs:322`)
  - `the_surface_stands_a_player_on_open_ground` (`crates/rl-bevy/src/testing.rs:221`)
  - `a_scripted_key_is_just_pressed_for_one_frame` (`crates/rl-bevy/src/testing.rs:240`)
  - `a_seeded_run_of_minds_combat_and_stealth_comes_to_the_same_run_every_time` (`crates/rl-bevy/tests/fingerprint.rs:109`)
- Each carries a `///` doc comment one line above restating the property in plain language, so the test doubles as documentation of the invariant.

**Structure:**
```
crates/<name>/src/<module>.rs
  ...implementation...
  #[cfg(test)]
  mod tests {
      use super::*;
      #[test]
      fn a_sentence_describing_the_property() { ... }
  }

crates/rl-bevy/tests/
  fingerprint.rs
  genres.rs
  genres/
    fantasy.ron
    pirates.ron
    scifi.ron
    medieval.ron
    crime.ron
```

## Test Counts (approximate, by `#[test]` occurrence, HEAD `3c44b47`)

| Crate | `#[test]` count |
|---|---|
| `rl-bevy` | 99 |
| `rl-core` | 85 |
| `rl-ui` | 134 |
| `rl-rules` | 84 |
| `rl-grid` | 54 |
| `rl-world` | 35 |
| `rl-render` | 25 |
| `rl-mapgen` | 17 |
| `rl-save` | 14 |
| `rl-overworld` | 1 |
| `rl-engine` | 0 |

`cargo test --workspace --no-run` was run during this analysis and completed cleanly: every crate's unit tests, `rl-bevy`'s two integration test binaries (`fingerprint`, `genres`), and every example's (`corsair`, `delve`, `heist`, `tutorial`) unit tests and tutorial-step binaries compiled without error.
The full suite was not executed (long-running; per instruction, only compiled).

## Doc-Tests

- `#![deny(missing_docs)]` on every crate forces every public item to carry a doc comment; where a doc comment includes a fenced ` ```rust ` block, it compiles and runs as a doc-test.
- CI runs `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`, which fails the build on any doc-test failure or missing-docs warning (`.github/workflows/ci.yml`, step "doc tests and missing docs").
- No doc-test in the workspace is fenced ` ```rust,ignore ` or ` ```ignore ` (grep across all crates returns zero matches) - `CLAUDE.md`'s rule "never fence an example as `ignore`" has no exceptions in the current tree.

## Property-Over-Seed-Range Tests

No property-testing crate is used; the pattern is a hand-rolled loop over a small integer range of seeds, each iteration building a fresh `StdRng::seed_from_u64(seed)` and asserting an invariant holds for that seed.

```rust
// crates/rl-grid/src/dijkstra.rs:322-340 (abbreviated)
#[test]
fn astar_and_dijkstra_agree_on_every_reachable_cost() {
    let registry = crate::tile::TileRegistry::standard();
    let mut astar = AStar::new();
    for seed in 0..10u64 {
        let mut rng = StdRng::seed_from_u64(seed);
        let terrain = crate::terrain::Terrain::from_fn(20, 15, |_| { /* random walls */ });
        // ...build a Dijkstra map and an A* search over the same terrain...
        for (start, _) in terrain.iter() {
            let found = astar.find(&view, start, goal, PathRules::default()).map(|p| p.cost as i32);
            assert_eq!(found, map.value(start), "seed {seed} start {start:?}");
        }
    }
}
```

Other occurrences of this pattern: `crates/rl-grid/src/fov.rs:224` (`for seed in 0..12u64`), `crates/rl-world/src/hydrology.rs:199,222,264` (`for seed in 1..=4` / `1..=6`), `crates/rl-world/src/graph.rs:407` (`for seed in 1..=3`), `crates/rl-world/src/roads.rs:418`, `crates/rl-world/src/elevation.rs:246`, `crates/rl-rules/src/fire.rs:130,142` (`for seed in 0..16u64` / `0..32u64`), `crates/rl-rules/src/gas.rs:259` (`for seed in 0..32u64`).

**When to use this pattern:** whenever a property should hold for any seed (an invariant of an algorithm, not a specific scenario) - loop a small range (typically 10-32) and assert the property per iteration, including the seed in the failure message so a broken seed is reproducible directly.

## Fingerprint Tripwires

A fingerprint tripwire is distinct from a property test: it pins a specific numeric outcome of a long, multi-system run and fails on any change to that number, whether or not the change is a bug.
It exists to catch silent reordering of rolls/decisions/queries that a property test would not notice.

`crates/rl-bevy/tests/fingerprint.rs` is the canonical (and currently only) example:

- Its module doc (lines 1-10) explicitly labels itself: "A fingerprint tripwire, not a property... That is the point."
- It documents the re-baseline procedure inline: a deliberate change re-baselines the number and says so in the changelog, since existing replay recordings will not survive the change.
- The hash is a hand-rolled FNV-1a fold (`fold`, `crates/rl-bevy/tests/fingerprint.rs:23-28`) over the turn clock and every actor's spawn-order rank, position and health - explicitly *not* keyed by entity index, because Bevy's entity indices shift when an unrelated system is added (`crates/rl-bevy/tests/fingerprint.rs:33-35`).
- The test asserts three things: same seed run twice gives the same fingerprint; a different seed gives a different fingerprint; and the known-good seed's fingerprint equals a hardcoded literal (`8_778_430_806_092_112_307`, `crates/rl-bevy/tests/fingerprint.rs:113-116`).

**When to add a new one:** for any full-loop scenario (minds + combat + stealth + items, or a new equivalent) where the property "the same seed and the same keys reproduce the same run" is the invariant that matters and no smaller property test would catch a reordering bug.
Label it in the module doc as a fingerprint tripwire, not a property, and state the re-baseline procedure.

## `rl_bevy::testing` - Shared Test Scaffolding

`crates/rl-bevy/src/testing.rs` is a `pub` module (not `#[cfg(test)]`-gated at the module level, so a game's own tests and other engine crates' tests can use it) providing the common pieces every headless-`App` test needs:

- `TEST_SEED: RunSeed = RunSeed(5)` (line 31) - the fixed seed every test world and stream builds from, so a failing test fails the same way twice.
- `TestWorld` (lines 40-90) - implements `WorldRules` and `ChunkRules` over a small deterministic land/sea layout: land is floor throughout so sight and movement are unobstructed, sea is wall so there is something to bump into.
- `surface(app) -> Point` / `surface_with(app, tiles) -> Point` (lines 99-111) - generates a `TestWorld`, inserts `WorldMap`/`WorldRes`/`ChunkRulesRes`, and returns a start point eight tiles into the first land region (guaranteed seven tiles of land in every direction).
- `two_sides(app) -> Sides` (lines 128-138) - registers two mutually hostile factions, one armored damage kind, and the run `Seed` resource, for any test involving combat.
- `KeyScriptPlugin` / `KeyScript` / `press(app, key)` (lines 147-209) - simulates keyboard input frame-accurately: Bevy clears `just_pressed` at the top of every frame, so a key set from outside the schedule needs a system in `PreUpdate` after `InputSystems` to be seen as "just pressed" by game logic in `Update`.

The module doc (lines 1-9) states its own rationale: nine duplicated test-world setups and three duplicated key-players were consolidated into this one shared module because each test module used to build its own and a setup change meant editing all of them.

**Convention for new engine tests that need a running `App`:** use `rl_bevy::plugin::headless_app()` plus `rl_bevy::testing::{surface, two_sides, TEST_SEED}` rather than assembling world/faction/seed state by hand.

## Mocking

No mocking framework is used or needed; the ECS/plugin architecture means a test builds a real (headless) `App` with only the plugins and resources the scenario requires, rather than mocking a dependency.
"What to mock" in the traditional sense does not apply here - the closest equivalent is choosing which plugins to add to a `headless_app()` for a given test.

## Fixtures and Content

- RON fixture files back both real games (`examples/*/assets/*.ron`) and a dedicated cross-cutting test (`crates/rl-bevy/tests/genres/*.ron`).
- Every RON fixture opens with a comment enumerating its full option space (see `CONVENTIONS.md`), which doubles as the schema documentation a test's fixture author needs.
- `crates/rl-bevy/tests/genres.rs` loads five genre-specific RON files through one loader into one set of registries, proving the engine's effect/ability machinery is genre-agnostic (see its module doc, `crates/rl-bevy/tests/genres.rs:1-10`).

## Benches

**Framework:** Criterion (`criterion = { version = "0.5", features = ["html_reports"] }` in the root `Cargo.toml`).

**Location:** `crates/rl-grid/benches/grid.rs` is the only bench file in the workspace currently.

- Module doc states its scope directly: "Benchmarks on the paths that actually cost something: FOV, A*, Dijkstra maps, region labelling and lighting, on maps shaped like the ones a game produces" (`crates/rl-grid/benches/grid.rs:1-3`).
- Test map generation uses a seeded `StdRng` (`cave(width, height, seed)`, lines 11-27) so benches are reproducible run to run.
- CI compiles but does not execute benches: `cargo bench --workspace --no-run` (`.github/workflows/ci.yml`, step "benches compile") - benches are a correctness-of-compilation gate in CI, and are run manually/locally for actual performance numbers.
- `CLAUDE.md`'s guidance (`docs/PLAN.md`/layout table) restricts benches to hot paths only, on realistic maps - consistent with the single bench file covering exactly the spatial/pathfinding/lighting hot paths.

## CI Jobs

Three workflows in `.github/workflows/`:

**`ci.yml`** (push to `main`, all PRs) - one job, `check`, running in order:
1. `cargo fmt --all --check` - formatting.
2. `scripts/check-tiers.sh` - tier boundary and Bevy-isolation check.
3. `scripts/check-guide.sh` - the mdBook guide's links/snippets/images resolve.
4. `cargo clippy --workspace --all-targets -- -D warnings` - lint, zero warnings tolerated.
5. `cargo test --workspace` - the full test suite (unit + integration + co-located).
6. `scripts/check-template.sh` - the starter template (under `templates/`, excluded from the main workspace) generates and builds standalone.
7. `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` - doc-tests run, missing-docs enforced.
8. `scripts/check-tiers.sh --wasm` - tiers 0 and 1, plus `rl-save`, build for `wasm32-unknown-unknown`.
9. `cargo bench --workspace --no-run` - benches compile.

Debug info is disabled for the whole job (`CARGO_PROFILE_{DEV,TEST,BENCH}_DEBUG: "0"`) specifically because building the workspace four different ways (native test, template, wasm, bench) exhausted the runner's disk otherwise (comment in `ci.yml`).

**`pages.yml`** (push to `main`, manual dispatch) - builds the mdBook guide plus wasm demo bundles (`scripts/build-demos.sh`) and publishes to GitHub Pages if a Pages site exists for the repo; otherwise logs a notice and exits cleanly rather than failing.

**`release.yml`** (tag push `v*`) - verifies the pushed tag matches `[workspace.package] version` in the root `Cargo.toml`, then publishes/edits a GitHub release page from `scripts/release-notes.sh`, sourced from `CHANGELOG.md`.

## `scripts/check-tiers.sh`

Reads `[package.metadata.rl-engine] tier` for every workspace member via `cargo metadata` (never from a hardcoded list in the script itself, so a new or renamed crate cannot silently skip the check).

Two modes:
- Default: every member declares a tier (0-3); no crate depends on a crate of a strictly higher tier (normal and build dependency edges only, not dev-dependencies); every tier-0/1 crate's dependency tree (`cargo tree`, all targets, native and build deps) is free of `bevy`.
- `--wasm`: `cargo check` every tier-0/1 crate plus `rl-save` (tier 2, but its browser-storage/unload-bridge code compiles only under `wasm32-unknown-unknown`) against the wasm target, so nothing there is caught only by a native build.

Tiers: 0 = core, 1 = no Bevy, 2 = the Bevy layer, 3 = the facade and games.

## `scripts/check-guide.sh`

Checks the mdBook guide's internal consistency without a full `mdbook build`:
- Every chapter's embedded code is expanded from the tutorial example crate (`scripts/expand-guide.py --check`), so a chapter's code is code that actually compiles.
- Every image link resolves to a file on disk.
- `SUMMARY.md` lists every chapter file and no chapter file is missing from it (bidirectional check).
- Every path of the form `docs/guide/src/*.md` referenced from `crates/`, `examples/`, `templates/`, `scripts/`, or top-level docs actually exists (catches a renamed chapter breaking a module-comment cross-reference).
- Every inter-chapter markdown link resolves.
- Every hand-typed Rust snippet line inside a fenced ` ```rust ` block (not using `{{#include}}`) must literally appear somewhere in `examples/**/*.rs`, unless it contains `...` (an intentional elision) - this catches a chapter quoting an API that has since changed shape.

This check runs as its own CI step (`ci.yml`, "the guide's links into the code"), separate from the `pages.yml` build/publish job.

## Common Patterns

**Headless app construction (async/system testing):**
```rust
let mut app = rl_bevy::plugin::headless_app();
app.add_plugins((FovPlugin, CombatPlugin, MindsPlugin, StealthPlugin, ItemsPlugin, StreamingPlugin));
let start = rl_bevy::testing::surface(&mut app);
app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
app.update();
```
(`crates/rl-bevy/tests/fingerprint.rs:55-76`)

**Scripted keyboard input:**
```rust
app.add_plugins((MinimalPlugins, KeyScriptPlugin));
rl_bevy::testing::press(&mut app, KeyCode::Space);
app.update();
```
(`crates/rl-bevy/src/testing.rs:248-252`)

**Seeded, reproducible terrain for a test or bench:**
```rust
let mut rng = StdRng::seed_from_u64(seed);
let terrain = Terrain::from_fn(width, height, |p| {
    if rng.random_range(0..100) < 35 { wall } else { floor }
});
```
(pattern repeated in `crates/rl-grid/benches/grid.rs:11-27` and multiple `#[cfg(test)]` modules)

---

*Testing analysis: 2026-09-17*
