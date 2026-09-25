# Prefab slots: props, items and guards named in data

Status: design, agreed in conversation on 2026-09-24 against `main` at `edadd7e`.
Nothing here is built yet.

## 1. What this is for

A prefab today is a Rust closure from a glyph to a tile, and every other glyph is a mark the game gives meaning to by hand.
Foundry's `place_on_arrival` looks up "armory locker" by name and puts one at every `A`; Delve spawns its heart warden at a `W`; Corsair remaps `$` to a treasure tag.
Each game writes that loop again, and a guard at a mark is a fixed monster, strong on the first floor and harmless on the tenth.

Containers already solved the same problem for items: a contents row says `(tag: "weapon", band: 2)`, and the item is drawn when the container is filled, against the band of wherever it landed.
This work gives a prefab cell the same power for props, items and monsters, so a guarded room keeps its shape and its tension on every floor while what guards it changes.

Done when:

- a prefab is a RON file whose legend maps each glyph to a tile or to one slot: a prop, an item, or a monster;
- an item slot names an item or a tag, and a monster slot names a monster or a role, and either may add a band offset;
- roles are their own RON file, listing the monsters that fit each role;
- the engine fills every slot on the first entry to a place, from its own stream, through the game's makers;
- a monster placed at a slot holds that post: it fights and searches as usual, and walks back when it has nothing better to do;
- every name, tag and role in every prefab and in the roles file is checked at startup, and all problems are reported together;
- a coverage report shows, for every prefab slot at every band, whether the draw finds something at that band, falls back to another, or finds nothing;
- Foundry's armory, stores and reactor are prefab files, and Foundry has at least one guarded prefab, checked in the running game.

## 2. What was decided

These were settled in conversation and are not reopened here.

1. **One file per prefab, one thing per cell.** The legend maps a glyph to a tile or to a slot, so a guard cannot be placed on a wall and a prop cannot share a cell with a monster, by construction. Fantasy-rogue kept tiles and content in separate coordinate lists and needed spawn-time checks for both mistakes.
2. **An item slot is the same row a container's contents use.** `Item(tag: "weapon", band: 2)`, `Item(name: "keycard")`, with the same `count` and the same rule that a named item takes no `band`. A tag is how a prefab says "a weapon, armor or a med here".
3. **A monster slot names a monster or a role.** `Monster(name: "heart warden")` is always that monster and takes no `band`; `Monster(role: "brute", band: 2)` draws from the role at the place's band plus two.
4. **Roles live in their own file**, not in the spawn table. A role is a name and the monsters that fit it, nothing more.
5. **A role draw rolls the spawn table.** It is the game's ordinary monster table, restricted to the rows whose monster is in the role, at the slot's band, with the rows' own weights. Which floors a monster appears on is written once, in the spawn table.
6. **A band with no candidate falls back the way tagged loot does:** to the nearest shallower band that has one, so a slot asking past the table's deepest band gets its deepest, and else to the nearest deeper one. The coverage report is how an author finds out it happened.
7. **A prop slot takes no band.** A container's own contents rows already carry their offsets.
8. **No stakes, themes or categories on a prefab.** A prefab is what it holds; a room that pays well and bites hard is one whose author wrote high offsets.
9. **Only one post: hold.** A monster placed at a slot returns to its cell when idle. Roaming within the room and walking a route are left out until a prefab needs them.
10. **Choosing which prefabs a place gets is out of scope.** The mapgen chain keeps naming its prefabs through `StampPrefab` and `StampOneOf`; a prefab pool with per-band eligibility, weights and a budget is its own spec.
11. **Terrain and contents are two steps with two streams.** Mapgen stamps terrain and records which prefab it stamped; contents are rolled on the first entry from their own stream, so a revisit or a change to one prefab never moves another. Fantasy-rogue learned this from a shared stream that relocated every prefab after the first.
12. **Every slot draws, then decides whether to spawn.** A slot on the arrival cell or on a cell something already holds still takes its draw and then spawns nothing, so the rest of the place never depends on where the player arrived.

## 3. The prefab file

