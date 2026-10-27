# What you can see, and the dark

> Run it: `cargo run -p tutorial --bin step02_light`
>
> Source: [`step02_light.rs`](https://github.com/rudehn/rl-engine/blob/main/examples/tutorial/src/bin/step02_light.rs)

<div class="demo" data-demo="step02_light">
  <img src="images/03-sight.png" alt="Rooms lit by the lantern, remembered corridors in cold blue">
  <button type="button">Play this step</button>
  <p class="weight">Loads about 8 MB</p>
</div>

The warren goes dark, and you carry the only light.
Three panels arrive: a status row, a message log, and a list of the keys.

## Panels

<!-- include: ../../../examples/tutorial/src/bin/step02_light.rs:main -->
```rust,no_run
fn main() -> AppExit {
    App::new()
        // The map now leaves the top row for the vitals and the bottom rows
        // for the log.
        .add_plugins(RoguelikePlugins::new("Warren", COLS, ROWS).map(GridRect::new(0, 1, COLS, ROWS - 1 - LOG_ROWS)))
        .add_plugins(Warren)
        .add_plugins((
            VitalsPanel::new(GridRect::new(0, 0, COLS - 13, 1)),
            LogPanel::new(GridRect::new(0, ROWS - LOG_ROWS, COLS, LOG_ROWS)),
            // `?` lists every key the game declared; the top-right corner
            // says so.
            ControlsPanel::new(GridRect::new(4, 3, COLS - 8, ROWS - LOG_ROWS - 6)).hint(GridRect::new(COLS - 13, 0, 12, 1)),
        ))
        .insert_resource(Seed(RunSeed(7)))
        .run()
}
```

The map now leaves the top row and the bottom five rows free.
Each panel is a plugin that takes the rectangle it draws in, and draws itself.

- `VitalsPanel` shows your health and anything the game adds to it.
- `LogPanel` shows the latest messages.
- `ControlsPanel` lists every declared key when you press `?`, and prints that hint in the top-right corner.

## Lighting

<!-- include: ../../../examples/tutorial/src/bin/step02_light.rs:plugin -->
```rust,no_run
/// The game: its floors, its start and its keys.
struct Warren;

impl Plugin for Warren {
    fn build(&self, app: &mut App) {
        let floors = Floors::new();
        app.insert_resource(floors.appearance()).places(floors);
        // Without lighting the world is lit everywhere. With it, you see
        // only what a light reaches.
        app.add_plugins(LightingPlugin).insert_resource(Lighting::dark());
        app.add_systems(NewRun, start);
        app.add_systems(Update, (player_input, tend_lantern).in_set(EngineSet::Input).run_if(no_modal));
        // The vitals view is data the panel in `main` draws; the game writes
        // a line onto it, so the game adds it.
        app.add_plugins(VitalsViewPlugin).add_systems(Update, note_explored.in_set(ViewSet::Annotate));
        declare_controls(app);
    }
}
```

Without `LightingPlugin` the whole world is lit.
With it and `Lighting::dark()`, you see only what a light reaches.
What you have seen stays on the map, drawn dim.

## A lantern

<!-- include: ../../../examples/tutorial/src/bin/step02_light.rs:lantern -->
```rust,no_run
/// The lantern's light: warm, and a little restless.
const LANTERN: LightSource = LightSource::new(150, 7, Rgb::new(255, 210, 140)).flickering(30);

/// The player, while it holds the turn, and whether its lantern is open.
type Lantern<'w, 's> = Query<'w, 's, (Entity, Has<LightSource>), (With<Player>, With<MyTurn>)>;

/// Opens or shades the lantern, which spends the turn. The light is a
/// component, so shading it is removing it.
fn tend_lantern(keys: ControlInput, binds: Res<Binds>, mut commands: Commands, player: Lantern, mut waits: MessageWriter<Intent<Wait>>, mut log: Log) {
    if !keys.just_pressed(binds.lantern) {
        return;
    }
    let Ok((entity, lit)) = player.single() else { return };
    if lit {
        commands.entity(entity).remove::<LightSource>();
        log.muted("You shade the lantern.");
    } else {
        commands.entity(entity).insert(LANTERN);
        log.notice("You open the lantern.");
    }
    waits.write(Intent::new(entity, Wait));
}
```

A light is a component, `LightSource`, so opening the lantern inserts one and shading it removes one.
Writing `Intent<Wait>` makes it cost a turn.

The player starts with it open:

<!-- include: ../../../examples/tutorial/src/bin/step02_light.rs:start -->
```rust,no_run
/// Spawns the player at the first floor's start, lantern open.
fn start(mut commands: Commands, seed: Res<Seed>, mut log: Log) {
    commands.spawn_player(MapId::FIRST, (Viewshed::new(9), RevealsMap, LANTERN, Glyph::new('@', Color::WHITE).on_layer(10)));
    log.notice(format!("Seed {}. You squeeze into the warren.", seed.0.0));
}
```

`Log` writes a line to the message log.

## The key

<!-- include: ../../../examples/tutorial/src/bin/step02_light.rs:keys -->
```rust,no_run
/// Warren's own keys.
#[derive(Resource, Clone, Copy)]
struct Binds {
    lantern: ControlId,
    quit: ControlId,
}

/// Declares the keys. Walking and waiting are the engine's standard keys.
fn declare_controls(app: &mut App) {
    app.add_plugins(PlayerKeysPlugin::new().walk("Move", "walk").wait("Act", "wait a turn"));
    let binds = Binds { lantern: app.add_control("Act", "lantern on or off", KeyCode::KeyT), quit: app.add_control("Game", "quit", KeyCode::KeyQ) };
    app.insert_resource(binds);
}
```

`t` joins `Binds`, and it shows up on the `?` screen with the rest.

## A line on the status row

<!-- include: ../../../examples/tutorial/src/bin/step02_light.rs:status -->
```rust,no_run
/// Adds a line to the vitals panel: how much of the map you have seen.
fn note_explored(mut vitals: ResMut<VitalsView>, knowledge: Res<Knowledge>) {
    vitals.facets.push(Facet::new("explored", format!("explored {}", knowledge.explored_count())));
}
```

The engine fills `VitalsView` each frame, and the panel draws it.
A game adds its own lines as a `Facet` in `ViewSet::Annotate`, after the engine has filled the view.

## Try it

- Press `t` in a corridor and watch it go black.
- Change the lantern's radius from `7` to `3`.

Next: [blows, and the log that tells you](03-blows-and-the-log.md).
