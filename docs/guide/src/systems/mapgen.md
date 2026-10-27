<!-- documents:
     plugins: none
     files: crates/atglyph-mapgen/src/chain.rs
            crates/atglyph-mapgen/src/context.rs
            crates/atglyph-mapgen/src/passes.rs
            crates/atglyph-mapgen/src/dungeon.rs
            crates/atglyph-mapgen/src/prefab.rs
            crates/atglyph-mapgen/src/pool.rs
            crates/atglyph-mapgen/src/dress.rs
            crates/atglyph-mapgen/src/lakes.rs
            crates/atglyph-mapgen/src/secrets.rs
            crates/atglyph-core/src/seed.rs
            crates/atglyph-mapgen/src/place.rs
     fingerprint: a675f310 -->

# Map generation

Every generated thing in the engine, a dungeon floor or a cave, is a chain of named passes run over a context.
A pass reads and writes the context's terrain, draws from a stream keyed by its own name, and either does its work or says why it could not.
The chain checks at assembly that its passes go forward through the phases and that no two share a name, and panics on either, because both are mistakes in how the chain was written rather than conditions to survive.
What a pass has to tell the rest of generation, a room it carved, a start it chose, a vault it stamped, it publishes as a typed output the later passes and the caller read back.

## Turning it on

There is no plugin and no system: `atglyph-mapgen` is tier 1, has no Bevy in it, and nothing in it runs unless a game calls `Chain::run`.
A game builds a `BaseContext` over a blank terrain, assembles a `Chain` with `then`, and runs it with a `PlaceSeed` or a `RunSeed`.
Where that happens is the game's business, and in practice it is inside the `PlaceRules::build` the engine calls the first time a place is entered, handing it that place's own `PlaceSeed`.
`Chain::run` stops at the first failure and hands back the `BuildError` naming the pass, so a caller that asked for six rooms on a map with space for four retries rather than shipping a map its game does not expect.
`Chain::run_retrying` is that retry: it takes a function that makes a fresh `BaseContext`, since a pass that fails may already have written part of its work, as `Rooms` does, and runs the chain again from the next attempt until one succeeds or the attempts are spent.

## The model

