# rl-engine overview

What exists in the engine, by tier and crate, and what does not yet.
This page is kept current: every slice that adds or removes a system updates it in the same commit.
`docs/PLAN.md` holds the reasoning and the milestone history; this page holds only the inventory.

Last updated: 2026-09-15, after fire and gas.

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

- Tile registry with flag tables, `Terrain` and views, `OpacitySource` and `CostSource` traits. A tile names what it opens and closes into, and how it burns and the tile it leaves, resolved to ids in the tables, which refuse a name nobody registered.
- `TileField<T>`: a value per tile stepped a turn at a time by a rule that reads the field as it stood, allocating nothing in a step, and moved with a window by `reframe`.
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
- `names`: `Names`, the registries a content file's names resolve against, borrowed for a load: the engine's by name, and any of a game's own with `with("item", &items)`. Every loader below reads through it and reports every unknown name in a file at once, saying which registry was missing when one was never given. A game's own definition type writes a named field as a `NameRef<T>` and loads its file with `Names::load`, so it holds ids from then on, with no validation pass, no second lookup at spawn, and no string that could still be wrong; a `NameRef` read any other way refuses to deserialize.
- Statuses with stacking rules, per-turn ticks and cures, loaded by name with `status::load`: a stat to modify and a damage kind to tick, each resolved to a typed id.
- A faction relation matrix.
- The equipment slot graph with displacement.
- The affix and enchant model: item tags, prefix and suffix affixes with level-scaled stat grants and extra strikes, an enhance rule for what a level buys, per-instance state, weighted rolling. `affix::load` reads affixes by tag, stat and damage kind name.
- `ability`: what an actor can spend a turn on besides a step and a swing, as data. A shape from the targeting module, an [`Aim`] saying what it wants under it, costs against a pool, an item charge, health or a tagged item, requirements over statuses, slots and stats, an integer time and cooldown, and a list of named effects with their arguments left unparsed for whoever registered them. `load` resolves every name in a file through `Names`; `blocked` answers whether a use is permitted and lists every reason it is not. `Aim::hits` says who a footprint catches, the user counting as its own ally, and `Aim::worth_aiming_at` what is worth pointing it at; `aim_blocked` refuses an aim with nowhere to go or out of the user's sight.
- `ai`: movement profiles, snapshots of what an actor sees, and a tactic-priority brain with melee, flee-when-hurt, hunt, wander and use-ability tactics. `Wits` is what a mind is able to do whatever its brain would like, as capabilities (flees, searches, opens doors, picks up, equips, throws) with mindless, animal and sapient presets, read from content as a preset or a list; the tactics that need one ask the snapshot first. The snapshot also holds what the actor carries to throw and what it sees lying about, with how much better off it would be wearing each; `ThrowAtRange` throws down a clear line at what is out of arm's reach, and `Scavenge` fetches gear better than what it wears, or something to throw while it has nothing to throw, walking round what is in the way and putting gear on where it lies. The ability tactic aims by `Aim::worth_aiming_at` and scores a footprint by `Aim::hits`, the rules the resolver lands it with, so a mind fires something it cannot understand, never learns the theme, and never counts a hit the resolver would not land.
- `events`: facts with kind, subject, object and amount, matchers, a ledger of named counters, and quests as objectives over facts with prerequisite chains and a victory flag.
- `balance`: threat scoring and the spawn-band report.
- `ai::awareness`: `NoticeStats` (a certain radius, a chance beyond it, a light bonus and a memory) and `StealthStats`, the pure `notices` roll, and `Awareness`, which goes `Unaware` to `Alert` on a sighting and back once its memory runs out; plus the `SearchLastKnown` tactic, which walks to where an enemy was last seen.
- `gas`: `GasDef` (spread, fade, the concentration that hides what is behind it, whether it burns, a status for breathing enough), loaded by status name, and `diffuse`, an exchange with each neighbour and a fade of at least one unit, so every cloud clears.
- `fire`: `Tinder` and `spread`, one catch chance per burning neighbour from rolls the caller passes in, so the spread does not depend on visiting order.
- `forecast`: what a fight is likely to cost, with the average roll put through the game's own mitigation pipeline in place of a real one; blows and turns to fell either side, and an `Outlook` read off the two counts. Pure, so an inspect panel's numbers cannot drift from the fight.

## Tier 2: the Bevy layer

### rl-bevy

