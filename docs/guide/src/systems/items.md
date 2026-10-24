<!-- documents:
     plugins: ItemsPlugin, ContainerPanel, ContainerViewPlugin, InventoryPanel,
              InventoryViewPlugin
     files: crates/atglyph-bevy/src/items.rs
            crates/atglyph-bevy/src/parts.rs
            crates/atglyph-bevy/src/lighting.rs
            crates/atglyph-bevy/src/combat.rs
            crates/atglyph-bevy/src/throwing.rs
            crates/atglyph-bevy/src/turn.rs
            crates/atglyph-bevy/src/props.rs
            crates/atglyph-rules/src/equip.rs
            crates/atglyph-rules/src/items.rs
            crates/atglyph-rules/src/parts.rs
            crates/atglyph-rules/src/tag.rs
            crates/atglyph-rules/src/stats.rs
            crates/atglyph-ui/src/view/inventory.rs
            crates/atglyph-ui/src/view/container.rs
            crates/atglyph-ui/src/view/inspect.rs
            crates/atglyph-ui/src/view/target.rs
            crates/atglyph-ui/src/view/gear.rs
            crates/atglyph-ui/src/view/vitals.rs
            crates/atglyph-ui/src/panel/inventory.rs
            crates/atglyph-ui/src/panel/container.rs
            crates/atglyph-ui/src/narrate.rs
     fingerprint: ef418e52 -->

# Items and equipment

An item is an entity in one of three states: on the ground with a `Position` and the map it lies on, in a bag listed in a carrier's `Inventory`, or worn and also claimed in that carrier's `Equipped` slots.
The engine owns the moves between the three and charges a turn for each, and it owns nothing about what an item is.
What wearing one is worth is read off the item itself when a blow is struck, so nothing is copied onto the wearer and nothing has to be unwound when it comes off.
Two markers change how a thing is shown and held: one not yet known shows nothing of what it is worth, and one that will not come off refuses whatever would take it off.
What using, throwing or firing one does, and what that costs it, is [Uses and charges](uses.md); two screens come with this system, because an inventory panel belongs where inventory does.

## Turning it on

`ItemsPlugin` declares no `needs` at all: a game with items and no registries has a bag that works and rows with no names on them.
It inserts `ItemRules`, registers `ItemEvent`, the eight actions `PickUp`, `DropItem`, `Equip`, `EquipFromGround`, `Unequip`, `UseItem`, `UseItemAt` and `UseOn`, and the `Fired` and `FiredOn` a use reports, and registers `DeathEvent` as a message it reads so that the dead can drop what they carried in a game with no combat plugin to write one, and `RemoveStatus` as one it writes, which in a game with no statuses nobody reads; it also runs the charges [Uses and charges](uses.md) describes.
Its systems are `perceive_belongings` in `PerceiveSet::Annotate`, `resolve_items` in `ResolveSet::Act`, `pulse_worn` in `ResolveSet::Triggers` before `land_triggers`, `end_unworn_statuses` in `ResolveSet::Effects` before `resolve_afflictions`, `fold_gear` in `ReactSet::Settle`, `put_on_worn_statuses`, `lend_dark_sight` and `light_worn_things` chained in `ReactSet::Lend` after it, `restart_pulses` in `TurnSet::React`, `drop_what_the_dead_carried` in `CleanupSet::Remove` and `forget_removed_items` in `CleanupSet::Requeue`.
Every `Actor` already carries an empty `StatBlock` from `CorePlugin`, so what worn gear bestows has somewhere to fold.
`InventoryPanel` takes its rectangle, adds `InventoryViewPlugin` behind it if the game has not, declares the `inventory` modal, and registers the intents the screen writes whether or not the plugin that resolves each was added, so a game without throwing still has a bag.
`ContainerPanel` does the same for the `container` modal and depends on `PropsPlugin`, since what it shows is a prop's contents.
Either view plugin can be added alone by a game that wants the data and draws it itself.

## The model

