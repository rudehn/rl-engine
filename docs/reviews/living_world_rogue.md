# Engine-extraction review: `living-world-rogue`

Reviewer scope: entire repo `/Users/nathanrude/Development/living-world-rogue`.
17,570 LOC of Rust across two crates. Bevy 0.19 workspace; `lwr-world` depends on `noise` and nothing else.

Verification run: `cargo test --offline -p lwr-world` → **249 passed, 0 failed, in 4.27s**, plus 1 doc-test.
Timing (release, `--dump`, which runs the full overworld pipeline):

| world size | cells | wall time |
| --- | --- | --- |
| 132x48 | 6,336 | 0.03s |
| 256x128 | 32,768 | 0.22s |
| 400x200 | 80,000 | 0.44s |
| 600x400 | 240,000 | 1.11s |

One local map (`--dump-local`) is under 10ms including the whole world it sits in.

**Headline judgement.** This is by a wide margin the best-engineered of the three repos I have seen for the
things an engine has to get right: boundaries, determinism, and testability. It is also the *least* directly
extractable, because almost every generic mechanism in it is welded to a fantasy-Earth content vocabulary
(`Biome`, `LocalTile`, `SiteKind`, `arable_value`) that lives in the same crate and the same enums. The
mechanisms are the prize; the content is not. The extraction job is to split roughly 45% of `lwr-world` into
engine primitives and leave the other 55% as one worked example.

---

## A. Subsystem inventory

### `crates/lwr-world` (11,140 LOC, zero Bevy)

