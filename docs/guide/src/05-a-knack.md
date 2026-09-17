# A knack of your own

> Run it: `cargo run -p tutorial --bin step05_knack`
>
> Source: [`step05_knack.rs`](https://github.com/rudehn/rl-engine/blob/main/examples/tutorial/src/bin/step05_knack.rs)

<div class="demo" data-demo="step05_knack">
  <img src="images/10-panels.png" alt="Warren with a rail down the right, the log along the bottom">
  <button type="button">Play this step</button>
  <p class="weight">Loads about 8 MB</p>
</div>

One ability for you, one for the ratlings, and both of them written in a file rather than in Rust.

## An ability is data

<!-- include: ../../../examples/tutorial/assets/knacks.ron -->
```ron
// The one knack the warren gives you.
//
// Every field (the ones marked "optional" may be left out):
//   name:        unique; what the game looks it up by
//   description: optional; what it is, in a sentence, for the knack list
//   look:        optional; (glyph:, color: (r:, g:, b:)), what flies and bursts
//   aim:         Foe (default) | Ally | SelfOnly | Ground | Anyone
//   mode:        Own | Adjacent | Bolt(range:) | Ball(range:, radius:) |
//                Beam(range:) | Cone(length:)
//   costs:       optional; this one costs no pool, only the wait
//   cooldown:    optional; hundredths of a step before it may be used again
//   effects:     each (kind:, chance:, args:); "Harm" takes a damage kind
//                this game registered and a dice roll
#![enable(implicit_some)]
[
    (
        name: "screech",
        description: "A shriek that goes through a rat like a nail. It carries, and it hurts.",
        look: (glyph: '*', color: (r: 220, g: 220, b: 255)),
        aim: Foe,
        mode: Ball(range: 6, radius: 1),
        cooldown: 300,
        effects: [(kind: "Harm", args: (kind: "din", roll: "1d6"))],
    ),
    (
        name: "gnaw",
        description: "Eat the crust you are carrying. A ratling knows to do this; a rat does not.",
        aim: SelfOnly,
        mode: Own,
        costs: [Item("bread", 1)],
        effects: [(kind: "Mend", args: (kind: "care", roll: "1d6"))],
    ),
]
```

An aim, a shape, what it costs, how long before it may be used again, and a list of effects.
The engine ships seven effects and a game registers its own beside them with `add_effect`, so `add_engine_effects` is what makes `Harm` and `Mend` names this file may use.

The shape is the engine's targeting footprint: `Ball(range, radius)` here, and `Bolt`, `Beam`, `Cone`, `Adjacent` and `Own` beside it.
Nothing about the shape is written twice, because the cursor previews with the same footprint the resolver lands the effects with.

## Loading it, and knowing it

<!-- include: ../../../examples/tutorial/src/bin/step05_knack.rs:knack -->
```rust,no_run
/// The knacks this game loads, and the one the player is given.
const KNACKS_RON: &str = include_str!("../../assets/knacks.ron");

/// The ability ids the game holds on to, so a key can name one.
#[derive(Resource)]
struct Knacks {
    screech: AbilityId,
}
```

`Abilities::load` checks the file against the registries and the effects the app registered, so a knack naming a damage kind nobody registered fails at start-up rather than three floors down.

The player is given the knack with `Grants`.
`Known` is derived from `Grants`, what is worn and what is carried, every turn, so an item that grants an ability lends it for exactly as long as it is held.

## A key that only aims

The key writes `AimAt { user, ability }` and stops.
The cursor, the preview of what the burst would cover, the check that you can afford it and the spending of the turn are all the engine's.
That is the same shape as throwing a rock in [chapter 4](04-things-and-minds.md), because it is the same cursor.

## The one a monster uses

The ratlings get a knack too, and it is how they eat.
`gnaw` costs `Item("bread", 1)` and mends what it heals, so a ratling that carries a crust can spend it to patch itself up.
The cost is paid from the bag by tag, which is what the `Tagged` component on the crust is for.

Their brain gains `UseAbility`, which scores what an ability's footprint would land on rather than learning what any particular ability does.
A ratling at full health gains nothing by eating, so it does not bother; a hurt one does.
Nothing in the engine knows what bread is.

## Try it

- Add a second knack to the file and give it to the player. No Rust changes.
- Change `screech` from `Ball(range: 6, radius: 1)` to `Cone(length: 5)` and watch the preview change with it.
- Give the player `gnaw` as well and eat with an ability instead of `e`.

Next: [two floors, and a way out](06-two-floors.md).
