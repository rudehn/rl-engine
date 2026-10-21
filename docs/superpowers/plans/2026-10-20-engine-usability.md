# Engine Usability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Act on `docs/reviews/engine_usability.md`, so a new game author meets one way to do each thing, a small prelude, and a hello world without ceremony.

**Architecture:** Six branches, run one after another, each a worktree from `scripts/worktree.sh` merged before the next starts.
Only the seed branch moves play, so it is the one place the fingerprints are re-baselined; every other branch holds them still, and a fingerprint that moves outside branch 1 is a bug in that branch.

**Tech Stack:** Rust, Bevy, cargo nextest, `scripts/gate.sh`.

**Spec:** `docs/reviews/engine_usability.md` (main 635b98e0).

## What the re-check found

The review asked for its counts to be re-checked before acting; they were, on 2026-10-20 at 635b98e0.

- Holding: 62 `depends_on`, 18 `is_plugin_added`, 26 `needs`, 14 `reads`, 13 `*Running` sites; 25 `CombatRules::new`; 26 `ResMut<Facets>`; one `BuildContext` impl; three `HitModel` impls; 38 log calls passing a turn; 20 `App` extension traits; eight hand-written `Stream` impls; 15 `Seed::stream(b"..")`, 41 `SeedDomain::new`, 10 `.derive(` sites.
- The duplicated names are real and some are true duplicates: `Turn` is declared as a separate enum in both `rl-mapgen/src/prefab.rs:71` and `rl-rules/src/prefab.rs:229`.
- Corrected: `DamageStages` already has a `Default` of `[SubtractArmor]` (`crates/rl-bevy/src/combat.rs:425`), so the eleven `DamageStages(vec![Box::new(SubtractArmor)])` sites are redundant, not missing an API.
- Corrected: not all of section 7 is Undermere-only. Foundry uses `accrete`, `dress`, `Offers` and revival. Trade, records, the party and sheet panels, `Lakes`, namegen, threat, `Keen` and `Reflects` are Undermere-only.
- Corrected: `Charts` has no user in any example at all.
- The six interner wrappers already share `Interner<T>`; they are thin and are left alone.
- `PlaceRules::build(&self, map: MapId)` takes no seed, so every game smuggles a `RunSeed` into its rules struct, which is how Undermere came to key one floor three ways.

## Global Constraints

- Tier 0 and 1 crates stay Bevy-free and build on `wasm32-unknown-unknown` (`scripts/check-tiers.sh --wasm`).
- No `HashMap`/`HashSet` in gameplay or generation; no `TODO` in source; plain dash, never an em dash.
- Every commit runs `scripts/gate.sh` and writes its `CHANGELOG.md` line under `Unreleased`; every branch ends with `scripts/gate.sh --all`.
- Guide and system pages are not rewritten per branch; the release's documentation pass does it from the CHANGELOG lines. The one exception is a renamed file the guide includes by path, whose include is fixed in the same commit, since that is mechanical.
- A fingerprint moves only in branch 1, re-baselined once, with the reason in the commit.
- Before each merge, read what landed on main since the base and re-gate the combination.

## Review Focus

- A save written before branch 1 and loaded after it: the places it regenerates would differ from what the player saw. Expected: the save is refused with a message naming the version, never continued on a silently different map.
- A game with a title screen: once play begins on its own, a `HoldStart` must still keep the run from starting until the title releases it.
- A soft plugin check made in `Plugin::build`: `is_plugin_added` there depends on the order plugins were added, so "is noise running" answered at build time is wrong when `NoisePlugin` is added after its reader. Expected: the answer is the same in either order.
- A game that globs two opt-in prelude modules: a name exported by both is an ambiguity error at every use. Expected: no name appears in two prelude modules.
- A game naming a seed domain `rl.loot`: it would silently share the engine's loot stream. Expected: it does not compile.

Each of these has its test in the owning task below.

---

## Branch 0: Clear the ground

Not code; done before branch 1 is cut.

- [ ] **Step 1: Settle the in-flight branches that touch the same files.**
  `fix-running` (2 ahead, 46 behind, wip) fixes the `*Running` bug A2 that branch 3 replaces; `fix-rules-rng` (1 ahead, 0 behind) changes `rl-mapgen/src/context.rs`, which branches 1 and 2 rewrite; `fix-saves`, `fix-ui-keys`, `fix-projectile`, `wave-palace` and `wave-town` touch Undermere's seeds or the plugins. Each is finished and merged, or closed with its intent written into `docs/TODO.md`. `fix-running` is the one to close rather than finish, if its wip is far behind: branch 3 deletes what it fixes.
