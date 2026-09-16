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

`RoguelikePlugins` is what every game adds, in one line.
It opens a window sized to an 80 by 40 cell glyph terminal and adds the plugins no game goes without:

- `TerminalPlugin`, the glyph grid, one Bevy sprite per cell. Nothing about it is roguelike; every other drawing plugin writes into it.
- `CorePlugin`, the engine: the turn loop, the clock, the occupancy index, the map and its places, the `EngineState` machine, and three actions that need nothing else (step, wait, go through).
- `FovPlugin`, which computes what each actor can see. The map view draws a tile only if the player's `Viewshed` says it is visible or the explored map says it was.
- `MapViewPlugin`, the map, drawn across the whole terminal unless `.map(rect)` gives it less.
- `UiPlugin`, the base every panel in chapter 3 onward needs.
- `CapturePlugin`, which is only how this guide's screenshots are taken, and does nothing unless asked.

Everything else is a plugin you name, starting in chapter 4, and nothing turns itself on because a resource happens to exist.
The group is Bevy's own kind, so any of it can be swapped or switched off: `RoguelikePlugins::new("Warren", 80, 40).build().disable::<CapturePlugin>()`.
Leave out the `WorldMap` and play refuses to begin, listing everything missing at once with how to make each.

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
Three tiles are fine to write out like this.
When the warren gains its bestiary in [chapter 8](08-content-in-files.md), the looks move into a file beside it, and `TileAppearance::load` reads them.

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