| Subsystem | Path | LOC | Purpose | Verdict |
| --- | --- | --- | --- | --- |
| `Rng` + `stage_seed` | `crates/lwr-world/src/rng.rs` | 114 | SplitMix64 + FNV-named-stream seed derivation | **EXTRACT-AS-IS** |
| `Grid<T>` | `crates/lwr-world/src/grid.rs` | 244 | Row-major dense grid, `nearest_from` ring search | **EXTRACT-AS-IS** |
| `stats` | `crates/lwr-world/src/stats.rs` | 128 | quantile, normalise, rank-normalise, smoothstep | **EXTRACT-AS-IS** |
| `Fbm` | `crates/lwr-world/src/fbm.rs` | 173 | pinned fractal/ridged simplex over `noise::Simplex` | **EXTRACT-AS-IS** |
| `SampleSpace` | `crates/lwr-world/src/sample.rs` | 108 | cell → noise/unit/latitude coordinate mapping | **EXTRACT-AS-IS** |
| `Direction` / `DirectionSet` | `crates/lwr-world/src/direction.rs` | 314 | 8-way compass, bitset-per-cell, `opposite` seam rule | **EXTRACT-AS-IS** |
| `Room` | `crates/lwr-world/src/local/room.rs` | 127 | rect with wall-ring and `too_close` margin test | **EXTRACT-AS-IS** |
| `Chain`/`Pass`/`Phase`/`Context` | `crates/lwr-world/src/local/chain.rs` | 376 | pass pipeline with phase-monotonicity + name-uniqueness asserts | **EXTRACT-WITH-REDESIGN** |
| `LocalMap` | `crates/lwr-world/src/local/map.rs` | 346 | tile grid + 4-way/8-way reachability floods + entry settling | **EXTRACT-WITH-REDESIGN** |
| Road routing (`Terrain`, `route`, `turn_between`) | `crates/lwr-world/src/roads.rs:255-455` | ~200 | turn-costed A* on `(cell, facing)`, octile heuristic | **EXTRACT-WITH-REDESIGN** |
| Road network (`neighbouring_pairs`, `Groups`, `journey`, shortcuts) | `crates/lwr-world/src/roads.rs:456-727` | ~270 | RNG-free relative-neighbourhood graph → MST → shortcut loops | **EXTRACT-WITH-REDESIGN** |
| `passable_regions` / `distance_field` | `roads.rs:462`, `roads.rs:145` | ~70 | connected-component labelling, multi-source BFS distance field | **EXTRACT-AS-IS** (after de-`Biome`-ing) |
| `place_kind` / `has_clearance` / `jitter_at` | `crates/lwr-world/src/sites.rs:587-661` | ~75 | score-jitter-sort-greedy-with-clearance placement | **EXTRACT-WITH-REDESIGN** |
| `Surroundings` / `TileFacts` | `crates/lwr-world/src/local/surroundings.rs` | 262 | one cell + its 8 neighbours, the change-of-scale seam | **EXTRACT-WITH-REDESIGN** |
| `Connect` / `Stranding` / `Steps` / `pave_route` | `crates/lwr-world/src/local/passes/finish.rs:27-338` | ~310 | strand detection, corridor digging, BFS paving | **EXTRACT-WITH-REDESIGN** |
| `Approaches` + `crossing` | `finish.rs:339-479` | ~140 | hash-agreed cross-map seam placement | **EXTRACT-WITH-REDESIGN** (the *idea* is the extractable part) |
| `Vegetation` + `smooth` (4-5 rule) | `local/passes/terrain.rs:154-268` | ~115 | cellular-automaton scatter with directional edge bias | **EXTRACT-WITH-REDESIGN** |
| `Caverns` | `local/passes/terrain.rs:340-440` | ~100 | two-condition CA cave carver | **EXTRACT-WITH-REDESIGN** |
| `Buildings` / `Palisade` / `Plaza` / `Docks` / `Fields` / `Roads` | `local/passes/settlement.rs` | 785 | town construction passes | **GAME-SIDE** (except `Buildings`' rejection sampler) |
| `Castle` / `Ruination` | `local/passes/ruin.rs` | 252 | build-then-degrade | **GAME-SIDE** (the *degrade* operator is generic) |
| `elevation` | `crates/lwr-world/src/elevation.rs` | 375 | warped+ridged noise, squircle shelf, quantile relief | **EXTRACT-WITH-REDESIGN** |
| `climate` | `crates/lwr-world/src/climate.rs` | 257 | latitude/lapse temperature, BFS-coastal moisture | **GAME-SIDE**-ish; **EXTRACT** `distance_to_water` |
| `Biome` + `classify` | `crates/lwr-world/src/biome.rs` | 367 | 18-variant Earth biome enum, Whittaker matrix | **GAME-SIDE** |
| `SiteKind` / suitability scorers | `crates/lwr-world/src/sites.rs:20-345` | ~325 | Town/BanditCamp/Cave/Ruin + `arable_value`/`cover_value`/`rock_value` | **GAME-SIDE** |
| `LocalTile` | `crates/lwr-world/src/local/tile.rs` | 238 | 17 fantasy materials + 4 predicates | **GAME-SIDE** (predicates are the engine part) |
| `Archetype` / `TownArchetype` / `Camp` / `Cave` / `Ruin` | `local/archetype.rs`, `town.rs`, `camp.rs`, `cave.rs`, `ruin.rs` | 540 | which flavour of a place this is | **GAME-SIDE** |
| `chain_for` | `crates/lwr-world/src/local/mod.rs:126-230` | ~105 | the one dispatch site: kind+biome → chain | **GAME-SIDE** |
| `local::fixtures` | `crates/lwr-world/src/local/fixtures.rs` | 239 | `Neighbourhood` test-fixture builder | **EXTRACT-AS-IS** (as a `test-support` feature) |

### `crates/lwr` (6,430 LOC, Bevy)

| Subsystem | Path | LOC | Purpose | Verdict |
| --- | --- | --- | --- | --- |
| `TerminalPlugin` / `Terminal` / `Cell` | `crates/lwr/src/terminal.rs` | 355 | virtual ASCII terminal, back/front buffer diff, letterboxed camera | **EXTRACT-WITH-REDESIGN** |
| `Renderable` trait | `crates/lwr/src/render.rs` | 136 | map → cells, shared by screen and `--dump` | **EXTRACT-AS-IS** |
| `TurnQueue` / `TurnEntry` / `reschedule_at` | `crates/lwr/src/game/turn.rs:29-300` | ~270 | integer-clock binary-heap scheduler, insertion-order tiebreak | **EXTRACT-AS-IS** |
| `TurnSet` + turn systems | `crates/lwr/src/game/turn.rs:300-750` | ~450 | Schedule/Decide/Resolve/Cleanup/Present + stall recovery | **EXTRACT-WITH-REDESIGN** |
| `View` / `clamp_origin` / `follow_cursor` | `crates/lwr/src/game/view.rs` | 164 | scale-agnostic scrolling camera over an `Extent` | **EXTRACT-AS-IS** |
| `Scale` / `Travel` / `LocalScene` | `crates/lwr/src/game/scale.rs` | 190 | the overworld↔local state transition, as a `SystemParam` | **EXTRACT-WITH-REDESIGN** |
| `actor` (`Player`, `Position`, `Appearance`) | `crates/lwr/src/game/actor.rs` | 64 | three components | **GAME-SIDE** (too thin to carry) |
| `draw` | `crates/lwr/src/game/draw.rs` | 490 | blit + actors + chrome | **GAME-SIDE**; lift `blit`/`draw_actors` |
| `input` | `crates/lwr/src/game/input.rs` | 442 | keybinds, movement intents | **GAME-SIDE** |
| `palette` | `crates/lwr/src/palette.rs` | 887 | glyph + colour tables, contrast tests | **GAME-SIDE** |
| `ScreenshotPlugin` | `crates/lwr/src/screenshot.rs` | 174 | in-renderer PNG capture with size-stability wait | **EXTRACT-AS-IS** |
| `dump` | `crates/lwr/src/dump.rs` | 284 | ANSI terminal dump sharing the palette | **EXTRACT-WITH-REDESIGN** |
| `cli` | `crates/lwr/src/cli.rs` | 247 | hand-rolled arg parser | **DROP** (use `clap` in the engine's examples) |

Nothing here is dead. There is no `DROP` verdict on any `lwr-world` module.

---

## B. Coupling & boundary analysis

### The crate split is real and it holds

`crates/lwr-world/Cargo.toml` has exactly one dependency: `noise`. No `bevy`, no `rand`, no `serde`, no
`std::time`. Every module in that crate is pure in the strict sense — a function of `(seed, config)` — and
`Overworld::fingerprint` (`overworld.rs:275-303`) exists solely as a tripwire on that claim. The 249 tests run
in 4.3s with no window, no GPU and no asset loading, which is a direct consequence.

**This is the right model for the new engine and I would not soften it.** The evidence is not the doc comment
but the test wall-clock: `roguelike_engine`'s builder tests drag `bevy::log` and `bracket_lib` in
(`/Users/nathanrude/Development/roguelike_engine/src/map/builders/mod.rs:40-42` imports `bevy::log::debug` and
`std::time::Instant` into the pipeline runner itself), and fantasy-rogue's generation is not testable at all
without an `App`. `lwr-world` shows the alternative is not merely possible but cheap.

Two caveats:

1. The purity is enforced by nothing but the `Cargo.toml`. There is no CI check, no `#![no_std]`-style guard,
   no `deny` on the dependency graph. One `bevy = ...` line is all it takes to lose it.
2. `local::WIDTH = 96` / `local::HEIGHT = 44` (`local/mod.rs:87-89`) are crate constants, not config, and they
   were chosen to fit the *terminal* (`crates/lwr/src/main.rs:26-29`: 132x50 minus 2 chrome rows). That is a
   renderer decision that has leaked into the engine-free crate as a `pub const`. In an engine this must be a
   field on a config struct.

### Where game content leaks into what should be engine code

This is the crate's central design debt, and it is pervasive rather than incidental. The engine-shaped
algorithms take *content enums* as parameters instead of traits:

- `roads::friction(biome: Biome) -> Option<f32>` (`roads.rs:227-252`) is the cost function for a general
  least-cost-path router, and it is an exhaustive match over 18 fantasy-Earth biomes. `Biome::MountainPeak |
  SeaIce | Coast | Ocean | DeepOcean => return None` hard-codes "roads do not cross water" into the router.
- `sites::arable_value` / `cover_value` / `rock_value` (`sites.rs:363-441`) are three more exhaustive
  `Biome` tables. A sci-fi game has no `Biome::Savanna` and no notion of arable land.
- `local::passes::terrain::ground_for` (`terrain.rs:38-52`) and `growth_for` (`terrain.rs:297-315`) map
  `Biome → LocalTile`, i.e. content to content, inside the shared crate.
- `Connect` is parameterised over `fill: LocalTile` and `paving: LocalTile` (`finish.rs:27-42`) — good — but
  its strand-worthiness rules are decided by `LocalTile::is_buildable()` and `is_walkable()`
  (`finish.rs:135-137`, `tile.rs:82-146`), so the algorithm reads tile semantics off a fantasy enum.
- `chain_for` (`local/mod.rs:126-230`) dispatches on `SiteKind` and `Biome` and builds `Vegetation`,
  `Plaza`, `Docks`, `Palisade`, `Fields` inline. This is the game, sitting in the engine-free crate.

The saving grace is that `LocalTile`'s four predicates (`is_walkable`, `blocks_sight`, `is_growth`,
`is_buildable`) are *exactly* the trait the engine wants. They are already the only thing the generic passes
consult. Turning `LocalTile` into `trait TileSemantics` is a mechanical change, and everything downstream of
it becomes theme-agnostic in one move.

### Dependency direction

Clean. No cycles, and one of them was avoided on purpose and documented: `Roads::distance_field`
(`roads.rs:145-171`) lives in `roads` rather than `sites` specifically so `sites` need not name `roads` — the
second placement stage receives a `Grid<u16>` distance field instead (`overworld.rs:118-131`,
`sites::with_wilds` at `sites.rs:520`). That is the correct instinct: pass *data*, not *modules*.

The two-stage placement (towns → roads → everything else) is a genuinely good structural finding and it is
worth carrying into the engine as a documented pattern: some placements are inputs to structure and some are
consumers of it, and trying to do both in one sweep is what forces the cycle.

### System ordering and SystemSet contracts

`crates/lwr/src/game/mod.rs:108-152` is the only ordering authority in the game crate, and it is well done:

- `TurnSet::{Schedule, Decide, Resolve, Cleanup, Present}` chained once, with `run_if(in_state(Scale::Local))`
  **and** `run_if(resource_exists::<LocalScene>)` applied to the *set tuple* rather than to each system. The
  comment at `mod.rs:88-92` correctly notes that set-level and system-level conditions AND rather than
  override, which is why per-system repetition would be misleading noise.
- The single gate is load-bearing beyond tidiness: it is what stops a `MovementIntent` message written in
  `Decide` from surviving into the overworld and being read a frame later against a map that no longer
  exists (`mod.rs:97-104`).
- The overworld frame is a plain `.chain()` because it has no stages worth naming (`mod.rs:73-85`).

Compare fantasy-rogue, where ordering is split across three owners and documented as a gotcha in `CLAUDE.md`.
LWR's model — **one file owns cross-cutting set placement, plugins order only within their own sets** — is the
same rule, but with five sets instead of a sprawl, and it is legible in one screen.

The weakness for an engine: `TurnSet` is defined in `game/turn.rs` and consumed in `game/mod.rs`, both inside
the binary crate. An engine would need to export the set enum and let games slot systems in, which raises the
question of whether the engine should own an `AppState`-equivalent at all. My answer in F.

### Event/message patterns

`MovementIntent`, `TurnSpent`, `TurnRefused` (`game/mod.rs:60-62`). This is a good seam and better than
fantasy-rogue's: the messages are *decisions*, not *effects*. `Decide` writes an intent, `Resolve` reads it and
either applies it or refuses, `Cleanup` reads the spent/refused verdict and reschedules. That three-message
protocol is the whole reason the turn queue itself contains no Bevy types beyond `Entity`.

The single-frame lifetime of a `MessageReader` makes this fragile in one specific way — a refused turn that
nobody rescheduled would strand the actor forever — and the repo knows it: `recover_stalled_actors` runs
*first* in `Cleanup` precisely so a stall is recovered inside the detecting frame (`game/mod.rs:105-108`,
`turn.rs` `MAX_CLOCK_ADVANCES_PER_FRAME` at `turn.rs:48`). That is the right instinct but it is a net under a
protocol that should not need one. An engine should make "an actor is either in the queue or wearing
`TakingTurn`" a type-level invariant rather than a recovered-from failure.

---

## C. Traits and extension points

### Traits that exist

**`Pass`** (`local/chain.rs:82-92`) — three methods: `name() -> &'static str`, `phase() -> Phase`,
`apply(&mut Context)`. This is the best trait in any of the three repos, and the reason is `name()`.

It is not a log label. It **keys the pass's random stream**: `ctx.rng = Rng::new(stage_seed(seed,
pass.name()))` (`chain.rs:161`). Two consequences fall out for free:

1. Inserting, removing or reordering a pass cannot change what any other pass draws. There is a test for
   exactly this (`chain.rs:308-330`, `adding_a_pass_does_not_disturb_the_ones_before_it`).
2. Two passes sharing a name would silently draw an identical sequence, so the chain **asserts** on duplicate
   names (`chain.rs:145-153`) with a message explaining why. The realistic trigger — one parameterised
   `Vegetation` used twice for canopy and undergrowth — is called out by name and covered by a
   `#[should_panic]` test (`chain.rs:286-300`).

