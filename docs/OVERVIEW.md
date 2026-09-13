# rl-engine overview

What exists in the engine, by tier and crate, and what does not yet.
This page is kept current: every slice that adds or removes a system updates it in the same commit.
`docs/PLAN.md` holds the reasoning and the milestone history; this page holds only the inventory.

Last updated: 2026-09-12, after the panels.

## The shape

Four tiers.
Tiers 0 and 1 never depend on Bevy and build for wasm; CI enforces both.
Tier 2 is the Bevy layer that runs the loops.
Tier 3 is the facade a game depends on.
Content is never named in the engine: tiles, damage kinds, stats, statuses, factions, slots, tags, affixes, facts, quests and bands are all opaque ids in registries the game fills, and behaviour is taken as traits and closures.

## Tier 0: rl-core

- Geometry: `Point`, `Rect`, `Direction` and `DirectionSet`, `Grid<T>` behind the `Grid2D` trait, Bresenham lines, discs, squares, cones, Chebyshev and Euclidean distance.
- `DisjointSet`.
- Seeds: `RunSeed`, `SeedDomain`, derived streams, position and pair hashes for seams.
- `DiceRoll`, parsed from `NdS+B` at content load.
- `Id<T>` typed dense ids with an interner.
- Quantile and rank helpers.
- `TurnQueue<Id>`: an integer clock scheduler with peek, pop, batch dequeue, entries for saving, and speed-scaled costs.

## Tier 1: no Bevy

### rl-grid

- Tile registry with flag tables, `Terrain` and views, `OpacitySource` and `CostSource` traits.
- `BitGrid`.
- Symmetric shadowcasting field of view.
- Region flood and labelling.
- A* with scratch buffers.
- Region-bounded `DijkstraMap` with scale and rescan for flee maps.
- `SpatialGrid`.
- Targeting: own, adjacent, bolt, ball, beam and cone shapes resolved to a footprint against a caller-named blocker, and `clear_shot`.
- Light: `Light` as an intensity, a landed colour and a waver the renderer alone reads, `Emitter` with a flicker, integer falloff to zero at the rim, screen blending, and a `LightField` cast through the shadowcast in an order-independent way, with a flood for glowing areas and a compose over two layers and an ambient.
- Criterion benches on realistic maps, lighting at twenty sources included.

### rl-mapgen

- The pass chain: named passes keying their own RNG stream, enforced phase order, fallible passes, typed outputs, snapshots.
- Passes: fill, border, scatter, scatter by a per-cell density, cellular cave, keep largest region, central start.
- Dungeon passes: rooms, BSP, doors, random start, farthest exit.
- Prefab stamping from ASCII with a legend and marks, placed at a point, centred, or in a room.

### rl-world

- FBM noise over a sample space.
- Elevation with quantile banding and a continuous height field.
- Priority-flood hydrology with lakes and rivers as a share of land.
- Climate: temperature, moisture, distance to water.
- Cell facts, relief, and a classifier hook the game fills.
- Scored site placement with clearances.
- A turn-cost road router with a distance field.
- The world graph and its rules trait.
- Chunks built from a neighbourhood with a seam hash so edges agree, road and river passes routed by A*.
- Per-tile sampling: warped bilinear climate, a clump field, `tile_facts`.
- A headless `world_dump` example.

### rl-rules

One crate, in modules: crate boundaries follow dependency weight, and content, rules, AI, events and tools all weighed the same.

- `content`: `Registry<T>` loaded from RON with validate-on-load, and `BandedTable` with weights, groups and gap detection.
- Stats with a modifier accumulator.
- The damage pipeline: kinds, resistances, hits with the attacker and credit split, composable stages.
- Statuses with stacking rules, per-turn ticks and cures.
- A faction relation matrix.
- The equipment slot graph with displacement.
- The affix and enchant model: item tags, prefix and suffix affixes with level-scaled stat grants and extra strikes, an enhance rule for what a level buys, per-instance state, weighted rolling.
- `ai`: movement profiles, snapshots of what an actor sees, and a tactic-priority brain with melee, flee-when-hurt, hunt and wander tactics.
- `events`: facts with kind, subject, object and amount, matchers, a ledger of named counters, and quests as objectives over facts with prerequisite chains and a victory flag.
- `balance`: threat scoring and the spawn-band report.
- `forecast`: what a fight is likely to cost, with the average roll put through the game's own mitigation pipeline in place of a real one; blows and turns to fell either side, and an `Outlook` read off the two counts. Pure, so an inspect panel's numbers cannot drift from the fight.

