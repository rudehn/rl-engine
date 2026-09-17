# Coding Conventions

**Analysis Date:** 2026-09-17

## Enforcement Model

This codebase treats most conventions as build-enforced, not style-guide-suggested.
Two layers exist: rules the build enforces (CI fails without them) and rules only review enforces (no tool catches a violation, but the pattern is consistent everywhere checked).
`CLAUDE.md` is the canonical source; this document maps each rule to where it lives in code and notes where reality deviates.

## Naming Patterns

**Crates:**
- `rl-` prefix, kebab-case: `rl-core`, `rl-grid`, `rl-mapgen`, `rl-world`, `rl-rules`, `rl-save`, `rl-engine`, `rl-bevy`, `rl-render`, `rl-overworld`, `rl-ui`.
- Each declares `tier` under `[package.metadata.rl-engine]` in its `Cargo.toml`, e.g. `crates/rl-core/Cargo.toml:14`.

**Files:**
- One module per subsystem noun: `crates/rl-bevy/src/combat.rs`, `crates/rl-bevy/src/minds.rs`, `crates/rl-bevy/src/stealth.rs`, `crates/rl-bevy/src/ability.rs`.
- `lib.rs` in every crate carries the crate-level `//!` doc and re-exports; see `crates/rl-rules/src/lib.rs`.
- A `prelude` submodule re-exports the common surface, e.g. `crates/rl-core/src/lib.rs:55`, `crates/rl-bevy/src/lib.rs:129`.

**Functions:**
- `snake_case`, verb-first for actions (`derive`, `reschedule_at`, `scaled_cost`), noun-first for accessors (`now`, `set_now`).
- Test function names read as a sentence describing the property under test, not `test_x`: `astar_and_dijkstra_agree_on_every_reachable_cost` (`crates/rl-grid/src/dijkstra.rs:322`), `the_surface_stands_a_player_on_open_ground` (`crates/rl-bevy/src/testing.rs:221`), `a_seeded_run_of_minds_combat_and_stealth_comes_to_the_same_run_every_time` (`crates/rl-bevy/tests/fingerprint.rs:109`).

**Types:**
- `PascalCase` structs and enums, ids suffixed `Id`: `TileId`, `FactionId`, `DamageKindId`, `SaveId`.
- Error enums named `<Domain>Error`: `RegisterError` (`crates/rl-grid/src/tile.rs:196`), `EquipError` (`crates/rl-rules/src/equip.rs:77`), `SaveError` (`crates/rl-save/src/backend.rs:17`).
- Plugin types are named `<Subsystem>Plugin`: `FovPlugin`, `CombatPlugin`, `MindsPlugin`, `StreamingPlugin`, `KeyScriptPlugin` (`crates/rl-bevy/src/testing.rs:147`).

**Constants:**
- `SCREAMING_SNAKE_CASE`, and units are named where they are not obvious: `BASE_ACTION_COST` (`crates/rl-core/src/turn.rs:22`), `TEST_SEED` (`crates/rl-bevy/src/testing.rs:31`).

## Code Style

**Formatting:**
- `rustfmt.toml`: `max_width = 160`, `use_small_heuristics = "Max"`, `edition = "2024"`.
- Enforced with `cargo fmt --all --check` in CI (`.github/workflows/ci.yml`).
- The wide line width means single-expression function bodies and short `impl` blocks are frequently one line, e.g. `crates/rl-core/src/turn.rs:79` (`Self { entries: BinaryHeap::new(), next_insertion_order: 0, now: 0 }`).
- Never hand-wrap lines to fight the formatter; let `cargo fmt` own line breaks.

**Linting:**
- `cargo clippy --workspace --all-targets -- -D warnings`, enforced in CI.
- No crate-wide `#![allow(clippy::...)]` anywhere in the workspace (grep confirms zero occurrences) - a system with too many parameters is split rather than silenced, per `CLAUDE.md`.
- `#![forbid(unsafe_code)]` appears at crate root in pure-logic crates, e.g. `crates/rl-rules/src/lib.rs:35`.

