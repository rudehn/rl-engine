<!-- documents:
     plugins: AbilitiesPlugin, ThrowingPlugin
     files: crates/atglyph-rules/src/ability.rs
            crates/atglyph-grid/src/targeting.rs
            crates/atglyph-rules/src/ai/snapshot.rs
            crates/atglyph-rules/src/ai/tactics.rs
            crates/atglyph-bevy/src/ability.rs
            crates/atglyph-bevy/src/effects/mod.rs
            crates/atglyph-bevy/src/effects/engine.rs
            crates/atglyph-bevy/src/throwing.rs
            crates/atglyph-bevy/src/combat.rs
            crates/atglyph-bevy/src/parts.rs
            crates/atglyph-rules/src/parts.rs
            crates/atglyph-bevy/src/cue.rs
            crates/atglyph-bevy/src/items.rs
            crates/atglyph-bevy/src/props.rs
            crates/atglyph-bevy/src/fire.rs
            crates/atglyph-bevy/src/gas.rs
            crates/atglyph-bevy/src/noise.rs
            crates/atglyph-ui/src/view/target.rs
            crates/atglyph-ui/src/view/inventory.rs
            crates/atglyph-save/src/engine.rs
     fingerprint: edd05b68 -->

# Abilities

An ability is the second thing an actor can spend a turn on, and the first is a blow.
It is that same sentence with every part named by data: a shape, what it wants under that shape, what it spends, what must be true of whoever uses it, what the turn costs, how long before it comes round again, and what lands.
A game writes them in a file and compiles nothing to add one.
What an ability *does* is a Rust type, because something has to know that a damage roll becomes a `DamageEvent` per actor in a footprint; everything else about it is a line of RON.

## Turning it on

`AbilitiesPlugin` declares `needs::<Abilities>`, hinting that one comes from `Abilities::load(ron, &EffectKinds, &names)`, and `needs::<Registries>` for the stats its costs and requirements name.
In `finish` it declares `depends_on::<CombatPlugin>`, since an ability's damage goes down the pipeline a sword's does.
It adds `EffectsPlugin` if the game has not, whose `EffectRng` is the stream every effect in the engine is rolled from whatever landed it, so writing one more ability cannot shift the combat stream and change every monster's rolls in a run that was going fine.
It adds `offer_abilities` in `DecideSet::Offer`, `perceive_abilities` in `PerceiveSet::Annotate`, `weigh_self_abilities` in `PerceiveSet::Weigh`, `refresh_known` in `TurnSet::React`, and `land_abilities` in `LandSet::Ability` chained ahead of `resolve_abilities` in `ResolveSet::Act`, because what is already in the air comes down before anything else is loosed.
`EffectsPlugin` registers `ApplyStatus`, `RemoveStatus`, `DamageEvent` and `Heal`: `EffectWorld` writes all four, a writer for an unregistered message fails its system at startup, and a game with abilities should not have to add the status and combat plugins to find that out.
Every `Actor` is given an empty `Known`, `Pools` and `Cooldowns` as it is spawned, so an actor carrying nothing but `Grants` can use what it was granted.
The engine's own effects are registered separately, by `add_engine_effects()` for the seven that need no subsystem and by `FirePlugin`, `GasPlugin` and `NoisePlugin` for the three that do, so an ability file naming `Ignite`, `Emit` or `Noise` loads exactly when the game has fire, gas or hearing.
`ThrowingPlugin` is its own opt-in and depends on items and combat both, because a throw is an item leaving a bag and a blow down the damage pipeline, and neither of those plugins has to know the other exists; it registers `Throw`, the `Fired` a landing reports and `Launched`.

## The model

