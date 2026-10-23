<!-- documents:
     plugins: none
     files: crates/atglyph-mapgen/src/accrete.rs
            crates/atglyph-mapgen/src/loops.rs
            crates/atglyph-mapgen/src/survey.rs
            crates/atglyph-mapgen/src/walk.rs
            crates/atglyph-mapgen/src/chain.rs
            crates/atglyph-rules/src/layout.rs
            crates/atglyph-bevy/src/layout.rs
     fingerprint: 8b64d613 -->

# Layout

A layout is a place grown instead of scattered: a first piece laid down, and every later one attached to a wall of what already stands.
The place is walkable from end to end by construction, its rooms share walls, and what is drawn depends on where on the map it is going, so one place has regions that differ.
Once grown it is surveyed, and a place that falls short of what its recipe asked for is thrown away and grown again from the same seed.

## Turning it on

There is no plugin: the passes are `atglyph-mapgen`'s, tier 1 with no Bevy in it, and they run where a game runs any chain.
A game writes a layout file, loads it once with `atglyph::rules::layout::load`, and asks `atglyph::ecs::layout::passes` for one recipe as `LayoutPasses`: the map's size, an `Accrete`, an `OpenLoops` and a `Require`.
It puts `accrete` and `loops` first in its chain over a map filled with solid tile, then its own stamps, its start and its exit, then `Survey`, and `require` last.
It runs that chain with `Chain::run_retrying`, which is what turns a failed requirement into another attempt.
A game with a shape of its own builds its `Accrete` in code; a file names only the shapes the engine ships.

## The model

A `Shape` draws a `Piece` from the pass's stream: the cells of its floor, relative to its own corner, and its sites.
A site is a wall cell against the outside of the body that touches exactly one body cell, with the direction that leads out through it; a cell in a concave corner touches two and is not one.
A shape also has a name, a `Join`, and `is_room`, which says whether a prefab may be stamped in what it draws.
The engine ships `Rectangle`, `Cross`, two bars overlapped on their centers, and `Passage`, a run of floor with a chance of one bend, which is the one that is not a room.
None of the three is in a prelude, since Bevy has a `Rectangle` of its own; a game names them from `atglyph::mapgen::accrete` where it builds a pass.
`Join` is `Door`, one cell of the door tile, `Arch`, one cell of floor, or `Mouth(n)`, floor up to `n` cells wide along the wall the two pieces share, widened to either side of where they met in turn.
A `District` is a name and a weighted list of shapes.
`Accrete` scatters district seeds, as many as `districts_per_10k` gives for the map's area and never fewer than one, each drawn a district from `mix`, and a cell belongs to its nearest seed.
It lays a first piece anywhere it fits, then draws a site from the frontier, draws a shape from the district that site is in, and places the piece so one of its own sites is that same cell.
A piece fits where its body and the ring one cell round it are solid and inside the map's border, which leaves exactly one wall between any two areas.
Growth stops when `open_pct` of the map is open, and fails when `patience` attempts in a row have fitted nothing.
Before it hands the place on it turns every area that is not a room and has one way in back into what it was carved from, and takes growth up again to refill the share, so a recipe that draws mostly passages may never reach it.
It emits an `Area` per piece, with its index, the names of its shape and its district, its cells, their bounds and the largest rectangle inside them, and an `Opening` per join, with its cells, its facing, its two areas and its `Join`.
It also emits a `Room` holding that inner rectangle for every area whose shape is a room, which is what the stamp passes read.
`OpenLoops` takes each pair of areas that touch across a wall and are not joined, draws one wall cell between them, and opens the one whose two sides are farthest apart on foot as a door, again and again, until the longest detour is under `min_detour` or `max` doors are open.
A door only shortens a walk, so after each it measures again only the candidates whose last measure could still be the longest; it never opens beside a way through that is already there, and emits an `Opening` for each.
`Survey` runs in `Exits`, after the start and the exit, and emits one `Surveyed`.
It holds an `AreaFacts` per area: how many openings it has, whether it is a `dead_end` with one, a `junction` with three or more, a `chokepoint` whose removal parts two others, and its `seclusion`, the steps from its nearest opening to the shortest route from the start to the exit, `u32::MAX` for an area a later pass walled off from it.
For the place it holds `loops`, openings less areas plus one, `open_pct`, and `route`, the length of that shortest route, `None` where there is no exit.
`Require` runs in `Finish` and fails with the first bound the survey misses: the open share, the fewest areas drawn by a named shape, the fewest loops, the largest share of dead ends, the shortest route.
`Chain::run_retrying` takes a function that makes a fresh context, a seed and a number of attempts, and derives every pass's stream at the attempt's index, so `Chain::run` is attempt nought.
`LayoutDef` is the file: `ShapeDef`s, each a name, a `ShapeKind` with its ranges and a `JoinDef`, `DistrictDef`s, and `RecipeDef`s, with every name in it resolved to an index.
`load` reports every mistake at once, by the name of the shape, district or recipe it is about.

## Using it

A game's place builder is a chain with the recipe's passes at either end of it, run until an attempt meets the recipe.