## Tier 2: the Bevy layer

### rl-bevy

- One plugin per subsystem, each opt-in: `CorePlugin` holds the loop, the map and its places; field of view, combat, statuses, items, lighting, streaming and facts are added by name. A plugin says what it needs, so a missing rule table or a missing plugin it depends on panics naming both, rather than a subsystem quietly doing nothing all run.
- The engine-owned loop: a `Turn` schedule run as many passes per frame as it takes, input once per frame, refusals that cost nothing, stall recovery.
- A mind may choose an action the engine has never heard of: a tactic returns a number of the game's own, the engine reports it as `MindChose`, and the game answers it in `DecideSet::Game`. A game's tactic can sit anywhere in the priority list beside the engine's.
- A reaction phase inside the turn: `TurnSet::React` runs after the actions of a pass resolve and before the turn is requeued, which is where a game answers what just happened. A drink heals before the next blow lands, a bite poisons on the bite, gear counts from the moment it is worn.
- Drawing is layered by `PresentSet`: narration, then the map, then the chrome, then whatever covers them. No crate orders itself after another crate's draw function.
- Actions are types, not a list: `Step`, `Attack`, `Wait`, `PickUp`, `DropItem`, `Equip`, `Unequip`, `UseItem` and `GoThrough` ship with the engine, each resolved by the module that owns the mechanic. A game registers its own with `add_action`, resolves it in `TurnSet::Resolve` by claiming the actor and reporting a cost, and the sweep refuses whatever no resolver claimed.
- Chunk streaming with edit deltas, per-map occupancy and knowledge, field of view.
- Combat: health, armor, resists, factions, melee and ranged attacks down a line of fire, extra strikes, the damage event pipeline, deaths that linger until the frame ends, flow fields per movement profile feeding the minds.
- Items on the ground, in bags and in slots, with stacks and enchantments.
- Places: bounded maps entered by transitions or warps, built on first arrival, kept whole, off-map actors frozen; `PlaceBuild::from_context` reads a finished chain; `WarpRequest::into_place` starts a run in one.
- The surface is optional, and everything regional belongs to it: `WorldMap::new` takes the tile tables alone, the region size is read from the world graph when the first window loads, and `Knowledge` is initialised by the engine. A delve names neither.
- Knowledge: explored tiles per map in buckets of its own, and the surface's seen regions and discovered sites kept apart from them, so going underground never hides the overworld's fog.
- Statuses ticked by the turn through the damage pipeline.
- Lighting, opt-in by inserting `Lighting`: `LightSource` on a prop, an actor or an item, shed from the carrier once carried; static and dynamic layers recast only when their sources change; the map's `opacity_epoch` so an edit that changes what blocks sight refreshes light and every viewshed without anyone moving; `DarkSight`; `Fuel` ticked by the turn with `LightEvent::BurntOut`; the viewshed keeps its geometric `line` and its seen `visible`, and minds perceive along a line only what is lit, within their dark sight or adjacent.
- Facts fed to quests and counters after the frame.
- Save exports for the scheduler, the world's edits and places, and knowledge.
- A headless app for tests.

### rl-render

- A diffed terminal back buffer.
- The map view with lit, remembered and unknown tiles.
- Shading, in the manner of Brogue: each tile authored with both colours and a `Vary` that jitters every cell by a hash of its position and can shimmer over time; light multiplies glyph and background channel by channel, down to a dark floor and up to a gain cap; the wavering part of a light dips on a smooth noise so flames ripple; `Memory` fades what was seen to a darker, greyer, cooler colour.
- `LightOverlay`: intensity drawn as digits.
- `CapturePlugin`: `RL_CAPTURE` plays `RL_CAPTURE_KEYS` through the real keyboard input, photographs the window without taking focus, refuses a black frame, and exits.
- Glyph entities filtered to the current map.

### rl-ui

Every panel splits three ways: a view (a resource of plain data), a collector (the system that rebuilds it each frame, in `ViewSet::Collect`), and a presenter (one way of drawing it, in a `PresentSet` layer).
The query is the half a game reuses; the drawing is the half it may replace or drop.
Opt-in is per panel, and a presenter pulls its view plugin in behind it.

