# Items: what a thing does

Status: built 2026-09-22, against `main` at `0797370`; carriers redrawn 2026-09-23, against `60c430d`, when effects became a subsystem of their own; a use aimed at somewhere and a use made on another thing, things not yet known and things that will not come off, and ammunition added 2026-10-05 for Undermere's second part, against `d61cd5b`.
The reasoning is here; `docs/OVERVIEW.md` lists what exists, and `docs/design/effects.md` says how an effect lands.

## 0. Summary

The engine owns no item definition and never will: what an item *is* belongs to a game's own registry, and the engine reads components off whatever the game spawned.
Where and when items turn up is the engine's, through a trait a game implements on that registry; `docs/design/loot.md` has why.
This page is about the other half, which the engine does own: what a thing *does*, and which carrier of an effect list it should use.
It also says why two markers are the engine's, a thing not yet known and a thing that will not come off, and why ammunition is, in sections 8 and 9.

Three carriers exist, and the reason there are three rather than one is that they differ in who chooses them and when, not in what they can do:

| Carrier | Written in | Lands | Aimed | Spends |
| --- | --- | --- | --- | --- |
| An ability, known through `Grants` | the ability file | when the ability resolves | yes: any mode the ability names | a pool, health, a tagged item from the bag |
| A prop's offer | the prop file | when the offer is taken up | no: on the prop's own cell | nothing, or the game's own answer |
| `Triggers`, on a prop or an item | the prop file, or the game's item file | at a moment: used, landed, fired, hit, entered, destroyed | by being thrown or fired, or a `use` aimed by an ability's own rule | a charge, when the thing is `Consumable` |

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

The second repair drew the line at two ways of aiming a thing, each owned by a subsystem that already existed:

- **It is thrown**, through throwing and its cursor, and its `land` trigger fires where it comes to rest.
- **It is fired**, as a weapon through combat, and its `hit` trigger fires on whoever the shot struck; a wand is a weapon whose shot carries effects and spends a charge.

A third, an aimed use with a cursor of its own, was refused then as an ability by another name, and Undermere's staffs are why a third exists now.
A staff is zapped from the pack at a foe, which is neither a throw nor a worn weapon's shot, and a game made to write it as either bends the thing to fit the engine: a staff in a hand is a blow's weapon, rolled to hit, and a staff thrown is gone.
So a `use` trigger may carry an aim, and the aim is an ability's own rather than a copy of one: `Targeting` gathers an ability's mode, what it wants under it and whether its cell must be seen, `AbilityDef::targeting` hands one out, and an aimed use is previewed through `Bystanders::land`, refused by `aim_blocked` and landed on the footprint those work out, so a bolt from the pack and a bolt an actor knows cannot disagree about where they land or whom they catch.
What made the refused version an ability by another name was a second cursor and a second rule, and neither exists; what keeps the line is that the aim belongs to the thing's own trigger, so nothing reaches `Known`, nothing takes a row on the abilities screen, and what it lands and what it spends stay the thing's.
Two other ways were weighed and rejected: the staff lending an ability, which is the mixing this section exists to undo, and an aim written in words of its own, a second rule for one shape to drift from.

## 2. One list, many deliveries

In a fantasy game a potion drunk and a potion thrown do the same thing to different people.
So an item may write what it holds once, as `effects`, and each trigger that names no list of its own delivers it:

