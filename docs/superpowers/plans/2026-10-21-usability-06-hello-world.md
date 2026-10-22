# Usability 06: The Hello World Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Tutorial step 1 is the review's target hello world: one game `Plugin`, one `PlaceRules`, a spawned player, standard keys, and a test that uses the same plugin; the starter template is a plugin too.

**Architecture:** The engine hands each place its own `PlaceSeed`, builds the `WorldMap` from the place rules' tiles when a start made none, spawns a player into a place in one call, and draws the standard tiles in a standard look the game did not set.
Undermere and Foundry take the new parameter but keep their own derivations until branch 07, so no fingerprint moves here.

**Tech Stack:** Rust 2024, Bevy 0.19.

**Spec:** `docs/reviews/engine_usability.md` sections 1.1, 1.2, 1.3, 1.4, 1.9 and "The target hello world"; index `docs/superpowers/plans/2026-10-21-engine-usability.md`.

## Global Constraints

- The index's "Rules for every branch" bind this branch: worktree `scripts/worktree.sh usability-06-hello-world`, after branch 04 merges.
- No fingerprint moves.
- `PlaceSeed` is tier 0 and builds for wasm.
- Keep every `ANCHOR` the guide includes; when its code is gone, move the markers to the code that replaced it.
- Never construct a generator from a constant: a warp with no `Seed` is a `BuildError`, not a seed of zero.
- Plain dash, never an em dash.

## Review Focus

1.
  A game that inserts `Seed` inside its plugin makes its own tests random: the hello world inserts its seed in `main`, and the test's `headless` inserts its own.
  Step 1 keeps `Seed(RunSeed(7))`, since `Seed::from_args` does not exist on wasm and step 1 is a browser demo.
2.
  A game whose start inserts its own `WorldMap` (Undermere, Foundry): `begin_run` never replaces it; pinned by `a_start_that_makes_its_own_world_keeps_it`.
3.
  A game with tiles named `wall` or `floor` and its own looks: a look the game set is never replaced; pinned by `a_look_the_game_set_is_never_replaced`.
4.
  Building floor 2 before floor 1 must not change floor 1: each place's seed depends only on the run and the map; pinned by `a_place_is_built_from_its_own_seed`.
5.
  A warp in an app with no `Seed`: refused with a `BuildError` naming the missing seed; `places` declares the need with `app.needs::<Seed>`, so a game using it is told before play.
6.
  Step 1 loses `q` to quit if `PlayerKeysPlugin` and the game menu offer none: check before deleting the hand-written input.

---

### Task 1: `PlaceSeed`

**Files:**
- Modify: `crates/rl-core/src/seed.rs`, `crates/rl-core/src/lib.rs` (re-export, prelude)
- Modify: `crates/rl-mapgen/src/chain.rs:118,148` (`run`, `run_retrying`)

**Interfaces:**
- Produces: `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)] pub struct PlaceSeed(RunSeed)`.
- Produces: `pub const fn PlaceSeed::of(run: RunSeed, map: u32) -> PlaceSeed`, which is `RunSeed(run.derive(SeedDomain::new(b"rl.place"), map as u64))`; `pub fn rng(self, domain: SeedDomain, index: u64) -> StdRng`; `pub const fn run_seed(self) -> RunSeed`; `impl From<PlaceSeed> for RunSeed`.
- Changes: `Chain::run(&self, ctx: &mut C, seed: impl Into<RunSeed>)` and `run_retrying(.., seed: impl Into<RunSeed>, ..)`; every existing `RunSeed` caller compiles unchanged.

- [ ] **Step 1: Write the failing tests** in `seed.rs`: `two_places_of_one_run_draw_unrelated_streams` (`of(r, 1)` and `of(r, 2)` give different first draws of `rng(D, 0)`), `one_place_draws_the_same_stream_every_time_it_is_asked` (same inputs, same ten draws), `a_place_seed_is_not_its_run_seed` (`of(r, 0).run_seed() != r`).
- [ ] **Step 2:** `cargo test -p rl-core seed`.
  Expected: FAIL to compile.
- [ ] **Step 3:** Implement; change `Chain::run` and `run_retrying`.
- [ ] **Step 4:** `cargo test -p rl-core seed`, `cargo test -p rl-mapgen chain`, `scripts/check-tiers.sh --wasm`.
  Expected: PASS.
- [ ] **Step 5:** `scripts/gate.sh`, commit.
  CHANGELOG: "`PlaceSeed`, one seed per place derived from the run's, and `Chain::run` takes it or a `RunSeed`. To change: nothing."

### Task 2: `PlaceRules` is handed its seed and names its tiles