- Tones: a semantic role interned as a `ToneId` over a `Palette` a game extends with roles the engine never heard of, warned about by name at startup if one has no colour. No widget takes a `Color`.
- Facets: `Facet { key, text, tone }` pushed onto a row in `ViewSet::Annotate`, so a game's vocabulary reaches a panel without an engine type learning a word.
- Views and their collectors: `NearbyView` (actors and things in the viewshed, nearest first, with health and a relation), `VitalsView` (bars, armor, status badges, turn, position), `GearView` (every registered slot, filled or not), `InspectView` (the look cursor's subject and a duel forecast).
- Panels: `NearbyPanel`, `VitalsPanel`, `GearPanel`, `InspectPanel`, `LogPanel`, each a plugin holding its rectangle and its headings.
- The look cursor: opens on the nearest actor, steps with the direction keys, cycles what is in sight, stays inside the loaded window, and owns input as a modal.
- `Modals`: a stack of interned modal ids with `modal_is`, `modal_open` and `no_modal` run conditions, so one gate covers every screen a game adds.
- `DirectionKeys`: arrows, vi keys and the numpad to the eight directions, in one resource a game may replace.
- A message log carrying a tone per line, folding a repeat into a count, filterable by tone.
- A framed scrolling list menu.
- Drawing helpers a game writing its own presenter reuses: frames, section headings, bars, clipping, and rectangle splits.

### rl-overworld

- Says what it needs: no layout, no screen, and it says so when play begins rather than drawing nothing.
- Open or closed on the shared `Modals` stack, so it and a game's own screens cannot both think they own the arrow keys.
- The world drawn from its bands with rivers, roads, discovered sites and the player.
- A portal picker.

### rl-save

- Backends for files, memory and browser storage behind one resource.
- An exact-match versioned envelope.
- Entity remapping.
- The engine's own state captured and restored.

## Tier 3: rl-engine

- The facade re-exporting every crate.
- A prelude worth globbing: core, grid, mapgen, world, rules, the Bevy layer, render, UI, overworld and save, in one `use`. It leaves out `Rect`, because Bevy's prelude has one of its own and a game that globs both would have to disambiguate every use; a doc test globs both preludes and names a type from each crate, so the next collision fails there rather than in someone's game.

## The guide

`docs/guide` is an mdBook that builds a small roguelike, Warren, in nine steps: a map on screen, walking, sight and memory, monsters, blows, items, floors, content in RON, and an action of the game's own, ending with the headless tests.
Each step is a runnable binary in `examples/tutorial/src/bin`, so every chapter's code is compiled by CI and can be played on its own; the chapters quote the sources through mdBook anchors rather than restating them, and `scripts/check-guide.sh` fails the build if an anchor, an image or a table-of-contents entry stops resolving.

## The third example: Lamplight

One dark cave: a lantern that burns oil and is lit or doused with a use action, a brazier, wisps that glow and drift, fungus that glows, pools that shimmer, a torch on the floor, and lurkers with dark sight and no glow.
`main.rs` is the whole game; it is the test that lighting is one resource and one component away.

## The second example: the Hollow Whale

Five floors of a beached leviathan, mouth to heart, with no surface and, below the Maw's grey daylight, no light but a brand, the bile and whatever a beast sheds: a cave with teeth, a BSP gullet, a stomach of rooms pooled with bile, a bone-walled ribcage, and a prefab heart chamber with a warden whose death wins the run.
`floors.rs` is the whole map builder; it is the test that a dungeon delve is first-class.

## The worked example: Corsair

Islands in daylight from the world graph, dark caves lit by the player's lantern and the smugglers' own, ports with huts, a bestiary, armory, affixes, statuses and quests from RON, caves with a treasure vault, a pistol, a ledger, saving and continuing, and a balance report.
It is built only on the public API, so it is the test that the seams are right.

## Not built yet

- Cursed items: nothing resists removal or carries a deliberate penalty.
- Throwing.
- A character sheet.
- Abilities as data over the targeting shapes.
- Tile fields for fire and gas, and the glow they would shed.
- Nights on Corsair's open water, and a lantern the player can douse or run out of.
- Lit detection ranges, a light-averse tactic, and a ranged penalty in the dark once accuracy exists.
- Scripted encounters.
- The wasm unload bridge.
- Seed replay.
- Instanced terminal rendering; one sprite per cell is the known scaling limit.
