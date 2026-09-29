# Items: what a thing does

Status: built 2026-09-22, against `main` at `0797370`; carriers redrawn 2026-09-23, against `60c430d`, when effects became a subsystem of their own.
The reasoning is here; `docs/OVERVIEW.md` lists what exists, and `docs/design/effects.md` says how an effect lands.

## 0. Summary

The engine owns no item definition and never will: what an item *is* belongs to a game's own registry, and the engine reads components off whatever the game spawned.
Where and when items turn up is the engine's, through a trait a game implements on that registry; `docs/design/loot.md` has why.
This page is about the other half, which the engine does own: what a thing *does*, and which carrier of an effect list it should use.

Three carriers exist, and the reason there are three rather than one is that they differ in who chooses them and when, not in what they can do:

| Carrier | Written in | Lands | Aimed | Spends |
| --- | --- | --- | --- | --- |
| An ability, known through `Grants` | the ability file | when the ability resolves | yes: any mode the ability names | a pool, health, a tagged item from the bag |
| A prop's offer | the prop file | when the offer is taken up | no: on the prop's own cell | nothing, or the game's own answer |
| `Triggers`, on a prop or an item | the prop file, or the game's item file | at a moment: used, landed, fired, hit, entered, destroyed | only by being thrown or fired | a charge, when the thing is `Consumable` |

All three build the same `(kind, chance, args)` out of content, into the same `Effects`, landed by `EffectsPlugin`.

## 1. The dividing line: an item never lends an ability

An ability is something an actor knows how to do.
A thing in the pack is not something anyone knows; it is something they carry.
So an item carries `Triggers` and never `Grants`, and `Known` is only what the actor itself has learned: `refresh_known` reads the actor's own `Grants` and nothing in its bag.

The line was drawn twice before it held.

Foundry's stim and medkit shipped as abilities first: each item named an ability in `grants`, and each ability was written with `costs: [Charge(1)]`, which the engine spent off the item's stack.
It worked, and it was wrong in three ways that each read fine in a diff:

1. **One file held two concepts.** `stims`, granted by an upgrade, is something the commando was taught; `med gel` is what a thing in the pack does when it is torn open. Nothing in the schema said which was which.
2. **The abilities screen listed the pack.** Granted abilities were folded into `Known`, so carrying two stims put two entries on the screen beside the one thing the commando actually knew.
3. **The item's economy was spelled as the ability's cost.** `Charge(1)` says "using me destroys one of whatever lent me", a fact about the item, written on the ability.

The first repair split them by what the use needed: a thing that just happens to its user became `OnUse`, and a thing that needed aiming stayed an ability the item lent.
Foundry's four grenades were the second kind, so a grenade was still an ability whose cost destroyed the item that lent it, and the mixing was only moved.

The second repair is the line as it stands.
An item is aimed in exactly two ways, and each is owned by a subsystem that already existed:

- **It is thrown**, through throwing and its cursor, and its `land` trigger fires where it comes to rest.
- **It is fired**, as a weapon through combat, and its `hit` trigger fires on whoever the shot struck; a wand is a weapon whose shot carries effects and spends a charge.

There is no third, "aimed use" with a cursor of its own; that would be an ability by another name.

## 2. One list, many deliveries

In a fantasy game a potion drunk and a potion thrown do the same thing to different people.
So an item may write what it holds once, as `effects`, and each trigger that names no list of its own delivers it:

```ron
(name: "healing draught", stack: true, throw: (range: 5),
 consumable: (charges: 1, when_empty: Destroyed),
 effects: [(kind: "Mend", args: (kind: "care", roll: "2d6"))],
 triggers: [(on: "use"), (on: "land", area: Burst(radius: 1))]),
```

A thing that does two different things writes two lists, one on each trigger.
The field names are a game's own; Foundry and Corsair read `triggers` into the engine's `TriggerSpec`, which is the shared part.

## 3. `Effects`, and why it exists at all

Before `Effects`, `ability.rs` had a private list with a loop that rolled each chance and applied each effect, and `props.rs` had a second copy of both, written months later and already differing in how it reported a name nobody registered.
A third carrier would have been a third copy.