- [ ] **Step 2: Remove the worktrees of merged branches.**
  `fix-stairs` and `soak-base-town` are 0 ahead of main: `git worktree remove` each and delete the branch. The detached worktrees at 49c4549a (`fix-projectile`, `fix-running`, `fix-saves`, `palace-base`) go once their branches are settled.

---

## Branch 1: `seeds` - one spelling, one root per place

The only branch that moves play.

### Task 1.1: One spelling in `rl-core`

**Files:**
- Modify: `crates/rl-core/src/seed.rs:46-110`
- Modify: `crates/rl-bevy/src/seed.rs:29-64`
- Modify: every caller of `RunSeed::derive`, `RunSeed::stream` and `Seed::stream`

**Interfaces:**
- Produces: `RunSeed::rng(self, domain: SeedDomain, index: u64) -> StdRng` (unchanged); `RunSeed::seed_for(self, domain: SeedDomain, index: u64) -> u64` (was `derive`); `Seed::rng(&self, domain: SeedDomain, index: u64) -> StdRng` (was `stream(&[u8], u64)`); `SeedDomain::new(name: &[u8]) -> SeedDomain`, a `const fn` that panics at compile time on a name starting `rl.`; `SeedDomain::engine(name: &[u8]) -> SeedDomain`, which prefixes `rl.` itself.
- Removed: `RunSeed::derive`, `RunSeed::stream(domain)` (callers write `seed_for(domain, 0)`), `Seed::stream`.

- [ ] **Step 1: Write the failing tests** in `crates/rl-core/src/seed.rs`:
  `a_game_domain_and_an_engine_domain_of_the_same_name_draw_different_streams` asserts `SeedDomain::new(b"loot") != SeedDomain::engine(b"loot")` and that `RunSeed(7).seed_for` differs under the two.
  A `compile_fail` doc-test on `SeedDomain::new`: `const D: SeedDomain = SeedDomain::new(b"rl.loot");` fails to compile.
- [ ] **Step 2: Run** `cargo test -p rl-core seed` and see both fail.
- [ ] **Step 3: Implement** the renames and the two constructors. The `rl.` check is a `const` loop over the first three bytes and a `panic!`, so it fails at compile time when the domain is a `const`.
- [ ] **Step 4: Move every caller** to the one spelling. Undermere's `b"lore"` and `b"crownaur"` become `undermere.lore` and `undermere.crownaur` in task 1.4, not here.
- [ ] **Step 5: Run** `scripts/gate.sh`; fingerprints move here, and are not re-baselined until task 1.5.
- [ ] **Step 6: Commit** with a CHANGELOG line naming each removed spelling and its replacement.

### Task 1.2: `RunRng<D>` replaces the eight hand-written streams

**Files:**
- Modify: `crates/rl-bevy/src/seed.rs:73-120`
- Modify: `combat.rs:438`, `minds.rs:760`, `loot.rs:170`, `stealth.rs:272`, `props.rs:662`, `effects/mod.rs:59`, `ability.rs:409`, `status.rs:515`, all in `crates/rl-bevy/src/`

**Interfaces:**
- Produces: `pub trait Domain: Send + Sync + 'static { const DOMAIN: SeedDomain; }`; `#[derive(Resource)] pub struct RunRng<D: Domain>` implementing `RngCore` and `Stream`; `app.add_stream::<RunRng<D>>(plugin)` as today.
- Keeps: `pub type CombatRng = RunRng<CombatDomain>;` and so on for all eight, so no system signature changes.

- [ ] **Step 1: Read the eight impls.** Any that holds more than a `StdRng` stays hand-written and the commit says which and why.
- [ ] **Step 2: Write the failing test** `every_engine_stream_derives_under_its_own_rl_prefixed_domain` in `crates/rl-bevy/src/seed.rs`: a `const ENGINE_DOMAINS: &[(&str, SeedDomain)]` listing all eight, asserted pairwise distinct and each name starting `rl.`.
- [ ] **Step 3: Implement** `RunRng<D>` and the eight `Domain` markers, each with `SeedDomain::engine(b"<old name>")`, plus the mapgen chain's own domain.
- [ ] **Step 4: Run** `cargo test -p rl-bevy seed` and then `scripts/gate.sh`.
- [ ] **Step 5: Commit.**