`AbilityDef` is what an ability file says: a `name`, a `description` for a menu, an optional `ParticleLook` of glyph and colour, an `Aim`, a `TargetMode` with its range inside it, `sight`, `requires`, `costs`, `time`, `cooldown` and `effects`.
`AbilityDef::targeting` gathers the mode, the aim and `sight` into a `Targeting`, the form a thing's aimed use is written in, `(mode: Bolt(range: 7), aim: Ground, sight: false)`, under the same three words, so a bolt an actor knows and a bolt sent from the pack are pointed by one rule.
Every id in it is a name resolved through `Names` at load, so an ability calls a stat, a status, a tag, a slot or a damage kind by the name the rest of the content does, and a typo is a startup failure listing every one it found.
`Aim` is `SelfOnly`, `Foe`, `Ally`, `Ground` or `Anyone`, closed because it enumerates the questions the faction matrix can answer about a cell rather than a taxonomy of content.
It is what lets a mind fire an ability it cannot understand, through five predicates: `Aim::wants` says whether a cell holds the kind of thing it is after and the rest are built on it, `hits` says who a footprint catches, `worth_aiming_at` who is worth pointing it at, `cycles_to` who a player's cursor stops on, and `needs_cursor` whether there is anywhere to point at all.
The user is its own ally whatever the matrix says, so a spray aimed at allies mends whoever sprays it and a burst on the ground burns whoever stands in it, while a foe-aimed shape never catches its user.
`Cost` is `Pool`, `Health` or `Item`, all or nothing, and closed for the same reason: each arm is something the engine already knows how to decrement.
A game that wants a sixth kind of fuel registers a stat and spends it with `Pool`, which is what mana, stamina, power, nerve, heat and powder all turn out to be.
`Requirement` is `Has`, `Lacks`, `Wielding`, `InSlot` or `Above`, each reading a table the engine owns, which is how a shield bash learns it needs a shield without the engine learning the word.
`blocked(def, &Gates, &Purse, now, ready_at)` returns every reason at once as a `Vec<Blocked>` rather than the first, so a row greyed in a menu says all of what is wrong with it, and `Gates` and `Purse` are borrowed views the caller fills from its own components.
A cooldown is an absolute time on the turn queue's clock rather than a countdown, so a save that restores the clock restores every cooldown with it and nothing has to be ticked.
`Use { ability, aim }` is an `Action` like any other, aimed at a cell because most shapes land on ground; one whose aim needs no cursor is aimed at the user's own feet.
`resolve_abilities` lands the aim first and then gates on the union of what the user cannot do and what the aim refuses, so one refusal carries both; then it pays, sets the cooldown and hands the ability's `Effects` the `Landing` to run over, and reports `AbilityEvent::Used` or `Refused`.
A refusal costs the player nothing and keeps the turn, and costs anyone else the turn, which is what stops a monster retrying forever what it cannot pay for.
A `Ball` flies as a bolt and bursts where it lands with `atglyph::grid::burst`, which stops at walls and not at whoever stands in the way, and bursts on the near side of a wall it flew into.
`Bystanders::land` is the one answer to where a use goes, handed an `Aimed`: who, a `Source`, a `Targeting`, from where and at what, which `Aimed::ability` builds for an ability and the effects subsystem builds for a thing's aimed use, so nothing past it can tell the two apart.
A body carrying `Reflects { percent }` turns an aimed use or an ability back on whoever aimed it at that chance, 50 by default, rolled on `ReflectRng`, a stream of its own.
`land` never reflects: `Bystanders::reflect(landing, targeting, map)` is a second step the two resolvers take once an aim is no longer refused, and the cursor's preview never does, so it spares the reflecting body and lands its share on the user instead, writing a `Reflected { by, onto, source }` for each one turned.
Only its own share turns: the rest of a burst still lands where it was aimed, on whoever else it caught, and the ground it covers stays there; when the turned-back share leaves nobody standing where it was aimed, as a bolt at one body does, the whole use lands on its user instead, its cells the shape `mode` draws on the user's own cell, a bolt lighting the user's cell and a ball bursting round them, from wherever the projectile stopped.
`aim_blocked(targeting, cells, sees_aim)` is the other half of the gate: whether an ability may be used at all against whether it may be used *here*.
The targeting cursor previews through that same call, so the cells it paints are the cells that will be hit, and a projectile stopped short of where it was pointed is `Blocked::OutOfReach` rather than a burst on a spot nobody chose.
`Landing` is the result: the user, what landed it, the origin, the aim, every cell covered, the flight path, where a projectile stopped, everyone under it the aim wanted there, a `bonus`, the default `EffectBonus` that adds nothing for an ability, which only a thing's own carries more than, as [Effects](effects.md) says, the thing a use was made `on`, and every body that `turned_by` its share.
Its `source` says what landed it, `Source::Ability`, `Trigger` or `Offer`, so an effect can tell a spell from a trap without assuming either, and the harm it lands carries it as its `DamageRoute`, so a narrator says whose bolt it was.
An ability with a look, and a thing's aimed use with one, is seen to fly to where its projectile stopped and bursts over its footprint as a blast going out from there, or from its user when it had none, so a ball goes off where it lands and a nova, a cone or a line goes out from whoever cast it.
An `Effect` is a type with `apply(&self, &Landing, &mut EffectWorld)` and a `describe(&self, &Registries, EffectBonus)` a menu reads with that bonus added, and `FromArgs` is its constructor, kept separate so the trait a game writes stays object-safe.
`EffectWorld` asks for what another subsystem owns rather than doing it: damage, a heal, a status on, a status off, along with the effect stream, cues, and `Commands` for whatever the engine never thought of.
Asking is what keeps a fireball mitigated by the same armor a sword is, and moving an actor is the one exception, since no other subsystem owns it: `position`, `is_free`, `is_transition`, `place` and `slide` are methods on it, and `slide` is what keeps a shove out of a wall.
`app.add_effect::<E>()` files `E` under its `KIND` in `EffectKinds`, and an `EffectSpec` is the `(kind, chance, args)` every content file that lands effects is read for, its arguments left as text for whoever registered the kind and a chance above 100 refused at load as the typo it is wherever it appears.
`Effects` is a list of those built, and the one thing three carriers share: an ability an actor knows, an offer a prop makes, and the triggers a prop or an item carries, which [Effects](effects.md) describes.
`Effects::build` builds every spec or reports every one that would not build, and `Abilities::build` runs it once per ability, keeps the results as a `Vec<Effects>` parallel to the ids and puts the ability's name in front of each failure.
`Effects::land` rolls each entry against its own chance from `EffectRng` before applying it, so a trap and a stim are dealt from the same deck a spell is, and `Effects::describe` is the fold a menu prints: one line per effect that has something to say, with its chance in front when it is not certain; `Abilities::describe` reads it with the default bonus, since an ability's own numbers are not a thing's to grow.
The effects module holds `Effects` and ten effects: `add_engine_effects()` registers the seven that need no subsystem, `Harm`, `Mend`, `Inflict`, `Cleanse`, `Shove`, `Pull` and `Teleport`, and `Ignite`, `Emit` and `Noise` sit beside them to be registered by fire, gas and hearing instead; all ten live in that one module rather than each in the module it asks, so effects depend on combat, statuses, fire, gas and noise and none of the five depends back.
`Known` is the set of abilities an actor knows, rebuilt every `TurnSet::React` from its own `Grants` and nothing else: a thing in the bag never lends an ability, because what an item does is its own triggers.
`UsableAbilities` is the turn-holder's abilities sorted into usable and refused once a pass by the gate the resolver uses, read through `why_for`, which answers only for the actor it was worked out for.
`perceive_abilities` copies `usable` into `Snapshot::usable` as `Usable { ability, aim, mode }`, everything the `UseAbility` tactic needs to score a footprint and nothing about what the ability does.
Throwing is the smaller half: `Throwable::new(range, strike)` is an item made to be thrown, with an optional `effective` range a hit model reads, `Throw { item, at }` its action, and `flight` the one answer to where it goes, shared with the cursor that previews it.
Its `range` is an `Option`: a thing that names one flies that far whoever throws it, and one with `None` flies as far as its thrower reaches, `CombatRules::reach(base, stat)`, a `ThrowReach` of `base` cells plus the thrower's value of `stat` when one is named; with no reach set, a thing with no range of its own cannot be thrown at all.
How far a thing flies therefore depends on who throws it, so `Throws::of(thrower, item)` is the one answer, a `ThrowAs` of its range, its effective range, its own or a third of how far it flies, its strike and what launches it, or `None` when that thrower cannot throw it, and the throw, its odds, the aiming cursor, the pack's rows and every mind weighing a missile read it rather than the item's `Throwable`; `ThrowRules::by(thrower, item, worn)` is the same answer for a system already holding what is worn.
A `Launcher { ammo, range, damage }` on a wearable thing, spawned from a `LaunchDef` in `Attacks::launch`, launches what its wearer throws that is tagged `ammo`: the first worn launcher in slot order for one of the thing's tags adds its `range` to how far it flies and its `damage` to what it strikes for, rolled beside the thing's own roll, and in the pack it launches nothing.
A launched thing flies, strikes and lands as any throw does and lies where it came down to be picked up again, and `Launched { actor, item, launcher }` is written beside its `ItemEvent::Thrown`, so whatever tells of throws can tell a shot.
A thrown knife and a bolt stop at the same first wall or body, and each hangs in the air until whatever is watching has seen it fly.

