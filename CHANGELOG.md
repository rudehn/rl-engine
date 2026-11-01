# Changelog

All notable changes to atglyph are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
atglyph is not on crates.io yet, so a game depends on a release by its git tag, as the README shows.

## [Unreleased]

### Added

- `Keep` can keep away from the others: `Station::Others` is the actors neither allied nor hostile, and `Keep::away(station, distance)` backs off inside `distance` and never closes in, named `keep_away`; a keep on the others steps by near fields, as `GiveWay` does.
- `Keep::watching`: a keep that waits in its band while the nearest of its station is in sight, so a wary animal that has backed off far enough stands and watches rather than wandering back in.
- A mind keeps to its ground: an actor's `Ground` id, read through the game's `GroundRule`, holds its idle steps to the cells the game names, and one off its ground walks back. Herds: actors sharing a `Herd` id keep together with no leader, by `Keep::herd` on the middle of their living mates.
- Carving a place out of solid rock a cell at a time: `DrunkardsWalk` opens a broad hall with rock standing in it, and `Aggregate`, diffusion-limited aggregation, grows branching tunnels; both open the share of the map asked for in one piece, from an `Origin`, with a brush width, a margin and, for aggregation, a pull toward the middle.
- `CellBrightness`: a game draws each cell of a map brighter or darker than its tile, as high ground brighter than low, in sight and in memory alike.
- Near fields: `Fields::descents_toward_near` and `Fields::descents_away_near` flood only the box round a mind and its goals, a margin wider, for a step among those a few cells off; the engine floods them afresh and keeps them out of the shared fields, and a `Fields` of a game's own answers them from its whole fields unless it says otherwise.
- `RangeRules::check` names every divisor under one and multiplier under nought, and `StealthPlugin::by_range` refuses rules that fail it, naming each; a multiplier too large for a range saturates.
- Cover: a tile may be `cover: true` in a tiles file, `TileProps::cover` and `TileTables::cover`, for a rule that makes an actor standing in it harder to notice.
- Stealth by range: `StealthPlugin::by_range(RangeRules)` judges noticing by the subject's own `StealthRange`, within which anything not allied may notice it by the game's chance a turn and beyond which nothing does, an alert observer holding it in view within `hold` times the range; the plugin gives every actor a range at the base and every mind an `Aware` as each appears, and a neutral is hidden from a mind's others until noticed. `Hides` marks whatever can hide under either rule. Each `StealthRange` is worked out every turn from the actor's `LastDeed` and where it stands: waiting divides it, attacking multiplies it, the dark band and cover each divide it.

### Changed

- Waking the struck to whoever struck them runs in `ReactSet::Settle`: a game that reads `Noticed` or writes an `Aware` in `TurnSet::React` orders itself after that set, so a blow's notice is read in the pass it lands and the two never write in an order the scheduler picks.
- **Breaking:** `Keep` has a `watch` field, false for every constructor; a game that writes a `Keep` literal adds `watch: false` to keep it as it was.
- **Breaking:** `NoticeStats`, `StealthStats`, `notices`, `certain_radius` and `notice_chance` moved from `atglyph_rules::ai::awareness` to `atglyph_rules::ai::awareness::per_observer`; a game naming them by the old path names the new one, and `atglyph_rules::ai` and the prelude keep their names.
- **Breaking:** `awareness::observe` takes the `Glimpse` a rule judges and whether the rule holds the subject, `observe(state, glimpse, holds, spots, at, memory)`, in place of a bare `in_view`; a game calling it passes `Glimpse { in_view, .. }` and `holds` as `in_view` to keep the per-observer answer.
- **Breaking:** `StealthPlugin` is built with the rule it notices by: a game that added `StealthPlugin` adds `StealthPlugin::per_observer()`, and plays as before.
- **Breaking:** A mind's idle moves are one tactic, `Idle`, moving by the `Movement` its actor carries in `Moves`: `Wander`, `Hold(cell)` or `Roam { at, reach }`. It replaces `KeepPost`, `Wander`, `Posted` and the `Post` component, and the standard chain's `keep_post` and `wander_chance` become one `idle_chance`, with `before_wander` renamed `before_idle`. A mind with no `Moves` has no idle move. A game moving over gives `Hold` where it gave `Post` and `Wander` to every actor that drifted. Saves are format 2.
- **Breaking:** `Scatter` and `ScatterBy` have a `phase`, `Phase::Growth` to cover the ground before anything is built and `Phase::Finish` to strew over what was; a literal adds `phase: Phase::Growth` to scatter as before.
- What is in sight, the nearby list and the look and target cursors, is only what the `MapView` draws: a player who sees the whole place no longer lists things far off the screen.
- Flow fields read the loaded window's costs once each time the map's costs or its window change, rather than asking the map at every step of every flood, which made each flood several times slower.
- Noticing looks only at the subjects an observer could see this turn, on its map and within its reach, and at those it already knows of, and works out whether one stands in the light only for the per-observer rule, which reads it: a turn costs a look per subject nearby, not per subject loaded, and plays as before.
- By range, a pass works out only the stealth ranges that could have changed, of an actor that moved or finished an action and of every actor when the light or the map changed, and a pass that relights nothing no longer marks `Lighting` changed.
- A `Keep` backing off a mind that stands on its ground keeps to that ground, as `Idle` does, and a watching keep waits only on it, leaving one off its ground to `Idle` to walk back.
- `Keep::herd` and `GiveWay` step by near fields, and `GiveWay` gives way only to those within its `space` and 8 cells past it: a herd closing up or a crowd of bystanders no longer floods every loaded cell for each member's step.

