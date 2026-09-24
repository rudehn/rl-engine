<!-- documents:
     plugins: ItemsPlugin, ConsumablesPlugin, ContainerPanel, ContainerViewPlugin,
              InventoryPanel, InventoryViewPlugin
     files: crates/rl-bevy/src/items.rs
            crates/rl-bevy/src/consumable.rs
            crates/rl-bevy/src/ability.rs
            crates/rl-bevy/src/effects/mod.rs
            crates/rl-bevy/src/effects/triggers.rs
            crates/rl-bevy/src/combat.rs
            crates/rl-bevy/src/props.rs
            crates/rl-bevy/src/status.rs
            crates/rl-bevy/src/turn.rs
            crates/rl-rules/src/ability.rs
            crates/rl-rules/src/equip.rs
            crates/rl-rules/src/affix.rs
            crates/rl-rules/src/stats.rs
            crates/rl-ui/src/view/inventory.rs
            crates/rl-ui/src/view/container.rs
            crates/rl-ui/src/panel/inventory.rs
            crates/rl-ui/src/panel/container.rs
            crates/rl-ui/src/narrate.rs
     fingerprint: 4d4feae4 -->

# Items and equipment

An item is an entity in one of three states: on the ground with a `Position` and the map it lies on, in a bag listed in a carrier's `Inventory`, or worn and also claimed in that carrier's `Equipped` slots.
The engine owns the moves between the three and charges a turn for each, and it owns nothing about what an item is.
What wearing one is worth is read off the item itself when a blow is struck, so nothing is copied onto the wearer and nothing has to be unwound when it comes off.
What using, throwing or firing one does is the item's own triggers, and what that costs it is its charges, which consumables keep.
Two screens come with the system, because an inventory panel belongs where inventory does.

## Turning it on

`ItemsPlugin` declares no `needs` at all: a game with items and no registries has a bag that works and rows with no names on them.
It registers `ItemEvent` and the six actions `PickUp`, `DropItem`, `Equip`, `EquipFromGround`, `Unequip` and `UseItem`, and registers `DeathEvent` as a message it reads so that the dead can drop what they carried in a game with no combat plugin to write one.
Its systems are `perceive_belongings` in `PerceiveSet::Annotate`, `resolve_items` in `ResolveSet::Act`, `fold_gear` in `TurnSet::React`, `drop_what_the_dead_carried` in `CleanupSet::Remove` and `forget_removed_items` in `CleanupSet::Requeue`.
Every `Actor` is given an empty `StatBlock` as it is spawned, with `try_register_required_components` rather than the plain call, because the status plugin asks for the same one and the order a game lists its plugins in must not matter.
What an item does at a moment is landed by `EffectsPlugin`, the same subsystem that lands a prop's trap and an ability, and `ItemsPlugin` only reports the moment.
`ConsumablesPlugin` is what makes doing it cost the thing, and it is opt-in on its own: it depends on `ItemsPlugin`, adds `EffectsPlugin` if the game has not, runs `spend_charges` in `ResolveSet::Triggers` after `land_triggers`, and runs `recharge_charges` in `TurnSet::React`.
It needs no abilities: a game can have things that are used and no `AbilitiesPlugin` at all.
`InventoryPanel` takes its rectangle, adds `InventoryViewPlugin` behind it if the game has not, declares the `inventory` modal, and registers the intents the screen writes whether or not the plugin that resolves each was added, so a game without throwing still has a bag.
`ContainerPanel` does the same for the `container` modal and depends on `PropsPlugin`, since what it shows is a prop's contents.
Either view plugin can be added alone by a game that wants the data and draws it itself.

## The model

