<!-- documents:
     plugins: AbilitiesPlugin, ThrowingPlugin
     files: crates/rl-rules/src/ability.rs
            crates/rl-grid/src/targeting.rs
            crates/rl-rules/src/ai/snapshot.rs
            crates/rl-rules/src/ai/tactics.rs
            crates/rl-bevy/src/ability.rs
            crates/rl-bevy/src/effects/mod.rs
            crates/rl-bevy/src/effects/engine.rs
            crates/rl-bevy/src/throwing.rs
            crates/rl-bevy/src/cue.rs
            crates/rl-bevy/src/items.rs
            crates/rl-bevy/src/props.rs
            crates/rl-bevy/src/fire.rs
            crates/rl-bevy/src/gas.rs
            crates/rl-ui/src/view/target.rs
            crates/rl-save/src/engine.rs
     fingerprint: 78e99243 -->

# Abilities

An ability is the second thing an actor can spend a turn on, and the first is a blow.
It is that same sentence with every part named by data: a shape, what it wants under that shape, what it spends, what must be true of whoever uses it, what the turn costs, how long before it comes round again, and what lands.
A game writes them in a file and compiles nothing to add one.
What an ability *does* is a Rust type, because something has to know that a damage roll becomes a `DamageEvent` per actor in a footprint; everything else about it is a line of RON.

## Turning it on

`AbilitiesPlugin` declares `needs::<Abilities>`, hinting that one comes from `Abilities::load(ron, &EffectKinds, &names)`, and `needs::<Registries>` for the stats its costs and requirements name.
In `finish` it declares `depends_on::<CombatPlugin>`, since an ability's damage goes down the pipeline a sword's does.
It adds `EffectsPlugin` if the game has not, whose `EffectRng` is the stream every effect in the engine is rolled from whatever landed it, so writing one more ability cannot shift the combat stream and change every monster's rolls in a run that was going fine.
It adds `offer_abilities` in `DecideSet::Offer`, `perceive_abilities` in `PerceiveSet::Annotate`, `refresh_known` in `TurnSet::React`, and `land_abilities` in `LandSet::Ability` chained ahead of `resolve_abilities` in `ResolveSet::Act`, because what is already in the air comes down before anything else is loosed.
`EffectsPlugin` registers `Afflict`, `Cure` and `DamageEvent`: `EffectWorld` writes all three, a writer for an unregistered message fails its system at startup, and a game with abilities should not have to add the status and combat plugins to find that out.
Every `Actor` is given an empty `Known`, `Pools` and `Cooldowns` as it is spawned, so an actor carrying nothing but `Grants` can use what it was granted.
The engine's own effects are registered separately, by `add_engine_effects()` for the seven that need no subsystem and by `FirePlugin` and `GasPlugin` for the two that do, so an ability file naming `Ignite` or `Emit` loads exactly when the game has fire or gas.
`ThrowingPlugin` is its own opt-in and depends on items and combat both, because a throw is an item leaving a bag and a blow down the damage pipeline, and neither of those plugins has to know the other exists.

## The model