`Effects` is that list, once: built from `&[EffectSpec]` against the registered kinds, reporting every spec that would not build rather than the first, and landed onto a `Landing` whose `Source` says what landed it.
`describe` moved with it, so a screen can say what a list does without knowing what carries it: the bag's `use:` and `on landing:` lines are written by the same code as an ability's description.

Two consequences worth stating:

- **Every effect rolls from one stream, `EffectRng`.** A trap, a medkit and a spell are dealt from the same deck, for the reason `docs/design/effects.md` section 1 gives.
- **`EffectSpec` deserializes itself.** Each content file used to mirror the authored form privately. It is one public shape, `(kind: "Harm", chance: 50, args: (..))`, and `TriggerSpec` beside it is the same for a trigger.

## 4. What a use is, and is not

`UseItem` is an action like any other: the items resolver charges a turn for it, checks the thing is in the bag and has a charge to spend, reports `ItemEvent::Used`, and reports the `use` moment.
`land_triggers` lands it in `ResolveSet::Triggers`, after every action and before damage, so a stim at one hit point is applied in the pass that spent the turn and beats the blow already queued behind it.

The user is the only target and their own cell the only cell.
A thing that can be worn is used only while it is worn: a plate carried in the bag and used would let a wearer keep one plate on and spend another's charge, which is the swap the attunement in section 5 exists to prevent.
A thing that should land somewhere else is thrown or fired.

The narrator says a use of a thing with a `use` trigger, `You use a stim.`, and says nothing of a use of anything else.
The engine landed the first and knows it did something; the second is the game's escape hatch, and only the game knows whether eating a crust of bread is worth a line.

An empty thing is refused before the turn is spent, so pressing use on a dry wand costs nothing; the bag says it is empty and does not offer the key.

## 5. `Consumable`, and the ladder it walks

`Consumable` counts charges: `left` of `max`, `WhenEmpty::Destroyed` or `Kept` at zero, and an optional `Recharge` that gives one back for each period of the clock.
A stim holds one; a wand holds five and is kept when empty, refilling as the turns pass.

A worn thing that holds charges may be `Attuned`: it refills only while worn and is emptied each time it is put on, which Foundry writes on its charges as `recharge: (every:, while_worn: true)`.
That is the anti-swap rule, and it is on the thing rather than on the wearer because it is a fact about the thing: a worn thing that hides its wearer is earned by wearing it.
Equip time was the other candidate, and it was rejected as the rule because time alone still lets a player swap between fights.
A pulse is attuned by nature, since its clock only runs while worn.
A status a worn thing's trigger puts on its wearer may be held by the thing, written `while_worn` on the `Inflict`, and then it ends in the pass the thing is no longer worn, however it came off.
That is the other half of the rule: attunement stops a charged thing being swapped to, and holding stops a used one being swapped from, so a plate that hides its wearer cannot be used and traded for one that mends while the hiding runs on.

Using, landing and firing spend a charge, and a game may mark a moment of its own as spending.
A hit does not, since one shot can strike and a flaming blade is not used up by landing a blow.
At zero a stack of more than one loses a unit and the next starts full; otherwise the thing is destroyed or kept empty.
An empty kept thing does nothing it is spent by: `Loadout` stops finding its attack, and a use is refused.
A thing with triggers and no `Consumable` survives every moment, which is what a tool is.

A thing spent to nothing is marked `Spent` and kept, as the dead are, until the log has been drawn, so it is named there in its own colour: `remove_spent` takes it out of play at the end of the pass, off the map and no longer an `Item`, `bury_spent` despawns it at the end of the frame, and `forget_removed_items` clears it from the bag, which is where everything that stops being an item is forgotten.

## 6. What is deliberately not here

