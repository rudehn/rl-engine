# Decisions

Synthesized from the ingest set on 2026-09-16.
There is one ADR in the set, `docs/PLAN.md`, classified locked (header: "Status: adopted, revised 2026-09-09 after Nate's review; being built").
It is a consolidated decision log rather than a single-decision record, so each decision is listed separately below.

Precedence inside the ADR: the dated progress log at the top of `docs/PLAN.md` records later revisions.
Where a progress entry contradicts section 3 text, the later entry is the effective decision (intra-document evolution, logged as INFO in `.planning/INGEST-CONFLICTS.md`).
Each entry gives the original statement and, where one exists, the effective state after revision.

Owner decisions recorded in lower-precedence docs are listed at the end, marked with their corroboration.

---

## ADR-PLAN-00 Owner pre-decisions

- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md ("Decisions Nate has already made")
- status: locked
- scope: project framing
- decisions:
  - The engine repo is `rl-engine`.
  - First consumer was WildReach, a Sunlorn-like open world larger than 1024x1024.
    Effective: WildReach abandoned 2026-09-10; Corsair (in-workspace) became the consumer, then two examples (Corsair open world, delve dungeon) on 2026-09-13; `examples/heist` also exists per docs/OVERVIEW.md.
  - Corsair is a small pirate-themed example game inside the workspace.
  - The overworld stays as a picture and a portal-destination picker, in an opt-in crate.
  - Far travel is walking once, then portal; travel does not simulate time; no route-travel command.
    Effective: still in force for the engine today; the PLAN "Next" line (Nate, 2026-09-10) says the living-world-rogue conversion keeps overworld token movement so `rl-overworld` regains travel on the map. See INFO entry in the conflicts report.
  - Rivers are in scope.
  - Build first, course-correct later.
  - `living-world-rogue` is source material for world generation, not the integration target.
    Effective: PLAN "Next" schedules a living-world-rogue conversion after the engine is done.
  - Randomness uses `rand`; cross-version stream stability is not a goal.
  - GOAP is not ported.
  - `rl-ui` is built in from the start.

## ADR-PLAN-01 Own the loop or leave the subsystem out

- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (section 1)
- status: locked
- scope: every engine subsystem
- decision: For every subsystem, either the engine owns the loop (behaviour, not just data structures) or the subsystem is left out.
  A struct plus a `SystemSet` marker is not a subsystem.
  The engine is developed against a real game from the first milestone.
- evidence: /Users/nathanrude/Development/rl-engine/docs/reviews/roguelike_engine.md (headline fact, F1)

## ADR-PLAN-02 Review consensus adopted as principles

- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (section 2)
- status: locked
- scope: engine-wide
- decision: Themes live in the game and engine crates are lexically theme-free; no `#[non_exhaustive]` + `Custom { id }`; no closed taxonomy enums; registries keyed by opaque ids plus traits for behaviour; Bevy-free core enforced by the compiler; renderer must not be one text entity per tile; benches on real hot paths.
- note: "not one text entity per tile" is not yet met; docs/OVERVIEW.md lists instanced terminal rendering as not built ("one sprite per cell is the known scaling limit").

## ADR-PLAN-3.1 Themes out of the engine

- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (section 3.1)
- status: locked
- scope: engine vocabulary, examples
- decision: Mechanics are engine, vocabulary is game.
  Lighting, factions, stealth, statuses, tile fields and quest triggers are engine mechanics; which ones exist is game content.
  Examples carry small shared content; a second deliberately different consumer proves theme-agnosticism.

## ADR-PLAN-3.2 Extension through registries and traits, never enum variants

- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (section 3.2)
- status: locked
- scope: all extension points
- decision: Data extensions use an opaque id and a registry loaded from RON with validate-on-load.
  Code extensions use traits taken as parameters.
  Generic type parameters are reserved for containers and the builder context; a generic `Map<T: TileSemantics>` was considered and rejected.
- effective refinements (progress log): actions are types (`Action` trait, `Intent<A>`, 2026-09-11); effects are types registered with `add_effect` (2026-09-12); `Names` and `NameRef<T>` resolve content names to typed ids (2026-09-13, 2026-09-14); one `Registries` resource (2026-09-14).

## ADR-PLAN-3.3 Randomness

- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (section 3.3)
- status: locked
- scope: all randomness
- decision: `rand` 0.9; `StdRng` for gameplay and world streams (same algorithm native and wasm); `SmallRng` allowed only for cosmetic streams.
  `RunSeed::derive(domain, index)`, open `SeedDomain` constants, a stream per generation pass keyed by its name, position-derived draws where iteration order could leak.
  Functions take `&mut impl Rng`; no `with_seed(u64)`; determinism within one build is an invariant, not across versions.
- effective refinements: the engine owns `Seed`; subsystems register a `Stream` with `add_stream`; a game's own draws come from `Seed::stream` (2026-09-14).
  Minds and stealth roll from their own streams `MindRng` and `StealthRng` (2026-09-16).
  Fire rolls are hashes of seed, turn and cell (2026-09-15).
- open gap: Corsair's loot drop rolls from `CombatRng`, which this decision forbids (source: /Users/nathanrude/Development/rl-engine/docs/TODO.md section 4).

## ADR-PLAN-3.4 Map generation pipeline

- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (section 3.4)
- status: locked
- scope: `rl-mapgen`
- decision: Base on `lwr-world`'s `Chain`: `Pass::name()` keys the RNG stream, duplicates rejected, mandatory `phase()`.
  Generic `BuildContext`, snapshots, fallible `apply`, `Send` chain, typed `emit` output, optional rooms.
  One `Chain` for world passes and dungeon builders.
  Fix three complexity classes when porting: incremental door sites, worklist-pruned choke map, single-pass region labelling.
- effective state: chain, dungeon passes (rooms, BSP, doors, random start, farthest exit) and prefab stamping built; choke map, cullers beyond keep-largest and decoration rules deferred from M4 with no current owner (see WARNING in conflicts report).

## ADR-PLAN-3.5 The overworld as data, never as a mode

- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (sections 3.5, 3.5.1, 3.5.2, 3.5.3)
- status: locked
- scope: world structure, streaming, `rl-overworld`, travel, rivers
- decision: Keep `WorldGraph` as coarse generation structure; generate chunks on demand from it with seam-hash agreement on linear features; walking scale is ground truth; any overworld screen is a lens, never the player's location.
  `rl-overworld` is opt-in: a view plus a portal picker; it never owns the player's position; the `lwr` Scale/Travel state machine is not ported.
  Far travel is walking; portals only to discovered sites; no cross-world route command.
  Rivers via a hydrology pass, `LinearFeature` per region, navigable for a sailing `MovementProfile`.
  Region size is config (default 64x64); 4096x4096 was the design point.
- effective refinements: the surface is optional and a delve is first-class (2026-09-11); places leave the overworld and `WorldMap::new` takes tile tables only (2026-09-11).
- pending revision: PLAN "Next" says the living-world-rogue conversion keeps overworld token movement and `rl-overworld` regains travel (see INFO in conflicts report).
- open gap: sailing profile not wired (docs/TODO.md section 3).

## ADR-PLAN-3.6 Turn scheduling: the engine owns the loop

- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (section 3.6)
- status: locked
- scope: `rl-core` `TurnQueue`, `rl-bevy` turn loop
- decision: Integer-clock heap generic over `Id`; engine-owned phase machinery; the scheduler advances the clock; death resolution before requeue; speed from a trait or message rather than named statuses; only actors in the active region are scheduled.
- effective refinements: every actor due runs within one frame (2026-09-10); `TurnSet::{Decide, Resolve, Sweep, React, Cleanup}` with `DecideSet`, `ResolveSet`, `CleanupSet` sub-sets (2026-09-11 to 2026-09-15); `Resolution` is every resolver's side of the loop (2026-09-13); new actors admitted player first then spawn order (2026-09-16); `TurnHold` for cues (2026-09-15).

## ADR-PLAN-3.7 Pathfinding: Dijkstra maps per pather class, A* for unique goals

- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (section 3.7)
- status: locked
- scope: `rl-grid` pathing, minds
- decision: One `DijkstraMap` per (goal set, movement profile); shared goals use Dijkstra maps, unique goals use A* with a path cache; flee maps by scaled rescan; bounded floods with bucket queue, `u16` cells, reusable scratch; deterministic neighbour order.
- effective refinements: `FlowFields` keyed by `(goals, profile, opens_doors, away)` and kept by cost epoch; tactics use `step_toward` and `step_away_from` (docs/design/minds.md section 3.5, built 2026-09-16).
- open gap: every map is built with `PathRules::default()`, so profiles do not change costs (docs/TODO.md section 3).