Compare `roguelike_engine`'s `MapBuilder<C>` (`builders/mod.rs:257-270`): `name()` there is "human-readable
name for timing logs" and RNG comes from a single shared `ctx.rng()`. That means inserting a builder reshuffles
every builder after it. LWR's version is strictly better and the difference is one line.

**`Phase`** (`chain.rs:41-55`) — five variants, `PartialOrd`, and `Chain::run` asserts non-decreasing order
(`chain.rs:136-143`). `roguelike_engine` has the same idea with six variants
(`builders/mod.rs:230-246`) but weakens it two ways: `phase()` returns `Option<BuilderPhase>` defaulting to
`None` ("legacy builders may not declare a phase"), and there is a `build_map_unchecked()` escape hatch
(`builders/mod.rs:347`). Both are pressure valves that make the invariant advisory. LWR has neither, and is
right not to.

**`Renderable`** (`crates/lwr/src/render.rs:22-32`) — `extent() -> UVec2`, `cell_at(i32, i32) -> Option<Cell>`.
Two methods, implemented for both `Overworld` and `LocalMap`, consumed by both the screen blit and the ANSI
dump. Its doc comment records exactly why it exists: four call sites had drifted apart. This is a textbook
narrow seam and it should go in the engine verbatim.

### Traits that are conspicuously missing

The crate makes a specific bet — **`#[non_exhaustive]`, `Custom { id }` and trait objects are all absent; every
content axis is a closed enum with an exhaustive `match`** — and it defends the bet explicitly:

> "An exhaustive match rather than a trait: there is one dispatch site, so a trait would buy no polymorphism,
> and this buys something a trait would not. Adding a `SiteKind` without giving it an archetype is a compile
> error here rather than a place that generates as wilderness at runtime." — `local/mod.rs:107-110`

> "Matched exhaustively rather than with a `_` arm, so a new biome is a compile error here instead of silently
> becoming grassland underfoot. Every other table keyed on `Biome` is exhaustive for the same reason: between
> them they are the checklist for adding one." — `terrain.rs:33-36`

For a single game this is the correct call and it clearly worked: `local/mod.rs:7-27` gives an honest,
measured account of what adding a fourth site kind actually cost (seven files, ~18 edits, "still cheap"), and
the compile errors are the checklist.

**For a multi-theme engine it is exactly the wrong call**, and the cost is not gradual — it is total. A sci-fi
game cannot add `Biome::Nebula`; it must fork the enum, and forking `Biome` forks `friction`, `arable_value`,
`cover_value`, `rock_value`, `ground_for`, `growth_for`, `undergrowth_for` and `classify` with it. This is the
single most important thing to change in extraction, and it is the concrete answer to the theme question in F.

Traits that should exist and do not:

| Missing trait | What it would abstract | Current hard-coding |
| --- | --- | --- |
| `TileSemantics` | walkable / opaque / buildable / diggable | `LocalTile`'s four predicates, `tile.rs:82-146` |
| `TerrainCost` | step cost, impassability | `roads::friction`, `roads.rs:227` |
| `Suitability` | "how good is this cell for X" | three `*_value` tables, `sites.rs:363-441` |
| `RngSource` | the randomness contract | `Rng` is a concrete struct threaded everywhere |
| `ScoredPlacement` | score → jitter → sort → greedy-with-clearance | `place_kind`, `sites.rs:598-633` |
| `Renderer` back-end | terminal / sprite / html | `Renderable` exists but `Cell` is `bevy::Color`-typed |

Note that `Rng` being concrete is *defensible* — the entire determinism guarantee rests on the exact bit
pattern of SplitMix64 (`rng.rs:21-27`, pinned by a reference-vector test at `rng.rs:80-89`), and a trait would
invite someone to swap in something whose stream is not stable across versions. But an engine still wants the
seam, if only so a game can use a counter-based RNG for parallel worldgen. The right shape is a trait with
`Rng` as the blessed default implementation, not a bare trait.

### Where traits are taken as parameters

Sparingly and well, where it happens: `Grid::from_fn(w, h, impl FnMut(u32,u32) -> T)` (`grid.rs:31`),
`Grid::nearest_from(from, impl Fn(&T) -> bool)` (`grid.rs:96`), `Surroundings::new(pos, here, impl FnMut(i32,
i32) -> Option<TileFacts>)` (`surroundings.rs:64`), `place_kind(..., score_at: impl Fn(u32,u32) -> f32)`
(`sites.rs:606`), `TurnQueue::pop_due(is_alive: impl Fn(Entity) -> bool)` (`turn.rs:249`).

