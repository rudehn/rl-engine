# Testing without a window

> Run it: `cargo test -p tutorial`
>
> Source: the test module of [`step10_panels.rs`](https://github.com/rudehn/rl-engine/blob/main/examples/tutorial/src/bin/step10_panels.rs)

Warren's tests run the real game: the same `start` system, the same map generation, the same turn loop, with no window.

## A headless app

<!-- include: ../../../examples/tutorial/src/bin/step10_panels.rs:headless -->
```rust,no_run
    /// The warren with no window: the engine plugins the game uses, the
    /// game's own systems, and nothing that needs a screen.
    fn headless(seed: u64) -> App {
        let mut app = rl_engine::rl_bevy::plugin::headless_app();
        app.add_plugins((FovPlugin, CombatPlugin, MindsPlugin, ItemsPlugin));
        app.insert_resource(Seed(RunSeed(seed)))
            .add_plugins(UiPlugin)
            .add_choice::<Shove>()
            .add_message::<Shoved>()
            .add_systems(NewRun, start)
            .add_systems(Turn, resolve_shoves.in_set(ResolveSet::Act))
            .add_systems(Turn, (populate, eat).in_set(TurnSet::React))
            .add_systems(Update, narrate.in_set(PresentSet::Narrate));
        app
    }

    /// A started run: two frames is enough for the warp to build floor one
    /// and the scheduler to deal the player its first turn.
    fn started(seed: u64) -> (App, Entity) {
        let mut app = headless(seed);
        app.update();
        app.update();
        let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        (app, player)
    }

    fn act<A: Action>(app: &mut App, actor: Entity, action: A) {
        app.world_mut().write_message(Intent::new(actor, action));
        app.update();
    }
```

`headless_app` is `MinimalPlugins`, states and `CorePlugin`.
`UiPlugin` goes in too, though nothing draws: it owns the log your systems write to.
You add the engine plugins your game uses and your own systems, exactly as `main` does, minus the three that draw.

Two `update` calls start a run: the first runs `NewRun`, your start system, and the warp that builds floor one, the second deals the player its first turn.
After that, one per action.

Writing an intent is how the game plays itself.
The suite runs in about forty milliseconds.

## What goes into one

`rl_engine::rl_bevy::testing` is the engine's own test kit, and a game's tests use the same copy:

- `KeyScriptPlugin` and `press(&mut app, key)` play a key the way a keyboard does, so a test drives your real input system rather than writing intents by hand.
- `surface(&mut app)` stands an open test world up and hands back ground to start on, for a test that needs a map and not the game's own.
- `two_sides(&mut app)` inserts combat rules for two sides at war, for a test that fights.

## Test a property

Where a property exists, assert it over a range of seeds.

<!-- include: ../../../examples/tutorial/src/bin/step10_panels.rs:property -->
```rust,no_run
    /// The property that has to hold for every floor of every run: you can
    /// stand where you arrive, and there is somewhere to go from there.
    #[test]
    fn every_floor_of_every_seed_has_a_walkable_way_in_and_a_way_on() {
        for seed in 1u64..=12 {
            let warren = Warren::new(RunSeed(seed));
            let tables = warren.tiles.tables();
            for depth in 1..=FLOORS {
                let built = warren.build(map_of(depth), None).unwrap_or_else(|e| panic!("seed {seed} floor {depth}: {e}"));
                let walkable = |p| built.terrain.get(p).is_some_and(|t: TileId| tables.walkable[t.index()]);
                assert!(walkable(built.entry), "seed {seed} floor {depth}: arrived inside a wall");
                assert!(built.exit.is_some_and(walkable), "seed {seed} floor {depth}: nowhere to go on to");
            }
        }
    }
```

Forty-eight generated floors, and the assertion is what actually has to be true: you can stand where you arrive, and there is somewhere to go on to.
It does not care where the rooms are, so tuning `min_size` does not break it, and it does catch a cave generator that walls off the stairs on seed 9.

It also never builds an `App`, which is why forty-eight floors cost milliseconds.
Generation is tier 1; test it there.

Where no property exists, use a fingerprint test and say so in the name, so a change reads as a change rather than a failure.

Names read as sentences:

```text
every_floor_of_every_seed_has_a_walkable_way_in_and_a_way_on
a_shove_at_nobody_is_refused_costs_nothing_and_leaves_the_turn_in_hand
```

## Testing an action

<!-- include: ../../../examples/tutorial/src/bin/step10_panels.rs:shove_tests -->
```rust,no_run
    /// A walkable cell next to the player with another walkable cell
    /// behind it, which is what a shove needs to land.
    fn room_to_shove(app: &App, from: Point) -> Direction {
        let map = app.world().resource::<WorldMap>();
        Direction::ALL
            .into_iter()
            .find(|d| map.is_walkable(from + d.offset()) && map.is_walkable(from + d.offset() + d.offset()))
            .expect("the entry of a built floor has room around it")
    }

    #[test]
    fn a_shove_moves_the_rat_one_cell_further_off_and_spends_half_a_turn() {
        let (mut app, player) = started(7);
        let at = app.world().get::<Position>(player).unwrap().0;
        let dir = room_to_shove(&app, at);
        let rat = app.world_mut().spawn((Actor, Blocks, Position(at + dir.offset()))).id();
        app.update();

        let before = app.world().resource::<Turns>().now();
        act(&mut app, player, Shove(dir));
        assert_eq!(app.world().get::<Position>(rat).unwrap().0, at + dir.offset() + dir.offset(), "the rat went back a cell");
        assert_eq!(app.world().resource::<Turns>().now(), before + SHOVE_COST, "and it cost half a turn");
    }

    #[test]
    fn a_shove_at_nobody_is_refused_costs_nothing_and_leaves_the_turn_in_hand() {
        let (mut app, player) = started(7);
        let at = app.world().get::<Position>(player).unwrap().0;
        let dir = room_to_shove(&app, at);

        let before = app.world().resource::<Turns>().now();
        act(&mut app, player, Shove(dir));
        assert_eq!(app.world().resource::<Turns>().now(), before, "no time passed");
        assert!(app.world().get::<MyTurn>(player).is_some(), "the player still holds the turn");
    }

    /// A hog's brain decides the same shove the player's key writes, and
    /// the engine resolves it the same way: the player goes back a cell.
    #[test]
    fn a_hog_beside_the_player_shoves_rather_than_bites() {
        let (mut app, player) = started(7);
        let at = app.world().get::<Position>(player).unwrap().0;
        // A line of three open cells through the player: the hog behind,
        // the player, and where the player is shoved to.
        let dir = {
            let map = app.world().resource::<WorldMap>();
            Direction::ALL
                .into_iter()
                .find(|d| map.is_walkable(at - d.offset()) && map.is_walkable(at + d.offset()))
                .expect("the entry of a built floor has room around it")
        };
        let hog_at = at - dir.offset();
        let hog = app.world().resource::<Bestiary>().defs.expect("warren hog");
        app.world_mut().resource_scope(|world: &mut World, bestiary: Mut<Bestiary>| {
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let mut commands = Commands::new(&mut queue, world);
            bestiary.spawn(&mut commands, hog, hog_at);
            queue.apply(world);
        });
        app.update();
        let hp = app.world().get::<Health>(player).unwrap().current;
        act(&mut app, player, Wait);
        assert_eq!(app.world().get::<Position>(player).unwrap().0, at + dir.offset(), "the hog shoved the player a cell away");
        assert_eq!(app.world().get::<Health>(player).unwrap().current, hp, "and did not bite");
    }
```

`room_to_shove` is worth copying as a habit.
On generated maps, a test that assumes there is space to the east fails on seed 12 for reasons unrelated to the code under test.

The second test is the one people leave out.
It asserts the refusal: no time passed, the player still holds the turn.
A wrong refusal crashes nothing; it silently eats a turn or freezes the loop.

## Try it

- Break `resolve_shoves` on purpose and watch which test says so first.
- Add a seed to the property test's range and see the cost stay flat.
- Write a test that presses a key with `press` rather than writing the intent, and delete `player_input` to watch it fail.

Next: [where to go next](12-where-to-go-next.md).
