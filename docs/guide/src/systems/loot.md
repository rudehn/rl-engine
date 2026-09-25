<!-- documents:
     plugins: LootPlugin
     files: crates/rl-rules/src/loot.rs
            crates/rl-rules/src/prop.rs
            crates/rl-bevy/src/loot.rs
            crates/rl-bevy/src/props.rs
            crates/rl-save/src/run.rs
     fingerprint: f85da7b7 -->

# Loot

Loot is where items turn up: on a place's floor the first time it is built, on a region of the streamed surface the first time it loads, where the dead fell, and inside containers.
The engine owns when, where, how many and from which stream; what an item is stays the game's, and the engine asks the game to make each thing through one trait.
A container may ask for a kind of thing by tag rather than naming it, and the same locker then holds better gear the deeper it stands.

## Turning it on

`LootPlugin::<M>` is opt-in, and generic over `M`, the resource holding the game's item definitions, which implements `ItemMaker`.
It declares `needs::<M>` with a hint on how to provide it and `needs::<Registries>`, and derives `LootRng` from the run's seed with `add_stream`.
A place is scattered in `LootSet::Scatter` and the dead leave their drops in `LootSet::Drop`, both inside `TurnSet::React`; a game that builds a place in the same pass orders what it puts down before `LootSet::Scatter`, so nothing lands under a crate.
A region is scattered in `EngineSet::Stream` as it loads, and containers are filled in `PropSet::Fill`, the stage the engine keeps for answering what props asked.
As play begins it refuses, by name, a container holding an item the game has no definition for, and one asking for a tag nothing in the loot table carries.
`PropsPlugin` refuses a container that asks by tag when no `LootPlugin` was added, since there is nothing to draw it from.

## The model

`ItemMaker` is the game's side, and all of it: `make` turns an item id and a count into entities, placed nowhere, one stack for a thing that stacks and that many of anything else; `id_of` finds a definition by name; `table` is the game's `LootTable`; `scatter` gives its `ScatterRules`; `band` says how deep a `LootArea` is; and `loose` may overrule how many loose items an area gets.
`LootArea` is a place by `MapId` or a region of the streamed surface by its coordinates, and a band is the game's own number for how deep, far or dangerous that is.
`Found` tells `make` why something is being made, `Scatter`, `Drop` or `Container`, and `make` is handed the engine's stream for whatever it rolls on the thing, a quality or an enchant.
`LootTable` is built by `loot::load` from a spawn file of rows, each an item, the bands it applies at, a weight and an optional group; its rows are sorted by item name, so the order they are written in draws nothing.
`pick` draws a row that applies at a band by weight and how many are found together; `pick_tagged` draws only among items carrying a tag, and when nothing carrying it applies as deep as asked, `band_for` falls back to the nearest band above that has something, so a request past the deepest row gets the deepest thing.
`ScatterRules` puts a count beside each place mark of a tag, a range of loose items, and more for each band; `plan_scatter` lays them on free cells over a `Layout` of marks and bounds, beside a mark and never on it, one to a cell.
A place is scattered from a stream derived for that place alone, a region from one derived for that region, and a container from one derived for its cell, so what lies anywhere never depends on what happened elsewhere first.
`Drops<D>` is a `DropTable` the game puts on an actor when it spawns it, each `DropRow` rolled on its own with its chance and count, from `LootRng`, which carries on from kill to kill and never touches combat's stream.
`Scattered` is the regions already scattered, which `rl-save` saves with `save_state::<Scattered>()`; a place needs no such record, being scattered on the arrival that built it.
A container's `ContentRoll` is `Stock::Item`, a named item in a count, or `Stock::Tag`, that many separate draws of the tag at the container's band plus the row's `band` offset; draws of one thing are made together, so six slugs are one stack.

## Using it

A game's item registry is its `ItemMaker`, and Foundry's is the armory: a deck's band is its number, and its floor has one item beside every armory mark, two beside every store and `3 + deck` more.

<!-- include: ../../../../examples/foundry/src/gear.rs:maker -->
```rust,no_run
/// The engine's loot is made here: what lies on a deck when it is first
/// entered, what a kill leaves, and what a crate or a locker holds. A
/// deck's band is its number, and Foundry has no streamed surface.
impl ItemMaker for Armory {
    type Def = ItemDef;

    fn make(&self, commands: &mut Commands, registries: &Registries, def: Id<ItemDef>, count: u32, _: Found, _: &mut rand::rngs::StdRng) -> Vec<Entity> {
        spawn_items(commands, self, def, count, registries)
    }

    fn id_of(&self, name: &str) -> Option<Id<ItemDef>> {
        self.defs.id(name)
    }

    fn table(&self) -> &LootTable<Id<ItemDef>> {
        &self.table
    }

    fn scatter(&self) -> ScatterRules {
        crate::loot::scatter_rules()
    }

    fn band(&self, area: LootArea) -> i32 {
        match area {
            LootArea::Place(map) => crate::decks::deck_of(map) as i32,
            LootArea::Region(_) => 0,
        }
    }
}
```

A container asks for kinds of thing, so one locker scales with the deck it stands on.

<!-- include: ../../../../examples/foundry/assets/props.ron:locker -->
```ron
    // An armory locker: better than a crate, and shut. A keycard opens
    // it, and `props::spend_the_keycard` takes the card doing it, so a
    // card found is a locker opened and no more. What is in it is drawn
    // at the deck it stands on, so a locker on deck eight holds a deck
    // eight weapon.
    (name: "armory locker", glyph: '&', color: (r: 205, g: 210, b: 220), blocks: true, health: 8,
     container: (contents: [(tag: "slug", count: (4, 9)), (tag: "weapon", count: 1), (tag: "armor", count: (0, 1)), (tag: "med", count: (0, 1))],
                 locked: "keycard", opened: (glyph: '"', color: (r: 130, g: 135, b: 145))),
     offers: [(verb: "open", time: 300)]),
```

## The line

The engine decides when loot is put down, where on a floor it lies, how many of each row a container or a death gives, and which stream each draw comes from.
The game decides what exists and what making one means: its item file, what a spawn file says turns up at each band, what a monster drops, what a container asks for, and whatever it rolls on the thing it makes.
The engine never learns what an item is, the line `docs/design/items.md` draws; it hands the game an id from the game's own registry and a count, and gets entities back.
A game with loot of its own it wants to place by hand, a hoard at a vault's marks, calls its own maker directly, and nothing here needs to know.
A band is the game's number and nothing else: a deck, a distance from home, a floor, and the engine only compares it.

## Where it lives

`rl-rules` holds the tables, the planning and the loader, all generic over the game's item id and tested with no `App`: that the order rows are written in draws nothing, that a deeper request never draws from a shallower band than a shallower request would, and that a scatter never puts two things on one cell or one on a mark.
`rl-bevy` holds the plugin, which is only when, where and which stream, over a trait a game implements on a resource it already had.
`rl-save` saves `Scattered`, since which regions have been stocked is the engine's record.
