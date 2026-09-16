# Monsters

> Run it: `cargo run -p tutorial --bin step04_monsters`
>
> Source: [`step04_monsters.rs`](https://github.com/rudehn/rl-engine/blob/main/examples/tutorial/src/bin/step04_monsters.rs)

Rats that hunt you. They cannot bite yet.

## Combat, and the minds that choose it

Deciding where to move and deciding whom to hit are the same decision, asked of the same priority list.
That decision is `MindsPlugin`'s, and what a blow does once it is struck is `CombatPlugin`'s, so a monster that thinks needs both.
Forget `MindsPlugin` and the first monster spawned says so in the log, rather than standing still all run.

`CombatPlugin` needs its rules before play begins, and panics naming them if they are missing:

```rust
    let kinds = Registry::from_defs(vec![DamageKind::new("bite"), DamageKind::new("kick")]).unwrap();
    let sides = Registry::from_defs(vec![FactionDef::new("you"), FactionDef::new("vermin")]).unwrap();
    let (you, vermin) = (sides.expect("you"), sides.expect("vermin"));
    commands.insert_resource(CombatRules::new(&sides).hostile(you, vermin));
    commands.insert_resource(Registries { damage_kinds: kinds.clone(), factions: sides, ..default() });
```

Damage kinds and factions are registries, like tiles.
They go in `Registries`, the one resource every subsystem reads its registries from.
`CombatRules` is only who is hostile to whom, held as a matrix over pairs rather than a flag on a monster, so a three-way war costs nothing extra and `hunts(a, b)` makes a grudge that is not returned.

Combat rolls from a stream the engine derives from the run's `Seed`, which `main` inserted in [chapter one](01-a-map-on-screen.md); a game never inserts a stream of its own.
Its own draws come from `seed.stream(name, index)`, below: randomness always comes from the seed, never from entropy and never from a constant.

## A brain is a priority list

<!-- include: ../../../examples/tutorial/src/bin/step04_monsters.rs:creatures -->
```rust,no_run
/// What every rat in the warren shares: one brain and one faction.
#[derive(Resource)]
struct Rats {
    mind: Arc<Brain<Entity>>,
    faction: FactionId,
}
```

```rust
        mind: Arc::new(Brain::new().then(Hunt).then(Wander { chance_pct: 40 })),
```

Tactics are asked in order and the first that answers wins.
Next chapter puts `MeleeAdjacent` at the front and `FleeWhenHurt` behind it, and the reading order is the behaviour.

The brain holds no state about any particular rat, so sixty rats share one `Arc`.
What a tactic needs is passed in: a snapshot of what that actor can see, its health, its position.

That snapshot is cut to the actor's own `Viewshed`, the same component the player has had since [chapter 3](03-what-the-player-knows.md), and then to its `Perception`, which is how far its mind considers what it sees and is eight cells unless you say otherwise.

`Hunt` does not pathfind per rat per turn.
It asks the engine for the way toward the enemies it sees, and the engine keeps one Dijkstra flow field per set of goals and movement class, so sixty rats after one player read their downhill step off one flood.
A companion asks the same way toward its allies, with `Follow`, and a searcher toward the cell it last saw you on.

## Filling the floor

<!-- include: ../../../examples/tutorial/src/bin/step04_monsters.rs:populate -->
```rust,no_run
/// Fills the floor the one time it is built. `PlaceEntered::first` is
/// true only on that arrival, so coming back does not restock it.
fn populate(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, rats: Res<Rats>, map: Res<WorldMap>, seed: Res<Seed>) {
    for ev in entered.read() {
        if !ev.first {
            continue;
        }
        let Some(place) = map.place(ev.map) else { continue };
        let bounds = place.terrain.bounds();
        // A stream of its own, keyed by name: adding another spawner later
        // cannot shift the numbers this one draws.
        let mut rng = seed.stream(b"warren.rats", ev.map.0 as u64);
        let mut placed = 0;
        while placed < 16 {
            let p = Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom()));
            // Not on top of the player, and not close enough to be unfair.
            if !map.is_walkable(p) || geometry::chebyshev(p, ev.entry) < 8 {
                continue;
            }
            commands.spawn((
                (Actor, Blocks, Position(p), Speed(110), Faction(rats.faction)),
                (Health::full(6), Perception(7), Mind(rats.mind.clone()), Glyph::new('r', Color::srgb(0.72, 0.55, 0.45)).on_layer(5)),
            ));
            placed += 1;
        }
    }
}
```

`PlaceEntered::first` is true exactly once: the arrival that built the map.
Spawning behind it is what stops the floor restocking every time you come back down.
It runs in `TurnSet::React`, inside the turn rather than after the frame is drawn, which [chapter 6](06-items.md) explains.

The random stream is named for this spawner and keyed by the floor.
Add a spawner for crusts later and it asks for a stream of its own, so it cannot shift the numbers this one draws.

Rats spawn at least eight cells from the arrival.
The engine will happily drop one on the player's head; fairness is your call.

## Try it

- Reverse the tactics to `Wander` then `Hunt` and watch the rats lose interest. Order is behaviour.
- Give them `Perception(20)` and see them converge from across the floor.
- Set the relation to `Neutral` and walk through a crowd that no longer cares.

Next: [blows](05-blows.md).