### Fixed

- A herd member standing on its herd's middle steps off it, as its `no_closer_than` says, rather than asking for a field and then doing nothing.

## [0.5.0] - 2026-10-29

### Added

- Streamed places: `PlaceRules::streamed` names a place built a chunk at a time, a `Streamed` layout, and `PlaceRules::build_chunk` builds each chunk. The chunks within the layout's `reach` of the player are kept loaded, a chunk that leaves keeps only what was changed on it and has it laid back when it returns, an actor on a chunk not loaded is dealt no turn, `ChunkLoaded` is written for every chunk that loads with `first` the first time, and a save keeps a streamed place's changes rather than its tiles. `WorldMap::stream`, `install_streamed` and `is_loaded_on`, `PlaceMap::bounds`, `tile`, `terrain` and `streamed`, and `testing::streamed_ground`.
- A tile drawn in several glyphs: a tiles file's `glyph` takes one character, `glyph: '.'`, as before, or a list, `glyph: ['"', '"', ',']`, and `TileLook::new` a `char` or an array. Each cell draws the glyph `TileGlyphs::at` picks by a hash of its world position, alike every frame and in memory, a glyph listed twice picked twice as often; under a `Tileset` each glyph is its own picture.
- A tile carries its look: `TileProps::look`, a `TileLook` of glyph and colours, set with a builder or read from a tiles file. `atglyph::rules::tiles::load` reads a whole `TileRegistry` from the file, one line a tile, and `TileAppearance::from_tiles` builds the colour table from it.
- A stack's limit: `ItemParts::stack_limit`, spawned as `StackLimit`, the most one stack of a thing holds. Picking up, taking from a container and buying fill the carried stacks of a kind to their limits in bag order and keep the rest a stack of its own, and `atglyph::rules::items::fill_stacks` is that rule.
- A thrower's reach: `CombatRules::reach(base, stat)`, a `ThrowReach`. A thing whose throw names no range flies as far as its thrower reaches, a base plus the thrower's value of a stat. `Throws::of(thrower, item)` answers how a thrower throws a thing, a `ThrowAs`, and the throw, its odds, the aim, the pack's rows and every mind weighing a missile read it.
- Launchers: a `LaunchDef` part in `Attacks::launch`, spawned as `Launcher`, worn, adds its range to how far a thing it launches flies and its roll to what that strikes for; the thing lands as any throw does, `Launched` is written beside its `ItemEvent::Thrown`, the narrator says it as shot, the aim names the launcher in `TargetView::launcher`, and a pack row's `Strike::also` holds the launcher's roll. `ThrowRules` answers as `Throws` does for a system already holding what is worn.
- `PickUpItem { item, count }`: one thing lying underfoot, or part of its stack, the rest left lying; `DropItem`'s count drops part of a stack and keeps the rest carried.
- The ground screen and the count prompt: `GroundPanel` (with `GroundView`, `GroundViewPlugin`, `GroundMenu` and `ground_modal`) lists what lies underfoot under `All` when the pick-up key finds more than one plain thing, and `Counting` asks how many of a stack to pick up, or to drop from the pack, starting at all of it, on `CountKeys`.

### Changed

