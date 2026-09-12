# Blows

> Run it: `cargo run -p tutorial --bin step05_combat`
> Source: [`step05_combat.rs`](https://github.com/rudehn/rl-engine/blob/main/examples/tutorial/src/bin/step05_combat.rs)

![Two rats in the lit room closing on the player, the log counting their bites in red, HP down to 20](images/05-blows.png)

## Bump to attack is your decision

```rust,no_run
{{#include ../../../examples/tutorial/src/bin/step05_combat.rs:input}}
```

The engine ships an `Attack` action and never decides when to use it.
Walking into an occupied cell being a strike is a convention, not a law; a game where you swap places with your pet writes a different branch here.

`Occupancy` is the engine's index of everything with `Blocks`, kept per map.
`first_at` is a lookup, not a scan.

## What a hit passes through

```rust
    commands.insert_resource(DamageStages(vec![Box::new(SubtractArmor)]));
```

A `Hit` carries the kind, the dice and who threw it.
Each stage in turn may change the number before it lands.

Warren has one stage.
Resistances by damage kind, a shield that eats the first hit each turn, a critical rule reading the attacker's stats: each is another entry in that list, in the order you put them.

```rust
            (Health::full(24), Armor(1), MeleeAttack { kind: kinds.expect("kick"), dice: DiceRoll::new(1, 6) }),
```

`DiceRoll::new(1, 6)` is `1d6`, and it parses from `"1d6+2"` when it comes out of a content file.

## Two events

- `DamageEvent` is a hit on its way in, before the stages run. Write one to hurt somebody: poison, a fall, a trap.
- `DamageDealt` is what landed, after the stages, with the final number.

Narration reads the second, because "bites you for 1" should be what the player lost, not what the rat rolled.

```rust,no_run
{{#include ../../../examples/tutorial/src/bin/step05_combat.rs:narrate}}
```

`DeathEvent` carries `was_player` and who gets the credit.
The dead linger until the end of the frame, so anything that wanted to react to a death still finds the entity.

## Ending the run

```rust
            next.set(EngineState::Idle);
```

Leaving `Playing` stops the loop: no turns, no input, no sight, and the last frame stays on screen with the log under it.

## Try it

- Add a stage that halves every hit and read the log to confirm the order.
- Give the rats `Resists` and a damage kind they shrug off.
- Write a `DamageEvent` from a key press. The pipeline does not care where a hit came from.