`Inventory` is a `Vec<Entity>` in pickup order, and a worn item stays listed in it, so a bag is the whole of what is carried rather than what is carried and not used.
`Equipped` wraps an `Equipment` over the slots the game registered as `SlotDef`s, and `Wearable` is the `EquipShape` that says where an item goes: `any_of` are the slots it may take, first free one wins, and `also` are the slots it claims wherever it went.
A two-hander is `EquipShape::in_slot(main).and_claims(off)` and a ring is `in_any([left, right])`, and `equip` answers with everything it displaced or with an `EquipError`, which is `NoSlot` for a shape that names nowhere to go and `UnknownSlot` for one that names a slot this wearer does not have.
`Stack { key, count }` makes an item countable: picking one up merges it into a carried item with the same key instead of adding a row, and the key is the game's, usually the definition id.
`Tagged` is what an item counts as, by registered tag; `Enchant` is its `+N` and rolled affixes; `Bestows` is what it does to registered stats while worn, as `(StatId, Op)` pairs the game writes when it spawns the item with the enchant already applied.
`GearScore` is what wearing it is worth in the game's own units, and only the comparison matters: a mind weighs an item in sight against everything it would displace and puts on what is worth more than all of them together.
`EquipFromGround` costs `EQUIP_FROM_GROUND_COST`, half a step more than picking up or putting on alone, so taking up a sword in the middle of a fight is a real choice rather than a free one or two turns wasted.
`resolve_items` reads all six intents into one list, so one turn spends one item action whichever kind it is; an impossible one is refused for the player and charged as a wait to anyone else, the way an impossible move is.
A use of an empty `Consumable` is impossible, so it costs the player nothing.
An accepted use writes `Fired` for the `use` moment at the user's cell, beside the `ItemEvent`.
`ItemEvent` is what happened: `PickedUp` with the stack it `merged_into` when it merged, `Dropped`, `Equipped`, `Unequipped` for an item taken off by choice or displaced, `Used`, and `Thrown`.
`fold_gear` runs whenever `Equipped` changed and rebuilds rather than edits: every modifier tagged `Source::Item` is dropped and each worn item's `Bestows` put back in slot order, so an item taken off takes its changes with it and a run restored from a save rebuilds its gear modifiers for nothing.
A status's modifiers carry their own tag and are left where they are, and so is anything the game filed under `Source::Game`.
`Loadout` reads an item's `Armor`, `Resists`, `MeleeAttack`, `RangedAttack` and `Strikes` straight off it at the moment of a blow, which is the other half of wearing something and needs no fold at all.
`Triggers` is what a thing does at its moments, the component a prop carries too: `use` lands on the user where they stand, `land` where a throw comes down, `fire` and `hit` when a worn weapon shoots and strikes, each over its `Area`.
`Consumable` is what those moments cost the thing: `left` of `max` charges, `WhenEmpty::Destroyed` or `Kept` at zero, and an optional `Recharge` on the clock.
`SpendingMoments` says which moments spend, `use`, `land` and `fire` unless a game adds its own, and `spend_charges` takes one for each: one off `left`, else the next unit of the `Stack` starts full, else the item is marked `Spent`, or kept empty.
A `Spent` thing is kept the way the dead are, so the log names it in its own colour: `remove_spent` takes it out of play at the end of the pass, no longer an `Item`, so `forget_removed_items` drops it from every bag and slot, and off the map, and `bury_spent` despawns it in `Last`.
Absent, the thing survives every moment, which is what a tool is, and a thing with charges and no trigger for a moment still spends, which is how a plain wand's shot costs one.
`recharge_charges` counts the clock's time into a refilling thing's `Recharge` and gives back a charge for each full period.
`drop_what_the_dead_carried` lets a dead non-player's bag fall where it died, and `forget_removed_items` drops a despawned item from every bag and every slot.
`perceive_belongings` tells the mind holding the turn what it carries that it could throw and what lies in sight worth having, and only a mind with the wits to pick up or put on is told the second.
`InventoryView` is the player's bag as plain data: an `ItemRow` per item with its label, glyph, count, the slot it is worn in and the slots it could go in by name, how far it flies and what it strikes for thrown, its armor, its blow, its shot, its extra strikes, what it bestows by the stat's name, its tags, its charges and whether it is empty, and the facets a game pushed.
Every number on a row is the item's own component, the one `Loadout` reads, so an item spawned to fight is described for free and a game says nothing twice.
`used` is what its triggers say of themselves in the registries' names, one line per effect led by its moment, `use: mends 5` or `on landing: 3 kinetic in a burst of 1`, and `usable()` says whether the use key does anything, read off the `use` trigger and the charges rather than off the description so a terse effect does not lose the key that uses it.
`InventoryPanel` is a modal the engine runs end to end: `InventoryKeys` opens and closes it and wears, drops, uses and throws the row picked out, and the footer offers only the keys that do something to that row.
The use key uses a thing where the player stands, and a row with no `use` trigger or no charge left is not offered it; aiming is the throw key's, which opens the targeting cursor, and firing is combat's.
The screen does not read a key in the frame it opened, so a game that opens the bag on one of the bag's own keys, as Foundry's `t` does, opens it and no more.
Every action closes every screen, since it spends a turn and the turn loop assumes nothing is up while it runs.
`ContainerView` is what the open container holds, as the shared `Row`, and which one is open is `OpenContainer`, the screen's own state rather than the world's: a container is not open, it is being looked into.
`ContainerPanel` opens itself on an `Interacted` whose verb is `open` on a `Container`, walks the rows, writes `Take { from, item }` for one or for all of it, and closes when the container goes out of reach.
Taking does not close the screen, because opening already cost what the definition said and a crate emptied a piece at a time would otherwise cost a screen a time.
A container that was emptied is still shown, empty, until the screen closes, since a screen that vanished as the last thing came out would read as a fault.

## Using it

An item is an entity carrying the components that say what it is worth, and Warren's floor is littered with two of them.

<!-- include: ../../../../examples/tutorial/src/bin/step04_things.rs:litter -->
```rust,no_run
/// What the floor is littered with: bread that mends, rocks that fly.
fn litter(commands: &mut Commands, rats: &Rats, p: Point, bread: bool) {
    if bread {
        commands.spawn((Item, Crust(8), Name::new("a crust of bread"), Position(p), Glyph::new('%', Color::srgb(0.85, 0.72, 0.40)).on_layer(2)));
    } else {
        commands.spawn((
            Item,
            Throwable { range: 7, strike: Some((rats.bite, DiceRoll::new(1, 4))) },
            Name::new("a rock"),
            Position(p),
            Glyph::new('*', Color::srgb(0.66, 0.66, 0.70)).on_layer(2),
        ));
    }
}
```