```ron
(name: "healing draught", stack: true, throw: (range: 5),
 consumable: (charges: 1, when_empty: Destroyed),
 effects: [(kind: "Mend", args: (roll: "2d6"))],
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

`UseItem` is an action like any other: the items resolver charges a turn for it, checks by `may_use` that the thing is in the bag, has a charge to spend and is worn if it can be worn, reports `ItemEvent::Used`, and reports the `use` moment.
`land_triggers` lands it in `ResolveSet::Triggers`, after every action and before damage, so a stim at one hit point is applied in the pass that spent the turn and beats the blow already queued behind it.

Where a use lands is written on its trigger rather than chosen by the key, in one of three ways:

- **On its user, where they stand**, for a trigger with neither a `targeting` nor an `asks`: the user is the only target and their own cell the only cell.
- **At a cell**, for a trigger with a `targeting`, used with `UseItemAt { item, at }` and pointed by the ability's rule, which section 1 has. With a look it is seen to fly, and while something watches it waits in the air and the turns wait with it, as an ability's bolt does.
- **On another of its user's things**, for a trigger that `asks`, used with `UseOn { item, on }`: it lands where its user stands, and the thing chosen goes to its effects as `Landing::on`. The engine's own effects never read it, so the engine carries the choice without learning what enchanting or identifying a thing is; a game's own effect reads it and does that.

Each of the three actions refuses the other two kinds for nothing, so a key that wrote the wrong one costs nothing and a bolt is never spent on its user's feet; `AimUse` is the one request a key writes for any thing, and the targeting cursor, the bag choosing or a use at once answers it.
What a use is made on is a choice in the bag rather than a cursor, because it is one of the player's own things and not a cell, and the bag offers its choices by the rule the resolver takes them by, `may_use_on`, as the cursor opens no aim `may_use` would refuse.
Both rules are public for that reason alone: a screen that worked out for itself what may be used would offer, one day, what the turn refuses.
A thing that can be worn is used only while it is worn: a plate carried in the bag and used would let a wearer keep one plate on and spend another's charge, which is the swap the attunement in section 5 exists to prevent.

The narrator says a use of a thing with a `use` trigger, `You use a stim.`, and says nothing of a use of anything else.
The engine landed the first and knows it did something; the second is the game's escape hatch, and only the game knows whether eating a crust of bread is worth a line.

An empty thing is refused before the turn is spent, so pressing use on a dry wand costs nothing; the bag says it is empty and does not offer the key.

A save taken while an aimed use is in the air keeps the charge it spent and loses the effect still waiting to land.
`EffectsPlugin` registers its in-air landings with `add_airborne`, which empties them when the run ends, as every landing in the air is emptied, an ability's among them, so the run that continues is short the bolt it paid for.
The loss is accepted rather than saved: a landing in the air holds entities and a built list of effects, which no save writes, and the window is the few frames a flight is watched for.

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

## 8. Things not yet known, and things that will not come off

Identification and curses are two rules every game with items writes, and a game writing either alone threads it through code it does not own.
A game that hides what a thing is redacts every panel that shows one, the bag's rows, the look cursor's panel, the targeting box and the gear rail's charges, in `ViewSet::Annotate`, and the first panel it forgets tells the player what the scroll is.
A game that makes a thing stay on checks every path that takes one off, taking it off, dropping it, throwing it and putting on what would push it off, in the bag's keys and in a mind's choice of gear, and the first path it forgets lets the curse off.
So the engine owns how each is honoured, as `Unknown` and `Stuck`, and a game owns when each is true.

`Unknown` withholds a thing's worth on every panel the engine draws and changes nothing the thing does: an unknown draught drunk heals, and an unknown blade strikes for what it strikes for.
Hiding what it does as well was rejected, since a game whose unknown thing did nothing until it was named would have to keep a second, real form of every effect; what is hidden is what the player is told, never what happens.
The armor total, the look cursor's duel forecast and the character sheet count an unknown worn thing at its true numbers, because those are what a blow meets, and the vitals mark the total uncertain while one is worn, `armor 5?`, the way a `+?` on a name marks a level.
What a thing not yet known seems to be is the game's, as its level is, so a game whose things are written from a level, where a true total tells the level the moment the thing goes on, writes the total and the forecast again in `ViewSet::Annotate` with each unknown thing counted as it seems, keeping the mark.
Undermere counts each one at its kind's plain numbers, the level of nought any thing of its kind is, so `+2 leather armor` not yet known reads `armor 1?`, and says under the forecast that it counted unknown gear plain.
Leaving the thing out of the total was rejected, since a total missing a coat is wrong every turn it is worn, and so was the engine counting it plain, since what plain is, and that a level is what is hidden, are the game's to say.
The character sheet is the one reading no game corrects yet, since it names every modifier by the thing it came from; a game that draws it with unknown gear worn corrects it the same way, and Undermere draws none.

`Stuck` refuses every way a worn thing comes off as any impossible item action is refused, the player keeping the turn, and says so with `ItemEvent::WouldNotComeOff`, which the narrator turns into `A ring will not come off.`
Which slot a thing that may go in several takes is `Equipment::equip_keeping`'s, beside the slot algebra, because a caller that only refused what `equip` displaced would refuse a ring for the finger it picked while the other finger's ring could have come off; a mind weighing gear asks the same rule, so it never walks to a better ring it would be refused.
Only being worn holds a thing, because a curse that held a thing in the bag would be a rule about carrying, which is a game's.

Neither marker is put on, taken off or saved by the engine.
What makes a thing known, a use, a reading or a long wear, and what frees it are the game's rules, so a game's own effect or system adds and removes the component and its own save keeps it, as Undermere's keeps its faces, its curses and the turns a thing has been worn.

## 9. Ammunition

Foundry's slugs were the first ammunition and were Foundry's own: a system of the game's spent one from the bag on every shot and benched the gun when the bag ran out.
A second game with a bow would have written the same loop again, so it moved into the engine as `AmmoPlugin`, opt-in since most weapons never run on it, and Foundry kept its content and its words.
A weapon carries `Ammo { tag }`, a shot spends one thing of that tag from its wielder's bag, and with none left the weapon is dry.
Dry is derived rather than tracked: `sync_ammo` reads every fed weapon's bag every pass and sets it dry or loaded from that alone.
The version that reacted to the events touching a bag, an equip, a pickup, a shot, needed a listener for every way a bag changes, and it missed two: two weapons on one bag fired the same last unit, and a dropped stack left a loaded gun behind.
A dry weapon's shot is put by in the same `Stowed` a game's own reason for benching a weapon uses, heat among them, so `Loadout` falls through to the next worn thing knowing nothing of ammunition, and `Dry` marks which of them ammunition put by, so each reason restores only its own.
What the gear rail says of a fed weapon is a facet the engine pushes in `ViewSet::Collect`, `N left` or `dry`, rather than a field of `Row`, since a magazine's count is a fact about a fed weapon and not about every row a panel draws; what a game says when one dries or loads is its own, over `AmmoChanged`.
There is no magazine and no reload action: a game that wants either writes it as the stack a reload makes.

## 10. Where the pieces are

- `crates/rl-bevy/src/effects/`: `Effects`, the engine's effects, `EffectBonus`, `Moments`, `Triggers`, `Fired`, `FiredOn`, `land_triggers` and `land_aimed_uses`.
- `crates/rl-bevy/src/consumable.rs`: `Consumable`, which moments spend, `spend_charges` and `recharge_charges`.
- `crates/rl-bevy/src/items.rs`: the actions, `UseItemAt` and `UseOn` among them, `may_use` and `may_use_on`, `ItemEvent`, `Bestows`, the gear fold, `Unknown` and `Stuck`.
- `crates/rl-bevy/src/ammo.rs`: `Ammo`, `Dry`, `AmmoChanged`, `spend_ammo` and `sync_ammo`; `Stowed` and `Loadout::dry` are in `combat.rs`.
- `crates/rl-bevy/src/ability.rs`: `Grants` and `Known`, an actor's own, and `Aimed` and `Bystanders::land`, which an aimed use shares.
- `crates/rl-rules/src/ability.rs`: `EffectSpec` and `TriggerSpec`, the authored forms any content file can read, and `Targeting` and `Asks`, a use's aim and its question.
- `crates/rl-rules/src/equip.rs`: `Equipment::equip_keeping`, the slot rule a stuck thing needs.
- `crates/rl-ui/src/view/target.rs` and `crates/rl-ui/src/panel/inventory.rs`: `AimUse`, answered by the targeting cursor and by the bag choosing.
- `examples/foundry/src/gear.rs` and `examples/foundry/assets/items.ron`: a game's own item file mapped onto the components above.