That last one is worth singling out. Liveness is asked of the caller rather than looked up, which is precisely
what lets the scheduler's whole ordering contract be proved with `Entity::from_raw_u32` and no `World`
anywhere (`turn.rs:22-27`). It is the cleanest example in any of the three repos of "take the capability as a
parameter and the Bevy dependency disappears".

---

## D. Performance review

Worldgen is fast enough today and the profile trick is doing real work. But three hot paths will not scale,
and one data-layout choice is wrong for an engine.

### The road router allocates ~2.6 MB per candidate pair

`route()` (`roads.rs:368-448`) allocates three full grids on every call:

```rust
let mut best      = Grid::filled(width, height, [u32::MAX; 8]);              // 32 B/cell
let mut came_from = Grid::filled(width, height, [None::<((u32,u32),u8)>; 8]); // 96 B/cell
let mut done      = Grid::filled(width, height, [false; 8]);                  //  8 B/cell
```

136 bytes per cell, times 19,200 cells at the default 160x120 = **2.6 MB, freshly zeroed, per call**. And
`route` is called twice per road: once over empty ground to price every candidate pair
(`roads.rs:648-654`) and again against the roads laid so far (`roads.rs:700-706`). With ~40 towns the relative
neighbourhood graph yields roughly 80 pairs, so a default world does ~120 calls and churns ~300 MB. At
600x400 that is 32 MB per call. This is the whole of the 1.11s in the timing table.

The `came_from` layout is the worst part: `Option<((u32,u32),u8)>` is 12 bytes (padding), 8 slots, and it is
*only* read during path reconstruction. Three fixes, all straightforward:

1. **Reuse the buffers.** Hoist the three grids into `Terrain` (or a `Router` scratch struct) and clear only
   the cells actually touched, tracked in a `dirty: Vec<usize>`. A* touches a small fraction of a large map.
2. **Pack the state.** `best` and `done` can be one `Grid<[u32; 8]>` with a sentinel; `came_from` can be
   `Grid<[u8; 8]>` storing the incoming direction index rather than the whole coordinate — the parent cell is
   recoverable from the direction. 32 + 8 = 40 B/cell instead of 136.
3. **Skip the second search where nothing changed.** A candidate pair whose corridor carries no new road
   re-derives the identical path.

The heuristic itself is sound: octile distance scaled by the cheapest admissible step (`roads.rs:378-390`),
searched over `(cell, entry direction)` so turns can be priced (`roads.rs:355-366`). The `(cell, facing)`
state expansion is genuinely clever and is *the* reason the roads read as built rather than machine-found;
`roads_bend_rather_than_zigzag` (`roads.rs:800+`) pins the claim (90°+ bends from 12% of steps down to 1%).
Keep the algorithm, fix the allocation.

### `Connect` is O(regions × cells)

`Connect::apply` (`finish.rs:118-146`) labels regions by calling `self.reach(&ctx.map, x, y)` for every
unlabelled walkable cell, and each `reach` allocates a fresh `Grid<bool>` and floods the whole component
(`map.rs:141-166`). It then does one *more* full `reach` at the end (`finish.rs:175`). For a town with a
handful of regions this is invisible. For a cave — which the code itself says fragments badly at 0.40 floor
over three rounds (`terrain.rs:325-329`) — this is dozens of full-map allocations per map.

The fix is standard: one labelling pass writing region ids directly into a `Grid<u32>` with a single reusable
queue, accumulating size and the `made` flag as it goes. Same output, one allocation, one traversal.

### `Caverns` clones the whole tile grid per round

`terrain.rs:407`: `let before = ctx.map.tiles.clone();` inside the round loop. Five rounds is five full-map
clones. Double-buffer instead: two grids, swap. Trivial, and the same pattern appears in `smooth`
(`terrain.rs:230`) which builds a fresh `Grid<bool>` per round via `Grid::from_fn`.

### Data layout

`Grid<T>` is `Vec<T>` row-major with `index(x,y) = y*width + x` (`grid.rs:5-9`, `grid.rs:72-74`). For the
*world layers* this is the right shape, and notably the design keeps each layer as its **own** grid —
`Grid<f32>` for height, `Grid<f32>` for temperature, `Grid<u16>` for distance-to-water
(`climate.rs:47-54`) — which is structure-of-arrays by accident of good taste. Sweeps over one layer are
dense and cache-friendly.

The `Tile` struct (`overworld.rs:58-70`) is the array-of-structs view, and it is correctly a *constructed
value* rather than storage: `Overworld::tile()` (`overworld.rs:218-230`) assembles one on demand from five
separate grids. That is the right call and the engine should keep it.

Two layout problems:

- `Grid::iter()` (`grid.rs:140-146`) computes `i % width` and `i / width` per element — an integer
  division per cell. `biome_counts` (`overworld.rs:234-241`) and `Roads::length` (`roads.rs:181-186`) walk
  whole maps through it. A nested `for y { for x { } }` or an iterator that increments x and wraps would be
  measurably faster on large worlds.
