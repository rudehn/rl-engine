# Loot

Status: built 2026-09-24, against `main` at `36fcb40`.
The reasoning is here; `docs/OVERVIEW.md` lists what exists, and `docs/guide/src/systems/loot.md` is the reference.

## 0. Summary

Two games put items into the world, and both wrote the same loop by hand.
Foundry and Corsair each had an item file with its own definition type, a spawn table banded by depth or distance, a hand-kept generator for what the dead drop so a kill would not shift combat's dice, a scatter that ran when a place or a region first appeared, and a function that turned an id into an entity.
Foundry also answered the engine's request to fill a container.
The rule the engine was written against is to own the loop or leave it out, and two games copying the same loop is the sign it belongs in the engine.

The engine now owns when, where, how many and from which stream.
What an item *is* stays each game's, as `docs/design/items.md` says it always will: a game implements `ItemMaker` on the resource that already holds its item definitions, and the engine asks it to make each thing.

## 1. The seam: a trait on the game's registry

The engine decides that three of something go on a cell; only the game knows what that something is.
`ItemMaker` is the whole of what the engine asks: `make` an id in a count, find an id by name, give the loot table, the scatter rules and a band, and optionally overrule how many loose items an area gets.

The id is the game's own typed id, `Id<Def>`, not a name.
The alternative was a name on every request, the way `FillContainer` asked for a container's contents before this.
`docs/PLAN.md` records the cost of that shape once already: Corsair's monsters and items held names as strings, checked once and then looked up again with `expect` at every spawn and every gear refresh.
A typed id is resolved once, when the file is read, and a typo is a load failure.

The trait sits on a resource the game already has, its armory, rather than on a new one the engine invents, so the only new code a game writes is the impl.

## 2. Streams

Every draw comes from a stream the engine derives from the run's seed, and which stream is the point:

- A place's floor is scattered from a stream derived for that place alone, a region's for that region, and a container's for its cell. What lies on deck four never depends on how many kills happened on deck three, and a region streamed in second draws what it would have drawn first.
- What the dead leave comes from `LootRng`, one stream kept from kill to kill, so the second kill rolls on from where the first left off. It is never combat's stream, so a kill's loot never shifts a blow that has not been struck.
- Whatever the game rolls on the thing it makes, a quality or an enchant, comes from the stream the engine hands `make`, so it is as deterministic as the rest.

Both games had these rules already, written separately, and one of them had drawn drops from combat's stream until a review caught it.

## 3. The table: sorted, grouped, tagged

`LootTable` is a banded, weighted table like the engine's `BandedTable`, with three differences.

**Its rows are sorted by item name when it is built.**
A banded draw walks the rows in order, so before this, moving a row in a spawn file changed what every seed built, and Foundry's file carried a warning saying so.
Sorting makes the file's order meaningless, which is what an author expects of a list.
Adding or removing a row still changes seeds, as it must.

**A row may give a group.**
The spawn table's group sizes were never used for items: both games picked a single row, so a slug on Foundry's floor was always one slug, and Corsair rolled its own stack size in code.
A group is now the row's, and a floor find of slugs is a handful.

**A draw may ask for a tag.**
This is what lets a container scale.
A locker that names its contents, a helmet and a plate, holds the same helmet on deck one and deck nine, and a game wanting better loot deeper has to write a deeper locker.
A locker that asks for "a weapon" draws among the rows whose item carries the tag, at the band it stands on plus an offset, so one definition serves every deck.

When nothing carrying the tag applies as deep as asked, the draw falls back to the nearest band above that has something.
A +2 weapon on the last deck asks past the end of the table, and the answer should be the best weapon, not nothing.
Above the shallowest row, the fallback is the shallowest, and a deeper request never draws from a shallower band than a shallower request would.

## 4. Containers

A container row is a fixed item in a count, or a tag in a number of draws, with an optional band offset.
A tag's count is draws, each its own, so a row of ammunition over two kinds of round may give some of each; the alternative, one draw yielding a stack of the count, could never mix.
Draws of one thing are made together, so six draws of slugs are one stack of six, not six stacks of one.

The engine rolls the counts, as it always did, and now answers the request itself when a `LootPlugin` is present.
A game with containers but no loot plugin still answers fixed items itself, and a container asking by tag is refused when play begins, because there is no table to draw from.

## 5. What was tried and not kept

- **The engine owning item definitions.** Every game's item has fields no other game's does: Foundry's heat and ammunition, Corsair's affixes and enchant levels. An engine item type would be either too small for any real game or an extension mechanism larger than the trait, and `docs/design/items.md` already settled that the engine reads components off whatever the game spawned.
- **Names on every request.** See section 1.
- **A rarity or quality system in the engine.** Corsair rolls quality and Foundry does not; `Found` tells the game why a thing is made and hands it a stream, which is all either needs.
- **Floor scatter on every place for every game.** Corsair's caves are stocked by hand at a vault's marks and must not also scatter loose finds; `ItemMaker::loose` lets a game say an area gets none, rather than the engine growing a switch per place kind.

## 6. Not here

- A shop, a trader or any economy: what something costs is a game's.
- Loot a mind picks up and uses; minds already pick up and equip through the item layer.
- Floor scatter on a revisit. A place is scattered once, on the arrival that built it; a game wanting respawning loot scatters it itself.
