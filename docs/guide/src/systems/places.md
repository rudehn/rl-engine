<!-- documents:
     plugins: StreamingPlugin
     files: crates/rl-bevy/src/places.rs
            crates/rl-bevy/src/world.rs
            crates/rl-bevy/src/turn.rs
            crates/rl-bevy/src/plugin.rs
            crates/rl-bevy/src/knowledge.rs
            crates/rl-bevy/src/minds.rs
            crates/rl-world/src/chunk.rs
            crates/rl-world/src/graph.rs
     fingerprint: 85a9f062 -->

# Places and streaming

A game's maps are of two kinds, and everything that reads tiles reads both through one resource.
A place is bounded, built once by the game's rules the first time something enters it, and kept whole from then on, actors and items included.
The surface is streamed: a window of regions around the player, generated on demand, dropped when it leaves the window, and remembered only as the edits made to it.
Every entity with a position is on exactly one map, and the turn loop freezes whatever is not on the one being played.

## Turning it on

Places need no plugin: `CorePlugin` puts `resolve_warps` in `ResolveSet::Travel` and `tag_new_positions` in `TurnSet::Schedule` before anyone is dealt a turn, and declares `needs::<WorldMap>`, so a game that builds no map is told so by name when play begins.
What a game adds is a `PlaceRulesRes` wrapping its `PlaceRules`, and the engine calls it the first time a map id is entered.
`StreamingPlugin` is the surface, and it is the part a game leaves out: it adds `stream_chunks` in `EngineSet::Stream` and declares `needs::<WorldRes>` and `needs::<ChunkRulesRes>`, each with a hint naming what to build one from.
`EngineSet::Stream` runs first in the frame so the window is loaded around wherever the player ended the previous one before anybody is dealt a turn on it.
A game of floors and nothing above them adds neither: with no `WorldRes` nothing streams, `WorldMap::new` takes the tile tables and nothing else, and how big a region is never enters its vocabulary.
`WorldSettings` is how much stays loaded, a `window_radius` of 1 giving the 3x3 default.

## The model

`MapId` is a `u32`, `MapId::SURFACE` is zero, and a game numbers its places however it likes above that.
`OnMap` is the component saying which; it is missing until something first gains a position, and `tag_new_positions` fills it in with the current map.
`WorldMap` is the resource every reader reads: `tile`, `set_tile`, `is_walkable`, `is_opaque`, `blocks_projectiles`, `cost`, `opens`, `closes` and `is_loaded` all answer for the current map, whichever kind it is, and `None` or opaque-and-impassable for a cell that is not loaded.
`current`, `window`, `window_tiles`, `to_world` and `to_local` say which map and which window; `view` and `opening_view` hand a `WindowView` to a grid algorithm, the second costing a closed door as the turn spent opening it plus the step onto what it opens into, so a flood routes through a door when that is shorter.
Three counters tell readers when to recompute without anyone asking them: `generation` moves when the window moves or the map changes, `opacity_epoch` when an edit changes what blocks sight, and `cost_epoch` when one changes where an actor may walk or what a step costs.
`set_veil` is the fourth case, a set of cells that hide what is behind them for a reason other than their tile, rewritten each turn by whatever makes it and lifted whenever the map or the window under it changes.
`PlaceRules::build` is the one method a game writes, taking a `MapId` and the `WorldGraph` when there is one, and returning a `PlaceBuild`: a `Terrain`, an `entry`, an optional `exit`, and a `Vec<Spot>` of points of interest tagged in the game's own numbering.
`PlaceBuild::from_context` is the usual way to make one from a finished chain, taking the chain's `StartPoint` as the entry, its `ExitPoint` as the exit, and every prefab mark as a `Spot` tagged with its character.
A `Spot` from a keyed piece carries its `prefab`, the key set on the `Prefab` that stamped it, so the game can trace it back to what defined it; `None` for a spot a game made itself or a mark of an unkeyed piece.
`Transition` is a component on an entity standing on a cell, holding the `Destination` it leads to: a cell of the surface, or a place with an `Arrive` of its entry, its exit or a named cell.
`GoThrough` is the action that takes the player through the one it is standing on, and `WarpRequest` does the same from anywhere for a portal or a first arrival, resolved without charging a turn so the game charges what it likes.
Only the player travels; anyone else asking to go through fails like any other impossible action.
A warp that lands on a map nobody has built calls the rules, installs the result with `install_place`, and writes `PlaceEntered` with `first` true, which is the one arrival on which a game populates a floor; `MapChanged` is written on every change of map.
The warp also switches the per-map indexes with the map: `Occupancy` swaps its `SpatialGrid` for the arriving map's and stashes the one it had, `Knowledge` does the same with explored tiles, and `FlowFields` is invalidated so the next mind rebuilds.
On the streaming side, `stream_chunks` asks `desired_window` for the square of radius `window_radius` around the player's region clipped to the world, and does nothing if that is the window already or if the player is in a place.
Loading generates each new region through `WorldGraph::build_chunk` and the game's `ChunkRules`, replays that region's stored delta onto it, keeps every chunk still in the window, and files the edits of the ones that left.
`ChunkLoaded` is written once per region generated, and every viewshed is marked stale when the window moves.
`export` and `import` are the save shape: every surface edit, loaded or not, and every built place whole, with loaded chunks dropped on import so the next stream replays the edits onto fresh generation.