- One plugin per subsystem, each opt-in: `CorePlugin` holds the loop, the map and its places; field of view, combat, minds, statuses, items, lighting, streaming and facts are added by name. A plugin says what it needs with `app.needs::<R>(plugin, hint)`, and on entering play one check lists every missing piece at once with how to make it, `CorePlugin`'s own `WorldMap` included, rather than a subsystem, or the whole frame, quietly doing nothing all run. A plugin that depends on another checks in `finish`, so the order a game lists its plugins in never matters, and a world built while play never began is warned about.
- The engine-owned loop: a `Turn` schedule run as many passes per frame as it takes, input once per frame, refusals that cost nothing, stall recovery.
- `Seed`, the run's seed, the one thing about randomness a game supplies, with `Seed::from_args` for `--seed`. Each subsystem that rolls owns a `Stream` (`CombatRng`, `AbilityRng`) that `add_stream` derives from the seed before the first turn and again whenever it changes, so a continued run reseeds by setting it; `Seed::stream(name, index)` is a game's own named draw, and `EngineSave` records the seed it captures.
- A mind may choose an action the engine has never heard of: a tactic returns a number of the game's own, the engine reports it as `MindChose`, and the game answers it in `DecideSet::Game`. A game's tactic can sit anywhere in the priority list beside the engine's.
- A reaction phase inside the turn: `TurnSet::React` runs after the actions of a pass resolve and before the turn is requeued, which is where a game answers what just happened. A drink heals before the next blow lands, a bite poisons on the bite, gear counts from the moment it is worn.
- Drawing is layered by `PresentSet`: narration, then the map, then the chrome, then whatever covers them. No crate orders itself after another crate's draw function.
- Actions are types, not a list: `Step`, `Attack`, `Wait`, `Close`, `PickUp`, `DropItem`, `Equip`, `EquipFromGround`, `Unequip`, `UseItem`, `Throw` and `GoThrough` ship with the engine, each resolved by the module that owns the mechanic. A game registers its own with `add_action`, resolves it in `ResolveSet::Act` through `Resolution`, and the sweep refuses whatever no resolver claimed. `Resolution` is every resolver's side of the loop, the engine's and a game's: `claim` the actor holding the turn, then `done` with a cost or `failed`, which keeps the player's turn and charges anyone else, so no resolver has to remember that a monster handed a free retry loops forever.
- Every stage a plugin fills is a named set, and no system orders itself after another's function: `DecideSet::{Notice, Offer, Minds, Game}`, `ResolveSet::{Travel, Act, Fields, Effects, Damage}` with `FieldSet::{Fire, Gas}` inside `Fields` with steps, waits and warps travelling before anything else acts, and `CleanupSet::{Remove, Requeue}` taking the dead out before a turn is requeued. The dead are buried in `Last`.
- Chunk streaming with edit deltas, per-map occupancy and knowledge, field of view.
- `Registries`, every registry the engine reads, in one resource the game fills once: damage kinds, factions, stats, statuses, tags and slots. An empty one means none, and `names()` hands the same tables to a load, so a game's content is resolved against exactly what the engine will read it with and no subsystem keeps a copy.
- Combat: health, armor, resists, factions as `CombatRules`, the matrix over the registry's sides, built by naming pairs with `CombatRules::new(&sides).hostile(a, b)`, `hunts` for a grudge that is not returned and `allied`, melee and ranged attacks down a line of fire, extra strikes, the damage event pipeline, and deaths that linger until the frame ends. It knows nothing of minds or abilities: a blow is a blow whoever chose it.
- Doors: walking into a tile that opens opens it and spends the turn where the actor stands, `Close` shuts one unless something stands or lies in the doorway, and `DoorEvent` reports both, for an actor with the wits to or one with no `Intelligence` at all. `WorldMap::cost_epoch` changes when an edit changes where anyone may walk.
- Minds, opt-in by adding `MindsPlugin` beside combat: `Mind`, `Perception` and `Profile` on a monster, and `Intelligence(Wits)`, which every mind carries and is sapient unless the spawn says otherwise; flow fields per movement profile and per whether the mover opens doors, which reads a closed door through `WorldMap::opening_view` as the turn to open it, the snapshot a brain reads, and the one system that turns a decision into a step, an attack, a wait, an ability, a pickup, an equip from the ground, a throw or a `MindChose`. The one place every action a monster can choose meets, so neither combat nor abilities has to know about the other; a `Mind` spawned without the plugin is reported once rather than left standing.
- Items on the ground, in bags and in slots, with stacks, tags and enchantments. A carried item is on no map, and one put down lies on the map it is put down on. `EquipFromGround` takes up and puts on what lies underfoot for a turn and a half; `GearScore` is what wearing an item is worth, which a mind compares against what it would displace; what a dead actor carried falls where it died.
- Throwing, opt-in by adding `ThrowingPlugin` beside items and combat: `Throwable` says how far an item goes and what it strikes for, `Throw` sends one from a stack or the item itself at a cell, and it strikes the first body in its way down combat's damage pipeline and comes to rest at its feet, short of a wall, or at the end of its reach, reported as `ItemEvent::Thrown`. `flight` is the one answer to where a throw goes, and the resolver and the targeting preview both call it.
- Abilities, opt-in, with every actor given an empty `Known`, `Pools` and `Cooldowns` as it is spawned, so `Grants` alone is enough to use what was granted: the `Use` action, `Known` rebuilt every turn from what an actor is and wears, `Pools` for whatever a game calls its fuel, `Cooldowns` as absolute times on the turn clock so a save restores them for nothing, `Grants` and `Charges` on the things that lend an ability, `Offered`, the gate's answer for whoever holds the turn, which the minds and a menu both read through accessors that answer only for that actor, and a resolver that gates, pays, resolves the footprint and lands the effects inside the pass. `Bystanders::land` is the one answer to where a use lands, who it hits and why the aim would be refused; the resolver and the targeting preview both call it. Effects are types, not a list: one per subsystem the engine owns, seven in `effects` registered together, and `Ignite` and `Emit` registered by the fire and gas plugins, which depends on combat and statuses so neither depends on abilities, and a game registers its own with `add_effect`. An effect asks for damage, a status or a move through `EffectWorld`, and reaches anything else through `Commands`.
- Places: bounded maps entered by transitions or warps, built on first arrival, kept whole, off-map actors frozen; `PlaceBuild::from_context` reads a finished chain; `WarpRequest::into_place` starts a run in one.
- The surface is optional, and everything regional belongs to it: `WorldMap::new` takes the tile tables alone, the region size is read from the world graph when the first window loads, and `Knowledge` is initialised by the engine. A delve names neither.
- Knowledge: explored tiles per map in buckets of its own, and the surface's seen regions and discovered sites kept apart from them, so going underground never hides the overworld's fog.
- Statuses ticked by the turn through the damage pipeline. `StatusPlugin` gives every actor an empty `Afflicted` and `StatBlock` as it is spawned, so a monster spawned without them still takes a status, and a game without statuses carries neither.
- Every actor requires a `Speed`, normal unless the spawn says otherwise.
- Stealth, opt-in by adding `StealthPlugin`: `Notice` on observers and `Stealth` on subjects, `Aware` remembering who has noticed whom, a roll to notice in `DecideSet::Notice` for the actor about to decide, waking on a blow in `TurnSet::React`, and a `Noticed` message on the flip; the minds act only on hiders they have noticed, search where they last saw them, and never descend a flow field toward a player they have not seen.
- Lighting, opt-in by inserting `Lighting`: `LightSource` on a prop, an actor or an item, shed from the carrier once carried; static and dynamic layers recast only when their sources change; the map's `opacity_epoch` so an edit that changes what blocks sight refreshes light and every viewshed without anyone moving; `DarkSight`; `Fuel` ticked by the turn with `LightEvent::BurntOut`; the viewshed keeps its geometric `line` and its seen `visible`, and minds perceive along a line only what is lit, within their dark sight or adjacent.
- Gas, opt-in by adding `GasPlugin`: a field per gas per map in `Gases`, given off by `Release` and `Vents`, stepped every whole turn over what does not stop a thrown thing, written into the map's veil where thick enough so sight and light stop there, and breathed, as `Breathed` and the gas's status.
- Fire, opt-in by adding `FirePlugin`: `Fire` per map, set alight by `Kindle` and by what is `Burning`, fed by burning tiles, `Flammable` things and gas that burns, stepped every whole turn from hashed rolls; burnt ground becomes the tile it leaves, whoever stands in it is `Scorched` and given `FireRules::inflicts`, and burning cells smoke and glow through `Lighting::set_glow`. `FireEvent` reports what the game answers. Minds will not step into fire.
- `MapFields`, what fire and gas keep per map: the current map's field fitted to the window, every other map's set aside, and every one saved.
- Facts fed to quests and counters after the frame.
- Save exports for the scheduler, the world's edits and places, and knowledge.
- A headless app for tests, and `testing`, what goes into one: `surface` stands a `TestWorld` up and hands back open ground to start on, `two_sides` inserts combat rules for two sides at war, and `KeyScriptPlugin` with `press` plays keys the way a keyboard does. One copy for the engine's crates and a game's tests alike, where there had been nine copies of the world and three of the key player.

