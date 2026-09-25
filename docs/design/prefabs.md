# Prefabs

Status: built 2026-09-25 on branch `prefab-slots`, against `main` at `b5dd60b`.
The reasoning is here; `docs/OVERVIEW.md` lists what exists, and `docs/guide/src/systems/prefabs.md` is the reference.

## 0. Summary

A prefab was a Rust closure from a glyph to a tile, and every other glyph was a mark the game gave meaning to by hand.
Foundry looked up "armory locker" by name and put one at every `A`; Delve spawns its heart warden at a `W`; Corsair lays a hoard at a vault's marks.
Each game wrote that loop again, and a guard at a mark was a fixed monster, strong on the first floor and harmless on the tenth.

Containers had already solved this for items: a contents row says `(tag: "weapon", band: 2)`, and the item is drawn against the band of wherever the container landed.
A prefab slot gives a cell the same power for props, items and monsters, so a guarded room keeps its shape and its tension on every floor while what guards it changes.

The engine now owns when a slot is filled, from which stream, what is skipped, how a role is drawn and falls back, and how a guard keeps its post.
What a monster and an item are stays the game's: it implements `ActorMaker` on the resource holding its monsters, beside the `ItemMaker` it already has, and the engine asks both to make what it decided.

## 1. The legend: one thing per cell

A prefab file is rows of glyphs and a legend, and the legend maps each glyph to exactly one of a `Tile`, a `Prop`, an `Item`, a `Monster` or a `Mark`.
A cell is a tile or a slot, never both, and every slot stands on the file's `ground`, which must be walkable.
So a guard cannot be placed on a wall and a prop cannot share a cell with a monster, by construction rather than by a check at spawn.

The alternative was the one fantasy-rogue shipped: a tile grid, and beside it three lists of coordinates for monsters, items and props.
Nothing tied a coordinate to what the grid held there, so a slot could be declared on a wall, two lists could name one cell, and a rotation had to be applied to four things that could each get it wrong.
Fantasy-rogue caught the first only at spawn, by skipping a slot whose cell was not walkable, and the second not at all; here neither can be written.

A slot's glyph is a mark to `rl-mapgen`, so it turns and mirrors with the tiles for nothing, and the stamp carries the prefab's key so each mark says which piece it came from.
`rl-rules` owns the file and every draw, and names no mapgen type: `Prefabs::piece` in `rl-bevy` turns a loaded file into a keyed `rl_mapgen::Prefab`, so a prefab file loads and every one of its mistakes is reported with no map and no `App`.

## 2. Items: the container's row

An item slot is the same `ContentRoll` a container's contents are: `Item(item: "keycard")` in a count, or `Item(tag: "weapon", count: .., band: 2)`, that many separate draws of the tag at the place's band plus the offset.
Two spellings of one idea would drift, and a locker's row and a floor slot's row answering "a weapon, two decks ahead" differently would be a bug no test names.
A fixed item takes no band, for the reason a container's does not: it is drawn from no band, so an offset would mean nothing.

A fixed item's name is checked when play begins, not at load.
A game's items are its own registry, the same as a container's fixed contents, and only the game's `ItemMaker::id_of` can say whether one exists; the prefab loader never sees it.
Every other name, a tile, a prop, a tag, a monster or a role, resolves at load.

A prop slot takes no band.
A prop is a fixed thing, and a container among them already carries its own offsets in its contents rows, so a band on the slot would be a second knob for one number.

## 3. Roles: their own file, rolling the spawn table

A monster slot names a monster, which it always is, or a role, drawn at the place's band plus an offset.
A role is a name and the monsters that fit it, in a file of its own, `roles.ron`, and nothing more.

It is not a column in the spawn table, because which floors a monster appears on is already written once, there.
A role draw is the game's own spawn table restricted to the role's members, at the slot's band, with the rows' own weights: `BandedTable::pick_where` over `band_where`.
A role file that also said where each member appears would be a second spawn table, and the two would disagree the first time one was retuned.

When no member applies at the band asked for, the draw falls back as a tagged loot draw does: to the deepest band shallower than the one asked that has a member, so a slot asking past the table's end gets its deepest, and else to the shallowest band deeper.
A guard post on the last deck asking for a brute two decks further should get the heaviest brute there is, not an empty doorway.
`prefab::coverage` is how an author finds out a slot fell back, since no load can: it runs every drawn slot against the game's tables at every band and prints `✓`, `~N` or `✗` for each.
Foundry's report shows its guard post's brute and weapon falling back to deck ten from deck nine, which is the table ending, not a mistake.

A role is only as good as its members' agreement.
Foundry's first brute role held a scrap crab beside its droids, and droids and vermin are hostile, so a post whose brute was a crab shot itself apart the moment a sentry woke; its roles are now all droids, and a test holds every role to one faction.
The engine does not check this, because a faction is a game's idea of who fights whom and a mixed role may be what a game wants.

## 4. Streams: one per cell, and a draw before the decision

Terrain and contents are two steps with two streams.
Mapgen stamps terrain and records which piece it stamped; contents are drawn on the arrival that built the place, and never again, so a revisit finds what it left.
Fantasy-rogue learned this from a shared stream: a revisit re-stamped terrain and restored contents from a cache, advanced the stream differently, and moved every prefab after the first.

Each slot then draws from a stream of its own, derived as `prefab.content` from the run's seed, hashed by the map and the slot's cell.
One stream for the whole place, drawn in spot order, would make every slot's contents depend on every slot before it, so adding a slot to one piece, or a chain stamping a piece in a different room, would change what an unrelated piece holds.
Per cell, a slot's contents depend on its place, its cell and the seed, and on nothing else.