- **`on_equip` effects.** Effects are one-shot; wearing is a standing state. A worn thing's clock is the other half, and it exists: a [`Pulse`] reports the `pulse` moment every period its thing is worn, so a plate that knits wounds is a trigger, not a system. What a worn thing does is already declarative and needs no list: `Armor`, `Resists`, an attack, `Bestows` for the registered stats. A one-shot when something goes on, a cursed plate that bites, is a real case and a small one; it waits for a game that wants it, and would be one more moment.
- **"Hold a status while worn."** The insulated suit that reads as arc-resistant from the moment it goes on. A status a trigger puts on can already be held by the thing (section 5); one held for as long as the thing is worn, with no trigger to land it, is still a game's own system, as Foundry's `WornDarkSight` and `sync_dark_sight` are. By the rule in `CLAUDE.md`, the second game to write it is the signal it belongs in the engine.
- **An engine item schema.** `triggers`, `consumable`, `throw` and the rest are Foundry's own field names in Foundry's own file; Corsair's happen to match and need not. What is shared is the components and the authored effect and trigger forms, never the file.
- **Items as offers.** A crate you open and a medkit you use are nearly the same shape, and props' offers already carry a verb, a time cost and a refusal reason in words. Only a use is chosen, though; a landing or a hit is not, so it would carry half of what an item does. The refusal reasons are the part worth taking later, onto the `use` trigger.

## 7. A level, and the one place a game writes what it does

A `+2` thing is the same definition as a `+0` one, spawned stronger.
`Enchant` records the level and its rolled affixes, and no system of the engine's reads it; the affix model's level-scaled grants are functions the game calls while it spawns the thing.
Everything a level changes is a component the game writes when it spawns the thing, the level already applied: `Armor`, an attack's dice, `Bestows`, a `Pulse`'s period, and `EffectBonus` for what its effects land, which `docs/design/effects.md` section 8 has.
So what a level does is the game's rule, written once where the game chooses, and the engine never learns the word.

Foundry writes it in the item's `enchant` block, the one place a reader looks to learn what a `+2` is: `max`, the highest level it is found at, and what one level adds, `armor`, `damage`, `dark_sight`, `pulse`, `turns` and `amount`.
The item's own numbers stay where they are and are what it is at `+0`.
Armor and damage are inferred, +1 a level on a thing that has any, because that is what a reader expects of a `+2` plate or blade; every other key is nought unless written, because a reader cannot tell a helmet's dark sight grows unless the file says so.
Balance is set by `max`, 3 for plain armor, 5 for a weapon and 9 for the two plates that do something worn, rather than by a slower rate, since the standard rate is the one a reader expects.
Each key is refused on a thing with nothing it adds to, `armor` and `damage` written even as nought included, `turns` on a thing with no `Inflict` among its effects and `amount` on one with no `Harm` or `Mend`, so a level rule in the wrong place is a load error naming the item rather than a plate that quietly does less.
A level never quickens a pulse past a turn, since a plate pulsing several times each time its wearer acts is a rule nobody wrote on purpose, but the floor is on what the level changes: a period written below a turn is the thing at `+0` and stays so, because the base numbers stay where they are.
What only a worn thing has is written where it belongs rather than at the item's top level: a pulse's period on its pulse trigger's `every`, and a charge that refills only while worn on its charges, `recharge: (every:, while_worn: true)`, which spawns the thing `Attuned`.

The approaches weighed, on 2026-09-27, are in `docs/superpowers/specs/2026-09-27-enchant-model-design.md`.
Scaling written on the number where it is used, `turns: (base: 10, per_level: 2)`, keeps level rules on the triggers, which is the scatter this removed; inferring every numeric key hides from a reader what grows; and the first build's `per_level` in an effect's arguments, with the level on the `Landing`, put the rule where no reader of an item looked.

## 8. Where the pieces are

- `crates/rl-bevy/src/effects/`: `Effects`, the engine's effects, `EffectBonus`, `Moments`, `Triggers`, `Fired` and `land_triggers`.
- `crates/rl-bevy/src/consumable.rs`: `Consumable`, which moments spend, `spend_charges` and `recharge_charges`.
- `crates/rl-bevy/src/items.rs`: the actions, `ItemEvent`, `Bestows` and the gear fold.
- `crates/rl-bevy/src/ability.rs`: `Grants` and `Known`, an actor's own.
- `crates/rl-rules/src/ability.rs`: `EffectSpec` and `TriggerSpec`, the authored forms any content file can read.
- `examples/foundry/src/gear.rs` and `examples/foundry/assets/items.ron`: a game's own item file mapped onto the components above.