## ADR-PLAN-3.8 Drop bracket-lib and petgraph; own the algorithms

- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (section 3.8)
- status: locked
- scope: `rl-core`, `rl-grid`
- decision: The engine owns geometry, Bresenham, symmetric shadowcast FOV into a bitset, A*, `DijkstraMap`, dice; `petgraph` dropped for `DisjointSet`.

## ADR-PLAN-3.9 Data layout for performance

- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (section 3.9)
- status: locked
- scope: engine data structures, benches
- decision: Terrain, occupancy and knowledge are separate; `SpatialGrid`; bitset viewsheds; `TileField<T>` double-buffered, allocation-free, one type for fire, gas, sound, scent, heat and blood; sparse active sets for tile promotion; reusable scratch buffers; interned ids; no `HashMap` or `HashSet` in gameplay paths; shadowcast lighting; viewport rendering.
  Benches from day one on FOV, A*, `DijkstraMap`, lighting at 20 sources, choke map, each builder, world chain at three sizes, `TileField` tick and render sweep.
- effective state: `crates/rl-grid/benches/grid.rs` covers fov, astar, dijkstra, regions, light and tile_field; no choke map, builder, world chain or render sweep benches were found in the repo.

## ADR-PLAN-3.10 Rules layer: shapes in the engine, vocabulary in the game

- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (section 3.10)
- status: locked
- scope: `rl-rules`
- decision: Engine ships damage kinds and resist ladder, status registry, modifier accumulator, slot graph, hook points, targeting footprints, staged damage pipeline, tactic trait, content registries, narration seam, typed world events and triggers, balance checker, faction matrix.
  Tactic-priority AI is the only AI; attacking is its own intent; quests are content over typed events.
- effective refinements: narration is `NarratorPlugin` with a `Phrasebook` rather than a `Narrator` trait (2026-09-15); reactions run in `TurnSet::React` rather than a `ReactionQueue` (2026-09-11); `Loadout` and `fold_gear` (2026-09-15); `Modifier::source` is a `Source` enum (2026-09-15).

## ADR-PLAN-3.11 Bevy layer conventions

- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (section 3.11)
- status: locked
- scope: every Bevy-layer plugin
- decision: Systems run in turn-driven, gated schedules; enough named phases that no plugin writes `.after(concrete_system)`; messages are the engine/game seam; automatic map teardown; a test app builder; Bevy 0.19.
- effective refinements: subsystems are opt-in plugins that declare needs with `app.needs::<R>(plugin, hint)`, reported together on entering play (2026-09-11, 2026-09-13); `EnginePlugins` removed, `RoguelikePlugins::new(title, cols, rows)` holds only non-subsystem basics (2026-09-13); the test builder became `rl_bevy::testing` (2026-09-13); a `Mind` without `MindsPlugin` is reported (2026-09-13).

## ADR-PLAN-3.12 UI reused from the start

- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (section 3.12, including its 2026-09-11 and 2026-09-12 revisions)
- status: locked
- scope: `rl-render`, `rl-ui`
- decision (effective, as revised in-section): every panel splits into a view, a collector and a presenter; the views do not know the backend; terminal presenters first; Bevy UI presenters later over the same views when a game asks for wrapping, hover or sub-cell bars; the map view stays a glyph grid.
  Original text: `GlyphGrid` drawn as one instanced mesh; `rl-ui` as Bevy UI ported from fantasy-rogue.
- detail: /Users/nathanrude/Development/rl-engine/docs/design/ui.md is the reference, with its section 11b superseding earlier sections.

## ADR-PLAN-3.13 Documentation and test rules

- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (section 3.13)
- status: locked
- scope: all crates
- decision: `#![deny(missing_docs)]`; doc-tests compile and run; no blanket clippy allows; why-and-why-not doc comments; every RON schema carries a top-of-file comment listing its full option space; property-over-seed-range, labelled fingerprint tripwires, `ALL` constants, headless `App` tests, guard tests over live assets.

## ADR-PLAN-4 Crate layout by dependency weight

- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (section 4 and progress 2026-09-11)
- status: locked
- scope: workspace
- decision (effective): tier 0 `rl-core`; tier 1 `rl-grid`, `rl-mapgen`, `rl-world`, `rl-rules` (with `content`, `events`, `ai`, `balance` modules); tier 2 `rl-bevy`, `rl-render`, `rl-ui`, `rl-overworld`, `rl-save`; tier 3 `rl-engine` facade.
  Tier 0 and 1 never depend on Bevy and build for wasm; CI enforces it.
  Original: separate `rl-content`, `rl-events`, `rl-ai`, `rl-tools` and `rl-test-support` crates, superseded 2026-09-11.

## ADR-PLAN-5-6 Provenance and exclusions

- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (sections 5 and 6)
- status: locked
- scope: what is ported and what is not
- decision: Not ported: `lwr` Scale/Travel mode switch, GOAP, `roguelike_engine` squad and stealth noise, two-entities-per-cell terminal, bracket-lib, petgraph.
  Not carried: `Custom { id }`, closed taxonomy enums, hand-rolled generators, one text entity per tile, hash containers in gameplay paths, occupancy inside terrain, string def ids in hot loops, melee inside movement, plugins registering other modules' messages, ungated `Update` systems, `std::time::Instant` on wasm, unseeded constructors, `TODO` comments in source.

## ADR-PLAN-8 Milestones

- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (section 8 and progress log)
- status: locked (as history); M0 through M6 first slices built 2026-09-10, deferrals worked through 2026-09-10 to 2026-09-16
- scope: delivery order
- decision: M0 scaffold, M1 walk the world, M2 things that hunt, M3 loot, M4 places, M5 encounters and quests, M6 persist and tune, M7 Corsair.
- effective state: see /Users/nathanrude/Development/rl-engine/.planning/intel/context.md "Built" and /Users/nathanrude/Development/rl-engine/.planning/intel/requirements.md for what remains; M7 Corsair's sea-specific scope (ships, boarding, weather field) has no current owner (WARNING in conflicts report).

## ADR-PLAN-NEXT Next line

- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (progress log, "Next", Nate 2026-09-10)
- status: locked (latest direction in the ADR)
- scope: sequencing
- decision: Finish the remaining deferred pieces (nights on Corsair's surface, scripted encounters, UI phase H deferred until a game asks), then the living-world-rogue conversion once the engine is done; that conversion keeps overworld token movement, so `rl-overworld` regains travel alongside the portal picker, and its maps stream as chunks.
  Work found by the 2026-09-15 architecture review is tracked in `docs/TODO.md`.

---

## Owner decisions recorded outside the ADR

## DEC-MINDS-01 Every actor carries its own Viewshed

- source: /Users/nathanrude/Development/rl-engine/docs/design/minds.md (section 3.1, "Decision, Nate, 2026-09-16")
- corroborated by: /Users/nathanrude/Development/rl-engine/docs/PLAN.md progress 2026-09-16 ("Nate: sight the same for the player and the monsters")
- status: treated as locked (owner-decided and recorded in the ADR)
- scope: sight, minds, stealth, `Watchers`
- decision: A `Mind` requires a `Viewshed` sized by `Perception`; the player's viewshed is no longer the single line-of-sight oracle; `perceivable` is removed.

## DEC-MINDS-02 Health is two fields

- source: /Users/nathanrude/Development/rl-engine/docs/design/minds.md (section 3.4, "Decision, Nate, 2026-09-16")
- related: /Users/nathanrude/Development/rl-engine/docs/PLAN.md progress 2026-09-15 ("health stays a component, since hit points are state and a maximum-health modifier is separate work")
- status: owner-decided; compatible with the ADR
- scope: combat `Health`
- decision: `Health { current, max }`.

## DEC-FIELDS-01 Fire consumes its fuel; rolls are hashed

- source: /Users/nathanrude/Development/rl-engine/docs/design/fields.md (section 3)
- corroborated by: /Users/nathanrude/Development/rl-engine/docs/PLAN.md progress 2026-09-15
- status: in force (built)
- scope: fire, gas
- decision: A tile's `burn.leaves` is required so every fire ends; fire rolls hash seed, turn and cell; gas is registry content; fire runs before gas in `ResolveSet::Fields`; both plugins opt-in.