- **Breaking:** `PlaceMap::terrain` is no longer a field; read `place.bounds()` for its extent, `place.tile(p)` for a tile, and `place.terrain()` for every tile of a place built whole, `None` for a streamed one.
- **Breaking:** `TileLook::glyph` is a `TileGlyphs` rather than a `char`, and `TileLook` is no longer `Copy`, nor `TileLook::new` a `const fn`; read one glyph as `look.glyph.first()` or compare `look.glyph == '#'`, borrow a look with `.as_ref()` where it was copied, and a literal writes `glyph: '#'.into()`.
- [Undermere](https://github.com/rudehn/undermere) and [Foundry](https://github.com/rudehn/foundry) moved to repositories of their own; a game changes nothing.
- The tutorial's nine steps share the starter template's shape, one game plugin that `main` and the tests both add, and each step only adds to the one before; its chapters are shorter.
- **Breaking:** `ItemParts` has a `stack_limit` field; a literal adds `stack_limit: None`, or ends in `..ItemParts::default()`.
- **Breaking:** `Throwable::range` and `ThrowDef::range` are `Option<i32>`, `None` flying as far as the thrower's reach; write `Throwable::new(Some(n), strike)`, and a RON throw keeps `range: n` under `implicit_some`.
- **Breaking:** `Attacks` has a `launch` field and `Strike` an `also` field; a literal adds `launch: None` or `also: None`.
- **Breaking:** `DropItem(item)` is `DropItem { item, count }`; write `DropItem::all(item)`.
- **Breaking:** `ItemEvent::PickedUp` has a `count`, how many were picked up, which the narrator names the thing at; a pattern adds `count` or `..`.
- **Breaking:** `PlayerKeysPlugin::pick_up` takes an `Otherwise`, said when nothing lies underfoot; write `Otherwise::Nothing` to say nothing.
- **Breaking:** `Throwable::effective_range` is gone, since how far a thing flies depends on its thrower; read `Throws::of(thrower, item)`'s `effective`. `Attempt::Throw` takes the `ThrowAs`.

### Removed

- **Breaking:** `atglyph::bevy`; name `bevy::` from the game's own `bevy = "0.19"` dependency, which Bevy's derive macros need anyway.
- **Breaking:** `TileAppearance::load(text, &tiles)`; read the tiles with `atglyph::rules::tiles::load(text)`, moving each tile's properties into its line, and call `TileAppearance::from_tiles(&tiles)`.
- **Breaking:** `atglyph::rules::tiles::light_tiles`; a tile's `light` is read with the rest of its line by `tiles::load`, or set in code with `TileProps::light`.
- **Breaking:** `TileProps` is no longer `Eq`, since a look holds colours.

### Fixed

- The tutorial's steps 4 to 6 draw the aiming cursor with `TargetPanel`, where `r` and `a` opened an unseen cursor that held the arrow keys, and steps 7 to 9 keep throwing, the screech and ratlings.

## [0.4.0] - 2026-10-24

### Added

- Work: an actor doing one thing across many turns, `WorkPlugin` with `app.add_work("word")`, for monsters and the player alike.
- Trade between two actors: `TradePlugin`, `TradePanel`, and priced offers with `Offering::priced`.
- Actors offer verbs as props do (`Offers`), and an offer may ask a question before it is taken up.
- A party: `Keep::leader`, `PartyPanel`, and followers that travel behind whoever went through.
- Remains keep a twin of the actor they were, and `remains::revive` stands one back up.
- Grown places: `Accrete`, `OpenLoops`, `Survey`, cave shapes, barriers and landmark pieces, read from layout files.
- Lakes and bridges, hidden openings and secret cells, and areas dressed from a file.
- Pool files, `atglyph_rules::pool::load`, and load-time checks that every content entry can show up.
- Prefabs as RON files, which turn, mirror and mark their entries.
- The engine lays the stairs between floors from `PlaceRules::ladder`.
- A title screen (`TitlePanel`), run records (`RecordsPlugin`, `RecordsPanel`), settings (`SettingsPanel`, `SettingsSavePlugin`), and fullscreen as a setting.
- `atglyph-tty` and `TtyRoguelikePlugins`: play in a text terminal, locally or over SSH, with per-user saves.
- Tilesets, so a game can be drawn in pictures, and a game's own typeface.
- Loot is the engine's (`LootPlugin`), with containers that ask for things by tag and enchant levels by band.
- Statuses that hold their holder's turns, put it to sleep, last until cured, hide it, or count as boons.
- Worn things that lend statuses, resists and charges, and things not yet known.
- Attacks that miss (`HitRules`), attack shapes, and ammunition (`Ammo`).
- Light from tiles and props, coloured light, three light bands, and ambient light per place.
- Names drawn from syllables, `atglyph_rules::namegen`.
- What every game wrote for itself: `RailLayout`, `PlayerKeysPlugin`, the `Standard` brain, and `atglyph::testing`.
- Rules as tier 1 functions in `atglyph-rules`, testable without an `App`: damage, faction stance, bumps, trade, equipping, work and more.
- `atglyph::bevy` and `atglyph::rand` re-export the Bevy and the `rand` the engine is built on.
- Every crate packages for crates.io.

### Changed

- **Breaking:** the engine is `atglyph`: `rl_engine::` is `atglyph::`, the crates are `atglyph::primitives`, `ecs`, `grid`, `mapgen`, `rules`, `render`, `ui`, `save` and `tty`, and `RL_*` variables are `ATGLYPH_*`.
- **Breaking:** every seed plays differently; re-baseline fingerprints and seed-pinned tests once.
- **Breaking:** `use atglyph::prelude::*` holds only what a first game names; `atglyph::prelude::all::*` is everything, and topic modules hold the rest.
- **Breaking:** each crate's `App` methods come through one trait, `EngineApp`, `UiApp` or `SaveApp`, which the prelude brings.
- **Breaking:** `Afflict`, `Cure` and `Afflicted` are `ApplyStatus`, `RemoveStatus` and `ActiveStatuses`; `Strikes` is `BonusDamage`; `Rect` is `GridRect`; `Moment` is `TriggerKind`.
- **Breaking:** `TurnHold` is `CueWait`, with `hold` and `is_held` written `wait` and `is_waiting`.
- **Breaking:** in `atglyph-bevy`, `Reach` is `DamageRoute` and `Offered` is `UsableAbilities`.
- **Breaking:** in `atglyph-rules`, `Source` is `ModifierSource` and `Look` is `ParticleLook`; in `atglyph-ui`, `Heard` is `NarratedEvents`, `Change` is `ModifierRow` and the targeting view's `Reach` is `TargetReach`.
- **Breaking:** a system writes to the log through the `Log` param, `log.notice(text)`, in place of `ResMut<MessageLog>`.
- **Breaking:** a facet is `Facet::new("key", text)`, and the `Facets` resource is gone.
- **Breaking:** `PlaceRules::build(map, seed)` is handed the place's own `PlaceSeed` and no world, and the new required `PlaceRules::tiles` names the tiles a place is built of.
- **Breaking:** the place model is tier 1, in `atglyph::mapgen::place`.
- **Breaking:** `app.places(rules)` and `commands.spawn_player(map, bundle)` replace setting up the world by hand, and play begins on its own.
- **Breaking:** a draw is `seed.rng(DOMAIN, index)` over a `SeedDomain`, and `CombatRules::new()` takes no registry.
- **Breaking:** what every run stands on goes in the new `PrepareRun` schedule and what a continued run does after its restore in `RunResumed`; `SavePlugin` continues a run itself, so a game's own load-and-restore goes.
- **Breaking:** `LaunchArgs::from_env()` replaces `replay::seed` and `replay::args`.
- **Breaking:** a test imports from `atglyph::testing`, in place of `atglyph::ecs::testing` and `atglyph::ui::testing`.

### Removed

- **Breaking:** the open world (`rl-world`, `rl-overworld`, `StreamingPlugin`), `BuildContext`, the affix model, charts, and `OnUse`, which a `use` trigger replaces.
- **Breaking:** `ConsumablesPlugin`, `StatusPlugin`, `UnloadPlugin` and `InteractKey` are parts of the plugins they belong to; delete them from a game's plugin list.
- **Breaking:** the engine's system functions are private; order against the system sets they run in.
- The Corsair, Delve and Heist example games.

Full detail: [CHANGELOG.md at v0.4.0](https://github.com/rudehn/rl-engine/blob/v0.4.0/CHANGELOG.md).

## [0.3.0] - 2026-09-23

### Added

- Props, `PropsPlugin`: crates, levers, plates and consoles as content in a `props.ron`.
- Remains, `RemainsPlugin` with `LeavesRemains` per actor: what dies stays where it fell.
- `Effects`, one built list of effects from `EffectSpec`, which a game's content file can carry.
- `OnUse` and `Consumable`, through `ConsumablesPlugin`.
- `OffersPanel`, the screen that asks which of several things the player meant.
- `Tell`, a game's own log line said inside a turn.
- `CursorStyle`, how a screen marks the cell it points at.
- Nearby rows say what something is doing about the player (`Row::alert`), and wash with its health.
- A noise reading on the vitals strip.
- `MeleeAttack` and `RangedAttack` gain a `cost`, a `look`, and `new` constructors.
- The `ShootAtRange` and `Shadow` tactics.
- Prefabs turn and mirror, marks included.
- `app.reset_on_new_run::<R>()`, `app.reads::<M>()`, `ViewSet::Sight`, `TurnSet::Record` and `LandSet`.
- A reference page for every system in the guide.
- Benches for a turn, a pass and a frame.

### Changed

- **Breaking:** `Follow` and `Shadow` are one tactic, `Keep`.
- **Breaking:** `Morgue` and `Obituary` are gone; the run's end is a screen.
- **Breaking:** one airborne queue, `cue::Airborne<L>` with `Lands` and `app.add_airborne::<L>()`.
- `DamageStages` defaults to `SubtractArmor`.
- `CombatRng` and `AbilityRng` left the prelude.
- The `Turn` schedule runs on one thread.

### Fixed

- A mind no longer decides, turn after turn, on a step the move resolver refuses.
- A shot is narrated as a shot, and a shooter the player never saw is not named.
- A key held when the window lost focus no longer stays held.
- A prop can no longer seal a room off.

Full detail: [CHANGELOG.md at v0.3.0](https://github.com/rudehn/rl-engine/blob/v0.3.0/CHANGELOG.md).

## [0.2.0] - 2026-09-16

### Added

- Doors that open and close, throwing (`ThrowingPlugin`), equipping from the ground, and `GearScore`.
- `Wits`, what a mind is able to do whatever its brain would like, with `MINDLESS`, `ANIMAL` and `SAPIENT` presets.
- Fire and gas, `FirePlugin` and `GasPlugin` over `TileField<T>`, with the `Ignite` and `Emit` effects.
- `InSight` and `Focus`; look and targeting cursors that stop on things as well as actors; `AimFire`.
- `Controls`, the registry of every key a game answers to, and `ControlsPanel`; a held direction key repeats.
- Seed replay, `Recording` and `ReplayPlugin`, and screenshot capture.
- Particles and cues: a projectile lands after it is seen to fly.
- The character sheet, the inventory view, the ability menu and the game menu.
- `Loadout` and `Bestows`: what an actor fights with, and what worn things lend.
- `Bump(Direction)`: walking into a cell steps, opens or attacks, and `BumpRules` swaps with allies.
- The `NewRun` and `EndRun` schedules.
- The narrator, `NarratorPlugin` and `Phrasebook`.
- The save seam: `Saveable`, `RunSave` and `Stash`.
- `TileAppearance::load`, tile looks from RON.
- Every actor has its own sight, flow fields lead to any goal, and `Decision::Own` decides a game's own action.
- A fingerprint test, the tripwire for determinism.

### Changed

- **Breaking:** `Health` is `current` and `max`; the field was `hp`.
- **Breaking:** `Decision::Own(Box<dyn Choice>)` replaces `Decision::Game(u32)`.

### Removed

- `is_status_source`, and the door branch of `resolve_moves`.

### Fixed

- An item carried to another map and put down there stays on that map.
- The first turn of a run goes to the player, not to a monster spawned after it.

Full detail: [CHANGELOG.md at v0.2.0](https://github.com/rudehn/rl-engine/blob/v0.2.0/CHANGELOG.md).

## [0.1.0] - 2026-09-14

### Added

- The engine's crates in four tiers, the lower two free of Bevy.
- `RoguelikePlugins`, and a plugin per subsystem: combat, minds, statuses, items, abilities and light.
- `Seed`, from which every subsystem derives its stream.
- `Registries`, and `Names` with `NameRef<T>`, so a game's content names content and holds ids.
- `CombatRules`, with `hostile`, `hunts` and `allied`.
- Panels as views, collectors and presenters, over `UiPlugin`.
- `templates/starter`, a game to generate with `cargo generate`.
- The guide, and three examples: the tutorial, the delve and Corsair.

[Unreleased]: https://github.com/rudehn/rl-engine/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/rudehn/rl-engine/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/rudehn/rl-engine/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/rudehn/rl-engine/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/rudehn/rl-engine/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/rudehn/rl-engine/releases/tag/v0.1.0
