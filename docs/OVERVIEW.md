# rl-engine overview

What exists in the engine, by tier and crate, and what does not yet.
This page is kept current: every slice that adds or removes a system updates it in the same commit.
`docs/PLAN.md` holds the reasoning and the milestone history; this page holds only the inventory.

Last updated: 2026-09-13, after combat stood alone.

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
- The damage pipeline: kinds, resistances, hits with the attacker and credit split, composable stages. A negative hit mends down the same stages, past armor and a block and scaled by resistance to its kind; whoever rolls a blow floors it at zero.
- `names`: `Names`, the registries a content file's names resolve against, borrowed for a load and filled with whichever a game has. Every loader below reads through it and reports every unknown name in a file at once, saying which registry was missing when one was never given, so no game writes a lookup or mirrors an engine schema in a type of its own.
- Statuses with stacking rules, per-turn ticks and cures, loaded by name with `status::load`: a stat to modify and a damage kind to tick, each resolved to a typed id.
- A faction relation matrix.
- The equipment slot graph with displacement.
- The affix and enchant model: item tags, prefix and suffix affixes with level-scaled stat grants and extra strikes, an enhance rule for what a level buys, per-instance state, weighted rolling. `affix::load` reads affixes by tag, stat and damage kind name.
- `ability`: what an actor can spend a turn on besides a step and a swing, as data. A shape from the targeting module, an [`Aim`] saying what it wants under it, costs against a pool, an item charge, health or a tagged item, requirements over statuses, slots and stats, an integer time and cooldown, and a list of named effects with their arguments left unparsed for whoever registered them. `load` resolves every name in a file through `Names`; `blocked` answers whether a use is permitted and lists every reason it is not. `Aim::hits` says who a footprint catches, the user counting as its own ally, and `Aim::worth_aiming_at` what is worth pointing it at; `aim_blocked` refuses an aim with nowhere to go or out of the user's sight.
- `ai`: movement profiles, snapshots of what an actor sees, and a tactic-priority brain with melee, flee-when-hurt, hunt, wander and use-ability tactics. The ability tactic aims by `Aim::worth_aiming_at` and scores a footprint by `Aim::hits`, the rules the resolver lands it with, so a mind fires something it cannot understand, never learns the theme, and never counts a hit the resolver would not land.
- `events`: facts with kind, subject, object and amount, matchers, a ledger of named counters, and quests as objectives over facts with prerequisite chains and a victory flag.
- `balance`: threat scoring and the spawn-band report.
- `ai::awareness`: `NoticeStats` (a certain radius, a chance beyond it, a light bonus and a memory) and `StealthStats`, the pure `notices` roll, and `Awareness`, which goes `Unaware` to `Alert` on a sighting and back once its memory runs out; plus the `SearchLastKnown` tactic, which walks to where an enemy was last seen.
- `forecast`: what a fight is likely to cost, with the average roll put through the game's own mitigation pipeline in place of a real one; blows and turns to fell either side, and an `Outlook` read off the two counts. Pure, so an inspect panel's numbers cannot drift from the fight.

## Tier 2: the Bevy layer

### rl-bevy

