# rl-engine extraction plan

Status: adopted, revised 2026-09-09 after Nate's review; being built.

## Progress

- 2026-09-10: M0, M1 and M2 are built and committed; every crate through `rl-ai` exists, tier 1 is Bevy-free by CI.
- 2026-09-10: WildReach is abandoned as a repo.
  Corsair, the pirate example inside this workspace, is the consumer the milestones are built against from here on.
  The `wildreach` directory is left as it was and is not maintained.
- 2026-09-10: the turn loop runs every actor due before the player's next turn inside one frame (the `Turn` schedule and `EngineSet::Input`), after a Corsair run showed one actor per frame.
- 2026-09-10: M3 first slice: the slot graph in `rl-rules`, items in `rl-bevy` (ground, bag, slots, stacks, use as a game event), the list menu in `rl-ui`, and Corsair's armory, loot drops, rum and sea chest.
  Deferred from M3: affixes and enchant, throwing, the character sheet.
- 2026-09-10: M4 first slice: `rl-mapgen` gains rooms, BSP, doors, random start, farthest exit and prefab stamping; `rl-bevy` gains places (`MapId`, `OnMap`, `Transition`, `WarpRequest`, `PlaceRules`, map-scoped occupancy and knowledge, frozen off-map actors); Corsair gains huts in its ports, cave mouths on its coves, two-level smugglers' caves with a treasure vault, and portals that warp out of a cave.
  Deferred from M4: rivers and bridges inside chunks beyond the channel pass, cullers beyond keep-largest, a choke map, decoration rules, settlement passes richer than huts.
- 2026-09-10: M5 first slice: `rl-events` (facts with kind, subject, object and amount; matchers; a ledger of named counters; quests as objectives over facts with `after` chains and a `victory` flag, tracked purely), `rl-bevy` wraps it as the `Happened` message, the `Quests` and `Counters` resources and `QuestChange`, fed after the frame; the dead now linger until the end of the frame so reactions can read what they were.
  Corsair reports kills, pickups, what it carries, cave levels and sites as facts, loads four tasks from RON that chain to "Retire rich", narrates them, draws a ledger on `t`, and wins the run when the hoard comes home.
  Deferred from M5: scripted encounters, abilities and targeting, `TileField<T>`, lighting, a generated victory condition.
