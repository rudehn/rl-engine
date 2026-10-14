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
            crates/rl-bevy/src/fire.rs
            crates/rl-bevy/src/gas.rs
            crates/rl-bevy/src/noise.rs
            crates/rl-bevy/src/registries.rs
            crates/rl-rules/src/ability.rs
            crates/rl-bevy/src/ability.rs
            crates/rl-bevy/src/cue.rs
            crates/rl-grid/src/targeting.rs
            crates/rl-ui/src/narrate.rs
     fingerprint: 626bf1d0 -->

# Effects

An effect is one thing that happens to a cell or to whoever stands in it: harm, a mend, a status, a shove, a fire, a cloud of gas.
Three kinds of thing land them, an ability an actor knows, a prop in the world and an item in a bag, and this is the subsystem all three use as peers.
It owns what an effect is, how a list of them is built, rolled and described, the one stream they roll from, and the moments at which a prop or an item does what it does.
What an effect costs the thing that carried it is not here; that is `ItemsPlugin`'s charges, which read the same messages.

## Turning it on

`EffectsPlugin` is added by any plugin that lands effects, `AbilitiesPlugin` and `PropsPlugin`, when the game has not added it, so a game rarely names it and the order a game lists its plugins in never matters.
It initializes `EffectKinds` and `Moments`, takes the `EffectRng` stream, registers `Fired`, `FiredOn` and the `DamageEvent`, `Afflict` and `Cure` every landing may write, reads `Cued`, adds `Airborne<UseLanding>` for a use in the air, and runs `report_remnants`, `land_aimed_uses` and `land_triggers`, chained, in `ResolveSet::Triggers`.
It is not unique and builds once, so a game that adds it by hand as well gets one copy and no error.
That stage sits between `ResolveSet::Act` and `ResolveSet::Fields`: after every action that reports a moment, and before fire, gas, statuses and damage, so a grenade's fire spreads and a stim's mend is applied in the pass that set them off.
`add_engine_effects()` registers the seven effects that need no subsystem, and `FirePlugin`, `GasPlugin` and `NoisePlugin` register `Ignite`, `Emit` and `Noise`, so a content file naming one loads exactly when the game has fire, gas or hearing.
A game registers an effect of its own with `app.add_effect::<E>()` and a moment of its own with `app.add_moment(name)`, both while the app is built.

## The model

