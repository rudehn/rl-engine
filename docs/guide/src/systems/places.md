<!-- documents:
     plugins: none
     files: crates/atglyph-bevy/src/places.rs
            crates/atglyph-bevy/src/world.rs
            crates/atglyph-bevy/src/turn.rs
            crates/atglyph-bevy/src/plugin.rs
            crates/atglyph-bevy/src/knowledge.rs
            crates/atglyph-bevy/src/minds.rs
     fingerprint: b71ec517 -->

# Places

A game's maps are places, and everything that reads tiles reads them through one resource.
A place is bounded, built once by the game's rules the first time something enters it, and kept whole from then on, actors and items included.
Every entity with a position is on exactly one map, and the turn loop freezes whatever is not on the one being played.

## Turning it on

Places need no plugin: `CorePlugin` puts `resolve_warps` in `ResolveSet::Travel` and `tag_new_positions` in `TurnSet::Schedule` before anyone is dealt a turn, and declares `needs::<WorldMap>`, so a game that builds no map is told so by name when play begins.
What a game adds is a `PlaceRulesRes` wrapping its `PlaceRules`, and the engine calls it the first time a map id is entered.
`WorldMap::new` takes the tile tables and nothing else.

## The model

`MapId` is a `u32`, and a game numbers its places however it likes; `MapId::ZERO` is where an entity with no `OnMap` stands.
`OnMap` is the component saying which; it is missing until something first gains a position, and `tag_new_positions` fills it in with the current map.
`WorldMap` is the resource every reader reads: `tile`, `set_tile`, `is_walkable`, `is_opaque`, `blocks_projectiles`, `cost`, `opens`, `closes` and `is_loaded` all answer for the current map, and `None` or opaque-and-impassable for a cell outside it.
`current`, `window_tiles`, `to_world` and `to_local` say which map and what it covers; `view` and `opening_view` hand a `WindowView` to a grid algorithm, the second costing a closed door as the turn spent opening it plus the step onto what it opens into, so a flood routes through a door when that is shorter.
Three counters tell readers when to recompute without anyone asking them: `generation` moves when the map changes, `opacity_epoch` when an edit changes what blocks sight, and `cost_epoch` when one changes where an actor may walk or what a step costs.
`set_veil` is the fourth case, a set of cells that hide what is behind them for a reason other than their tile, rewritten each turn by whatever makes it and lifted whenever the map under it changes.
`PlaceRules::build` is the one method a game writes, taking a `MapId` and returning a `PlaceBuild`: a `Terrain`, an `entry`, an optional `exit`, and a `Vec<Spot>` of points of interest tagged in the game's own numbering.
`PlaceBuild::from_context` is the usual way to make one from a finished chain, taking the chain's `StartPoint` as the entry, its `ExitPoint` as the exit, and every prefab mark as a `Spot` tagged with its character.
A mark inside the bounds of a stamp emitted after its own is dropped, keyed or not, because the piece drawn on top owns its cells.
A `Spot` from a keyed piece carries its `prefab`, the key set on the `Prefab` that stamped it, so the game can trace it back to what defined it; `None` for a spot a game made itself or a mark of an unkeyed piece.
`Transition` is a component on an entity standing on a cell, holding the `Destination` it leads to: a place with an `Arrive` of its entry, its exit or a named cell.
`GoThrough` is the action that takes an actor through the one it is standing on, and `WarpRequest` does the same from anywhere for a portal or a first arrival, resolved without charging a turn so the game charges what it likes.
The player always may go through; anyone else is refused unless its `Intelligence` has `Wits::TRAVELS`, which no preset has, while a `WarpRequest` moves anyone, since that is the game doing the moving.
A mind that travels, stood beside an actor as it went through, and was after it or beside it on its own last turn goes through behind it in the same pass; a warp is never followed.
An arrival on a taken cell lands on the nearest free one that can be walked to from it, so a follower stands beside whom it followed.
A warp that lands on a map nobody has built calls the rules and installs the result with `install_place`, whoever arrived; `PlaceEntered` is written for the player alone, with `first` true on the player's first arrival, which is the one arrival on which a game populates a floor.
`MapChanged` is written when the player changes map and `Travelled` when anyone does, naming the cell it left and whom it went after.
The player's warp also switches the per-map indexes with the map: `Occupancy` swaps its `SpatialGrid` for the arriving map's and stashes the one it had, `Knowledge` does the same with explored tiles and the still things last seen on them, and `FlowFields` is invalidated so the next mind rebuilds.
`export` and `import` are the save shape: every built place whole, and which one is current.

## Using it

A game reaches a place by spawning a `Transition` on a cell, and where those cells are is the game's to decide; a game whose places are floors one under another describes them as a `Ladder` from `PlaceRules::ladder` instead, and the engine lays a stair up on each floor's entry and down on its exit when the floor is built, as it does Foundry's lifts.

<!-- include: ../../../../examples/foundry/src/decks.rs:ladder -->
```rust,no_run
    /// The decks, one under another, with a lift between each two: up at
    /// a deck's entry, down at the farthest exit the deck was built with,
    /// so the way on is as far from where the commando arrived as the
    /// deck allows. Deck one's way up is the lift out, which the game
    /// lays, and the core has nothing below it.
    fn ladder(&self) -> Option<Ladder> {
        let (up, down) = (StairLook::new('<', crate::lifts::LIFT, "lift up"), StairLook::new('>', crate::lifts::LIFT, "lift down"));
        Some(Ladder::new((1..=DECKS).map(map_of), up, down).top(LadderEnd::Game))
    }
```

## The line

The engine decides when a place is built, which is the first time anything enters it, and that it is never built twice.
It decides that a place is kept whole once built, so leaving one freezes what is in it and returning finds it as it was.
It decides that the map being read follows the player and nobody else, that an actor on another map is frozen and requeued a full step without acting, and that such an actor can neither act nor be seen.
It decides who follows whom through a way and where each lands; the game decides which kinds travel at all, by writing `travels` in their wits.
The game decides what a map id means, what is built there, where the transitions stand and what they lead to, what an arrival costs, and everything that goes in a place on the arrival `PlaceEntered` marks as its first.

## Where it lives

`atglyph-bevy` is tier 2 and owns places, because they are about what the running game is reading right now, which is not a question a lower tier can answer.
What a place is built from is below the line: a `PlaceBuild` is a `Terrain` and points, made by an `atglyph-mapgen` chain a test runs with no `App`, so the generator is tested without a frame and only which map is current, and which indexes have to be swapped when that changes, is left above it.
