<!-- documents:
     plugins: FovPlugin, LightingPlugin
     files: crates/atglyph-bevy/src/fov.rs
            crates/atglyph-bevy/src/lighting.rs
            crates/atglyph-bevy/src/components.rs
            crates/atglyph-bevy/src/minds.rs
            crates/atglyph-bevy/src/knowledge.rs
            crates/atglyph-grid/src/fov.rs
            crates/atglyph-grid/src/light.rs
            crates/atglyph-bevy/src/items.rs
            crates/atglyph-mapgen/src/place.rs
     fingerprint: 03562a80 -->

# Sight and lighting

Sight is a viewshed per actor: a symmetric shadowcast from where it stands, out to its range, over the loaded window.
Lighting is a field over that same window, cast from every source that glows and blended with one ambient level for the map.
Where both are on, the field cuts a viewshed down to what is lit at or above a threshold, what is within the actor's dark sight, and what the actor is touching.
The player and every mind are cast and cut by the same code, so an unlit monster is not seen at all and a lamp-bearing one is seen coming.

## Turning it on

`FovPlugin` recasts every stale viewshed once a frame, in `EngineSet::Fov`.
With it alone there is no dark: `visible` is a copy of `line`, and an actor sees every tile it has an unobstructed line to.
`LightingPlugin` adds the field and the gate.
It inserts `Lighting::dark()`, lights each place built and casts in `EngineSet::Light` ahead of sight, and again in `DecideSet::Light` in any pass since a light changed, so a mind decides by the light as it is on its own turn, burns `Fuel` in `ResolveSet::Effects`, and resets `Lighting` to dark on a new run.
Adding it without `FovPlugin` builds a field nothing reads, because the gate is applied by the one function that writes a viewshed.
Both declare `depends_on::<CorePlugin>` in `finish`, which runs after every plugin is added, so the order they go into `add_plugins` does not matter.
A game that adds neither has no sight: nothing ever casts a `Viewshed`, `can_see` is false on every tile, and the map view draws nothing.

## The model

`LightSource(pub Glow)` is the one component for everything that glows, the `Glow` being the one light type, which a tile's definition, a prop's and a game's content all write.
`intensity` is the brightness at the source's own tile, `radius` how far it reaches, falling to exactly zero at the rim, and `color` its hue; `LightSource::darkness(intensity, radius)` takes light away instead, pitch black at its heart at 255.
Every cell of a place whose tile's definition carries a `Glow` gets a fixture when the place is built, and again when a continued run restores it, read off the terrain rather than saved; a prop whose definition carries a `light` gets its `LightSource` when its kind is put on it.
An entity that takes no turns is a fixture in the static layer and an actor or a carried item is in the dynamic layer; either is recast when its sorted list of emitters differs from the last cast, and both are recast whole when the window moves, when play crosses to another map, or when the map's opacity changes.
An item on the floor lights the tile it lies on, and once picked up it sheds from its carrier's tile instead.
`DarkSight(pub i32)` is how far an actor sees with no light at all; absent, it sees only what it is touching.
Set by hand it is left as set; an actor with an `OwnDarkSight`, or wearing a thing with a `WornDarkSight`, has it kept by `ItemsPlugin` as the best of the two, and none while the game has put `DarkSightSuppressed` on it.
`Fuel(pub u32)` is turns of light left, burned one per whole turn, and at zero the engine removes the `LightSource` and writes `LightEvent::BurntOut`.
The `Lighting` resource carries `ambient`, `threshold` and `bright` as public fields and its light layers privately; `at` gives a tile's light and `is_lit` compares that light against the threshold.
`ambient` is the current place's own, taken the first pass a place is current, and what a game writes there stands until the current map changes.
`threshold` starts at `DEFAULT_THRESHOLD`, which is 16, `bright` at 64, and `band` names a tile's `LightBand`: `Dark` below `threshold`, `Dim` from there to `bright`, and `Lit` at or above it.
The engine reads a missing `Lighting` as lit, the same way a game without lighting sees everywhere.
`Viewshed` holds both bit grids: `line` is the shadowcast, and `visible` is what the gate left of it.
`gate` writes the second from the first, and `cast` is the whole recast for one viewer, the shadowcast and then the gate.
`Concealed`, which a game puts on a thing, hides it from a viewer while its cell is not in the lit band and the two are not beside each other.
`PlayerSight` is the one answer to what the player sees of things, asked by the map, the list of what is in sight and every panel and cursor on it: `shows` is a thing on the current map, in the player's viewshed and not concealed from where it stands.
`Viewshed::everywhere()` is one that sees every tile of the loaded window: its cast sets both grids whole, asking neither the shadowcast nor the light, which is what an onlooker carries so that a run nobody plays is shown and told in full.
A mind's range comes from `Perception`, and an actor with `RevealsMap` writes `visible` into `Knowledge`, so a dark corridor is not remembered until something lights it.
`Knowledge` also keeps, per map, a `Recalled` glyph and colour for each tile a still thing was last seen on, which the map view writes through `see_still` and reads through `recalled`; `learn` marks a tile known without sight and has the next look take in what stands on it.

## Using it

The tutorial's lantern is a `LightSource` constant that a key puts on the player and takes off again.

<!-- include: ../../../../examples/tutorial/src/bin/step02_light.rs:lantern -->
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

## The line

Whether a thing glows is the game's call, made by inserting or removing a `LightSource`: the engine reads what is there and lights nothing on its own.
`ambient` is each place's own, set by the place's build, and a plain field a game may still write from a system of its own for a game with nights.
There is no clock hook and no notion of a day in the engine, so a game without a day cycle carries none of that machinery.
`flicker` is presentation: it reaches the renderer as the `waver` channel of a tile's light, and gameplay reads the steady `intensity`, so a guttering torch never changes what is seen and never changes a replay.
The engine puts a light out when `Fuel` reaches zero and reports it; what becomes of that entity, refilled or dropped or despawned, is the game's answer.
`Lighting` is derived and never saved, while `Fuel` and the presence of a `LightSource` on an item are the game's to save with its item state.
What a light means is the game's too: the engine knows emitters and one ambient level, and never a torch, a sun or a noon.
What the bands are worth is the game's: the engine names them, stealth reads the lit band for its bonus, and a to-hit model may read either.

## Where it lives

`atglyph-grid` is tier 1 and has no Bevy in it: `fov.rs` is the symmetric shadowcast, and `light.rs` is `Rgb`, `Light`, `Emitter` and the `LightField` that casts and composes them.
Both read a borrowed `OpacitySource` and write into buffers the caller owns, so a recast allocates nothing and either can be tested without an `App`.
`atglyph-bevy` is tier 2 and has the plugins: `fov.rs` holds `cast` and the recast of every stale viewshed over the `Viewshed` that `components.rs` defines, and `lighting.rs` holds the light components, the `Lighting` resource, the casting, the lighting of places and props, the fuel and `gate`.
`atglyph-render` reads the composed field once more, for the color and the waver that gameplay ignores.
