<!-- documents:
     plugins: none
     files: crates/rl-mapgen/src/chain.rs
            crates/rl-mapgen/src/context.rs
            crates/rl-mapgen/src/passes.rs
            crates/rl-mapgen/src/dungeon.rs
            crates/rl-mapgen/src/prefab.rs
            crates/rl-core/src/seed.rs
            crates/rl-world/src/chunk.rs
            crates/rl-bevy/src/places.rs
     fingerprint: 17a75ca9 -->

# Map generation

Every generated thing in the engine, a dungeon floor or a chunk of open world, is a chain of named passes run over a context.
A pass reads and writes the context's terrain, draws from a stream keyed by its own name, and either does its work or says why it could not.
The chain checks at assembly that its passes go forward through the phases and that no two share a name, and panics on either, because both are mistakes in how the chain was written rather than conditions to survive.
What a pass has to tell the rest of generation, a room it carved, a start it chose, a vault it stamped, it publishes as a typed output the later passes and the caller read back.

## Turning it on

There is no plugin and no system: `rl-mapgen` is tier 1, has no Bevy in it, and nothing in it runs unless a game calls `Chain::run`.
A game builds a `BaseContext` over a blank terrain, assembles a `Chain` with `then`, and runs it with a `RunSeed`.
Where that happens is the game's business, and in practice it is inside the `PlaceRules::build` the engine calls the first time a place is entered, or inside the `ChunkRules::chain` the streamer calls for a region.
`Chain::run` stops at the first failure and hands back the `BuildError` naming the pass, so a caller that asked for six rooms on a map with space for four retries rather than shipping a map its game does not expect.
A retry builds a fresh `BaseContext` and runs the chain again with a different seed or a different index: a pass that fails may already have written part of its work, as `Rooms` does, so the context it failed in is not a clean slate to try again in.

## The model

`Pass<C>` is three methods: `name`, which must be stable because it keys the pass's stream, `phase`, which says when it runs, and `apply`, which does the work or returns a `BuildError`.
`Phase` is six variants in pipeline order: `Ground`, `Growth`, `Structures`, `Connect`, `Exits`, `Finish`.
`Chain::then` asserts the new pass's phase is not before the last one's and that its name is fresh, so a chain that would have run its doors before its rooms fails where it was written instead of in a screenshot.
`Chain::run` installs `seed.rng(SeedDomain::new(name), 0)` on the context before each `apply`, then takes a snapshot, and `names` and `len` let a caller report what a chain is.
`BuildContext` is the boundary an engine pass sees: `terrain` and `terrain_mut`, `tiles`, `rng` and `set_rng`, `emit` and `outputs`, and a `take_snapshot` that defaults to doing nothing.
Every engine pass is generic over `C: BuildContext`, which is the whole extension mechanism: a game wraps `BaseContext` in a struct carrying its own fields, implements the trait by delegation, and the engine's passes still run over it unchanged.
`rl-world`'s `ChunkContext` is the engine's own instance of that, a `BaseContext` with a region's neighbourhood, per-tile heights and per-tile facts alongside it.
`BaseContext` is a terrain, a `TileRegistry`, a stream, `Outputs` and optional snapshots; `with_snapshots` records the terrain after every pass, for a tool that wants to watch a map being built, and `finish` takes the terrain and the outputs apart.
`Outputs` is typed rather than keyed: `emit` pushes any `Any + Send` value, and `iter::<T>`, `first::<T>` and `take::<T>` read back only the ones of that type, so two passes publishing different things never collide.
`passes` holds what needs nothing but a terrain and a registry: `Fill` and `Border` in `Ground`, `Scatter` and `ScatterBy` in `Growth`, `CellularCave` in `Ground`, `KeepLargestRegion` in `Connect`, and `CentralStart` in `Exits`.
`Scatter` and `ScatterBy` carry their own `name` field because two scatters in one chain must draw different streams, and `ScatterBy` takes a `ChanceFn<C>` so a chance can follow moisture or slope rather than being one number.
`dungeon` holds the bounded-map passes: `Rooms` and `Bsp` in `Structures`, `Doors` behind them, and `RandomStart` and `FarthestExit` in `Exits`.
`Rooms` scatters non-overlapping rectangles and joins each to the one before it with an L-shaped corridor, failing below `min_rooms`; `Bsp` splits the map into a tree of leaves, carves a room per leaf and joins siblings, so the layout fills the map evenly.
Both publish a `Room` per room in placement order, which is what lets `Doors` find the ring around each one and a prefab ask for a room to sit in.
`RandomStart` and `CentralStart` publish a `StartPoint`, and `FarthestExit` walks from it and publishes the cell furthest away as an `ExitPoint`.
`RandomStart.clear_of_stamps(n)` is the same start kept at least `n` cells, Chebyshev, from every `Stamped` a pass emitted before it, drawn from the first room with a cell that clear, so a piece holding a guard never stands it beside the arrival.
Where no cell anywhere is that clear it is the plain pick, from the same stream, rather than a failed chain, and a chain that never asks draws the start it always has.
`prefab` is the hand-drawn half: a `Prefab` is rows of characters and a legend, a character the legend does not know is transparent so a piece can be an irregular shape, and `rotated` and `flipped` carry a piece's marks around with its tiles.
A legend maps a character to a `Cell`: `Tile` paints, `Mark` marks the position and may paint the tile under it, and `Clear` leaves the map as it was.
`StampPrefab` places one at a `Placement` with an `Orient` saying how it may be turned first, authored per stamp because the same vault may turn freely in a cave and be fixed against the corridor its door has to meet.
`StampOneOf` is the same pass with a weighted choice in front of it; a zero weight is a piece in the list that is never drawn, which is how a piece stays in while it is being worked on, and nothing carrying weight fails the chain.
Either stamp publishes a `Stamped` with its bounds and its marks in map coordinates, so a later pass can keep out of it or spawn into it.
A piece `keyed` with an opaque number carries that key through every turn and mirror to its stamp's `Stamped::prefab`, so whoever fills the marks can trace them back to the piece that gave them meaning.

