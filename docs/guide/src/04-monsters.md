# Monsters

> Run it: `cargo run -p tutorial --bin step04_monsters`
> Source: [`step04_monsters.rs`](https://github.com/rudehn/rl-engine/blob/main/examples/tutorial/src/bin/step04_monsters.rs)

Rats that hunt you. They cannot bite yet.

## Minds live in the combat plugin

Deciding where to move and deciding whom to hit are the same decision, asked of the same priority list.

`CombatPlugin` needs two resources before play begins, and panics naming them if they are missing:

```rust
    let kinds = Registry::from_defs(vec![DamageKind::new("bite"), DamageKind::new("kick")]).unwrap();
    let sides = Registry::from_defs(vec![FactionDef { name: "you".into() }, FactionDef { name: "vermin".into() }]).unwrap();
    let (you, vermin) = (sides.expect("you"), sides.expect("vermin"));
    let mut factions = Factions::new(&sides);
    factions.set_mutual(you, vermin, Relation::Hostile);
    commands.insert_resource(CombatRules { kinds: kinds.clone(), factions });
    commands.insert_resource(CombatRng::for_run(seed.0));
```

Damage kinds and factions are registries, like tiles.
`Factions` is a relation matrix, so hostility is a fact about a pair rather than a flag on a monster: three-way wars cost nothing extra.

`CombatRng::for_run` derives the combat stream from the run seed.
Randomness always comes through `RunSeed`, never from entropy and never from a constant.

## A brain is a priority list

```rust,no_run
{{#include ../../../examples/tutorial/src/bin/step04_monsters.rs:creatures}}
```

```rust
        mind: Arc::new(Brain::new().then(Hunt).then(Wander { chance_pct: 40 })),
```

Tactics are asked in order and the first that answers wins.
Next chapter puts `MeleeAdjacent` at the front and `FleeWhenHurt` behind it, and the reading order is the behaviour.

The brain holds no state about any particular rat, so sixty rats share one `Arc`.
What a tactic needs is passed in: a snapshot of what that actor can see, its health, its position.

`Hunt` does not pathfind per rat per turn.
The engine keeps a Dijkstra flow field per movement profile and every hunter reads its downhill step off the same field.

## Filling the floor

```rust,no_run
{{#include ../../../examples/tutorial/src/bin/step04_monsters.rs:populate}}
```

`PlaceEntered::first` is true exactly once: the arrival that built the map.
Spawning behind it is what stops the floor restocking every time you come back down.

The random stream is named for this spawner and keyed by the floor.
Add a spawner for crusts later and it asks for a stream of its own, so it cannot shift the numbers this one draws.

Rats spawn at least eight cells from the arrival.
The engine will happily drop one on the player's head; fairness is your call.

## Try it

- Reverse the tactics to `Wander` then `Hunt` and watch the rats lose interest. Order is behaviour.
- Give them `Perception(20)` and see them converge from across the floor.
- Set the relation to `Neutral` and walk through a crowd that no longer cares.