**Files:**
- Modify: `crates/rl-bevy/src/places.rs:206-220` (the trait), `:51-56` (`MapId::FIRST`), `:472` (`Maps` gains `seed: Option<Res<'w, Seed>>`), `:624` (the call)
- Modify: the six engine test impls `places.rs:716,1205`, `lighting.rs:732,871`, `plugin.rs:1228`, `charts.rs:184`
- Modify: the game impls `examples/tutorial/src/bin/step0*.rs` (9), `templates/starter/src/main.rs:129`, `examples/foundry/src/decks.rs:342`, `examples/undermere/src/dungeon.rs:869`

**Interfaces:**
- Changes: `fn build(&self, map: MapId, seed: PlaceSeed) -> Result<PlaceBuild, BuildError>`.
- Produces: required `fn tiles(&self) -> TileTables`, the tables every place of the game is built of.
- Produces: `pub const MapId::FIRST: MapId = MapId(1)`.
- The engine calls `build(map, PlaceSeed::of(seed.0, map.0))`; with no `Seed` it returns `BuildError::new("warp", "no Seed to build a place from")`.

- [ ] **Step 1: Write the failing test** `a_place_is_built_from_its_own_seed` in `places.rs`: rules that record the seed they are handed; warp into maps 3 then 2; recorded seeds equal `PlaceSeed::of(run, 3)` and `of(run, 2)`.
- [ ] **Step 2:** `cargo test -p rl-bevy own_seed`.
  Expected: FAIL to compile.
