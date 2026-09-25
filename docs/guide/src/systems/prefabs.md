<!-- documents:
     plugins: PrefabPlugin
     files: crates/rl-mapgen/src/prefab.rs
            crates/rl-mapgen/src/dungeon.rs
            crates/rl-rules/src/prefab.rs
            crates/rl-rules/src/role.rs
            crates/rl-rules/src/prop.rs
            crates/rl-rules/src/content/table.rs
            crates/rl-rules/src/ai/tactics.rs
            crates/rl-bevy/src/prefabs.rs
            crates/rl-bevy/src/places.rs
            crates/rl-bevy/src/minds.rs
            crates/rl-bevy/src/loot.rs
            crates/rl-bevy/src/remains.rs
            crates/rl-save/src/run.rs
     fingerprint: 881f5904 -->

# Prefabs

A prefab is a piece of a place written as data: rows of glyphs, and a legend saying what each glyph stands for, a tile or one slot.
A slot is a prop, items, a monster, or a mark left to the game; the engine fills every slot but a mark on the arrival that built the place, drawn at that place's depth, and the game fills its marks itself.
A monster slot may name a role rather than a monster, so a guarded room keeps its shape on every floor while what guards it changes, and a monster put there holds its cell as a post.

## Turning it on

`PrefabPlugin::<A, I>` is opt-in, and generic over `A`, the resource holding the game's monsters, which implements `ActorMaker`, and `I`, the one holding its items, which implements `ItemMaker`.
It declares `needs::<Prefabs<A::Def>>`, both makers, `Registries` and `Seed`, from which each slot's stream derives, each with a hint on how to provide it, and `depends_on::<CorePlugin>`; it does not need `LootPlugin`, so a game may lay items at slots and nowhere else.
Slots are filled in `PrefabSet::Fill`, inside `TurnSet::React` and before `LootSet::Scatter`, so no loot lands under a prefab's prop; a game that populates a place in the same pass orders that after `PrefabSet::Fill`, so its own spawns can keep off the slots.
As play begins it refuses, all at once and by name, a slot holding an item the game has no definition for, one asking for a tag nothing in the loot table carries, and one asking for a role none of whose members has a weighted row in the spawn table.
Only a keyed piece has slots: the game's chain stamps what `Prefabs::piece` hands it, and a piece still parsed from a closure stamps exactly as before, every mark the game's.

## The model

`rl_rules::prefab::load` reads one file into a `PrefabDef`: its `name`, which a chain asks for; its `ground`, painted under every slot and refused unless walkable whenever it is given; its rows; and a legend resolved to a tile or a `Slot` per glyph, one thing to a cell.
`Slot::Prop` is a prop by id, from `Prop("name")`.
`Slot::Item` is a `ContentRoll`, the row a container's contents use: `Item(item: "name", count: ..)` is a fixed item, and `Item(tag: "tag", count: .., band: N)` is that many draws of the tag at the place's band plus `N`.
`Slot::Monster` holds a `Pick` and an offset: `Pick::Kind` from `Monster(monster: "name")`, always that monster, or `Pick::Role` from `Monster(role: "role", band: N)`.
`Slot::Mark`, from `Mark`, is a position left to the game.
The loader resolves every tile, prop, tag, monster and role name and reports every mistake in the file at once; a fixed item's name is the one it leaves, since only the game's `ItemMaker` knows its items, and the plugin checks it when play begins.
`rl_rules::role::load` reads a roles file, a map from a role's name to the monsters that fit it, into a `Registry` of `RoleDef`, refusing an empty role, an unknown monster and a member named twice.
`role::draw` is the game's spawn table restricted to the role's members, with the rows' own weights, through `BandedTable::pick_where` at the band `role::band_for` answers with `BandedTable::band_where`: the band asked when a member applies there, else the deepest shallower band that has one, else the shallowest deeper.
`prefab::coverage` runs every drawn slot against a game's `Sources`, its roles, spawn table, loot table and tags, at every band in a range; `Coverage::render` prints `✓`, `~N` for a fallback to band `N`, or `✗`, and `empties` lists the gaps for a test.
`Prefabs` holds the loaded prefabs and roles behind one shared copy, so the game's place builder and the engine read the same files.
`Prefabs::piece` hands a chain a prefab by name as a `Prefab` keyed by its registry id, each slot a mark painted with the ground, and `Prefabs::slot` says what a stamped glyph stands for.
`PlaceBuild::from_context` turns each stamped mark into a `Spot` carrying its piece's key, and drops a mark inside the bounds of any stamp emitted after its own: the piece drawn on top owns its cells.
`ActorMaker` is the game's side for monsters: `make` spawns one at a cell on a map and returns it, `table` is the spawn table a role draws from, and `band` says how deep a map is.
`fill_prefabs` reads each first `PlaceEntered`, walks the place's keyed spots in the order they were recorded, and draws each slot from a stream derived as `prefab.content` for its own map and cell, so what one slot holds never depends on another.
It draws before it decides: on the arrival cell, on a cell no longer walkable, or on one already filled that arrival, a slot spawns nothing, and nothing anywhere else moves because of it.
An item slot whose count rolls nought lays nothing, so a maker is never asked to make none.
A prop is spawned as any prop is, items are made through `ItemMaker` as `Found::Placed` and laid on the cell, and a monster is made through `ActorMaker::make` and given `Post` on its cell.
A `Mark`, and every spot of an unkeyed piece, is the game's, found among the place's spots by its glyph.
`Post(Point)` is read by `KeepPost`, which walks a guard home when it has nothing better to do, waits when it stands beside its post and cannot step onto it, and passes for an actor with no post.
`rl-save` saves the post with the actor, and it comes off the dead with `WasLiving`.

