# Usability 07: One Seed Spelling and One Root per Place Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every draw is spelled `rng(DOMAIN, index)` over a named `SeedDomain` constant, engine domains live under `rl.`, the engine's streams share one generic type, and every per-place draw comes from that place's `PlaceSeed`.

**Architecture:** Two commits that move no stream (the new API with the old names, then the constants), then one commit that moves every stream it means to and re-baselines the three fingerprints.
This is the only branch of the usability series that moves play.

**Tech Stack:** Rust 2024, Bevy 0.19, rand 0.9.

**Spec:** `docs/reviews/engine_usability.md` section 3.9; index `docs/superpowers/plans/2026-10-21-engine-usability.md`.

## Global Constraints

- The index's "Rules for every branch" bind this branch: worktree `scripts/worktree.sh usability-07-seeds`, after branch 06 merges.
- Tasks 1 and 2 move no fingerprint.
  Task 3 re-baselines rl-bevy's, Undermere's and Foundry's, once, in the commit that moves them, and the message says it is the seed branch's one re-baseline.
- A test that pinned an outcome to a seed gets a seed that shows the same case; it is never weakened.
  A property test over `seeds(full)` needs no change.
- Never construct a generator from a constant in engine code.
- No save version bump: `rl-core/src/seed.rs` promises determinism only within one build, and a continued run's built places come from its save (`WorldMapSave`).
- Plain dash, never an em dash.

## Review Focus

1.
  A game naming a domain `rl.loot` would share the engine's stream: it fails to compile when the domain is a `const`; pinned by a `compile_fail` doc-test.
2.
  Two engine streams with one name: pinned by `every_engine_stream_derives_under_a_name_of_its_own`.
3.
  A game domain equal to an engine one: each game's `every_seed_domain_the_game_draws_from_is_distinct` checks against `ENGINE_DOMAINS` too.
4.
  Population's three draws folded into one domain must stay independent: indices 0, 1 and 2 of one domain, never the same index twice.
5.
  Building floor 2 before floor 1 changes neither: every per-place draw is keyed by the place's own seed; Undermere's existing order test still passes.

---

### Task 1: The spelling, with no stream moved

**Files:**
- Modify: `crates/rl-core/src/seed.rs:46-110` (`RunSeed::stream` deleted; `SeedDomain::new` refuses `rl.`; `SeedDomain::engine`), and `PlaceSeed::of` from branch 06
- Modify: `crates/rl-bevy/src/seed.rs:55-61` (`Seed::stream` deleted), `:73-120` (`RunRng`)
- Modify: the eight `impl Stream for` in `crates/rl-bevy/src/{combat.rs:438,minds.rs:760,loot.rs:170,stealth.rs:279,props.rs:664,effects/mod.rs:59,ability.rs:409,status.rs:515}`
- Modify: every `seed.stream(b"..", i)` caller (16 sites in `examples/` and `templates/`), which become `seed.rng(SeedDomain::new(b".."), i)` for now

**Interfaces:**
- Produces: `pub const fn SeedDomain::new(name: &[u8]) -> SeedDomain`, panicking when `name` starts with `rl.`; `pub const fn SeedDomain::engine(name: &[u8]) -> SeedDomain`, hashing `rl.` then `name`, so `engine(b"place")` equals what `new(b"rl.place")` hashed before.
- Produces: `pub trait StreamDomain: Send + Sync + 'static { const DOMAIN: SeedDomain; }`; `#[derive(Resource)] pub struct RunRng<D: StreamDomain>(pub StdRng, PhantomData<D>)` with `Deref`/`DerefMut` to `StdRng` and `impl<D: StreamDomain> Stream for RunRng<D>`.
- Keeps: `pub type CombatRng = RunRng<CombatDomain>;` and so for the other seven, each marker `pub(crate)` where its alias is.
- Produces: `pub const ENGINE_DOMAINS: &[(&str, SeedDomain)]` in `rl_bevy::seed`, one entry per engine stream and per engine `derive` (`fire`, `gas spill`, `gas swirl`, `restart`, `place`).
- `Seed` derefs to `RunSeed`, so `seed.rng(D, i)` is the one spelling for a draw; `RunSeed::derive` stays as the primitive a sub-seed is built from.

- [ ] **Step 1: Write the failing tests:** in `rl-core`, `an_engine_domain_hashes_as_its_name_under_rl` (`SeedDomain::engine(b"place") == ` the private FNV of `b"rl.place"` with the low bit set) and a `compile_fail` doc-test on `SeedDomain::new`: `const D: SeedDomain = SeedDomain::new(b"rl.loot");`; in `rl-bevy`, `every_engine_stream_derives_under_a_name_of_its_own` over `ENGINE_DOMAINS` (names and domains pairwise distinct).
- [ ] **Step 2:** `cargo test -p rl-core seed` and `cargo test -p rl-bevy seed`.
  Expected: FAIL.
- [ ] **Step 3:** Implement with today's names: `SeedDomain::new(b"combat")` and so on, so no stream moves; `PlaceSeed::of` uses `SeedDomain::engine(b"place")`.
- [ ] **Step 4:** `scripts/gate.sh`, which plays the fingerprints.
  Expected: PASS, fingerprints unchanged.
- [ ] **Step 5:** Commit.
  CHANGELOG: "One spelling for a draw: `seed.rng(DOMAIN, index)`. `Seed::stream` and `RunSeed::stream` are gone, and a game's `SeedDomain::new` refuses a name starting `rl.`, which is the engine's. To change: `seed.stream(b\"name\", i)` becomes `seed.rng(NAME, i)` with `const NAME: SeedDomain = SeedDomain::new(b\"name\")`."