`AbilityDef` is what an ability file says: a `name`, a `description` for a menu, an optional `Look` of glyph and colour, an `Aim`, a `TargetMode` with its range inside it, `sight`, `requires`, `costs`, `time`, `cooldown` and `effects`.
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
A `Ball` flies as a bolt and bursts where it lands with `rl_grid::burst`, which stops at walls and not at whoever stands in the way, and bursts on the near side of a wall it flew into.
`Bystanders::land` is the one answer to where a use goes, and `aim_blocked` the other half of the gate: whether an ability may be used at all against whether it may be used *here*.
The targeting cursor previews through that same call, so the cells it paints are the cells that will be hit, and a projectile stopped short of where it was pointed is `Blocked::OutOfReach` rather than a burst on a spot nobody chose.
`Landing` is the result: the user, the ability, the origin, the aim, every cell covered, the flight path, where a projectile stopped, and everyone under it the aim wanted there.
Its `source` says what landed it, `Source::Ability`, `Trigger` or `Offer`, so an effect can tell a spell from a trap without assuming either, and only an ability has a look to fly.
An `Effect` is a type with `apply(&self, &Landing, &mut EffectWorld)` and a `describe` a menu reads, and `FromArgs` is its constructor, kept separate so the trait a game writes stays object-safe.
`EffectWorld` asks for what another subsystem owns rather than doing it: damage, a status on, a status off, along with the effect stream, cues, and `Commands` for whatever the engine never thought of.
Asking is what keeps a fireball mitigated by the same armor a sword is, and moving an actor is the one exception, since no other subsystem owns it: `position`, `sight_of`, `is_free`, `place` and `slide` are methods on it, and `slide` is what keeps a shove out of a wall.
`app.add_effect::<E>()` files `E` under its `KIND` in `EffectKinds`, and an `EffectSpec` is the `(kind, chance, args)` every content file that lands effects is read for, its arguments left as text for whoever registered the kind and a chance above 100 refused at load as the typo it is wherever it appears.
`Effects` is a list of those built, and the one thing three carriers share: an ability an actor knows, an offer a prop makes, and the triggers a prop or an item carries, which [Effects](effects.md) describes.
`Effects::build` builds every spec or reports every one that would not build, and `Abilities::build` runs it once per ability, keeps the results as a `Vec<Effects>` parallel to the ids and puts the ability's name in front of each failure.
`Effects::land` rolls each entry against its own chance from `EffectRng` before applying it, so a trap and a stim are dealt from the same deck a spell is, and `Effects::describe` is the fold a menu prints: one line per effect that has something to say, with its chance in front when it is not certain.
The effects module holds `Effects` and nine effects: `add_engine_effects()` registers the seven that need no subsystem, `Harm`, `Mend`, `Inflict`, `Cleanse`, `Shove`, `Pull` and `Teleport`, and `Ignite` and `Emit` sit beside them to be registered by fire and gas instead; all nine live in that one module rather than each in the module it asks, so effects depend on combat, statuses, fire and gas and none of the four depends back.
`Known` is the set of abilities an actor knows, rebuilt every `TurnSet::React` from its own `Grants` and nothing else: a thing in the bag never lends an ability, because what an item does is its own triggers.
`Offered` is the turn-holder's abilities sorted into `usable` and `refused` once a pass by the gate the resolver uses, read through `usable_by` and `why_for`, which answer only for the actor it was worked out for.
`perceive_abilities` copies `usable` into `Snapshot::usable` as `Usable { ability, aim, mode }`, everything the `UseAbility` tactic needs to score a footprint and nothing about what the ability does.
Throwing is the smaller half: `Throwable { range, strike }` is an item made to be thrown, `Throw { item, at }` its action, and `flight` the one answer to where it goes, shared with the cursor that previews it.
A thrown knife and a bolt stop at the same first wall or body, and each hangs in the air until whatever is watching has seen it fly.

## Using it

A verb the engine does not ship is a type with two impls and one registration line, which is Corsair's `Plunder`.

<!-- include: ../../../../examples/corsair/src/abilities.rs:plunder -->
```rust,no_run
/// Shake a foe down: whatever is in its purse spills onto the ground at its
/// feet, to be picked up like any other loot.
///
/// A purse is Corsair's, not the engine's, so no engine effect could reach
/// it; this one reaches it through `commands`, which is the whole of the
/// escape hatch. It spills rather than pockets because a stack on the ground
/// merges into the bag through the engine's own pick-up, and an effect that
/// merged stacks itself would be a second copy of that rule.
#[derive(Debug, Clone, Copy, Default)]
pub struct Plunder;

impl Effect for Plunder {
    fn describe(&self, _: &Registries) -> String {
        "spills its purse at its feet".to_string()
    }

    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        for target in landing.targets.clone() {
            world.commands.queue(move |w: &mut World| {
                let Some(coin) = w.get::<Purse>(target).map(|p| p.0).filter(|n| *n > 0) else { return };
                let Some(at) = w.get::<Position>(target).map(|p| p.0) else { return };
                w.entity_mut(target).insert(Purse(0));
                w.resource_scope(|w: &mut World, armory: Mut<Armory>| {
                    let mut queue = bevy::ecs::world::CommandQueue::default();
                    let mut commands = Commands::new(&mut queue, w);
                    armory.spawn(&mut commands, armory.defs.expect("doubloon"), coin, Some(at));
                    queue.apply(w);
                });
            });
        }
    }
}

impl FromArgs for Plunder {
    const KIND: &'static str = "Plunder";

    fn from_args(_: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        Ok(Plunder)
    }
}
```

