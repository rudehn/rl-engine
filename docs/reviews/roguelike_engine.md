# Code review: `roguelike_engine` (~15.7k LOC, Bevy 0.17)

Reviewed at `/Users/nathanrude/Development/roguelike_engine`, 61 `.rs` files, 15,706 LOC.
Build state: `cargo test --offline` → **457 passed, 0 failed, 0.29 s**. `cargo bench --no-run` → builds clean.
`cargo run --example minimal_roguelike` → runs to completion. 4 compiler warnings (all load-bearing, see §C).

## The headline fact

**`fantasy-rogue` does not depend on this crate.** `Cargo.toml` in the game has no `roguelike_engine`
entry. The extraction was performed, published as a crate, and then the game it was meant to serve
went on to reimplement the same modules itself — `fantasy-rogue/src/core/turns.rs` (1179 LOC) contains
byte-level twins of this crate's `dequeue_next_batch_pure` / `compute_reinsert_time` / `add_entity` /
`insert_at`, plus the ~900 LOC of phase machinery the engine never extracted.

That is the single most important datum for designing the next engine. The failure mode was not
"the code was bad". The code is tidy, tested, and documented. The failure mode was **the engine
extracted the data structures and left the behaviour in the game**, so adopting it would have meant
carrying a dependency that solved none of the hard problems. Everything below is really a
elaboration of that one point.

---

## A. Subsystem inventory

| Subsystem | Path | LOC | Purpose | Verdict |
|---|---|---|---|---|
| Builder framework | `src/map/builders/mod.rs` | 577 | `BuildContext`, `MapBuilder<C>`, `BuilderChain<C>`, `BuilderPhase`, `FloorProfile` | **EXTRACT-WITH-REDESIGN** — best idea in the crate, four fixable flaws (§C1) |
| BrogueLike generator | `src/map/builders/brogelike.rs` | 1069 | Room-accretion generator w/ caverns, hallways, reward room, loops | **EXTRACT-WITH-REDESIGN** — algorithmically good, O(attempts×W×H) inner loop (§D1) |
| Choke-point analysis | `src/map/builders/choke_map.rs` | 760 | Brogue `Architect.c` loop-pruning + choke values | **EXTRACT-WITH-REDESIGN** — genuinely rare/valuable, quadratic pruning loop (§D2) |
| Grid algorithms | `src/map/builders/algorithms.rs` | 574 | `Grid<T>`, cellular automata, flood fill, blob gen | **EXTRACT-WITH-REDESIGN** — `randomize_grid` breaks determinism (§E3) |
| Lake builder | `src/map/builders/lake_builder.rs` | 553 | BrogueCE designLakes/fillLakes/createWreath | **EXTRACT-WITH-REDESIGN** — inherits the blob nondeterminism |
| Turn scheduling | `src/turn/mod.rs` | 689 | `TurnManager` (BinaryHeap), `dequeue_next_batch_pure` | **EXTRACT-AS-IS** — strongest module; needs the phase machine added (§F2) |
| GOAP planner | `src/ai/goap.rs` | 668 | Forward A* planner over boolean world state | **EXTRACT-WITH-REDESIGN** — `WorldState` is 15 hardcoded fantasy booleans (§C3) |
| Squad coordination | `src/squad/mod.rs` | 561 | Alert propagation, morale, blackboard, roles | **GAME-SIDE** — balance constants + fixed 7-role vocabulary, half-built |
| Tile types | `src/map/tile.rs` | 622 | 3-layer `Tile`, walkability/opacity/promotion | **EXTRACT-WITH-REDESIGN** — semantics hardcoded in `match`, kills `Custom` (§C2) |
| Map resource | `src/map/map.rs` | 609 | `Map`, `MapWithMode`, bracket-lib `BaseMap`/`Algorithm2D` | **EXTRACT-WITH-REDESIGN** — `MapWithMode` must become a trait (§C4) |
| Status effects | `src/status/mod.rs` | 501 | `StatusEffects`, tick, DoT, speed/damage modifiers | **EXTRACT-WITH-REDESIGN** — magnitudes hardcoded, `Custom` inert |
| Combat events | `src/combat/events.rs` | 457 | `DamageEvent`/`DeathEvent`/`HealEvent` + apply systems | **EXTRACT-WITH-REDESIGN** — damage pipeline is a closed 2-stage formula (§C5) |
| Map mutation | `src/map/mutation.rs` | 455 | Mutation messages + apply systems + `MapMutationSet` | **EXTRACT-AS-IS** — best seam design in the crate (§B4); strip moss/fungus |
| Combat math | `src/combat/mod.rs` | 442 | `DamageType`, `Resistances`, `Health`, pure helpers | **EXTRACT-WITH-REDESIGN** — `Resistances` is a 4-key HashMap in a hot path |
| Lighting | `src/lighting/mod.rs` | 440 | `LightMap`, Bresenham accumulation, `LightingPlugin` | **EXTRACT-WITH-REDESIGN** — O(r³), theme constants, forces global FOV redirty (§D3) |
| Abilities | `src/abilities/mod.rs` | 406 | `AbilityDef`, `TargetingRule`, cooldowns | **EXTRACT-WITH-REDESIGN** — closed effect vocabulary: dice + one status (§C6) |
| AI decisions | `src/ai/decisions.rs` | 395 | Pure tactical predicates (flee/kite/leash/erratic) | **EXTRACT-AS-IS** — model module; RNG injected, fully pure |
| Save framework | `src/save/mod.rs` + `platform.rs` | 472 | String-payload envelope, migrations, native/WASM I/O | **EXTRACT-WITH-REDESIGN** — no world serialization at all (§F6) |
| Geometry | `src/geometry/mod.rs` + `direction.rs` | 488 | Distances, `Direction`, AoE tiles | **EXTRACT-AS-IS** — `tiles_in_aoe` should return an iterator |
| Monster AI state | `src/ai/monster_ai.rs` | 309 | `MonsterAI` knob struct, 3-state mode | **GAME-SIDE** — a fixed behaviour model, not a framework |
| FOV system | `src/components/fov_system.rs` | 306 | bracket-lib shadowcast driver | **EXTRACT-WITH-REDESIGN** — `ResMut<Map>` serializes it (§D4) |
| Decoration propagator | `src/map/builders/decoration_propagator.rs` | 303 | Seeded BFS decoration spread from `DecorationRule` | **EXTRACT-AS-IS** — genuinely generic once `Decoration` is generic |
| Pathfinding | `src/ai/pathfinding.rs` | 286 | A* wrappers over bracket-lib | **EXTRACT-WITH-REDESIGN** — no Dijkstra map / flow field at all (§D5) |
| Doors / exits / cullers | `finish_doors`, `exit_points`, `*_culler`, `corridors`, `room_drawer`, `start_point`, `cave_eroder`, `bsp_dungeon` | ~1100 | Post-processing builders | **EXTRACT-AS-IS** — clean, small, well tested |
| Tile promotion | `src/map/promotion.rs` | 239 | Per-turn timed promotions | **EXTRACT-WITH-REDESIGN** — fresh OS-seeded RNG per turn (§E3), full-map scan |
| Awareness | `src/stealth/awareness.rs` | 246 | Per-perceiver/target awareness state machine | **EXTRACT-WITH-REDESIGN** — `HashMap` tie-break nondeterminism |
| Factions | `src/factions/mod.rs` | 218 | String-keyed symmetric hostility matrix | **EXTRACT-WITH-REDESIGN** — 2 `String` allocations per lookup (§D6) |
| Stealth probability | `src/stealth/probability.rs` | 56 | Opposed d20 notice probability | **GAME-SIDE** — d20 is a system choice, not an engine primitive |
| Noise map | `src/stealth/noise.rs` | 92 | Per-tile noise + decay | **DROP** — dead stub, "always returns 0 in practice" (`noise.rs:1-5`) |
| `SaveEnvelope` type | `src/save/mod.rs:70` | — | Versioned wrapper | **DROP** — derives Serialize, never constructed; format is hand-rolled |
| Components | `position`, `viewshed`, `name`, `inventory`, `collider`, `patrol_route`, `movement_mode`, `faction` | ~500 | Shared ECS components | Mixed — `Position`/`Collider` **AS-IS**; `Name` collides with `bevy::Name`; `Inventory` (22 LOC) is not worth shipping |
| Constants | `src/constants.rs` | 67 | Tile size, Z-layers, base action cost | **DROP** — pixel sizes and Z-layers in a *headless* engine |
| Benchmarks | `benches/engine_benchmarks.rs` | 206 | 11 criterion benches | **EXTRACT-WITH-REDESIGN** — measures the wrong things (§D8) |
| Example | `examples/minimal_roguelike.rs` | 267 | Headless 10-turn demo | **DROP** — demonstrates nothing moving (§E5) |