- `Grid::is_empty()` returns a hard-coded `false` (`grid.rs:62-64`). It is documented ("Always false: grids
  cannot be empty") and true given the constructor asserts, but it is a footgun for a public engine API — a
  caller writing `if !grid.is_empty()` gets a tautology. Either drop the method or make it honest.

### Other hot paths

- `distance_to_water` (`climate.rs:119-146`) and `Roads::distance_field` (`roads.rs:145-171`) are the same
  multi-source BFS written twice. They are correct and linear. Deduplicate into one engine function.
- `Overworld::site_at` was a linear scan and is now a `BTreeMap` index (`overworld.rs:200-216`), with the
  reasoning recorded: `facts_at` asks nine times per local map, and `--dump` asks once per cell. Correct fix,
  correct container (ordered, so iteration cannot vary between runs).
- `neighbouring_pairs` (`roads.rs:520-554`) is O(n³) over sites — 40 towns is 64,000 separation checks, which
  is nothing, but a 400-site world would be 64 million. Worth a note, not a rewrite.
- `TurnQueue::contains` is a linear heap walk (`turn.rs:283-289`), explicitly justified for a few dozen
  actors. Fine now; an engine with hundreds of actors wants an `EntityHashSet` beside the heap. Note the doc
  correctly draws the line: a container may be asked whether it holds something, but iterating one where the
  answer feeds an *ordering* breaks reproducibility.

### The terminal renderer

`spawn_grid` (`terminal.rs:175-238`) spawns **two entities per cell**: a `Sprite` for the background and a
`Text2d` for the glyph. At 132x50 that is 13,200 entities, 6,600 of them `Text2d`. `Text2d` carries a full
text layout pipeline per entity.

The mitigation is good — a front buffer, and only cells whose `Cell` actually changed are touched
(`terminal.rs:246-283`), with the reasoning stated: mutating `Text2d` marks the entity for a full relayout.
In a turn-based game most frames change nothing, so steady-state cost is near zero.

But the *spawn* cost and the memory are paid up front and unconditionally, and it does not scale to a larger
grid or a second terminal. A real engine terminal renderer should be one mesh with a glyph atlas and a
per-cell instance buffer, or a single texture updated on the CPU. That is a rewrite, not a port.

### The `opt-level` profile trick

`Cargo.toml:20-24`:

```toml
[profile.dev.package."*"]
opt-level = 3
[profile.dev]
opt-level = 1
```

Dependencies (`noise`, Bevy) fully optimised; workspace crates at `opt-level = 1` so debug builds stay fast to
compile while noise sampling is not crawling. The comment says a debug run took "seconds per map" without it.
This is a small thing that pays for itself daily and every engine workspace should ship it.

### Benchmarks

**There are none.** No `benches/`, no `criterion` dev-dependency, no `examples/`. `roguelike_engine` has
criterion benches and LWR does not, which is a real gap: every performance claim in this section is inferred
from reading allocation sites and one wall-clock table I measured myself. An engine cannot ship on that.

---

## E. Quality assessment

### Test coverage — the best of the three repos

249 tests in `lwr-world`, ~100 in `lwr`, running in 4.3s. Four distinct styles, each used where it fits:

**Property-over-seed-range** is the dominant idiom and it is used correctly — properties that must hold for
*every* world, not for one lucky seed:

- `land_fraction_is_stable_across_seeds` (`elevation.rs:277-291`) — five seeds, land share within 2% of
  configured. This is the quantile approach's whole claim, asserted.
- `worlds_contain_a_useful_spread_of_biomes` (`overworld.rs:361-380`) — "A world that is 95% ocean or one
  flat prairie is a generator bug, not a taste question."
- `every_town_can_be_reached_from_every_other_a_road_could_get_to` (`roads.rs:735-780`) — six seeds, and it
  reconstructs `passable_regions` to exclude pairs no road *could* join. This is the property that makes the
  road output a network rather than a scattering.
- `towns_never_stand_on_unbuildable_ground`, `towns_sit_nearer_water_than_land_does_generally`,
  `town_count_follows_the_amount_of_land` (`sites.rs` test module).

**Tripwires** where no property can be stated, and labelled as such:

- `splitmix64_matches_reference_vector` (`rng.rs:80-89`) — Vigna's published output for seed 0. "Guards
  against an accidental 'improvement' to the constants silently rerolling every world."
- `sampling_is_pinned_to_known_values` (`fbm.rs:135-149`) — six exact `f64` values.
- `Overworld::fingerprint` (`overworld.rs:275-303`) and `town_digest` (`sites.rs:690+`), the latter filtered to
  `SiteKind::Town` with the reasoning spelled out: the unfiltered fingerprint would change the day a second
  site kind existed and "the obvious fix would be to delete the test".

**Structural guard tests** that fail when a new variant arrives: `Biome::ALL`, `LocalTile::ALL`,
`SiteKind::ALL`, `TownArchetype::ALL` all exist specifically so palette tests iterate the set rather than
naming a member — `SiteKind::ALL`'s doc says it outright (`sites.rs:38-42`): "a test that names one kind
passes forever, however many kinds arrive after it."

**Headless `App` tests** in the game crate (`game/mod.rs:265+`): `App::new()` with `MinimalPlugins` +
`StatesPlugin`, driving real input through `ButtonInput<KeyCode>` and asserting on `Position` queries. These
exercise the actual system graph including state transitions and `DespawnOnExit`, with no window. There is even
a comment measuring the cost (`mod.rs:611`).

Two more things worth stealing:

- **The fixture builder.** `local::fixtures::Neighbourhood` (`local/fixtures.rs`, 239 LOC, `#[cfg(test)]`)
  builds a `Surroundings` fluently: `Neighbourhood::new(Biome::Grassland).at((1,1)).build()`. Without it every
  pass test would hand-assemble nine `TileFacts`. An engine must ship this as a `test-support` feature, not a
  `#[cfg(test)]` module, or downstream games cannot test their own passes.
- **Tests that pin behaviour precisely because it is arbitrary.** `the_nearest_search_scans_a_ring_in_a_fixed_order`
  (`grid.rs:217-228`): "Which of several equally near cells wins decides where a plaza lands and so how a whole
  town is laid out. Changing this reshapes every town ever generated, so it is pinned rather than left to
  chance." That is exactly the discipline a reusable engine needs and almost nobody applies.

Gaps: no benches, no `examples/`, no integration-test directory (everything is `#[cfg(test)] mod tests`
in-file), and `local/cave.rs`, `local/camp.rs`, `local/ruin.rs`, `local/archetype.rs`, `local/passes/ruin.rs`,
`game/scale.rs`, `game/actor.rs` have zero tests of their own (they are covered transitively).

### Documentation

`#![deny(missing_docs)]` on `lwr-world/src/lib.rs:25`; every public item is documented and the crate builds
clean. The `lwr` binary crate has no such attribute but is documented to the same standard anyway.

The prose style is unusual and, for engine purposes, a genuine asset. Doc comments state **why**, and
specifically why-not — what was tried, what broke, what the alternative would cost:

- `rng.rs:1-6` explains why not `rand`: "whose generator output is only stable within a major version".
- `fbm.rs:74-81` warns that `shape` is applied before amplitude scaling, that float multiplication does not
  associate, that at `GAIN = 0.5` the amplitudes are exact powers of two so the orders agree *today*, and that
  a different gain need not — "which is why the tests below pin actual values".
- `roads.rs:76-83` explains `turn_weight` with a measurement: twelve worlds, 90°+ bends from 12% to 1%.
- `local/mod.rs:7-27` retracts an earlier promise ("a pass and a line") with a measured correction ("nearer
  seven files and around eighteen edits") and explains why the retraction matters: "'a pass and a line' was a
  hope, and a promise the next person would have costed their work against."
- `terrain.rs:317-335` documents a failed prototype in numbers: 0.45/5 rounds over-smooths to one cavern,
  0.40/3 fragments so badly that keeping the largest region discards two thirds of the map — "not a tuning
  problem but a stability one".

One doc-test, on `lib.rs:17-23`. That is thin for a library; an engine wants runnable doc-tests on every
extension point, and an `examples/` directory (criterion #5 in the brief) which does not exist here at all.

There is real *over*-documentation in places. `game/turn.rs` is 1,259 lines of which a large fraction is prose;
`TurnQueue::clear_entries` has a 14-line doc comment for a one-line body. `game/draw.rs:31-60` spends thirty
lines on why actors are drawn with `put` rather than `set`. It is all correct and all useful once, but the
signal-to-line ratio drops and it makes the files harder to navigate.

### Determinism model — the best of the three, and it should be the engine's

Three mechanisms, layered:

1. **A generator with a fixed published specification.** SplitMix64, ~20 lines (`rng.rs:21-27`), pinned to
   Vigna's reference vector. The stated reason is that `rand`'s output is only stable within a major version.
   This is correct and it is the thing bracket-lib cannot promise either.
2. **Named streams.** `stage_seed(world_seed, "elevation.continents")` (`rng.rs:57-65`) — FNV-1a over the
   stage name, XORed with the world seed, run once through SplitMix64. Every consumer does this:
   `elevation.rs:145-165` (four streams), `climate.rs:63`/`climate.rs:82`, `roads.rs:637`,
   `sites.rs:449`/`sites.rs:530-570`, `local/mod.rs:96-98`, `chain.rs:161`.
3. **Position-derived draws instead of stream draws where iteration order could matter.** `jitter_at`
   (`sites.rs:653-657`) hashes `(x, y)` into the stage seed rather than pulling from a running stream, "so it
   does not depend on how many cells were visited first. Iteration order can then change without rerolling
   every town." Same trick in `crossing` (`finish.rs:411-425`), where it does something stronger: two
   independently generated neighbouring maps must agree where a road crosses their shared boundary, and they
   manage it by hashing the pair in a canonical order rather than negotiating.

Supporting hygiene: `BTreeMap` not `HashMap` everywhere it matters, with the reason stated ("Ordered, so
nothing about iterating it could vary between runs", `overworld.rs:78-82`); integer fixed-point costs in the
road router so "two runs on the same seed cannot disagree over a last-bit difference in a float compare"
(`roads.rs:254-258`); stable sorts over row-major-built candidate lists so equal scores always break the same
way (`sites.rs:462-465`, `sites.rs:618-621`); candidate pricing done *before* anything is built so build order
cannot affect which roads get built (`roads.rs:645-647`).

**Ranking against the other repos.** fantasy-rogue's `RunSeed::derive(SeedDomain::X, floor)` is the same idea
with an enum instead of a string, which is more typo-proof but requires editing the enum to add a domain — LWR's
strings compose better and are what let `Pass::name()` double as a stream key. fantasy-rogue also has to keep
three RNG families separate by hand (`GameRng`, per-entity `StdRng`, per-floor `StdRng`) and documents that as
a gotcha; LWR needs no such rule because streams are named rather than shared. `roguelike_engine` uses one
`RandomNumberGenerator` for the whole builder chain, so inserting a builder reshuffles everything after it.

**LWR's model wins and the new engine should adopt it wholesale**, with one change: keep the string API for
composition but hash at compile time where possible, and add a debug-mode registry that panics on a
stage-name collision across the whole run rather than only within one chain.

### Notable bugs, smells and design debt

- **Duplicated doc comment.** `sites::with_wilds` has its entire doc block written twice — `sites.rs:494-506`
  and `sites.rs:507-519` say the same three paragraphs in slightly different words. Harmless, visible in
  rustdoc, and a sign the file has grown past comfortable review size.
- **`Grid::is_empty()` hard-codes `false`** (`grid.rs:62-64`). Documented, but a public API tautology.
- **`local::WIDTH`/`HEIGHT` as crate constants** (`local/mod.rs:87-89`) sized to the terminal. A renderer
  concern in the engine-free crate.
- **`place_kind` takes nine parameters** with `#[allow(clippy::too_many_arguments, reason = "one call site,
  all of it needed")]` (`sites.rs:591`). Four of them (`config`, `cells_per_site`, `most`, plus the kind) want
  to be one `PlacementRules` struct — which is exactly the shape the engine trait needs anyway.
- **`generate_towns` deliberately does not call `place_kind`** despite being the same algorithm
  (`sites.rs:583-585`: "Rewriting the town placer to share this code risks moving a town, and no world may
  move"). Honest, and the right call for a live game, but it is duplicated logic that an engine extraction
  gets to delete for free since no worlds are being preserved.
- **`Approaches` `landfall` fallback** slides along the edge up to `span` cells in both directions
  (`finish.rs:456-479`) using `saturating_sub`, so at `offset` near 0 the two arms collapse onto cell 0
  repeatedly. Wasteful rather than wrong.
- **`finish.rs` is 1,304 lines** and `settlement.rs` 785 — past the point where the phase grouping ("grouped
  by what they do rather than by phase", `passes/mod.rs:3-5`) is helping navigation.
- **No CI-enforced boundary.** The engine-free property is a `Cargo.toml` convention with nothing checking it.

---

## F. Recommendations for the new engine, ranked

### 1. Keep the engine-free core crate, and enforce it in CI

Adopt `lwr-world`'s split as the engine's spine: `<engine>-core` with **no** Bevy, no ECS, no async, no
`std::time`. Everything that is a function of `(seed, config) -> data` lives there. Best starting point:
`crates/lwr-world/Cargo.toml` — a manifest with one dependency.

What must change: add a CI job that fails if `bevy` appears anywhere in `cargo tree -p <engine>-core`. LWR's
purity is a convention held by a person, and an engine with contributors needs it held by a machine. Add a
`no_std`-compatible subset if you can afford it; it costs little and it proves the boundary.

### 2. Take LWR's determinism model whole, and make it the engine's advertised guarantee

`crates/lwr-world/src/rng.rs` is 114 lines including tests, and it is the single most valuable file in the
repo. SplitMix64 + `stage_seed(seed, name)` + position-derived draws where iteration order could leak.

What must change: expose `trait RngSource` with `Rng` as the blessed default (games may want counter-based for
parallel worldgen), add the reference-vector test to the engine's own suite, and add a debug-mode global
registry that panics on stage-name collisions across a whole run rather than only within one `Chain`. Keep the
string keys — they are what let `Pass::name()` double as a stream key, and that is the mechanism recommendation
#4 depends on.

Explicitly reject: bracket-lib's RNG (no cross-version stability promise), and `rand`'s `StdRng` (stable only
within a major version, as `rng.rs:1-6` correctly notes).

### 3. Answer the theme question: **themes live in the game, but the engine must ship one worked theme as a separate crate**

The brief asks whether themes should be baked in. LWR is the strongest available evidence for "no", because it
is the repo that baked one in most thoroughly and you can see exactly where the seams tore.

`Biome` (18 Earth biomes), `LocalTile` (17 fantasy materials), `SiteKind` (Town/BanditCamp/Cave/Ruin) and
`TownArchetype` (FishingVillage/WalledTown/Hamlet) are closed enums in the engine-free crate, and every
generic algorithm matches exhaustively on them: `roads::friction`, `sites::arable_value`/`cover_value`/
`rock_value`, `terrain::ground_for`/`growth_for`/`undergrowth_for`, `biome::classify`. A sci-fi game cannot add
a variant; it must fork all eight tables. The exhaustive-match discipline that makes this *safe for one game*
(`local/mod.rs:107-110` argues the case well) is precisely what makes it *unusable across games*.

The fix is not `#[non_exhaustive]` + `Custom { id }` — that gives you the worst of both, since the exhaustive
compile-error checklist evaporates and you are back to string ids at runtime. The fix is **generic parameters
with trait bounds**, which preserves the compile-error property *inside each game*:

```rust
pub trait TileSemantics: Copy + Eq {
    fn is_walkable(self) -> bool;
    fn blocks_sight(self) -> bool;
    fn is_buildable(self) -> bool;
}
pub struct LocalMap<T: TileSemantics> { tiles: Grid<T>, .. }
pub trait TerrainCost<B> { fn step_cost(&self, from: B, to: B) -> Option<f32>; }
```

`LocalTile`'s four predicates (`tile.rs:82-146`) are *already exactly this trait*, and they are already the
only thing the generic passes consult. The change is close to mechanical.

Then ship `<engine>-fantasy` as a separate crate containing LWR's `Biome`, `LocalTile`, `SiteKind`,
`TownArchetype` and the eight tables, unchanged. It becomes the worked example the brief asks for (criterion
#5) and the regression suite for the generic layer, and a sci-fi game ignores it entirely.

### 4. Base the map-builder pipeline on LWR's `Chain`, not `roguelike_engine`'s `BuilderChain`

`crates/lwr-world/src/local/chain.rs` beats `/Users/nathanrude/Development/roguelike_engine/src/map/builders/mod.rs`
on three specific points:

| | LWR `Chain` | `roguelike_engine` `BuilderChain` |
| --- | --- | --- |
| `name()` | keys the pass's RNG stream (`chain.rs:161`) | log label only (`builders/mod.rs:266`) |
| duplicate names | asserts, with a test (`chain.rs:145-153`) | not checked |
| `phase()` | mandatory, `Phase` (`chain.rs:85`) | `Option<BuilderPhase>`, defaults `None` (`builders/mod.rs:268`) |
| escape hatch | none | `build_map_unchecked()` (`builders/mod.rs:347`) |
| Bevy in the runner | none | `bevy::log::debug`, `std::time::Instant` |

Take LWR's. What must change for an engine:

- Generic over the context: `trait Pass<C>` rather than the concrete `Context` struct (`chain.rs:60-79`).
  `Context`'s `plaza`/`buildings`/`doors`/`gates`/`landings` fields are town vocabulary in a generic type.
  `roguelike_engine`'s `BuildContext` trait (`builders/mod.rs:59-78`) has the right shape here and LWR does
  not — take the trait idea from one and the phase/name rigour from the other.
- `Pass::apply` should return `Result`, not panic. Chain-assembly errors panicking is right; a pass that
  cannot place its rooms should be able to say so.
- Add opt-in snapshot capture (`roguelike_engine`'s `take_snapshot` hook, `builders/mod.rs:77`) for mapgen
  visualisation. LWR has no equivalent and it is genuinely useful.
- Keep the phase assert mandatory with no unchecked variant.

### 5. Extract the primitives crate first — it is nearly free

`Grid<T>`, `Direction`/`DirectionSet`, `Room`, `stats`, `Fbm`, `SampleSpace`, `Rng` are ~1,200 LOC with 60-odd
tests and essentially no game coupling. This is one afternoon and it unblocks everything else.

Changes: fix `Grid::is_empty` (`grid.rs:62`); make `Grid::iter` avoid the per-cell `%`//` (`grid.rs:140-146`);
add `Grid::neighbours(x, y, Steps)` since four modules hand-roll the same neighbour loop
(`climate.rs:131`, `roads.rs:159`, `map.rs:150`, `finish.rs:258`); add `Grid::flood`/`Grid::label_regions` as
first-class operations, since `passable_regions` (`roads.rs:462`), `Connect`'s labelling (`finish.rs:118`) and
`LocalMap::flood_from` (`map.rs:141`) are three copies of one algorithm.

### 6. Make the two distance-field BFS copies one engine function

`climate::distance_to_water` (`climate.rs:119-146`) and `Roads::distance_field` (`roads.rs:145-171`) are
character-for-character the same multi-source BFS over different predicates. This generalises directly to
`fn distance_field(w, h, sources: impl Iterator<Item=(u32,u32)>, passable: impl Fn(u32,u32)->bool) -> Grid<u16>`,
which is also a Dijkstra map / flow field — the single most reused structure in roguelike AI. The engine should
ship it once with a `u16` and a `u32` variant, plus a `descend`/`ascend` gradient walk.

Note the design insight that motivated both call sites, worth putting in the docs: "a place that wants to sit
*near* something needs a gradient to climb, not a yes-or-no" (`roads.rs:139-143`).

### 7. Rebuild the road router as a general `PathRouter`, and fix the allocation

`roads.rs:255-455` contains the best pathfinder in any of the three repos: A* over `(cell, entry direction)`
with a quadratic turn cost (`roads.rs:325-328`), an octile heuristic scaled by the cheapest admissible step
(`roads.rs:378-390`), integer fixed-point costs for exact ordering (`roads.rs:254-258`), and a reuse discount
consulted live so later routes snake onto earlier ones (`roads.rs:290-292`).

Generalise it: take `impl TerrainCost` instead of `friction(Biome)`, make the turn cost and reuse discount
optional features, and expose it as one router serving roads, corridors, rivers and NPC travel.

Fix the allocation per D: hoist `best`/`came_from`/`done` into reusable scratch, store the parent as a
direction index (`[u8; 8]`, 8 B/cell) rather than a coordinate (`[Option<((u32,u32),u8)>; 8]`, 96 B/cell), and
clear only touched cells. 136 B/cell → 40 B/cell and no per-call zeroing.

### 8. Take the network layer (RNG-free MST + shortcuts) and skip petgraph

`neighbouring_pairs` (relative neighbourhood graph, `roads.rs:520-554`), `Groups` (union-find with path
halving, `roads.rs:586-612`), Kruskal via cheapest-first sort (`roads.rs:672-681`), `journey` (Dijkstra over
the site graph, `roads.rs:557-583`), and shortcut re-admission by detour ratio (`roads.rs:686-694`).

That is roughly 130 lines of actual algorithm for a settlement network with real junctions. petgraph would
supply the MST and the Dijkstra but not the relative-neighbourhood pruning, not the region-aware candidate
filter, and not the shortcut heuristic — which are the three parts that make the output *read* as a road
network. It would also add a dependency to a crate whose whole selling point is having one.

**Recommendation: do not adopt petgraph for this.** Do lift `Groups` into the primitives crate as a public
`DisjointSet`, since choke-point analysis and region merging want it too.

The one caveat: `neighbouring_pairs` is O(n³) (`roads.rs:531-548`). Fine to 100 sites, quadratic-with-a-constant
past that. Document the bound.

### 9. Ship the change-of-scale architecture as a first-class engine concept

LWR's overworld→local model is its most distinctive structural idea and neither other repo has anything like
it. Three parts:

- **`Surroundings`/`TileFacts`** (`surroundings.rs`) — a cell plus its eight neighbours, passed to local
  generation so a coastal town's piers run out over sea that is really there (`surroundings.rs:1-10`).
- **Nothing is stored.** A local map is a pure function of `(world_seed, cell_position)` via `local_seed`
  (`local/mod.rs:92-96`), so revisiting rebuilds exactly what you left (`local/mod.rs:50-54`). No save format
  needed until places gain mutable state — and the doc says exactly when a cache starts earning its keep.
- **Hash-agreed seams.** `crossing` (`finish.rs:411-425`) lets two independently generated neighbours agree
  where a road crosses their shared boundary by hashing the cell pair in canonical order. No negotiation, no
  shared state, no generation order dependency. This generalises to any cross-chunk feature: rivers, walls,
  corridors, tunnels.

The seam-hash trick is the reusable gem. It should be an engine primitive:
`fn seam_value(seed, a: (i32,i32), b: (i32,i32), range) -> u32`.

What must change: `TileFacts` is fantasy-typed (`biome: Biome`, `site: Option<SiteKind>`,
`land_height`/`temperature`/`moisture`). Make `Surroundings<F>` generic over the facts type; the neighbourhood
machinery does not care what a fact is.

### 10. Take the turn scheduler as-is; harden the protocol around it

`TurnQueue` (`crates/lwr/src/game/turn.rs:29-300`) is small, correct and unusually well reasoned:

- Integer clock throughout, with the rejection of `f32` speed multipliers argued explicitly (`turn.rs:64-70`:
  "an `f32` clock cannot promise that the same inputs always give the same order once it has accumulated
  enough additions to start losing the low bits").
- `Ord` on `(time, insertion_order)` and deliberately **not** on `Entity`, with the reason given
  (`turn.rs:96-107`): Bevy orders `Entity` by a bit pattern that runs *inverted* relative to the index, so a
  third key meaning "whoever was scheduled first" would say the opposite.
- Hand-written `PartialEq` matching exactly the two fields `Ord` compares, because a derive over three fields
  would violate Rust's `a == b ⟺ cmp == Equal` contract (`turn.rs:121-131`) — a bug the design it was ported
  from has latently.
- `pop_due(is_alive: impl Fn(Entity) -> bool)` (`turn.rs:249`) takes liveness as a parameter, which is what
  lets the entire ordering contract be tested with `Entity::from_raw_u32` and no `World`.
- `clear_entries` preserves the clock and the insertion counter, both with stated reasons (`turn.rs:291-300`).

Only `Entity` ties it to Bevy. Make it generic over an `Id: Copy + Eq` and it drops into the core crate.

What must change: the surrounding protocol. `recover_stalled_actors` (`game/mod.rs:105-108`,
`MAX_CLOCK_ADVANCES_PER_FRAME` at `turn.rs:48`) is a net under a `MessageReader` lifetime hazard — an actor
whose `TurnSpent` nobody read is stranded. An engine should make "in the queue XOR wearing `TakingTurn`" a
type-level invariant, e.g. by having the scheduler hand out a `TurnToken` that must be consumed.

### 11. Ship `Renderable` and a terminal back-end, but rewrite the renderer

`render.rs:22-32` — two methods, `extent()` and `cell_at()`. Its doc records that four call sites had already
drifted apart before it existed. Take it verbatim, but make `Cell` generic or engine-owned rather than
`bevy::Color`-typed (`terminal.rs:20-28`), so a headless ANSI dump does not pull in Bevy.

**Yes, a Bevy terminal-grid renderer belongs in the engine** — every roguelike needs one and writing it is
tedious and easy to get subtly wrong (LWR's letterboxed `AutoMin` projection at `terminal.rs:189-197`, the
front-buffer diff at `terminal.rs:246-283`, `ImagePlugin::default_nearest()` at `main.rs:78` are all
non-obvious and all correct). But ship it as a **separate optional crate** — `<engine>-term` — behind a feature,
and **rewrite the backing store**: two entities per cell with a `Text2d` each (`terminal.rs:203-234`) does not
scale. One quad, a glyph atlas, a per-cell instance buffer.

Also take `ScreenshotPlugin` (`crates/lwr/src/screenshot.rs`) into the same crate. Capturing from inside the
renderer rather than grabbing the window, and waiting for the drawable size to *stop changing* rather than for
a fixed frame count (`screenshot.rs:14-22`), is exactly right and is the difference between a usable visual
regression tool and one that produces images of differing height between runs of the same binary.

### 12. Ship the test-support crate, and steal the test idioms

Make `local::fixtures::Neighbourhood` (`crates/lwr-world/src/local/fixtures.rs`) a public
`<engine>-test-support` crate rather than a `#[cfg(test)]` module. Games writing their own passes need to build
a neighbourhood without hand-assembling nine facts, and today they cannot.

Codify three idioms in the engine's own suite and document them for downstream use:

- **Property-over-seed-range**, not one-golden-seed. `elevation.rs:277-291`, `overworld.rs:361-380`,
  `roads.rs:735-780`.
- **A fingerprint tripwire** where no property can be stated, labelled as a tripwire so nobody mistakes it for
  a property. `overworld.rs:275-303`, `sites.rs:690+`.
- **`ALL` constants** on every content enum, with palette/table tests iterating them so a new variant fails a
  test rather than reaching a player. `sites.rs:38-42` argues this better than I can.

### 13. Add the benches and the examples that LWR lacks

There is no `benches/`, no `examples/`, no criterion. Every performance claim in section D is inferred from
reading allocation sites. Criterion #3 and #5 in the brief both fail here.

The engine needs, at minimum: benches for FOV, pathfinding (with and without turn costs), distance fields,
a full worldgen pass chain, and label-regions; and `examples/` covering a minimal roguelike, a custom tile
type implementing `TileSemantics`, a custom `Pass`, and a headless world dump. `roguelike_engine` has criterion
benches already and is the better starting point for the bench harness.

### 14. Ship the `opt-level` profile trick in the engine template

`Cargo.toml:20-24`. Dependencies at `opt-level = 3`, workspace crates at `1`. The comment records that without
it a debug run took seconds per map. Costs nothing, pays daily, and worldgen-heavy engines need it more than
most.

### 15. Crate split boundaries

Based on what actually separates cleanly in this repo:

| Crate | Contents | Deps |
| --- | --- | --- |
| `<engine>-core` | `Grid`, `Direction`/`DirectionSet`, `Room`, `Rng`/`stage_seed`, `stats`, `DisjointSet`, distance fields, flood/label, `TileSemantics` + `TerrainCost` traits | none |
| `<engine>-noise` | `Fbm`, `SampleSpace`, quantile-based banding helpers | `noise` |
| `<engine>-mapgen` | `Chain`/`Pass`/`Phase`, generic passes (CA, rooms, connect, strand, pave), snapshots | core |
| `<engine>-path` | A* with optional turn cost, Dijkstra maps, flow fields, RNG-free network builder | core |
| `<engine>-world` | overworld scaffolding, `Surroundings<F>`, seam hashing, scored placement | core, noise |
| `<engine>-turns` | `TurnQueue<Id>`, action-cost model | none |
| `<engine>-bevy` | plugins, `TurnSet`, `Scale`/`Travel`, `View` | bevy + the above |
| `<engine>-term` | terminal grid renderer, `Renderable`, screenshot | bevy |
| `<engine>-fantasy` | LWR's `Biome`/`LocalTile`/`SiteKind`/`TownArchetype` + tables, as the worked example | the above |
| `<engine>-test-support` | `Neighbourhood`-style fixtures, seed-range property helpers | core |

The load-bearing line is between everything above `<engine>-bevy` and everything from it down. LWR proves that
line can be held; the CI check in recommendation #1 is what keeps it held.

---

## Closing note on what not to copy

Three things in this repo are right for a personal game and wrong for an engine, and it is worth naming them so
they are not carried across by admiration:

1. **Exhaustive matches on closed content enums.** They are the reason adding a biome is a safe compile-error
   checklist, and the reason a sci-fi game cannot use the crate. Section F.3.
2. **Refusing to refactor because worlds would move** (`sites.rs:583-585`). Correct when seeds are a promise to
   a player; pure cost when you are building a library nobody has generated worlds with yet. Extraction is the
   one moment this constraint does not apply — spend it.
3. **The doc-comment density.** The *content* of these comments — what was tried, what broke, what the
   alternative costs — is the best I have seen in the three repos and should absolutely be the engine's house
   style. The *volume* in `turn.rs` and `draw.rs` should not be.
