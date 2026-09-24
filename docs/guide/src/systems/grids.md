<!-- documents:
     plugins: none
     files: crates/rl-core/src/point.rs
            crates/rl-core/src/grid.rs
            crates/rl-core/src/direction.rs
            crates/rl-core/src/geometry.rs
            crates/rl-grid/src/tile.rs
            crates/rl-grid/src/terrain.rs
            crates/rl-grid/src/bitgrid.rs
            crates/rl-grid/src/spatial.rs
            crates/rl-grid/src/astar.rs
            crates/rl-grid/src/dijkstra.rs
            crates/rl-grid/src/region.rs
            crates/rl-grid/src/targeting.rs
     fingerprint: 82b0b78f -->

# Grids and tiles

Everything in the engine that has a place is at a `Point`, on a grid addressed row-major, with `y` growing downward.
A tile is two bytes of identity, and everything the engine knows about one is a record in a registry the game filled before play.
An algorithm never asks what a tile is; it asks a grid one of two questions, does this cell block sight and what does it cost to enter, and both answers come from that registry through parallel tables of flags.
This is the vocabulary every other system's page is written in.

## Turning it on

There is no plugin here and no system, because there is nothing to run: `rl-core` and `rl-grid` are tier 1 and below, with no Bevy in them at all.
A game turns them on by building a `TileRegistry`, calling `tables()` on it once, and handing the `TileTables` to whatever holds the map.
`TileRegistry::standard` is the conventional starting set, `void`, `wall`, `floor`, `door_closed` and `door_open`, with `void` at id 0 so an unwritten cell is solid rather than an open field; `TileRegistry::new` is an empty one for a game that wants none of those names.
A registration is refused with `RegisterError::Duplicate` if the name is taken, and `tables()` panics naming the tile if one opens, closes or burns into a name nobody registered.
Names are resolved there rather than at registration, which is what lets a door be registered before the tile it opens into.

## The model

`Point` is two `i32`s ordered on `y` and then `x`, so a sorted list of points reads top to bottom and left to right, and several generation passes lean on that to break ties the same way every run.
`Rect` is half-open on the right and bottom, and carries the predicates a layout pass wants: `contains`, `is_border`, `intersects`, `too_close` with a margin, `inflate`, `intersection`, `union`, and `cells`, `interior` and `border` as row-major iterators.
`Direction` is the eight compass points declared clockwise from north, with `index` pinned to that order so anything stored per direction is laid out the same way, and `DirectionSet` packs a subset into one byte: what a cell records is not whether a road is here but which ways it leaves by.
`Grid2D` is the addressing contract every map-shaped type implements: `width` and `height` are all an implementor writes, and `bounds`, `xy_idx`, `idx_point`, `in_bounds`, `checked_idx` and `neighbours` come with it, so an algorithm is written once against the trait and runs on a terrain, a scratch grid or a streamed window alike.
`Steps` picks four neighbours or eight, and `Grid<T>` is the dense row-major container every layer uses; a layer per grid rather than one grid of fat structs, so a sweep over one layer stays in cache.
`geometry` is the pure shape arithmetic over plain points: `chebyshev`, `euclidean_sq`, `octile_milli` as an integer heuristic scaled by a thousand, `line` by Bresenham, and `square`, `disc` and `cone` as iterators so a caller that only tests membership allocates nothing.
`TileId` is a `u16` index; `TileProps` is the record behind it, carrying `walkable`, `passable`, `opaque`, `blocks_projectiles`, `move_cost`, `opens_to`, `closes_to` and an optional `Burn`.
`walkable` against `passable` is the load-bearing pair: a closed door is not walkable this turn but a corridor through it still connects two rooms, and a connectivity pass that confused the two would wall off half a map.
`TileTables` is what the hot loops actually read, a `Vec` per flag indexed by id, with `opens` and `closes` resolved to ids and `burn` resolved to a `Kindling`.
`Terrain` is a `Grid<TileId>` and nothing else, and `Terrain::view(&registry)` makes a `TerrainView` that answers the two questions from the tables.
Those two questions are the traits `OpacitySource` and `CostSource`, both supertraits of `Grid2D`, and both answer for an out-of-bounds cell without being asked: off the grid is opaque and impassable, so a scan stops at the edge rather than checking twice.
A newtype over a view that overrides one method is how smoke, a hazard or a swimmer's costs go on top without copying the terrain, and the doc-test on `TerrainView` is that pattern written out.
`BitGrid` is one bit per cell for viewsheds, visited sets and explored maps; `SpatialGrid<E>` is a `BTreeMap<Point, Vec<E>>` for who is standing where, kept out of the terrain so moving every turn does not mark the terrain changed.
`AStar` is one mover to one goal, holding its scratch buffers across searches, and returns a `Path` of steps and a total cost; `DijkstraMap` is one flood any number of movers descend, bounded to a `Rect` so a continuous world never floods a million cells a turn, with `UNREACHED` for what it did not reach and signed values so `scale` and `rescan` invert it into the safety map a fleeing monster follows.
`PathRules` says whether diagonals are allowed and whether one may cut a corner between two blocked orthogonals, off by default, and a diagonal costs 1414 to an orthogonal's 1000.
`region::flood` marks what is reachable into a `BitGrid` and `region::label_regions` numbers every connected component at once, both taking passability as a closure over flat indices so they run on anything.
`targeting::footprint` resolves a `TargetMode`, own cell, adjacent, bolt, ball, beam or cone, into the `Footprint` of cells it covers and the path a projectile took, with blocking passed in as two closures because what blocks is the caller's to say: what stops a projectile, walls and whoever stands in the way, and what stops a burst, walls alone.
`targeting::burst` is the burst on its own, every cell in the radius a straight line from the centre reaches without crossing a wall, which a ball bursts with where it lands and a thrown thing's trigger with where it comes down; a ball that hits a wall bursts on the near side of it.