`Pass` is three methods: `name`, which must be stable because it keys the pass's stream, `phase`, which says when it runs, and `apply`, which does the work or returns a `BuildError`.
`Phase` is six variants in pipeline order: `Ground`, `Growth`, `Structures`, `Connect`, `Exits`, `Finish`.
`Chain::then` asserts the new pass's phase is not before the last one's and that its name is fresh, so a chain that would have run its doors before its rooms fails where it was written instead of in a screenshot.
`Chain::run` installs `seed.rng(SeedDomain::new(name), 0)` on the context before each `apply`, then takes a snapshot, and `names` and `len` let a caller report what a chain is; an attempt of `run_retrying` is the same with the attempt's number in place of the nought.
`BaseContext` is the one context every pass works on: `terrain` and `terrain_mut`, `tiles`, `rng` and `set_rng`, `emit`, `outputs` and `outputs_mut`, so a game's own pass is written against the same struct as the engine's, and what it needs beyond a terrain arrives as an output an earlier pass emitted.
`BaseContext` is a terrain, a `TileRegistry`, a stream once one is installed, `Outputs` and optional snapshots; `with_snapshots` records the terrain after every pass, for a tool that wants to watch a map being built, and `finish` takes the terrain and the outputs apart.
`Outputs` is typed rather than keyed: `emit` pushes any `Any + Send` value, and `iter::<T>` and `first::<T>` read back only the ones of that type, so two passes publishing different things never collide.
`passes` holds what needs nothing but a terrain and a registry: `Fill` and `Border` in `Ground`, `Scatter` and `ScatterBy` in `Growth`, `CellularCave` in `Ground`, `KeepLargestRegion` in `Connect`, and `CentralStart` in `Exits`.
`Scatter` and `ScatterBy` carry their own `name` field because two scatters in one chain must draw different streams, and `ScatterBy` takes a `ChanceFn`, a chance read off the `&BaseContext` and the cell, so it can follow anything the context carries rather than being one number.
A place grown piece by piece, `Accrete` and the passes that survey it and hold it to a recipe, is [the layout page](layout.md)'s.
`dungeon` holds the bounded-map passes: `Rooms` and `Bsp` in `Structures`, `Doors` behind them, and `RandomStart` and `FarthestExit` in `Exits`.
`Rooms` scatters non-overlapping rectangles and joins each to the one before it with an L-shaped corridor, failing below `min_rooms`; `Bsp` splits the map into a tree of leaves, carves a room per leaf and joins siblings, so the layout fills the map evenly.
Both publish a `Room` per room in placement order, which is what lets `Doors` find the ring around each one and a prefab ask for a room to sit in.
`RandomStart` and `CentralStart` publish a `StartPoint`, and `FarthestExit` walks from it and publishes the cell furthest away as an `ExitPoint`.
It walks through a shut door as a walker who opens doors does, since a passable door joins the rooms either side of it, and it never picks the door itself, only a cell to stand on, breaking a tie toward the lower `Point` so a seed always picks the same one.
`RandomStart.clear_of_stamps(n)` is the same start kept at least `n` cells, Chebyshev, from every `Stamped` a pass emitted before it, drawn from the first room with a cell that clear, so a piece holding a guard never stands it beside the arrival.
Where no cell anywhere is that clear it is the plain pick, from the same stream, rather than a failed chain, and a chain that never asks draws the start it always has.
`FarthestExit.clear_of_stamps(n)` and `among_farthest(pct)` build a `FarExit` kept off the pieces and drawn among the farthest cells, `MarkedStart` and `MarkedExit` stand either on a cell a stamp marked, and `Stairs`, `Free`, `Beyond` or `Within(glyph)`, keeps the two either side of a grown place's `Landmark` or puts the way down inside it.
`prefab` is the hand-drawn half: a `Prefab` is rows of characters and a legend, a character the legend does not know is transparent so a piece can be an irregular shape, and `rotated` and `flipped` carry a piece's marks around with its tiles.
A legend maps a character to a `PrefabCell`: `Tile` paints, `Mark` marks the position and may paint the tile under it, `Clear` leaves the map as it was, and `Entry` is a cell of the outer wall where a piece grown as an area may be joined.
A prefab declares how far its drawing may be turned and mirrored, a `Rotation` and a `Mirror`, and a stamp's `Orient` is a ceiling over that, so no pass lays a piece a way its file refuses.
`StampPrefab` places one at a `Placement`, the center, a point, a room, an edge, anywhere or a span across the middle, with an `Orient` saying how it may be turned first, authored per stamp because the same vault may turn freely in a cave and be fixed against the corridor its door has to meet.
`StampOneOf` is the same pass with a weighted choice in front of it; a zero weight is a piece in the list that is never drawn, which is how a piece stays in while it is being worked on, and nothing carrying weight fails the chain.
`StampEachRoom` is the pass that keeps a map from having empty rooms: in `Finish`, every emitted `Room` no `Stamped` touches gets one piece drawn by weight among those that fit it in a facing `Orient::facings` allows, with a cell of the room all round it.
It is laid centred, and slid to the nearest place where it paints and marks neither the `StartPoint` nor the `ExitPoint`, which is why it runs after both exist; a room nothing fits is left as it was, and only a list with no weight at all fails the chain.
Every stamp publishes a `Stamped` with its bounds and its marks in map coordinates, so a later pass can keep out of it or spawn into it.
A piece `keyed` with an opaque number carries that key through every turn and mirror to its stamp's `Stamped::prefab`, so whoever fills the marks can trace them back to the piece that gave them meaning.
`StampPicks` lays a place's `PoolPick`s in `Finish`, each a stamp or a dressing in a room no stamp took, by a `Prefer` of `Secluded`, `DeadEnd`, `Junction`, `Hidden` or `Anywhere`, failing the place for a required pick that fits nowhere; `Dress` then lays each `Dressing`, things spread over an area's walls, corners, center, rows or anywhere, in the areas named and the rooms left over.
`Lakes` grows water over the floor in `Structures`, `Bridges` joins what it parted or would make a long detour of in `Connect`, and `HideOpenings` walls over dead-end doors and loop openings in `Finish`, publishing each as a `SecretCell` only where the route and every cell stay reachable once it is found.

## Using it

A game's generation is a chain assembled for the map being built, run with a seed of that map's own.