### rl-render

- A diffed terminal back buffer.
- The map view with lit, remembered and unknown tiles. `MapViewPlugin::new(rect)` takes the rectangle it draws in, the way every panel does, and needs field of view, since without it nothing is ever seen.
- Shading, in the manner of Brogue: each tile authored with both colours and a `Vary` that jitters every cell by a hash of its position and can shimmer over time; light multiplies glyph and background channel by channel, down to a dark floor and up to a gain cap; the wavering part of a light dips on a smooth noise so flames ripple; `Memory` fades what was seen to a darker, greyer, cooler colour.
- `LightOverlay`: intensity drawn as digits.
- `FieldAppearance`: flames over every burning cell in sight, flickering cell by cell, and the ground tinted by the densest gas on it, drawn as haze where it hides what is behind it.
- `CapturePlugin`: `RL_CAPTURE` plays `RL_CAPTURE_KEYS` through the real keyboard input, photographs the window without taking focus, refuses a black frame, and exits.
- Glyph entities filtered to the current map.

### rl-ui

Every panel splits three ways: a view (a resource of plain data), a collector (the system that rebuilds it each frame, in `ViewSet::Collect`), and a presenter (one way of drawing it, in a `PresentSet` layer).
The query is the half a game reuses; the drawing is the half it may replace or drop.
Opt-in is per panel, and a presenter pulls its view plugin in behind it.

