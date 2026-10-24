# A map, and walking on it

> Run it: `cargo run -p tutorial --bin step01_walking`
>
> Source: [`step01_walking.rs`](https://github.com/rudehn/rl-engine/blob/main/examples/tutorial/src/bin/step01_walking.rs)

<div class="demo" data-demo="step01_walking">
  <img src="images/01-a-map.png" alt="A dug room in warm browns, the player's @ in the middle">
  <button type="button">Play this step</button>
  <p class="weight">Loads about 8 MB</p>
</div>

Generate a floor, hand it to the engine, draw it from the player's point of view, and walk about on it.

## The app

<!-- include: ../../../examples/tutorial/src/bin/step01_walking.rs:main -->
```rust,no_run
fn main() -> AppExit {
    // What every game adds: the window and the glyph terminal, the turn
    // loop, sight, the map across the whole terminal, and the UI base.
    // Then the game, which is one plugin, and the seed it is played on.
    App::new().add_plugins(RoguelikePlugins::new("Warren", COLS, ROWS)).add_plugins(Warren).insert_resource(Seed(RunSeed(7))).run()
}
```

`RoguelikePlugins` is what every game adds, in one line.
It opens a window sized to an 80 by 40 cell glyph terminal and brings the plugins no game goes without: the glyph grid, the engine's turn loop and map, field of view, the map view, particles, and the UI base.

The game itself is one plugin, `Warren`, so `main` and the step's test add the same game.
Everything else is a plugin you name, starting in [chapter 3](03-blows-and-the-log.md), and nothing turns itself on because a resource happens to exist.

`Seed` is where all randomness comes from.
Every stream the engine draws on is derived from it, so the same number always builds the same warren.

## An intent, not a move

<!-- include: ../../../examples/tutorial/src/bin/step01_walking.rs:input -->
```rust,no_run
        // The keys every roguelike shares, named in Warren's words for the
        // `?` screen. Each press becomes an intent; the engine claims the
        // turn, charges it, and refuses what cannot be done.
        app.add_plugins(PlayerKeysPlugin::new().walk("Move", "walk").wait("Act", "wait a turn"));
```

Input never moves anybody.
`PlayerKeysPlugin` reads the keys every roguelike shares, the arrows, `hjklyubn` and the numpad for walking and `.` for waiting, and writes an `Intent<Bump>` or an `Intent<Wait>` for the player while it holds the turn.
The engine decides whether the actor may act, whether the move is legal, what it costs and what to do when it is not.
A key held down keeps walking, and a key pressed while something else is moving is dropped.

Each verb is named in the game's own words, which is how a controls screen lists it once a game adds one in [chapter 9](09-where-to-go-next.md).
A verb left out is neither bound nor read.
Quitting is Warren's own key: `add_control` declares it under a group and returns its id, and a system reads it through `ControlInput`.

## Tiles are ids

<!-- include: ../../../examples/tutorial/src/bin/step01_walking.rs:tiles -->
```rust,no_run
        // The engine's standard tiles, a wall, a floor and doors, drawn in
        // its standard look until the game gives them its own.
        app.places(Floors(TileRegistry::standard()));
```

`TileRegistry::standard()` is the engine's starting set: `void`, `wall`, `floor`, a closed door and an open one.
All the engine knows about a tile is whether it is walkable, whether it blocks sight and shots, and what it opens or closes into.
There is no `Tile::Wall` to extend, so lava, glass or a tile only ghosts can cross needs no engine change.

The standard tiles come with a standard look, filled in when the world comes up for any of the five the game gave no look of its own.
[Chapter 2](02-sight-and-light.md) registers tiles of Warren's own and says how each one looks.

`app.places` hands the engine the rules every floor is built by, once, where the game is put together.

## The floor is a chain of passes

<!-- include: ../../../examples/tutorial/src/bin/step01_walking.rs:rules -->
```rust,no_run
/// How a floor is built. The engine calls this once, the first time
/// something enters the map, with that floor's own seed, and keeps what
/// comes back.
struct Floors(TileRegistry);

impl PlaceRules for Floors {
    fn build(&self, _: MapId, seed: PlaceSeed) -> Result<PlaceBuild, BuildError> {
        let (wall, floor) = (self.0.expect("wall"), self.0.expect("floor"));
        let mut ctx = BaseContext::blank(84, 42, self.0.clone(), wall);
        Chain::new().then(dungeon::Rooms { floor, attempts: 40, min_size: 5, max_size: 10, min_rooms: 6 }).then(dungeon::RandomStart).run(&mut ctx, seed)?;
        PlaceBuild::from_context(ctx)
    }

    fn tiles(&self) -> TileTables {
        self.0.tables()
    }
}
```

`PlaceRules::build` is called once, the first time anything enters that map, and the result is kept for the run.
Its `PlaceSeed` is derived from the run's `Seed` and the map's number alone, so building one floor before another cannot change either.
`tiles` names the tile tables the places are built of, and the engine makes the run's `WorldMap` over them.

`Rooms` carves rectangles and joins them with corridors, and `RandomStart` picks a floor cell and reports it as the chain's start point.
Each pass keys its own random stream off its name, so adding a pass later does not shift the numbers an earlier pass draws.

`PlaceBuild::from_context` reads the finished chain: the start point becomes the entry, an exit point becomes the exit if some pass emitted one, and prefab marks become spots to populate.

## The player

<!-- include: ../../../examples/tutorial/src/bin/step01_walking.rs:start -->
```rust,no_run
/// The player, spawned into the first floor at its entry. The engine built
/// the world from the floors' tiles; the first warp in builds the floor.
fn start(mut commands: Commands) {
    commands.spawn_player(MapId::FIRST, (Viewshed::new(9), RevealsMap, Glyph::new('@', Color::WHITE).on_layer(10)));
}
```

`NewRun` is the schedule that starts a run, and the engine runs it again on a restart with the old run torn down first, which is why a game's setup goes there instead of in Bevy's `Startup`.

`spawn_player` spawns an `Actor` that `Blocks`, marked `Player`, with the bundle the game gives it, and warps it to the entry of `MapId::FIRST`, building that floor on the way in.
`Actor` takes turns, `Player` is the one the loop waits on for input, `Blocks` puts it in the occupancy index, and `RevealsMap` marks whose sight fills in the explored map.
The player is components, not a class.

Play begins on its own once the start has run and the run has a world.
Leave out the `Seed` and play refuses to begin, listing everything missing at once with how to make each.

## One pass of the turn loop

| Stage | What happens |
|---|---|
| `Schedule` | The clock advances and one actor is dealt `MyTurn` |
| `Decide` | Minds choose for everyone who is not the player |
| `Resolve` | Intents become changes to the world |
| `Sweep` | Anything nobody resolved is refused, with a warning naming it |
| `React` | The game answers what the turn caused |
| `Listen` | The pass's noises are heard, when the game added `NoisePlugin` |
| `Record` | What the pass did is read for the log |
| `Cleanup` | The actor is charged and requeued |

One actor holds a turn at a time and is out of the queue while it does.
`Cleanup` puts it back at `now + cost`.

Costs are hundredths of a normal step, and `BASE_ACTION_COST` is 100.
`Speed(200)` is twice as fast and the scheduler scales cost by it.
No floats in the clock, so a seed replays.

Walk into a wall and nothing happens: no time passes, and you keep the turn.
A monster handed a free retry would spin forever, so a blocked monster is charged for a wait instead.

## Try it

- Change `Viewshed::new(9)` to `4` and watch the room close in.
- Change the seed, then change it back and confirm you get the same floor.
- Give the player `Speed(200)` and watch the turn counter climb half as fast.

Next: [what you can see, and the dark](02-sight-and-light.md).