---

## B. Coupling and boundary analysis

### B1. Purity split is real and mostly good

Genuinely pure (no Bevy `World`, no ECS): `combat/mod.rs` math helpers, `geometry/`, `dice/`,
`ai/decisions.rs`, `ai/goap.rs`, `ai/pathfinding.rs`, `turn/mod.rs`'s `dequeue_next_batch_pure` +
`compute_reinsert_time`, `stealth/probability.rs`, and the whole `map/builders/` tree.

Bevy-plugin-shaped: `CombatPlugin`, `StatusEffectPlugin`, `AbilityPlugin`, `FovPlugin`,
`LightingPlugin`, `MapMutationPlugin`, `TilePromotionPlugin`, `SquadPlugin`, `FactionsPlugin`,
`StealthPlugin`.

Mixed: `map/map.rs` (pure `Map` + `populate_blocked_tiles` system), `squad/mod.rs`,
`status/mod.rs`, `combat/events.rs`.

The pure/impure line is drawn in a defensible place and the pure half is where the test mass lives.
Keep this. It is the crate's most transferable habit.

### B2. Content leaking into engine code

- **`src/ai/goap.rs:57`** — literal comment `// --- Kobold hoarder ---` above `at_hoard: bool`.
  `WorldState` also carries `can_cast_useful_spell` (`:68`) and `adjacent_to_chest` (`:62`).
  A sci-fi game inherits 15 booleans of which perhaps 6 apply.
- **`src/lighting/mod.rs:113-134`** — `FUNGAL_LIGHT_*`, `PHOSPHORESCENT_MOSS_LIGHT_*` constants and
  the constructors `fungal_light()` / `phosphorescent_moss_light()`, both re-exported from
  `src/prelude.rs:130-137`. An engine prelude should not contain the word "moss".
- **`src/map/mutation.rs:213-218`** — `apply_decoration_mutations` special-cases
  `Decoration::PhosphorescentMoss` to register/unregister a light. Engine plumbing hardcoding one
  content item's behaviour.
- **`src/map/tile.rs:124-160`** — `Decoration` ships `Fungus`, `Moss`, `Cobweb`, `Bloodstain`,
  `Embers`, `CrackedFloor`, `PhosphorescentMoss`. `TerrainType` ships `Portal` documented as
  *"Escape portal on the final floor"* (`tile.rs:38`) — a specific game's win condition, in the
  engine's terrain enum.
- **`src/squad/mod.rs:294-315`** — `SquadRole::{Scout, Guard, Flanker, Bodyguard, Skirmisher,
  Support, Commander}`, a D&D party composition with no `Custom` variant.
- **`src/stealth/probability.rs`** — the d20 opposed-roll model is baked in as the only option.
- **`src/constants.rs:22-28`** — `TILE_SIZE_X/Y = 16` and `Z_PLAYER/Z_MONSTER/Z_ITEM` float layers
  in a crate whose own doc says *"the engine is headless; games own their sprites"* (`lib.rs:55`).

### B3. Dependency direction

No inversions — nothing engine-ish imports anything game-ish, because there is no game in the repo.
But two dependencies are unjustified:

- **`petgraph`** is used in exactly one place, `src/map/builders/bsp_dungeon.rs:88-122`, to hold a
  BSP tree of `Rect`s. `README.md:57` claims it is for *"graph analysis for choke-point detection"* —
  false; `choke_map.rs` uses hand-rolled BFS and never imports petgraph. A whole graph crate for a
  tree that a `Vec<Rect>` with parent indices would hold.
- **`rand`** is used in exactly one place, `src/map/builders/algorithms.rs:137`, where it introduces
  the determinism bug in §E3. Removing that line removes the dependency.

`src/stealth/awareness.rs:1-2` cites a spec path inside a *different, private* repo
(`bevy_rpg's docs/superpowers/specs/...`) — a dangling doc reference in a would-be published crate.

### B4. System-ordering contract

The contract is stated consistently and is a good idea: every plugin declares an empty marker
`SystemSet` (`CombatEventSet`, `StatusEffectSet`, `AbilitySet`, `FovSet`, `LightingSet`,
`MapMutationSet`, `TilePromotionSet`, `SquadAlertSet`, `SquadReactionSet`) and refuses to call
`.after()` / `.run_if()` itself, leaving that to the game. `lib.rs:67-71` states it explicitly.