## Using it

A game's tile layer is a registry filled by name and a table of its own keyed by the same ids: the tutorial registers a wall and a floor with the flags left at their defaults, and keeps the colours they are drawn in beside the registry rather than in the props.

<!-- include: ../../../../examples/tutorial/src/bin/step01_walking.rs:tiles -->
```rust,no_run
/// The warren's tiles, and how each one looks in full light.
struct Warren {
    tiles: TileRegistry,
    seed: RunSeed,
}

impl Warren {
    fn new(seed: RunSeed) -> Self {
        let mut tiles = TileRegistry::new();
        tiles.register(TileProps::wall("earth")).unwrap();
        tiles.register(TileProps::floor("dirt")).unwrap();
        Self { tiles, seed }
    }

    /// Both colours of every tile, and how much each cell jitters from
    /// its neighbours. The renderer derives darkness and memory from these.
    fn appearance(&self) -> TileAppearance {
        let mut look = TileAppearance::new();
        let t = |name| self.tiles.expect(name);
        look.set_varied(t("earth"), Cell::new('#', Color::srgb(0.78, 0.66, 0.50)).on(Color::srgb(0.34, 0.27, 0.21)), Vary::new(0.20, 0.05));
        look.set_varied(t("dirt"), Cell::new('.', Color::srgb(0.66, 0.58, 0.45)).on(Color::srgb(0.18, 0.15, 0.12)), Vary::new(0.28, 0.06));
        look
    }
}
```

## The line

Nothing on a gameplay or generation path is keyed by a `HashMap` or a `HashSet`.
`SpatialGrid` is a `BTreeMap`, `TileRegistry` looks names up through a `BTreeMap`, a viewshed is a `BitGrid` and a per-id table is a `Vec`, and the reason is the same one every time: a hash container iterates in an order that depends on the hasher, so a run would branch on something no seed controls and a fingerprint test would fail for no reason anyone could find.
The ordered container is also the faster one at these sizes, so the rule costs nothing to keep.
Costs are integers in hundredths of a step, the same unit as the turn clock, so a path's cost is the time it takes and `NORMAL_MOVE_COST` is 100.
`octile_milli` is scaled by a thousand rather than being a float for the same reason: two runs must never disagree over a last-bit compare.
The engine decides how a grid is addressed, what order neighbours come back in, which ring of cells `nearest_from` searches first, and how a tie in a path or a flood is broken; all of it is pinned by tests because a whole town's layout hangs off it.
The game decides what tiles exist, what each is called, which flags it carries, what it costs, what it opens into and how it burns.
The engine ships no tile enum, and there is nothing in it that matches on a tile: a game that wants ice, vacuum or a force field registers one and it is first-class from the first frame.
Anything only the game cares about, a glyph, a colour, a line of flavour text, belongs in a game-side table keyed by the same id, not in `TileProps`.

## Where it lives

`rl-core` is tier 0: points, rectangles, directions, the `Grid2D` contract and the geometry, with no dependency on anything above it and none on Bevy, so the row-major arithmetic and the shape iterators are tested by value and proved by property over a range of inputs rather than by looking at a screenshot.
`rl-grid` is tier 1 and adds what runs over a grid: tiles, terrain, the two source traits, the bit grid, the spatial index, the two pathfinders, regions and targeting shapes.
Both build for `wasm32-unknown-unknown`, which `scripts/check-tiers.sh --wasm` checks, so nothing in them may reach for `std::time::Instant`.
The split that matters is the one between `Terrain` and `TileRegistry`: identity in the grid, meaning in the registry, joined only by a `TerrainView`.
That is what lets the same terrain be read under a swimmer's costs, a smoke overlay or a viewer's knowledge at the same time, and what keeps a pathfinder from ever needing to know what a door is.