**Doc coverage:**
- `#![deny(missing_docs)]` at the top of every crate's `lib.rs`: `rl-core`, `rl-grid`, `rl-mapgen`, `rl-world`, `rl-rules`, `rl-save`, `rl-engine`, `rl-bevy`, `rl-render`, `rl-overworld`, `rl-ui` all declare it.
- Doc-tests compile and run as part of `cargo doc --workspace --no-deps` with `RUSTDOCFLAGS="-D warnings"` in CI; grep across the workspace finds zero ` ```rust,ignore ` or ` ```ignore ` fences - no example is ever excused from compiling.

## Doc Comment Density

The reference density is `crates/rl-core/src/turn.rs`. Model new doc comments on it:

- Module doc (`//!`) states what the type is, why its invariant exists, and what it deliberately does not own. `crates/rl-core/src/turn.rs:1-16` explains the integer clock's determinism reasoning and explicitly hands the game loop's ownership to the Bevy layer, citing the previous engine's mistake.
- Every public item gets a one-line summary, followed by a paragraph only when there is a reason (a saturating vs. wrapping choice, a panic condition, a why-not).
- `# Panics` sections are used, not prose burial: `crates/rl-core/src/turn.rs:117-120` (`TurnQueue::set_now`).
- Comments justify a design decision against an alternative that was rejected, in one or two sentences - not paragraphs of history. Compare `crates/rl-core/src/seed.rs:1-16` (why `StdRng` and not `SmallRng`, why `position_hash`/`pair_hash` exist).
- Non-doc `//` comments on private items explain the "why", not the "what": `crates/rl-core/src/turn.rs:49-51` explains why `Ord` deliberately excludes the id field.
- Do not exceed this density. `CLAUDE.md` is explicit: "at the density of `rl-core/src/turn.rs`, not more."

## Randomness

**Rule:** every random draw traces back to `RunSeed`, never a bare constant or OS entropy, inside engine or game logic.

- `RunSeed` and `SeedDomain` live in `crates/rl-core/src/seed.rs`.
- `RunSeed::derive(domain, index)` (`crates/rl-core/src/seed.rs:53`) mixes seed, domain salt and index through `mix64`, so every domain/index pair is an independent stream.
- `RunSeed::stream(domain)` (`crates/rl-core/src/seed.rs:58`) is the run-scoped, index-free form.
- `RunSeed::rng(domain, index)` (`crates/rl-core/src/seed.rs:62`) hands back a seeded `StdRng` directly.
- `RunSeed::fresh()` (`crates/rl-core/src/seed.rs:82`) is the only place entropy enters, gated `#[cfg(not(target_arch = "wasm32"))]`, and it still funnels through `mix64`/`from_entropy` rather than seeding a generator directly from the clock.
- `StdRng` is used deliberately over `SmallRng`: the doc comment (`crates/rl-core/src/seed.rs:8-11`) states this is because `StdRng` behaves identically across platforms, which a browser/desktop replay requires.
- The Bevy layer wraps this in a `Stream` trait plus `AddStream`/`app.add_stream::<S>(plugin)` (`crates/rl-bevy/src/seed.rs:75-99`); a subsystem's own RNG resource (`CombatRng`, `MindRng`, `AbilityRng`, `StealthRng`) implements `Stream` and is re-derived whenever the run `Seed` resource changes (`crates/rl-bevy/src/combat.rs:201`, `crates/rl-bevy/src/minds.rs:427`, `crates/rl-bevy/src/ability.rs:252`, `crates/rl-bevy/src/stealth.rs:199`).
- Functions that draw take `&mut impl Rng` rather than a concrete type, e.g. tactic/awareness code in `crates/rl-rules/src/ai/tactics.rs`.
- **No deviation found.** No `thread_rng()`, `OsRng`, or bare `StdRng::seed_from_u64` with a literal appears outside `rl-core/src/seed.rs` itself and test code (tests legitimately seed with literals for determinism, e.g. `crates/rl-grid/src/dijkstra.rs:328`, `crates/rl-world/src/hydrology.rs:199`).

## Collections in Gameplay/Generation Paths

**Rule:** no `HashMap`/`HashSet` in gameplay or generation paths; use `BTreeMap`, `Vec`, or `BitGrid`, with the reason stated.