`Effect` is a type with `apply(&self, &Landing, &mut EffectWorld)`, a `describe(&self, &Registries, EffectBonus)` a menu reads with that bonus added, `FromArgs` to build one from the text arguments a content file gave it, and a `would_act(&self, &Landing, &World)` that answers true unless an effect says otherwise, read from the world without changing it: the engine's `Mend` says true only with someone hurt among the landing's targets, so a game answering it for an effect of its own says the same of whatever its own effect needs there to act on.
Every engine effect's arguments refuse a field they do not name, so a typo fails the load naming it rather than leaving a number at its default.
`EffectKinds` files each under its `KIND`, and `Effects::build` turns a list of `EffectSpec` into built effects or reports every spec that would not build.
`Landing` is what an effect sees: the user, what landed it as a `Source` of `Ability`, `Trigger { on, moment }` or `Offer`, the origin and aim, every cell covered and every actor under them, `bonus`, the `EffectBonus` of whatever landed it, which `land_triggers` copies off the carrier and which is the default, adding nothing, for an ability, an offer or a plain thing, and `on`, the thing a use was made on when the use asked its user for one.
`Landing::reach` is how its harm arrives, `Reach::Ability` for an ability, `Reach::Thing` for the trigger of a thing other than its own user, a rod zapped or a flask broken, and `Effect` for anything else, a prop springing its own trap among them, and `Harm` stamps it on every `DamageEvent` it writes so the narrator names what did it.
`EffectBonus { turns, amount }` is a component the game writes on a thing when it spawns it, whatever made it stronger already applied, as it writes `Armor` and `Bestows`: `Harm` and `Mend` add its `amount` to the roll's bonus through `roll_with` and `Inflict` adds its `turns` through `turns_with`, and every other engine effect ignores it, so the engine never learns what a level is.
`Noise { sound, loudness }` makes one `MakeNoise` by the landing's user where a projectile stopped, else where it was aimed, however many cells the footprint covers, and describes itself as nothing, since a bag line saying a grenade makes a noise would crowd out what it does.
Its sound is resolved at load against the names it is built with, and those carry the sounds only when a loader chains `SoundNames::sounds` onto them, since sounds are interned while the app is built and `Registries::names` has none; a sound nobody declared, or a `loudness` below one, is a load error naming it.
`Inflict` also takes `while_worn`, false unless written: landed by a trigger, its `Afflict` is then held by the thing the trigger is on, and the status ends when that thing is no longer worn; landed by an ability or an offer, it is not held.
It is meant for a trigger that only ever lands on its own wearer, since a held refresh reaching an area's or a hit's bystander who already carries the status unheld takes their instance over and the very next pass cures it for not wearing the thing.
It describes itself with ` while worn` after the turns, so the bag says so under the thing.
`EffectWorld` asks the subsystem that owns a thing to do it, damage, a heal, a status on or off, a cue, and moves an actor itself, since nothing else owns that.
`Effects::land` rolls each entry against its own chance from `EffectRng`, which keeps the derivation domain `b"ability"` it had as `AbilityRng`, so a trap, a stim and a spell are dealt from one deck and every seed rolls what it rolled before.
`Moments` interns the names of moments, the engine's `use`, `land`, `fire`, `hit`, `entered`, `destroyed` and `pulse` first so their ids are constants on the type.
`TriggerSpec`, in `rl-rules` beside `EffectSpec`, is the authored form: `on`, a moment by name; `area`, `Here` or `Burst { radius }`; `fires`, how many times before it stops; `effects`, a list of its own; `look`, what shows over the cells it lands on and, for an aimed use, what flies there; `every`, the hundredths between pulses, which a `pulse` trigger must carry and no other may; `targeting`, a `Targeting` for a `use` pointed at somewhere, written in an ability's words; and `asks`, an `Asks` of a prompt and an optional tag, for a `use` made on another of its user's things.
`Triggers::build(specs, shared, moments, kinds, names)` builds a definition's triggers once, sharing each list through an `Arc`, and fails on a moment nobody registered, a pulse with no `every` or an `every` of nought, an `every` on any other moment, a `targeting` or an `asks` on anything but a `use`, the two together, a targeting aimed at nobody but its user, and a second `use` trigger beside one that aims or asks, naming each.
The built `Trigger` keeps its `every`, and `Triggers::pulse_every` reads a carrier's back, so a game building a worn thing's `Pulse` takes its period from the trigger rather than keeping a second copy, and `Triggers::targeting` and `Triggers::asks` read back how a carrier's use is pointed and what it asks for.
A spec with no list takes the definition's `shared` one, and a spec with neither fails, since a trigger that does nothing is a typo.
`Triggers` is the component, and each copy counts its own `fires`, so springing one cable spends nothing of another.
`Fired { on, moment, by, at }` is how a subsystem reports a moment, and all it does: items report `use` on an accepted use, where its user stands or at the cell an aimed one was pointed at, and `pulse` for each worn `Pulse` whose period came round, throwing reports `land` where a throw comes to rest, combat reports `fire` for each attack made with a worn thing, `hit` at each struck actor's cell and the attacker's own `hit` beside it, and `struck` for the one struck, before its damage lands, and props and actors alike report `entered`, `destroyed` and `died`.
An actor that carries `Triggers` answers its own `hit`, `struck` and `died`, credited to it directly rather than to whoever it names `by`, since an actor is the one a thing is used by rather than a thing used: a `struck` aura spares the actor it is on, unlike a prop's or a thing's area, which catches everyone standing in it.
`land_triggers` reads each `Fired` in the order written, takes the entity's triggers for that moment in list order, skips any spent, cues a blast over the cells for one with a `look`, going out from the cell the moment happened on, lands the list over `area_cells`, the one cell or the burst `rl_grid::burst` works out inside the loaded window, with everyone in the area as a target, the one who set it off included, and counts `fires` down.
A `use` of a carrier whose `Consumable` has nothing left lands nothing at all, read before that use's charge is spent, since only a thing not yet known is let be used empty.
The landing's user, who a hit is credited to, is `by`, or the carrier itself when it is `LandsAsItself`, which every armed prop is.
A use made on one of its user's things is reported beside its `Fired` by a `FiredOn { carrier, moment, on }`, its own message so no other moment carries a field it never fills, and `land_triggers` pairs the two and lands the use where its user stands with the thing as `Landing::on`, for the game's own effect that reads it.
A use with a `targeting` is landed as an ability is, by `Bystanders::land` from wherever its user stands to the cell it was pointed at, on whoever its aim wants under its shape, the landing worked out as the use is made.
With a look it is seen to fly there and to burst in the same look, and while something watches it waits in `Airborne<UseLanding>` until its flight has been seen, when `land_aimed_uses` lands it ahead of the pass's other triggers; without a look it lands at once.
A `Remnant` is a carrier that lands its triggers once and is despawned, for something gone by the time they land: it holds its moment, `by` and `at` as data, and `report_remnants` writes its `Fired` in the pass that lands it, so a pass held back while something is shown loses nothing.
A broken prop leaves one for its `destroyed` moment, a dead actor one for its `died`, and a watched shot from a thing its last charge spent leaves one for its `hit`; each `LandsAsItself`, so what it lands is credited to the thing or the actor it stands in for rather than to its killer.
What a remnant carries beyond its triggers is `RemnantKeeps`, a resource of the components it copies across, the `Name` by default so the log and a record of a death can still say whose, and a plugin adds to it with `app.keep_on_remnant::<C>()`, the narrator's own `NarratorPlugin` adding `Glyph` so a dead actor's or a broken prop's name keeps the colour a living one's would have.
`bury_remnants`, in `Last`, despawns a remnant once every system has read this frame's landings, so whatever names it later in the same pass, the log among them, still finds it.

