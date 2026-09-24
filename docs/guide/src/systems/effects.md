<!-- documents:
     plugins: EffectsPlugin
     files: crates/rl-bevy/src/effects/mod.rs
            crates/rl-bevy/src/effects/triggers.rs
            crates/rl-bevy/src/effects/engine.rs
            crates/rl-bevy/src/consumable.rs
            crates/rl-bevy/src/plugin.rs
            crates/rl-bevy/src/items.rs
            crates/rl-bevy/src/throwing.rs
            crates/rl-bevy/src/combat.rs
            crates/rl-bevy/src/props.rs
            crates/rl-rules/src/ability.rs
            crates/rl-grid/src/targeting.rs
     fingerprint: 5ceb8ab3 -->

# Effects

An effect is one thing that happens to a cell or to whoever stands in it: harm, a mend, a status, a shove, a fire, a cloud of gas.
Three kinds of thing land them, an ability an actor knows, a prop in the world and an item in a bag, and this is the subsystem all three use as peers.
It owns what an effect is, how a list of them is built, rolled and described, the one stream they roll from, and the moments at which a prop or an item does what it does.
What an effect costs the thing that carried it is not here; that is `ConsumablesPlugin`'s, which reads the same messages.

## Turning it on

`EffectsPlugin` is added by any plugin that lands effects, `AbilitiesPlugin`, `PropsPlugin` and `ConsumablesPlugin`, when the game has not added it, so a game rarely names it and the order a game lists its plugins in never matters.
It initializes `EffectKinds` and `Moments`, takes the `EffectRng` stream, registers `Fired` and the `DamageEvent`, `Afflict` and `Cure` every landing may write, reads `Cued`, and runs `report_remnants` then `land_triggers` in `ResolveSet::Triggers`.
It is not unique and builds once, so a game that adds it by hand as well gets one copy and no error.
That stage sits between `ResolveSet::Act` and `ResolveSet::Fields`: after every action that reports a moment, and before fire, gas, statuses and damage, so a grenade's fire spreads and a stim's mend is applied in the pass that set them off.
`add_engine_effects()` registers the seven effects that need no subsystem, and `FirePlugin` and `GasPlugin` register `Ignite` and `Emit`, so a content file naming either loads exactly when the game has fire or gas.
A game registers an effect of its own with `app.add_effect::<E>()` and a moment of its own with `app.add_moment(name)`, both while the app is built.

## The model

`Effect` is a type with `apply(&self, &Landing, &mut EffectWorld)` and a `describe` a menu reads, and `FromArgs` builds one from the text arguments a content file gave it.
`EffectKinds` files each under its `KIND`, and `Effects::build` turns a list of `EffectSpec` into built effects or reports every spec that would not build.
`Landing` is what an effect sees: the user, what landed it as a `Source` of `Ability`, `Trigger { on, moment }` or `Offer`, the origin and aim, every cell covered and every actor under them.
`EffectWorld` asks the subsystem that owns a thing to do it, damage, a status on or off, a cue, and moves an actor itself, since nothing else owns that.
`Effects::land` rolls each entry against its own chance from `EffectRng`, which keeps the derivation domain `b"ability"` it had as `AbilityRng`, so a trap, a stim and a spell are dealt from one deck and every seed rolls what it rolled before.
`Moments` interns the names of moments, the engine's `use`, `land`, `fire`, `hit`, `entered` and `destroyed` first so their ids are constants on the type.
`TriggerSpec`, in `rl-rules` beside `EffectSpec`, is the authored form: `on`, a moment by name; `area`, `Here` or `Burst { radius }`; `fires`, how many times before it stops; `effects`, a list of its own; and `look`, what shows over the cells it lands on.
`Triggers::build(specs, shared, moments, kinds, names)` builds a definition's triggers once, sharing each list through an `Arc`, and fails on a moment nobody registered, naming it.
A spec with no list takes the definition's `shared` one, and a spec with neither fails, since a trigger that does nothing is a typo.
`Triggers` is the component, and each copy counts its own `fires`, so springing one cable spends nothing of another.
`Fired { on, moment, by, at }` is how a subsystem reports a moment, and all it does: items report `use` on an accepted use, throwing reports `land` where a throw comes to rest, combat reports `fire` for each attack made with a worn thing and `hit` at the struck actor's cell, and props report `entered` and `destroyed`.
`land_triggers` reads each `Fired` in the order written, takes the entity's triggers for that moment in list order, skips any spent, cues a burst over the cells for one with a `look`, lands the list over `area_cells`, the one cell or the burst `rl_grid::burst` works out inside the loaded window, with everyone in the area as a target, the one who set it off included, and counts `fires` down.
The landing's user, who a hit is credited to, is `by`, or the carrier itself when it is `LandsAsItself`, which every armed prop is.
A `Remnant` is a carrier that lands its triggers once and is despawned, for something gone by the time they land: it holds its moment, `by` and `at` as data, and `report_remnants` writes its `Fired` in the pass that lands it, so a pass held back while something is shown loses nothing.
A broken prop leaves one for its `destroyed` moment, and a watched shot from a thing its last charge spent leaves one for its `hit`.

## Using it