## Using it

An ability is a line of RON naming its aim, its shape, what it costs, how long it cools and what lands, and Warren's two are a screech that bursts where it lands and a crust eaten to mend.

<!-- include: ../../../../examples/tutorial/assets/knacks.ron -->
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
        effects: [(kind: "Mend", args: (roll: "1d6"))],
    ),
]
```

A verb the engine does not ship is a type with two impls, `Effect` and `FromArgs`, and one registration line, `app.add_effect::<E>()`, after which a file names it by its `KIND` as these name `Harm` and `Mend`.

## The line

The engine decides whether an ability may be used, what it spends, where it lands and who is under it; what a verb it does not ship means is the game's, written as an effect and registered by name.
There is no enum of effect kinds and no `Custom { id }`, so a game's own effect sits beside the engine's and the resolver cannot tell them apart.
The boundary is drawn at the verb and nowhere further in: making a damage roll into data too would mean shipping an expression language, an interpreter and a debugger for it, and the interesting half of every game would be written where there are no types and no stack traces.
An ability is content and its vocabulary is code, so a fireball, a smoke bomb or a rally is a RON edit, and `Bribe` or `Hack` is one file a game writes once and then never again.
Untyped arguments are what that buys, and validating them at load is what pays for them: a bad argument fails at startup naming the ability, not the first time somebody presses the key.
An ability is something an actor knows, so an item never lends one: a medkit or a grenade costs no ability id, takes no row on the screen beside what its carrier actually knows, and says that it is used up as a fact about the item rather than as a cost of an ability.
A thing used at somewhere, a staff's bolt, is still the thing's own `use` trigger and not an ability it lends: it is written in an ability's words and landed, previewed and refused through the same `Bystanders::land` and `aim_blocked`, so the two cannot drift, while what it lands and what it spends stay the thing's.
A thing not yet known is landed through them too, but neither previewed nor refused by its targeting, since either would tell what it is, as [Uses](uses.md) says.
A key aims nothing itself; it writes `AimAt`, or `AimUse` for a thing, and stops, and whether a cursor opens, where it opens and what the use costs are the engine's, which is why a game's input never learns what a broadside does.
A mind is handed only what the gate already allowed, which is why it cannot loop on something it cannot afford, and it scores by `Aim` alone, so a monster given a new ability needs no new tactic.
A self-aimed one is weighed once more before that, by `weigh_self_abilities`, which keeps only those `Effect::would_act` says would act on something landed now, a mend kept for an actor that is hurt and a mend with nobody hurt left out, so a mind never spends its turn casting one that does nothing.
How far a throw flies and what a worn launcher adds are the engine's to work out, once, for the throw and everything that forecasts it; what each thing's range is, or that it has none, what a thrower's reach is built from, and which tag a launcher is for are the game's content.
An ability is never rolled to hit: it lands unconditionally, where a blow and a shot are rolled by `HitRules`, as [Combat](combat.md) says.
Ability trees, schools, levelling, spell failure and casting interrupted by a blow are each a game's rule over this data, and the engine should not guess which of them a game wants.
Saving is the engine's: `EngineSave` writes each actor's pools and cooldowns, so a game that saves keeps a cooling ability cooling without writing a line for it.
What an ability was called on for is never asked; a use is an `AbilityEvent`, and a game reads one and counts whatever its run is about.

## Where it lives

`atglyph-rules` is tier 1 and has no Bevy in it: `ability.rs` decides and never acts, answering whether a use is permitted over borrowed views of the user, so the whole gate is tested against a `Gates` and a `Purse` filled by hand with no `App` anywhere.
`Aim`'s five predicates live there too, which is what lets the resolver, the cursor's preview and the scoring in `tactics.rs` share one rule rather than drifting four ways apart.
`atglyph-bevy` is tier 2 and owns everything that touches the world: `ability.rs` is the action, the resolver and the state a use spends, and the effects module is the seam every effect is registered through, `Effects`, and the ten the engine ships through that seam.
The seam sits there rather than in `ability.rs` because an ability is not the only thing that lands a list: what an ability, a prop and an item share is how a list is built, rolled and described, never when it lands or on whom, and that much was written three times before it was written once.
The effects sit in a module of their own rather than each in the subsystem it asks, because `Harm` in `combat.rs` would make combat depend on abilities to implement a trait, and the dependency is meant to run the other way.
`throwing.rs` is beside them rather than inside items or combat, for that same reason in two directions at once.
`atglyph-ui` owns the aiming and the menu and `atglyph-save` the save kind, so nothing below either has to know they exist.