### Task 2: Each game names its domains once

**Files:**
- Create: `examples/undermere/src/seeds.rs`, `examples/foundry/src/seeds.rs`
- Modify: every Undermere and Foundry draw site (`dungeon.rs:543,570,1293`, `dungeon/biomes.rs:539,562`, `dungeon/lairs.rs:117,156`, `vaults.rs:169,362`, `writings.rs:159`, `finds.rs:187`, `population.rs:539,565,616`, `lore.rs:226`, `foundry/src/props.rs:154`)
- Modify: the tutorial binaries (`warren.rats`, one `const` at the top of each) and `templates/starter/src/main.rs` (`starter.populate`)

**Interfaces:**
- Produces: in each `seeds.rs`, one `pub const NAME: SeedDomain = SeedDomain::new(b"<game>.<name>");` per domain, with today's bytes (`LORE` keeps `b"lore"` until Task 3), and `pub const ALL: &[(&str, SeedDomain)]`.

- [ ] **Step 1: Write the failing test** `every_seed_domain_the_game_draws_from_is_distinct` in each `seeds.rs`: names and domains pairwise distinct, and none equal to an entry of `ENGINE_DOMAINS`.
- [ ] **Step 2:** Move every site to a constant; `rg 'SeedDomain::new' examples/*/src` outside `seeds.rs`, the tutorial's one `const` per binary and test modules finds none.
- [ ] **Step 3:** Fingerprints and `scripts/gate.sh`.
  Expected: PASS, unchanged.
- [ ] **Step 4:** Commit.
  No CHANGELOG line.

### Task 3: The streams move, once

**Files:**
- Modify: `crates/rl-bevy/src/seed.rs` (`ENGINE_DOMAINS` names, `Seed::place`), the eight markers, `fire.rs:295`, `gas.rs:138,143`, `plugin.rs:457`
- Modify: `examples/undermere/src/seeds.rs`, `dungeon.rs:565-571` (`place_seed` goes; `generate` takes `seed: PlaceSeed`), its callers (`build`, `--recipe` in `main.rs`, tests), `vaults.rs`, `writings.rs`, `finds.rs`, `population.rs`
- Modify: `examples/foundry/src/decks.rs:262` (the XOR goes), `examples/foundry/src/props.rs:154`
- Modify: the tutorial's rats in `step03` to `step09` (`seed.stream(b"warren.rats", ev.map.0)` today) and the template's `starter.populate` (`templates/starter/src/main.rs:377`)
- Modify: `crates/rl-bevy/tests/fingerprint.rs`, `examples/undermere/tests/fingerprint.rs`, `examples/foundry/tests/fingerprint.rs`

The keying rule: a draw about one place comes from its `PlaceSeed`; a draw about the run's shape, made before or apart from building a place, comes from `Seed` indexed by depth.
The tutorial and the template follow it too, so the guide teaches the one way Undermere draws.

**Interfaces:**
- Produces: `pub fn Seed::place(&self, map: MapId) -> PlaceSeed`, which is `PlaceSeed::of(self.0, map.0)`: the spelling a system uses for a place's draws, `seed.place(ev.map).rng(RATS, 0)`.

| Domain | Keyed by |
|---|---|
| engine: `rl.combat`, `rl.minds`, `rl.loot`, `rl.props`, `rl.stealth`, `rl.status.stumble`, `rl.ability`, `rl.ability.reflect`, `rl.fire`, `rl.gas.spill`, `rl.gas.swirl`, `rl.restart`, `rl.place` | the run, as today |
| Undermere `pool`, `captive`, `final_boss`, `lairs`, `recipe`, `biome`, `undermere.lore` | the run; drawn in `Undermere::new` before any floor is built |
| Undermere `vaults.fixtures`, `vaults.marks`, `writings`, `finds`, `population` (indices 0, 1, 2 for the plan, the bands and the rest) | the place's `PlaceSeed` |
| Undermere's chain | the handed `PlaceSeed` |
| Foundry's chain, `foundry.props` | the place's `PlaceSeed` |
| the tutorial's `warren.rats`, the template's `starter.populate` | the place's `PlaceSeed`, index 0 |

- [ ] **Step 1: Write the failing test** `a_seed_s_place_is_the_place_seed_of_its_run_and_map` in `crates/rl-bevy/src/seed.rs`, a plain test with no `App`: `Seed(r).place(MapId(3)) == PlaceSeed::of(r, 3)`.
- [ ] **Step 2:** Make the changes in the table; `seed.place(map)` wherever a system needs a place's seed outside `build`.
- [ ] **Step 3:** Run `cargo nextest run --workspace --profile full` and `scripts/check-template.sh`.
  Expected: the three fingerprints FAIL; list every other failure.
- [ ] **Step 4:** For each other failure, the tutorial's and the template's included: a test pinned to a seed gets the first seed that shows its case, found with `first_seeds`; anything else is a bug in this task.
  Never widen an assertion.
- [ ] **Step 5:** Re-baseline the three fingerprints.
- [ ] **Step 6:** `scripts/gate.sh`, commit with the message saying it is the seed branch's one re-baseline.
  CHANGELOG: "The engine's streams are named under `rl.`, and every place draws from its own `PlaceSeed`, so every seed plays differently from the last release. To change: nothing in code, though a game's own per-place draws may follow, `seed.place(map).rng(DOMAIN, i)`; a recorded seed or replay from before plays another run, and a continued save builds the floors it had not reached differently."

### Task 4: Branch end

- [ ] `scripts/gate.sh --all`, rebase, re-gate.
  If anything that landed on `main` meanwhile moves the fingerprints again, re-baseline in the same rebase and say so.
- [ ] Merge.