### Task 1.3: `PlaceSeed`, handed to `PlaceRules::build`

**Files:**
- Modify: `crates/rl-bevy/src/places.rs:207-220` (the trait), `:717`, `:1206` (its test impls), and the call site that invokes `build`
- Modify: the 12 game `PlaceRules` impls in `examples/`

**Interfaces:**
- Produces: `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub struct PlaceSeed(u64)` in `crates/rl-core/src/seed.rs` (tier 0, so mapgen and rules can take it); `PlaceSeed::of(run: RunSeed, map: u32) -> PlaceSeed`, derived under `SeedDomain::engine(b"place")`; `PlaceSeed::rng(self, domain: SeedDomain, index: u64) -> StdRng`; `PlaceSeed::seed_for(self, domain: SeedDomain, index: u64) -> u64`.
- Changes: `PlaceRules::build(&self, map: MapId, seed: PlaceSeed) -> Result<PlaceBuild, BuildError>`. The engine computes the seed from `Seed` at the one call site; a system that runs on arrival computes the same one with `PlaceSeed::of(seed.0, map.0)`.

- [ ] **Step 1: Write the failing tests** in `crates/rl-core/src/seed.rs`: `two_places_of_one_run_draw_unrelated_streams` (`PlaceSeed::of(r, 1)` and `of(r, 2)` give different `rng(D, 0)` first draws) and `one_place_draws_the_same_stream_every_time_it_is_asked` (same inputs, same first ten draws).
- [ ] **Step 2: Run** them; they fail on the missing type.
- [ ] **Step 3: Implement** `PlaceSeed` and change the trait and its call site.
- [ ] **Step 4: Move the 12 game impls** to take the seed; a rules struct that held a `RunSeed` only to derive a place seed drops the field.
- [ ] **Step 5: Run** `scripts/gate.sh` and commit.

### Task 1.4: Each game's `seeds` module

**Files:**
- Create: `examples/undermere/src/seeds.rs`, `examples/foundry/src/seeds.rs`
- Modify: every Undermere and Foundry draw site (`dungeon.rs:570`, `vaults.rs:169,362`, `writings.rs:159`, `population.rs:539,565,616`, `dungeon/biomes.rs:539,562`, and the rest found by `rg 'b"' examples/*/src`)

**Interfaces:**
- Produces: one `pub const NAME: SeedDomain = SeedDomain::new(b"undermere.name");` per domain, and `pub const ALL: &[(&str, SeedDomain)]`.

The keying rule, which decides each site: a draw about one place comes from its `PlaceSeed`; a draw about the run's shape, read before or apart from building a place (which stratum a depth is, where a landmark falls, the final boss), comes from `Seed` indexed by depth.

- [ ] **Step 1: Classify every Undermere domain** by that rule, reading what else reads each draw. Expected: `place`, `vaults.fixtures`, `vaults.marks`, `writings`, `population` (all three), `finds`, `lairs`, `tavern`, `wares` are per place; `final_boss`, `names`, `captive` are per run. `recipe` and `biome` are per place unless the strata plan or a preview reads them before the floor is built, in which case they stay per run by depth; the commit says which and cites the reader.
- [ ] **Step 2: Fold the population split.** `population`, `population.bands` and `population.plan` become one `POPULATION` domain drawn at indices 0, 1 and 2, which keeps the three draws independent without three names.
- [ ] **Step 3: Write** `every_seed_domain_the_game_draws_from_is_distinct` in each `seeds.rs`, over `ALL`.
- [ ] **Step 4: Move every site** to a constant; `rg 'SeedDomain::new|b"' examples/*/src` outside `seeds.rs` and tests finds none.
- [ ] **Step 5: Run** `scripts/gate.sh` and commit.

### Task 1.5: Saves and the one re-baseline

**Files:**
- Modify: the save version constant in `rl-save` or each game's `SavePlugin`; the fingerprint baselines in Undermere, Foundry and the tutorial

- [ ] **Step 1: Read whether a load regenerates places from the seed.** If it does, bump the save version, and write `a_save_from_before_place_seeds_is_refused_by_name` loading a fixture saved at the old version and asserting the refusal message names it.
- [ ] **Step 2: Re-baseline every fingerprint once**, in one commit whose message says it is the seed branch's single re-baseline.
- [ ] **Step 3: Run** `scripts/gate.sh --all` and merge.

---

## Branch 2: `one-user-generics` - remove what has one user

Holds every fingerprint still.