**But every plugin adds its systems to `Update` with no run condition.** Grep of all 10
`add_systems` call sites shows zero `run_if` in engine code. The engine's out-of-the-box behaviour
is therefore: FOV recomputes, lighting rebuilds, status effects tick, and ability cooldowns
decrement **60 times per second** in a turn-based game, including in the main menu where `Map` does
not exist. `status_effect_tick_system` (`status/mod.rs:255`) ticking per frame would burn a 10-turn
poison in 167 ms.

This is the exact anti-pattern `fantasy-rogue/CLAUDE.md` names as a hard rule
(*"Adding gameplay systems without `.run_if(in_state(AppState::InGame))` … ungated systems run in the
menu and crash on missing world resources"*). Defaults that are wrong for every consumer are not
neutral; they are a trap. A `TurnEndEvent`-driven schedule should be the default, with the freedom to
override.

### B5. Message pattern

`#[derive(Message)]` + `MessageWriter`/`MessageReader` is used well and is the right engine seam.
`map/mutation.rs` is the best example in the crate: request messages
(`TileMutationMessage` / `DecorationMutationMessage` / `LiquidMutationMessage`), engine apply systems
that do only universal data sync (Map ↔ tile entity ↔ `Viewshed.dirty` ↔ `LightSources.dirty` ↔
`Collider`), and an explicit contract that game reactions run `.after(MapMutationSet)` reading the
same messages (`mutation.rs:13-19`). Copy this pattern wholesale.

The combat pipeline uses the same machinery less successfully — see §C5.

---

## C. Traits and extension points

### C1. `BuildContext` / `MapBuilder<C>` / `BuilderChain<C>` — the good idea, four flaws

The shape is right: builders are generic over the context (`MapBuilder<C: BuildContext>`,
`builders/mod.rs:246`), so engine builders compose with a game's extended context, and
`BuilderChain<C>` holds it by value. `BuilderPhase` (`:232`) giving the pipeline a declared,
enforced ordering is a genuinely good idea I have not seen elsewhere.

Flaws, in severity order:

1. **`BuilderChain` is not `Send`.** `builders: Vec<(_, _, Box<dyn FnMut(&mut C)>)>`
   (`builders/mod.rs:283`) — no `+ Send` on the box, despite `MapBuilder: Send + 'static`
   (`:246`). Map generation therefore cannot be moved onto `AsyncComputeTaskPool`. At the measured
   18.5 ms/floor (§D1) that is a visible hitch on every stair descent, and it is unfixable
   downstream.
2. **`std::time::Instant` unguarded.** `builders/mod.rs:44,307,328,348,355`. The crate declares
   `wasm32` support (`Cargo.toml` target dep on `web-sys`; `save/platform.rs` has a full
   `localStorage` path), and `Instant::now()` panics on `wasm32-unknown-unknown`. **Calling
   `build_map()` in a browser build panics.** The timing is only for a `debug!` log.
3. **Builders cannot fail.** `fn build(&mut self, ctx: &mut C)` returns `()`. A generator that
   produces a disconnected map has no way to say so, and the chain has no retry. Real roguelike
   generators need `Result<(), BuildError>` and a "regenerate this floor" loop.
4. **Phase violations `assert!`** (`:317`) — a panic in a shipped game for what is a static
   property of the chain. Validate at `add()` or return `Result` from `build_map()`.

Two smaller ones: `rng(&mut self) -> &mut RandomNumberGenerator` (`:73`) hardcodes bracket-lib's RNG
into the public trait, and `BuildContext` is an 11-method trait games must implement by hand-written
delegation with no derive or blanket helper.

### C2. `#[non_exhaustive]` + `Custom { id }` — it does not work

This is the crate's stated extension strategy (`lib.rs:59-65`, `README.md:53`, `CLAUDE.md`). It
fails on three independent grounds.

**(a) The documentation is false.** 13 enums carry `#[non_exhaustive]`; only 6 have a `Custom`
variant. `lib.rs:63-64` promises `DamageType::Custom`, `DamageSource::Custom` and
`MovementMode::Custom`. None of the three exists. `MovementMode` (`components/movement_mode.rs:19`)
is `#[non_exhaustive]` with exactly three variants and no escape hatch, so a downstream game can
never add a flying or phasing creature.

**(b) `#[non_exhaustive]` does nothing inside the defining crate, and the compiler says so.**
The build emits:

```
warning: unreachable pattern
   --> src/map/tile.rs:320:9    |  _ => is_walkable(tile),
warning: unreachable pattern
   --> src/ai/monster_ai.rs:134:13  |  _ => "Active",
```

`monster_ai.rs:132-134` even carries the comment *"`#[non_exhaustive]`: fall back to a neutral label
if a future engine variant doesn't have one yet"* directly above an arm rustc proves is dead. The
attribute constrains *downstream* matches only; it buys the engine nothing internally, and the
defensive style it encourages produces dead code.

**(c) The fatal one: all semantics live in engine-side `match` arms, so `Custom` is inert.**
Every meaningful property of a tile is a method on the enum with a hardcoded match:
`is_walkable` (`tile.rs:294`), `is_opaque` (`:352`), `is_passable` (`:324`),
`is_pathing_blocker` (`:338`), `flammability` (`:59`, `:172`), `blocks_fov` (`:196`),
`movement_cost` (`:190`), `on_step_promotion` (`:201`), `timed_promotion` (`:212`),
`entangles` (`:250`).

For `Custom { id }` every one of these returns the conservative default:

```rust
TerrainType::Custom { .. } => false,   // tile.rs:308  is_walkable
LiquidType::Custom  { .. } => false,   // tile.rs:319  is_walkable
TerrainType::Custom { .. } => false,   // tile.rs:328  is_passable
```

So the answer to the brief's question — *does it work for a game that needs sci-fi terrain types?* —
is **no, and not marginally**. A sci-fi game adding `TerrainType::Custom { id: FORCE_FIELD }` gets a
tile that is not walkable, not passable, not opaque, non-flammable, has no promotion rule, and
reports its name as the literal string `"Custom"`. It cannot be walked on, seen through, pathed
around, or burned. The variant exists purely as a payload the game must intercept *before* any
engine code touches it — at which point the game has replaced the tile system, and the enum bought
nothing.

The same holds elsewhere. `StatusEffectKind::Custom` (`status/mod.rs:46`) cannot modify speed or
damage, because `compute_speed_modifier` (`:161`) and `compute_damage_modifier` (`:180`) check only
`Hasted`/`Slowed`/`Stunned`/`Strengthened`/`Weakened` by name — and hardcode the magnitudes
(`*= 0.5`, `*= 1.5`, `*= 100.0`) while ignoring the `magnitude` field the instance carries. Nor can
it deal damage over time; `status_effect_tick_system` (`:264`) matches `Burning`/`Poisoned` only.

The one place `Custom` genuinely works is `WorldStateProp::Custom` (`goap.rs:98`), because there the
value is *data* (`custom: BTreeMap<u32, bool>`, `goap.rs:73`) rather than a match arm. That is the
tell: **`Custom { id }` works exactly when the enum carries no behaviour**, which is exactly when you
did not need an enum.

**What to do instead.** Separate identity from semantics. Terrain identity becomes an opaque
`TerrainId(u16)`; semantics move into a `TerrainProperties { walkable, opaque, flammability,
move_cost, promotion }` record held in a `TileRegistry` the game populates from its own data files.
Engine code asks the registry, never matches. The engine ships a `standard()` registry with the
usual wall/floor/door/stairs so a fantasy game gets them free, and a sci-fi game registers force
fields, vacuum and airlocks that are first-class from the first frame. This also deletes the
`from_str` parsers (`combat/mod.rs:55`) that currently swallow unknown damage types as `Physical`.

### C3. GOAP `WorldState` — a struct where a bitset belongs

`WorldState` (`goap.rs:~45-74`) is 15 named `bool` fields plus a `BTreeMap<u32, bool>` escape hatch,
and `get`/`set` are two 16-arm matches (`:101-143`). The named fields are the game's vocabulary, not
an engine's (`at_hoard`, `adjacent_to_chest`, `can_cast_useful_spell`, `near_leader`).

Replace the whole thing with a `u128` bitset (or `[u64; N]`) whose bit indices the game names.
This removes the theme leak, deletes ~90 lines of match, makes `WorldState` `Copy`, and fixes the
planner's allocation problem in §D7 as a side effect.

### C4. `MapWithMode` should be a trait, and `fantasy-rogue` proves it

`MapWithMode<'a> { map, mode }` (`map/map.rs:169`) is a concrete struct implementing `BaseMap` with
a hardcoded three-arm cost function (`:176-232`). When `fantasy-rogue` needed gas clouds to affect
pathing it could not extend this — `fantasy-rogue/src/map/map.rs:211-212` shows its own wrapper
carrying `map: &'a Map` **and** `gas: &'a GasField`. The engine's abstraction was not reusable by the
one game that tried.

The right shape is a `trait PathingRules { fn cost(&self, map: &Map, idx: usize) -> Option<f32>; }`
taken as a parameter, with `MovementMode` as one implementation. This is precisely the brief's
criterion 7 (*take traits as parameters*) and the crate mostly does not do it: of the traits it
defines, only `MapBuilder` and `SaveMigration` are ever consumed generically, and
`dequeue_next_batch_pure(_, is_player: impl Fn(Entity) -> bool)` (`turn/mod.rs:161`) is the single
best example of the pattern in the codebase.

### C5. Combat has no extension point at all

`damage_application_system` (`combat/events.rs:105`) hardcodes the entire pipeline: subtract armor,
apply resistance percentage, subtract from `Health`, emit `DeathEvent`. There is no hook for a to-hit
roll, a crit, a dodge, a shield block, a sneak multiplier, or damage-type conversion.
`fantasy-rogue/src/combat/mod.rs` is 7,620 LOC and none of it can be expressed here; adopting the
engine would mean discarding `CombatPlugin` on day one.

Two bugs while in the neighbourhood:
- `combat/events.rs:130` — `if final_damage <= 0 { continue; }` silently drops fully-resisted hits.
  No message is emitted, so a game cannot log *"the flames wash over it harmlessly"* or fire on-hit
  effects for a zero-damage strike.
- Same line contradicts `combat/mod.rs:80` which documents resistance `>100` as *"absorb/heal
  (returns a negative value)"*. `apply_resistance` does return negative, and the apply system
  discards it. Documented absorb does not work.
- `combat/events.rs:139` hardcodes `suppression.0 = 3` — a balance number in engine code.

### C6. `AbilityDef` is a fireball description

`AbilityDef` (`abilities/mod.rs:68-88`) can express: optional damage dice, one damage type, one
optional status effect, one AoE radius. That is a fireball. It cannot express summon, teleport,
dig, polymorph, knockback, chain lightning, multi-effect, or anything conditional. There is also no
`AbilityRegistry` in the engine despite `AbilitySlot.def_id: u32` (`:112`) pointing at one — every
game must build the `u32 → AbilityDef` map itself.

The fix is `effects: Vec<E>` generic over a game-defined effect type (or `Vec<Box<dyn AbilityEffect>>`),
plus an engine-side registry type. `damage_dice: Option<String>` should be a pre-parsed
`Dice { n, sides, bonus }` — see §D9.

### C7. Traits that should exist and do not

| Missing trait | Why | Current situation |
|---|---|---|
| `TileRegistry` / `TerrainProperties` | Kills the `Custom` problem (§C2) | Hardcoded matches in `tile.rs` |
| `PathingRules` / cost function | Games need gas, ice, hazard avoidance | Concrete `MapWithMode` (§C4) |
| `DamageResolver` / pre-post damage hooks | Every game has its own combat math | Closed system (§C5) |
| `RngSource` | `BuildContext::rng` hardcodes bracket-lib | `&mut RandomNumberGenerator` |
| `ContentRegistry<T>` | Monsters, items, abilities all need id→def | Each game rebuilds it |
| `ActorStats` | HP is the only stat the engine knows | `Health` only |
| `WorldSnapshot` / save codec | See §F6 | Opaque `String` payload |
| Renderer/back-end trait | Games rewrite the ASCII/sprite layer | Nothing |

---

## D. Performance review

Measured on this machine, criterion, release:

| Benchmark | Time |
|---|---|
| `map_gen_brogelike_80x60` | 18.5 ms |
| `goap_plan_5_actions_2_goals` | 3.49 µs |
| `turn_insert_1000` | 4.96 µs |
| `turn_dequeue_100_entities` | 3.13 µs |
| `geometry_aoe_radius_5` | 375 ns |
| `combat_damage_modifiers_5` | 3.61 ns |
| `combat_full_pipeline` | 2.15 ns |
| `ai_should_flee` | 1.53 ns |
| `geometry_manhattan_distance` | 1.58 ns |

### D1. Map generation: 18.5 ms for one small floor

That is the *cheap* configuration — 80×60, `target_rooms: 8`, and `..Default::default()` means zero
hallway chance, zero erosion, no lakes, no decoration. A real 25-floor descent with lakes and erosion
will be several hundred milliseconds to seconds, on the main thread, in a chain that cannot be sent
to another thread (§C1.1).

The dominant cost is in `brogelike.rs:588-599`: for each of up to **2000** placement attempts, the
builder rescans the entire map to rebuild the candidate door-site list —

```rust
let mut dungeon_sites = Vec::new();
for y in 1..self.height - 1 {
    for x in 1..self.width - 1 {
        if terrain_cache[idx] == TerrainType::Wall
            && let Some(dir) = self.direction_of_door_site(...) { dungeon_sites.push(...) }
    }
}
shuffle_with_rng(&mut dungeon_sites, ctx);
```

2000 × 4800 tiles, with a fresh `Vec` allocation and a full Fisher-Yates shuffle each time, then for
each surviving site a `room_fits` scan of up to 40×40. Door sites change only near the last placed
room; maintain the list incrementally and dirty only the affected neighbourhood.

Two more in the same function:
- `brogelike.rs:664-666` copies all 4800 terrain values into `terrain_cache` after every successful
  placement, then `:677-680` **copies them again immediately**, under a comment that says
  *"Reuse terrain_cache (already synced after room placement loop)"*. The comment describes the
  correct behaviour; the code below it does the copy anyway.
- `brogelike.rs:737-739` — `let mut tiles_clone = ctx.map().tiles.clone();` … `add_loops(...)` …
  `ctx.map_mut().tiles = tiles_clone;`. A full map clone purely to satisfy the borrow checker,
  because `BuildContext` hands out the whole `Map` rather than a tile slice.

The shadow `terrain_cache` that must be manually resynced after every mutation is a correctness
hazard as much as a perf one.

### D2. `ChokeMap::generate` is quadratic in map area

`choke_map.rs:39-51` prunes non-loop tiles with a `while changed` loop that rescans the whole grid
on every pass. A long dead-end corridor is pruned one tile per pass, giving O((W·H)²) worst case —
~23 M operations at 80×60, and ~1.6 B at 200×200. This should be a worklist: on pruning a tile,
re-enqueue its eight neighbours; O(W·H) amortized.

Then `choke_map.rs:76-98` runs up to four flood fills per chokepoint, and
`flood_fill_count_with_block` (`:185-187`) allocates a fresh `vec![false; W*H]` on each call. A few
hundred chokepoints means ~1000 full-map allocations per generation. Use one scratch buffer with a
generation counter.

`ChokeMap` is called once, from `brogelike.rs:676`, and is otherwise unexposed — a valuable
algorithm with no public entry point.

### D3. Lighting is O(r³) per source, with an absurd default radius

`add_light_source` (`lighting/mod.rs:~318`) iterates every tile in the `(2r+1)²` bounding square and
runs an independent Bresenham `has_los` walk (`:306`) of up to `r` steps for each. That is O(r³) per
source where symmetric shadowcasting — which the FOV path already uses via bracket-lib — is O(r²).

`CANDLE_RADIUS = 30.0` (`lighting/mod.rs:111`) makes this concrete: 61×61 = 3,721 tiles, each with a
~30-step LOS walk, ≈ **110,000 opacity lookups for one candle**. On an 80×60 map of 4,800 tiles, a
single candle touches 78% of the map and does 23× more work than the map has cells. Whatever unit
`30.0` was meant to be, it is not tiles.

Worse, `rebuild_light_map_system` (`:293-296`) sets `dirty = true` on **every** `Viewshed` in the
world whenever any light changes, forcing a full FOV recompute for every actor. One flickering torch
re-runs shadowcasting for all fifty monsters. It also allocates two fresh `Vec`s per rebuild
(`:272-273`) rather than clearing in place.

Latent bug: `sync_entity_lights_system:190` short-circuits
`!added.is_empty() || removed.read().next().is_some() || !query.is_empty()`. When `added` is
non-empty, `removed.read()` is never called, so removal events are left unconsumed and lights that
were removed that frame are processed late or missed.

### D4. FOV is serialized on `ResMut<Map>`

`fov_update_system` (`components/fov_system.rs:62`) takes `ResMut<Map>` although it needs `&Map` for
the shadowcast and `&mut` only for `explored_tiles` (`:83-86`). That exclusive borrow prevents FOV
from running in parallel with anything reading `Map`, and forces a serial loop over all viewsheds.
Split `explored_tiles` into its own resource, take `Res<Map>`, and use `par_iter_mut`.

`Viewshed.visible_tiles` is a `HashSet<Point>` (`components/viewshed.rs:23`) — SipHash on every tile
inserted, on the hottest per-turn path, and a fresh `HashSet` allocated by `field_of_view_set` per
entity per update. A `Vec<Point>` or a bitset over map indices is both faster and deterministic
(§E4).

### D5. No Dijkstra map, no flow field

`ai/pathfinding.rs` is three thin A* wrappers. Fifty monsters chasing one player each run an
independent A* every turn. One Dijkstra map rooted at the player is O(W·H) once and answers all
fifty queries by gradient descent. bracket-lib provides `DijkstraMap`; the engine does not use or
re-export it. This is the largest single AI-performance win available and it is simply absent —
`fantasy-rogue`'s noise system reached for exactly this (Dijkstra sound propagation) and had to build
it in the game.

### D6. Faction lookups allocate two `String`s each

`FactionMatrix::get` (`factions/mod.rs:~95`):

```rust
self.relations.get(&(a.to_string(), b.to_string()))
```

Two heap allocations per hostility check, and `is_hostile_to` / `is_allied_to` / `is_neutral` each
call it. Target selection asks this per candidate per monster per turn. Intern to `FactionId(u16)` at
load and use a dense `Vec<Relation>` matrix — the roster is a dozen entries.

### D7. GOAP: 3.5 µs for a 5-action toy problem

That number is far too large for a search space this small, and the cause is allocation. Per expanded
edge, `search` (`goap.rs:212-260`) performs `node.state.clone()` (clones a `BTreeMap`),
`visited.insert(new_state.clone())` (another), and `node.actions.clone()` (a `Vec<usize>`) — three
allocations per edge. With the §C3 bitset `WorldState`, state becomes `Copy`, `visited` becomes
`HashSet<u128>`, and the planner becomes allocation-free. Fifty monsters replanning at 3.5 µs is
175 µs/turn on a five-action set; a realistic twenty-action set will be far worse and will scale
combinatorially on top of the allocation cost.

### D8. The benchmark suite measures the wrong things

Six of eleven benchmarks measure operations under 4 ns (`should_flee`, `manhattan_distance`,
`combat_full_pipeline`, `combat_damage_modifiers_5`, `flee_direction`). Meanwhile there is **no
benchmark for FOV, pathfinding, lighting, `ChokeMap`, `create_blob`, or `populate_blocked_tiles`** —
every one of the hot paths in §D1-D6. The suite's fastest and slowest entries differ by a factor of
8.8 million, and the slow end is represented by a single case in the cheapest possible configuration.

### D9. Smaller data-layout notes

- `Resistances(HashMap<DamageType, i32>)` (`combat/mod.rs:86`) — a hash map with at most four
  possible keys, hashed on every damage application. Use `[i32; N]` indexed by the type.
- `TileEntityIndex(HashMap<(i32,i32), Entity>)` (`map/tile_entity_index.rs:17`) — should be
  `Vec<Option<Entity>>` indexed by `xy_idx`; the key space is dense and known.
- `tile_promotion_tick_system` (`map/promotion.rs:74-114`) scans all W·H tiles every turn calling
  `timed_promotion()` twice each, and builds two fresh `HashSet<(i32,i32)>` per turn (`:71`, `:74`).
  Almost no tiles have promotion rules; keep a sparse active set updated on mutation.
- `squad_coordinator_system` (`squad/mod.rs:~355`) iterates *all* squad members once per squad
  leader and collects a fresh `Vec` each time — O(squads × members). Group by `SquadId` in one pass.
- `roll_dice_string` (`dice/mod.rs:35`) parses the dice string on every roll and returns `1` on parse
  failure, silently masking typos in content files. Pre-parse to a `Dice` struct at load.
- `notice_probability` (`stealth/probability.rs:6`) runs a 400-iteration double loop to compute a
  function of one integer in `[-20, 20]`. A 41-entry table or a closed form.
- `tiles_in_aoe` (`geometry/mod.rs:46`) allocates a `Vec<(i32,i32)>` per call (375 ns).
- `create_blob` (`algorithms.rs:~302`) runs 50 rounds, each doing `round_count` cellular-automata
  iterations, each of which does a full `grid.clone()` (`algorithms.rs:154`). Double-buffer instead.
- `TurnManager::remove_entity` (`turn/mod.rs:120`) rebuilds the entire heap via
  `into_iter().filter().collect()`, and `contains` (`:114`) is a linear scan. Fine at 50 actors,
  wrong shape at 500.

---

## E. Quality assessment

### E1. Tests: excellent quantity, narrow kind

457 tests in 0.29 s, zero failures, colocated `#[cfg(test)] mod tests` in nearly every file. The
`choke_map.rs` tests are the standout: an ASCII-art map fixture helper (`:236`) and hand-verified
cases (`ring_with_dead_end_spur`, `chokepoint_at_ring_junction`, `choke_value_asymmetric_corridor`).
`tile.rs` covers the walkability matrix exhaustively. `turn/mod.rs` pins FIFO tie-breaking.

What is missing is the kind of test that would have caught the bugs in this report:
- **No property-over-seed-range tests.** Not one test generates maps across a seed range and asserts
  an invariant (connectivity, stairs reachable, no unreachable rooms). `builds_without_panic`
  (`brogelike.rs:768`) asserts only that `rooms` is non-empty at seed 42. The nondeterminism in §E3
  would have been caught instantly by "same seed twice → identical map".
- **No headless `App` integration tests** exercising a plugin's real schedule. `fov_system.rs` builds
  an `App` six times (`:148-280`) but only to drive one system.
- **No guard tests** of the kind `fantasy-rogue` uses (`every_item_class_covers_every_floor`).

### E2. Documentation: excellent prose, zero verification, some falsehoods

Module-level `//!` docs are genuinely good — they explain *why*, not just what (`constants.rs:5-13`
on the engine/game split; `decisions.rs:18-20` on why `should_flee` uses strict inequality).

But: **all 7 doc-tests are `ignored`.** Every example in the crate is fenced ```rust,ignore``` — the
`lib.rs` quick start, the prelude example, all four `SystemSet` configuration snippets, the
`dice` example. Nothing verifies that a single documented example compiles. That is exactly how
`lib.rs:63-64` came to promise three `Custom` variants that do not exist, and how `README.md:57`
came to attribute choke-point detection to petgraph. There is no `#![deny(missing_docs)]`.

### E3. Determinism: two live holes, and the model is weaker than the game's

**Hole 1 — worldgen.** `algorithms.rs:137`:

```rust
pub fn randomize_grid<T>(grid: &mut Grid<T>, alive_percent: i32, floor_val: T, wall_val: T) {
    let mut rng = rand::rng();   // thread-local, OS-seeded
```

Reached from `create_blob` (`:310`) ← `generate_blob_on_full_grid` (`lake_builder.rs:62`) ←
`LakeBuilder` in the builder chain. **Every lake on every floor is different on every run, even with
a fixed seed.** The module doc four lines above (`algorithms.rs:5-7`) admits this in passing —
*"`randomize_grid` uses `rand::rng()` (thread-local) so tests that care about exact output should seed
externally"* — but there is no way to seed it externally; it takes no RNG parameter. The fix is
one line: take `&mut RandomNumberGenerator` like every other builder does, which also deletes the
`rand` dependency (§B3).

**Hole 2 — runtime.** `map/promotion.rs:68`, inside the per-turn system:

```rust
let mut rng = RandomNumberGenerator::new();   // fresh OS-seeded RNG, every turn
```

Door auto-close, grass regrowth, and cracked-floor collapse are therefore unreproducible from a run
seed, and a new PRNG is seeded on every turn boundary.

**Hole 3 — hash iteration.** `Viewshed.visible_tiles: HashSet<Point>` (`viewshed.rs:23`) uses std's
`RandomState`, so iteration order varies per process. Any game logic that iterates visible tiles to
pick a target is nondeterministic. Same exposure in `SquadBlackboard.roles: HashMap<Entity,
SquadRole>` and `reserved_positions: HashMap<Point, Entity>` (`squad/mod.rs:327,330`), and in
`Awareness::highest` (`awareness.rs:61`) where `max_by_key` over `.values()` breaks ties by hash
order.

**The model itself is thin.** `EngineBuilderMap::with_seed(depth, w, h, name, seed: u64)`
(`builders/mod.rs:124`) takes one bare `u64` for the whole floor. Every subsystem on a floor shares
one stream, so changing how many numbers the decoration pass draws silently reshuffles the lakes.

`fantasy-rogue/src/core/rng.rs` is dramatically better and should be lifted wholesale: a `RunSeed`
rolled once per run, with `derive(SeedDomain, floor) -> u64` folding a per-domain odd salt and the
floor through SplitMix64's finalizer (`rng.rs:112-155`). Nine independent domains (`Map`, `Prefabs`,
`PrefabContent`, `Monsters`, `Items`, `Props`, `ItemScatter`, `Combat`, `Hordes`), each documented
with *why* it is separate — the `ItemScatter` comment (`rng.rs:99-103`) explains that sharing a
stream with `Props` would mean retuning trap density silently reshuffled every rock. That is the
determinism model the new engine wants, and it is the clearest instance of the game having solved a
problem the engine did not.