- [ ] **Step 3:** Change the trait and the call; move the six engine test impls, each building from the handed seed in place of its XOR.
  An engine test rig that installs `PlaceRulesRes` with no `Seed` (such as `places.rs`'s `rig`) inserts `Seed(TEST_SEED)`.
  A test pinned to a layout gets the first seed that shows its case again; never widen an assertion.
- [ ] **Step 4:** Move the twelve game impls.
  The tutorial and the template build from the handed seed: the four `RunSeed(self.seed.0 ^ (depth as u64) << 32)` lines go, and a rules struct drops a `seed` field it kept only for that.
  Undermere and Foundry take `_seed` and keep `place_seed` and their XOR until branch 07.
- [ ] **Step 5:** `scripts/gate.sh`, which plays the fingerprints.
  Expected: PASS, fingerprints unchanged.
- [ ] **Step 6:** Commit.
  CHANGELOG: "`PlaceRules::build` takes the place's `PlaceSeed`, and `PlaceRules::tiles` names the tile tables places are built of. To change: add `seed: PlaceSeed` to `build` and run the chain on it in place of a seed kept in the rules or mixed by hand; add `fn tiles(&self) -> TileTables { self.tiles.tables() }`."

### Task 3: `places`, and the world built for a start that made none

**Files:**
- Modify: `crates/rl-bevy/src/places.rs` (the trait), `crates/rl-bevy/src/title.rs:44` (`begin_run`), `crates/rl-bevy/src/lib.rs` (re-export, prelude)
- Test: `crates/rl-bevy/src/title.rs` test module

**Interfaces:**
- Produces: `pub trait AddPlaces { fn places(&mut self, rules: impl PlaceRules + 'static) -> &mut Self; }` for `App`, inserting `PlaceRulesRes` and declaring `needs::<Seed>("places", ..)` with the hint `Seed` already uses.
- Changes: `begin_run`, after `NewRun` and before branch 04's play check, inserts `WorldMap::new(rules.0.tiles())` when there is no `WorldMap` and there is a `PlaceRulesRes`.

- [ ] **Step 1: Write the failing tests:** `a_game_that_names_its_places_gets_a_world_built_from_their_tiles` (an app with `places(rules)` and an empty `NewRun`; after two updates a `WorldMap` with the rules' tiles exists and play has begun) and `a_start_that_makes_its_own_world_keeps_it` (the start inserts a `WorldMap` of other tables; it is the one left).
- [ ] **Step 2:** `cargo test -p rl-bevy title`.
  Expected: the first FAILS.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test -p rl-bevy title`.
  Expected: PASS.
- [ ] **Step 5:** `scripts/gate.sh`, commit.
  CHANGELOG: "`app.places(rules)` hands the engine a game's place rules once, and a run whose start makes no `WorldMap` gets one built from `PlaceRules::tiles`. To change: nothing; a start may drop its `insert_resource(WorldMap::new(..))` and `PlaceRulesRes`."

### Task 4: `spawn_player`

**Files:**
- Modify: `crates/rl-bevy/src/places.rs`, `crates/rl-bevy/src/lib.rs`
- Test: `crates/rl-bevy/src/places.rs` test module

**Interfaces:**
- Produces: `pub trait SpawnPlayer { fn spawn_player(&mut self, map: MapId, bundle: impl Bundle) -> EntityCommands<'_>; }` for `Commands`: spawns `(Actor, Player, Blocks, Position(Point::ZERO), bundle)` and queues `WarpRequest::into_place(id, map)` with `Commands::queue`, so the warp is written when the spawn is applied.

- [ ] **Step 1: Write the failing test** `a_player_spawned_into_a_place_stands_on_its_entry`, with the `Caves` test rules.
- [ ] **Step 2:** Run it.
  Expected: FAIL to compile.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run it.
  Expected: PASS.
- [ ] **Step 5:** `scripts/gate.sh`, commit.
  CHANGELOG: "`commands.spawn_player(map, bundle)` spawns the player into a place: an actor that blocks, warped to the place's entry. To change: nothing; a start may replace its spawn at `Point::ZERO` and its `WarpRequest::into_place`."

### Task 5: The standard tiles have a standard look

**Files:**
- Modify: `crates/rl-render/src/map_view.rs:79` (`TileAppearance`), `:366` (`MapViewPlugin::build`)
- Test: `crates/rl-render/src/map_view.rs` test module

**Interfaces:**
- Produces: `pub fn TileAppearance::fill_standard(&mut self, names: &[String])`: each id whose name is one of `TileRegistry::standard()`'s five and which has no look gets the standard one.
- Standard looks: `void` `' '` black on black; `wall` `'#'` srgb(0.60, 0.60, 0.60) on srgb(0.15, 0.15, 0.15); `floor` `'.'` srgb(0.45, 0.45, 0.45) on srgb(0.05, 0.05, 0.05); `door_closed` `'+'` and `door_open` `'\''` srgb(0.70, 0.50, 0.30) on the floor's background.
- `MapViewPlugin` adds `fill_standard_looks` in `Update`, `run_if(resource_added::<WorldMap>)`, before `draw_map`.

- [ ] **Step 1: Write the failing tests:** `a_standard_tile_the_game_gave_no_look_is_drawn_in_the_standard_look`, `a_look_the_game_set_is_never_replaced`, `a_tile_with_another_name_keeps_the_missing_look` (the magenta `?`).
- [ ] **Step 2:** `cargo test -p rl-render standard`.
  Expected: FAIL to compile.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test -p rl-render`.
  Expected: PASS.
- [ ] **Step 5:** `scripts/gate.sh`, commit.
  CHANGELOG: "The five tiles of `TileRegistry::standard()` are drawn in a standard look when the game gives them none. To change: nothing."

### Task 6: The tutorial and the template use them

**Files:**
- Modify: `examples/tutorial/src/bin/step01_walking.rs` (whole file), the start of every other step
- Modify: `templates/starter/src/main.rs` (a `Starter` plugin; the test at `:432-449` uses it)

- [ ] **Step 1:** Step 1 becomes the review's target: `main` adds `RoguelikePlugins` and a `Warren` plugin and inserts `Seed(RunSeed(7))`; `Warren` adds `PlayerKeysPlugin::new().walk("Move", "walk").wait("Act", "wait a turn")`, `places(Floors(TileRegistry::standard()))` and a `NewRun` system calling `spawn_player(MapId::FIRST, (Viewshed::new(9), RevealsMap, Glyph::new('@', Color::WHITE).on_layer(10)))`.
  If `UiPlugin`'s game menu offers no quit, keep a `quit` control as step 9 declares one (`step09_panels.rs`, `declare_controls`).
  The `input` anchor moves to the `PlayerKeysPlugin` line; the hand-written version is what chapter 8 already teaches.
- [ ] **Step 2:** Add `the_player_arrives_standing_on_floor` to step 1: `rl_engine::testing::headless(RunSeed(1), (), Warren)`, then `player` and `at` from `rl_engine::testing`, asserting the cell is walkable.
- [ ] **Step 3:** Steps 2 to 9 use `places` and `spawn_player` and drop their `WorldMap` insert and warp; steps 2 to 6 replace their copied `player_input` with `PlayerKeysPlugin` for what it covers, keeping only their own verbs (the lantern, eating) by hand.
- [ ] **Step 4:** The template becomes `struct Starter; impl Plugin for Starter`, used by `main` and by the test module's `headless`, which becomes `rl_engine::testing::headless(RunSeed(seed), (), (Starter, KeyScriptPlugin))`; its `PlayerIntents`, `Binds`, `declare_controls` and `player_input` (`main.rs:283-356`) give way to `PlayerKeysPlugin` for what it covers.
- [ ] **Step 5:** Run `cargo nextest run -p tutorial`, `scripts/check-template.sh`, `scripts/check-guide.sh`, `scripts/build-demos.sh step01_walking`.
  Expected: PASS.
  Walk and wait by hand in `step01_walking`.
- [ ] **Step 6:** Count step 1's code lines (no blank, comment or doc lines) and its distinct engine names, and put both in the commit message beside the review's 83 and 34.
- [ ] **Step 7:** `scripts/gate.sh`, commit.
  CHANGELOG: "The template is one `Starter` plugin that `main` and its tests both add. To change: nothing for a game; a game made from the template may do the same."

### Task 7: Branch end

- [ ] `scripts/gate.sh --all`, rebase, re-gate, merge.