What an item does at its moments and what doing it costs are two components, and Foundry's armory hangs them on every item whose definition has them.

<!-- include: ../../../../examples/foundry/src/gear.rs:use -->
```rust,no_run
    // What it does at its moments, a stim's use and a grenade's landing,
    // built once by the armory and shared by every copy; the engine lands
    // them. And what doing it costs the thing, which the engine spends.
    if let Some(triggers) = armory.triggers.get(id.index()).filter(|t| !t.0.is_empty()) {
        e.insert(triggers.clone());
    }
    if let Some(c) = d.consumable {
        let charges = Consumable::new(c.charges, c.when_empty);
        e.insert(match c.recharge {
            Some(every) => charges.recharging(every),
            None => charges,
        });
    }
```

The two screens are added the way every other panel is, each with its own rectangle and the words the engine has none for.

<!-- include: ../../../../examples/foundry/src/main.rs:bags -->
```rust,no_run
        InventoryPanel::new(screen.pack).title("Pack").called("pack").empty("Nothing but dust."),
        // What is inside a crate or a wreck, opened by walking into it or
        // by the key that does what is here.
        ContainerPanel::new(screen.chest).empty("Stripped already."),
```

## The line

The engine knows that an item exists, where it is, what moving it costs, what putting it on displaces and what its combat components are worth in a fight.
It does not know what an item *is*: there is no `ItemKind` enum, no `Custom { id }` and no table of categories, and a potion, a cutlass and a key are the same `Item` told apart by the components a game hung on them and by the ids those components hold.
Every one of those ids points into a registry the game filled, so slots, tags, stats, damage kinds and affixes are all content and adding a twelfth hardpoint or a fourth ring finger is a registry entry.
An item never lends an ability: an ability is something an actor knows, and `Known` is only ever what the actor itself learned.
What an item does is its triggers, and it is aimed in exactly two ways, each owned by a system that already existed: thrown, where its `land` trigger fires where it comes down, or fired as a weapon, where its `hit` trigger fires on whoever the shot struck.
There is no aimed use with a cursor of its own, because that would be an ability by another name.
Foundry drew the line the hard way: its stims were abilities for a day, which put two consumables on the abilities screen beside the one thing the commando knew, and its grenades were abilities whose cost destroyed the item that lent them.
A trigger with no list of its own lands the item's shared `effects`, so a draught drunk and a draught thrown are one list delivered two ways.
A use of an item with a `use` trigger is said by the narrator, `You use a stim.`; an item with no trigger reports `ItemEvent::Used` and the game answers it, in its own words, which is the escape hatch and is meant to be one: the bag cannot know that a crust of bread mends, and a use that did nothing would still have spent a turn.
Nothing about wearing goes through effects, and none is offered: what a worn thing does is `Armor`, `Resists`, an attack and `Bestows`, every one of them a standing state, where a list of effects lands once and is done.
The engine never names anything either, which is why `InventoryLayout` carries a title, a word for the bag and a line for when it is empty; the engine has no word for a sea chest and will not invent one.
Affixes fold down to the vocabulary the rest of the rules already speak, changes to registered stats and dice of a registered damage kind, so an enchanted blade needs no new machinery on the way to a blow.
The container screen is take-only, because putting things back is a stash mechanic and the engine has no opinion about stashes.
Weight, bulk, encumbrance, durability, identification, cursed gear and prices are each a game's rule over these components and these messages, and none of them is assumed here.

## Where it lives

`rl-rules` is tier 1 and has no Bevy in it: `equip.rs` is generic over the item handle, so the slot algebra, what a shape claims and what an equip displaces, is proved with integers standing in for items and no `App` anywhere.
`Equipped` is that same code at `Equipment<Entity>`, which is the whole of what the Bevy layer adds to it, and a tool that weighed a loadout of definition ids would add no more.
`affix.rs` is the rolling and the naming, and it ends at `(StatId, Op)` and `(DamageKindId, DiceRoll)` rather than anything an item has to interpret.
`rl-bevy` is tier 2 and owns the three states and the moves between them, in `items.rs`, which is also where the fold of what worn gear bestows lives, since only that layer knows what is worn right now.
`consumable.rs` is beside it rather than inside it because a bag that works is not a bag that costs anything, and a game may want the first without the second; what the two share is one message, `Fired`.
The triggers it spends for belong to the effects subsystem, and a prop carries them the same way: an item and a prop differ in which moments they report, never in how a list was built, rolled or landed.
`rl-ui` holds the two screens, each split the way every panel is: the view is plain data with no colour and no rectangle in it, the collector refills it in `ViewSet::Collect`, and the presenter takes its rectangle in its constructor and draws in `PresentSet::Overlay`.
The split is what lets a game that wants a different bag screen keep the view and write its own presenter, and what lets the view be tested with a bag filled by hand.
