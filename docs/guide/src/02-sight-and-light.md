# What you can see, and the dark

> Run it: `cargo run -p tutorial --bin step02_light`
>
> Source: [`step02_light.rs`](https://github.com/rudehn/rl-engine/blob/main/examples/tutorial/src/bin/step02_light.rs)

<div class="demo" data-demo="step02_light">
  <img src="images/03-sight.png" alt="Rooms lit by the lantern, remembered corridors in cold blue">
  <button type="button">Play this step</button>
  <p class="weight">Loads about 8 MB</p>
</div>

The warren goes dark, and you carry the only light in it.

## Walkable and opaque are separate flags

<!-- include: ../../../examples/tutorial/src/bin/step02_light.rs:tiles -->
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
        // Walkable and opaque: you can step through a curtain of roots,
        // but you cannot see past one until you do.
        tiles.register(TileProps::floor("roots").opaque(true)).unwrap();
        Self { tiles, seed }
    }

    /// Both colours of every tile, and how much each cell jitters from
    /// its neighbours. The renderer derives darkness and memory from these.
    fn appearance(&self) -> TileAppearance {
        let mut look = TileAppearance::new();
        let t = |name| self.tiles.expect(name);
        look.set_varied(t("earth"), Cell::new('#', Color::srgb(0.78, 0.66, 0.50)).on(Color::srgb(0.34, 0.27, 0.21)), Vary::new(0.20, 0.05));
        look.set_varied(t("dirt"), Cell::new('.', Color::srgb(0.66, 0.58, 0.45)).on(Color::srgb(0.18, 0.15, 0.12)), Vary::new(0.28, 0.06));
        look.set_varied(t("roots"), Cell::new('+', Color::srgb(0.55, 0.74, 0.45)).on(Color::srgb(0.16, 0.22, 0.13)), Vary::new(0.18, 0.05));
        look
    }
}
```

Doors depend on it.
A curtain of roots is walkable and opaque: you can step through one, but you cannot see past it until you do.

Field of view reads opacity off the tile tables through an `OpacitySource`, so a tile you invented five minutes ago blocks sight correctly.

## Seen, and once seen

`Viewshed` carries two bit grids.
`line` is every tile with an unobstructed line to it, and `visible` is what the actor really sees.
They are the same set until lighting is on, when `visible` shrinks to what is lit, within dark sight, or adjacent.

`Knowledge` is the explored map, kept per map id and filled by whoever carries `RevealsMap`.
It is a resource, not a component: the player's map is the game's map.

The map view combines the three.
Visible tiles are drawn in their authored colours, explored-but-unseen ones run through `Memory`, and everything else is blank.
The cold blue of a remembered corridor is not a colour anyone chose. It is the brown floor, remembered.

## A light to carry

<!-- include: ../../../examples/tutorial/src/bin/step02_light.rs:lantern -->
```rust,no_run
/// What the lantern sheds when it is open: a warm, slightly restless pool.
const LANTERN: LightSource = LightSource::new(150, 7, Rgb::new(255, 210, 140)).flickering(30);

/// The player and whether its lantern is open, while it holds the turn.
type Lantern<'w, 's> = Query<'w, 's, (Entity, Has<LightSource>), (With<Player>, With<MyTurn>)>;

/// `l` opens the lantern or shades it, and spends the turn either way.
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
    if !keys.just_pressed(KeyCode::KeyL) {
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

`Lighting::dark()` is the whole of turning the lights off, and `LightingPlugin` is the subsystem that then matters.
A `LightSource` is a component, so shading the lantern is removing one and opening it is putting it back.

Sight is unchanged by any of this.
What changes is which of the tiles in line are lit enough to resolve, which is why walking into the dark with the lantern shut still fills in the floor you are standing on.

Every actor has its own viewshed, cast the same way, so a monster that sheds no light is found only where a light reaches it.
That cuts both ways, and [chapter 3](03-blows-and-the-log.md) gives the rats dark sight so they can still find you.

## Try it

- Shade the lantern and walk a corridor. The explored map keeps what the light already reached.
- Change `LANTERN`'s radius from 7 to 2 and feel the warren close in.
- Give the lantern `Fuel` and watch it burn out.

Next: [blows, and the log that tells you](03-blows-and-the-log.md).