- One plugin per subsystem, each opt-in: `CorePlugin` holds the loop, the map and its places; field of view, combat, minds, statuses, items, lighting, streaming and facts are added by name. A plugin says what it needs with `app.needs::<R>(plugin, hint)`, and on entering play one check lists every missing piece at once with how to make it, `CorePlugin`'s own `WorldMap` included, rather than a subsystem, or the whole frame, quietly doing nothing all run. A plugin that depends on another checks in `finish`, so the order a game lists its plugins in never matters, and a world built while play never began is warned about.
- The engine-owned loop: a `Turn` schedule run as many passes per frame as it takes, input once per frame, refusals that cost nothing, stall recovery.
- A mind may choose an action the engine has never heard of: a tactic returns a number of the game's own, the engine reports it as `MindChose`, and the game answers it in `DecideSet::Game`. A game's tactic can sit anywhere in the priority list beside the engine's.
- A reaction phase inside the turn: `TurnSet::React` runs after the actions of a pass resolve and before the turn is requeued, which is where a game answers what just happened. A drink heals before the next blow lands, a bite poisons on the bite, gear counts from the moment it is worn.
- Drawing is layered by `PresentSet`: narration, then the map, then the chrome, then whatever covers them. No crate orders itself after another crate's draw function.
- Actions are types, not a list: `Step`, `Attack`, `Wait`, `PickUp`, `DropItem`, `Equip`, `Unequip`, `UseItem` and `GoThrough` ship with the engine, each resolved by the module that owns the mechanic. A game registers its own with `add_action`, resolves it in `ResolveSet::Act` through `Resolution`, and the sweep refuses whatever no resolver claimed. `Resolution` is every resolver's side of the loop, the engine's and a game's: `claim` the actor holding the turn, then `done` with a cost or `failed`, which keeps the player's turn and charges anyone else, so no resolver has to remember that a monster handed a free retry loops forever.
- Every stage a plugin fills is a named set, and no system orders itself after another's function: `DecideSet::{Notice, Offer, Minds, Game}`, `ResolveSet::{Travel, Act, Effects, Damage}` with steps, waits and warps travelling before anything else acts, and `CleanupSet::{Remove, Requeue}` taking the dead out before a turn is requeued. The dead are buried in `Last`.
- Chunk streaming with edit deltas, per-map occupancy and knowledge, field of view.
- Combat: health, armor, resists, factions, melee and ranged attacks down a line of fire, extra strikes, the damage event pipeline, and deaths that linger until the frame ends. It knows nothing of minds or abilities: a blow is a blow whoever chose it.
- Minds, opt-in by adding `MindsPlugin` beside combat: `Mind`, `Perception` and `Profile` on a monster, flow fields per movement profile, the snapshot a brain reads, and the one system that turns a decision into a step, an attack, a wait, an ability or a `MindChose`. The one place every action a monster can choose meets, so neither combat nor abilities has to know about the other; a `Mind` spawned without the plugin is reported once rather than left standing.
- Items on the ground, in bags and in slots, with stacks, tags and enchantments.
- Abilities, opt-in, with every actor given an empty `Known`, `Pools` and `Cooldowns` as it is spawned, so `Grants` alone is enough to use what was granted: the `Use` action, `Known` rebuilt every turn from what an actor is and wears, `Pools` for whatever a game calls its fuel, `Cooldowns` as absolute times on the turn clock so a save restores them for nothing, `Grants` and `Charges` on the things that lend an ability, `Offered`, the gate's answer for whoever holds the turn, which the minds and a menu both read through accessors that answer only for that actor, and a resolver that gates, pays, resolves the footprint and lands the effects inside the pass. `Bystanders::land` is the one answer to where a use lands, who it hits and why the aim would be refused; the resolver and the targeting preview both call it. Effects are types, not a list: one per subsystem the engine owns, all seven in `effects`, which depends on combat and statuses so neither depends on abilities, and a game registers its own with `add_effect`. An effect asks for damage, a status or a move through `EffectWorld`, and reaches anything else through `Commands`.
- Places: bounded maps entered by transitions or warps, built on first arrival, kept whole, off-map actors frozen; `PlaceBuild::from_context` reads a finished chain; `WarpRequest::into_place` starts a run in one.
- The surface is optional, and everything regional belongs to it: `WorldMap::new` takes the tile tables alone, the region size is read from the world graph when the first window loads, and `Knowledge` is initialised by the engine. A delve names neither.
- Knowledge: explored tiles per map in buckets of its own, and the surface's seen regions and discovered sites kept apart from them, so going underground never hides the overworld's fog.
- Statuses ticked by the turn through the damage pipeline. `StatusPlugin` gives every actor an empty `Afflicted` and `StatBlock` as it is spawned, so a monster spawned without them still takes a status, and a game without statuses carries neither.
- Every actor requires a `Speed`, normal unless the spawn says otherwise.
- Stealth, opt-in by adding `StealthPlugin`: `Notice` on observers and `Stealth` on subjects, `Aware` remembering who has noticed whom, a roll to notice in `DecideSet::Notice` for the actor about to decide, waking on a blow in `TurnSet::React`, and a `Noticed` message on the flip; the minds act only on hiders they have noticed, search where they last saw them, and never descend a flow field toward a player they have not seen.
- Lighting, opt-in by inserting `Lighting`: `LightSource` on a prop, an actor or an item, shed from the carrier once carried; static and dynamic layers recast only when their sources change; the map's `opacity_epoch` so an edit that changes what blocks sight refreshes light and every viewshed without anyone moving; `DarkSight`; `Fuel` ticked by the turn with `LightEvent::BurntOut`; the viewshed keeps its geometric `line` and its seen `visible`, and minds perceive along a line only what is lit, within their dark sight or adjacent.
- Facts fed to quests and counters after the frame.
- Save exports for the scheduler, the world's edits and places, and knowledge.
- A headless app for tests, and `testing`, what goes into one: `surface` stands a `TestWorld` up and hands back open ground to start on, `two_sides` inserts combat rules for two sides at war, and `KeyScriptPlugin` with `press` plays keys the way a keyboard does. One copy for the engine's crates and a game's tests alike, where there had been nine copies of the world and three of the key player.

