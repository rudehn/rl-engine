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
