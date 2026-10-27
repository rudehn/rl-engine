# Changelog

All notable changes to atglyph are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
atglyph is not on crates.io yet, so a game depends on a release by its git tag: `atglyph = { git = "https://github.com/rudehn/rl-engine", tag = "v0.4.0" }`.

## [Unreleased]

### Changed

- [Undermere](https://github.com/rudehn/undermere) and [Foundry](https://github.com/rudehn/foundry) moved to repositories of their own; a game changes nothing.

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
- `LaunchArgs::from_env()`, and `SavePlugin` continuing a run itself through `PrepareRun` and `RunResumed`.
- Rules as tier 1 functions in `atglyph-rules`, testable without an `App`: damage, faction stance, bumps, trade, equipping, work and more.
- `atglyph::bevy` and `atglyph::rand` re-export the Bevy and the `rand` the engine is built on.
- Every crate packages for crates.io.

### Changed

- **Breaking:** the engine is `atglyph`: `rl_engine::` is `atglyph::`, the crates are `atglyph::primitives`, `ecs`, `grid`, `mapgen`, `rules`, `render`, `ui`, `save` and `tty`, and `RL_*` variables are `ATGLYPH_*`.
- **Breaking:** every seed plays differently; re-baseline fingerprints and seed-pinned tests once.
- **Breaking:** `use atglyph::prelude::*` holds only what a first game names; `atglyph::prelude::all::*` is everything, and topic modules hold the rest.
- **Breaking:** each crate's `App` methods come through one trait, `EngineApp`, `UiApp` or `SaveApp`, which the prelude brings.
- **Breaking:** `Afflict`, `Cure` and `Afflicted` are `ApplyStatus`, `RemoveStatus` and `ActiveStatuses`; `TurnHold` is `CueWait`; `Strikes` is `BonusDamage`; `Reach` is `DamageRoute`; `Rect` is `GridRect`; `Moment` is `TriggerKind`.
- **Breaking:** a system writes to the log through the `Log` param, `log.notice(text)`, and a facet is `Facet::new("key", text)`.
- **Breaking:** `PlaceRules::build(map, seed)` is handed the place's own `PlaceSeed`, and the place model is tier 1, in `atglyph::mapgen::place`.
- **Breaking:** `app.places(rules)` and `commands.spawn_player(map, bundle)` replace setting up the world by hand, and play begins on its own.
- **Breaking:** a draw is `seed.rng(DOMAIN, index)` over a `SeedDomain`, and `CombatRules::new()` takes no registry.
- **Breaking:** a test imports from `atglyph::testing`.

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

[Unreleased]: https://github.com/rudehn/rl-engine/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/rudehn/rl-engine/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/rudehn/rl-engine/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/rudehn/rl-engine/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/rudehn/rl-engine/releases/tag/v0.1.0