- `BTreeMap` is the default map type across the workspace: `crates/rl-grid/src/tile.rs`, `crates/rl-grid/src/spatial.rs`, `crates/rl-world/src/graph.rs`, `crates/rl-rules/src/names.rs`, `crates/rl-bevy/src/ability.rs`, `crates/rl-bevy/src/world.rs`, `crates/rl-bevy/src/stealth.rs`, `crates/rl-bevy/src/minds.rs`, `crates/rl-bevy/src/knowledge.rs`, `crates/rl-bevy/src/fields.rs`, `crates/rl-core/src/id.rs`, and more.
- Two confirmed `HashMap`/`HashSet` usages outside test code, both justified and outside gameplay/generation:
  - `crates/rl-save/src/remap.rs:9,22` - a `HashMap<Entity, SaveId>` used only during save/load remapping, not a gameplay-turn path.
  - `crates/rl-core/src/id.rs:178` - a `HashSet` inside a `#[test]` block (equality/uniqueness check), not production code.
- **No deviation found** in gameplay or generation code proper.
- Where iteration order over a set of positions could otherwise leak into a random draw, the codebase uses `position_hash`/`pair_hash` (documented at `crates/rl-core/src/seed.rs:14-16`) instead of iterating a container and drawing per-element.

## No Theme Words in Engine Crates

**Rule:** no fantasy, sci-fi or pirate vocabulary in engine types, docs or constants; content is an id in a registry.

- `crates/rl-rules/src/lib.rs:3-6` states the rule directly: "Nothing here names a stat, a damage type, a status, a faction, an equipment slot, an item tag, an affix, a fact or a quest."
- Engine types are generic: `StatDef`, `DamageKind`, `StatusDef`, `FactionDef`, `SlotDef`, `TagDef`, `AbilityDef` - all data-driven through `Registry<T>` / `Registry::from_defs`.
- Theme vocabulary (`cutlass`, `pistol`, `bite`, `claw`, `mana`, `stamina`, `broadside`, `grapnel`) appears only in RON asset files under `examples/*/assets/*.ron` and in `crates/rl-bevy/tests/genres/*.ron` - never in engine source (`crates/rl-*/src/`).
- `crates/rl-bevy/tests/genres.rs` is the explicit proof-by-test of this separation: five genre RON files differing only in vocabulary all load through one loader against one set of engine-defined effect kinds (`crates/rl-bevy/tests/genres.rs:1-10`).
- **No deviation found.**

## Costs and Clocks

**Rule:** costs and clocks are integers, the same unit everywhere (hundredths of a step).

- `BASE_ACTION_COST: u32 = 100` (`crates/rl-core/src/turn.rs:22`) is hundredths of a turn.
- `TurnQueue`'s clock (`now: u32`) is integer throughout; `crates/rl-core/src/turn.rs:6-9` explicitly rejects `f32` for the clock because it cannot guarantee bit-identical accumulation.
- `scaled_cost` (`crates/rl-core/src/turn.rs:34`) does speed-to-cost conversion entirely in integer arithmetic (`div_ceil`), never floating point.
- **No deviation found** in the turn/cost path; `f32` appears elsewhere (light, elevation, climate) where it is explicitly not a cost or clock value.

## Error Handling

**Pattern:** hand-written error enums, not `thiserror` or `anyhow` (neither is a dependency anywhere in the workspace).

```rust
/// Why a registration was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterError {
    /// A tile with this name already exists.
    Duplicate(String),
    /// The registry is full.
    Full,
}

impl std::fmt::Display for RegisterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegisterError::Duplicate(n) => write!(f, "tile {n:?} is already registered"),
            RegisterError::Full => write!(f, "tile registry is full"),
        }
    }
}

impl std::error::Error for RegisterError {}
```
(`crates/rl-grid/src/tile.rs:193-212`)

- Each variant carries its own doc comment explaining what condition produced it.
- `Display` messages are lowercase, no trailing punctuation, and read as a sentence fragment.
- The same three-part shape (enum + `Display` + `std::error::Error`) repeats at `crates/rl-rules/src/equip.rs:77-93` (`EquipError`) and `crates/rl-save/src/backend.rs:17-47` (`SaveError`).
- `expect()` with a message is used freely in engine code where a precondition is the caller's bug, not a runtime possibility, e.g. `crates/rl-bevy/src/testing.rs:83` (`self.tiles.expect("floor")`), and in `TurnQueue::set_now`'s `assert!` (`crates/rl-core/src/turn.rs:118`), which documents the panic in a `# Panics` section rather than returning a `Result`.

