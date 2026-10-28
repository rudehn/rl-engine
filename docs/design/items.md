# Items: what a thing does

Status: built 2026-09-22, against `main` at `0797370`; carriers redrawn 2026-09-23, against `60c430d`, when effects became a subsystem of their own; a use aimed at somewhere and a use made on another thing, things not yet known and things that will not come off, and ammunition added 2026-10-05 for [Undermere](https://github.com/rudehn/undermere)'s second part, against `d61cd5b`; authored parts, the level part and the saved item added 2026-10-13, against `43d0b4a`, when the affix model was removed.
Since 0.4.0 a worn thing's clock is a `WornTimer`, an attack put by is a `BenchedAttack`, the statuses a worn thing keeps on its wearer are `ItemParts::worn_statuses` and `WornStatuses`, a status is asked for with `ApplyStatus`, a moment is a `TriggerKind`, and the rule what picking up, dropping, wearing and using come to is `atglyph_rules::items::resolve_item`, which `ItemsPlugin` applies.
The reasoning is here; `docs/OVERVIEW.md` lists what exists, and `docs/design/effects.md` says how an effect lands.

## 0. Summary

The engine owns no item definition and never will: what an item *is* belongs to a game's own type, and the engine reads components off whatever the game spawned.
What the engine does own is the shape of every field that describes something it already simulates, as authored parts a game types its own fields with, and the code that turns a definition's parts into components; section 6a has why, and why it is parts and not a schema.
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

[Foundry](https://github.com/rudehn/foundry)'s stim and medkit shipped as abilities first: each item named an ability in `grants`, and each ability was written with `costs: [Charge(1)]`, which the engine spent off the item's stack.
It worked, and it was wrong in three ways that each read fine in a diff:

1. **One file held two concepts.** `stims`, granted by an upgrade, is something the commando was taught; `med gel` is what a thing in the pack does when it is torn open. Nothing in the schema said which was which.
2. **The abilities screen listed the pack.** Granted abilities were folded into `Known`, so carrying two stims put two entries on the screen beside the one thing the commando actually knew.
3. **The item's economy was spelled as the ability's cost.** `Charge(1)` says "using me destroys one of whatever lent me", a fact about the item, written on the ability.

The first repair split them by what the use needed: a thing that just happens to its user became `OnUse`, and a thing that needed aiming stayed an ability the item lent.
[Foundry](https://github.com/rudehn/foundry)'s four grenades were the second kind, so a grenade was still an ability whose cost destroyed the item that lent it, and the mixing was only moved.

The second repair drew the line at two ways of aiming a thing, each owned by a subsystem that already existed:

- **It is thrown**, through throwing and its cursor, and its `land` trigger fires where it comes to rest.
- **It is fired**, as a weapon through combat, and its `hit` trigger fires on whoever the shot struck; a wand is a weapon whose shot carries effects and spends a charge.

A third, an aimed use with a cursor of its own, was refused then as an ability by another name, and [Undermere](https://github.com/rudehn/undermere)'s staffs are why a third exists now.
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
The field names are a game's own; [Foundry](https://github.com/rudehn/foundry) and Undermere read `triggers` into the engine's `TriggerSpec`, which is the shared part.

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
A thing not yet known is the exception, since the refusal would say it was empty: it keeps its key, and its use spends the turn and lands nothing, `land_triggers` landing no `use` of a thing with nothing left, which only an unknown thing can make.

A save taken while an aimed use is in the air keeps the charge it spent and loses the effect still waiting to land.
`EffectsPlugin` registers its in-air landings with `add_airborne`, which empties them when the run ends, as every landing in the air is emptied, an ability's among them, so the run that continues is short the bolt it paid for.
The loss is accepted rather than saved: a landing in the air holds entities and a built list of effects, which no save writes, and the window is the few frames a flight is watched for.

## 5. `Consumable`, and the ladder it walks

`Consumable` counts charges: `left` of `max`, `WhenEmpty::Destroyed` or `Kept` at zero, and an optional `Recharge` that gives one back for each period of the clock.
A stim holds one; a wand holds five and is kept when empty, refilling as the turns pass.

A worn thing that holds charges may be `Attuned`: it refills only while worn and is emptied each time it is put on, which the `Charges` part writes as `while_worn: true` beside its `recharge`.
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

- **`on_equip` effects.** Effects are one-shot; wearing is a standing state. A worn thing's clock is the other half, and it exists: a [`WornTimer`] reports the `pulse` moment every period its thing is worn, so a plate that knits wounds is a trigger, not a system. What a worn thing does is already declarative and needs no list: `Armor`, `Resists`, an attack, `Bestows` for the registered stats. A one-shot when something goes on, a cursed plate that bites, is a real case and a small one; it waits for a game that wants it, and would be one more moment.
- **An engine item schema.** No engine type is an item's definition, and no file's shape is the engine's; what is shared is the authored parts in section 6a, which a game's own fields are typed with, and the components they make.
- **Items as offers.** A crate you open and a medkit you use are nearly the same shape, and props' offers already carry a verb, a time cost and a refusal reason in words. Only a use is chosen, though; a landing or a hit is not, so it would carry half of what an item does. The refusal reasons are the part worth taking later, onto the `use` trigger.

## 6a. Parts, not a schema

Until 2026-10-13 the engine shared the components and the authored effect and trigger forms, and each game wrote the rest of its item file itself.
[Undermere](https://github.com/rudehn/undermere)'s `ItemDef` then opened with "Foundry's item schema trimmed", and it was: the same `MeleeDef`, `RangedDef`, `ThrowDef` and `ConsumableDef`, the same `slot`, `either` and `also`, the same validation, and a `spawn_item_at` of sixty lines that differed from Foundry's in a resist added rather than set and a hand pair named in code rather than read from the slots.
By the rule in `CLAUDE.md` the second game writing a thing is the signal it belongs in the engine, and it did, but not as a schema.

What moved is `atglyph_rules::parts`, a set of authored parts, each the type of one field and each describing only what the engine already simulates:

| Part | Fields | Makes |
| --- | --- | --- |
| `Melee` | `roll`, `kind`, `look`, `shape`, `unaware` | `MeleeAttack` |
| `Ranged` | `range`, `effective`, `roll`, `kind`, `look` | `RangedAttack` |
| `ThrowDef` | `range`, `effective`, `strike`, `range` left out to fly as far as the thrower reaches | `Throwable` |
| `LaunchDef` | `ammo`, `range`, `damage` | `Launcher`, worn, adding its range and roll to what it launches |
| `Wear` | `slot`, `either`, `also` | `Wearable(EquipShape)`, the pair `either` reaches for read from the slot registry's `SlotDef::pair` |
| `Charges` | `charges`, `when_empty`, `recharge`, `while_worn` | `Consumable`, its `Recharge` in hundredths of a step, and `Attuned` |
| `Level` | `min`, `max`, `armor`, `damage`, `amount`, `turns`, `pulse`, `charges`, `dark_sight`, `stats` | what each level adds to all of the above, in section 7 |

The rest of what an item's line says for the engine is a plain list, its armor, its tags, its effects and triggers, its bestows and caps, its ammunition tag, whether it stacks and how many one stack holds, its `Glow` and when it shines, the statuses it holds on its wearer and the sight in the dark it lends, needing no part of its own; `parts::resistances` reads the one list that adds up, a kind written twice resisting by both, as two worn things' resistances add on a wearer.
A game builds an `atglyph_bevy::ItemParts` from its own fields once at load, its triggers built and its wear resolved, and keeps it beside its definition; `insert_parts` spawns a thing of it at a level and `ItemParts::check` names what the engine would never play, a worn stack or a pulse on a thing never worn.
`Attacks { melee, ranged, throw, launch, cost }` is the attack half on its own, which a monster uses, since a blow is one thing whoever strikes it: [Foundry](https://github.com/rudehn/foundry)'s droids and Undermere's monsters write their attacks as the same parts their weapons do.

Every field past the essentials is optional, and absent means the engine's plain behaviour, never a mechanic switched on: a thing with no `Level` takes no level, a `Level` grows only the amounts it writes, and no part has a default the engine chose.
That is the line drawn by the guardrail the user set: the engine shares the shape of what it already plays, and a game's rules stay in the game.
[Undermere](https://github.com/rudehn/undermere)'s curse, its identification, the strength a thing asks, the traits it lends, how a level is rolled when found and what a staff's level does to its refill are Undermere's fields and Undermere's code; Foundry's heat and its `levels.ron` are Foundry's.
Using a part is optional too: the tutorial inserts its components by hand and nothing it did changed.

A single engine struct holding every part, embedded in a game's definition, was weighed and rejected.
It would be the engine's item schema under another name, every game's file would nest its engine half one level down, and a game wanting a different file would have to build the struct anyway; RON's `flatten` cannot be used beside `deny_unknown_fields`, which both games rely on to refuse a misplaced field.
Parts cost a game one field per part in its own type and one struct literal at load, and buy it its own file.

The look stays with the games.
A thing's glyph is `atglyph-render`'s and its mass noun `atglyph-ui`'s, neither of which `atglyph-rules` may see, and a helper in a crate that sees both would be a part that is not about simulation at all: a game drawing in tiles, or naming things with articles of its own, has a different look, and the engine has nothing to gain by choosing one.

## 6b. What a worn thing lends beyond its stats

Until 2026-10-15 two games each kept a wearer in step with what it wore by a system of their own.
[Foundry](https://github.com/rudehn/foundry)'s `sync_dark_sight` folded a droid's `NativeDarkSight` and a helmet's `WornDarkSight` into `DarkSight`, unless an ion jam had it blinded; Undermere's `lend` did the same for the traits a ring lends, with a status held while worn and a light lit while worn beside it, and kept the wearer's own sight in an `OwnSight` to give back.
That is the second game writing the same thing, which section 6 had named as the signal, so it is the engine's, and every piece of it is optional.

`ItemParts` gains `worn_statuses`, written `holds` before 0.4.0, the statuses held on the wearer until cured while the thing is worn; `dark_sight`, the tiles its wearer sees without light, grown by the `Level` part's own `dark_sight` as every level amount is; and `light_when`, `Carried`, today's behaviour and the default, or `Worn`, for a light dark on the ground and in a pack.
They become `WornStatuses`, `WornDarkSight` and `LitWhenWorn` on the thing, so a game that inserts components by hand gets the same as one that uses parts.
`ActorParts::dark_sight` puts an `OwnDarkSight` beside the `DarkSight`, which is what a worn thing's sight is folded with rather than written over.

`ItemsPlugin` keeps each every pass in `ReactSet::Lend`, read off what is worn now rather than answered as a thing goes on or comes off, which is the lesson [Foundry](https://github.com/rudehn/foundry)'s jam taught: a helmet swapped while its wearer was blinded kept the old helmet's radius the moment the jam cleared, because nothing had answered the swap while it was jammed.
A held status is asked for through `ApplyStatus`, held by the thing, and ended by `end_unworn_statuses` as any held status is.
A wearer's `DarkSight` is the best of its own and what it wears, or none while a game has put `DarkSightSuppressed` on it; what blinds is the game's, [Foundry](https://github.com/rudehn/foundry)'s ion jam, and the engine only keeps the folded sight from coming back meanwhile.
An actor with no sight of its own and nothing worn lending any is never touched, so a `DarkSight` a game sets by hand stays; one whose sight was lent with nothing of its own beneath is marked `LentDarkSight`, so the sight goes with the thing.
A game that sets an actor's sight by hand and also lets it wear dark sight writes its own as `OwnDarkSight`, since the fold cannot tell a sight a game set from one it lent.

The set comes after `ReactSet::Settle`, so a slot changed in the pass is read as it stands, and a game that blinds in its own reactions orders that before it: [Foundry](https://github.com/rudehn/foundry)'s `jam_sensors` runs before it, as it ran before `sync_dark_sight`, so a fresh ion hit blinds within the pass it lands.
Neither game's fingerprint moved.
What stays each game's: [Foundry](https://github.com/rudehn/foundry)'s jam, its clock and its `sensors down` badge, and the pack's dark-sight facet in its own words; Undermere's traits and which of them an item lends, folded into the item's parts by its armory.

## 7. A level, and the one place a game writes what it does

A `+2` thing is the same definition as a `+0` one, spawned stronger.
The `Level` part says what one level adds, and the engine's rule for it is linear and nothing else: a thing at level `n` is its own numbers plus `n` times each amount, armor to its `Armor`, damage to every damage roll of its blow, its shot and its thrown strike, `amount` and `turns` to its `EffectBonus`, `pulse` to its `WornTimer`'s period, `charges` to what a fresh unit holds, and each of `stats` to its `Bestows`.
Absent amounts are nought, and a level part with none at all is a thing that takes levels and changes nothing with them.
The only floors are what a component cannot hold: a pulse of under one hundredth, a unit of no charges, a status inflicted for less than nothing.
`min` and `max` bound the level, nought and no ceiling where neither is written, so a negative level is something a line asks for rather than something every thing can fall to.
The level a thing is at is `ItemLevel`, which no system of the engine's reads to play; it is what a game names and prices a thing by and what a save keeps.

`atglyph_bevy::parts::level_up(world, item, by)` moves a thing's level and fits every component the level decides from the parts it was made of, never from the thing as it stands, so a level is never added to a number that already carries one; what it has left of its charges grows with what it holds, an attack put by while it cannot be made is changed where it is put by, and a wearer's stats are folded again.
It never lifts `Stuck`: that a thing below nought will not come off is [Undermere](https://github.com/rudehn/undermere)'s curse and not the engine's level, so Undermere's own `level_up` calls the engine's and then frees a thing raised to nought.

Each game writes its own rule over the part, where it reads the file.
[Undermere](https://github.com/rudehn/undermere)'s is by tag, in `armory::level_defaults`: a weapon's level adds a damage and five accuracy, armor's a point of armor, a staff's and a wand's a charge, and a weapon, armor and ring go down to three below nought, each line's own `level` written over its tag's amount by amount with `Level::over`, so `items.ron` writes a ring's worth and a staff's power and not the rule every weapon shares.
How fast a staff refills at its level divides rather than adds, so it is not a level amount at all: the line writes its pace at level one, in hundredths, and [Undermere](https://github.com/rudehn/undermere)'s `recharge_at` and `staffs` keep it divided.
[Foundry](https://github.com/rudehn/foundry)'s is that armor and damage are inferred, +1 a level on a thing that has any, because that is what a reader expects of a `+2` plate or blade; every other key is nought unless written, because a reader cannot tell a plate's clock quickens unless the file says so.
Balance is set by `max`, 3 for plain armor, 5 for a weapon and 9 for the two plates that do something worn, rather than by a slower rate, since the standard rate is the one a reader expects.
[Foundry](https://github.com/rudehn/foundry) refuses each key on a thing with nothing it adds to, `armor` and `damage` written even as nought included, `turns` on a thing with no `Inflict` among its effects and `amount` on one with no `Harm` or `Mend`, so a level rule in the wrong place is a load error naming the item rather than a plate that quietly does less; and it refuses a `pulse` that would quicken a clock past a turn by its `max`, the rule its runtime floor used to be, since a plate pulsing several times each time its wearer acts is a rule nobody wrote on purpose.
What a level adds to the sight in the dark a thing lends is the level part's `dark_sight`, since that sight is the engine's (section 6b); until 2026-10-15 it was [Foundry](https://github.com/rudehn/foundry)'s own `dark_sight: (tiles:, per_level:)`.

The affix model this replaced, prefixes and suffixes with level-scaled grants and extra strikes, an enhance rule and weighted rolling, was never used by a game: both spawned `Enchanted` with no affixes and wrote their own level arithmetic beside it.
It was removed rather than kept beside the level part, because two engine answers to what a level is would be one too many.

The approaches weighed for [Foundry](https://github.com/rudehn/foundry)'s block, on 2026-09-27, are in `docs/superpowers/specs/2026-09-27-enchant-model-design.md`.
Scaling written on the number where it is used, `turns: (base: 10, per_level: 2)`, keeps level rules on the triggers, which is the scatter this removed; inferring every numeric key hides from a reader what grows; and the first build's `per_level` in an effect's arguments, with the level on the `Landing`, put the rule where no reader of an item looked.

## 7a. Saving a thing made of parts

`atglyph_save::SaveableItem`, registered with `save_items`, saves an item as which definition it is, by its maker's `name_of`, at which level, and the game's own extra beside them.
It is restored through the game's `ItemMaker::make_at`, so the thing is made again exactly as it was made when found, the level applied once, and only then is the game's extra put back on it, since what a game keeps is often about the level: [Undermere](https://github.com/rudehn/undermere)'s curse and how long it has been worn towards knowing it, Foundry's heat.

## 8. Things not yet known, and things that will not come off

Identification and curses are two rules every game with items writes, and a game writing either alone threads it through code it does not own.
A game that hides what a thing is redacts every panel that shows one, the bag's rows, the look cursor's panel, the targeting box and the gear rail's charges, in `ViewSet::Annotate`, and the first panel it forgets tells the player what the scroll is.
A game that makes a thing stay on checks every path that takes one off, taking it off, dropping it, throwing it and putting on what would push it off, in the bag's keys and in a mind's choice of gear, and the first path it forgets lets the curse off.
So the engine owns how each is honoured, as `Unknown` and `Stuck`, and a game owns when each is true.

`Unknown` withholds a thing's worth on every panel the engine draws and changes nothing the thing does: an unknown draught drunk heals, and an unknown blade strikes for what it strikes for.
Hiding what it does as well was rejected, since a game whose unknown thing did nothing until it was named would have to keep a second, real form of every effect; what is hidden is what the player is told, never what happens.

A refusal tells as much as a panel, and an aim's preview tells more, so `Unknown` withholds them too.
A cursor that showed an unknown staff's footprint, painted where its reach ended, refused an aim past it or out of its sight, named who it would catch or cycled only to what its aim wants told its kind in a few keys and for nothing: a beam from a bolt, a reach of twelve from one of eight, a wand that wants foes from one that takes anyone.
So an unknown thing's aim is a plain line from its user to the cursor and the cursor's cell, cycling to anyone, and the only refusals are what do not depend on the thing, not carried and worn to be used and not worn, since where it goes is shown.
Confirmed, it is used and spent, and lands by its true targeting where its flight stops: past its reach at its reach along the line, at a cell its sight forbids at the farthest cell on the line its user sees, by `made_at`, and on nobody its aim does not want, which a game hears as any use that changed nothing.
An empty one keeps its use key and comes to nothing for the turn, since a key that went away would say it was empty.
Refusing with a reason that did not name it, "you cannot aim there", was rejected: the refusal itself is what tells, whatever it says.
Previewing the true flight without its footprint was rejected as well, since where a line stops is its reach.
The armor total, the look cursor's duel forecast and the character sheet count an unknown worn thing at its true numbers, because those are what a blow meets, and the vitals mark the total uncertain while one is worn, `armor 5?`.
What a thing not yet known seems to be is the game's, as its level is, so a game whose things are written from a level, where a true total tells the level the moment the thing goes on, writes the total, the forecast and the targeting box's chance to hit again in `ViewSet::Annotate` with each unknown thing counted as it seems, keeping the mark, which under the forecast and the chance is a facet.
[Undermere](https://github.com/rudehn/undermere) counts each one at its kind's plain numbers, the level of nought any thing of its kind is, so `+2 leather armor` not yet known reads `armor 1?`, and says under the forecast that it counted unknown gear plain.
The mark goes on for any unknown worn thing, not only one that gives armor, because a mark set by what the thing truly gives would tell what it is: a ring that lends armor marked and one that lends light not, armor cursed to nothing read as no armor at all.
A game that knows which unknown things cannot move the figure, by what the player knows of them, clears the mark in `Annotate`; [Undermere](https://github.com/rudehn/undermere) clears it while every unknown thing worn is of a known kind whose level moves no armor, so an unidentified ring of light leaves `armor 1` plain.
Leaving the thing out of the total was rejected, since a total missing a coat is wrong every turn it is worn, and so was the engine counting it plain, since what plain is, and that a level is what is hidden, are the game's to say.
The thing itself reads the same way.
The bag and the look cursor print an unknown thing as `unknown` and nothing it is worth, which is right for a potion whose kind is the secret and says less than the player knows of a coat whose kind is plain and only its level is not.
So `ItemRow::seems` and `InspectView::seems` are what a game says the thing seems, facets printed where `unknown` would be, and a game may write the numbers it seems to have into the row in `Annotate`, which the bag prints as a known thing's.
[Undermere](https://github.com/rudehn/undermere) calls such a thing `unidentified chain mail`, and says what any thing of the kind is at nought, that its numbers are shown so, and how long wearing it will take to tell its level.
A second marker for a thing whose kind is known and whose level is not was rejected: what is known of a thing is the game's partition to draw, a level here and a maker or a charge count in another game, and `Unknown` already keeps the true numbers off every panel; what the engine lacked was only a way to stop saying `unknown` where the game had said more.
The character sheet is the one reading no game corrects yet, since it names every modifier by the thing it came from; a game that draws it with unknown gear worn corrects it the same way, and [Undermere](https://github.com/rudehn/undermere) draws none.

`Stuck` refuses every way a worn thing comes off as any impossible item action is refused, the player keeping the turn, and says so with `ItemEvent::WouldNotComeOff`, which the narrator turns into `Your ring will not come off.`; the bag and the gear rail both say `will not come off` while it is worn, in the same words and the same bad tone, so one mark means one thing on every panel.
Which slot a thing that may go in several takes is `Equipment::equip_keeping`'s, beside the slot algebra, because a caller that only refused what `equip` displaced would refuse a ring for the finger it picked while the other finger's ring could have come off; a mind weighing gear asks the same rule, so it never walks to a better ring it would be refused.
Only being worn holds a thing, because a curse that held a thing in the bag would be a rule about carrying, which is a game's.

Neither marker is put on, taken off or saved by the engine.
What makes a thing known, a use, a reading or a long wear, and what frees it are the game's rules, so a game's own effect or system adds and removes the component and its own save keeps it, as [Undermere](https://github.com/rudehn/undermere)'s keeps its faces, its curses and the turns a thing has been worn.

## 9. Ammunition

[Foundry](https://github.com/rudehn/foundry)'s slugs were the first ammunition and were Foundry's own: a system of the game's spent one from the bag on every shot and benched the gun when the bag ran out.
A second game with a bow would have written the same loop again, so it moved into the engine as `AmmoPlugin`, opt-in since most weapons never run on it, and [Foundry](https://github.com/rudehn/foundry) kept its content and its words.
A weapon carries `Ammo { tag }`, a shot spends one thing of that tag from its wielder's bag, and with none left the weapon is dry.
Dry is derived rather than tracked: `sync_ammo` reads every fed weapon's bag every pass and sets it dry or loaded from that alone.
The version that reacted to the events touching a bag, an equip, a pickup, a shot, needed a listener for every way a bag changes, and it missed two: two weapons on one bag fired the same last unit, and a dropped stack left a loaded gun behind.
A dry weapon's shot is put by in the same `BenchedAttack` a game's own reason for benching a weapon uses, heat among them, so `Loadout` falls through to the next worn thing knowing nothing of ammunition, and `Dry` marks which of them ammunition put by, so each reason restores only its own.
What the gear rail says of a fed weapon is a facet the engine pushes in `ViewSet::Collect`, `N left` or `dry`, rather than a field of `Row`, since a magazine's count is a fact about a fed weapon and not about every row a panel draws; what a game says when one dries or loads is its own, over `AmmoChanged`.
There is no magazine and no reload action: a game that wants either writes it as the stack a reload makes.

## 10. Where the pieces are

- `crates/atglyph-bevy/src/effects/`: `Effects`, the engine's effects, `EffectBonus`, `TriggerKinds`, `Triggers`, `Fired`, `FiredOn`, `land_triggers` and `land_aimed_uses`.
- `crates/atglyph-bevy/src/consumable.rs`: `Consumable`, which moments spend, `spend_charges` and `recharge_charges`.
- `crates/atglyph-bevy/src/items.rs`: the actions, `UseItemAt` and `UseOn` among them, `may_use` and `may_use_on`, `ItemEvent`, `Bestows`, the gear fold, `Unknown` and `Stuck`.
- `crates/atglyph-bevy/src/ammo.rs`: `Ammo`, `Dry`, `AmmoChanged`, `spend_ammo` and `sync_ammo`; `BenchedAttack` and `Loadout::dry` are in `combat.rs`.
- `crates/atglyph-bevy/src/ability.rs`: `Grants` and `Known`, an actor's own, and `Aimed` and `Bystanders::land`, which an aimed use shares.
- `crates/atglyph-rules/src/ability.rs`: `EffectSpec` and `TriggerSpec`, the authored forms any content file can read, and `Targeting` and `Asks`, a use's aim and its question.
- `crates/atglyph-rules/src/equip.rs`: `Equipment::equip_keeping`, the slot rule a stuck thing needs, and `SlotDef::pair` with `pair_of`, which `either` reaches for.
- `crates/atglyph-rules/src/parts.rs`: the authored parts, `Melee`, `Ranged`, `ThrowDef`, `Wear`, `Charges` and `Level`, and `resistances`; tested on plain values, with no `App`.
- `crates/atglyph-bevy/src/parts.rs`: `Attacks`, `ItemParts` and its check, `insert_parts`, `level_up`, `ItemLevel` and `MadeOf`.
- `crates/atglyph-save/src/run.rs`: `SaveableItem`, `SavedItem` and `save_items`.
- `crates/atglyph-ui/src/view/target.rs` and `crates/atglyph-ui/src/panel/inventory.rs`: `AimUse`, answered by the targeting cursor and by the bag choosing.
- [Foundry](https://github.com/rudehn/foundry)'s `gear.rs` and `items.ron`, and [Undermere](https://github.com/rudehn/undermere)'s `armory.rs` and `items.ron`, in their own repositories: two games' own item files typed with the parts above, each with its own rules for a level beside them.
