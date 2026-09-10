# fantasy-rogue: core / map / render / save / infra review

Scope: `src/core/`, `src/map/`, `src/render/`, `src/audio/`, `src/save/`, `src/main.rs`, `src/lib.rs`,
`src/actors/{auto_explore,horde,run_stats}.rs`, `src/actors/ai/pathfinding.rs`, the FOV system
(which actually lives in `src/core/components.rs:758`), `examples/`, `Cargo.toml`, Trunk/wasm, docs.
Reviewed against `/Users/nathanrude/Development/roguelike_engine` (the prior extraction attempt) and
`/Users/nathanrude/Development/living-world-rogue`'s `lwr-world` RNG.

Verification: `cargo test --offline --lib core::` → 120 passed, 0 failed (0.91s, warm cache).
No repo files were modified.

---

## A. Subsystem inventory

| Subsystem | Path | LOC | Purpose | Verdict |
|---|---|---|---|---|
| Run-seed derivation | `src/core/rng.rs` | 414 | `RunSeed` + `SeedDomain` + SplitMix64 finalizer; `GameRng` / `FxRng` resources | **EXTRACT-AS-IS** (drop the fantasy domain names, see C) |
| Turn scheduler primitives | `src/core/turns.rs:15-215` | ~200 | `TurnManager` min-heap, `dequeue_next_batch_pure`, `compute_reinsert_time` | **EXTRACT-AS-IS** |
| Turn pipeline plugin | `src/core/turns.rs:243-600` | ~360 | `TurnState`, `ProcessingPhase`, `TurnOrderPlugin`, `select_next_actor`, speed modifiers | **EXTRACT-WITH-REDESIGN** — the plugin hardwires 15 game intent messages and 4 named status components |
| Grid addressing | `src/map/grid.rs` | 229 | `Grid2D` trait + parametric `Grid<T>` scratch buffer | **EXTRACT-AS-IS** — best small piece in the repo |
| Map resource | `src/map/map.rs` | 409 | `Vec<Tile>` grid, `blocked`/`costly_tiles`/`explored_tiles` parallel arrays, `BaseMap`/`Algorithm2D` impls, `SmokeOpacityMap` overlay | **EXTRACT-WITH-REDESIGN** — flat `Tile` enum, hard-coded cost magic numbers |
| Tile semantics | `src/map/tile.rs` | 490 | Flat `Tile` enum + `flammability()` + `promotion_rule()` tables | **EXTRACT-WITH-REDESIGN** — combinatorial enum, fantasy variants (Crystal, Portal, Fungus) |
| Builder framework | `src/map/builders/mod.rs` | 532 | `BuildContext`, `EngineBuilderMap`, `MapBuilder<C>`, `BuilderChain<C>`, `BuilderPhase` | **EXTRACT-AS-IS** — already the engine's design, improved (wasm-safe `Instant`, no unseeded ctor) |
| Grid algorithms | `src/map/builders/algorithms.rs` | 499 | CA iteration, flood fill, region analysis, best-of-N blob, all on `Grid<T>` with a caller-owned RNG | **EXTRACT-AS-IS** |
| Room accretion | `src/map/builders/brogue.rs` | 812 | Brogue-style room accretion + loop doors | **EXTRACT-WITH-REDESIGN** — correct but O(attempts × W×H); see D-1 |
| Cleanup builders | `src/map/builders/cleanup.rs` | 661 | `is_fully_connected`, CaveEroder, DiagonalCuller, PillarCuller, IsolatedAreaCuller, FinishDoors | **EXTRACT-AS-IS** |
| Decoration pass | `src/map/builders/decoration.rs` | 310 | Ordered `DecorationRule { target, on: TilePredicate, placement: Placement }` list | **EXTRACT-WITH-REDESIGN** — the *shape* is excellent, but `target: Tile` binds it to the concrete enum |
| BSP / corridors / stairs | `src/map/builders/{geometry,stairs}.rs` | 536 | BSP rooms, nearest-neighbour corridors, start point, Dijkstra distant exit | **EXTRACT-AS-IS** |
| CA / drunkard / split | `src/map/builders/{cellular_automata,drunkard,split}.rs` | 718 | Alternative geometry builders | **EXTRACT-AS-IS** |
| Floor generation | `src/map/generate.rs` | 498 | Assembles the one chain for all 25 floors; `map_dims_for_depth`; portal swap | **GAME-SIDE** (the chain assembly is the game's content decision) |
| Prefab system | `src/map/prefab.rs` | 2483 | RON templates, rotate/flip orientations, weighted placement, connectivity rollback, ECS content spawn | **EXTRACT-WITH-REDESIGN** — the terrain half is generic, the content half is entirely fantasy |
| Dungeon lifecycle | `src/map/dungeon.rs` | 1744 | `Floor`, `FloorCache`, `MonsterCache`, transitions, `setup_world`/`teardown_world`/`spawn_dungeon` | **EXTRACT-WITH-REDESIGN** — the "cache floors by depth, regenerate on miss" pattern is generic; the implementation names monsters/items/props/webs/corpses explicitly |
| Fire simulation | `src/map/fire.rs` | 520 | Per-tile burn countdown, per-neighbour spread rolls, auras, ignition | **EXTRACT-WITH-REDESIGN** — genuinely a generic tile-CA, but named `fire`/`Burn`/`heat_factor` |
| Gas diffusion | `src/map/gas.rs` | 489 | Per-kind `u8` layers, double-buffered diffusion + dissipation, RNG-free | **EXTRACT-WITH-REDESIGN** — the diffusion kernel is theme-free; `GasKind::{Poison,Smoke}` is not |
| Tile promotion | `src/map/tile_promotion.rs` | 932 | On-enter + per-turn probabilistic tile state machines, occupancy guard, FOV-dirty reactor | **EXTRACT-WITH-REDESIGN** — full-map scan per turn (D-2), rules live in a `match` in `tile.rs` |
| Props runtime | `src/map/props/mod.rs` | 1616 | Data-driven `PropDef` with `Periodic`/`OnEnter` triggers and effect lists | **GAME-SIDE** (the trigger/effect vocabulary is combat-coupled) |
| Prop / item scatter | `src/map/props/spawn.rs` | 848 | Per-floor hazard + item scatter | **GAME-SIDE** |
| Webs | `src/map/web.rs` | 47 | One spawn helper for a floor object | **DROP** — 47 lines of glue, not a subsystem |
| FOV | `src/core/components.rs:693-784` | ~90 | `FovPlugin`, `fov_update_system`, smoke-dirty reactor | **EXTRACT-WITH-REDESIGN** — `HashSet<Point>` viewshed is the wrong data structure (D-3) |
| Shared components | `src/core/components.rs` | 1259 | ~60 components, most of them combat stats | **GAME-SIDE** except `Position`, `Collider`, `Viewshed`, `Name`, `Renderable` |
| Pathfinding | `src/actors/ai/pathfinding.rs` | 573 | A* wrappers, `HazardMap` cost overlay, `PathCache` + `step_toward_cached` | **EXTRACT-WITH-REDESIGN** — `PathCache` is excellent and generic; `HazardMap` hard-codes `FireField` |
| Auto-explore / travel | `src/actors/auto_explore.rs` | 1478 | `KnownMap` adapter, frontier detection, Dijkstra downhill stepping, stop rules | **EXTRACT-WITH-REDESIGN** — split the pure half (`KnownMap`, `is_frontier`, `compute_goals`, `downhill_step`, ~350 LOC) from the stop-rule half |
| Geometry helpers | `src/core/geometry.rs` | 638 | `Direction`, distances, Bresenham line, cone, disc, AoE | **EXTRACT-AS-IS** |
| Dice | `src/core/dice.rs` | 250 | `NdS+B` parsed at RON load, `Copy`, serde round-trip | **EXTRACT-AS-IS** |
| App state | `src/core/app_state.rs` | 62 | MainMenu → InGame → GameOver/Victory | **GAME-SIDE** (but the *pattern* should be a documented engine convention) |
| Log message | `src/core/log.rs` | 181 | `GameLogMessage` + C0-delimited inline colour spans | **EXTRACT-AS-IS** — the inline-span trick is neat and theme-free |
| Constants | `src/core/constants.rs` | 135 | Tile pixel size, Z ladder, `BASE_ACTION_COST` | **EXTRACT-WITH-REDESIGN** — should be a config struct, not consts (see C) |
| Renderer | `src/render/mod.rs` | 1125 | One `Text2d` entity per tile, lazy explored-tile spawn, FOV visibility sync, z-priority occlusion | **EXTRACT-WITH-REDESIGN** — the *rules* are good, the *backend* is wrong (D-4) |
| Particles | `src/render/particles.rs` | 1385 | Glyph particles with anchor/motion enums, floating numbers, bolts | **EXTRACT-WITH-REDESIGN** — motion/anchor model is generic; the spawn systems read `DamageEvent`/`DeathEvent` |
| Camera | `src/render/camera.rs` | 66 | Follow player by grid `Position` (deliberately not `Transform`) | **EXTRACT-AS-IS** |
| FX bridges | `src/render/{bump,fire,gas,status}_fx.rs` | 798 | Per-system glue from sim state to visuals | **GAME-SIDE** |
| Audio | `src/audio/mod.rs` | 477 | Embedded OGG, `SoundSettings`, `SfxEvent`/`SfxKind` | **EXTRACT-WITH-REDESIGN** — the embed + settings + event pattern is generic; `SfxKind`'s 12 variants are not |
| Save backend | `src/save/backend.rs` | 256 | `SaveBackend` trait, native `std::fs` + wasm `localStorage` impls, platform-erased resource | **EXTRACT-AS-IS** — the single best extraction candidate outside the builder chain |
| Save schema | `src/save/schema.rs` | 444 | Versioned `SaveGame`, no-migration version gate, `load_from_str` seam | **EXTRACT-WITH-REDESIGN** — the *versioning discipline* is the reusable part; the struct is 100% game content |
| Save capture/restore | `src/save/{capture,restore}.rs` | 1184 | World → `SaveGame` and back | **GAME-SIDE** |
| Save triggers | `src/save/triggers.rs` | 232 | Autosave on transition, `AppExit` autosave in `Last`, delete on death | **EXTRACT-AS-IS** (as a documented pattern; ~50 LOC of real logic) |
| Hall of Heroes | `src/save/hall.rs` | 423 | Leaderboard through the same backend | **GAME-SIDE** |
| wasm unload bridge | `src/save/web_unload.rs` | 114 | `beforeunload` thread-local cache + synchronous flush | **EXTRACT-AS-IS** — hard-won and non-obvious |
| Hordes | `src/actors/horde.rs` | 216 | Periodic reinforcement waves off `TurnEndEvent` | **GAME-SIDE** |
| Run stats | `src/actors/run_stats.rs` | 166 | Kills / turns / deepest floor / cause of death | **GAME-SIDE** |
| Examples | `examples/{balance_report,xp_pacing}.rs` | ~400 | Runnable analyses over the live RON content | **GAME-SIDE** — but the *pattern* (an example that loads the real registries through the same parse seams the game uses) is the best documentation practice in any of the three repos |

---

## B. Coupling & boundary analysis

### Purity map

**Pure (no Bevy `World`, testable standalone):**
- `src/map/grid.rs` (entire file), `src/map/builders/algorithms.rs`, `src/core/geometry.rs`,
  `src/core/dice.rs`, the RNG derivation math at `src/core/rng.rs:120-180`.
- `src/map/builders/*` builders — they touch `bevy::log::debug` and `bevy::platform::time::Instant`
  (`src/map/builders/mod.rs:47-48`) but never the ECS.
- Simulation kernels: `tick_gas_inner` (`src/map/gas.rs:174`), `tick_tile_promotions_inner`
  (`src/map/tile_promotion.rs:93`), `stamp_prefabs` (`src/map/prefab.rs:685`),
  `dequeue_next_batch_pure` (`src/core/turns.rs:163`), `downhill_step` (`src/actors/auto_explore.rs:325`),
  `find_path_hazard` (`src/actors/ai/pathfinding.rs:157`).

This pure-kernel-plus-thin-Bevy-wrapper split is the repo's strongest architectural habit and appears
consistently. `tick_gas` (`src/map/gas.rs:268`) is 6 lines wrapping a 90-line pure function; `tick_tile_promotions`
(`src/map/tile_promotion.rs:152`) is 18 lines wrapping `tick_tile_promotions_inner`. Copy this habit
into the engine as a rule, not an accident.

**Bevy-plugin-shaped:** `TurnOrderPlugin`, `FovPlugin`, `DungeonPlugin`, `WorldLifecyclePlugin`,
`RenderPlugin`, `SavePlugin`, `FirePlugin`, `GasPlugin`, `TilePromotionPlugin`, `ParticlesPlugin`,
`GameAudioPlugin`.

**Mixed (the problem cases):**
- `src/map/prefab.rs` — a pure builder (`PrefabStamper`, lines 904-990) and an ECS spawner
  (`spawn_prefab_content`, line 996) in one 2483-line file, importing `crate::ai`, `crate::combat`,
  `crate::items`, `crate::monster_data`, `crate::turns` (lines 55-72).
- `src/map/dungeon.rs` — `FloorCache`/`Floor`/`decide_transition` are engine-shaped; they sit beside
  `spawn_dungeon` (line 922) which despawns tile sprites, monsters, items, props, webs and corpses by
  concrete component type (`FloorEntities`, lines 51-78).
- `src/core/components.rs` — `Position`/`Viewshed`/`Collider` sit in the same file as `Phylactery`,
  `SummonPower`, `Lifesteal`, `Retaliate`, `ShieldBlock`, `CritThreshold`.

### Content leaking into engine-shaped code

1. **`SeedDomain` is a fantasy enum.** `src/core/rng.rs:80-113` — `Monsters`, `Items`, `Props`,
   `ItemScatter`, `Hordes`, `Combat`. The derivation math is perfectly generic; the tag vocabulary is not.
   living-world-rogue solved this correctly: `stage_seed(world_seed, "elevation")` takes a `&str` and
   FNV-1a hashes it (`lwr-world/src/rng.rs:56-64`), so a game names its own streams with zero engine edits.
2. **`Tile` is fantasy-bound and combinatorially exploded.** `src/map/tile.rs:22-58` — 17 flat variants
   including `Crystal`, `Lava`, `Portal`, `Fungus`, `TallFungus`, `TrampledFungus`. Adding a fourth
   vegetation family costs three variants, three `is_walkable` arms, three `is_opaque` arms, three
   `flammability` rows, three `promotion_rule` arms and three glyph/colour entries. roguelike_engine's
   layered `Tile { terrain, liquid, decoration }` with `#[non_exhaustive]` + `Custom { id: u32 }`
   (`roguelike_engine/src/map/tile.rs:24-42, 89-110, 120-140`) is strictly the better model and fantasy-rogue
   regressed away from it.
3. **`crate::constants` claims to be engine but isn't.** `src/core/constants.rs:1-14` explicitly documents
   the engine/game split and then holds `TILE_SIZE_X`, `CAMERA_ZOOM_SCALE`, and a `Z_MONSTER`/`Z_ITEM`/
   `Z_CORPSE`/`Z_WEB` ladder — the ladder enumerates game entity kinds.
4. **`ProcessingPhase` scheduling registers game intents.** `src/core/turns.rs:305-330` registers
   `FireBowIntent`, `EnchantIntent`, `ThrowIntent`, `PickupIntent`, `EquipIntent`… and
   `src/core/turns.rs:512-527` reads `Chilled`, `Hasted`, `Slowed`, `Berserking` components by name to
   compute the reinsert delay. The scheduler cannot move to an engine crate in this shape.

### Dependency direction problems

- `src/core/turns.rs:243-250` imports from `crate::actions`, `crate::combat::archery`, `crate::items::affixes`,
  `crate::spawner`, `crate::player`. The *core* module depends on four domain modules. This is the single
  worst inversion in the repo.
- `src/map/tile_promotion.rs:33-37` imports `crate::audio::{SfxEvent, SfxKind}` so a door close can play a
  sound. Map simulation depends on audio.
- `src/map/dungeon.rs:26-41` imports 15 crate paths spanning ai, combat, items, props, render, save, spawner, ui.
- `src/map/prefab.rs` imports `crate::turns::TurnManager` (line 72) so it can insert spawned monsters into
  the turn queue — map code reaching into the scheduler.
- Nothing enforces layering: `src/lib.rs:24-36` re-exports every leaf module at the crate root
  (`pub use crate::core::{...}`, `pub use actors::{...}`), which deliberately erases the folder boundary.
  It keeps call sites stable but means no compiler check exists for "map must not import combat".

### System-ordering contracts

The three-owner rule in CLAUDE.md is real and it works. `src/main.rs:90-113` owns cross-plugin placement
(`DungeonSet` into `ProcessingPhase::Cleanup`, `CombatEventSet` into `Cleanup`, one `InGame` gate over
six sets). `TurnOrderPlugin` owns the `Brain → ResolveMovement → ResolveActions → Cleanup` chain
(`src/core/turns.rs:331-344`). Plugins place their own systems.

Two problems with it as an engine seam:

- **The contract is a comment, not a type.** Nothing prevents a plugin from calling `configure_sets` on
  another plugin's set. `SavePlugin` explicitly notes it is obeying the rule (`src/save/mod.rs:74-77`) —
  which is exactly what a convention that needs enforcement looks like.
- **Ordering edges are stated as prose reasoning, densely.** `src/render/mod.rs:110-130`,
  `src/map/dungeon.rs:288-360`, `src/render/particles.rs:772-781` each carry 5-15 lines of comment
  justifying one `.after()`. That is very good documentation of a very fragile mechanism.

### Event/message pattern

`#[derive(Message)]` + `MessageWriter`/`MessageReader` is used consistently and is a good engine seam.
Notable instances: `TurnEndEvent` (`src/core/turns.rs:32`) as the universal per-turn tick that fire, gas,
tile promotion and hordes all subscribe to independently; `GasDepositEvent` (`src/map/gas.rs:150`) as a
single-tile deposit with deliberately no radius field, letting diffusion own all spreading;
`MapTransitionMessage`/`SpawnDungeonMessage` (`src/map/dungeon.rs:221,258`).

The pattern's weakness: message *ordering* relative to `Cleanup`-phase systems is load-bearing and
invisible. `resolve_turn_end` (`src/core/turns.rs:498`) has to defensively skip despawned entities
because `process_deaths` may have run first; `dequeue_next_batch_pure` needed an `is_alive` closure added
for the same race (`src/core/turns.rs:171-178`, absent in roguelike_engine's copy at
`roguelike_engine/src/turn/mod.rs:160`). Two independent workarounds for one unspecified ordering.

---

## C. Traits and extension points

### Existing traits

**`Grid2D`** (`src/map/grid.rs:26-47`) — near-perfect. Two required methods (`width`, `height`), three
provided (`xy_idx`, `idx_xy`, `in_bounds_xy`). Implemented by both `Map` and `Grid<T>`. Zero-cost, no
allocation, no lifetime noise. This is the model for what an engine trait should look like. roguelike_engine
has no equivalent — it duplicates `xy_idx`/`idx_xy` as inherent methods on `Map`
(`roguelike_engine/src/map/map.rs:47-54`), which is why its builders can't operate on scratch grids.

**`BuildContext` / `MapBuilder<C>` / `BuilderChain<C>`** (`src/map/builders/mod.rs:65-300`) — byte-for-byte
the same design as `roguelike_engine/src/map/builders/mod.rs:54-300`, with three fantasy-rogue improvements
worth keeping:
- `bevy::platform::time::Instant` instead of `std::time::Instant` (`src/map/builders/mod.rs:47`).
  roguelike_engine's version panics on wasm.
- `EngineBuilderMap::new()` (unseeded, `RandomNumberGenerator::new()`) was **removed**; only `with_seed`
  survives (`src/map/builders/mod.rs:110`). Determinism by construction.
- `FloorProfile` (`roguelike_engine/src/map/builders/mod.rs:200-220`) was dropped, which is a regression:
  fantasy-rogue now has literally one chain for all 25 floors (`src/map/generate.rs:79-135`), and TODO.md
  §4 tracks "biomes exist only as comment headers" as a consequence.

Design flaws in `BuildContext` as it stands:
- `fn rng(&mut self) -> &mut RandomNumberGenerator` (`src/map/builders/mod.rs:87`) hard-binds every builder
  to bracket-lib's concrete generator. Should be a trait.
- There is no way for a builder to *publish* structured output. `PrefabStamper` works around this with an
  `Arc<Mutex<Vec<PlacedPrefab>>>` side channel handed back from its constructor
  (`src/map/prefab.rs:904-923, 979-981`), documented as "because `BuildContext` is the engine's
  game-agnostic seam and prefabs are game content". That is the right *diagnosis* and the wrong *fix* — a
  mutex in single-threaded generation is a smell, and it means the placement list escapes the chain's
  ownership model.
- `rooms: Option<&Vec<Rect>>` (line 78) forces the room concept on cave builders that have no rooms.

**`BaseMap` / `Algorithm2D`** (bracket-lib) — used well. The wrapper-composition idiom is the best pattern
here: `SmokeOpacityMap` (`src/map/map.rs:200-250`) forwards everything but `is_opaque`, so dense smoke
blocks FOV symmetrically for player and monsters at *one* call site (`src/core/components.rs:770`).
`HazardMap` (`src/actors/ai/pathfinding.rs:88-150`) does the same for pathing costs, and `KnownMap`
(`src/actors/auto_explore.rs:168`) for explored-only traversal. Three independent overlays, one idiom,
no changes to `Map`. **This is the single most reusable idea in the map layer** and the engine should
formalize it as a first-class `MapOverlay` concept.

**`SaveBackend`** (`src/save/backend.rs:29-38`) — four synchronous methods, two cfg-split impls, exposed
through a platform-erased resource so no call site branches on `target_arch`. Clean, complete, correct.

**`DecorationRule` / `TilePredicate` / `Placement`** (`src/map/builders/decoration.rs:20-53`) — a small
declarative DSL that subsumed two bespoke builders. Data, not code. The right shape; only `target: Tile`
and `TilePredicate::Exact(Tile)` bind it to the concrete enum.

### Enum-extension strategies compared

| Repo | Strategy | Verdict |
|---|---|---|
| roguelike_engine | `#[non_exhaustive]` + `Custom { id: u32 }` on `TerrainType`/`LiquidType`/`Decoration` (`src/map/tile.rs:24-42,89-110,120-160`) | Works for *storage* but not *behaviour*: `TerrainType::Custom{..}.name()` returns the literal `"Custom"` (line 60), `flammability()` returns 0 (line 68), `timed_promotion()` returns `None` (line 73). The game gets a slot in the enum but no way to give it semantics — every custom tile is inert. |
| fantasy-rogue | Flat closed enum + free functions returning `Option<Rule>` (`src/map/tile.rs:60-232`) | Behaviour is fully expressible and the co-location comment ("adding a variant forces the author to decide whether it burns", line 160) is a genuine benefit. But it is closed: a game cannot add a tile without editing the engine. |
| living-world-rogue | String-keyed stages (`stage_seed(seed, "elevation")`) | Open, zero engine edits, no compile-time exhaustiveness. Right for seeds; wrong for hot-path tile lookups. |

None of the three is right for the engine. The correct answer is **a tile registry indexed by a newtype id**:
`TileId(u16)` in `Vec<TileId>`, with a `TileRegistry` resource holding `TileProps { walkable, opaque,
passable, obstacle, move_cost, glyph, flammability: Option<..>, promotion: Option<..> }`. Lookup is one
array index (faster than the current `matches!` chains), games register their own tiles at startup with
full behaviour, and the engine ships a small standard set. This also kills the combinatorial explosion:
`Grass`/`TallGrass`/`TrampledGrass` becomes one tile with a `trample_to` field.

### Traits that should exist but don't

1. **`RngSource`** — everything takes `&mut RandomNumberGenerator` concretely
   (`src/map/builders/mod.rs:87`, `src/core/dice.rs`, `src/map/builders/algorithms.rs:229`). The repo
   already runs *three* incompatible generators (bracket `RandomNumberGenerator` for combat, `rand::StdRng`
   for spawns/prefabs/fire, plus `FxRng`), converting between them by hand at every boundary. One trait
   with `next_u64`, `range`, `roll_dice`, `f32` would let all three coexist. living-world-rogue's
   hand-rolled SplitMix64 (`lwr-world/src/rng.rs`) exists precisely because `rand`'s stream stability is
   only guaranteed within a major version — a real long-term determinism hazard the engine must decide about.
2. **`TileSemantics`** — see the registry proposal above.
3. **`ContentRegistry<T>`** — `MonsterRegistry`, `ItemRegistry`, `PrefabRegistry`, `PropRegistry`,
   `AffixRegistry`, `LootPoolRegistry`, `ScatterTable`, `SpawnRules`, `ItemSpawnTable`,
   `ItemClassWeights` are ten separate resources with the same shape (embedded RON → parse seam → resource).
   `FloorContent` (`src/map/dungeon.rs:81-95`) is a `SystemParam` bundling eleven of them because the
   sprawl became unmanageable.
4. **`Renderer` / glyph back-end** — `src/render/mod.rs` hard-codes `Text2d` + `bevy_text`. A game wanting
   a tileset, a real terminal back-end, or a bitmap-font atlas has to rewrite the module. The *rules*
   (`tile_color`, one-glyph-per-tile z-priority, lazy explored-tile spawn) are back-end independent and
   should be separated from the Bevy entity mechanics.
5. **`WorldLifecycle` / `FloorContent` hook** — `spawn_dungeon` (`src/map/dungeon.rs:922`) hard-codes which
   entity kinds are floor-bound. An engine needs a `#[derive(FloorBound)]`-equivalent marker so teardown is
   automatic.
6. **A `save`-schema trait** — the versioning discipline in `src/save/schema.rs:30-58` (a bump comment per
   version, hard reject on mismatch, no migration) is a genuinely good policy that the engine should encode
   as a `Versioned` trait rather than re-derive per game.

---

## D. Performance review

### D-1. Worldgen: room accretion is O(attempts × W × H) with a per-attempt allocation

`src/map/builders/brogue.rs:444-463`. Every accretion attempt rescans the entire map for door sites,
allocating a fresh `Vec<(Point, Direction)>` and shuffling it:

```
while placed < target_rooms && attempts < 2000 && since_last_place < 150 {
    let mut dungeon_sites: Vec<(Point, Direction)> = Vec::new();
    for y in 1..self.height - 1 { for x in 1..self.width - 1 { ... } }
    shuffle_with_rng(&mut dungeon_sites, ctx);
```

Worst case on the terminal floor (96×60, `src/map/generate.rs:52-55`): 2000 attempts × 5640 interior
cells ≈ 11.3M `direction_of_door_site` calls plus 2000 allocations and shuffles. `terrain_cache` is only
`copy_from_slice`-refreshed on a *successful* placement (line 521), so consecutive failures rescan
identical data. Fix: maintain the door-site set incrementally, invalidated only in the neighbourhood of a
stamped room. Also note `is_fully_connected` (`src/map/builders/cleanup.rs:81-111`) allocates a full
`vec![false; tiles.len()]` per call, and `PrefabStamper` calls it once per wall-bearing placement attempt
(`src/map/prefab.rs:826-838`).

### D-2. `populate_blocked_tiles` defeats every `Map` change-detection gate in the game

This is the most consequential finding in my scope.

`populate_blocked_tiles` and `populate_costly_tiles` (`src/map/map.rs:258, 285`) take `ResMut<Map>` and
run **every frame** in `InGame` (`src/map/dungeon.rs:295-305`), unconditionally clearing and rebuilding
their arrays. In Bevy, `ResMut` deref marks the resource changed. Therefore `Map::is_changed()` is
**true on every frame**, which silently defeats:

- `ensure_explored_tile_sprites` (`src/render/mod.rs:269`) — guarded by `if !map.is_changed() { return; }`
  at line 277, so it iterates all `explored_tiles` and does a `HashMap` lookup per explored cell, 60×/second.
- `sync_tile_visibility` (`src/render/mod.rs:320`) — its `player_changed_q.single().is_err() && !map.is_changed()`
  guard (line 327) never short-circuits, so it sweeps every spawned tile sprite every frame, doing a
  `HashSet<Point>` lookup, an `OccludedTiles` lookup, a colour recompute and a glyph string compare per tile.
- `dirty_viewsheds_on_opacity_change` (`src/map/tile_promotion.rs:188`) — guarded by `if !map.is_changed()`
  at line 194, so it **hashes all ~5760 tiles' opacity through `DefaultHasher` every frame**.

Three full-map passes per frame that the code was explicitly written to avoid. The comment at
`src/render/mod.rs:304-310` even says "without this the system would mutably touch all ~4000 tile sprites
at 60 Hz" — it does. Fixes: rebuild `blocked` only when a `Position`/`Collider` actually changed
(`Query<..., Changed<Position>>` plus a removal detector), or move occupancy out of `Map` into its own
resource so terrain change-detection stays meaningful. The engine should never put mutable per-frame
occupancy in the same resource as immutable-per-floor terrain.

### D-3. FOV: `HashSet<Point>` viewshed

`src/core/components.rs:654-661`. `Viewshed.visible_tiles: HashSet<Point>` is rebuilt from scratch on
every dirty recompute (`src/core/components.rs:768-773`) — clear, allocate through
`field_of_view_set`, `filter`, `collect`. With up to 64 NPCs per batch (`MAX_NPC_BATCH`,
`src/core/turns.rs:158`) each holding one, plus `contains()` in the render sweep, this is the hottest
allocation in the game. A per-entity `Vec<bool>` or a bitset over map indices is ~10× cheaper for both
build and query, and the 71 `visible_tiles` reference sites would mostly keep working through a
`contains(Point)` method.

There is also a real hash-iteration-order dependence: `src/actors/auto_explore.rs:612-618` iterates
`visible_tiles` and uses `newly_sighted.get_or_insert(tile)`, so *which* staircase the stop message names
depends on `HashSet` iteration order. Cosmetic today; exactly the class of bug that becomes a
non-reproducible replay tomorrow.

### D-4. Renderer: one `Text2d` entity per tile

`src/render/mod.rs:283-300` spawns a `Text2d` + `TextFont` + `TextColor` + `Transform` + `Visibility`
entity per explored tile. On the terminal floor that is up to 5760 text entities, each paying Bevy's
per-entity visibility check, transform propagation and text layout/extraction. Lazy spawning (line 269)
and `set_if_neq` (lines 353, 365) are good mitigations but the data model is wrong for a grid renderer.
The engine should provide a single-mesh / instanced-quad glyph grid: one entity, one vertex buffer, one
texture atlas, updated from a `Vec<GlyphCell>`. That is a 100×-ish reduction in ECS pressure and makes a
terminal back-end trivially swappable.

`sync_entity_visibility` (`src/render/mod.rs:428`) allocates a `HashMap<(i32,i32), (f32,u64,Entity)>` and
a `HashSet<(i32,i32)>` every frame, ungated. Small in absolute terms, but it runs unconditionally.

### D-5. Tile promotion: full-map scan and RNG draw per turn

`src/map/tile_promotion.rs:99-116` iterates every tile every turn and calls `rng.0.rand::<f32>()` for each
tile that has a `per_turn` rule. With grass decoration on every floor (`src/map/generate.rs:98-110`) that is
hundreds to thousands of draws per turn — **on the shared combat `GameRng` stream**. Terrain regrowth
therefore advances the combat stream, which means the number of grass tiles on a floor perturbs every
subsequent combat roll. That works (it is deterministic), but it makes combat reproducibility hostage to
terrain, and it is the reason the code has to carefully roll *before* the occupancy guard (line 78-81
comment). Better: give promotion its own derived stream, and index promotion candidates instead of scanning.

### D-6. Auto-explore: full Dijkstra flood per step

`src/actors/auto_explore.rs:325-336` builds a fresh `DijkstraMap` over the whole map for every single
auto-explore step, plus `compute_goals` scanning every cell (line 289-297). At 60ms/step
(`STEP_INTERVAL`, line 55) this is fine in practice, but the engine should offer an incremental /
cached flow field since the goal set changes slowly.

### D-7. Gas diffusion

`src/map/gas.rs:174-268` is the best-optimized simulation here: integer `u8`, double-buffered per kind,
no RNG, no allocation in the tick, `std::mem::swap` between buffers. Two notes: it is a 9-neighbour
scan over `W×H×2` layers per turn (~104K cell visits on the terminal floor — fine turn-based, wasteful
if it ever ran per frame), and the diffusion is deliberately **not** mass-conservative (documented at
lines 206-222, gain is credited without debiting the donor). That is a signed-off design choice but it
means the model can't be reasoned about with a conservation invariant, which will bite whoever tunes it next.

### D-8. Pathfinding

`PathCache` (`src/actors/ai/pathfinding.rs:196-240`) is genuinely good: O(1) `next_step` via an index
probe of `idx`, `idx+1`, `idx-1`, with a documented ~95% A* reduction in flat chases. `HazardMap`'s
immune fast path (line 130-134) returns the base exits unmodified when `heat_factor == 0`, so the common
case costs one branch. `find_path_hazard` allocates a full `Vec<Point>` path per recompute; a
`SmallVec` or a reusable scratch buffer would help at 64 NPCs/batch.

### D-9. Benchmarks

**fantasy-rogue has none.** No `benches/`, no criterion dependency (`Cargo.toml:10-16`).
roguelike_engine has `benches/engine_benchmarks.rs` covering map generation, turn scheduling
(dequeue-100, insert-1000), combat math, GOAP planning, geometry and AI decisions
(`roguelike_engine/benches/engine_benchmarks.rs:6-197`). That harness is worth carrying forward
wholesale — but note it benchmarks none of the three actual hot paths above (FOV, per-frame render
sweeps, Dijkstra flood).

---

## E. Quality assessment

### Test coverage

Genuinely strong, and the *style* is more valuable than the coverage number. Categories:

- **Property-over-seed-range guards.** `every_floor_of_every_seed_keeps_its_exits`
  (`src/map/generate.rs:349-380`) walks 4 seeds × 25 floors asserting exactly one exit each, with a
  comment recording that the bug it guards soft-locked ~1 run in 3 (17.9% of floors lost a staircase).
  `domains_never_collide_on_the_same_floor` and `a_different_run_seed_moves_every_stream`
  (`src/core/rng.rs:270-320`) sweep all 9 domains × 25 floors.
- **Regression tests that name the bug they pin.** `game_rng_survives_ron_round_trip`
  (`src/core/rng.rs:243`), `stamp_never_overwrites_stairs_or_portal` (`src/map/prefab.rs:1919`),
  `a_throw_up_the_screen_moves_the_glyph_up_the_screen` (`src/render/particles.rs:799`),
  `consecutive_runs_in_one_process_never_repeat` (`src/core/rng.rs:341`).
- **Headless `App` tests.** `src/render/mod.rs:605-830` builds a minimal `App`, adds one system, and
  asserts `Visibility` outcomes. `src/map/tile_promotion.rs:520-930` does the same for promotion ticks.
  ~10 in tile_promotion alone.
- **Aesthetic guard tests.** `walls_read_brighter_than_the_floor`, `lava_is_the_reddest_terrain`,
  `remembered_tiles_are_dimmer_than_lit_ones` (`src/render/mod.rs:1010-1120`) assert on computed
  luminance/saturation. Unusual and effective.

Gaps: no test asserts that `Map::is_changed()` gating actually gates (D-2 would have been caught); no
perf regression tests; `spawn_dungeon` (the 400-line lifecycle function) has no direct test.

### Documentation

Module `//!` docs are excellent — dense, and they explain *why* rather than restating the code. The best
examples are `src/map/prefab.rs:1-46` (the two-halves split and why each owns its own RNG stream),
`src/core/rng.rs:1-40`, `src/map/gas.rs:1-14` (explicitly contrasting its diffusion model with the fire
model's object-injection model), `src/render/mod.rs:33-41` (the one-tile-one-glyph rule).

Weaknesses: no `#![deny(missing_docs)]` anywhere; no doc-tests (the RNG examples are ` ```ignore `,
`src/core/rng.rs:11,32`); `examples/` holds two analysis tools, not API demonstrations. For a library
crate that is the wrong shape — but the *pattern* of `balance_report.rs` (loading the real registries
through the same parse seams the game uses, `examples/balance_report.rs:12-16`) is the right idea for
engine examples too.

Stale docs are tracked honestly: TODO.md §8 is literally "Stale docs & dead code cleanup", and TODO.md
§0 carries eight fixed bugs with post-mortems plus two open ones.

### Determinism model

fantasy-rogue's is the best of the three, and it is not close:

- Every world stream is `mix64(mix64(seed ^ domain.salt()).wrapping_add(floor))`
  (`src/core/rng.rs:180-183`), so run seed, domain and floor all avalanche.
- The entropy source is documented with the mistake that produced it: Bevy's `Time` was tried first and
  produced identical dungeons under `FR_AUTOSTART=1` because elapsed-since-startup is constant at a fixed
  launch point (`src/core/rng.rs:190-200`). It now uses wall clock XOR a per-process run counter, with a
  test proving 500 consecutive in-process runs never repeat (`src/core/rng.rs:341`).
- Cosmetic randomness is *architecturally* separated: `FxRng` is documented as outside the determinism
  contract entirely, so FX systems need no ordering edges against gameplay (`src/core/rng.rs:352-370`).
  This is the insight roguelike_engine and lwr both lack.
- AI idle movement uses per-entity `StdRng` seeded from `entity.index()`, spawn placement uses per-floor
  `StdRng` — deliberately off the combat stream so combat stays predictable in tests.

Three real gaps:

1. **`FireRng` is not derived from `RunSeed`.** `src/map/fire.rs:103-108` seeds from the fixed constant
   `0xF19E_5EED` and is never reseeded (grep confirms: only `init_resource` at line 368 and three test
   insertions). Fire spread is therefore identical across every run of the game. There is no
   `SeedDomain::Fire`. This violates the stated invariant.
2. **Terrain promotion draws from the combat stream** (D-5), coupling combat reproducibility to grass count.
3. **Hash-iteration order leaks into output** (`src/actors/auto_explore.rs:612`, D-3), and `visible_tiles`
   is iterated at 7 sites.

Comparison: living-world-rogue's `stage_seed` is *more extensible* (string-keyed, no engine edit to add a
stage) and its hand-rolled SplitMix64 is *more durable* (a pinned reference vector test at
`lwr-world/src/rng.rs:82-90` guards against constant drift; `rand` only guarantees stream stability within
a major version). roguelike_engine has no seed-derivation model at all — builders take a bare
`seed: u64` and `EngineBuilderMap::new()` even offers an unseeded constructor
(`roguelike_engine/src/map/builders/mod.rs:106-118`). **The engine wants fantasy-rogue's separation
discipline, lwr's string-keyed openness, and lwr's hand-rolled generator.**

### Notable bugs and debt spotted in passing

- **D-2** (change-detection defeated by per-frame `ResMut<Map>`) — real, measurable, unreported.
- **`FireRng` never run-seeded** — real, contradicts CLAUDE.md.
- `src/map/prefab.rs` at 2483 LOC and `src/map/dungeon.rs` at 1744 LOC both blow the stated ~1000 LOC
  target in CLAUDE.md, and both are the mixed-purity files.
- `PrefabStamper`'s `Arc<Mutex<...>>` output channel (`src/map/prefab.rs:908`) — a mutex in
  single-threaded generation, because the trait has no output seam.
- `Tile::name()` (`src/map/tile.rs:61-79`) is a 17-arm string table maintained by hand alongside
  `is_walkable`, `is_opaque`, `is_passable`, `is_obstacle`, `flammability`, `promotion_rule`,
  `tile_glyph` and `tile_base_color` — eight parallel matches over one enum.
- `TODO.md` §0 open item: a corpse under its own loot is invisible because `Z_CORPSE < Z_ITEM` and dying
  monsters drop onto their own tile. A z-ladder is the wrong model for "this tile has several things on it".
- CI runs `clippy -D warnings` and the full test suite but no `cargo fmt --check` (deliberate, documented
  at `.github/workflows/ci.yml:8-10`) and no wasm build check, despite wasm being a shipping target.

---

## F. Recommendations for the new engine (ranked)

**1. Take fantasy-rogue's `BuilderChain` framework as-is, then add an output seam and a floor profile.**
Start from `src/map/builders/mod.rs:65-300` (not roguelike_engine's — fantasy-rogue's fixes the wasm
`Instant` and removes the unseeded constructor). Changes: replace `fn rng(&mut self) -> &mut RandomNumberGenerator`
with `fn rng(&mut self) -> &mut dyn RngSource`; add `fn emit<T: Any>(&mut self, value: T)` so builders can
publish structured results (killing `PrefabStamper`'s `Arc<Mutex>`); make `rooms()` optional-by-design
rather than `Option<&Vec<Rect>>`; restore roguelike_engine's `FloorProfile`
(`roguelike_engine/src/map/builders/mod.rs:200-220`) so one chain can express biome variation, which is
exactly what fantasy-rogue lost.

**2. Replace the flat `Tile` enum with a `TileId(u16)` + `TileRegistry` model.** Neither fantasy-rogue's
closed enum (`src/map/tile.rs:22-58`) nor roguelike_engine's `Custom { id }`
(`roguelike_engine/src/map/tile.rs:41`) works: the first can't be extended, the second extends storage but
not behaviour (every `Custom` tile is inert — `name()` → `"Custom"`, `flammability()` → 0,
`timed_promotion()` → `None`). A registry gives games full semantics, makes lookup an array index instead of
a `matches!` chain, and collapses the Grass/TallGrass/TrampledGrass explosion into one tile with a
`trample_to` field. Keep fantasy-rogue's four predicates (`is_walkable`/`is_opaque`/`is_passable`/`is_obstacle`)
as registry fields — the distinction between walkable and passable is load-bearing for connectivity checks
(`src/map/tile.rs:110-130`) and is the kind of thing a first-time engine author gets wrong.

**3. Adopt fantasy-rogue's turn scheduler, but only the pure half, and specify the death ordering.**
Take `src/core/turns.rs:15-215` verbatim: min-heap with insertion-order tiebreak, pure `dequeue_next_batch_pure`
with `is_player`/`is_alive` closures, `compute_reinsert_time`. It is strictly better than
`roguelike_engine/src/turn/mod.rs` (which is the same code minus the `is_alive` fix and with `MAX_NPC_BATCH`
at 16 instead of 64). **Do not** take `TurnOrderPlugin` — it registers 15 game intents and reads four named
status components to compute delay (`src/core/turns.rs:305-330, 498-527`). Instead: the engine provides
the queue plus a `SpeedModifier` trait or an `ActionCost` message the game fills in, and the engine
*specifies* that death resolution runs before requeue so the two defensive `is_alive` checks
(`src/core/turns.rs:171-178, 502-505`) become unnecessary rather than duplicated.

Also take the `ProcessingPhase` shape (`Brain → ResolveMovement → ResolveActions → Cleanup`) and the
three-owner ordering rule from CLAUDE.md — but make it enforceable: expose sets as an opaque type whose
`configure_sets` is only callable by the owning plugin, or at minimum document ordering edges as data the
engine can assert on at startup.

**4. Seed derivation: lwr's openness + fantasy-rogue's discipline + lwr's generator.**
Combine `stage_seed(world_seed, "elevation")` (`lwr-world/src/rng.rs:56-64`, string-keyed so games name
their own streams) with fantasy-rogue's per-floor `derive(domain, floor)` and its explicit
gameplay-vs-cosmetic split (`src/core/rng.rs:352-370`). Use lwr's hand-rolled SplitMix64
(`lwr-world/src/rng.rs:8-49`) with its pinned reference-vector test (line 82) rather than `rand` or
bracket's generator — stream stability across years is the whole point and `rand` only promises it within
a major version. Ship fantasy-rogue's four determinism property tests (`src/core/rng.rs:265-345`) as
engine tests. Fix the two gaps found here: every simulation stream (fire included) must derive from the
run seed, and terrain simulation must not share the combat stream.

**5. Formalize map overlays as a first-class engine concept.** `SmokeOpacityMap` (`src/map/map.rs:200-250`),
`HazardMap` (`src/actors/ai/pathfinding.rs:88-150`) and `KnownMap` (`src/actors/auto_explore.rs:168`)
are three independent instances of the same idiom: wrap `&Map`, forward all `BaseMap`/`Algorithm2D`
methods but one, compose a new view without mutating the map or adding a tile variant. This is how
lighting, sound propagation, faction knowledge and stealth should all be built. Provide a
`#[derive(MapOverlay)]` or a `struct Overlay<'a, F>` that forwards by default, so a game writes one
method instead of five.

**6. Fix the occupancy/terrain split before it becomes structural.** `Map` currently holds immutable
per-floor terrain (`tiles`), per-viewer knowledge (`explored_tiles`) and per-frame occupancy
(`blocked`, `costly_tiles`) in one Bevy resource, and the per-frame rebuild
(`src/map/map.rs:258-300`, scheduled at `src/map/dungeon.rs:295-305`) marks the whole resource changed
every frame, silently defeating three change-detection gates (D-2). In the engine: `Terrain` (per floor,
rarely mutated), `Occupancy` (per frame, spatial index), `Knowledge` (per viewer) as three resources.
Occupancy should be an incremental spatial index updated from `Changed<Position>` plus removal detection,
not a full rebuild — that also gives you the entity-at-tile query the game currently lacks (there is a
known trap here recorded in memory: the player has no `Collider` so it is never in `map.blocked`, and
player-facing occupancy logic has to query live `Position`s instead).

**7. Ship a real grid renderer crate, not per-tile text entities.** `src/render/mod.rs` spawns one
`Text2d` entity per explored tile (line 283) — up to 5760 on the terminal floor. Keep the *rules*
(`tile_color`'s lit/remembered hue split at line 537, the one-glyph-per-tile z-priority pass at line 428,
lazy explored-tile spawning at line 269) and put them behind a `GlyphGrid` abstraction backed by a single
instanced-quad mesh + font atlas. That makes a terminal back-end, a tileset back-end and the Bevy back-end
interchangeable, which matters a lot for a theme-agnostic engine.

**8. Extract `SaveBackend` verbatim and generalize the schema *policy*, not the schema.**
`src/save/backend.rs:29-38` is four synchronous methods with native/wasm impls behind a platform-erased
resource — take it unchanged. Add the `beforeunload` bridge (`src/save/web_unload.rs`, 114 hard-won
lines). Encode the versioning *discipline* from `src/save/schema.rs:30-58` (bump on shape change, reject
on mismatch, no migration path, one `load_from_str` parse seam) as a `Versioned` trait plus a doc page.
Do not try to make `SaveGame` itself generic — the capture/restore pair
(`src/save/{capture,restore}.rs`, 1184 LOC) is irreducibly game-specific and every attempt to generalize
"snapshot the world" produces reflection soup.

**9. Fix FOV's data structure and pin the ordering hazard.** Replace `Viewshed.visible_tiles: HashSet<Point>`
(`src/core/components.rs:656`) with a bitset or `Vec<bool>` over map indices, exposing
`contains(Point)`/`iter()` so the 71 existing reference sites port mechanically. Keep fantasy-rogue's
`dirty` protocol (only the FOV system clears it, `src/core/components.rs:775`; the smoke reactor at line
724 and the opacity reactor at `src/map/tile_promotion.rs:188` only ever set it) — it is a clean
invariant. Make `iter()` yield a deterministic order so bugs like
`src/actors/auto_explore.rs:612-618` can't recur. Benchmark FOV: it is the hottest path in the game and
neither repo measures it.

**10. Extract the tile-CA simulations as one parameterized system, not two.** `src/map/fire.rs` (spread
by per-neighbour roll, fuel countdown, promote to inert) and `src/map/gas.rs` (double-buffered diffusion,
dissipation) are two shapes of the same thing, and their module docs even define each in contrast to the
other (`src/map/gas.rs:1-14`). An engine `TileField<T>` with pluggable `spread`/`decay` kernels, driven off
one `TurnEndEvent`, covers fire, gas, water, blood, sound, scent and heat. Take gas's implementation
qualities (integer math, no allocation in the tick, double-buffered, RNG-free) as the default and make
randomized spread opt-in. Both fields also need floor-change lifecycle, which both currently hand-roll
(`src/map/fire.rs:174`, `src/map/gas.rs:295`) — that belongs in the engine.

**11. Split `auto_explore` and ship the pure half.** `src/actors/auto_explore.rs` is 1478 LOC, of which
~350 are genuinely generic: the `KnownMap` overlay, `is_frontier` (lines 251-282 — the diagonal-corner
rule is a subtle, correct fix for an infinite-oscillation bug worth carrying), `compute_goals`,
`downhill_step` with its explicit goal-plateau stamp (lines 335-341, working around bracket-lib not
zeroing seed depths) and its strict-descent guard. The stop rules (monster sighted, HP dropped, status
gained, stairs spotted) are game policy and should be a `trait ExploreInterrupt`.

**12. Provide a `ContentRegistry<T>` and one RON loading convention.** Ten separate registry resources
plus an eleven-field `SystemParam` to bundle them (`src/map/dungeon.rs:81-95`) is what registry sprawl
looks like at 92K LOC. The engine should ship: a registry type generic over the def, a
`load_from_str` parse-seam convention (fantasy-rogue does this consistently and correctly —
`load_prefabs_from_str` at `src/map/prefab.rs:184`, `PrefabRegistry::embedded()` at line 173 so tests and
the game can never diverge), and `include_str!`-based embedding as the default so wasm needs no filesystem.

**13. Carry roguelike_engine's criterion harness forward and extend it to the real hot paths.**
`roguelike_engine/benches/engine_benchmarks.rs` covers mapgen, turn scheduling, combat math, GOAP,
geometry and AI decisions. fantasy-rogue has zero benchmarks despite being where the perf problems
actually are. Add: FOV recompute at 64 actors, Dijkstra flood on a 96×60 map, the per-frame render sweep,
gas tick, A* with and without `PathCache`.

**14. Take fantasy-rogue's test *style* as an engine requirement.** Specifically: property tests over seed
ranges (`src/core/rng.rs:270-345`, `src/map/generate.rs:349-380`), guard tests that assert content
coverage over the live data (`every_floor_actually_receives_prefabs`, `src/map/generate.rs:382`), headless
`App` tests for system wiring, and regression tests whose comment records the observed failure rate
(the 17.9%-of-floors staircase bug, `src/map/generate.rs:333-347`). Add `#![deny(missing_docs)]` and real
doc-tests, which fantasy-rogue lacks entirely (its RNG examples are ` ```ignore `, `src/core/rng.rs:11`).

**15. Enforce layering with crate boundaries, because conventions did not hold.** `src/lib.rs:24-36`
deliberately re-exports every leaf module at the crate root, erasing the folder split — and
`src/core/turns.rs:243-250` (core importing combat, items, actions, player) and
`src/map/tile_promotion.rs:33` (map importing audio) show what happens next. In a multi-crate workspace
those imports simply won't compile.

### On the theme question (criterion #4)

**Themes belong entirely in the game repo. The engine should ship zero content and one worked example.**

The evidence in these three repos is unambiguous. roguelike_engine tried to be theme-flexible by adding
`Custom { id: u32 }` escape hatches to its tile enums, and the result is an extension point that stores a
custom tile but cannot give it behaviour — `name()`, `flammability()` and `timed_promotion()` all fall
through to inert defaults (`roguelike_engine/src/map/tile.rs:56-80`). fantasy-rogue went the other way and
put its theme *in* the shared types: `SeedDomain::{Monsters, Items, Hordes}` (`src/core/rng.rs:80-113`),
`Tile::{Crystal, Lava, Portal, Fungus}` (`src/map/tile.rs:33-58`), `Z_MONSTER`/`Z_CORPSE`/`Z_WEB` in a
module whose own doc comment says it holds only engine values (`src/core/constants.rs:1-14`).
A sci-fi game on that base would be editing engine enums on day one.

The seam that actually works in practice is the one used three times independently and by accident: a
**registry keyed by an opaque id, plus a trait for behaviour**. Prefabs use it (`PrefabRegistry` +
RON, `src/map/prefab.rs:173-190`), monsters use it (`MonsterDefId(String)` pointing back at a registry),
props use it. The engine should make that the *only* extension mechanism: `TileId` + `TileRegistry`,
`ActorId` + `ActorRegistry`, string-keyed seed stages, and traits for anything with behaviour.
`#[non_exhaustive]` + `Custom { id }` should not appear in the new engine at all — it is strictly worse
than a registry and it makes every `match` a lie.

Two carve-outs. First, the engine should ship one *worked* theme in `examples/` — a complete, playable,
few-thousand-line fantasy crawl — because an engine with no runnable game is an engine whose seams have
never been tested, and criterion #5 asks for exactly this. Second, `roguelike_engine`'s existing
`lighting/`, `factions/`, `squad/`, `stealth/`, `status/` and `goap/` modules are *mechanics*, not themes,
and belong in the engine as opt-in crates — the theme is which factions exist, not that factions exist.

### On the multi-crate split

Layered so each crate compiles without the one above it, which is the only mechanism that will actually
prevent the `core → combat` import that already happened here:

| Crate | Contents | Depends on |
|---|---|---|
| `rl-core` | `Grid2D`, `Grid<T>`, `Position`, `Direction`, geometry, distances, Bresenham, `DiceRoll`, `RngSource` trait + SplitMix64 impl + `stage_seed` | nothing (no Bevy) |
| `rl-map` | `Terrain`, `TileId`/`TileRegistry`, overlay trait, `BuildContext`/`MapBuilder`/`BuilderChain`/`BuilderPhase`, all builders, `algorithms`, connectivity | `rl-core` |
| `rl-field` | `TileField<T>` (fire, gas, water, sound, scent, heat) with pluggable spread/decay kernels; tile promotion | `rl-core`, `rl-map` |
| `rl-fov` | Shadowcasting FOV over an opacity source, bitset viewsheds, lighting | `rl-core`, `rl-map` |
| `rl-path` | A*, Dijkstra maps, flow fields, `PathCache`, cost overlays | `rl-core`, `rl-map` |
| `rl-turn` | `TurnManager` heap, pure dequeue, cost/speed model | `rl-core` (Bevy `Entity` only) |
| `rl-ecs` | Bevy plugins wiring the above: `ProcessingPhase` sets, FOV plugin, field plugins, occupancy index, floor lifecycle | all of the above + Bevy |
| `rl-render` | `GlyphGrid` abstraction + Bevy back-end, camera, particles, visibility rules | `rl-ecs` |
| `rl-save` | `SaveBackend` trait, native + wasm impls, `beforeunload` bridge, `Versioned` policy | `rl-core` |
| `rl-content` | `ContentRegistry<T>`, RON load-seam convention, `include_str!` embedding | `rl-core` |
| `roguelike` | Facade re-exporting a curated prelude | all |

The critical boundary is `rl-core` … `rl-path` staying Bevy-free. That is what makes them benchmarkable
with criterion, testable without an `App`, and usable from a worldgen tool or a server. fantasy-rogue
already proves this is achievable — `algorithms.rs`, `grid.rs`, `geometry.rs`, `dice.rs` and the four
simulation kernels are already pure. It just never drew the line where the compiler could see it.