### Task 2.1: `Pass` without a context parameter

**Files:** `crates/rl-mapgen/src/context.rs`, every `impl Pass<BaseContext>` (35 engine, 12 game)

**Interfaces:** `BuildContext` is deleted; `BaseContext` is renamed `Context`; `pub trait Pass { fn apply(&self, ctx: &mut Context) ... }`.

- [ ] **Step 1:** Delete the trait, rename, and remove the type parameter at every impl and every chain.
- [ ] **Step 2:** `scripts/gate.sh`; no fingerprint moves. Commit.

### Task 2.2: `HitRules` is an enum

**Files:** `crates/rl-bevy/src/combat.rs` (or wherever `HitModel` lives), its three impls

**Interfaces:** `pub enum HitRules { Certain, Percent(Percent) }` with the method `HitModel` had; `HitModel` and `D20` are deleted, and the test that used `D20` tests `Percent` instead.

- [ ] **Step 1:** Convert, `scripts/gate.sh`, commit.

### Task 2.3: Facet keys are `&'static str`

**Files:** `crates/rl-ui/src/facet.rs`, `crates/rl-ui/src/view/nearby.rs`, the 26 `ResMut<Facets>` sites

**Interfaces:** `Facet { key: &'static str, text: String, tone: ToneId }`; `Facets` and `FacetKey` are deleted.

- [ ] **Step 1:** Convert; the nearby filter compares strings. `scripts/gate.sh`, commit.

### Task 2.4: Dead weight

- [ ] **Step 1: Delete `Charts`** (`crates/rl-bevy/src/charts.rs`, 332 lines) with its minds and save hooks; it has no user and overlaps `Knowledge`. The CHANGELOG line says to use `Knowledge`.
- [ ] **Step 2: Delete the eleven `DamageStages(vec![Box::new(SubtractArmor)])` sites**, relying on `Default`; `CombatPlugin` calls `init_resource::<DamageStages>()` so a game that wants the default writes nothing. Undermere keeps its own list with `HalveArmor`.
- [ ] **Step 3: One prefab `Turn`.** `rl-mapgen/src/prefab.rs:71` re-exports `rl_rules::prefab::Turn` rather than declaring its twin, if `rl-mapgen` already depends on `rl-rules`; otherwise the type moves down to the lower tier and the other re-exports it.
- [ ] **Step 4:** `scripts/gate.sh`, commit each, `scripts/gate.sh --all`, merge.

---

## Branch 3: `plugin-presence` - one way per question

Two questions, one answer each:

| Question | The one way |
|---|---|
| Does this plugin require another? | `app.depends_on::<P>()`, which asserts. |
| Does this plugin integrate with another if it is there? | `app.is_plugin_added::<P>()`, asked in `Plugin::finish`, never in `build`. |
| Does it need a resource the game makes? | `app.needs::<R>(plugin, hint)`. |
| Does it read a resource whose absence means something? | `app.reads::<R>(plugin, meaning)`. |

### Task 3.1: The `*Running` markers go

**Files:** `crates/rl-bevy/src/noise.rs`, `stealth.rs`, `minds.rs`, and the 13 sites reading them

- [ ] **Step 1: Write the failing test** `a_reader_sees_noise_running_whichever_order_the_plugins_were_added` in `noise.rs`: two `App`s adding the reader before and after `NoisePlugin`, asserting the same answer. This is the Review Focus order case.
- [ ] **Step 2: Implement:** each reader resolves presence once in `finish` and stores it in its own config resource, or adds its systems only there. `NoiseRunning`, `StealthRunning` and `MindsRunning` are deleted.
- [ ] **Step 3: Audit the 18 `is_plugin_added` sites** and move any in `build` to `finish`.
- [ ] **Step 4:** `scripts/gate.sh`, commit.

### Task 3.2: One extension trait per crate

**Files:** the 20 traits (`rg -n "^pub trait (Add|Keep|Needs|Reads|Resets)" crates`)

**Interfaces:** `pub trait EngineApp` in `rl-bevy` holding every method of `AddAction`, `AddChoice`, `AddEffect`, `AddMoment`, `AddVerb`, `AddWork`, `AddSound`, `AddSpending`, `AddStream`, `AddAirborne`, `AddControls`, `AddEngineEffects`, `KeepOnRemnant`, `Needs`, `Reads`, `ResetsOnNewRun`; `pub trait UiApp` in `rl-ui` for `AddTone`, `AddModal`, `AddSettings`; `pub trait SaveApp` in `rl-save` for `AddSaveable`. Method names do not change, so call sites change only their `use`.