`Inventory` is a `Vec<Entity>` in pickup order, and a worn item stays listed in it, so a bag is the whole of what is carried rather than what is carried and not used.
`Equipped` wraps an `Equipment` over the slots the game registered as `SlotDef`s, and `Wearable` is the `EquipShape` that says where an item goes: `any_of` are the slots it may take, first free one wins, and `also` are the slots it claims wherever it went.
A two-hander is `EquipShape::in_slot(main).and_claims(off)` and a ring is `in_any([left, right])`, and `equip` answers with everything it displaced or with an `EquipError`, which is `NoSlot` for a shape that names nowhere to go, `UnknownSlot` for one that names a slot this wearer does not have, and `Kept` for one that would push off something that stays on.
`equip_keeping(item, shape, stays)` is the same equip displacing nothing `stays` holds on: of the slots a thing may take it picks a free one, else the first whose occupant may come off, so a ring goes on the finger whose ring may come off, and with no slot that will do it refuses with nothing moved.
`Stack { key, count }` makes an item countable: picking one up merges it into a carried item with the same key instead of adding a row, and the key is the game's, usually the definition id.
`Tagged` is what an item counts as, by registered tag; `ItemLevel` is its `+N`; `Bestows` is what it does to registered stats while worn, as `(StatId, Op)` pairs written when the item is spawned with its level already applied, and `EffectBonus` is what its triggers' effects land stronger by, written the same way.
`ItemParts` is every part of one kind of item, built once by a game from its own fields typed with `atglyph_rules::parts`, `Wear`, `Charges`, `Level`, `Melee`, `Ranged` and `ThrowDef` among them, and `check` names what the engine would never play.
`insert_parts(e, parts, level)` puts on a spawned item every component its parts describe at that level, and `level_up` fits a thing to a new level from the parts it was made of, never lifting `Stuck`.
A worn thing may also lend what is not a stat, each optional: `worn_statuses` held on its wearer as `WornStatuses`, `dark_sight` lent as `WornDarkSight` unless a game put `DarkSightSuppressed` on the wearer, and a light lit only while worn, `LightWhen::Worn`.
`GearScore` is what wearing it is worth in the game's own units, and only the comparison matters: a mind weighs an item in sight against everything it would displace and, for a thing whose blow would replace its own, against its `OwnBlowScore` too, and puts on what is worth more than all of them together, never a thing that would push off something that stays on.
`Unknown` is a thing not yet known: the bag, the look cursor's panel and the targeting box show its name and where it goes and say `unknown` in place of what it does and is worth, the gear rail shows none of its charges, and the vitals mark the armor total uncertain while one is worn, any one, since a mark set by what the thing truly gives would tell what it is.
A game that knows part of it, its kind and not its level, pushes what it seems as facets on `ItemRow::seems` or `InspectView::seems`, which the bag and the look cursor print in place of `unknown`, and may write the numbers it seems to have into its `ItemRow`.
Nor is its use refused by anything only its kind, its reach or its charges would explain: `may_use` lets it be used empty, for the turn and to land nothing, and its aim is previewed as a plain line, as [Uses](uses.md) says.
What it does is untouched, so an unknown draught drunk does what it does and an unknown blade strikes for what it strikes for.
`Stuck` is a thing that will not come off while it is worn: taking it off, dropping or throwing it, and putting on anything that would push it off are each refused as any impossible item action is, the player keeping the turn, and reported as `ItemEvent::WouldNotComeOff`, which the narrator says as `Your ring will not come off.`, and the bag and the gear rail both say `will not come off` beside it while it is worn.
Only being worn holds it, so in the bag a stuck thing is put on, dropped and thrown as anything else is, and dying lets it fall.
`EquipFromGround` costs `ItemRules::equip_from_ground`, by default half a step more than picking up or putting on alone, so taking up a sword in the middle of a fight is a real choice rather than a free one or two turns wasted.
`resolve_items` reads all eight intents into one list, so one turn spends one item action whichever kind it is, and asks `atglyph_rules::items::resolve_item` what each comes to; an impossible one is refused for the player and charged as a wait to anyone else, the way an impossible move is, and what refuses a use is told in [Uses and charges](uses.md).
`ItemEvent` is what happened: `PickedUp` with the stack it `merged_into` when it merged, `Dropped`, `Equipped`, `Unequipped` for an item taken off by choice or displaced, `Used`, `Thrown`, and `WouldNotComeOff`.
`fold_gear` runs whenever `Equipped` changed and rebuilds rather than edits: every modifier tagged `Source::Item` is dropped and each worn item's `Bestows` put back in slot order, so an item taken off takes its changes with it and a run restored from a save rebuilds its gear modifiers for nothing, while a status's modifiers and anything the game filed under `Source::Game` carry tags of their own and are left where they are.
`Loadout` reads an item's `Armor`, `Resists`, `MeleeAttack`, `RangedAttack` and `BonusDamage` straight off it at the moment of a blow, which is the other half of wearing something and needs no fold at all.
`drop_what_the_dead_carried` lets a dead non-player's bag fall where it died, and `forget_removed_items` drops a despawned item from every bag and every slot.
`perceive_belongings` tells the mind holding the turn what it carries that it could throw and what lies in sight worth having, and only a mind with the wits to pick up or put on is told the second.
`InventoryView` is the player's bag as plain data: an `ItemRow` per item with its label, glyph, count, the slot it is worn in, the slots it could go in and the ones it takes as well by name, how far it flies and what it strikes for thrown, its armor, what it resists by the damage kind's name, its blow, its shot, its extra strikes, what it bestows by the stat's name, its tags, what its triggers do and how its use is aimed or what it asks for, its charges, whether it is empty, when its next charge is due and whether it is attuned, whether it is `unknown` or `stuck`, what a game says it seems while it is unknown, and the facets a game pushed.
Every number on a row is the item's own component, the one `Loadout` reads, so an item spawned to fight is described for free and a game says nothing twice; an unknown thing's row keeps its name, its look, its count and where it goes, and what the bag's keys act on, and drops the rest, whether it is empty among them, so its use key stays.
`used` is what its triggers say of themselves in the registries' names, one line per effect led by its moment, `use: mends 5` or `on landing: 3 kinetic in a burst of 1`, with a `use` and a `land` that say the same lines said once under `use or thrown`, each described with the item's own `EffectBonus` added so a thing its game made stronger says what it does as it is.
`InventoryPanel` is a modal the engine runs end to end, lettered `a)` to `z)` and in two steps, so no key means two things on it: a letter or Enter opens a row, only an opened row takes `InventoryKeys`' wear, drop, use and throw, and the close key goes back to the list and then puts the bag away.
The footer offers only the keys that do something to the opened row, none that would take off a worn thing that is stuck, and a game opens the bag on one thing with `InventoryMenu::open(row)`.
The use key uses a thing the way its use is written, which [Uses and charges](uses.md) describes; aiming a throw is the throw key's, which opens the targeting cursor, and firing is combat's.
A bag taller than its rectangle counts the rows it has no room for, `▲ 2 more` on the frame over its list and `▼ 3 more` on the rule under it, in the muted tone, and its list moves only as far as keeps the row picked out in view.
The screen does not read a key in the frame it opened, so a game that opens the bag on one of the bag's own keys, as Foundry's `t` does, opens it and no more.
Every action closes every screen, since it spends a turn and the turn loop assumes nothing is up while it runs.
`ContainerView` is what the open container holds, as the shared `Row`, and which one is open is `OpenContainer`, the screen's own state rather than the world's: a container is not open, it is being looked into.
`ContainerPanel` opens itself on an `Interacted` whose verb is `open` on a `Container`, walks the rows, writes `Take { from, item }` for one or for all of it, and closes when the container goes out of reach.
Taking does not close the screen, because opening already cost what the definition said and a crate emptied a piece at a time would otherwise cost a screen a time, and an emptied container is still shown, empty, until the screen closes, since a screen that vanished as the last thing came out would read as a fault.