## Using it

A game reaches a place by spawning a `Transition` on a cell, and where those cells are is the game's to decide: corsair puts a cave mouth in every cove the moment the region holding it streams in.

<!-- include: ../../../../examples/corsair/src/places.rs:entrances -->
```rust,no_run
/// Regions whose cave mouth has been placed.
#[derive(Resource, Default)]
pub struct Entrances(pub std::collections::BTreeSet<Point>);

/// Puts a cave mouth in the middle of each cove the first time it streams in.
pub fn mark_entrances(mut commands: Commands, mut loaded: MessageReader<ChunkLoaded>, mut done: ResMut<Entrances>, world: Res<WorldRes>) {
    for ev in loaded.read() {
        let Some(site) = world.site_index_at(ev.region) else { continue };
        if world.sites()[site].kind != COVE || !done.0.insert(ev.region) {
            continue;
        }
        let at = world.region_tiles(ev.region).center();
        commands.spawn((Stairway, Position(at), Transition { to: Destination::Place { map: cave_id(site, 0), arrive: Arrive::Entry } }, stair_glyph('>')));
    }
}
```

## The line

The engine decides when a place is built, which is the first time anything enters it, and that it is never built twice.
It decides that a place is kept whole once built, so leaving one freezes what is in it and returning finds it as it was, and that the surface is not: a region outside the window is thrown away and only its edits survive.
It decides that a chunk is a pure function of the run seed and its region, through `WorldGraph::chunk_seed`, so a region regenerates identically however many times it has been walked across, which is what makes storing a delta instead of a map correct rather than merely cheap.
It decides that only the player travels, that an actor on another map or outside the loaded window is frozen and requeued a full step without acting, and that such an actor can neither act nor be seen.
The game decides what a map id means, what is built there, where the transitions stand and what they lead to, what an arrival costs, and everything that goes in a place on the arrival `PlaceEntered` marks as its first.
The game also decides whether there is a surface at all, and that decision is made by adding `StreamingPlugin` or not rather than by any resource being present or absent.
Which regions are in the window is the engine's; what a region generates into is the game's `ChunkRules`, and the seam where two chunks meet is arithmetic both sides compute from the same unordered pair of regions rather than a negotiation either could lose.
A game that wants a region populated the first time it is seen listens for `ChunkLoaded` and remembers which it has done, because the engine will load a region again after it has been unloaded and will not tell it apart from the first time.

## Where it lives

`rl-bevy` is tier 2 and owns both halves, because both are about what the running game is reading right now, which is not a question a lower tier can answer.
`places.rs` is the vocabulary and the warp; `world.rs` is `WorldMap`, the window, the chunk store and `StreamingPlugin`.
The part that can be tested without an `App` was pushed below: `rl-world` is tier 1 and owns the world graph, the chunk builder and the seam arithmetic, so that two neighbouring chunks agreeing at their shared edge is a property checked over a range of region pairs by calling `build_chunk` twice, with no frame anywhere.
What is left above the line is genuinely frame-shaped: which map is current, which window is loaded, and which indexes have to be swapped when that changes.
Keeping the surface's whole existence optional is the other thing the split buys, since a game of floors alone links `rl-world` without ever constructing a `WorldGraph`, and nothing in its vocabulary mentions a region.