<!-- include: ../../../../examples/tutorial/src/bin/step06_descent.rs:rules -->
```rust,no_run
/// How a floor is built. The engine calls this the first time something
/// enters the floor, with that floor's own seed, and keeps the result.
impl PlaceRules for Floors {
    fn build(&self, map: MapId, seed: PlaceSeed) -> Result<PlaceBuild, BuildError> {
        let (wall, floor) = (self.tiles.expect("earth"), self.tiles.expect("dirt"));
        let mut ctx = BaseContext::blank(84, 42, self.tiles.clone(), wall);
        let chain = match floor_of(map) {
            // Dug rooms near the surface.
            1 | 2 => Chain::new()
                .then(dungeon::Rooms { floor, attempts: 40, min_size: 5, max_size: 10, min_rooms: 6 })
                .then(dungeon::Doors { door: self.tiles.expect("roots") }),
            // Gnawed-out caves below them.
            3 => Chain::new().then(passes::CellularCave { wall, floor, fill_pct: 45, ..Default::default() }).then(passes::KeepLargestRegion { wall }),
            // One wide chamber at the bottom.
            _ => Chain::new().then(dungeon::Rooms { floor, attempts: 60, min_size: 12, max_size: 18, min_rooms: 2 }),
        };
        // A start, and the point farthest from it: where the stairs down go.
        chain.then(dungeon::RandomStart).then(dungeon::FarthestExit).run(&mut ctx, seed)?;
        PlaceBuild::from_context(ctx)
    }

    fn tiles(&self) -> TileTables {
        self.tiles.tables()
    }

    /// The floors, top first, and how their stairs look. The engine lays a
    /// stair up at each floor's start and a stair down at its far point.
    /// The last floor's far point is the game's: the way out.
    fn ladder(&self) -> Option<Ladder> {
        let stone = Rgb::new(204, 204, 219);
        let (up, down) = (StairLook::new('<', stone, "stairs up"), StairLook::new('>', stone, "stairs down"));
        Some(Ladder::new((1..=FLOORS).map(map_of), up, down).bottom(LadderEnd::Game))
    }
}
```

## The line

Nothing in generation is seeded from a constant or from entropy: a fresh `BaseContext` holds no stream, `Chain::run` installs each pass's own before its `apply`, and a pass applied by hand outside a chain that draws before a stream is set with `set_rng` panics naming the fix rather than drawing the same numbers every run.
A pass's stream is `RunSeed::derive` of the run's seed against a `SeedDomain` made from the pass's name, so what a pass draws depends on the seed and its name and on nothing else.
That is what inserting, removing and reordering buy: adding a decoration pass in the tenth week cannot shift a room the room pass placed in the first, so a chain is edited without every earlier seed becoming a different map.
It is also what renaming costs, since a renamed pass is a new stream and rerolls, and the assertion against two passes sharing a name exists because two that did would draw an identical sequence and lay the same pattern twice.
The seed a chain is run with is the caller's to choose, and the choice matters as much as the passes: a map keyed by its own id is the same map whether or not the player dawdled on the one above, and a map keyed by a counter is not.
A pass that rolls takes the stream from the context rather than making one, and a helper that rolls takes `&mut impl Rng`, so a test hands it a fixed generator and reads the arithmetic.
The engine decides the pipeline: what a phase is and what order the six come in, that a name keys a stream, that a failure stops the chain and names itself, and how each shipped pass does its work.
The game decides which passes are in the chain, which tile ids each one writes, what the numbers are, what the context carries beyond a terrain, and what the outputs mean.
No shipped pass knows what a wall or a cave or a road is; every tile it writes arrives as a parameter, which is why the same `CellularCave` carves a warren, a mine and a nest of tunnels in three different games.
The passes that place a piece by room, `Placement::InRoom`, `Placement::AnyRoom`, `StampEachRoom` and `StampPicks`, read the `Room`s that `Rooms`, `Bsp` and `Accrete` emit and nothing else, so on a cave, or any map whose builder emits no `Room`, they have nowhere to put a piece; a cave that wants furnishing places its pieces `At` points of its own choosing for now.

## Where it lives

`atglyph-mapgen` is tier 1 with no Bevy in it and nothing Bevy-shaped either: a chain is a value, a context is a struct, and running one is a function call.
That is why a whole floor can be generated in a test in microseconds, with no `App` and no frame, and why the properties that matter are checked as properties: that a chain with a pass added draws the same values for the passes around it, that a different seed or a different name rolls differently, and that a failing pass stops the chain, names itself, and leaves every pass behind it unrun.
The crate builds for `wasm32-unknown-unknown` like the rest of its tier, so nothing in generation may time itself.
The split from `atglyph-grid` is the one to keep in mind: `atglyph-grid` owns the terrain, the tiles and the flood fill, and `atglyph-mapgen` owns only the order things happen in and the streams they happen from.