## Using it

An item is an entity carrying the components that say what it is worth, and Warren's floor is littered with two of them.

<!-- include: ../../../../examples/tutorial/src/bin/step04_things.rs:litter -->
```rust,no_run
/// What the floor is littered with: bread that mends, rocks that fly.
fn litter(commands: &mut Commands, rats: &Rats, p: Point, bread: bool) {
    if bread {
        commands.spawn((Item, Crust(8), Name::new("crust of bread"), Position(p), Glyph::new('%', Color::srgb(0.85, 0.72, 0.40)).on_layer(2)));
    } else {
        commands.spawn((
            Item,
            Throwable::new(7, Some((rats.bite, DiceRoll::new(1, 4)))),
            Name::new("rock"),
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
Every one of those ids points into a registry the game filled, so slots, tags, stats and damage kinds are all content and adding a twelfth hardpoint or a fourth ring finger is a registry entry.
The engine never names anything either, which is why `InventoryLayout` carries a title, a word for the bag and a line for when it is empty; the engine has no word for a sea chest and will not invent one.
A level folds down to the vocabulary the rest of the rules already speak, armor, dice of a registered damage kind, registered stats, charges and a pulse's period, so an enchanted blade needs no new machinery on the way to a blow.
The engine applies a level once, as the thing is made, and never reads it in play: `insert_parts` writes its `Armor`, its dice, its `Bestows`, its `WornTimer` and its `EffectBonus` with the level applied, `Level` adding the same for each level and nothing else.
What one level adds is still the game's content, written once where the game chooses; Foundry's is a `level` block on each item beside numbers that stay what the thing is at `+0`, Undermere's a default per tag that each line writes over, and another game's may be a table, a curse or nothing.
Whether a thing is known and whether it stays on are the engine's to honour and the game's to decide: what `Unknown` withholds is withheld on every panel alike, and what `Stuck` refuses is every path that would take a thing off, so a game writing identification or a curse writes when its marker goes on and comes off, not a redaction of every panel and a check on every key.
Neither marker is put on, taken off or saved by the engine, since what makes a thing known, a use, a reading or a long wear, and what frees it are the game's, and a game keeps both in its own save.
`Unknown` hides what a thing is worth and never what it does, because a game whose unknown draught did nothing until named would have to mirror every effect, and the armor total still counts an unknown worn thing, marked as uncertain, because the total is what a blow meets, until a game that knows what the thing seems to be counts it so in `ViewSet::Annotate`, and clears the mark there for an unknown thing it knows cannot move the figure.
The container screen is take-only, because putting things back is a stash mechanic and the engine has no opinion about stashes.
Weight, bulk, encumbrance, durability, prices, and what makes a thing known or lifts a curse are each a game's rule over these components and these messages, and none of them is assumed here.

## Where it lives

`atglyph-rules` is tier 1 and has no Bevy in it: `equip.rs` is generic over the item handle, so the slot algebra, what a shape claims, what an equip displaces and which slot keeps what stays on, is proved with integers standing in for items and no `App` anywhere.
`Equipped` is that same code at `Equipment<Entity>`, which is the whole of what the Bevy layer adds to it, and a tool that weighed a loadout of definition ids would add no more.
`parts.rs` is how a thing's wear, charges, level and attacks are written, read through `Names`, and `items.rs` is `resolve_item`, what picking up, dropping, wearing and using come to, so a front end with no `App` plays the same rule.
`atglyph-bevy` is tier 2 and owns the three states and the moves between them, in `items.rs`, with the components a kind's parts become in `parts.rs`, which is also where the fold of what worn gear bestows lives, since only that layer knows what is worn right now, and where the two markers sit beside the resolver that honours them.
`atglyph-ui` holds the two screens, each split the way every panel is: the view is plain data with no colour and no rectangle in it, the collector refills it in `ViewSet::Collect`, and the presenter takes its rectangle in its constructor and draws in `PresentSet::Overlay`.
The split is what lets a game that wants a different bag screen keep the view and write its own presenter, and what lets the view be tested with a bag filled by hand.