### rl-render

- A diffed terminal back buffer.
- The map view with lit, remembered and unknown tiles. `MapViewPlugin::new(rect)` takes the rectangle it draws in, the way every panel does, and needs field of view, since without it nothing is ever seen.
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
- Views and their collectors: `NearbyView` (actors and things in the viewshed, nearest first, with health and a relation), `VitalsView` (bars, armor, status badges, turn, position), `GearView` (every registered slot, filled or not), `InspectView` (the look cursor's subject and a duel forecast), `TargetView` (the ability being aimed, the footprint it would cover, whether the resolver would accept it and why not, and what is under it), `AbilityView` (every ability the turn-holder knows, in registration order, with the gate's reasons for the ones it cannot use).
- Panels: `NearbyPanel`, `VitalsPanel`, `GearPanel`, `InspectPanel`, `LogPanel`, `ScrollbackPanel`, `TargetPanel` and `AbilityPanel`, each a plugin holding its rectangle and its headings.
- The scrollback: the whole log on a modal screen, wrapped rather than clipped, ruled off per turn, scrolled by line and by page with both ends clamped, and filtered by cycling only the tones the log actually holds. A second presenter over the same `MessageLog` the strip draws, with its cursor and filter in a `Scrollback` resource of its own.
- `cursor`: what the two cursors share. `CursorKeys` is the one set of bindings both answer to, and `steer` the one reading of a frame's keys: close over confirm over moving, the next candidate asked for only when it is wanted, ordered nearest first with a positional tie-break and cycled with a wrap, and a step that never leaves the loaded window.
- The look cursor: opens on the nearest actor, steps with the direction keys, cycles what is in sight, stays inside the loaded window, and owns input as a modal.
- The targeting cursor: a game writes `AimAt` and the engine does the rest.
  It opens on the nearest thing worth aiming at by `Aim::worth_aiming_at`, the rule a mind's tactic aims by, previews through `Bystanders::land`, the call the resolver lands the use with, and writes the `Use` intent on confirm.
  A property test over seeded layouts holds the preview to it: who the banner lists and whether it reads as refused are what the resolver does.
  An ability that wants no cursor is used at once, so a game binds every ability the same way.
  The overlay repaints the backgrounds the map already drew, keeping every glyph: the cells hit, the flight to them, and the whole footprint in the bad tone with the reason in the banner when the resolver would refuse.
- Awareness on the panels: `Row::aware` says whether each actor in sight has noticed the player, the rail mutes the ones that have not and marks the ones that have, and `VitalsView::seen` reads hidden or seen.
- `Modals`: a stack of interned modal ids with `modal_is`, `modal_open` and `no_modal` run conditions, so one gate covers every screen a game adds.
- `DirectionKeys`: arrows, vi keys and the numpad to the eight directions, in one resource a game may replace.
- A message log carrying a tone per line, folding a repeat into a count, filterable by tone.
- A framed scrolling list menu.
- Drawing helpers a game writing its own presenter reuses: frames, section headings, bars, clipping, word wrapping, and rectangle splits.

### rl-overworld

- Says what it needs: no layout, no screen, and it says so when play begins rather than drawing nothing.
- Open or closed on the shared `Modals` stack, so it and a game's own screens cannot both think they own the arrow keys.
- The world drawn from its bands with rivers, roads, discovered sites and the player.
- A portal picker.

### rl-save