- 2026-09-10: M6 first slice: `rl-save` (the `SaveBackend` seam with file, memory and `localStorage` backends, the `Versioned` envelope with an exact-match policy, `EntityRemap` and `SaveId`, and `EngineSave` capturing and restoring the scheduler, the world's edits and places, and knowledge), `rl-tools` (`ThreatSubject`, threat, and the spawn-band `Report`).
  Corsair saves with `S`, saves and quits with `q`, continues with `--continue`, deletes the save on death, and prints its balance report with `--balance`.
  Deferred from M6: the wasm `beforeunload` bridge, seed replay, the headless dump.
- Next: the deferred pieces of M3 to M6 (affixes and enchant, throwing, a character sheet, abilities and targeting, `TileField<T>`, lighting, scripted encounters, the unload bridge), then the living-world-rogue conversion once the engine is done (Nate, 2026-09-10).
  That conversion keeps its overworld token movement, so `rl-overworld` regains travel on the map alongside the portal picker, and its maps stream as chunks.

Inputs: four Opus code reviews of the three source repos, kept beside this file.

- `roguelike_engine.md` - the prior extraction attempt (~15.7k LOC, Bevy 0.17).
- `living_world_rogue.md` - the overworld game with the Bevy-free `lwr-world` crate (~17.6k LOC, Bevy 0.19).
- `fantasy_rogue_core.md` - this repo's core, map, render, save and infra (~30k LOC).
- `fantasy_rogue_game.md` - this repo's combat, actors, items, ui and assets (~62k LOC).

Every claim below is backed by a `path:line` citation in one of those reports.
This document only synthesises and decides.

Decisions Nate has already made:

- The engine repo is `rl-engine`.
- The first consumer is a brand-new game, **WildReach**, not a port.
  It is a Sunlorn-like: ASCII, one continuous open world you walk across at a single scale, loot, dungeons, encounters, quests, town portals, and possibly a generated victory condition.
  The world is larger than 1024x1024 tiles.
  Content is added incrementally, so the engine's crates and milestones are ordered by what WildReach needs first.
- The second consumer is a small pirate-themed example game, **Corsair**, that lives in the `rl-engine` workspace.
- The overworld stays as a picture of how rivers, biomes and climate fit together, and as the place to pick a portal destination.
  It is an opt-in crate so WildReach can drop it later without touching anything else.
- Travelling far means walking there once to discover it, then using a portal to return instantly.
  Travel does not simulate time.
  There is no route-travel command.
- Rivers are in scope.
- No more questions for now; build, and course-correct later.
- `living-world-rogue` is source material for world generation, not the integration target.
- Randomness uses `rand`.
  Cross-version stream stability is not a goal because the game will evolve and break determinism anyway.
- GOAP is not ported.
- `rl-ui` is built in from the start.

## 1. The one lesson that matters most

`roguelike_engine` was extracted, published, tested (457 tests, 0 failures) and then not used.
This repo does not depend on it.
`src/core/turns.rs` holds a byte-level copy of its `TurnManager`, plus the ~900 lines of turn-phase machinery the engine never shipped.

The failure was not code quality.
The engine shipped data structures and left the behaviour in the game.
It shipped `TurnManager` but not the turn loop, `MonsterAI` knobs but not the execute step, `AbilityDef` but no resolution, FOV but nothing that consumed it.
Its own example runs ten turns in which nothing moves, fights or sees.
Adopting it solved none of the hard problems, so the game copied the easy parts and moved on.

Rule for `rl-engine`: **for every subsystem, either own the loop or leave the subsystem out.**
A struct plus a `SystemSet` marker is not a subsystem.

Corollary: the engine is developed against a real game from the first milestone.
Every milestone in section 8 is defined by a playable slice of the new game, and the engine ships only what that slice pulls on.

## 2. What the reviews agreed on

All four reviewers converged independently on these points.

1. **Themes live in the game.**
   The engine is lexically theme-free.
   No fantasy word appears in an engine crate.
2. **`#[non_exhaustive]` + `Custom { id }` does not work and must not appear.**
   In `roguelike_engine` every custom tile is inert: not walkable, not opaque, not flammable, named `"Custom"`.
   The compiler even flags the defensive `_ =>` arms as unreachable inside the defining crate.
   `Custom` works only where the enum carries no behaviour, which is exactly where an enum was not needed.
3. **Closed enums do not work either.**
   This repo has thirteen closed taxonomy enums (`DamageType`, `StatusEffect`, `EquipEffect`, `ItemCategory`, `EquipSlot`, `Faction`, `TacticId`, ...).
   `lwr-world` has eight exhaustive tables over `Biome`.
   A sci-fi game cannot add `Radiation` or `Nebula` without forking.
4. **The seam that worked, three times by accident, is a registry keyed by an opaque id plus a trait for behaviour.**
   Prefabs, monsters and props in this repo already do this, and exactly one hard-coded content string survives in 62k lines.
5. **The Bevy-free core is the right spine.**
   `lwr-world` proves it: one dependency, 249 tests in 4 seconds, no window.
   The compiler must hold that line, because conventions did not: this repo's `core/turns.rs` imports combat, items and player.
6. **The renderer must not be one text entity per tile.**
   Both games do this (5,760 and 13,200 entities).
7. **Neither game has benchmarks, and the engine's benches measure the wrong things.**
   Six of eleven are under 4 ns; FOV, pathfinding, lighting and choke maps are unmeasured.

## 3. Decisions

### 3.1 Themes: out of the engine

Mechanics are engine.
Vocabulary is game.
Lighting, factions, stealth, statuses, tile fields and quest triggers are mechanics and belong in the engine.
Which factions, statuses or tiles exist is the game's business.

The engine's `examples/` carry a small shared content module (a dozen tiles, a few monsters, a few items) so every example is a real playable demo.
That is what `roguelike_engine`'s example lacked.
The full worked theme is the new game itself.
A second, deliberately different consumer (sci-fi or pirate) is the only real proof of theme-agnosticism and is scheduled last.

### 3.2 Extension mechanism: registries and traits, never enum variants

Two mechanisms, chosen by whether the extension is data or code.

**Data extensions use an opaque id and a registry.**
`TileId(u16)` plus `TileRegistry` holding `TileProps { walkable, passable, opaque, obstacle, move_cost, flammability, promotion, glyph }`.
Same shape for `DamageTypeId`, `StatusId`, `StatId`, `SlotId`, `FactionId`, item tags and seed domains.
Lookup is an array index, which is faster than the `matches!` chains it replaces.
Games register from RON at startup.
The engine ships a `standard()` set (wall, floor, door, stairs) for convenience.
The compile-time exhaustiveness that closed enums gave is recovered by a `validate()` hook on registry load plus guard tests over the live assets, which this repo already practises.

**Code extensions use traits taken as parameters.**
`Pass<C>`, `BuildContext`, `OpacitySource`, `CostSource`, `MapOverlay`, `Tactic`, `DamageStage`, `Narrator`, `ThreatSubject`, `ExploreInterrupt`, `SaveBackend`, `Renderable`, and `rand::Rng` for randomness.
The best existing examples are `dequeue_next_batch_pure(_, is_player: impl Fn(Entity) -> bool)`, `accumulate_equip_effects(items: impl Iterator<...>)`, `trace_shot(hostile_at: impl Fn(i32, i32) -> Option<Entity>)` and `TurnQueue::pop_due(is_alive: impl Fn(Entity) -> bool)`.
Each removes a Bevy dependency by asking the caller for the capability.

Generic type parameters are reserved for containers (`Grid<T>`, `TileField<T>`) and the builder context (`Pass<C>`).
Making `Map<T: TileSemantics>` generic was considered and rejected: it infects every downstream type and blocks data-driven tiles.

### 3.3 Randomness: `rand`, named streams, per-run root, cosmetic split

Generator: `rand` 0.9.
Gameplay and world streams use `StdRng` (ChaCha12), not `SmallRng`.
`SmallRng` picks Xoshiro256++ on 64-bit and Xoshiro128++ on 32-bit, so a world seed would generate different terrain on native and on wasm32.
`StdRng` is the same algorithm on both.
Cosmetic streams may use `SmallRng`.

Derivation keeps what worked in the two games, which is about stream isolation, not about the generator:

- `RunSeed` rolled once per run, saved, shown on the outcome screens.
- `RunSeed::derive(domain, index) -> u64` folding a domain salt and an index (floor, chunk, pass) through a mixer, then `StdRng::seed_from_u64`.
  From this repo's `src/core/rng.rs`.
- Domains are open constants, `SeedDomain::new(b"WEATHER")`, so a game adds a stream without editing the engine.
  From `lwr-world`'s string-keyed `stage_seed`.
- Every pass in a generation chain gets its own stream keyed by its name, so inserting a pass cannot reroll its neighbours.
  From `lwr-world`'s `Chain`.
- `FxRng` for cosmetic randomness, declared outside the determinism contract so FX systems need no ordering edges.
  From this repo.
- Position-derived draws where iteration order could leak, as in `lwr-world`'s `jitter_at`.

Functions take `&mut impl Rng`.
There is no engine `RngSource` trait; `rand::Rng` is that trait.
No `with_seed(u64)` on anything; seeds come only from `RunSeed::derive`.

Determinism is still an invariant within one build: same seed, same world, same combat, for replays, bug reports and tests.
It is not promised across engine versions.
The test that guards it is "same seed twice gives byte-equal tiles", which none of the three repos has.
Two live holes get fixed on the way in: `roguelike_engine`'s `randomize_grid` uses `rand::rng()` so every lake differs at a fixed seed, and this repo's `FireRng` is seeded from a constant and never reseeded.

### 3.4 Map generation: `lwr`'s pass rigour on `roguelike_engine`'s context trait

Base the pipeline on `lwr-world/src/local/chain.rs`, not on `BuilderChain`.
`Pass::name()` keys the pass's RNG stream.
Duplicate names are rejected.
`phase()` is mandatory and there is no unchecked escape hatch.

Take from `roguelike_engine` the `BuildContext` trait so passes are generic over a game-extended context, the `take_snapshot` hook, `FloorProfile`, and the `BuilderPhase` ordering.
Take from this repo the wasm-safe timing and the removal of the unseeded constructor.

Add what none of them have:

- `apply` returns `Result<(), BuildError>` so a chain can retry.
- `Chain` is `Send` so generation can move off the main thread.
- A typed output seam, `ctx.emit::<T>(value)`, so `PrefabStamper` can drop its `Arc<Mutex<Vec<..>>>` side channel.
- Rooms are optional by design, not `Option<&Vec<Rect>>` forced on cave builders.

One `Chain` type serves both world passes (terrain, vegetation, settlements, roads) and dungeon builders (room accretion, BSP, caves, cullers, doors, stairs, decoration, prefabs, choke map).
Fix the three complexity classes when porting: incremental door sites in room accretion (currently 2,000 attempts each rescanning the map), worklist pruning in the choke map (currently quadratic in map area), and single-pass region labelling (currently one flood per unlabelled cell).

### 3.5 The overworld: what to drop and what to keep

Nate's instinct is against an overworld.
The thing worth being against is the *mode switch*: a zoomed-out screen where the player is a token, and local maps that are boxes with edges you enter and leave.
`living-world-rogue` has that model and it is the part that feels like a menu.
Sunlorn does not, and that is the feel WildReach wants.

But "overworld" names two different things, and only one of them is the mode switch.
The other is a coarse world structure: where the biomes, sites and roads are, decided before any tile exists.
Every open-world roguelike that is bigger than a few screens has that structure whether or not it shows it.
Cataclysm DDA presents one continuous map and still keeps an overmap underneath it plus a zoomed view of it on a key.
Dwarf Fortress adventure mode is the purest continuous world in the genre, and it is unplayable without its travel mode.
Sunlorn's towns and roads did not place themselves at tile scale either.

A world larger than 1024x1024 settles the question.

- **Generation.**
  `lwr-world` generates 240k cells in about a second.
  A 4096x4096 world is 16 million tiles, so a full eager generation is a minute or more at new game, before any settlement pass.
  Tiles have to be generated on demand from a structure that was generated eagerly.
  That structure is an overworld.
- **Traversal.**
  Crossing 4096 tiles at one tile per turn is a long walk even with key repeat.
  Sunlorn mitigates this with portals and road travel.
  At this size WildReach needs a travel command, and a travel command plans over the coarse structure.
- **Memory and simulation.**
  Sixteen million tiles fit in memory, but nothing may touch them per turn.
  The active region has to become the set of loaded chunks around the player, which is Cataclysm's reality bubble.

So the recommendation is: **keep the overworld as a data model and as generation structure, and treat any overworld screen as a lens onto that data, never as the player's location.**
Nate wants the lens now, including travelling through it, and wants to be able to remove the travelling later.
That is achievable with one invariant and one crate boundary, spelled out in 3.5.1.
Concretely:

1. **`WorldGraph`**, the coarse layer.
   The world is divided into regions of, say, 64x64 tiles; a 4096x4096 world is a 64x64 grid of regions.
   Each region carries its elevation band, climate, biome band, an optional site, and the roads passing through it.
   This is `lwr-world`'s `Overworld` almost verbatim, and it generates in milliseconds.
   The quantile banding is computed here, once, over the whole world.
2. **Chunks**, the fine layer, generated on demand.
   A chunk is a pure function of the run seed, its region, and its neighbouring regions.
   It samples the same continuous noise the coarse layer sampled, at tile resolution, so it agrees with its region's bands.
   This is `lwr-world`'s local map generation with `Surroundings` and `TileFacts`, made generic over the facts type.
3. **Seamless boundaries.**
   Chunks stitch with no edge.
   Linear features that cross a boundary (roads, rivers, walls) agree on the crossing point through `lwr-world`'s seam hash, which the reviewer called the most reusable idea in that repo.
   Compact features never straddle a boundary: a settlement is placed at its region's centre with a footprint smaller than the region, so buildings, docks and dungeon entrances always belong to exactly one chunk.
   That single constraint is what makes independent chunk generation sound.
4. **Chunk lifecycle.**
   The loaded set is the chunks within the active region.
   A chunk that leaves the region is unloaded; if it was mutated (dug, burned, looted, a door left open) its delta is kept and replayed when it loads again.
   Unmutated chunks are simply regenerated, which is what makes a 4096x4096 world cost nothing to store.
5. **Walking scale is the ground truth.**
   The player's position is always a world tile coordinate.
   Walking off the edge of a chunk streams the next chunk in; it never returns the player to an overworld screen, which is the `lwr` behaviour WildReach does not want.
   Town portals teleport between discovered sites.
6. **Dungeons** are separate maps on a map stack behind entrances, as before.

Everything Nate liked about Sunlorn survives: no boxes, the whole world walkable at one scale.
Everything `lwr-world` got right survives too: deterministic worlds, fast generation, testable passes, and the world picture.

World size target: 4096x4096 tiles as the design point, with nothing in the architecture that depends on it.
Region size is a config field, not a constant, because `lwr-world` sized its local maps to the terminal and regretted it.

#### 3.5.1 The overworld screen: use it now, drop it later

The overworld screen has two jobs, kept separate so either can be switched off.

| Job | Where it lives | WildReach |
|---|---|---|
| **View**: render `WorldGraph` (bands, rivers, sites, roads, discovered regions) as a map | `rl-render`, a `Renderable` over regions | always |
| **Portal picker**: choose a discovered site and portal to it | `rl-overworld`, opt-in | now; droppable |

The invariant that makes the second row removable: **the overworld never owns the player's position.**
In `lwr` the overworld cell is the location and entering it builds a local map.
Here the marker on the overworld is derived from the tile position divided by the region size.
Choosing a discovered site writes a portal request; the portal mechanic in `rl-bevy` moves the player and streams chunks.
Undiscovered regions cannot be chosen, because getting somewhere the first time means walking there.

Because `rl-overworld` only reads `WorldGraph` and the discovery set and writes a portal request, nothing else depends on it.
Removing it from WildReach's `Cargo.toml` leaves the view and the portal mechanic working unchanged.
`lwr`'s `Scale` / `Travel` state machine is not ported; the overworld is a screen, never a mode the player moves in.

#### 3.5.2 Travel

Far travel is walking.
The first visit to a place is always on foot, through whatever lies between.
Once a site is discovered, a portal takes the player there instantly, with no time simulated and no encounter rolled.
Auto-explore and travel-to-a-seen-tile within the active region are conveniences over walking, and they do simulate every step, because they are walking.
There is no cross-world route command in either form.

#### 3.5.3 Rivers

Rivers are the second linear feature after roads, and the reason the seam primitive is designed as a general `LinearFeature` rather than a road special case.

- **Hydrology pass on `WorldGraph`**, between elevation and climate.
  Sources are regions above an elevation quantile with enough moisture.
  Each river follows steepest descent across the region grid, merging where paths meet, pooling into a lake at a local minimum, and ending at the sea.
  Flow accumulation gives each segment a width class.
  This is the same shape as `lwr-world`'s road network: a few hundred lines over the coarse grid, deterministic, testable with seed-range properties ("every river reaches sea or lake", "no river flows uphill").
- **Rivers feed the rest of the coarse layer.**
  `lwr-world`'s `distance_to_water` becomes distance to sea or river, so moisture and biome bands respond to rivers.
  Site scoring prefers river banks.
  The road router prices a river crossing so roads bridge at narrow points.
- **Per region, a river is a `LinearFeature { kind, enters: (edge, offset), exits: (edge, offset), width }`.**
  Offsets on shared edges come from the seam hash, so two neighbouring chunks agree where the river crosses without negotiating.
  A chunk draws its channel between its entry and exit points with the given width, and a bridge tile where a road's crossing coincides.
- **At walking scale** a river is water tiles from the registry with a `CostSource` that makes them impassable to land walkers, fordable at marked shallows, and navigable for the sailing `MovementProfile`.
  That last point is what Corsair will lean on.

Hydrology lands in M1 so the overworld view shows a coherent world from the first build.
River channels and bridges in chunks land in M4 with roads and settlements.

### 3.6 Turn scheduling: the engine owns the loop

The pure heap is identical in all three repos and comes across as-is, generic over `Id: Copy + Eq`.
Take `lwr`'s hand-written `PartialEq` matching `Ord`, its `pop_due(is_alive)` signature, and its argument for an integer clock.
Take this repo's `is_alive` guard and `MAX_NPC_BATCH` of 64.

Then lift this repo's `ProcessingPhase` chain (Brain, ResolveMovement, ResolveActions, Cleanup) and `TurnState` gate into the Bevy crate.
This is the machinery `roguelike_engine` left behind and the reason it was not adopted.
Three changes on the way:

- The scheduler advances the clock; games stop hand-rolling it.
- Death resolution is specified to run before requeue, so the two duplicated defensive checks become one contract.
- Speed comes from a `SpeedModifier` trait or an `ActionCost` message, not from reading `Chilled`, `Hasted`, `Slowed` and `Berserking` by name.

Open world addition: an **active region**.
The active region is the set of loaded chunks around the player.
Only actors inside it are scheduled.
Actors outside it are frozen with their chunk, or advanced coarsely when the region reaches them.
This is what keeps a sixteen-million-tile world at turn-based cost.

### 3.7 Pathfinding: Dijkstra maps per pather class, A* for unique goals

A Dijkstra map is a grid of integers where each cell holds its cost to the nearest goal cell.
It is built by one search that starts from every goal at once, so the whole map costs one pass regardless of how many goals or how many consumers there are.
A monster uses it by looking at its eight neighbours and stepping onto the lowest value.
That step is eight array reads.

The contrast with what this repo does today: every hunting monster runs its own A* per turn toward the same player.
Fifty monsters means fifty searches, each evaluating the hazard cost function along its own frontier.
One Dijkstra map rooted at the player answers all fifty.

**Why one map per pather class.**
The values in a map depend on the cost function, and the cost function depends on what the mover can do.
A monster that opens doors sees a different map from one that cannot.
A swimmer, a flier, and something immune to fire each see different maps.
Two monsters with the same capabilities see the same map and share it.
So the engine interns a `MovementProfile` (bitflags: opens_doors, swims, flies, fire_immune, avoids_webs, ...) to a small `ProfileId`, and keeps one map per `(goal set, ProfileId)`.
A typical map has two to four distinct profiles among the monsters that are awake, so it is two to four floods per turn instead of fifty searches.

**Goal sets.**
"Toward the player" is one map.
Others come free from the same machinery: toward the nearest item of a kind (fetch), toward the nearest exit, toward allies, toward the last heard noise.
Sound propagation with decay is literally a Dijkstra map, which is what this repo's noise system built by hand.
`lwr-world`'s `distance_to_water` and `distance_field` are two hand-written copies of the same multi-source flood.

**Fleeing.**
Take the player map, multiply every value by a negative factor slightly above one (Brogue uses -1.2), and rescan from those values as the starting costs.
The result is a safety map whose low points are far from the player *and* reachable without passing near the player.
Fleeing monsters descend it.
It costs one extra pass and replaces the corner-walking heuristics in `flee_direction`.

**Invalidation.**
The player map is recomputed when the player moves, which is most turns.
A profile's map is also dirty when its cost field changed: a door opened, fire spread, a web was cut.
Those arrive through the map mutation messages.
Maps are built lazily on the first monster request in a turn, so a turn with nothing awake costs nothing.

**On a continuous open world.**
The flood must not visit a million cells per profile per turn.
Bound it to the active region from section 3.6, or stop expanding at a maximum distance.
Cells outside are unreachable, which is correct because actors outside are not scheduled.
Use a bucket queue (Dial's algorithm) since step costs are small integers, giving linear time in cells visited rather than log-factor heap time.
Store `u16` per cell with `u16::MAX` meaning unreached.
Keep one reusable scratch buffer per profile with a generation counter, so a rebuild neither allocates nor clears.

**Descending.**
Pick the minimum neighbour with a fixed neighbour order for deterministic ties.
Occupancy is not in the map, so a step onto an occupied cell falls back to the next-lowest neighbour or waits, which is what produces natural queueing in corridors rather than conga lines.
Apply the diagonal-corner rule this repo's `is_frontier` already gets right.

**Where A* stays.**
A single mover with a unique, distant goal: a courier walking to a named town, a quest NPC heading home, the player's travel-to command.
Those keep A* with this repo's `PathCache`, which probes the cached path before searching again.
The rule is: shared goal, Dijkstra map; unique goal, A*.

**The API.**
One type, `DijkstraMap`, with `build(goals, cost: impl Fn(idx) -> Option<u16>, bound)`, `descend(idx) -> Option<idx>`, `rescan_scaled(factor)`, and `value(idx)`.
It serves AI, sound, auto-explore, stair placement, site scoring and flee maps.
It lives in `rl-grid` with a benchmark at three active-region sizes.

### 3.8 Drop bracket-lib; own the algorithms

The fork is narrow in use (Point, Rect, FOV, A*, Dijkstra, Bresenham, dice) and carries three small patches over an upstream that is no longer maintained.
Its FOV returns a `HashSet<Point>`, which is the hottest allocation in the game and a hash-iteration-order hazard.
Its `DijkstraMap` does not zero seed depths, which this repo works around in auto-explore, and it has no bound, no rescan and no bucket queue.

The engine owns: `Point`, `Rect`, `Direction`, `DirectionSet`, Bresenham, symmetric shadowcasting FOV writing into a bitset, A* with an optional entry-direction state and turn cost (from `lwr`'s road router), `DijkstraMap` as above, and a pre-parsed `Dice` struct.
That is roughly 1,500 lines, all benchmarkable, all tested against the current outputs before bracket-lib is removed.

`petgraph` is also dropped.
`roguelike_engine` uses it once, to hold a tree of rectangles.
`lwr`'s road network needs a `DisjointSet` and a Dijkstra over sites, which are 130 lines.

### 3.9 Data layout for performance

- **Three resources, not one `Map`.**
  `Terrain` (per map, rarely mutated), `Occupancy` (per frame, a spatial index), `Knowledge` (per viewer).
  This repo rebuilds `blocked` every frame through `ResMut<Map>`, which marks the whole resource changed and silently defeats three full-map change-detection gates at 60 Hz, including a `DefaultHasher` pass over every tile.
- **`SpatialGrid`** with `at(x, y) -> &[Entity]` and `in_radius`, updated on `Changed<Position>`.
  This repo has no entity-at-tile index; every AoE, cleave, explosion and occupancy check is a scan over all actors.
- **Bitset viewsheds**, a word per 64 tiles, deterministic iteration.
- **`DijkstraMap` per pather class** per section 3.7.
- **`TileField<T>`** with double-buffered integer kernels, no allocation in the tick, RNG-free by default.
  Fire, gas, sound, scent, heat and blood are one type with different kernels.
  This repo's `gas.rs` is the reference implementation.
  Fields are bounded to the active region on a large map.
- **Sparse active sets** for tile promotion instead of a full-map scan per turn.
- **Reusable scratch buffers** in the router, keyed by a dirty list.
  `lwr`'s router allocates 136 bytes per cell per call, which is the whole of its 1.1 s at 600x400.
- **Interned `Id<T>(u32)`** for content references.
  `Cooldowns(HashMap<String, u32>)` puts string hashing inside the AI's double loop.
- **No `HashMap` or `HashSet` in gameplay paths.**
  `BTreeMap`, `Vec` or bitsets, with the reason stated as `lwr` does.
- **Shadowcast lighting**, O(r^2) per source, and no global viewshed redirty when a light changes.
- **Viewport rendering.**
  The glyph grid draws the visible window, never the world.

Benches from day one, on the real paths: FOV at 64 actors, A* and `DijkstraMap` at three region sizes, lighting at 20 sources, choke map, each builder, a full world chain at three sizes, `TileField` tick, and the render sweep.
`roguelike_engine`'s criterion harness is the starting point; its cases are not.

### 3.10 Rules layer: shapes in the engine, vocabulary in the game

| Engine ships the shape | Game ships the instances |
|---|---|
| `DamageTypeId`, resistance ladder, `ResistAccum` | which damage types exist |
| `StatusDef` registry: duration, stacking, per-turn effect, modifiers, badge | which statuses exist |
| `Modifier { stat: StatId, op: Add / Mul / Compound / Max, value }` and one accumulator | which stats exist |
| Equipment slot graph: occupancy, two-handed claiming, dynamic resolution | which slots exist |
| Hook points: on_hit, on_defend, on_kill, on_death, on_block, damage_mods | what each hook does |
| `TargetMode`, `AoeShape`, footprint resolution, LOS and wall bounding | which abilities exist |
| `DamageEvent` with the `attacker` / `credit` split, and a staged pipeline | the balance numbers |
| `Tactic` trait, `TacticCtx`, the pure decision helpers | which tactics exist |
| `ContentRegistry<T>`, `BandedWeightedTable<T>`, validate-on-load, RON loading | the RON |
| `Narrator` trait over typed log events | the sentences |
| Typed world events and a trigger registry (entered region, killed def, picked up tag, talked to) | which quests exist and what victory means |
| Balance checker over `ThreatSubject` | the threat inputs |
| `FactionId` and a relation matrix | which factions exist |

Specific ports:

- The damage pipeline becomes named stages the game composes (`ScaleByAttacker`, `ApplyResistance`, `TryBlock`, `SubtractArmor`, `ScaleByDefender`, `Commit`, `RunHooks`).
  This repo's single 615-line system at Bevy's 16-parameter ceiling is the thing being replaced.
- Second-pass reactions (thorns, cleave, attacker statuses, splits, blinks, steals) go on one engine `ReactionQueue` instead of five game-owned messages the engine plugin registers.
- `AttackIntent` exists.
  Attacking is not a side effect of walking into someone.
- The affix and enchant model (`AffixDef`, `ScaledOnHit`, `EnhanceRule`) ports nearly as-is, keyed on game-defined tags instead of `ItemCategory`.
- The tactic-priority AI is the only AI.
  GOAP is not ported.
- The log carries a category and a structured payload.
  This repo classifies log lines by matching about thirty English substrings.
- Quests are game content.
  The engine's contribution is that every gameplay outcome is a typed event a game can subscribe to, plus a registry of named triggers and counters, so a quest or a generated victory condition is data over those events.

### 3.11 Bevy layer conventions

- Every engine plugin's systems run in a turn-driven schedule gated by `run_if` by default.
  `roguelike_engine` puts every system in `Update` ungated, so poison ticks sixty times a second and FOV runs in the main menu.
  The `SystemSet` markers stay exposed for reordering.
- The three-owner ordering rule from this repo's `CLAUDE.md` is kept and given enough named phases that no downstream plugin ever writes `.after(concrete_system)`.
- `#[derive(Message)]` with `MessageWriter` and `MessageReader` is the seam between engine and game.
  `roguelike_engine/src/map/mutation.rs` is the model: request messages, engine apply systems doing only data sync, game reactions `.after(MapMutationSet)`.
- A `MapBound(MapId)` marker makes map teardown and the map stack automatic instead of a hand-maintained list of component types.
- A `TestApp` builder replaces plugins registering other modules' messages for test convenience.
- Bevy 0.19, matching `living-world-rogue`.
  The Bevy-free tier is what makes engine version churn survivable.

### 3.12 UI: reused from the start

`rl-render` owns the world view: a `GlyphGrid` drawn as one instanced mesh over a font atlas, a `Renderable` trait with an engine-owned `Cell`, the lit / remembered / occluded rules from this repo, camera follow, particles and screenshot capture.
Both games spawn one text entity per cell; that backing store is rewritten, the rules are kept.

`rl-ui` owns chrome and modals as Bevy UI, ported from this repo's `src/ui/`: theme tokens (spacing scale, z-ladder, semantic palette, single font installer), the list-to-detail widget, key hints and keybinds, the tabbed window, the framed modal, the semantic log view, and the side panel skeleton (stat bars, status badges).
The `ActiveModal` closed enum becomes a registry of modal ids the game populates.
The widget must not import its consumer; today `list_detail` re-exports helpers from `inventory_preview`.

Bevy UI rather than drawing chrome on the glyph grid, because that is the code that exists, it already handles wrapping and layout, and this repo's visual verification flow works against it.
The map view stays a glyph grid because an ASCII game's map is a glyph grid.

### 3.13 Documentation and test rules

- `#![deny(missing_docs)]` on every crate.
- Doc-tests compile and run.
  All seven in `roguelike_engine` are `ignore`d, which is how its docs came to promise three `Custom` variants that do not exist.
- No blanket `#![allow(clippy::too_many_arguments, clippy::type_complexity)]`.
  Those are the lints that would have flagged the 16-parameter systems.
- House style is `lwr`'s "why and why-not" doc comment, at this repo's density.
- Every RON schema carries a top-of-file comment listing the full option space.
- Test idioms required in the engine and documented for games: property-over-seed-range, fingerprint tripwires labelled as such, `ALL` constants on content tables, headless `App` tests, and guard tests over live assets.
- `examples/` grows one example per milestone, each runnable: headless world dump with snapshots, a walking `@` with FOV, monsters that hunt and fight, loot and equipment, a dungeon entered and left by portal, a save round-trip, the balance report, a custom tile, a custom `Pass`, a custom `Tactic`.

## 4. Crate layout

Boundaries are drawn on dependency weight, not subject.
The load-bearing line is between tier 1 and tier 2.
CI fails if `cargo tree` for any tier-1 crate shows `bevy`.
Crates are listed in the order they come into existence; the milestone column says when.

| Tier | Crate | Contents | Deps | Milestone |
|---|---|---|---|---|
| 0 | `rl-core` | `Grid<T>`, `Grid2D`, `Point`, `Rect`, `Direction`, `DirectionSet`, geometry (Bresenham, cone, disc, distances), `DisjointSet`, `RunSeed` + `SeedDomain` + derive, `Dice`, `Id<T>` interning, quantile stats, `TurnQueue<Id>` and the action cost model | `rand` | M1 |
| 1 | `rl-grid` | `TileId` / `TileRegistry` / `TileProps`, `Terrain`, `MapOverlay`, `OpacitySource`, `CostSource`, flood / label / distance field, shadowcast FOV into bitsets, A* (optional turn cost), `DijkstraMap`, `PathCache`, `SpatialGrid`, `TileField<T>`, tile promotion, lighting | core | M1 |
| 1 | `rl-mapgen` | `Chain` / `Pass<C>` / `Phase` / `BuildContext`, snapshots, decoration rules; later the dungeon builders, prefab stamping, choke map | core, grid | M1 (chain), M4 (dungeons) |
| 1 | `rl-world` | `Fbm`, `SampleSpace`, quantile banding, `WorldGraph` (regions, bands, hydrology, sites, roads), `LinearFeature` and the seam hash, chunk generation from `Surroundings<F>`, scored site placement, road router and network, settlement and vegetation passes, river channels and bridges, chunk deltas | core, grid, mapgen, `noise` | M1 (graph, hydrology, chunks), M4 (settlements, channels) |
| 1 | `rl-test-support` | ASCII map fixtures, seed-range property helpers, neighbourhood fixtures | core, grid | M1 |
| 2 | `rl-bevy` | plugins, `Position` / `Viewshed` / `Collider`, `ProcessingPhase` and `CombatPhase` sets, the turn loop, active region and chunk streaming, occupancy index, map stack and `MapBound`, mutation messages, `ReactionQueue`, `TestApp` | all above, `bevy` | M1 |
| 2 | `rl-render` | `GlyphGrid` instanced renderer, engine-owned `Cell`, `Renderable`, the `WorldGraph` view, camera, particles, screenshot | bevy | M1 |
| 2 | `rl-overworld` | opt-in: overworld screen with a portal picker over discovered sites; reads `WorldGraph` and discovery, writes a portal request, owns nothing else | bevy, world | M1 |
| 2 | `rl-ui` | theme tokens, list-detail widget, key hints, tab chrome, framed modal, semantic log view, side panel skeleton, modal registry | bevy | M1 (shell), M3 (inventory widgets) |
| 1 | `rl-content` | `ContentRegistry<T>`, `BandedWeightedTable<T>`, validate hook, RON loading, `include_str!` convention | core, `serde`, `ron` | M2 |
| 1 | `rl-rules` | stat / modifier stack, damage stages and `DamageEvent` shape, status registry with generic tick and expire, hook vocabulary, targeting and AoE footprints, factions matrix; later the slot graph and the affix and enchant model | core, grid, content | M2 (combat), M3 (equipment) |
| 1 | `rl-ai` | `Tactic` + registry + `TacticCtx`, `MovementProfile`, pure decisions, ability scorer, stealth and awareness; later the auto-explore and travel pure half | core, grid, rules | M2 |
| 1 | `rl-events` | typed world events, trigger registry, named counters | core, content | M5 |
| 2 | `rl-save` | `SaveBackend`, native and wasm backends, `beforeunload` bridge, `Versioned` policy, `SaveId` remap, chunk delta persistence | core, bevy | M6 |
| 1 | `rl-tools` | balance checker over `ThreatSubject`, seed replay, headless dump | content, rules | M6 |
| 3 | `rl-engine` | facade with a curated prelude, `examples/`, `benches/` | everything | M1 |
| 3 | `corsair` | the pirate example game, a workspace member with its own assets | everything | M7 |

Sixteen crates, of which nine exist after the first milestone.
`rl-grid` may later split into `rl-fov`, `rl-path` and `rl-field` without breaking consumers.

Workspace `Cargo.toml` ships `lwr`'s profile trick: dependencies at `opt-level = 3`, workspace crates at `1` in dev.

## 5. What comes from where

| Piece | Source | Change on the way in |
|---|---|---|
| `RunSeed::derive`, `FxRng`, four determinism tests | `fantasy-rogue/src/core/rng.rs` | `StdRng` behind it, domains become open constants |
| Named per-pass streams | `lwr-world/src/rng.rs`, `local/chain.rs` | keyed through `RunSeed::derive` |
| `Chain` / `Pass` / `Phase` | `lwr-world/src/local/chain.rs` | generic context, `Result`, `Send`, `emit` |
| `BuildContext`, `FloorProfile`, snapshots | `roguelike_engine/src/map/builders/mod.rs` | `impl Rng`, rooms optional |
| Elevation, climate, banding, `Fbm`, `SampleSpace`, stats | `lwr-world/src/{elevation,climate,fbm,sample,stats}.rs` | band-to-tile through a game classifier |
| Site placement, road router and network | `lwr-world/src/{sites,roads}.rs` | `CostSource` instead of `friction(Biome)`, scratch buffers, `PlacementRules` struct |
| `Overworld` as `WorldGraph`, `Surroundings` / `TileFacts`, seam hash | `lwr-world/src/{overworld,local/surroundings,local/passes/finish}.rs` | `Surroundings<F>` generic over facts, seam hash generalised to `LinearFeature`, chunks instead of local maps, hydrology pass added |
| Settlement, vegetation, cavern, ruin passes | `lwr-world/src/local/passes/` | stamp a region of one map, tiles via registry |
| `Grid<T>`, `Direction`, `Room` | `lwr-world/src/{grid,direction,local/room}.rs` | fix `is_empty`, avoid per-cell division in `iter` |
| `Grid2D` trait | `fantasy-rogue/src/map/grid.rs` | as-is |
| Map overlay idiom | `fantasy-rogue` `SmokeOpacityMap`, `HazardMap`, `KnownMap` | formalise as `MapOverlay` with default forwarding |
| Multi-source distance field | `lwr-world` `distance_to_water`, `distance_field`; `fantasy-rogue` auto-explore | one `DijkstraMap` with bound, rescan, bucket queue |
| Dungeon builders, cullers, decoration rules | `fantasy-rogue/src/map/builders/` | `TileId` targets, incremental door sites |
| Choke map, lakes | `roguelike_engine/src/map/builders/` | worklist pruning, seeded blobs |
| Prefab stamper | `fantasy-rogue/src/map/prefab.rs:685-990` | output via `emit`, content half stays game-side |
| Turn heap | any of the three | generic `Id`, `lwr`'s `PartialEq` |
| `ProcessingPhase`, `TurnState` | `fantasy-rogue/src/core/turns.rs:273-410` | engine-owned, no game intents registered |
| Mutation message pattern | `roguelike_engine/src/map/mutation.rs` | strip moss and fungus |
| `DamageEvent` shape, `CombatPhase` | `fantasy-rogue/src/combat/mod.rs:1195-1372` | staged pipeline replaces the monolith |
| `accumulate_equip_effects`, `merged_with` | `fantasy-rogue/src/combat/equip_stats.rs:189-226`, `items/data.rs:289-332` | generic `StatId` accumulator |
| `TimedStatus`, `DotStatus`, `expire_status::<T>` | `fantasy-rogue/src/combat/mod.rs:2216-2335` | driven by a `StatusDef` registry |
| Tactic dispatch, `TacticCtx`, pure decisions | `fantasy-rogue/src/actors/ai/{brain,decisions}.rs` | `Tactic` trait, snapshot resource instead of 16 queries |
| Ability scorer | `fantasy-rogue/src/actors/ai/targeting.rs` | score over a hook trait, not `AbilityEffect` |
| `PathCache`, `trace_shot` | `fantasy-rogue/src/actors/ai/pathfinding.rs`, `combat/archery.rs:120-147` | as-is |
| Auto-explore pure half | `fantasy-rogue/src/actors/auto_explore.rs` | `ExploreInterrupt` trait for stop rules |
| Affixes, enchant | `fantasy-rogue/src/items/{affixes,enchant}.rs` | tags instead of `ItemCategory` |
| Registry loaders and validate hook | `fantasy-rogue/src/actors/monster_data.rs:832-878` | one `ContentRegistry<T>` |
| Gas diffusion kernel | `fantasy-rogue/src/map/gas.rs:174-268` | becomes the reference `TileField<T>` kernel |
| Balance checker | `fantasy-rogue/src/actors/balance.rs` | `ThreatSubject` trait |
| `SaveBackend`, `web_unload` | `fantasy-rogue/src/save/{backend,web_unload}.rs` | as-is, plus `SaveId` remap |
| `Renderable`, screenshot | `lwr/src/{render,screenshot}.rs` | engine-owned `Cell` |
| Render rules (lit vs remembered, z-priority, lazy spawn) | `fantasy-rogue/src/render/mod.rs` | behind `GlyphGrid` |
| Narration seam | `fantasy-rogue/src/combat/narrate.rs` | `Narrator` trait |
| Theme tokens, list-detail, key hints, tabs, modals, log | `fantasy-rogue/src/ui/{theme,list_detail,key_hint,tabs,game_log}.rs` | modal registry, widget must not import its consumer |
| Test fixtures | `lwr-world/src/local/fixtures.rs` | public `rl-test-support` |
| Criterion harness | `roguelike_engine/benches/` | replace every case |

Not ported: `lwr`'s `Scale` / `Travel` mode switch, GOAP, `roguelike_engine`'s `squad/` and `stealth/noise.rs`, `lwr`'s two-entities-per-cell terminal, bracket-lib, petgraph.

## 6. What is not carried

- `#[non_exhaustive]` + `Custom { id }`, and every `_ =>` arm the compiler calls dead.
- Closed taxonomy enums in engine types.
- bracket-lib and petgraph.
- Hand-rolled generators.
- One text entity per tile.
- `HashSet<Point>` viewsheds and any hash container in a gameplay path.
- Occupancy inside the terrain resource.
- `String` def ids in hot loops.
- Melee inside the movement handler.
- Engine plugins registering other modules' messages.
- Ungated `Update` systems in engine plugins.
- `std::time::Instant` in anything that must run on wasm.
- Unseeded constructors and fresh generators inside per-turn systems.
- `TODO` comments in source.
- `lwr`'s "no world may move" refactoring constraint.
  It is right for a live game and pure cost for a library nobody has generated worlds with.

## 7. Open questions

Parked by Nate's decision to build first and course-correct.
Defaults taken: region size 64x64; lakes only where hydrology makes them.

## 8. Milestones

Each milestone is a playable slice of WildReach, and the engine ships only what that slice pulls on.
Each ends with a runnable example in `rl-engine` and a tagged build of WildReach.

**M0. Scaffold.**
Two repos: `rl-engine` and `wildreach`.
Workspace, tier layout, CI (tier-1 `cargo tree` check, `deny(missing_docs)`, doc-tests, clippy without the blanket allows, criterion baseline, wasm build check), profile trick, and a `CONTRIBUTING` page carrying sections 3.11 to 3.13.

**M1. Walk the world.**
Crates: `rl-core`, `rl-grid`, `rl-mapgen` (chain only), `rl-world` (graph, hydrology, chunks), `rl-test-support`, `rl-bevy`, `rl-render`, `rl-overworld`, `rl-ui` (shell), `rl-engine`.
Engine: grid, geometry, `RunSeed`, tile registry, bitset FOV, A*, `DijkstraMap`, `SpatialGrid`, `WorldGraph` from noise with hydrology, chunk generation with seamless boundaries, chunk streaming as the active region, the engine-owned turn loop, viewport glyph grid, the `WorldGraph` view, the overworld screen with a portal picker, theme, side panel, log, key hints.
Game: a seeded 4096x4096 world, an `@` that walks it with FOV across chunk boundaries with no visible seam, a look command, an overworld screen showing biomes, temperature, rivers and the player's marker.
The "same seed twice is byte-equal" test per chunk, a "neighbouring chunks agree at every boundary tile" property test, "every river reaches sea or lake", and benches for FOV, A*, `DijkstraMap`, one chunk, and the world graph.
Example: `walk` and a headless `world_dump --seed N --region X,Y`.

**M2. Things that hunt.**
Crates: `rl-content`, `rl-rules` (combat), `rl-ai`.
Engine: registries with validate-on-load, stats and modifiers, staged damage pipeline, `AttackIntent`, statuses, factions matrix, tactic-priority AI over `DijkstraMap` per `MovementProfile`, flee maps, stealth and awareness, semantic log with a `Narrator`.
Game: wilderness encounters from RON, death, a victory-less loop.
Example: `hunt`.

**M3. Loot.**
Crates: `rl-rules` (equipment), `rl-ui` (inventory widgets).
Engine: slot graph, item tags, affixes and enchant, pickup / equip / drop / use handlers, throwing, `BandedWeightedTable`, list-detail and tabbed inventory, character sheet.
Game: loot drops, equipment, consumables.
Example: `loot`.

**M4. Places.**
Crates: `rl-mapgen` (dungeon builders), `rl-world` (settlements).
Engine: room accretion, BSP, caves, cullers, doors, stairs, choke map, decoration rules, prefab stamping, settlement and vegetation passes inside a chunk, roads and river channels crossing chunks as `LinearFeature`s, bridges and fords, map stack with `MapBound`, transitions, discovery, town portals.
Game: towns on the map, roads and rivers between them, dungeon entrances, descend and return by portal, portal to a discovered town from the overworld screen.
Example: `places`.

**M5. Encounters and quests.**
Crates: `rl-events`.
Engine: typed world events for every gameplay outcome, trigger registry, named counters, abilities and targeting, `TileField<T>` for fire and gas, lighting.
Game: scripted and generated encounters, a quest log, a generated victory condition as data over events.
Example: `quest`.

**M6. Persist and tune.**
Crates: `rl-save`, `rl-tools`.
Engine: save backend with `SaveId` remap and versioning policy, chunk delta persistence, balance checker over `ThreatSubject`, seed replay.
Game: save and continue with mutated chunks restored, a balance report over its RON.
Example: `save_roundtrip`, `balance_report`.

**M7. Corsair.**
A small pirate example game as a workspace member of `rl-engine`, a few thousand lines with its own RON.
It is chosen because it stresses different seams from WildReach.

- A world that is mostly ocean: the quantile banding knob turned the other way, islands as regions, ports as sites.
- Water walkable only by ships: `MovementProfile` per class, so navy and merchant ships and land walkers each get their own `DijkstraMap`.
- The player's ship as a vehicle: boarding switches the player's profile, no sub-map.
- Boarding combat: an adjacent ship pushes a small deck map onto the map stack.
- Three factions with a relation matrix: navy, pirates, merchants.
- Non-fantasy vocabulary registered from RON: cannon and cutlass damage, scurvy as a status, rum as a consumable, doubloons as loot.
- A treasure map quest as data over events: dig at a marked tile.
- Weather as a `TileField<T>`.

The point is not the game; it is that nothing in tiers 0 to 2 changes to make it possible.
If something does have to change, that is a seam the plan got wrong.