## Using it

A trigger is a moment, an area and a list, and Foundry's grenades are four of them, each a burst where the grenade comes down and a noise heard from there.

<!-- include: ../../../../examples/foundry/assets/items.ron:grenades -->
```ron
    // The grenades: thrown, and what each does is its land trigger, a
    // burst where it comes to rest, which spends it there. Plate takes half
    // a frag burst off a droid; an ion burst undoes a chassis and blinds
    // every radar in it, and barely touches flesh. Smoke is thirty cells'
    // worth, more than its burst holds, so it spills past it: a room of
    // cover in the open, a long plume down a corridor. Each is heard where
    // it lands: a burst fourteen steps down open deck, smoke's hiss six.
    (name: "frag grenade", glyph: '*', color: (0.7, 0.72, 0.45), stack: true, throw: (range: 6), consumable: (charges: 1, when_empty: Destroyed),
     triggers: [(on: "land", area: Burst(radius: 1), look: (glyph: '*', color: (r: 255, g: 200, b: 90)), effects: [(kind: "Harm", args: (kind: "kinetic", roll: "3d6")), (kind: "Noise", args: (sound: "blast", loudness: 14))])]),
    (name: "smoke grenade", glyph: '*', color: (0.75, 0.75, 0.75), stack: true, throw: (range: 6), consumable: (charges: 1, when_empty: Destroyed),
     triggers: [(on: "land", area: Burst(radius: 2), look: (glyph: '*', color: (r: 200, g: 200, b: 200)), effects: [(kind: "Emit", args: (gas: "smoke", amount: 590)), (kind: "Noise", args: (sound: "hiss", loudness: 6))])]),
    (name: "ion grenade", glyph: '*', color: (0.4, 0.7, 1.0), stack: true, throw: (range: 6), consumable: (charges: 1, when_empty: Destroyed),
     triggers: [(on: "land", area: Burst(radius: 2), look: (glyph: '*', color: (r: 90, g: 170, b: 255)), effects: [(kind: "Harm", args: (kind: "ion", roll: "1d4")), (kind: "Noise", args: (sound: "blast", loudness: 14))])]),
    (name: "incendiary grenade", glyph: '*', color: (0.95, 0.45, 0.2), stack: true, throw: (range: 6), consumable: (charges: 1, when_empty: Destroyed),
     triggers: [(on: "land", area: Burst(radius: 1), look: (glyph: '*', color: (r: 255, g: 110, b: 40)), effects: [(kind: "Harm", args: (kind: "thermal", roll: "2d4")), (kind: "Ignite", args: (turns: 4)), (kind: "Noise", args: (sound: "blast", loudness: 14))])]),
```

