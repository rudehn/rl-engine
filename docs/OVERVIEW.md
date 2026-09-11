# rl-engine overview

What exists in the engine, by tier and crate, and what does not yet.
This page is kept current: every slice that adds or removes a system updates it in the same commit.
`docs/PLAN.md` holds the reasoning and the milestone history; this page holds only the inventory.

Last updated: 2026-09-11, after the delve.

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
- Criterion benches on realistic maps.

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

### rl-content

- `Registry<T>` loaded from RON with validate-on-load.
- `BandedTable` with weights, groups and gap detection.

### rl-rules

- Stats with a modifier accumulator.
- The damage pipeline: kinds, resistances, hits with the attacker and credit split, composable stages.
- Statuses with stacking rules, per-turn ticks and cures.
- A faction relation matrix.
- The equipment slot graph with displacement.
- The affix and enchant model: item tags, prefix and suffix affixes with level-scaled stat grants and extra strikes, an enhance rule for what a level buys, per-instance state, weighted rolling.

### rl-ai

- Movement profiles.
- Snapshots of what an actor sees.
- A tactic-priority brain with melee, flee-when-hurt, hunt and wander tactics.

### rl-events

- Facts with kind, subject, object and amount, and matchers.
- A ledger of named counters.
- Quests as objectives over facts, with prerequisite chains and a victory flag.

### rl-tools

- Threat scoring and the spawn-band balance report.

### rl-test-support

- ASCII map fixtures, renders, seed sweeps, a neighbourhood builder.

## Tier 2: the Bevy layer

### rl-bevy

- The engine-owned loop: a `Turn` schedule run as many passes per frame as it takes, input once per frame, refusals that cost nothing, stall recovery.
- Actions: move, attack, wait, pick up, drop, equip, unequip, use, enter.
- Chunk streaming with edit deltas, per-map occupancy and knowledge, field of view.
- Combat: health, armor, resists, factions, melee and ranged attacks down a line of fire, extra strikes, the damage event pipeline, deaths that linger until the frame ends, flow fields per movement profile feeding the minds.
- Items on the ground, in bags and in slots, with stacks and enchantments.
- Places: bounded maps entered by transitions or warps, built on first arrival, kept whole, off-map actors frozen; `PlaceBuild::from_context` reads a finished chain; `WarpRequest::into_place` starts a run in one.
- The surface is optional: a game with no world graph streams nothing and lives in its places.
- Statuses ticked by the turn through the damage pipeline.
- Facts fed to quests and counters after the frame.
- Save exports for the scheduler, the world's edits and places, and knowledge.
- A headless app for tests.

### rl-render

- A diffed terminal back buffer.
- The map view with lit, remembered and unknown tiles.
- Glyph entities filtered to the current map.

### rl-ui

- Theme tokens.
- A categorised message log.
- The chrome plugin: status line and log.
- A framed scrolling list menu.

### rl-overworld

- The world drawn from its bands with rivers, roads, discovered sites and the player.
- A portal picker.

### rl-save

- Backends for files, memory and browser storage behind one resource.
- An exact-match versioned envelope.
- Entity remapping.
- The engine's own state captured and restored.

## Tier 3: rl-engine

- The facade re-exporting every crate.

## The second example: the Hollow Whale

Five floors of a beached leviathan, mouth to heart, with no surface: a cave with teeth, a BSP gullet, a stomach of rooms pooled with bile, a bone-walled ribcage, and a prefab heart chamber with a warden whose death wins the run.
`floors.rs` is the whole map builder; it is the test that a dungeon delve is first-class.

## The worked example: Corsair

Islands from the world graph, ports with huts, a bestiary, armory, affixes, statuses and quests from RON, caves with a treasure vault, a pistol, a ledger, saving and continuing, and a balance report.
It is built only on the public API, so it is the test that the seams are right.

## Not built yet

- Cursed items: nothing resists removal or carries a deliberate penalty.
- Throwing.
- A character sheet.
- Abilities as data over the targeting shapes.
- Tile fields for fire and gas.
- Lighting.
- Scripted encounters.
- The wasm unload bridge.
- Seed replay.
- Instanced terminal rendering; one sprite per cell is the known scaling limit.