### E4. Notable bugs and smells

- `Instant::now()` panics the whole build path on wasm32 (§C1.2).
- `Decoration::name()` returns `"TrumpledGrass"` / `"TrumpledFungus"` for `TrampledGrass` /
  `TrampledFungus` (`tile.rs:165-166`) — typo in a user-facing string.
- `Decoration::movement_cost` (`tile.rs:190`) is `match self { _ => 1.0 }` — a match with one
  wildcard arm returning a constant, consulted in three pathfinding cost functions
  (`map.rs:107`, `:218`, `:230`) that will never see anything but 1.0.
- `MapWithMode::get_pathing_cost` for `Land` returns `None` for `LiquidType::Water` (`map.rs:187`)
  before reaching `is_pathing_blocker` (`:189`) which would have returned `5.0` — dead branch, and it
  disagrees with `Map::get_pathing_cost` (`:104`) which *does* return 5.0 for water. Two cost
  functions, two different answers for the same tile.
- `squad/mod.rs:398`: `let has_healer = false; // TODO: detect healer role once assignments are
  wired up`, making `if has_healer { modifier += 0.1 }` provably dead. A `TODO` in source and a
  half-built feature. (`lake_builder.rs:365` has the other `TODO`.)
- `SaveEnvelope` (`save/mod.rs:70`) derives `Serialize`/`Deserialize`, is re-exported from the
  prelude, and is never constructed — the actual format is a hand-rolled four-line
  `format!("{}\n{}\n{}\n{}")` (`:97`).