## RON Schema Comments

**Rule:** every RON schema carries a top-of-file comment listing the full option space.

Every asset RON file under `examples/*/assets/*.ron` and `crates/rl-bevy/tests/genres/*.ron` opens with a `//` comment block enumerating every field, marking which are optional, and spelling out enum variants inline. Example (`examples/corsair/assets/statuses.ron:1-6`):

```ron
// Corsair's statuses: what a bite leaves behind and what a bottle does.
//
// Every field (the ones marked "optional" may be left out):
//   name:      unique, referred to by monsters' `inflicts` and by the code that cures
//   stacking:  optional; what a second application does: Refresh (longer wins, default) | Extend (durations add) | Stack | Ignore
//   ticks:     optional; (damage kind, amount) dealt every whole turn: kind "cutlass" | "pistol" | "bite" | "claw" | "fist" | "fire"
```

- Files that share an option space with a sibling point at it rather than repeat it: `crates/rl-bevy/tests/genres/scifi.ron:1-2` says "The full option space is listed at the top of `fantasy.ron`."
- **No deviation found**; every `.ron` file checked (14 asset files, 5 test genre files) opens with this comment.

## No TODO Comments

**Rule:** no `TODO`, `FIXME`, `HACK`, or `XXX` comments in source; outstanding work goes in `docs/TODO.md` or an issue.

- A workspace-wide grep across `crates/`, `examples/`, `templates/` for `TODO|FIXME|HACK|XXX` returns zero hits.
- **No deviation found.**

## Import Organization

- Bevy-heavy files import `bevy::prelude::*` first, then engine crates, then local `crate::` modules: `crates/rl-bevy/tests/fingerprint.rs:12-20`.
- `use crate::seed::Stream;` style imports are frequently scoped to a single function or `#[cfg(test)]` module rather than hoisted to the top, when the import is only needed for a trait impl or test block: `crates/rl-bevy/src/combat.rs:201`, `crates/rl-bevy/src/ability.rs:1118`.
- No path aliases beyond the workspace crate names declared in the root `Cargo.toml` `[workspace.dependencies]`.

## Function Design

**Size:** small; a system with many parameters is split rather than allowed via clippy lint suppression (`CLAUDE.md`, confirmed by zero `#[allow(clippy::too_many_arguments)]` in the workspace).

**Return values:** `const fn` is used wherever the body permits it, even for non-trivial bit-mixing math (`mix64`, `fnv1a`, `RunSeed::derive`, `RunSeed::stream`, `reschedule_at`, `scaled_cost` - all in `crates/rl-core/src/turn.rs` and `crates/rl-core/src/seed.rs`), so domain salts and costs fold at compile time where possible.

**Parameters:** builder-style chained methods are preferred for assembling multi-step configuration, e.g. `Chain::new().then(Fill { tile })` (`crates/rl-bevy/src/testing.rs:88`) and `Brain::new().then(MeleeAdjacent).then(FleeWhenHurt { .. })` (`crates/rl-bevy/tests/fingerprint.rs:67`).

## Module Design

- A crate's `lib.rs` doc comment (`//!`) is a map of its own modules, each bullet naming the module and its responsibility in one line: `crates/rl-rules/src/lib.rs:8-27`.
- Public items are re-exported at the crate root in addition to being reachable via their module path (`crates/rl-rules/src/lib.rs:29` and following `pub use` lines are typical), so a consumer can `use rl_rules::Thing` without knowing the internal module.
- A `pub mod` for `testing` support code is exposed from the crate needing it (not a separate `-test-utils` crate): `crates/rl-bevy/src/testing.rs`, documented as replacing nine duplicated test-world setups with one shared module (`crates/rl-bevy/src/testing.rs:1-9`).

## Where This Document Was Verified Against Code

Every rule above was checked against the current tree (HEAD `3c44b47`) with `grep`/`find` across `crates/`, `examples/`, `.github/workflows/`, and `scripts/`; no rule in `CLAUDE.md`'s "rules that only review enforces" section was found violated in application code.
Test-only exceptions (a `HashSet` inside a `#[test]` fn, literal RNG seeds inside tests) are noted above and are consistent with the rules, which target gameplay/generation and engine-code randomness respectively, not test assertions.

---

*Convention analysis: 2026-09-17*
