# Prefab slots: props, items and guards named in data

Status: built on branch `prefab-slots`.
Designed in conversation on 2026-09-24 against `main` at `edadd7e`; where the build moved from the first draft, the body below says what was built.
`docs/design/prefabs.md` is the reasoning as built and `docs/guide/src/systems/prefabs.md` the reference.

## 1. What this is for

A prefab today is a Rust closure from a glyph to a tile, and every other glyph is a mark the game gives meaning to by hand.
Foundry's `place_on_arrival` looks up "armory locker" by name and puts one at every `A`; Delve spawns its heart warden at a `W`; Corsair remaps `$` to a treasure tag.
Each game writes that loop again, and a guard at a mark is a fixed monster, strong on the first floor and harmless on the tenth.

Containers already solved the same problem for items: a contents row says `(tag: "weapon", band: 2)`, and the item is drawn when the container is filled, against the band of wherever it landed.
This work gives a prefab cell the same power for props, items and monsters, so a guarded room keeps its shape and its tension on every floor while what guards it changes.

Done when:

- a prefab is a RON file whose legend maps each glyph to a tile or to one slot: a prop, an item, a monster, or a mark left to the game;
- an item slot names an item or a tag, and a monster slot names a monster or a role, and either may add a band offset;
- roles are their own RON file, listing the monsters that fit each role;
- the engine fills every slot on the first entry to a place, each from a stream derived for its own cell, through the game's makers;
- a monster placed at a slot holds that post: it fights and searches as usual, and walks back when it has nothing better to do;
- every name, tag and role in every prefab and in the roles file is checked at load, or when play begins where the check needs the game's own tables, and all problems are reported together;
- a coverage report shows, for every prefab slot at every band, whether the draw finds something at that band, falls back to another, or finds nothing;
- Foundry's armory, stores and reactor are prefab files, and Foundry has at least one guarded prefab, checked in the running game.

## 2. What was decided

These were settled in conversation and are not reopened here.

1. **One file per prefab, one thing per cell.** The legend maps a glyph to a tile or to a slot, so a guard cannot be placed on a wall and a prop cannot share a cell with a monster, by construction. Fantasy-rogue kept tiles and content in separate coordinate lists, caught a slot on a wall only at spawn by skipping it, and never caught two slots on one cell.
2. **An item slot is the same row a container's contents use.** `Item(tag: "weapon", band: 2)`, `Item(item: "keycard")`, with the same `count` and the same rule that a named item takes no `band`. A tag is how a prefab says "a weapon, armor or a med here".
3. **A monster slot names a monster or a role.** `Monster(monster: "heart warden")` is always that monster and takes no `band`; `Monster(role: "brute", band: 2)` draws from the role at the place's band plus two.
4. **Roles live in their own file**, not in the spawn table. A role is a name and the monsters that fit it, nothing more.
5. **A role draw rolls the spawn table.** It is the game's ordinary monster table, restricted to the rows whose monster is in the role, at the slot's band, with the rows' own weights. Which floors a monster appears on is written once, in the spawn table.
6. **A band with no candidate falls back the way tagged loot does:** to the nearest shallower band that has one, so a slot asking past the table's deepest band gets its deepest, and else to the nearest deeper one. The coverage report is how an author finds out it happened.
7. **A prop slot takes no band.** A container's own contents rows already carry their offsets.
8. **No stakes, themes or categories on a prefab.** A prefab is what it holds; a room that pays well and bites hard is one whose author wrote high offsets.
9. **Only one post: hold.** A monster placed at a slot returns to its cell when idle. Roaming within the room and walking a route are left out until a prefab needs them.
10. **Choosing which prefabs a place gets is out of scope.** The mapgen chain keeps naming its prefabs through `StampPrefab` and `StampOneOf`; a prefab pool with per-band eligibility, weights and a budget is its own spec.
11. **Terrain and contents are two steps with two streams.** Mapgen stamps terrain and records which prefab it stamped; contents are rolled on the first entry, each slot from a stream of its own, `prefab.content` derived from the run's seed and hashed by the map and the slot's cell, so a revisit or a change to one prefab never moves another. Fantasy-rogue learned this from a shared stream that relocated every prefab after the first.
12. **Every slot draws, then decides whether to spawn.** A slot on the arrival cell, on a cell no longer walkable, or on a cell already filled this pass still takes its draw and then spawns nothing, so the rest of the place never depends on where the player arrived.
13. **The piece drawn on top owns its cells.** `PlaceBuild::from_context` drops any mark inside the bounds of a stamp emitted after its own, keyed or not, so a slot of an earlier piece never fills a cell a later piece painted over. No shipped game can see this, since all stamp with `Placement::AnyRoom`, which never overlaps an earlier stamp.
14. **A `Mark` is a legend entry.** A glyph the game gives meaning to itself, such as Foundry's reactor console `R`, is written `Mark` in the legend, so every glyph in a row is accounted for and the file still loads with no game code.