## Using it

A game's generation is a chain assembled for the map being built, run with a seed of that map's own.

<!-- include: ../../../../examples/tutorial/src/bin/step06_descent.rs:rules -->
```rust,no_run
/// How a floor is built. The engine calls this once, the first time
/// something enters the map, and keeps what comes back.
impl PlaceRules for Warren {
    fn build(&self, map: MapId, _: Option<&WorldGraph>) -> Result<PlaceBuild, BuildError> {
        let depth = floor_of(map);
        let (wall, open, roots) = (self.tiles.expect("earth"), self.tiles.expect("dirt"), self.tiles.expect("roots"));
        let mut ctx = BaseContext::blank(84, 42, self.tiles.clone(), wall);
        // A stream per floor, so the second floor is the same whether or
        // not you dawdled on the first.
        let seed = RunSeed(self.seed.0 ^ (depth as u64) << 32);
        let chain = match depth {
            // Dug rooms near the surface.
            1 => Chain::new().then(dungeon::Rooms { floor: open, attempts: 40, min_size: 5, max_size: 10, min_rooms: 6 }).then(dungeon::Doors { door: roots }),
            // Gnawed-out caves under them.
            _ => Chain::new().then(passes::CellularCave { wall, floor: open, fill_pct: 45, ..Default::default() }).then(passes::KeepLargestRegion { wall }),
        };
        // Every floor gets a start and a point as far from it as the floor
        // allows: the stairs down on the first, the way out on the second.
        chain.then(dungeon::RandomStart).then(dungeon::FarthestExit).run(&mut ctx, seed)?;
        PlaceBuild::from_context(ctx)
    }
}
```

## The line

Nothing in generation is seeded from a constant or from entropy, once a chain is running: a fresh `BaseContext` carries a placeholder generator seeded from zero, and `Chain::run` replaces it with the pass's own before any `apply`, so only a pass applied by hand outside a chain would draw from it.
A pass's stream is `RunSeed::derive` of the run's seed against a `SeedDomain` made from the pass's name, so what a pass draws depends on the seed and its name and on nothing else.
That is what inserting, removing and reordering buy: adding a decoration pass in the tenth week cannot shift a room the room pass placed in the first, so a chain is edited without every earlier seed becoming a different map.
It is also what renaming costs, since a renamed pass is a new stream and rerolls, and the assertion against two passes sharing a name exists because two that did would draw an identical sequence and lay the same pattern twice.
The seed a chain is run with is the caller's to choose, and the choice matters as much as the passes: a map keyed by its own id is the same map whether or not the player dawdled on the one above, and a map keyed by a counter is not.
A pass that rolls takes the stream from the context rather than making one, and a helper that rolls takes `&mut impl Rng`, so a test hands it a fixed generator and reads the arithmetic.
The engine decides the pipeline: what a phase is and what order the six come in, that a name keys a stream, that a failure stops the chain and names itself, and how each shipped pass does its work.
The game decides which passes are in the chain, which tile ids each one writes, what the numbers are, what the context carries beyond a terrain, and what the outputs mean.
No shipped pass knows what a wall or a cave or a road is; every tile it writes arrives as a parameter, which is why the same `CellularCave` carves a warren, a mine and a nest of tunnels in three different games.

## Where it lives

`rl-mapgen` is tier 1 with no Bevy in it and nothing Bevy-shaped either: a chain is a value, a context is a struct, and running one is a function call.
That is why a whole floor can be generated in a test in microseconds, with no `App` and no frame, and why the properties that matter are checked as properties: that a chain with a pass added draws the same values for the passes around it, that a different seed or a different name rolls differently, and that a failing pass stops the chain, names itself, and leaves every pass behind it unrun.
The crate builds for `wasm32-unknown-unknown` like the rest of its tier, so nothing in generation may time itself.
The split from `rl-grid` is the one to keep in mind: `rl-grid` owns the terrain, the tiles and the flood fill, and `rl-mapgen` owns only the order things happen in and the streams they happen from.
