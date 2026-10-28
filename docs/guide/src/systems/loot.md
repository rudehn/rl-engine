<!-- documents:
     plugins: LootPlugin
     files: crates/atglyph-rules/src/loot.rs
            crates/atglyph-rules/src/prop.rs
            crates/atglyph-bevy/src/loot.rs
            crates/atglyph-bevy/src/props.rs
            crates/atglyph-bevy/src/parts.rs
     fingerprint: e8b8581e -->

# Loot

Loot is where items turn up: on a place's floor the first time it is built, where the dead fell, and inside containers.
The engine owns when, where, how many and from which stream; what an item is stays the game's, and the engine asks the game to make each thing through one trait.
A container may ask for a kind of thing by tag rather than naming it, and the same locker then holds better gear the deeper it stands.

## Turning it on

`LootPlugin::<M>` is opt-in, and generic over `M`, the resource holding the game's item definitions, which implements `ItemMaker`.
It declares `needs::<M>` with a hint on how to provide it and `needs::<Registries>`, and derives `LootRng` from the run's seed with `add_stream`.
A place is scattered in `LootSet::Scatter` and the dead leave their drops in `LootSet::Drop`, both inside `TurnSet::React`; a game that builds a place in the same pass orders what it puts down before `LootSet::Scatter`, so nothing lands under a crate.
Containers are filled in `PropSet::Fill`, the stage the engine keeps for answering what props asked.
As play begins it refuses, by name, a container holding an item the game has no definition for, and one asking for a tag nothing in the loot table carries.
`PropsPlugin` refuses a container that asks by tag when no `LootPlugin` was added, since there is nothing to draw it from.

## The model

`ItemMaker` is the game's side, and all of it: `make` turns an item id and a count into entities, placed nowhere, one stack for a thing that stacks and that many of anything else; `make_at` makes one thing at a given level, as it is made when found, which is also how a saved item comes back; `id_of` finds a definition by name and `name_of` names one; `table` is the game's `LootTable`; `scatter` gives its `ScatterRules`; `band` says how deep a `LootArea` is; and `loose` may overrule how many loose items an area gets.
`LootArea` is a place by `MapId`, and a band is the game's own number for how deep, far or dangerous that is.
`make` is handed a `Provenance { found, band }`: `found` is why something is being made, `Scatter`, `Drop`, `Container` or `Placed` at a prefab's slot by `PrefabPlugin`, and `band` is the one the draw was asked at, a place's for its floor, a container's plus its row's offset, a prefab slot's plus its own, and for a drop the band of the place the actor died in; `make` is also handed the engine's stream for whatever it rolls on the thing, a quality or an enchant.
`LootTable` is built by `loot::load` from a spawn file of rows, each an item, the bands it applies at, a weight and an optional group; its rows are sorted by item name, so the order they are written in draws nothing.
`pick` draws a row that applies at a band by weight and how many are found together; `pick_tagged` draws only among items carrying a tag, and when nothing carrying it applies as deep as asked, `band_for` falls back to the nearest band above that has something, so a request past the deepest row gets the deepest thing.
`ScatterRules` puts a count beside each place mark of a tag, a range of loose items, and more for each band; `plan_scatter` lays them on free cells over a `Layout` of marks and bounds, beside a mark and never on it, one to a cell.
`LevelTable`, loaded with `loot::load_levels`, rolls an enchant level by band for a game's `make` to put on the things it knows are enchantable.
A place is scattered from a stream derived from its own `PlaceSeed`, and a container from one derived from its place's seed and its cell, so what lies anywhere never depends on what happened elsewhere first.
`Drops<D>` is a `DropTable` the game puts on an actor when it spawns it, each `DropRow` rolled on its own with its chance and count, from `LootRng`, which carries on from kill to kill and never touches combat's stream.
A place is scattered on the arrival that built it, so nothing records which have been scattered.
A container's `ContentRoll` is `Stock::Item`, a named item in a count, or `Stock::Tag`, that many separate draws of the tag at the container's band plus the row's `band` offset; draws of one thing are made together, so six slugs are one stack.

## Using it

A game's item registry is its `ItemMaker`: `band` numbers a place, most games by its depth, `scatter` says how its floor is laid when it is first built, an item beside every mark of a kind and so many more loose, and `make` builds what was drawn at the `Provenance` it was found at, rolling on the stream it is handed whatever the game rolls on a thing, a level from its own table at that band say.

A container asks for kinds of thing, a weapon or a stack of some tag, and draws each at the band it stands on, so one locker holds deeper gear on a deeper floor.

## The line

The engine decides when loot is put down, where on a floor it lies, how many of each row a container or a death gives, and which stream each draw comes from.
The game decides what exists and what making one means: its item file, what a spawn file says turns up at each band, what a monster drops, what a container asks for, and whatever it rolls on the thing it makes.
The engine never learns what an item is, the line `docs/design/items.md` draws; it hands the game an id from the game's own registry and a count, and gets entities back.
A game with loot of its own it wants to place by hand, a hoard at a vault's marks, calls its own maker directly, and nothing here needs to know.
A band is the game's number and nothing else: a deck, a distance from home, a floor, and the engine only compares it.

## Where it lives

`atglyph-rules` holds the tables, the planning and the loader, all generic over the game's item id and tested with no `App`: that the order rows are written in draws nothing, that a deeper request never draws from a shallower band than a shallower request would, and that a scatter never puts two things on one cell or one on a mark.
`atglyph-bevy` holds the plugin, which is only when, where and which stream, over a trait a game implements on a resource it already had.