- Backends for files, memory and browser storage behind one resource.
- An exact-match versioned envelope.
- Entity remapping.
- The engine's own state captured and restored, including the pools, cooldowns and charges of every entity the game saved, so a continued run keeps what its abilities had spent and a cooldown is still live at the clock it was saved on.

## Tier 3: rl-engine

- The facade re-exporting every crate.
- `RoguelikePlugins`, the front door: `RoguelikePlugins::new(title, cols, rows)` opens a window sized to the glyph terminal and adds what every game adds, `TerminalPlugin`, `CorePlugin`, `FovPlugin`, `MapViewPlugin`, `UiPlugin` and `CapturePlugin`, with `.cell`, `.font` and `.map` for the rest. No subsystem is in it, and anything in it can be replaced or switched off as in any Bevy plugin group. Every example game and tutorial step starts from it.
- A prelude worth globbing: core, grid, mapgen, world, rules, the Bevy layer, render, UI, overworld and save, in one `use`. It leaves out `Rect`, because Bevy's prelude has one of its own and a game that globs both would have to disambiguate every use; a doc test globs both preludes and names a type from each crate, so the next collision fails there rather than in someone's game.

## The guide

`docs/guide` is an mdBook that builds a small roguelike, Warren, in nine steps: a map on screen, walking, sight and memory, monsters, blows, items, floors, content in RON, and an action of the game's own, ending with the headless tests.
Each step is a runnable binary in `examples/tutorial/src/bin`, so every chapter's code is compiled by CI and can be played on its own; the chapters quote the sources through mdBook anchors rather than restating them, and `scripts/check-guide.sh` fails the build if an anchor, an image or a table-of-contents entry stops resolving.

## The dungeon: the Hollow Whale

Five floors of a beached leviathan, mouth to heart, with no surface and, below the Maw's grey daylight, no light but a brand, the bile and whatever a beast sheds: a cave with teeth, a BSP gullet, a stomach of rooms pooled with bile, a bone-walled ribcage, and a prefab heart chamber with a warden whose death wins the run.
`floors.rs` is the whole map builder; it is the test that a dungeon delve is first-class.
It is where lighting, stealth and abilities meet. The brand burns `Fuel` and shift and `L` smothers it, which is a way past a beast that has not noticed you rather than only a way to see less; a torch lies on the first floor to carry and set down, whalers' lamps are the fixtures, and `v` shows light as digits. The delver's five knacks are data in `assets/abilities.ron`, with `Drain` the delve's own effect in `effects.rs`, and gut eels spit back.
The rail shows vitals with the mana bar and which beasts in sight have noticed you.

## The open world: Corsair

Islands in daylight from the world graph, dark caves lit by the player's lantern and the smugglers' own, ports with huts, a bestiary, armory, affixes, statuses and quests from RON, caves with a treasure vault, a pistol, a ledger, saving and continuing, and a balance report.
Its abilities are a pirate's: a broadside on powder, a grapnel, a swig of rum and a shakedown, `Plunder`, which is Corsair's own effect; cutthroats throw the grapnel back, and its cave dwellers notice rather than see on sight.
That all five genres of ability share one registry is `crates/rl-bevy/tests/genres.rs`, not a game.
It is built only on the public API, so it is the test that the seams are right.

## Not built yet

- Cursed items: nothing resists removal or carries a deliberate penalty.
- Throwing.
- A character sheet.
- Tile fields for fire and gas, and the glow they would shed.
- Nights on Corsair's open water, and a lantern the player can douse or run out of.
- Lit detection ranges, a light-averse tactic, and a ranged penalty in the dark once accuracy exists.
- Scripted encounters, which want an ability's effect list without the turn, the cost and the cursor.
- The wasm unload bridge.
- Seed replay.
- Bevy UI presenters over the panel views (phase H of `docs/design/ui.md`). The views and collectors already do not know which backend draws them; what a node tree would add is wrapping, proportional text, mouse hover and sub-cell bars. Deferred until a game asks for one of those, since it is a second set of presenters to keep and its tests are node trees rather than the exact-text ones that have caught the bugs so far.
- Mouse-to-tile in `rl-render`, which hover, tooltips and click-to-travel all wait on.
- Instanced terminal rendering; one sprite per cell is the known scaling limit.
