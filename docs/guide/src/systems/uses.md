<!-- documents:
     plugins: none
     files: crates/atglyph-bevy/src/items.rs
            crates/atglyph-bevy/src/consumable.rs
            crates/atglyph-bevy/src/effects/mod.rs
            crates/atglyph-bevy/src/effects/triggers.rs
            crates/atglyph-bevy/src/effects/engine.rs
            crates/atglyph-bevy/src/ability.rs
            crates/atglyph-bevy/src/combat.rs
            crates/atglyph-bevy/src/throwing.rs
            crates/atglyph-bevy/src/status.rs
            crates/atglyph-bevy/src/cue.rs
            crates/atglyph-rules/src/ability.rs
            crates/atglyph-ui/src/view/inventory.rs
            crates/atglyph-ui/src/view/target.rs
            crates/atglyph-ui/src/panel/inventory.rs
            crates/atglyph-ui/src/narrate.rs
     fingerprint: e8806eb0 -->

# Uses and charges

What a thing does is its triggers, the effects it lands when each kind of trigger comes: when it is used, where it comes down when thrown, when a worn weapon fires and strikes, and every so often while it is worn.
A use lands on its user where they stand, at a cell it is pointed at for a thing aimed as an ability is, or on another of its user's things for one that asks for a thing, so a draught, a staff's bolt and a scroll read onto a blade are three ways of writing one trigger.
What doing it costs the thing is its charges, which a consumable keeps, spends, may refill on a clock, and runs out of.
The engine owns the use, the aiming, the choosing and the spending; what any of it does is the game's content.

## Turning it on

There is no plugin of its own: a use is an item action, resolved by `ItemsPlugin` from `UseItem`, `UseItemAt` or `UseOn`, which [Items](items.md) turns on, and what it lands is landed by `EffectsPlugin`, the subsystem that lands a prop's trap and an ability, so `ItemsPlugin` only reports the trigger.
`ItemsPlugin` is also what makes doing it cost the thing, needing no `AbilitiesPlugin`: it spends charges in `ResolveSet::Triggers` after the triggers land, and refills them in `ReactSet::Settle`; what a use lands needs `EffectsPlugin`, which a game adds or another plugin that lands effects, `AbilitiesPlugin` or `PropsPlugin`, adds for it.
A game's own use key writes `AimUse` for any thing, which the targeting cursor answers by aiming a use that is aimed and using at once one that is not, and the bag answers by choosing for one that asks; the bag's own use key does the same, and a game with neither screen writes the three actions itself.

## The model

`Triggers` is what a thing does at each `TriggerKind`, the component a prop carries too: `use` when it is used, `land` where a throw comes down, `fire` and `hit` when a worn weapon shoots and strikes, and `pulse` while it is worn, each over its `Area`, which [Effects](effects.md) describes.
A `use` trigger may carry a `targeting`, a `Targeting` written in an ability's words, `targeting: (mode: Bolt(range: 7), aim: Ground, sight: false)`, and then the thing is used at a cell with `UseItemAt { item, at }`, refused as an ability's aim is, by `Bystanders::land`, the call the targeting cursor previews it by, so a use the cursor shows as legal is one the resolver takes.
Or it may carry an `asks`, `asks: (prompt: "Hone which?", tag: "edge")`, and then the thing is used on another with `UseOn { item, on }`, which takes a thing carried, not the one used, and carrying the tag when one is named, worn or not, by `may_use_on`, the rule the bag offers its choices by.
A thing with neither is used where its user stands with `UseItem`, and each of the three refuses the others' kind for nothing, so a bolt is never spent on its user's feet.
`may_use` is the rule every use is refused by before where it is pointed: the thing must be carried, have a charge to spend when it counts them and its user knows it, and be worn when it can be worn, since a worn thing is used by wearing it; a refused use costs the player nothing, as any impossible item action does.
An accepted use writes `Fired` for the `use` trigger kind where its user stands, or at the cell an aimed one is made at, where it was pointed, and for a use made on a thing a `FiredOn` beside it, which the landing hands its effects as `Landing::on` for a game's own effect to read.
A use aimed somewhere lands as an ability does, on its footprint from where its user stands to the cell it was pointed at, and with a look it is seen to fly there while the turns wait.
A use that lands on a body carrying `Reflects` is turned back by the same rule an ability is, as often as its percent says: left standing alone where it was aimed, the whole of it lands on its user instead, as [Abilities](abilities.md) describes.
A thing not yet known, `Unknown`, is refused for none of what its targeting says: `may_use` takes whether it is unknown and lets it be used empty, the landing lands no `use` of a thing with nothing left, and `UseItemAt` makes it where it may still be pointed, by `atglyph::rules::ability::made_at`, the farthest cell its user sees on the line when it wants its cell seen, so it lands by its true targeting where its flight stops.
Its aim is previewed as a plain line to the cursor and the cursor's cell, `TargetView::unknown`, with no footprint, reach, refusal or target, and cycles as `Aim::Anyone` would.
`AimUse { user, item }` opens the targeting cursor on a thing whose use is aimed, as on an ability aimed the same way, and confirming writes the `UseItemAt` while putting it away spends nothing and passes no turn; a thing whose use lands on its user is used at once and opens nothing, one that asks opens the bag choosing for it, and one that cannot be used now opens nothing at all.
The bag's use key writes `AimUse` for an aimed thing, turns the bag to choosing for one that asks, `InventoryMenu::choosing` holding a `Choosing` of a `ChoosingFor`, titled with the use's prompt and listing only what it may be made on, and otherwise uses the thing where the player stands.
Choosing, confirm writes `UseOn` for the thing picked out and puts every screen away, and putting the bag away spends nothing.
`ItemRow::usable` says whether the use key does anything to a row, read off the `use` trigger, the charges, the wearing and, for one that asks, whether there is anything to choose, rather than off the description, so a terse effect does not lose the key that uses it; `targeting` and `asks` say how a row's use is pointed and what it may be made on, an `Asking` of the prompt and the choices.
A status a worn thing's trigger put on with `while_worn` is held by the thing, and the items plugin cures it in the pass its holder is no longer wearing that thing, taken off, dropped, displaced from its slot, spent or despawned alike, since it asks only whether the thing is worn.
`WornTimer` is a worn thing's own clock, written when the thing is spawned from its `pulse` trigger's `every` with its level applied, or by `WornTimer::every`: the `pulse` trigger kind is reported every `every` hundredths it is worn, on the wearer's cell and only on the current map, and the clock starts from nothing each time the thing is put on.
`Consumable` is what those triggers cost the thing: `left` of `max` charges, `WhenEmpty::Destroyed` or `Kept` at zero, and an optional `Recharge` on the clock.
`SpendingMoments` says which trigger kinds spend, `use`, `land` and `fire` unless a game adds its own, and each takes one charge: one off `left`, else the next unit of the `Stack` starts full, else the item is marked `Spent`, or kept empty.
A `Spent` thing is kept the way the dead are, so the log names it in its own colour: it is taken out of play at the end of the pass, no longer an `Item`, so it drops from every bag and slot and off the map, and it is despawned in `Last`'s `EndOfFrame::Bury`, beside the dead.
Absent, the thing survives every trigger, which is what a tool is, and a thing with charges and no trigger for a kind that spends still spends, which is how a plain wand's shot costs one.
Refilling counts the clock's time into a refilling thing's `Recharge` and gives back a charge for each full period.
An `Attuned` thing refills only while worn and is emptied each time it is put on, which the bag says of one off the body, charged or not, as `empties when put on; charges only while worn`, as it says `ready in N turns` of one charging.