- `save/platform.rs:19` writes to `saves/<key>.ron` relative to the current working directory, not a
  platform config dir, and `write_bytes` (`:26`) writes in place with no temp-file-and-rename, so a
  crash mid-write corrupts the save.
- Two unused-import warnings (`lighting/mod.rs:27`, `decoration_propagator.rs:14`).
- `engine::Name` (`components/name.rs`) collides with `bevy::prelude::Name`, forcing every consumer
  to qualify it — visible in the example at `minimal_roguelike.rs:22`.

### E5. The example demonstrates the core problem

`examples/minimal_roguelike.rs` runs. Its complete output over ten turns is four goblins and a hero
printing the same coordinates twenty times, every monster reporting `Sleeping` at the end, HP
untouched. Nothing moves, nothing fights, nothing is seen.

That is not a flaw in the example. It is an accurate demonstration of what the crate can do
unaided: it can schedule turns and print state. The engine ships `MonsterAI` but not the loop that
executes it (`monster_ai.rs:14-18` says so outright); it ships `TurnManager` but not `TurnState` or
the processing phases; it ships `AbilityDef` but no resolution; it ships FOV but nothing consumes it.
The example also has to hand-roll clock advancement (`:211`) because `dequeue_next_batch_pure` never
writes `current_time` despite the field's doc claiming *"Advances as actors are dequeued"*
(`turn/mod.rs:64`), and it exits via `std::process::exit(0)` (`:203`).

