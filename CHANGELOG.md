# Changelog

A release is a tag, `v` and the workspace version.
The starter template pins the release it was written against, and a game made from it moves to a newer one by changing that tag and reading here what moved.
Pushing a tag publishes its release page from its section here, through `scripts/release-notes.sh`.

## Unreleased

- `cargo generate` instructions name the release with `--tag`, so the template and the engine it pins come from the same release.
- `Wits`, what a mind is able to do whatever its brain would like, with `MINDLESS`, `ANIMAL` and `SAPIENT` presets, carried as `Intelligence(Wits)`. A `Mind` is sapient unless its spawn says otherwise. `FleeWhenHurt` needs the wits to flee and `SearchLastKnown` the wits to search. `Snapshot` gains `wits`.
- Doors: `TileProps::opens_to` and `closes_to`, walking into a closed door opens it, `Close(Direction)` shuts one, and `DoorEvent` reports both. The standard `door_closed` and `door_open` now open and close. `FlowFields::approach` takes whether the mover opens doors.
- Throwing: `ThrowingPlugin`, `Throwable`, the `Throw` action, and `flight`, where a throw goes; `ItemEvent::Thrown`. The targeting cursor aims a throw from `AimThrow`, `TargetView` gains `throwing` and `what`, and `TargetPanel` names what is aimed from the view.
- `EquipFromGround`, for a turn and a half, and `GearScore`, what wearing an item is worth to a mind.
- `Wits` gains `PICKS_UP`, `EQUIPS` and `THROWS`, all in `SAPIENT`. `Snapshot` gains `missiles` and `items`; `Decision` gains `PickUp`, `EquipFromGround` and `Throw`; the `ThrowAtRange` and `Scavenge` tactics.
- What a dead actor carried falls where it died.
- Fixed: an item carried to another map and put down there stayed on the map it was picked up from, where nobody could see it or take it back. A carried item is on no map now.
- Fixed: `TargetViewPlugin` in a game without abilities failed on its first frame, for a message only `AbilitiesPlugin` registered.
- `TileField<T>`, a value per tile stepped a turn at a time by a rule that reads the field as it stood.
- Gas: `GasDef` and `gas::load` in `rl-rules`, and `Registries::gases`; `GasPlugin`, `Gases`, `Release`, `Vents` and `Breathed` in `rl-bevy`. Gas thick enough hides what is behind it through `WorldMap::set_veil`.
- Fire: `TileProps::burn`, which must name the tile a burnt tile leaves; `fire::spread` and `Tinder` in `rl-rules`; `FirePlugin`, `Fire`, `Kindle`, `Flammable`, `Burning`, `FireRules` and `FireEvent` in `rl-bevy`. Burning cells glow through `Lighting::set_glow`, and a mind will not step into fire.
- `Ignite` and `Emit` ability effects, registered by the fire and gas plugins.
- `ResolveSet::Fields`, between `Act` and `Effects`, with `FieldSet::Fire` before `FieldSet::Gas`.
- `FieldAppearance`: how the map view draws flames and gas. `EngineSave::fields` keeps both.
- `InSight`, what the player can see in the nearby list's order, and `Focus`, the one entity picked out of it. With nothing open, Tab steps the focus down the nearby rows and Escape lets go; `NearbyPanel` draws the row picked out and its map tile on the selection tone. `NearbyView` gains `focused`.
- Both cursors cycle `InSight` rather than the actors alone, entity by entity, so two things on one tile are two stops, and Shift with Tab steps back. They open on the focus when they can take it and move it as they go. `cursor::steer` takes the `Focus` and `Sighting` candidates; `cursor::ordered` and `next_of` are gone, for `focus::cycle`.
- The look cursor stops on things as well as actors. The targeting cursor cycles by `Aim::cycles_to`: an aim at the ground stops on anything in sight rather than on cells.
- `AimFire`, a shot through the targeting cursor with a `RangedAttack`; `TargetView` gains `firing`. `shot` is the line a shot flies, which `line_of_fire` now answers by.
- `panel::tint`, and `panel::bar` draws on the background its cells already have.
- `Controls`, the registry every key a game answers to is declared in, with `AddControls::add_control`, `ControlId`, `Chord` (a key with Shift or not, matched exactly), `Keys` (chords, the direction keys with or without Shift, or one of the engine's own bindings as an `EngineKey`, read from its resource when listed), and `ControlInput`, this frame's keys read through it. The engine's cursors, scrollback, overworld map and controls screen declare their own keys, in `finish`, so a game's groups are listed first.
- `ControlsPanel`: every declared control on one screen, grouped, in columns and pages, opened with `?` from `ControlsKeys`, and a `hint` row naming that key. Corsair, the delve, the starter and the tutorial's step 10 declare their keys once and read them by name, and their hand-typed key lines are gone. `?` in `RL_CAPTURE_KEYS`.

## 0.1.0

The first release.

- The engine's crates in four tiers: `rl-core`; `rl-grid`, `rl-mapgen`, `rl-world` and `rl-rules`, which never depend on Bevy; `rl-bevy`, `rl-render`, `rl-ui`, `rl-overworld` and `rl-save`; and `rl-engine`, the facade a game depends on.
- `RoguelikePlugins`, what every game adds, and a plugin per subsystem: combat, minds, statuses, items, abilities, lighting, stealth, streaming, facts.
- `Seed`, the one thing about randomness a game supplies; every subsystem derives its stream from it.
- `Registries`, every registry the engine reads, filled once; `Names` and `NameRef<T>`, so a game's own content names content and holds ids.
- `CombatRules::new(&sides).hostile(a, b)`, and `hunts` and `allied`.
- Panels as views, collectors and presenters, over `UiPlugin`, which owns the message log, with `add_tone` and `add_modal`.
- `templates/starter`, a game to generate with `cargo generate`.
- The guide in `docs/guide`, and three examples: the tutorial, the delve and Corsair.