A trigger is a moment, an area and a list, and Foundry's grenades are four of them, each a burst where the grenade comes down.

<!-- include: ../../../../examples/foundry/assets/items.ron:grenades -->
```ron
    // The grenades: thrown, and what each does is its land trigger, a
    // burst where it comes to rest, which spends it there. Plate takes half
    // a frag burst off a droid; an ion burst undoes a chassis and blinds
    // every radar in it, and barely touches flesh.
    (name: "frag grenade", glyph: '*', color: (0.7, 0.72, 0.45), stack: true, throw: (range: 6), consumable: (charges: 1, when_empty: Destroyed),
     triggers: [(on: "land", area: Burst(radius: 1), look: (glyph: '*', color: (r: 255, g: 200, b: 90)), effects: [(kind: "Harm", args: (kind: "kinetic", roll: "3d6"))])]),
    (name: "smoke grenade", glyph: '*', color: (0.75, 0.75, 0.75), stack: true, throw: (range: 6), consumable: (charges: 1, when_empty: Destroyed),
     triggers: [(on: "land", area: Burst(radius: 1), look: (glyph: '*', color: (r: 200, g: 200, b: 200)), effects: [(kind: "Emit", args: (gas: "smoke", amount: 160))])]),
    (name: "ion grenade", glyph: '*', color: (0.4, 0.7, 1.0), stack: true, throw: (range: 6), consumable: (charges: 1, when_empty: Destroyed),
     triggers: [(on: "land", area: Burst(radius: 2), look: (glyph: '*', color: (r: 90, g: 170, b: 255)), effects: [(kind: "Harm", args: (kind: "ion", roll: "1d4"))])]),
    (name: "incendiary grenade", glyph: '*', color: (0.95, 0.45, 0.2), stack: true, throw: (range: 6), consumable: (charges: 1, when_empty: Destroyed),
     triggers: [(on: "land", area: Burst(radius: 1), look: (glyph: '*', color: (r: 255, g: 110, b: 40)), effects: [(kind: "Harm", args: (kind: "thermal", roll: "2d4")), (kind: "Ignite", args: (turns: 4))])]),
```

A game builds its definitions' triggers once when it reads the file, against the moments and kinds the run registered, so a typo is a startup failure and not a grenade that lands nothing.

<!-- include: ../../../../examples/foundry/src/gear.rs:triggers -->
```rust,no_run
        // Each definition's triggers, built once against the moments and
        // effect kinds the run registered, and every problem in the file
        // named at once: a grenade that lands nothing is a typo.
        let mut triggers = Vec::new();
        let mut errors = Vec::new();
        for (_, d) in defs.iter() {
            triggers.push(match Triggers::build(&d.triggers, &d.effects, moments, kinds, &names) {
                Ok(t) => t,
                Err(mine) => {
                    errors.extend(mine.into_iter().map(|e| format!("{}: {e}", d.name)));
                    Triggers::default()
                }
            });
        }
        assert!(errors.is_empty(), "assets/items.ron: {}", errors.join("; "));
```

## The line

The engine decides what an effect does to the world, when a moment's triggers land, over which cells and with what dice; what a thing is, and which moments it answers, is the game's content.
A subsystem that owns a moment reports it and does nothing else, so "find the list, build the area, land it" is written once and a game's own moment lands exactly as an engine one does.
Moments are an open registry and not an enum, for the reason every other name in the engine is: a blaster that does something when it overheats is a line of the game's, not a fork.
One list can be delivered several ways, which is the whole of the potion model: what a bottle holds is written once as the item's `effects`, and a `use` trigger and a `land` trigger each deliver it to different people.
An item is aimed only by being thrown or fired, so there is no cursor here; that is throwing's and combat's, and an aimed use with a cursor of its own would be an ability by another name.
A burst stops at walls and not at whoever stands in it, and it is the call an ability's `Ball` bursts with, so a grenade and a fireball of one radius reach the same cells and neither reaches round a wall.
A trap lands in the pass it was stepped on, and a prop's `destroyed` trigger a pass after the blow, since a death is known only after damage.
A trap's harm is the trap's own and not the doing of whoever stepped on it, so the log never says they hurt themselves; who stepped on it is still on the report.
Charges are not here: a game with triggers and no costs adds no `ConsumablesPlugin`, and `spend_charges` reads `Fired` after `land_triggers` so what the last charge did lands before the thing is gone.
Only what changes in play is saved, a trigger's fires left beside a consumable's charges, and the lists are rebuilt from the definitions.

## Where it lives

`rl-rules` is tier 1 and has no Bevy in it: `EffectSpec` and `TriggerSpec` are the authored forms any game's own file can deserialize, so a content file is checked for shape without an `App`.
`rl-bevy` is tier 2 and holds the rest in one module, because an effect needs the world to land: the machinery and the plugin, the engine's nine effects beside it, and the triggers with the moments they answer.
The effects sit in their own module rather than each in the subsystem it asks, so combat, statuses, fire and gas never depend on the code that asks them, and abilities, props and items depend on effects rather than on each other.
That split is what let a prop's trap stop reaching into abilities to land anything, and what lets a triggers test build a floor, report a moment by hand and read the damage without an ability, a prop or an item anywhere.