---

## F. Recommendations for the new engine, ranked

**1. Ship behaviour, not just data — or do not ship the subsystem.**
This is the lesson of the whole repo. `turn/` ships `TurnManager` but leaves `TurnState`,
`ProcessingPhase` and the state machine in the game (`turn/mod.rs:8-12` admits it); `ai/` ships
`MonsterAI` knobs but leaves `execute` in the game (`monster_ai.rs:14-18`); `abilities/` ships
`AbilityDef` but no resolution (`lib.rs:52`). `fantasy-rogue` reimplemented all three. For each
subsystem, decide either to own the loop or leave the subsystem out entirely. A struct definition
plus a `SystemSet` marker is not a subsystem.

**2. Lift the turn phase machine into the engine.**
Start from `fantasy-rogue/src/core/turns.rs:273-410`: `TurnState`, `ProcessingPhase`
(Brain → ResolveMovement → ResolveActions → Cleanup) chained and gated, `handle_turn_boundary`,
`select_next_actor`, `resolve_free_actions`, `resolve_turn_end`. Combine with this repo's
`BinaryHeap` `TurnManager` (`turn/mod.rs:57-121`), which is the better queue. Change
`DequeueOutcome::Empty` into `Empty` vs `WaitUntil(u32)` and have the dequeue advance the clock, so
games stop hand-rolling it.

