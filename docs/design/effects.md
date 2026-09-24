# Effects: what lands, and what sets it off

Status: built 2026-09-23, against `main` at `60c430d`.
The reasoning is here; `docs/OVERVIEW.md` lists what exists, and `docs/design/items.md` says which carrier a thing should use.

## 0. Summary

An effect is one thing that happens to a cell or an actor: harm, a mend, a status, a shove, a fire, a cloud of gas.
Three kinds of thing land effects: an ability an actor knows, a prop in the world, and an item in a bag.
Until this slice the machinery lived in `ability.rs`, so props and items reached into abilities to land anything, and each plugin registered the ability stream for itself.

`EffectsPlugin` is now the subsystem, and abilities, props and items use it as peers.
It owns four things:

- the `Effect` trait, `EffectKinds` and `AddEffect`, which is how a game registers an effect of its own;
- `Landing`, `EffectWorld` and `Source`, which is what an effect sees when it lands and what landed it;
- `EffectRng`, the one stream every effect rolls from;
- `Moments`, `Triggers` and `Fired`, which is how a prop or an item says what it does at a moment, and `land_triggers`, which lands it.

It does not own charges.
What a use costs a thing is `ConsumablesPlugin`'s, and it reads the same messages.

## 1. One stream, and why it keeps its old seed

Every effect rolls from `EffectRng`, whoever landed it: a trap, a stim and a spell are dealt from one deck.
A subsystem that rolled from a stream of its own would shift another's dice by the order passes happen to run in, which is the bug `docs/design/lighting.md` section 4 and the seeding rules in `CLAUDE.md` exist to prevent.

The stream was `AbilityRng`, and it keeps that stream's derivation domain, `b"ability"`.
Renaming the domain would have moved every roll of every run for no gain, and `effects::tests` pins the first draw for seed seven as a tripwire so a change that moves it is a change someone meant.

## 2. Moments are a registry, not an enum

A moment is when a thing does what it does: `use`, `land`, `fire`, `hit`, `entered`, `destroyed`.
`Moments` is an `Interner<Moment>`, the same shape as props' `Verbs`, with the engine's six interned first so their ids are constants on the type.
A game adds its own with `app.add_moment("overheat")` and reports it from a system of its own.

A closed enum would be the `#[non_exhaustive]` taxonomy `CLAUDE.md` rules out.
The first game that wants a moment the engine did not think of, a blaster that does something when it overheats, would have to fork the enum or fake the moment through one that exists.

A content file names a moment as a string.
It is resolved when the triggers are built, and a name nobody registered fails that build with the name in the message, the way an unregistered effect kind does, so a typo is caught while the file is read rather than by a trap that quietly never fires.

## 3. `Triggers`, and the potion model

`TriggerSpec` is the authored form, in `rl-rules` beside `EffectSpec` so any game's file can read it: a moment by name, an `Area`, an optional count of `fires`, and an optional effect list.
`Triggers::build(specs, shared, moments, kinds, names)` turns a definition's specs into the `Triggers` component once, and every copy of that item or prop shares each list through an `Arc`.
`fires` is per entity: the list is shared, the count is not, so springing one cable does not spend another.

A trigger with no list of its own lands the definition's shared `effects`.
That is the whole of the potion model.
In a fantasy game a potion drunk and a potion thrown do the same thing to different people, so what the bottle holds is written once and each trigger is a way of delivering it:

```ron
(name: "healing draught", stack: true, throw: (range: 5),
 consumable: (charges: 1, when_empty: Destroyed),
 effects: [(kind: "Mend", args: (kind: "care", roll: "2d6"))],
 triggers: [(on: "use"), (on: "land", area: Burst(radius: 1))]),
```

A thing that does two different things writes two lists.
A trigger with neither a list nor a shared one fails the build, because a trigger that does nothing is a typo.

`Area` is `Here`, the one cell the moment happened at, or `Burst { radius }`, the cells within the radius that a straight line from the centre reaches without crossing a wall.
Walls stop a burst and nobody standing does: a burst catches whoever is in the way rather than stopping on them, which is the difference between it and a projectile.
It is the same call, `rl_grid::burst`, that an ability's `Ball` bursts with once it lands, so a grenade and a fireball of one radius reach the same cells, and a ball that flies into a wall bursts on the near side of it.
The first build left bursts as plain discs, reaching through walls because a ball did; walls were chosen for both on 2026-09-23, since a grenade that hurt a droid behind a wall read as a bug to anyone playing.

## 4. Reporting a moment, and landing it

A subsystem that owns a moment writes `Fired { on, moment, by, at }` and does nothing else.
It does not find the triggers, build the footprint or land anything, so the rule is written once.

| Moment | Reported by | Where | Lands |
| --- | --- | --- | --- |
| `use` | the items resolver, on an accepted `UseItem` | `ResolveSet::Act` | the same pass |
| `land` | throwing, where a throw comes to rest | `LandSet::Throw` | the same pass |
| `fire` | combat, for each attack made with a worn item | `ResolveSet::Act` | the same pass |
| `hit` | combat, at the struck actor's cell | `ResolveSet::Act`, `LandSet::Shot` | the same pass |
| `entered` | props, on a step onto an armed prop's cell | `ResolveSet::Travel`, after every step, swap and warp | the same pass |
| `destroyed` | props, on the prop's death | `TurnSet::React` | the next pass |