## The line

The engine decides whether an ability may be used, what it spends, where it lands and who is under it; what a verb it does not ship means is the game's, written as an effect and registered by name.
There is no enum of effect kinds and no `Custom { id }`, so a game's own effect sits beside the engine's and the resolver cannot tell them apart.
The boundary is drawn at the verb and nowhere further in: making a damage roll into data too would mean shipping an expression language, an interpreter and a debugger for it, and the interesting half of every game would be written where there are no types and no stack traces.
An ability is content and its vocabulary is code, so a fireball, a smoke bomb or a rally is a RON edit, and `Bribe` or `Hack` is one file a game writes once and then never again.
Untyped arguments are what that buys, and validating them at load is what pays for them: a bad argument fails at startup naming the ability, not the first time somebody presses the key.
An ability is something an actor knows, so an item never lends one: a medkit or a grenade costs no ability id, takes no row on the screen beside what its carrier actually knows, and says that it is used up as a fact about the item rather than as a cost of an ability.
An item is aimed only by being thrown or fired, which throwing and combat already own, and there is no third kind of aimed use for abilities to take over.
A key aims nothing itself; it writes `AimAt` and stops, and whether a cursor opens, where it opens and what the use costs are the engine's, which is why a game's input never learns what a broadside does.
A mind is handed only what the gate already allowed, which is why it cannot loop on something it cannot afford, and it scores by `Aim` alone, so a monster given a new ability needs no new tactic.
Accuracy does not exist: an ability lands unconditionally, as every melee blow does, and a to-hit roll when it comes is a stage in the damage pipeline rather than a change here.
Ability trees, schools, levelling, spell failure and casting interrupted by a blow are each a game's rule over this data, and the engine should not guess which of them a game wants.
Saving is the engine's: `EngineSave` writes each actor's pools and cooldowns, so a game that saves keeps a cooling ability cooling without writing a line for it.
What an ability was called on for is never asked; a use is an `AbilityEvent`, and a game reads one and counts whatever its run is about.

## Where it lives

`rl-rules` is tier 1 and has no Bevy in it: `ability.rs` decides and never acts, answering whether a use is permitted over borrowed views of the user, so the whole gate is tested against a `Gates` and a `Purse` filled by hand with no `App` anywhere.
`Aim`'s five predicates live there too, which is what lets the resolver, the cursor's preview and the scoring in `tactics.rs` share one rule rather than drifting four ways apart.
`rl-bevy` is tier 2 and owns everything that touches the world: `ability.rs` is the action, the resolver and the state a use spends, and the effects module is the seam every effect is registered through, `Effects`, and the nine the engine ships through that seam.
The seam sits there rather than in `ability.rs` because an ability is not the only thing that lands a list: what an ability, a prop and an item share is how a list is built, rolled and described, never when it lands or on whom, and that much was written three times before it was written once.
The effects sit in a module of their own rather than each in the subsystem it asks, because `Harm` in `combat.rs` would make combat depend on abilities to implement a trait, and the dependency is meant to run the other way.
`throwing.rs` is beside them rather than inside items or combat, for that same reason in two directions at once.
`rl-ui` owns the aiming and the menu and `rl-save` the save kind, so nothing below either has to know they exist.
