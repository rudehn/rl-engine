<!-- documents:
     plugins: ItemsPlugin, ContainerPanel, ContainerViewPlugin, InventoryPanel,
              InventoryViewPlugin
     files: crates/rl-bevy/src/items.rs
            crates/rl-bevy/src/ability.rs
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
     fingerprint: befc63e5 -->

# Items and equipment

An item is an entity in one of three states: on the ground with a `Position` and the map it lies on, in a bag listed in a carrier's `Inventory`, or worn and also claimed in that carrier's `Equipped` slots.
The engine owns the moves between the three and charges a turn for each, and it owns nothing about what an item is.
What wearing one is worth is read off the item itself when a blow is struck, so nothing is copied onto the wearer and nothing has to be unwound when it comes off.
Two screens come with the system, because an inventory panel belongs where inventory does.

## Turning it on

`ItemsPlugin` declares no `needs` at all: a game with items and no registries has a bag that works and rows with no names on them.
It registers `ItemEvent` and the six actions `PickUp`, `DropItem`, `Equip`, `EquipFromGround`, `Unequip` and `UseItem`, and registers `DeathEvent` as a message it reads so that the dead can drop what they carried in a game with no combat plugin to write one.
Its systems are `perceive_belongings` in `PerceiveSet::Annotate`, `resolve_items` in `ResolveSet::Act`, `fold_gear` in `TurnSet::React`, `drop_what_the_dead_carried` in `CleanupSet::Remove` and `forget_removed_items` in `CleanupSet::Requeue`.
Every `Actor` is given an empty `StatBlock` as it is spawned, with `try_register_required_components` rather than the plain call, because the status plugin asks for the same one and the order a game lists its plugins in must not matter.
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
`ItemEvent` is what happened: `PickedUp` with the stack it `merged_into` when it merged, `Dropped`, `Equipped`, `Unequipped` for an item taken off by choice or displaced, `Used`, and `Thrown`.
`fold_gear` runs whenever `Equipped` changed and rebuilds rather than edits: every modifier tagged `Source::Item` is dropped and each worn item's `Bestows` put back in slot order, so an item taken off takes its changes with it and a run restored from a save rebuilds its gear modifiers for nothing.
A status's modifiers carry their own tag and are left where they are, and so is anything the game filed under `Source::Game`.
`Loadout` reads an item's `Armor`, `MeleeAttack`, `RangedAttack` and `Strikes` straight off it at the moment of a blow, which is the other half of wearing something and needs no fold at all.
`drop_what_the_dead_carried` lets a dead non-player's bag fall where it died, and `forget_removed_items` drops a despawned item from every bag and every slot.
`perceive_belongings` tells the mind holding the turn what it carries that it could throw and what lies in sight worth having, and only a mind with the wits to pick up or put on is told the second.
`InventoryView` is the player's bag as plain data: an `ItemRow` per item with its label, glyph, count, the slot it is worn in and the slots it could go in by name, how far it flies and what it strikes for thrown, its armor, its blow, its shot, its extra strikes, what it bestows by the stat's name, its tags, what it `lends` and the facets a game pushed.
Every number on a row is the item's own component, the one `Loadout` reads, so an item spawned to fight is described for free and a game says nothing twice.
A `Lent` is an ability an item grants: which, what it is called, what it says, whether using it needs somewhere to point, and the charges left.
`InventoryPanel` is a modal the engine runs end to end: `InventoryKeys` opens and closes it and wears, drops, uses and throws the row picked out, and the footer offers only the keys that do something to that row.
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
What an item does when used is the ability it `Grants`, spent from the item by `Cost::Charge`, so a potion is a line of RON and the item resolver leaves a use of one alone for the abilities plugin to redirect.
An item that grants nothing reports `ItemEvent::Used` and the game answers it, which is the escape hatch and is meant to be one: the bag cannot know that a crust of bread mends, and a use that did nothing would still have spent a turn.
The engine never names anything either, which is why `InventoryLayout` carries a title, a word for the bag and a line for when it is empty; the engine has no word for a sea chest and will not invent one.
Affixes fold down to the vocabulary the rest of the rules already speak, changes to registered stats and dice of a registered damage kind, so an enchanted blade needs no new machinery on the way to a blow.
The container screen is take-only, because putting things back is a stash mechanic and the engine has no opinion about stashes.
Weight, bulk, encumbrance, durability, identification, cursed gear and prices are each a game's rule over these components and these messages, and none of them is assumed here.

## Where it lives

`rl-rules` is tier 1 and has no Bevy in it: `equip.rs` is generic over the item handle, so the slot algebra, what a shape claims and what an equip displaces, is proved with integers standing in for items and no `App` anywhere.
`Equipped` is that same code at `Equipment<Entity>`, which is the whole of what the Bevy layer adds to it, and a tool that weighed a loadout of definition ids would add no more.
`affix.rs` is the rolling and the naming, and it ends at `(StatId, Op)` and `(DamageKindId, DiceRoll)` rather than anything an item has to interpret.
`rl-bevy` is tier 2 and owns the three states and the moves between them, in `items.rs`, which is also where the fold of what worn gear bestows lives, since only that layer knows what is worn right now.
`rl-ui` holds the two screens, each split the way every panel is: the view is plain data with no colour and no rectangle in it, the collector refills it in `ViewSet::Collect`, and the presenter takes its rectangle in its constructor and draws in `PresentSet::Overlay`.
The split is what lets a game that wants a different bag screen keep the view and write its own presenter, and what lets the view be tested with a bag filled by hand.
