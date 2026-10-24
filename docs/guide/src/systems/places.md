<!-- documents:
     plugins: none
     files: crates/atglyph-bevy/src/places.rs
            crates/atglyph-mapgen/src/place.rs
            crates/atglyph-bevy/src/app.rs
            crates/atglyph-bevy/src/title.rs
            crates/atglyph-rules/src/travel.rs
            crates/atglyph-grid/src/landing.rs
            crates/atglyph-bevy/src/world.rs
            crates/atglyph-bevy/src/turn.rs
            crates/atglyph-bevy/src/plugin.rs
            crates/atglyph-bevy/src/knowledge.rs
            crates/atglyph-bevy/src/minds.rs
     fingerprint: 1defee28 -->

# Places

A game's maps are places, and everything that reads tiles reads them through one resource.
A place is bounded, built once by the game's rules the first time something enters it, and kept whole from then on, actors and items included.
Every entity with a position is on exactly one map, and the turn loop freezes whatever is not on the one being played.

## Turning it on

Places need no plugin: `CorePlugin` puts `resolve_warps` in `ResolveSet::Travel` and `tag_new_positions` in `TurnSet::Schedule` before anyone is dealt a turn, and declares `needs::<WorldMap>`, so a game that builds no map is told so by name when play begins.
What a game adds is its `PlaceRules`, handed over once with `app.places(rules)` where the game is put together, which also says the game needs a `Seed`; the engine calls them the first time a map id is entered.
A run whose start makes no `WorldMap` gets one built over `PlaceRules::tiles`, a start that makes its own keeps it, and `commands.spawn_player(map, bundle)` spawns the player as a blocking actor warped to that place's entry.

## The model

`MapId` is a `u32`, and a game numbers its places however it likes; `MapId::FIRST` is map one, and `MapId::ZERO` is where an entity with no `OnMap` stands.
`OnMap` is the component saying which; it is missing until something first gains a position, and `tag_new_positions` fills it in with the current map.
`WorldMap` is the resource every reader reads: `tile`, `set_tile`, `is_walkable`, `opens`, `closes` and `is_loaded` all answer for the current map, and `None` or impassable for a cell outside it.
`current`, `window_tiles`, `to_world` and `to_local` say which map and what it covers; `view` and `opening_view` hand a `WindowView` to a grid algorithm, the second costing a closed door as the turn spent opening it plus the step onto what it opens into, so a flood routes through a door when that is shorter.
Three counters the engine keeps tell its readers when to recompute without anyone asking them: one moves when the map changes, one when an edit changes what blocks sight, and one when an edit changes where an actor may walk or what a step costs.
A veil is the fourth case, a set of cells that hide what is behind them for a reason other than their tile, as thick gas does, rewritten each turn by whatever makes it and lifted whenever the map under it changes.
`lock` says what is shut on a cell and `unlock` takes a lock or a seal off, `secrets` lists what a place still hides and `find_secret` lays the real tiles of a found secret's whole group, and `is_peaceful` says whether a map's builder made it a place where nobody is harmed.
The place model is `atglyph::mapgen::place`, tier 1: `PlaceRules::build` takes a `MapId` and the place's own `PlaceSeed`, derived from the run's seed and the map's number alone, and returns a `PlaceBuild`: a `Terrain`, an `entry`, an optional `exit`, a `Vec<Spot>` of points of interest tagged in the game's own numbering, the `locks` and `secrets` its chain laid, its `ambient` light and whether it is `peaceful`.
`PlaceRules::tiles` names the tile tables places are built of, and `ladder` is `None` unless the game's places are floors one under another.
`PlaceBuild::from_context` is the usual way to make one from a finished chain, taking the chain's `StartPoint` as the entry, its `ExitPoint` as the exit, and every prefab mark as a `Spot` tagged with its character.
A mark inside the bounds of a stamp emitted after its own is dropped, keyed or not, because the piece drawn on top owns its cells.
A `Spot` from a keyed piece carries its `prefab`, the key set on the `Prefab` that stamped it, so the game can trace it back to what defined it; `None` for a spot a game made itself or a mark of an unkeyed piece.
`Transition` is a component on an entity standing on a cell, holding the `Destination` it leads to: a place with an `Arrive` of its entry, its exit or a named cell.
`GoThrough` is the action that takes an actor through the one it is standing on, refused at no cost off one, and `WarpRequest::new(actor, to)` does the same from anywhere for a portal or a first arrival, resolved without charging a turn so the game charges what it likes.
The player always may go through; anyone else is refused unless its `Intelligence` has `Wits::FLEES_THROUGH`, which no preset has, while a `WarpRequest` moves anyone, since that is the game doing the moving.
A mind that `FOLLOWS_THROUGH`, stood beside an actor as it went through, and was after it or beside it on its own last turn goes through behind it in the same pass, nearest first by `atglyph_rules::travel::followers`; a warp is never followed, though one may name whom it goes `after`, so a party a game warps behind its leader is told as following and is skipped when the leader was refused.
An arrival on a taken cell lands on the nearest free one that can be walked to from it, by `atglyph_grid::landing`, so a follower stands beside whom it followed.
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
It decides who follows whom through a way and where each lands; the game decides which kinds travel at all, by writing `follows_through`, `flees_through` or `travels`, both, in their wits.
The game decides what a map id means, what is built there, where the transitions stand and what they lead to, what an arrival costs, and everything that goes in a place on the arrival `PlaceEntered` marks as its first.

## Where it lives

The place model is tier 1, in `atglyph-mapgen`: `MapId`, `PlaceRules`, `PlaceBuild`, `Spot`, `Lock` and `Ladder` need nothing but a terrain and a chain, so a terminal game or a tool builds the same places as the framework with no Bevy, and the generator is tested without a frame.
`atglyph-bevy` is tier 2 and owns only what the running game is reading right now: which map is current, the warps, and which indexes have to be swapped when that changes, which is not a question a lower tier can answer.
