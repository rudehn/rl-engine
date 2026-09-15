# Changelog

A release is a tag, `v` and the workspace version.
The starter template pins the release it was written against, and a game made from it moves to a newer one by changing that tag and reading here what moved.
Pushing a tag publishes its release page from its section here, through `scripts/release-notes.sh`.

## Unreleased

- `cargo generate` instructions name the release with `--tag`, so the template and the engine it pins come from the same release.
- `Wits`, what a mind is able to do whatever its brain would like, with `MINDLESS`, `ANIMAL` and `SAPIENT` presets, carried as `Intelligence(Wits)`. A `Mind` is sapient unless its spawn says otherwise. `FleeWhenHurt` needs the wits to flee and `SearchLastKnown` the wits to search. `Snapshot` gains `wits`.
- Doors: `TileProps::opens_to` and `closes_to`, walking into a closed door opens it, `Close(Direction)` shuts one, and `DoorEvent` reports both. The standard `door_closed` and `door_open` now open and close. `FlowFields::approach` takes whether the mover opens doors.

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