```ron
// A prefab: a piece of a place, drawn as rows of glyphs, and what each
// glyph stands for.
//
// Every field:
//   name:   the prefab's name, which a mapgen chain asks for
//   ground: the tile painted under every slot; required when the legend
//           has any slot, and it must be a tile a monster can stand on
//   rows:   the piece, one string per row, all the same width; a space is
//           left as the map had it, and every other glyph must be in the
//           legend
//   legend: glyph to what it stands for, one of
//     Tile("name")                          a tile from the game's tiles
//     Prop("name")                          a prop from props.ron
//     Item(name: "item", count: 1)          that item; count is a number
//                                           or a range, default 1
//     Item(tag: "tag", count: 1, band: 0)   drawn from the item table by
//                                           tag at the place's band plus
//                                           band; count is separate draws
//     Monster(name: "monster")              that monster
//     Monster(role: "role", band: 0)        drawn from the spawn table
//                                           among the role's monsters, at
//                                           the place's band plus band
//   A monster at a slot holds that cell as its post.
(
    name: "guarded locker",
    ground: "deck",
    rows: [
        "#######",
        "#s.A.s#",
        "#..w..#",
        "##.b.##",
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

A mapgen chain asks for a prefab by name and stamps it with the `Placement` and `Orient` it already chooses; orientation stays a property of the stamp, not of the file.
Slot glyphs travel with rotation and mirroring because they are marks, which `Prefab::rotated` and `Prefab::flipped` already move with the tiles.

## 4. The roles file

```ron
// Roles: which monsters fit which part in a prefab.
//
// A role is a name a prefab's `Monster(role: ...)` slot asks for, and the
// monsters that fit it. Where and how often each monster turns up is the
// spawn table's; a role only says who may stand in the slot. A monster may
// fit several roles.
{
    "sentry": ["trooper droid", "probe droid"],
    "brute":  ["heavy droid", "scrap crab", "line droid"],
    "swarm":  ["coolant rat", "lamp moth"],
}
```

## 5. How it is built

### 5.1 Crates and types

`rl-rules` owns the file formats, the draws and the coverage report, all testable without an `App`.

- `prefab::PrefabDef`: the name, the ground tile, the rows, the tile legend resolved to `TileId`s, and the slots keyed by glyph.
- `prefab::Slot`: `Prop(PropId)`, `Item(ContentRoll)` reusing the container row as it is, and `Monster(Pick, i32)` where `Pick` is `Kind(K)` or `Role(RoleId)`.
- `prefab::load(text, tiles, names, ...) -> Result<PrefabDef, ContentError>` resolves every name in one pass and reports every problem at once.
- `role::RoleDef` in a `Registry`, loaded by `role::load`, with every member resolved to a monster id.
- `BandedTable` gains a filtered pick and a filtered `band_for`, so a role draw is the spawn table restricted to the role's members, with the same fallback `LootTable::band_for` has.
- `prefab::coverage(prefabs, roles, monsters, items, bands) -> Coverage`, with a `render()` for a terminal and an `empties()` for a test.

`rl-rules` gains no dependency on `rl-mapgen`.
A `PrefabDef` hands the chain its rows and a legend closure, and the game builds the `rl_mapgen::Prefab` from them as it does today.

`rl-mapgen` carries identity through the stamp.

- `Prefab` gains an optional key, an opaque `u32` set by whoever parsed it, and `Stamped` gains `prefab: Option<u32>`.
- A prefab parsed from a closure, as Corsair's and Delve's still are, has no key, and its marks behave exactly as today.

`rl-bevy` owns the loop.

- `Spot` gains `prefab: Option<u32>`, filled by `PlaceBuild::from_context`.
- `ActorMaker`, a trait beside `ItemMaker`: `id_of`, `table` (the spawn table), `band` for a place, and `make`, which spawns one monster at a cell. Foundry's own deck population calls the same `make`, so a guard and a roaming monster of one kind cannot drift apart; fantasy-rogue's copied prefab spawner gave its guards the wrong dodge.
- `Found::Placed`, the item's origin when it is laid at a slot, passed to `ItemMaker::make`.
- `PrefabPlugin<A: ActorMaker, I: ItemMaker>` with a `Prefabs` resource holding every `PrefabDef` and the `Roles`. It reads `PlaceEntered` with `first`, walks each keyed spot in the order the place recorded them, draws its slot from the stream `prefab.content` indexed by the map, and spawns the prop, the items or the monster.
- A startup check in `OnEnter(EngineState::Playing)`, in the manner of `check_containers`: every role a prefab asks for exists and has at least one member in the spawn table at some band, and every tag a prefab asks for is carried by the item table.
- Props placed at slots are spawned before `PropSet::Fill`, so a container in a prefab is stocked the same turn as any other.

### 5.2 Holding a post

- `rl-rules`: a `Posted { at }` sense and a `KeepPost` tactic. With a post and nothing to do, it steps toward the post, and at the post it waits. Without a post it returns `None`, so it can sit in every brain.
- `rl-bevy`: a `Post(Point)` component, saved with the actor, and the sense filled from it. Fantasy-rogue's guards lost their posts on a revisit because the route was not in the snapshot.
- The game puts `KeepPost` in its brains after `SearchLastKnown` and before `Wander`. A guard that loses the player searches where it last saw them, then walks home.
- A guard is spawned `Unaware`, and an unaware guard runs its lowest tactic that fires, which is `KeepPost`, so it stands its post rather than freezing or wandering off.

### 5.3 What checks what

At load, `prefab::load` and `role::load` refuse:

- a glyph in `rows` with no legend entry, rows of unequal width, a legend entry no row uses;
- an unknown tile, prop, item, tag, role or monster name;
- a `band` on a named item or a named monster;
- slots in the legend and no `ground`, or a `ground` a monster cannot stand on;
- a role with no members, or a member that is not a monster.

At startup, the plugin refuses the cases that need two tables at once, listed in 5.1.

The coverage report answers what no check can: whether a slot finds anything at the band it will meet.
A column is the place's band, not the slot's; a cell reads `✓` when the draw is exact at the slot's band, `~N` when it falls back to band `N`, and `✗` when nothing can be drawn at all.
In the example, the tables end at band 10, so every `+2` slot on the last two decks draws from band 10.

```
guarded locker   1   2   3   4   5   6   7   8   9   10
  s sentry +1    ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓   ~10
  b brute  +2    ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓   ~10 ~10
  w weapon +2    ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓   ~10 ~10
```

Named slots and prop slots are always exact and are left out of the grid.
Until a prefab pool gives each prefab its own bands, the report runs over a range the game passes, which for Foundry is decks 1 to 10.

## 6. Foundry

- The armory, both stores, the reactor and the core become prefab files under `examples/foundry/assets/prefabs/`, and `decks.rs` stamps them by name.
- `place_on_arrival` stops placing lockers and crates at marks; the prefabs say so. It keeps the loose crates and cables it places by count.
- `roles.ron` names at least a sentry, a brute and a swarm role over the existing roster.
- At least one new guarded prefab, stamped on some decks by the chain, with role slots and a tagged item slot, so the running game shows a guard holding its post and the item it guards.
- `Roster` implements `ActorMaker`, and `populate_deck` spawns through `make` and never onto a cell a guard already holds.
- A `--prefabs` flag prints the coverage report, beside Corsair's `--balance`, and a test asserts it has no `✗`.

Corsair and Delve keep their closure-built prefabs, which still work unchanged.

## 7. Tests

- Every Foundry prefab slot, on every deck, over a range of seeds, is filled, or is skipped only because its cell was the arrival cell or already held.
- A prefab's contents are the same whichever cell the player arrived on, apart from a slot on that cell.
- A role draw never returns a monster outside the role, and at a band with members never falls back.
- A guard that loses the player searches, then returns to its post, and stands there.
- A guard's post survives a save and a load.
- Each refusal in 5.3 has a test naming the problem it reports.
- A keyless prefab stamps exactly as before: the existing mapgen and Corsair fingerprints hold.

## 8. Documentation

This is a new subsystem, so it pays in full in the commit that lands it.

- `docs/design/prefabs.md`: why slots are a legend, why roles are their own file and roll the spawn table, why contents have their own stream, and fantasy-rogue's lessons as the rejected alternatives. Listed in `CLAUDE.md`'s layout and `docs/README.md`.
- `docs/guide/src/systems/prefabs.md`: the system page, with a manifest over the files in 5.1.
- `docs/guide/src/systems/minds.md`: `KeepPost` and `Post`, re-blessed.
- `docs/OVERVIEW.md`: `PrefabPlugin` in the plugin table, and the new types.
- `CHANGELOG.md`: `Found::Placed` breaks an exhaustive match on `Found`; `Spot` and `Stamped` gain a field.
- `README.md`: prefabs as data, with guards drawn by role and depth.

## 9. Left for later

- A prefab pool: per-prefab bands and weights, groups a chain draws from, and a per-place budget.
- Roaming within the prefab's bounds and walking a route, which need the stamp's bounds and orientation carried into the place.
- A group slot: a leader and escort drawn together.
- Stamping walls into rock with a connectivity check and rollback, which fantasy-rogue does and no current prefab needs.
- Moving Corsair's vault and Delve's heart into prefab files.