- [ ] **Step 1:** Merge, each method keeping its doc comment. `AddAirborne`'s method stays `pub(crate)` if no game calls it.
- [ ] **Step 2:** `scripts/gate.sh`, commit, `scripts/gate.sh --all`, merge.

---

## Branch 4: `names` - one meaning per name

Renames are mechanical and done with the compiler's help; each is one commit with its CHANGELOG line, so a game migrates by search and replace.

### Task 4.1: "Hold" means one thing

| Now | After | Why |
|---|---|---|
| `TurnHold` | `CueWait` | The turn loop waiting on cues. |
| `HoldStart` | `HoldStart` | Keeps the name: it is the one "hold" left. |
| `StatusDef::holds()` | `StatusDef::skips_turns()` | Says what it does. |
| `HeldWhileWorn` | `WornStatuses` | Statuses lent by worn gear. |
| `held_for_life` | `innate` | Statuses an actor is born with. |
| `Charts::holds` | gone with `Charts` in branch 2 | |

Key repeat's "the engine's hold" becomes "key repeat" in its docs.

### Task 4.2: No name declared twice

Each pair keeps the bare name where its meaning is the one a new author meets first, and the other takes its domain:

| Name | Keeps it | Renamed |
|---|---|---|
| `Reach` | `rl_rules::arms::Reach` | prefab's `Reach` to `BandFallback`; combat's narration `Reach` to `BlowRange`; ui's `Reach` SystemParam to `TargetFootprint` |
| `Turn` | `rl_bevy::Turn` (the set) | the prefab enum to `Rotation` (one type after branch 2) |
| `Offered` | `rl_rules::bump::Offered` | ability's to `AbilityOffer` |
| `Cell` | `rl_render::terminal::Cell` | mapgen prefab's to `PrefabCell` |
| `Spent` | `rl_bevy::consumable::Spent` | charges' to `ChargeOutcome` |
| `Heard` | `rl_bevy::noise::Heard` | narrate's SystemParam to `HeardNarration` |
| `Secret` | `rl_bevy::secrets::Secret` | mapgen's to `SecretDoor` |
| `Placed` | `rl_rules::loot::Placed` | pool's to `Placement` |

- [ ] **Step 1: Write a check** `scripts/check-names.sh`, run by `scripts/gate.sh`: every `pub struct|enum|type|trait` name across `crates/` is declared once, listing any duplicate. It fails now.
- [ ] **Step 2:** Rename until it passes; read each type's doc first and adjust the new name if its doc says something the table missed.

### Task 4.3: The names that mislead

Renamed: `Moment` to `Trigger` (with `Moments`, `AddMoment`), `Afflict`/`Cure`/`Afflicted` to `ApplyStatus`/`RemoveStatus`/`Statuses`, `Strikes` to `BonusDamage`, `Among`/`Chooser` to `OfferFilter`, `Stowed` to `Unusable`, `Life` to `RevivalSnapshot`, `Doing` to `CurrentTactic`, `Keen` to `SearchBonus`, `Pulse` to `WornTimer`.
Kept, since each is a defined term a glossary carries: `Loadout`, `Facet`, `Cue`, `Bestows`, `Provenance`, and the place vocabulary.

- [ ] **Step 1:** One commit per rename group, `scripts/gate.sh` each, `scripts/gate.sh --all`, merge.
- [ ] **Step 2:** Add to `docs/TODO.md` that the release's documentation pass writes a guide glossary page: `MapId` and what hangs off it, the turn sets, cues, facets, loadouts, and every rename above.

---

## Branch 5: `prelude` - small by default

After branches 2 to 4, so the names it exports are final.

**Files:** every crate's `pub mod prelude`, `crates/rl-engine/src/lib.rs`

**Interfaces:**
- `rl_engine::prelude`: what tutorial steps 1 to 3 use and no more; the target is about 60 names.
- Opt-in: `prelude::combat`, `prelude::items`, `prelude::minds`, `prelude::stealth`, `prelude::mapgen`, `prelude::ui`, `prelude::save`, and `prelude::extras` for trade, records, party, sheet, `Lakes`, namegen, threat and `Reflects`.
- `prelude::all` re-exports every module above, so a game migrates in one line first and narrows later.
- Plumbing in no prelude: `Airborne`, `Lands`, `Charting`, `Marksmanship`, `SideWas` and their like, reached by path.