## Using it

A prefab file is a piece and its slots, and Foundry's guard post is a locker and a weapon held by two sentries and a brute, each drawn at the deck it is stamped on.

<!-- include: ../../../../examples/foundry/assets/prefabs/guard_post.ron:post -->
```ron
(
    name: "guard post",
    ground: "deck",
    rows: [
        "#####",
        "#sAs#",
        "#.w.#",
        "#.b.#",
        "#...#",
    ],
    legend: {
        '#': Tile("bulkhead"),
        '.': Tile("deck"),
        'A': Prop("armory locker"),
        'w': Item(tag: "weapon", band: 2),
        's': Monster(role: "sentry", band: 1),
        'b': Monster(role: "brute", band: 2),
    },
)
```

Turning it on takes the roles the piece's monster slots name and a maker for monsters, whose `make` Foundry answers with the function its own population spawns by, so a guard and a roaming droid of one kind are built the same way.

<!-- include: ../../../../examples/foundry/assets/roles.ron:roles -->
```ron
{
    "sentry": ["probe droid", "trooper droid"],
    "brute":  ["line droid", "heavy droid"],
}
```

<!-- include: ../../../../examples/foundry/src/droids.rs:maker -->
```rust,no_run
/// The engine's guards are made here: a monster at a prefab's slot is
/// made exactly as one a deck's population places, and a deck's band is
/// its number.
impl ActorMaker for Roster {
    type Def = MonsterDef;

    fn make(&self, commands: &mut Commands, registries: &Registries, def: Id<MonsterDef>, at: Point, map: MapId, _: &mut rand::rngs::StdRng) -> Entity {
        spawn_monster(commands, self, def, at, map, registries)
    }

    fn table(&self) -> &BandedTable<Id<MonsterDef>> {
        &self.table
    }

    fn band(&self, map: MapId) -> i32 {
        crate::decks::deck_of(map) as i32
    }
}
```

## The line

The engine decides when a slot is filled, on the arrival that built the place and never again; from which stream, one derived for each slot's cell; what is skipped, and that a skipped slot still draws; how a role is drawn from the spawn table and where it falls back; and how a post is kept, saved and dropped at death.
The game decides what its pieces and roles are, what a monster and an item are and what making one means, where its chain stamps each piece and which way it faces, and what every `Mark` stands for.
A band is the game's number, answered by each maker for itself, and a slot's offset is the only thing on a piece that makes its draw deeper: a prefab carries no rarity, theme or stakes.
Which pieces a place gets is the chain's, through `StampPrefab` and `StampOneOf`; the engine owns what a piece holds, not how often it appears.
So is how far the arrival stands from a piece's guards: a slot on the arrival cell is skipped, and one beside it is not, so a chain whose pieces hold monsters draws its start with `RandomStart.clear_of_stamps(n)`, as Foundry's does at seven cells.
Whether a role's members fight side by side is the game's too, since who fights whom is its idea: Foundry's roles are all droids, because a scrap crab drawn to hold a post beside droid sentries was shot by them the moment either woke.

## Where it lives

`rl-rules` holds the file formats, the role draw and the coverage report, all tested with no `App` and no map: that every mistake in a file is reported together, that a role draw only ever returns a member of the role, and that a report names every slot that can draw nothing.
`rl-mapgen` carries a piece's key through the stamp and knows nothing of slots, so a keyless piece stamps as it always has and it never learns what a monster is; `rl-rules` knows a monster only as the game's generic id, the `M` in `RoleDef<M>`.
`rl-bevy` holds when, from which stream and what is skipped, the `Post` a placed monster keeps, `Prefabs::piece` handing a chain its keyed piece, and the check as play begins of what only the game's tables can answer, all over two traits a game implements on resources it already had; `rl-save` saves the post, the engine's record of where a guard belongs.