A game builds its definitions' triggers once when it reads the file, against the moments and kinds the run registered and names with its sounds chained on, so a typo is a startup failure and not a grenade that lands nothing.

<!-- include: ../../../../examples/foundry/src/gear.rs:triggers -->
```rust,no_run
        // Each definition's parts, its triggers built once against the
        // moments and effect kinds the run registered, and every problem in
        // the file named at once: a grenade that lands nothing is a typo.
        let mut parts = Vec::new();
        let mut errors = Vec::new();
        for (id, d) in defs.iter() {
            let built = Triggers::build(&d.triggers, &d.effects, moments, kinds, &names).and_then(|triggers| parts_of(d, id, triggers, registries));
            parts.push(Arc::new(built.unwrap_or_else(|mine| {
                errors.extend(mine.into_iter().map(|e| format!("{}: {e}", d.name)));
                ItemParts::default()
            })));
        }
        assert!(errors.is_empty(), "assets/items.ron: {}", errors.join("; "));
```

## The line

The engine decides what an effect does to the world, when a moment's triggers land, over which cells and with what dice; what a thing is, and which moments it answers, is the game's content.
A subsystem that owns a moment reports it and does nothing else, so "find the list, build the area, land it" is written once and a game's own moment lands exactly as an engine one does.
Moments are an open registry and not an enum, for the reason every other name in the engine is: a blaster that does something when it overheats is a line of the game's, not a fork.
One list can be delivered several ways, which is the whole of the potion model: what a bottle holds is written once as the item's `effects`, and a `use` trigger and a `land` trigger each deliver it to different people.
A thing's use may be aimed, and it is aimed by an ability's rule rather than one of its own: its `targeting` is the `Targeting` an ability's mode, aim and sight make, landed by the same `Bystanders::land`, so a bolt from the pack and a bolt an actor knows reach the same cells and catch the same people, while the list it lands is still the thing's.
A use made on a thing carries the thing to its effects and no further, since the engine's effects land on `targets` and never read `Landing::on`: what makes a thing better or known is the game's, written as an effect of its own.
A burst stops at walls and not at whoever stands in it, and it is the call an ability's `Ball` bursts with, so a grenade and a fireball of one radius reach the same cells and neither reaches round a wall.
A trap lands in the pass it was stepped on, and a prop's or an actor's `destroyed` or `died` trigger a pass after the blow, since a death is known only after damage; an actor's `struck` lands before that, in the same pass as the blow, so a killing blow sets off `struck` and then, the pass after, `died`.
Whether a self-aimed ability or use would act on anything at all is the engine's own question to ask and the effect's to answer: a game's effect that needs something there to act on says so through `would_act`, and one that needs nothing is kept by every tactic that asks, as it always was.
A trap's harm is the trap's own and not the doing of whoever stepped on it, so the log never says they hurt themselves; who stepped on it is still on the report.
Charges are not here: a thing with triggers and no `Consumable` costs nothing, and `spend_charges` reads `Fired` after `land_triggers` so what the last charge did lands before the thing is gone.
The engine carries a bonus on the carrier and never learns what a level is: the game writes the `EffectBonus` with the level applied when it spawns the thing, and an effect adds it without asking where it came from, so an enchant, a blessing and a curse are one number to the engine and three rules to the game.
Only what changes in play is saved, a trigger's fires left beside a consumable's charges, and the lists are rebuilt from the definitions.
A carrier's `EffectBonus` is not saved either: the game's spawn writes it again from whatever the game saved, a level, the way it wrote it the first time.

## Where it lives

`rl-rules` is tier 1 and has no Bevy in it: `EffectSpec` and `TriggerSpec` are the authored forms any game's own file can deserialize, so a content file is checked for shape without an `App`.
`rl-bevy` is tier 2 and holds the rest in one module, because an effect needs the world to land: the machinery and the plugin, the engine's ten effects beside it, and the triggers with the moments they answer.
The effects sit in their own module rather than each in the subsystem it asks, so combat, statuses, fire, gas and hearing never depend on the code that asks them, and abilities, props and items depend on effects rather than on each other.
That split is what let a prop's trap stop reaching into abilities to land anything, and what lets a triggers test build a floor, report a moment by hand and read the damage without an ability, a prop or an item anywhere.