**3. Replace `#[non_exhaustive] + Custom{id}` with registries.**
Per §C2. `TerrainId(u16)` + `TerrainProperties` in a game-populated `TileRegistry`, with a
`standard()` set shipped for convenience. Do the same for damage types, status kinds, and squad
roles. Keep `#[non_exhaustive]` only on enums with genuinely no behaviour, and delete the `_ =>` arms
the compiler already calls dead.

**4. On the theme question: themes live in the game, but the engine must ship a *default content
pack*.**
The evidence here is decisive in both directions. Theme words baked into engine types
(`Decoration::Fungus`, `TerrainType::Portal` as *"escape portal on the final floor"*,
`WorldState::at_hoard`, `SquadRole::Bodyguard`, `fungal_light()` in the prelude) are pure liability —
a sci-fi game inherits vocabulary it cannot use and cannot extend. But the opposite extreme is what
produced §E5: an engine so content-free that its own example cannot make a monster take a step.

Resolve it structurally rather than by degree. The engine crates define *mechanisms* and are
lexically theme-free — no fantasy word appears in `engine-*`. Themes ship as separate, optional
crates (`roguelike-content-dungeon`, `roguelike-content-scifi`) that are nothing but registry
populations: terrain tables, damage types, status kinds, builder chain presets. A fantasy game adds
one dependency and gets walls, doors, grass and fire; a sci-fi game adds a different one, or writes
its own. Critically, the *engine's* examples depend on a content crate, so they can be real playable
demos — which is what §E5 shows this repo lacked. That satisfies both the brief's criterion 4
(theme-agnostic) and criterion 5 (runnable examples) without the compromise that produced
`PHOSPHORESCENT_MOSS_LIGHT_COLOR` in a prelude.