## 3. The prefab file

```ron
// A prefab: a piece of a place, drawn as rows of glyphs, and what each
// glyph stands for.
//
// Every field:
//   name:   the prefab's name, which a mapgen chain asks for
//   ground: the tile painted under every slot; required when the legend
//           has any slot, and whenever it is given, slots or none, it
//           must be a tile a monster can stand on
//           whenever it is given
//   rows:   the piece, one string per row, all the same width; a space is
//           left as the map had it, and every other glyph must be in the
//           legend
//   legend: glyph to what it stands for, one of
//     Tile("name")                          a tile from the game's tiles
//     Prop("name")                          a prop from props.ron
//     Item(item: "item", count: 1)          that item; count is 1 or a
//                                           range, count: (fewest, most),
//                                           default 1
//     Item(tag: "tag", count: 1, band: 0)   drawn from the item table by
//                                           tag at the place's band plus
//                                           band; count is separate draws
//     Monster(monster: "monster")           that monster
//     Monster(role: "role", band: 0)        drawn from the spawn table
//                                           among the role's monsters, at
//                                           the place's band plus band
//     Mark                                  a position the game finds by
//                                           this glyph and fills itself
//   A monster at a slot holds that cell as its post.
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
    "sentry": ["probe droid", "trooper droid"],
    "brute":  ["line droid", "heavy droid"],
}
```

These are Foundry's roles as built.
The first draft had a scrap crab among the brutes and a swarm role of vermin; droids and vermin are hostile to each other, so a post whose brute was a crab had its guards kill each other the moment one woke, and every role is now one faction's.

## 5. How it is built

### 5.1 Crates and types

`rl-rules` owns the file formats, the draws and the coverage report, all testable without an `App`.

- `prefab::PrefabDef`: the name, the ground tile, the rows, the tile legend resolved to `TileId`s, and the slots keyed by glyph.
- `prefab::Slot`: `Prop(PropId)`, `Item(ContentRoll)` reusing the container row as it is, `Monster { pick, band }` where `Pick` is `Kind(Id<M>)` or `Role(RoleId<M>)`, and `Mark`.
- `prefab::load(text, tiles, names) -> Result<PrefabDef, ContentError>` resolves every name in one pass and reports every problem at once, except a fixed item's name, which only the game's `ItemMaker` can resolve and the plugin checks when play begins.
- `role::RoleDef` in a `Registry`, loaded by `role::load`, with every member resolved to a monster id.
- `BandedTable` gains `pick_where` and `band_where`, a pick and a fallback band over only the rows a predicate keeps, so `role::draw` is the spawn table restricted to the role's members, with the same fallback `LootTable::band_for` has: the band asked, else the deepest shallower band with a member, else the shallowest deeper.
- `prefab::coverage(prefabs, &Sources { roles, monsters, items, tags }, bands) -> Coverage`, with a `render()` for a terminal and an `empties()` for a test.

`rl-rules` gains no dependency on `rl-mapgen`.
A `PrefabDef` exposes its rows, `tile(glyph)` and `slot(glyph)`, and `Prefabs::piece` in `rl-bevy` builds the keyed `rl_mapgen::Prefab` from them.

`rl-mapgen` carries identity through the stamp.

- `Prefab::parse_cells` builds a piece from a `Cell` per glyph, `Tile`, `Mark` with the ground to paint under it, or `Clear`; `Prefab::keyed` sets an opaque `u32` key, and `Stamped` gains `prefab: Option<u32>`.
- A prefab parsed from a closure, as Corsair's and Delve's still are, has no key, and its marks behave exactly as today.

`rl-bevy` owns the loop.