<!-- include: ../../../../examples/foundry/src/decks.rs:grow -->
```rust,no_run
    /// Runs the deck's chain and hands back the context still open, so a
    /// test can read a [`Stamped`](atglyph::mapgen::prefab::Stamped)
    /// pass emitted before [`PlaceBuild::from_context`] flattens every
    /// mark into a [`Spot`] and drops the stamp's own bounds.
    fn generate(&self, map: MapId) -> Result<BaseContext, BuildError> {
        let deck = deck_of(map);
        // One stream per deck, so building deck 2 first does not change deck 1.
        let seed = RunSeed(self.seed.0 ^ (deck as u64) << 32);
        // Foundry grows no locked or sealed piece, so its hatch stands for both.
        let tiles = LayoutTiles { floor: self.deck, door: self.hatch, locked: self.hatch, sealed: self.hatch };
        let grown = atglyph::ecs::layout::passes(&self.layout, recipe_of(deck), &tiles)?;
        let (width, height) = grown.size;
        let mut chain = Chain::new()
            .then(grown.accrete)
            .then(grown.loops)
            .then(StampOneOf { name: "armory", choices: self.armories()?, at: Placement::AnyRoom, orient: Orient::TurnedOrMirrored })
            .then(StampOneOf { name: "store", choices: self.stores()?, at: Placement::AnyRoom, orient: Orient::TurnedOrMirrored });
        // Decks two, four, five, seven and eight hold a guard post: a locker
        // and a weapon held by what the deck has. Not deck one, which
        // teaches the lamp and the droids, and not the charge decks, which
        // already fit a third piece in their rooms.
        if matches!(deck, 2 | 4 | 5 | 7 | 8) {
            chain = chain.then(StampPrefab {
                name: "guard post",
                prefab: self.prefabs.piece("guard post")?,
                at: Placement::AnyRoom,
                orient: Orient::TurnedOrMirrored,
            });
        }
        // Decks three, six and nine each feed a section of the plant, and
        // deck ten is the core: one reactor chamber on each, so the four
        // charges have somewhere to be set. The core is its own shape and
        // marks `R` like the others, so one system plants all four
        // consoles. Both are stamped fixed, since nothing about either
        // chamber reads better turned.
        chain = match deck {
            3 | 6 | 9 => chain.then(StampPrefab { name: "reactor", prefab: self.prefabs.piece("reactor")?, at: Placement::AnyRoom, orient: Orient::Fixed }),
            DECKS => chain.then(StampPrefab { name: "core", prefab: self.prefabs.piece("core")?, at: Placement::AnyRoom, orient: Orient::Fixed }),
            _ => chain,
        };
        // The start keeps the population's own distance from every piece,
        // so no guard at a post stands beside the arrival.
        let start = RandomStart.clear_of_stamps(crate::droids::MIN_DISTANCE_FROM_ENTRY);
        // Last, once the ways in and out are known so no room piece lands
        // on either: every room nothing was stamped in is furnished.
        let rooms = StampEachRoom { name: "furnish", choices: self.rooms()?, orient: Orient::TurnedOrMirrored };
        // The survey once the ways in and out are known, and what the
        // recipe requires last: a deck that falls short is built again.
        // The phases put the survey before the furnishing, so the open
        // share it is held to is the deck's before its machinery; a room
        // piece keeps floor all round it, so the route is the same after.
        let make = || BaseContext::blank(width, height, self.tiles.clone(), self.hull);
        chain.then(start).then(FarthestExit).then(Survey).then(rooms).then(grown.require).run_retrying(make, seed, ATTEMPTS)
    }
```

## The line

The engine decides how a place grows: that a piece is attached and never dropped, that one wall stands between two areas, that a passage leading nowhere is taken back, which detour is cut first, and what is measured.
It decides that one seed builds one place however many attempts that took, because an attempt is only an index into each pass's stream.
The game decides everything a player would call the place: which shapes exist and how large, which districts favor which, how full the map is grown, how many loops, and the bounds a place must meet.
How full a place is grown and how many districts it has are a share and a density and never a count of cells of map, so a recipe grows a small map and a large one, and a place's size is one field.
The loops opened and the bounds required are plain counts, so a recipe moved to a much larger map wants those raised with it.
A shape's name means nothing to the engine.
There is no enumeration of what an area is: an area carries the name of the shape that drew it, and a requirement asks for areas by that name, so a hall is a rectangle a game's file called one.
A district builds nothing and no wall follows its edge; it weights a draw, and a recipe with one district is the generator with none.
The survey decides nothing either.
It is in the engine because every game that puts a thing somewhere for where that is would otherwise measure the same facts again, differently, and it is what a furnisher or a director reads to put a reward off the direct route or a guard at a chokepoint.
What goes in a room is not here: the stamp passes fill the `Room`s a grown place emits exactly as they fill the ones `Rooms` and `Bsp` do, and nothing is ever stamped in a passage.
A requirement a recipe can never meet is not found at load, since only growing a place can say; it is found by a test that builds every place over a span of seeds within the attempts a game allows.

## Where it lives

The passes are in `atglyph-mapgen`, so a place is grown, surveyed and held to its bounds in a test with no `App`, over a span of seeds and at more than one size, in seconds.
The file is in `atglyph-rules`, which names no mapgen type, so a layout loads and every mistake in it is reported with no map at all.
`atglyph-bevy` holds the one function that sees both, as it does for a prefab, and it adds no system.