**5. Multi-crate split.** Draw boundaries on dependency weight, not subject matter:

| Crate | Contents | Depends on |
|---|---|---|
| `rl-grid` | `Map`, `Tile`/registry, `Grid<T>`, geometry, `Direction`, FOV, pathfinding, Dijkstra maps, `ChokeMap` | **no Bevy**, no bracket-lib |
| `rl-rng` | `RunSeed`, `SeedDomain`, SplitMix64 derivation, dice | nothing |
| `rl-mapgen` | `BuildContext`, `MapBuilder`, `BuilderChain`, all builders | `rl-grid`, `rl-rng` |
| `rl-sim` | Turn scheduler, combat resolution traits, status, abilities, GOAP, squad | `rl-grid`, `rl-rng` |
| `rl-bevy` | Plugins, components, system sets, message wiring over the above | all + Bevy |
| `rl-content-*` | Registry populations per theme | `rl-grid`, `rl-mapgen` |

The load-bearing rule: **`rl-grid`, `rl-mapgen` and `rl-rng` must not depend on Bevy.** They are
already effectively Bevy-free here (`builders/` imports Bevy only for `debug!` logging at
`builders/mod.rs:41`), and cutting the dependency makes worldgen testable in milliseconds, usable
from a CLI tool, and trivially movable onto a thread.

**6. Solve world serialization, because neither repo did.**
This crate's save layer is an opaque `String` payload with a hand-rolled envelope (§A, §E4);
`fantasy-rogue` built `capture.rs` + `restore.rs` + `schema.rs` (~1,800 LOC) itself. The unsolved
core is entity-id stability — this repo hits it and gives up at `status/mod.rs:78`, where
`StatusEffectInstance.source` is `#[serde(skip)]` *"because `Entity` identifiers are not stable
across save/load cycles"*. Ship a `SaveId` component + remap table, a `WorldSnapshot` trait games
implement per component type, and atomic write-temp-and-rename in a real config directory. Keep the
version/migration design from `save/mod.rs:139-183`, which is sound.

**7. Make `BuilderChain` `Send`, fallible, and non-panicking.**
Add `+ Send` to the boxed builder (`builders/mod.rs:283`), change `MapBuilder::build` to return
`Result<(), BuildError>`, replace the phase `assert!` (`:317`) with validation at `add()`, and
remove `std::time::Instant` (`:44`) or gate it behind `cfg(not(target_arch = "wasm32"))`. Then add
the retry loop that fallible builders make possible. Base: `map/builders/mod.rs`, largely as-is.

**8. Adopt `fantasy-rogue`'s seed model verbatim.**
`fantasy-rogue/src/core/rng.rs:112-160` — `RunSeed::derive(SeedDomain, floor)`. Make it the *only*
way a builder or spawner obtains a seed; delete `EngineBuilderMap::with_seed(u64)`. Then add the
determinism test neither repo has: generate the same floor twice from one seed and assert byte
equality of `Map.tiles`. That single test catches §E3 holes 1 and 2 on the spot.

**9. Purge hash containers from hot and gameplay paths.**
`Viewshed.visible_tiles` → bitset over map indices. `Resistances` → fixed array.
`TileEntityIndex` → `Vec<Option<Entity>>`. `FactionMatrix` → interned `FactionId(u16)` + dense
matrix. `SquadBlackboard`'s maps → `Vec` sorted by a stable key. This is one refactor addressing
both §D6/§D9 (speed) and §E3 hole 3 (determinism) at once.

**10. Fix the three algorithmic complexity classes before anything else.**
Incremental door sites in the room-accretion loop (`brogelike.rs:588`, O(attempts×W·H) → amortized
O(1) per attempt); worklist loop-pruning in `ChokeMap` (`choke_map.rs:39`, O((W·H)²) → O(W·H));
shadowcast lighting instead of per-tile Bresenham (`lighting/mod.rs:318`, O(r³) → O(r²)) and set
`CANDLE_RADIUS` to something under 10. These three account for essentially all of the 18.5 ms and
all of the per-frame lighting cost.

**11. Bench the hot paths, not the arithmetic.**
Delete the sub-4-ns benchmarks. Add: FOV at 50 actors, A* and Dijkstra on a real generated map,
lighting rebuild at 20 sources, `ChokeMap::generate`, `create_blob`, and map generation at 80×60 /
160×120 / 320×240 with a *full* profile (lakes, erosion, decoration) to expose the superlinear
scaling. Base: `benches/engine_benchmarks.rs` structure is fine; the cases are wrong.

**12. Add a `PathingRules` trait and take it as a parameter.**
Per §C4, with `fantasy-rogue`'s gas-aware wrapper as the proof case. Same treatment for
`DamageResolver` in combat (§C5) and an `RngSource` abstraction replacing
`BuildContext::rng() -> &mut RandomNumberGenerator` (`builders/mod.rs:73`). The brief's criterion 7
is the one this repo satisfies least: `dequeue_next_batch_pure`'s `impl Fn(Entity) -> bool`
(`turn/mod.rs:161`) is nearly the only trait actually taken as a parameter.

**13. Replace GOAP `WorldState` with a bitset.**
Per §C3 and §D7 — one change that removes the theme leak, deletes ~90 lines of match arms, makes the
state `Copy`, and eliminates three allocations per search edge.

**14. Turn on doc-tests and `#![deny(missing_docs)]`.**
Un-`ignore` the seven examples (§E2). Three of the falsehoods in this report
(`lib.rs:63-64`'s phantom `Custom` variants, `README.md:57`'s petgraph claim, `turn/mod.rs:64`'s
"advances as actors are dequeued") are documentation drifting from code with nothing to catch it.
Add property-over-seed-range tests for every builder.

**15. Default to correct scheduling; let games override.**
Per §B4. Engine plugins should register into a turn-driven schedule gated on a
`run_if(engine_is_running)` condition by default, with the `SystemSet` markers still exposed for
reordering. The current contract — every system in `Update`, ungated, documented as the game's
problem — makes the out-of-the-box behaviour wrong for every consumer and, in a `MainMenu` state,
crashes on the missing `Map` resource.
