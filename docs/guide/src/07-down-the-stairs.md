# Down the stairs

> Run it: `cargo run -p tutorial --bin step07_floors`
>
> Source: [`step07_floors.rs`](https://github.com/rudehn/rl-engine/blob/main/examples/tutorial/src/bin/step07_floors.rs)

## A place is a map that is kept

<!-- include: ../../../examples/tutorial/src/bin/step07_floors.rs:floors -->
```rust,no_run
/// How deep the warren goes.
const FLOORS: u32 = 4;

/// Map zero is the streamed surface, which the warren has none of, so its
/// floors are maps one upward.
fn map_of(floor: u32) -> MapId {
    MapId(floor)
}

/// The floor a map id is.
fn floor_of(map: MapId) -> u32 {
    map.0
}

/// What a floor is called.
fn name_of(floor: u32) -> &'static str {
    match floor {
        1 => "the Burrow",
        2 => "the Middens",
        3 => "the Bone Nest",
        _ => "the King's Chamber",
    }
}
```

Map zero is the streamed surface, which Warren does not have.
Everything above it is a *place*: built the first time it is entered, then kept.

Kept, not regenerated.
Leaving freezes the actors and items where they stand; the rat you ran from is still in the corridor at the health you left it.
Actors on other maps are skipped by the scheduler instead of simulated, so a deep run costs no more per turn than a shallow one.

Every positioned entity is on exactly one map, tagged `OnMap`, which the engine fills in.

## One rules object, four floors

```rust
        let chain = match depth {
            1 | 2 => {
                Chain::new().then(dungeon::Rooms { floor: open, attempts: 40, min_size: 5, max_size: 10, min_rooms: 6 }).then(dungeon::Doors { door: roots })
            }
            3 => Chain::new().then(passes::CellularCave { wall, floor: open, fill_pct: 45, ..Default::default() }).then(passes::KeepLargestRegion { wall }),
            _ => Chain::new().then(dungeon::Rooms { floor: open, attempts: 60, min_size: 12, max_size: 18, min_rooms: 2 }),
        };
        chain.then(dungeon::RandomStart).then(dungeon::FarthestExit).run(&mut ctx, seed)?;
```

`build` is handed the `MapId`, so the shape of the run is a `match`.

`KeepLargestRegion` after a cellular cave is not optional.
Cave generators produce islands, and an unreachable half is a floor where the stairs are sometimes unreachable.
[Chapter 11](11-testing.md) asserts it.

```rust
        let seed = RunSeed(self.seed.0 ^ (depth as u64) << 32);
```

Floor three is the same floor whether you went straight down or spent two hundred turns on floor one.
Derive from the run seed and the thing being built, never from a counter the player can move.

`FarthestExit` runs on every floor.
On one to three that point is the stairs down; on the last there are none, so it is where the king waits.

## Stairs are entities

```rust
            commands.spawn((
                Position(down),
                Transition { to: Destination::Place { map: map_of(depth + 1), arrive: Arrive::Entry } },
                Glyph::new('>', stone).on_layer(1),
            ));
```

There is no stairs table and no special tile flag.
Nothing stops you putting one on a rat.

`Arrive` says where you land: the map's `Entry`, its `Exit`, or a cell.
Down arrives at the entry, up arrives at the exit, which lines the stairs up both ways.

`GoThrough` takes one and is refused off a transition.
`WarpRequest` does the same from anywhere, which is how the run started and how a trapdoor would work.

## Winning

```rust
        if kings.contains(d.entity) {
            over.write(RunOver::won().saying("The rat king falls. The scratching stops."));
```

A victory condition is a component and an `if`, and the ending is one message.
`RunOver::won()` ends the run the way the player's death does: the menu opens over the last frame with Warren's words above it, and the morgue file says `Won`.
[Chapter 12](12-where-to-go-next.md) points at the quest system, which is this with the objectives in a file.

## Try it

- Add a fifth floor. You should only touch the `match` and `FLOORS`.
- Put a one-way `Transition` back to floor one on the bottom floor.
- Go down, kill a rat, come back up, go down again. The rat stays dead.

Next: [content in files](08-content-in-files.md).
