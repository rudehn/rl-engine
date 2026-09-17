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
    let mut app = App::new();
    // What every game adds: the window and the glyph terminal, the turn
    // loop, sight, the map across the whole terminal, and the UI base.
    app.add_plugins(RoguelikePlugins::new("Warren", COLS, ROWS))
        .insert_resource(Seed(RunSeed(7)))
        .add_systems(NewRun, start)
        // Once a frame, before the turns: whatever the player pressed becomes
        // at most one intent, however many passes the turn loop then runs.
        .add_systems(Update, player_input.in_set(EngineSet::Input));
    app.run()
}
```

`RoguelikePlugins` is what every game adds, in one line.
It opens a window sized to an 80 by 40 cell glyph terminal and brings the plugins no game goes without: the glyph grid, the engine's turn loop and map, field of view, the map view, particles, and the UI base.

Everything else is a plugin you name, starting in [chapter 3](03-blows-and-the-log.md), and nothing turns itself on because a resource happens to exist.
Leave out the `WorldMap` and play refuses to begin, listing everything missing at once with how to make each.

`Seed` is where all randomness comes from.
Every stream the engine draws on is derived from it, so the same number always builds the same warren.
`NewRun` is the schedule that starts a run, and the engine runs it again on a restart with the old run torn down first, which is why a game's setup goes there instead of in Bevy's `Startup`.

## Tiles are ids

<!-- include: ../../../examples/tutorial/src/bin/step01_walking.rs:tiles -->
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

`register` returns a dense `TileId`; `expect` looks one up by name and panics if it is missing.
All the engine knows about a tile is whether it is walkable and whether it blocks sight.
There is no `Tile::Wall` to extend, so lava, glass or a tile only ghosts can cross needs no engine change.

`TileAppearance` holds what each id looks like in full light.
Both colours are authored because light multiplies them channel by channel and memory fades them.
`Vary` jitters each cell's colour by a hash of its position, so the floor is not graph paper.

## The floor is a chain of passes

<!-- include: ../../../examples/tutorial/src/bin/step01_walking.rs:rules -->
```rust,no_run
/// How a floor is built. The engine calls this once, the first time
/// something enters the map, and keeps what comes back.
impl PlaceRules for Warren {
    fn build(&self, _: MapId, _: Option<&WorldGraph>) -> Result<PlaceBuild, BuildError> {
        let (wall, floor) = (self.tiles.expect("earth"), self.tiles.expect("dirt"));
        let mut ctx = BaseContext::blank(84, 42, self.tiles.clone(), wall);
        Chain::new()
            .then(dungeon::Rooms { floor, attempts: 40, min_size: 5, max_size: 10, min_rooms: 6 })
            .then(dungeon::RandomStart)
            .run(&mut ctx, self.seed)?;
        PlaceBuild::from_context(ctx)
    }
}
```

`PlaceRules::build` is called once, the first time anything enters that map, and the result is kept for the run.

`Rooms` carves rectangles and joins them with corridors, and `RandomStart` picks a floor cell and reports it as the chain's start point.
Each pass keys its own random stream off its name, so adding a pass later does not shift the numbers an earlier pass draws.

`PlaceBuild::from_context` reads the finished chain: the start point becomes the entry, an exit point becomes the exit if some pass emitted one, and prefab marks become spots to populate.

## Handing it over

<!-- include: ../../../examples/tutorial/src/bin/step01_walking.rs:start -->
```rust,no_run
/// Hands the engine the map rules and the player, then warps the player in.
fn start(mut commands: Commands, seed: Res<Seed>, mut warps: MessageWriter<WarpRequest>, mut next: ResMut<NextState<EngineState>>) {
    let warren = Warren::new(seed.0);
    commands.insert_resource(warren.appearance());
    commands.insert_resource(WorldMap::new(warren.tiles.tables()));
    commands.insert_resource(PlaceRulesRes(Box::new(warren)));

    let player =
        commands.spawn(((Actor, Player, Blocks, Position(Point::ZERO)), (Viewshed::new(9), RevealsMap, Glyph::new('@', Color::WHITE).on_layer(10)))).id();
    warps.write(WarpRequest::into_place(player, WARREN));
    next.set(EngineState::Playing);
}
```

`WorldMap::new` takes the tile tables alone.
No world graph, no region size, no surface: a game with an overworld adds those, a delve never names them.

The player is components, not a class.
`Actor` takes turns, `Player` is the one the loop waits on for input, `Blocks` puts it in the occupancy index, `RevealsMap` marks whose sight fills in the explored map.

## An intent, not a move

<!-- include: ../../../examples/tutorial/src/bin/step01_walking.rs:input -->
```rust,no_run
/// The player, but only while it is holding the turn.
type PlayerTurn<'w, 's> = Query<'w, 's, Entity, (With<Player>, With<MyTurn>)>;

/// Keys to intents. Writing an intent is the whole of asking to act: the
/// engine claims the turn, charges it, and refuses what cannot be done.
fn player_input(
    keys: Res<ButtonInput<KeyCode>>,
    dirs: Res<DirectionKeys>,
    player: PlayerTurn,
    mut steps: MessageWriter<Intent<Step>>,
    mut waits: MessageWriter<Intent<Wait>>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::KeyQ) {
        exit.write(AppExit::Success);
        return;
    }
    // No turn in hand means it is somebody else's move; the key is dropped.
    let Ok(entity) = player.single() else { return };
    if let Some(dir) = dirs.just_pressed(&keys) {
        steps.write(Intent::new(entity, Step(dir)));
    } else if keys.just_pressed(KeyCode::Period) || keys.just_pressed(KeyCode::Numpad5) {
        waits.write(Intent::new(entity, Wait));
    }
}
```

Input never moves anybody.
It writes an `Intent<Step>` and stops.
The engine decides whether the actor may act, whether the move is legal, what it costs and what to do when it is not.

`With<MyTurn>` makes the query empty unless the player is holding a turn, so a key pressed while something else is moving is dropped.

`DirectionKeys` is the engine's binding of the arrows, `hjklyubn` and the numpad to the eight directions, and `just_pressed` answers which one was struck.
It is a resource, so a game that wants other keys replaces it and writes no match statement of its own.

## One pass of the turn loop

| Stage | What happens |
|---|---|
| `Schedule` | The clock advances and one actor is dealt `MyTurn` |
| `Decide` | Minds choose for everyone who is not the player |
| `Resolve` | Intents become changes to the world |
| `Sweep` | Anything nobody resolved is refused, with a warning naming it |
| `React` | The game answers what the turn caused |
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