A slot draws first and then decides whether to spawn.
It spawns nothing on the arrival cell, on a cell a later pass left unwalkable, or on a cell another slot already filled this arrival, and it still takes its draw.
With one stream per cell the draw is not strictly needed to keep the others still, but the rule costs nothing and keeps a slot's result independent of where the player happened to arrive, which is the property the tests hold.
Fantasy-rogue drew first and skipped second for the same reason, and the rule is kept from it.

## 5. The piece on top owns its cells

`PlaceBuild::from_context` drops every mark inside the bounds of a stamp emitted after its own, keyed or not.
Two stamps can overlap when a chain places them at points; the later one painted its own tiles there, and a slot of the earlier piece filling a cell that now belongs to another would put a guard inside someone else's wall or on someone else's console.
No shipped game can see this yet, since every one stamps with `Placement::AnyRoom`, which never overlaps an earlier stamp, but a chain that stamps at a point would.

## 6. A post, and only one kind

A monster placed at a slot is given `Post(cell)`, and `KeepPost` in its brain walks it back there when it has nothing better to do.
It fights, flees and searches as it otherwise would, since `KeepPost` sits after `SearchLastKnown` and before the idle tactic: a guard that loses the player searches where it last saw them, then goes home.
A guard that has noticed no one has no enemy in its snapshot and nowhere to search, so `KeepPost` is the first tactic that answers, and it stands its post rather than wandering off before it has seen anyone.
`KeepPost` passes for an actor with no post, so a kind shares one brain whether or not this one was posted, and waits beside its post when something else stands on it rather than wandering.

The post is saved: `rl-save` writes it with the actor, and it comes off the dead with `WasLiving`, since a body walks back to nothing.
Fantasy-rogue's guards lost their posts on a revisit because the patrol route was a component its save never wrote down.

Only holding a post is built.
Fantasy-rogue also offered roaming within the room, walking a loop of waypoints, and wandering the floor; each needs the stamp's bounds and orientation carried into the place, and no prefab here needs one yet.

Foundry found the one shape rule a post needs.
Its guard post first had a one-cell door with the brute standing in it, and a sentry that went out after the commando could never get home past it; the mouth is now three cells wide.
That is a property of a piece, not of the engine, and Foundry holds it with a test that every floor cell of a stamped guard post can be walked to from the deck's entry with its guards at home.
It also found a guard standing beside the arrival on about one guard post deck in sixteen, because the start was drawn from anywhere in the first room and a post could be stamped there.
Where the start goes is the chain's, so the fix is a mapgen option rather than a slot rule: `RandomStart.clear_of_stamps(n)` keeps the start `n` cells from every stamped piece, and Foundry asks for seven, its own population's distance from the way in.
A slot that skipped a guard near the arrival instead would have left a post empty on the decks where it mattered most.

## 7. What is checked, and when

At load, `prefab::load` and `role::load` refuse, all at once and each by name: a glyph in a row with no legend entry, rows of unequal width, a legend entry no row uses, a space given a meaning, an unknown tile, prop, tag, monster or role, a band on a fixed item or a fixed monster, a slot naming both or neither of its two fields, a count written backwards, slots with no `ground`, a `ground` nothing can stand on, a role with no members, and a member named twice.

When play begins, `PrefabPlugin` refuses what needs the game's tables, which a file cannot see as it loads: a fixed item the `ItemMaker` has no definition for, a tag nothing in the loot table carries, and a role none of whose members has a weighted row in the spawn table.
Fantasy-rogue found an unknown name only when it spawned the slot, logged a warning and left the cell empty, and an empty draw was skipped with no warning at all, so a typo was a room with nothing in it that no one noticed.

## 8. What was tried and not kept

Fantasy-rogue's prefab system is the alternative, and every lesson here is from it.

- **Coordinate lists beside a tile grid.** See section 1: one thing per cell by construction, where it caught a slot on a wall only at spawn and two slots on one cell never.
- **A copied spawner.** Its prefab monster was spawned by a function that mirrored the ordinary spawner and drifted from it: the ordinary one gave a monster a dodge of six plus its authored bonus plus one per hit die plus its gear, and the copy only the authored bonus plus its gear, so every guard was easier to hit than the same monster in the open. Here the engine calls the game's `ActorMaker::make`, and Foundry answers it with the `spawn_monster` its own population spawns by, so there is one way to build a monster.
- **Unsaved patrol state.** See section 6.
- **Silent empty slots.** See section 7: every name is checked at load or at play, and the coverage report says where a draw falls back or finds nothing.
- **A rarity string doing two jobs.** A prefab's `rarity` was both its placement weight and a bonus to its loot's depth, so a rare room could not be made common without also making its loot worse. Here a slot's offset is the only thing that makes its draw deeper, and how often a piece is placed is the chain's; a room that pays well and bites hard is one whose author wrote high offsets.
- **Stakes, themes or categories on a prefab.** A prefab is what it holds; nothing on it says what kind of room it is.

## 9. Not here

- A prefab pool: per-prefab bands and weights, groups a chain draws from, and a per-place budget. A chain still names its pieces through `StampPrefab` and `StampOneOf`, and the coverage report runs over a band range the game passes until a pool gives each piece its own.
- Roaming within the prefab's bounds and walking a route, which need the stamp's bounds and orientation carried into the place.
- A group slot: a leader and escort drawn together.
- Stamping walls into rock with a connectivity check and rollback, which fantasy-rogue does and no current prefab needs.
- Moving Corsair's vault and Delve's heart into prefab files; both still build their pieces from closures, which stamp exactly as before.