- `Spot` gains `prefab: Option<u32>`, filled by `PlaceBuild::from_context`, which drops a mark under a later stamp (decision 13).
- `ActorMaker`, a trait beside `ItemMaker`: `table` (the spawn table), `band` for a place, and `make`, which spawns one monster at a cell on a map. Foundry's `make` is the same `spawn_monster` its own deck population calls, so a guard and a roaming monster of one kind cannot drift apart; fantasy-rogue's copied prefab spawner gave its guards the wrong dodge.
- `Found::Placed`, the item's origin when it is laid at a slot, passed to `ItemMaker::make`.
- `PrefabPlugin<A: ActorMaker, I: ItemMaker>` with a `Prefabs<A::Def>` resource holding every `PrefabDef` and every `RoleDef` behind one shared copy, with `piece(name)` for a chain and `slot(key, glyph)`. It reads `PlaceEntered` with `first`, walks each keyed spot in the order the place recorded them, draws its slot from a stream derived for its own cell, `prefab.content` hashed by map and cell, and spawns the prop, the items as `Found::Placed`, or the monster; a `Mark` draws nothing and is left to the game.
- A startup check in `OnEnter(EngineState::Playing)`, in the manner of `check_containers`: every named item a prefab holds is one the `ItemMaker` knows, every role a prefab asks for has at least one member with a weighted row in the spawn table, and every tag a prefab asks for is carried by the item table.
- Slots are filled in `PrefabSet::Fill`, inside `TurnSet::React` and before `LootSet::Scatter`, so no loot lands under a prefab's prop, and a game's own population goes after it so it can keep off the slots.

### 5.2 Holding a post

- `rl-rules`: a `Posted(Point)` sense and a `KeepPost` tactic. With a post and nothing to do, it steps toward the post, and at the post it waits; beside its post and unable to step onto it, it waits rather than sidestepping. Without a post it returns `None`, so it can sit in every brain.
- `rl-bevy`: a `Post(Point)` component, the sense filled from it by `sense_posts`, and `Post` in `WasLiving`, so it comes off the dead. `rl-save` saves it with the actor. Fantasy-rogue's guards lost their posts on a revisit because the route was not in the snapshot.
- The game puts `KeepPost` in its brains after `SearchLastKnown` and before `Wander`. A guard that loses the player searches where it last saw them, then walks home.
- A guard that has noticed no one has no enemy in its snapshot and nowhere to search, so `KeepPost` is the first tactic that answers, and it stands its post rather than wandering off.

### 5.3 What checks what

At load, `prefab::load` and `role::load` refuse:

- a glyph in `rows` with no legend entry, rows of unequal width, a legend entry no row uses;
- an unknown tile, prop, tag, role or monster name;
- a `band` on a named item or a named monster, and a slot naming both or neither of its two fields;
- slots in the legend and no `ground`, or a `ground` a monster cannot stand on, whether or not there are slots;
- a role with no members, a member that is not a monster, or a member named twice.

At startup, the plugin refuses the cases that need the game's own tables, listed in 5.1, a named item among them.

The coverage report answers what no check can: whether a slot finds anything at the band it will meet.
A column is the place's band, not the slot's; a cell reads `✓` when the draw is exact at the slot's band, `~N` when it falls back to band `N`, and `✗` when nothing can be drawn at all.
Foundry's tables end at deck 10, so the brute and the weapon, two ahead, fall back from deck 9, and the sentry, one ahead, from deck 10; this is `foundry --prefabs` as built.

```
guard post        1   2   3   4   5   6   7   8   9  10
  b brute +2      ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓ ~10 ~10
  s sentry +1     ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓ ~10
  w weapon +2     ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓ ~10 ~10
```

Named slots and prop slots are always exact and are left out of the grid.
Until a prefab pool gives each prefab its own bands, the report runs over a range the game passes, which for Foundry is decks 1 to 10.

## 6. Foundry

- The armory, both stores, the reactor and the core become prefab files under `examples/foundry/assets/prefabs/`, and `decks.rs` stamps them by name. The reactor's and the core's console is a `Mark`, `R`, which Foundry fills itself; the core chamber is an aisle from its hatch to the console, `#####`, `#cRc#`, `#c.c#`, `#c.c#`, `##h##`, since with the console in the middle it walled off the two cells beside it. The tall store's opening was moved off its crate.
- `place_on_arrival` stops placing lockers and crates at marks; the prefabs say so. It keeps the loose crates and cables it places by count.
- `roles.ron` names a sentry role, the probe and trooper droids, and a brute role, the line and heavy droids; no swarm role, and no scrap crab, since droids and vermin are hostile and a mixed post killed itself.
- The guard post, stamped on decks two, four, five, seven and eight, with role slots and a tagged item slot, so the running game shows a guard holding its post and the item it guards. Its mouth is three cells wide: a one-cell door with the brute in it locked its sentries out of their posts once they had gone after the commando.
- `Roster` implements `ActorMaker` with the `spawn_monster` that `populate_deck` spawns by, and `populate_deck` never spawns onto a cell any piece marked.
- Foundry's fingerprint tripwire is re-baselined, since the engine now places the stores' and armories' props and the guards.
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