## Using it

A use is a trigger and what it costs is its charges, and a staff is both: a `use` trigger carrying a `targeting` of `(mode: Bolt(range: 8))` in an ability's words, with a list that harms and ignites, and a `Consumable` that is `Kept` at nought and gives a charge back each period of its `Recharge`.

A game hangs both on what it spawns from a definition, the `Triggers` it built once when it read the file and a fresh `Consumable` on every copy, and marks `Attuned` the one whose charge comes back only while worn.

## The line

What a thing does is its triggers, and an item never lends an ability: an ability is something an actor knows, and `Known` is only ever what the actor itself learned.
A use aimed at somewhere is still the thing's own trigger: it is written in an ability's words and pointed, previewed and refused by an ability's rule, so the two cannot drift, while what it lands and what it spends stay the thing's and it takes no row on the abilities screen.
A game drew the line the hard way: its stims were abilities for a day, which put two consumables on the abilities screen beside the one thing its hero knew, and its grenades were abilities whose cost destroyed the item that lent them.
A use made on a thing carries the thing to its effects and no further, since the engine's own effects never read it: what makes a thing better, known or free is the game's, written as an effect of its own.
A trigger with no list of its own lands the item's shared `effects`, so a draught drunk and a draught thrown are one list delivered two ways, and the bag says them once.
A thing not yet known is used, aimed and refused by nothing only what it is would explain, since a refusal for nothing or a footprint drawn before it is used tells the player its kind as surely as its name would; what is refused still is what does not depend on it, not carried and worn to be used and not worn, and a game that tells a use that came to nothing says so of one made empty as of one that caught nobody.
A use of an item with a `use` trigger is said by the narrator, `You use a stim.`; an item with no trigger reports `ItemEvent::Used` and the game answers it, in its own words, which is the escape hatch and is meant to be one: the bag cannot know that a crust of bread mends, and a use that did nothing would still have spent a turn.
What wearing a thing is worth while it stays on is `Armor`, `Resists`, an attack and `Bestows`, standing states that go through no effects, and what it does when a trigger comes, a pulse or a use, is a list that lands once and is done, bridged to the wearing only by a status the thing holds, which the engine ends when the thing comes off so a game never writes the unwinding.
Charges are not a use's own: a thing with triggers and no `Consumable` costs nothing, and spending reads `Fired` after the triggers land, so what the last charge did lands before the thing is gone.
A save taken while an aimed use is in the air keeps the charge it spent and loses what it would have landed, since what is in the air belongs to the run and is emptied when the run ends.

## Where it lives

`atglyph-rules` is tier 1 and has no Bevy in it: `TriggerSpec`, `Targeting` and `Asks` are the authored forms, so a use's aim and its question are checked for their shape and their names without an `App`.
`atglyph-bevy` is tier 2: the use and its refusals are the items resolver's, because a use is an item action and one turn spends one of them, the landing is the effects subsystem's, shared with props and abilities, and the aiming is `Bystanders::land`, the ability resolver's, so a thing and an ability are pointed by one rule.
`consumable.rs` is beside items rather than inside it because a bag that works is not a bag that costs anything, and a game may want the first without the second; what the two share is one message, `Fired`.
`atglyph-ui` does the asking: the targeting cursor answers `AimUse` for an aimed use and the bag answers it by choosing, each through the rule the resolver refuses by, `may_use` and `may_use_on`, public in `atglyph-bevy` for exactly that, so a screen never offers what the turn would refuse.