- [ ] **Step 1: Write** `scripts/check-prelude.sh`, run by `scripts/gate.sh`: the core prelude exports at most 80 names, and no name is exported by two prelude modules (the Review Focus glob case). It fails now.
- [ ] **Step 2: Build the core prelude from the tutorial:** switch steps 1 to 3 to `use rl_engine::prelude::*;` alone and add names until they compile.
- [ ] **Step 3: Sort the rest** into the opt-in modules; each later tutorial step adds the module its chapter introduces.
- [ ] **Step 4: Move Undermere and Foundry** to `prelude::all`, then narrow each to the modules it uses.
- [ ] **Step 5:** `scripts/gate.sh --all`, merge.

Section 7's other suggestion, cargo features or an `rl-extras` crate, is not taken: an unused plugin already costs nothing at run time, and features would add a configuration matrix every CI run and every test pays forever. Out of the core prelude is what a reader needs; a crate split is reconsidered only if compile time is measured to matter.

---

## Branch 6: `hello-world` - the first game is short

### Task 6.1: Play begins on its own

**Files:** `crates/rl-bevy/src/plugin.rs:855-862`, `crates/rl-bevy/src/title.rs:32`, the 49 non-test `set(EngineState::Playing)` sites

**Interfaces:** a `CorePlugin` system in `EngineState::Idle` sets `Playing` on the first frame where a `WorldMap` exists and no `HoldStart` is present; the 120-frame warning is deleted.

- [ ] **Step 1: Write the failing tests** in `plugin.rs`: `play_begins_on_its_own_once_a_world_is_given` and `a_held_start_keeps_play_from_beginning_until_released` (the Review Focus title case).
- [ ] **Step 2: Implement**, then delete the explicit sets from the games and tutorial; a test helper that set it keeps working, since setting `Playing` by hand stays legal.
- [ ] **Step 3:** `scripts/gate.sh`, commit.

### Task 6.2: `place_rules` and `spawn_player`

**Interfaces:**
- `EngineApp::place_rules(&mut self, rules: impl PlaceRules + 'static) -> &mut Self`: inserts `PlaceRulesRes` and builds `WorldMap` from the tile tables at startup, declaring `needs::<TileAppearance>`.
- `Commands::spawn_player(&mut self, bundle: impl Bundle, map: MapId) -> EntityCommands` via an extension trait on `Commands`: spawns at the place's arrival point, replacing spawn-at-`Point::ZERO` then `WarpRequest::into_place`.

- [ ] **Step 1: Write** `a_player_spawned_into_a_place_stands_on_its_arrival_cell` in `places.rs`.
- [ ] **Step 2: Implement**, move the tutorial and both games, `scripts/gate.sh`, commit.

### Task 6.3: The log stamps its own turn

**Interfaces:** `#[derive(SystemParam)] pub struct Log` over `ResMut<MessageLog>` and the turn clock, with `push(text, tone)` and `notice(text)` stamping the current turn; `MessageLog::push_at(text, tone, turn)` stays for replay and tests.

- [ ] **Step 1: Write** `a_line_pushed_through_the_log_carries_the_turn_it_was_pushed_on`.
- [ ] **Step 2: Implement**, move the 38 sites, `scripts/gate.sh`, commit.

### Task 6.4: Factions in one place

**Interfaces:** `CombatRules` drops its `factions` field and reads them from `Registries`; `CombatRules::new(&sides)` becomes `CombatRules::default()` with the stat fields set by builder.

- [ ] **Step 1:** Implement, move the 25 sites, `scripts/gate.sh`, commit.

### Task 6.5: The tutorial lines up with the guide

- [ ] **Step 1:** Rename `step08_content` to `step07`, `step09_shove` to `step08`, `step10_panels` to `step09`, so chapter N runs `stepN`, and fix the guide's include paths in the same commit.
- [ ] **Step 2:** `PlayerKeysPlugin` from step 1; step 1's 35-line `player_input` is deleted.
- [ ] **Step 3:** The README's step count says nine.
- [ ] **Step 4:** `scripts/gate.sh --all`, merge.

---

## After the six

Re-measure what the review measured: core prelude size, step 1's line count and `insert_resource` count, and the duplicate-name check, and add the before and after to the bottom of `docs/reviews/engine_usability.md` so it records what was done.
The release's documentation pass then rewrites the guide and the system pages from the CHANGELOG lines, writes the glossary, and re-blesses the pages.
