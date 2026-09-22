<!-- documents:
     plugins: FovPlugin, LightingPlugin
     files: crates/rl-bevy/src/fov.rs
            crates/rl-bevy/src/lighting.rs
            crates/rl-bevy/src/components.rs
            crates/rl-bevy/src/minds.rs
            crates/rl-bevy/src/knowledge.rs
            crates/rl-grid/src/fov.rs
            crates/rl-grid/src/light.rs
     fingerprint: 84060e1e -->

# Sight and lighting

Sight is a viewshed per actor: a symmetric shadowcast from where it stands, out to its range, over the loaded window.
Lighting is a field over that same window, cast from every source that glows and blended with one ambient level for the map.
Where both are on, the field cuts a viewshed down to what is lit at or above a threshold, what is within the actor's dark sight, and what the actor is touching.
The player and every mind are cast and cut by the same code, so an unlit monster is not seen at all and a lamp-bearing one is seen coming.

## Turning it on

`FovPlugin` recasts every stale viewshed once a frame, in `EngineSet::Fov`.
With it alone there is no dark: `visible` is a copy of `line`, and an actor sees every tile it has an unobstructed line to.
`LightingPlugin` adds the field and the gate.
It inserts `Lighting::dark()`, casts in `EngineSet::Light` ahead of sight, burns `Fuel` in `ResolveSet::Effects`, and resets `Lighting` to dark on a new run so that each run writes its own ambient.
Adding it without `FovPlugin` builds a field nothing reads, because the gate is applied by the one function that writes a viewshed.
Both declare `depends_on::<CorePlugin>` in `finish`, which runs after every plugin is added, so the order they go into `add_plugins` does not matter.
A game that adds neither has no sight: nothing ever casts a `Viewshed`, `can_see` is false on every tile, and the map view draws nothing.

## The model

`LightSource` is the one component for everything that glows.
`intensity` is the brightness at the source's own tile, `radius` how far it reaches, falling to exactly zero at the rim, and `color` its hue.
An entity that takes no turns is a fixture in the static layer and an actor or a carried item is in the dynamic layer; either is recast when its sorted list of emitters differs from the last cast, and both are recast whole when the window moves, when play crosses to another map, or when the map's opacity changes.
An item on the floor lights the tile it lies on, and once picked up it sheds from its carrier's tile instead.
`DarkSight(pub i32)` is how far an actor sees with no light at all; absent, it sees only what it is touching.
`Fuel(pub u32)` is turns of light left, burned one per whole turn, and at zero the engine removes the `LightSource` and writes `LightEvent::BurntOut`.
The `Lighting` resource carries `ambient` and `threshold` as public fields and its three light layers privately; `at` gives a tile's light and `is_lit` compares that light against the threshold.
`threshold` starts at `DEFAULT_THRESHOLD`, which is 16.
`Viewshed` holds both bit grids: `line` is the shadowcast, and `visible` is what the gate left of it.
`gate` writes the second from the first, and `perceives` answers the same question about a single target without a viewshed; with no `Lighting` it is always true.
A mind's range comes from `Perception`, and an actor with `RevealsMap` writes `visible` into `Knowledge`, so a dark corridor is not remembered until something lights it.

## Using it

The tutorial's lantern is a `LightSource` constant that a key puts on the player and takes off again.

<!-- include: ../../../../examples/tutorial/src/bin/step02_light.rs:lantern -->
```rust,no_run
/// What the lantern sheds when it is open: a warm, slightly restless pool.
const LANTERN: LightSource = LightSource::new(150, 7, Rgb::new(255, 210, 140)).flickering(30);

/// The player and whether its lantern is open, while it holds the turn.
type Lantern<'w, 's> = Query<'w, 's, (Entity, Has<LightSource>), (With<Player>, With<MyTurn>)>;

/// `t` opens the lantern or shades it, and spends the turn either way.
///
/// The light is a component on the player, so shading it is removing one.
/// Nothing else changes: sight is still sight, and the explored map still
/// remembers what the light once reached.
fn tend_lantern(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    player: Lantern,
    mut waits: MessageWriter<Intent<Wait>>,
    mut log: ResMut<MessageLog>,
    turns: Res<Turns>,
) {
    if !keys.just_pressed(KeyCode::KeyT) {
        return;
    }
    let Ok((entity, lit)) = player.single() else { return };
    if lit {
        commands.entity(entity).remove::<LightSource>();
        log.muted("You shade the lantern. The warren closes to arm's length.", turns.turn_number());
    } else {
        commands.entity(entity).insert(LANTERN);
        log.notice("You open the lantern. The dirt comes up warm around you.", turns.turn_number());
    }
    waits.write(Intent::new(entity, Wait));
}
```

## The line

Whether a thing glows is the game's call, made by inserting or removing a `LightSource`: the engine reads what is there and lights nothing on its own.
`ambient` is a plain field the game writes, once at startup for a dungeon and from a system of its own for a surface with nights.
There is no clock hook and no notion of a day in the engine, so a game without a day cycle carries none of that machinery.
`flicker` is presentation: it reaches the renderer as the `waver` channel of a tile's light, and gameplay reads the steady `intensity`, so a guttering torch never changes what is seen and never changes a replay.
The engine puts a light out when `Fuel` reaches zero and reports it; what becomes of that entity, refilled or dropped or despawned, is the game's answer.
`Lighting` is derived and never saved, while `Fuel` and the presence of a `LightSource` on an item are the game's to save with its item state.
What a light means is the game's too: the engine knows emitters and one ambient level, and never a torch, a sun or a noon.

## Where it lives

`rl-grid` is tier 1 and has no Bevy in it: `fov.rs` is the symmetric shadowcast, and `light.rs` is `Rgb`, `Light`, `Emitter` and the `LightField` that casts and composes them.
Both read a borrowed `OpacitySource` and write into buffers the caller owns, so a recast allocates nothing and either can be tested without an `App`.
`rl-bevy` is tier 2 and has the plugins: `fov.rs` holds `is_stale`, `cast` and `update_viewsheds` over the `Viewshed` that `components.rs` defines, and `lighting.rs` holds the light components, the `Lighting` resource, `update_lighting`, `tick_fuel` and `gate`.
`rl-render` reads the composed field once more, for the color and the waver that gameplay ignores.