`land_triggers` runs in `ResolveSet::Triggers`, a stage of its own between `Act` and `Fields`.
It reads each `Fired` in the order written, takes the entity's triggers for that moment in list order, skips any whose `fires` is spent, lands the effects on the area with every actor in the area as a target, the one who set it off included, and counts `fires` down.
A trigger with a `look` is cued as a burst over its cells first, the way an ability with a look is; one with none shows nothing, which is what a hidden trap wants.

The stage is new, and the spec that preceded the code put the landing in `ResolveSet::Effects` instead.
Between `Act` and `Fields` is better on both sides: after every action that reports a moment, and before fire, gas, statuses and damage, so an incendiary grenade's fire spreads and a stim's mend is applied in the pass that set them off.
In `Effects` a grenade's fire would have waited a pass to spread.

Who the landing's user is decides who a hit is credited to, and it is not always whoever set the moment off.
A stim is the drinker's doing, a grenade the thrower's and a wand's hit the shooter's, so for those the user is `by`.
A trap is not the doing of whoever stepped on it: credited to them, the log would say they hurt themselves, and a droid a plate killed would have killed itself.
So a carrier marked `LandsAsItself`, which every armed prop is, lands as itself, and `by` stays on `Fired` for a game that wants to know who stepped on it.

A prop that is destroyed is despawned at the end of the frame it died in, which can be before its `destroyed` trigger lands.
So `report_destroyed` puts the prop's triggers and its name on a `Remnant`, which carries the moment, who broke it and where as data, and `report_remnants` reports it in the next pass's `Triggers` stage, ahead of `land_triggers`, which despawns it once it has landed.
The moment is on the entity rather than in a message because the next pass may be a long time coming: the loop holds while something is shown, and a message nobody reads for two frames is gone, where an entity stays until a new run clears it.
A watched shot from a thing its last charge spent uses the same carrier: the shot takes the thing's triggers as it is fired, and if the thing is gone when the shot lands, its `hit` lands from a remnant in its place, as the shooter's doing.

Two timings moved against the old prop triggers, which both landed in `TurnSet::React`:

- **A trap lands a pass sooner.** Its damage is applied in the pass the step was taken.
- **A `destroyed` trigger lands a pass later**, since a death is known only after `Damage`. Damage it deals is applied in the same pass as before; a status or a fire it starts begins a pass later.

## 5. `Consumable`, and who spends it

`Consumable` counts charges: `left` of `max`, what happens at zero, and an optional `Recharge` on the integer clock.
It was a marker once, beside a separate `Charges` for a thing that counted its own, and an ability's `Cost::Charge` that spent the item which lent it; all three are gone.

Which moments spend is a property of the moment, kept in `SpendingMoments`: the engine marks `use`, `land` and `fire`, and a game may mark its own.
`hit` does not spend, since one shot can strike and a flaming blade is not used up by landing a blow.

`spend_charges` reads the same `Fired` messages, in the same stage, after `land_triggers`, so what the last charge did has landed before the thing is gone.
A thing with no triggers for that moment still spends, which is how a plain wand's ordinary shot costs a charge.
At zero a stack of more than one loses a unit and the next starts full; otherwise `Destroyed` despawns the thing and `Kept` leaves it empty.
An empty thing does nothing it is spent by: `Loadout` stops finding its attack, and a use of it is refused and costs no turn.

`EffectsPlugin` knows nothing about charges, and a game with triggers and no charges does not add `ConsumablesPlugin`.

## 6. The dependency, made explicit

`AbilitiesPlugin`, `PropsPlugin` and `ConsumablesPlugin` each need `EffectsPlugin`, and each adds it when the game has not, through `effects::ensure`.
The plugin is not unique and builds once, so a game that adds it by hand as well, before or after, gets one copy and no error.
That is a departure from the usual `depends_on`, which only reports: a game that adds props should not have to learn that effects are a separate plugin before its first trap works.
`add_engine_effects` does not add the plugin, because it may be called after the app is finished, where adding a plugin panics; it registers the kinds, and whichever plugin lands them brings the rest.

## 7. What is saved

Only what changes in play.
`EngineSave` keeps a consumable's `left` and recharge progress, and each trigger's remaining `fires`, beside the pools and cooldowns it already kept.
A prop's firings stay with the prop's own save, as its old count did, so each fact has one owner.
Effect lists are never saved: a game rebuilds them from its definitions when it spawns the thing.

A prop restored before `PropEffects` is built carries its saved firings as `PendingFires`, which `arm_props` applies when it arms the prop, since a load can come before the first frame builds the triggers.

## 8. The roads not taken

- **A. One component per moment**, `OnUse`, `OnLand`, `OnHit`, each landed by the plugin that owns the moment. Typed and plain, but "find the list, build the area, land it, spend a charge" would be written once per moment and again for props, and a game could not add a moment without another component and another system.
- **C. Items as offers**, the shape props' `open` uses. It brings a verb, a time cost and refusal reasons in words, but only for a use: a landing or a hit is not an act anyone chooses, so every item file would be offers plus A's fields. The refusal reasons are the part worth taking later, on the `use` trigger.

Out of scope, and in `docs/TODO.md`: a shape for a shot, so a scattergun's `hit` lands in a cone; `on_equip`, for the reason `docs/design/items.md` gives; and a mind that uses a thing from its bag when hurt.
