# A map on screen

> Run it: `cargo run -p tutorial --bin step01_a_map`
> Source: [`step01_a_map.rs`](https://github.com/rudehn/rl-engine/blob/main/examples/tutorial/src/bin/step01_a_map.rs)

Generate a dungeon floor, hand it to the engine, draw it from the player's point of view.
Nothing moves yet.

![A dug room drawn in warm browns, the player's @ in the middle, everything past the walls black](images/01-a-map.png)

## The app

```rust,no_run
{{#include ../../../examples/tutorial/src/bin/step01_a_map.rs:main}}
```

`TerminalPlugin` is the glyph grid: 80 by 40 cells, one Bevy sprite each.
Nothing about it is roguelike; every other drawing plugin writes into it.

`CorePlugin` is the engine: the turn loop, the clock, the occupancy index, the map and its places, the `EngineState` machine, and three actions that need nothing else (step, wait, go through).
Everything else is a plugin you name.
Nothing turns itself on because a resource happens to exist.

`FovPlugin` computes what each actor can see.
It is not optional here: the map view draws a tile only if the player's `Viewshed` says it is visible or the explored map says it was, so `MapViewPlugin` refuses to start without it and says which plugin to add.
Leave out the `MapView` or the `WorldMap` and play refuses to begin the same way, listing everything missing at once with how to make each.

`CapturePlugin` is only how this guide's screenshots are taken.

## Tiles are ids

```rust,no_run
{{#include ../../../examples/tutorial/src/bin/step01_a_map.rs:tiles}}
```

`register` returns a dense `TileId`; `expect` looks one up by name and panics if it is missing.
The engine knows whether a tile is walkable and whether it blocks sight, and nothing else.
There is no `Tile::Wall` to extend, so lava, glass or a tile only ghosts can cross needs no engine change.

`TileAppearance` holds what each id looks like in full light.
Both colours are authored because light multiplies them channel by channel and memory fades them.
`Vary` jitters each cell's colour by a hash of its position, so the floor is not graph paper; `shimmering` makes that jitter drift over time, which is how water moves.

## The floor is a chain of passes

```rust,no_run
{{#include ../../../examples/tutorial/src/bin/step01_a_map.rs:rules}}
```

`PlaceRules::build` is called once, the first time anything enters that map, and the result is kept for the run.

`Rooms` carves rectangles and joins them with corridors.
`RandomStart` picks a floor cell and reports it as the chain's start point.
Each pass keys its own random stream off its name, so adding a pass later does not shift the numbers an earlier pass draws.

`PlaceBuild::from_context` reads the finished chain: the start point becomes the entry, an exit point becomes the exit if some pass emitted one, and prefab marks become spots to populate.
It fails if no pass emitted a start.

## Handing it over

```rust,no_run
{{#include ../../../examples/tutorial/src/bin/step01_a_map.rs:start}}
```

`WorldMap::new` takes the tile tables alone.
No world graph, no region size, no surface: a game with an overworld adds those, a delve never names them.

The player is components, not a class.
`Actor` takes turns, `Player` is the one the loop waits on for input, `Blocks` puts it in the occupancy index, `RevealsMap` marks whose sight fills in the explored map.

Map zero is the streamed surface, which Warren has none of, so its one floor is map one.
`WarpRequest::into_place` puts the player there, which triggers the build.
The engine's system sets do not run outside `EngineState::Playing`, which is what lets a title screen exist.

## Try it

- Change `Viewshed::new(9)` to `4` and watch the room close in.
- Change the seed, then change it back and confirm you get the same floor.
- Register a third tile and add `.then(passes::Scatter { name: "rubble", tile: rubble, on: floor, chance_pct: 6 })`.