- `UiPlugin` owns the `MessageLog`, because a game writes to the log from its own systems whether or not it draws it: a headless test adds the plugin and has a log with no panel.
- Tones: a semantic role interned as a `ToneId` over a `Palette` a game extends with roles the engine never heard of, with `add_tone(name, colour)` declaring and colouring one while the app is built, and a warning by name at startup for any left without a colour. No widget takes a `Color`.
- Facets: `Facet { key, text, tone }` pushed onto a row in `ViewSet::Annotate`, so a game's vocabulary reaches a panel without an engine type learning a word.
- Views and their collectors: `NearbyView` (actors and things in the viewshed, in the order `InSight` lists them, with health, a relation, and the row picked out), `VitalsView` (bars, armor, status badges, turn, position), `GearView` (every registered slot, filled or not), `InspectView` (the look cursor's subject and a duel forecast), `TargetView` (the ability, throw or shot being aimed, the footprint it would cover, whether the resolver would accept it and why not, and what is under it), `AbilityView` (every ability the turn-holder knows, in registration order, with the gate's reasons for the ones it cannot use), `SheetView` (every registered stat with its base, its value and each modifier between them tagged by source, what is resisted, the blows and the shot, the statuses with their turns left and what they do, and every slot filled or not; a status names its own modifiers, and a game names its items' through `name_source`).
- Panels: `NearbyPanel`, `VitalsPanel`, `GearPanel`, `InspectPanel`, `LogPanel`, `ScrollbackPanel`, `TargetPanel`, `AbilityPanel`, `ControlsPanel` and `SheetPanel`, each a plugin holding its rectangle and its headings.
- `SheetPanel`: the character sheet as a modal, opened by `SheetKeys` (`c` by default, and listed on the controls screen), drawn a section at a time with a section that has nothing in it left out.
- `controls`: `Controls`, the registry every key is declared in as a group, an action and its `Keys`; `ControlInput`, a frame's keys read through it by `ControlId`, with `which` for a slot row, `direction` and `direction_held` for the direction keys, and `label` for what a player reads; `Chord`, a key with Shift matched exactly. An `EngineKey` names one of the engine's own bindings and is read from `DirectionKeys`, `CursorKeys`, `ScrollbackKeys` or `ControlsKeys` when listed, so a rebound key is listed as rebound. Every engine plugin with keys declares them in `finish`, in shared words, so a control two plugins read is listed once and a game's groups come first.
- `ControlsPanel`: the registry as a screen, groups in declaration order flowed into columns and then pages, a heading never ending a column, opened and closed by `ControlsKeys` (`?` and Escape, the arrows for pages), with an optional one-row hint naming the key that opens it.
- The scrollback: the whole log on a modal screen, wrapped rather than clipped, ruled off per turn, scrolled by line and by page with both ends clamped, and filtered by cycling only the tones the log actually holds. A second presenter over the same `MessageLog` the strip draws, with its cursor and filter in a `Scrollback` resource of its own.
- `focus`: `InSight`, what the player can see, actors nearest first and then things nearest first, ties by name and then by entity, which is the one list the nearby panel prints and both cursors cycle; and `Focus`, the one entity picked out of it. With nothing open, Tab steps the focus down the list, Shift with it steps back, and Escape lets go. `NearbyPanel` draws the row picked out and its map tile on the selection tone, scrolling to keep the row on the rail, and either cursor opens on it when it can take it.
- `cursor`: what the two cursors share. `CursorKeys` is the one set of bindings both answer to, the nearby list included, and `steer` the one reading of a frame's keys: close over confirm over moving, the candidates asked for only when the cursor moves, cycled entity by entity with a wrap both ways so two things on one tile are two stops, the `Focus` moved onto whatever the cursor lands or steps on, and a step that never leaves the loaded window.
- The look cursor: opens on the focus or the top of the list, steps with the direction keys, cycles actors and things in sight, describes the one picked out of two on a tile, stays inside the loaded window, and owns input as a modal.
- The targeting cursor: a game writes `AimAt` for an ability, `AimThrow` for something carried or `AimFire` for a shot, and the engine does the rest. It cycles what the aim can take by `Aim::cycles_to`, so an aim at the ground stops on anything in sight rather than on cells. A throw opens on the nearest foe, previews through `flight` who it strikes first and where it lands, and writes the `Throw` on confirm; the banner says what is aimed by the name the view carries. A shot previews through `shot`, the line `line_of_fire` answers by, and writes an `Attack` on whoever stands under the cursor.
  It opens on the nearest thing worth aiming at by `Aim::worth_aiming_at`, the rule a mind's tactic aims by, previews through `Bystanders::land`, the call the resolver lands the use with, and writes the `Use` intent on confirm.
  A property test over seeded layouts holds the preview to it: who the banner lists and whether it reads as refused are what the resolver does.
  An ability that wants no cursor is used at once, so a game binds every ability the same way.
  The overlay repaints the backgrounds the map already drew, keeping every glyph: the cells hit, the flight to them, and the whole footprint in the bad tone with the reason in the banner when the resolver would refuse.
- Awareness on the panels: `Row::aware` says whether each actor in sight has noticed the player, the rail mutes the ones that have not and marks the ones that have, and `VitalsView::seen` reads hidden or seen.
- `Modals`: a stack of interned modal ids with `modal_is`, `modal_open` and `no_modal` run conditions, so one gate covers every screen a game adds, declared with `add_modal(name)` by the engine's own screens and a game's alike.
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
- The engine's own state captured and restored, including every burning cell and every cell with gas on every map, and the pools, cooldowns and charges of every entity the game saved, so a continued run keeps what its abilities had spent and a cooldown is still live at the clock it was saved on.
- The unload bridge: `Stash`, the last save the game encoded, and `UnloadPlugin`, which writes it through `Saves` when a browser page is hidden or unloaded and, everywhere, on the frame the app exits. The handler runs outside the app and cannot borrow the world, so the game keeps the stash fresh and clears it when a save must not survive. `Saves` shares its backend by `Arc` so the handler writes through the same one.

## Tier 3: rl-engine

- The facade re-exporting every crate.
- `RoguelikePlugins`, the front door: `RoguelikePlugins::new(title, cols, rows)` opens a window sized to the glyph terminal and adds what every game adds, `TerminalPlugin`, `CorePlugin`, `FovPlugin`, `MapViewPlugin`, `UiPlugin` and `CapturePlugin`, with `.cell`, `.font` and `.map` for the rest. No subsystem is in it, and anything in it can be replaced or switched off as in any Bevy plugin group. Every example game and tutorial step starts from it.
- A prelude worth globbing: core, grid, mapgen, world, rules, the Bevy layer, render, UI, overworld and save, in one `use`. It leaves out `Rect`, because Bevy's prelude has one of its own and a game that globs both would have to disambiguate every use; a doc test globs both preludes and names a type from each crate, so the next collision fails there rather than in someone's game. The Bevy layer's part holds what a game writes: the effects a game names only in RON, and the pieces other engine crates build on such as `Bystanders`, `Offered` and `FlowFields`, stay at the crate root.
- `templates/starter`, a `cargo generate` template for a new game: one file with a generated floor of rooms, a carried torch and braziers in the dark, goblins that notice by sight and light and search where they last saw the player, bump-to-attack combat, a status row, a log and a look cursor, and three headless tests. `scripts/check-template.sh` renders it with the engine taken from the checkout and runs its formatting, clippy and tests, and CI runs the script, so the template cannot fall behind the API. The script also refuses a `cargo generate` line in the README or the guide that does not name the release with `--tag`.
- Releases: a tag is `v` and the workspace version, and pushing one publishes its release page from its `CHANGELOG.md` section through `scripts/release-notes.sh`.

## The guide

`docs/guide` is an mdBook that builds a small roguelike, Warren, in nine steps: a map on screen, walking, sight and memory, monsters, blows, items, floors, content in RON, and an action of the game's own, ending with the headless tests.
Each step is a runnable binary in `examples/tutorial/src/bin`, so every chapter's code is compiled by CI and can be played on its own; the chapters quote the sources through mdBook anchors rather than restating them, and `scripts/check-guide.sh` fails the build if an anchor, an image or a table-of-contents entry stops resolving.

## The dungeon: the Hollow Whale

Five floors of a beached leviathan, mouth to heart, with no surface and, below the Maw's grey daylight, no light but a brand, the bile and whatever a beast sheds: a cave with teeth, a BSP gullet, a stomach of rooms pooled with bile, a bone-walled ribcage, and a prefab heart chamber with a warden whose death wins the run.
`floors.rs` is the whole map builder; it is the test that a dungeon delve is first-class.
It is where lighting, stealth and abilities meet. The brand burns `Fuel` and shift and `L` smothers it, which is a way past a beast that has not noticed you rather than only a way to see less; a torch lies on the first floor to carry and set down, whalers' lamps are the fixtures, and `v` shows light as digits. The delver's five knacks are data in `assets/abilities.ron`, with `Drain` the delve's own effect in `effects.rs`, and gut eels spit back.
Fire and gas meet here: slicks of fat catch and burn to cinder, sinew curtains burn away, one bile pool in four reeks of a gas that burns and dazes, burning flesh smokes enough to hide in, and the fireball sets what it lands on alight.
The rail shows vitals with the mana bar and which beasts in sight have noticed you.

## The open world: Corsair

Islands in daylight from the world graph, dark caves lit by the player's lantern and the smugglers' own, ports with huts, a bestiary, armory, affixes, statuses and quests from RON, caves with a treasure vault, a pistol, a ledger, saving and continuing, and a balance report.
Its abilities are a pirate's: a broadside on powder, a grapnel, a swig of rum and a shakedown, `Plunder`, which is Corsair's own effect; cutthroats throw the grapnel back, and its cave dwellers notice rather than see on sight.
Its doors shut, and only a mind with the wits opens one: the crab is mindless, the dog and jaguar are animals, and the cutthroats and marines are sapient, which the log shows when one works a door in sight; `c` shuts a door.
The crew have hands: cutthroats come with throwing knives and throw them from range, marines put on better gear they find, a sapient monster picks up a knife when it has none, and the log says what one in sight takes up, puts on or throws. `r` throws, `t` throws from the chest, `e` puts on what lies underfoot, and a monster's bag survives a save.
That all five genres of ability share one registry is `crates/rl-bevy/tests/genres.rs`, not a game.
It is built only on the public API, so it is the test that the seams are right.

## Not built yet

- Cursed items: nothing resists removal or carries a deliberate penalty.
- Heat and cold that put fire out, liquids that flow, and wind.
- Nights on Corsair's open water, and a lantern the player can douse or run out of.
- Lit detection ranges, a light-averse tactic, and a ranged penalty in the dark once accuracy exists.
- Scripted encounters, which want an ability's effect list without the turn, the cost and the cursor.
- Seed replay.
- Bevy UI presenters over the panel views (phase H of `docs/design/ui.md`). The views and collectors already do not know which backend draws them; what a node tree would add is wrapping, proportional text, mouse hover and sub-cell bars. Deferred until a game asks for one of those, since it is a second set of presenters to keep and its tests are node trees rather than the exact-text ones that have caught the bugs so far.
- Mouse-to-tile in `rl-render`, which hover, tooltips and click-to-travel all wait on.
- Instanced terminal rendering; one sprite per cell is the known scaling limit.
