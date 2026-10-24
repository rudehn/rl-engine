# Places, and a run's start

Status: places built 2026-09-10, as dungeons under the open world, and every map a place since 2026-10-19; the place model moved to tier 1, each place given its own seed, and the run's start and its continuing made the engine's, all on 2026-10-22 and 2026-10-23, as usability branches 4, 16 and 18 of `docs/reviews/engine_usability.md`.
The reasoning is here; `docs/OVERVIEW.md` lists what exists, and `docs/guide/src/systems/places.md` and `docs/guide/src/systems/seeds.md` are the reference.

## 0. Summary

A place is a bounded map, built once by the game's `PlaceRules` the first time anything enters it and kept whole after, actors and items included: leaving freezes what is in it, returning finds it as it was.
What a place is, `MapId`, `PlaceRules`, `PlaceBuild`, `Ladder` and the rest, is plain data in `atglyph_mapgen::place`, so a terminal game or a tool builds the very places the framework does.
Each place is built from its own `PlaceSeed`, derived from the run's seed and the place's number alone.
A game hands its rules over once with `app.places(rules)`, spawns its player with `commands.spawn_player(map, bundle)`, and play begins on its own.

## 1. The model is tier 1

`MapId`, `Spot`, `Lock`, `PlaceBuild`, `PlaceRules`, `Ladder`, `Climb` and `StairLook` lived in `atglyph-bevy`, though they are plain data; only a `Component` derive on `MapId`, which nothing used, tied them to Bevy.
So a game without the framework, `examples/terminal` or a level tool, could run a mapgen chain but not build a place, and wrote its own copy of what one is.

They moved to `atglyph-mapgen` rather than `atglyph-rules`, for the reason `atglyph-rules` depends on `atglyph-mapgen` and not the reverse: a map-only user should not compile every rule to build a floor.
`Lock::key` is the raw tag id `Join::Locked` carries, since the crate sits below the rules that name tags, and a save writes it as the same number, so saves loaded unchanged.
`atglyph::ecs` re-exports every one of them, so a framework game's paths did not move; what stays in the Bevy layer is the components, the warp and the travel.

`Ladder::stairs` is the floors' stairs as plain data, `LadderStair`s saying where each stands, which way it goes, how it looks and where it leads, and the framework lays them as `Stair` and `Transition` entities; a toolkit game lays them its own way from the same list.

## 2. A seed per place

`PlaceRules::build` was handed no seed, so every game made one: four tutorial steps and Foundry mixed the depth into the run's seed by hand with a shift and an exclusive or, which the guide then quoted, and one Undermere floor drew from three roots.
`PlaceRules::build(map, seed)` is handed the place's own `PlaceSeed`, and the engine draws everything it places on a place, its scattered loot, its containers and its slots, from the same seed.

Derived from the run's seed and the place's number alone, so building floor two before floor one cannot change floor one, and a floor built again after a failed attempt is built from the same seed.
A seed in its own right rather than the run's seed and an index carried together, so a generator handed one cannot reach the run's other streams by mistake.
A warp into a place not yet built, in a game with no `Seed`, builds nothing and fails naming the missing seed, rather than building every place from one constant.

## 3. The run's start is one call each

Every tutorial step, both games and the template inserted a `WorldMap` and the place rules, spawned the player at the origin and warped it into its first place, and set the engine playing, at twelve and fourteen sites over, with a warning after 120 frames for a game that forgot the last.

`app.places(rules)` is where the game is put together, not in its start, and says the game needs a `Seed`; a run whose start makes no `WorldMap` gets one built over `PlaceRules::tiles`, and a start that makes its own keeps it.
`commands.spawn_player(map, bundle)` spawns an actor that blocks, with the game's bundle, and warps it to the place's entry once the spawn is applied.
Play begins once a start has built a `WorldMap` and no `HoldStart` holds it, in the frame a start used to set it, so a start that still sets it by hand works and one that forgets it is not a bug.

## 4. Continuing a run is the engine's

A game's `NewRun` start used to ask whether `ContinueRun` stood, and if so load the slot, set its seed, rebuild its places, restore the run and fix up what the restore did not carry, by hand, in a `resume` beside its start.
Every game wrote the same steps in the same order, and the order is the engine's to know: the places must be standing before the run is restored into them.

So a run's start is three schedules.
`PrepareRun` is what every run stands on, fresh or continued, rules, places, the `WorldMap` and a mission loaded fresh, run once the run's seed is set.
`NewRun` is a fresh run's own start, the player and its first place, and a continued run never runs it.
`ContinueSaved`, which `SavePlugin` fills, loads the save, sets its seed, runs `PrepareRun`, restores the run and runs `RunResumed`, where a game puts what it does after the restore.
`begin_run` is the one door: `PrepareRun` then `NewRun` on a fresh run, `ContinueSaved` while `ContinueRun` stands.

A save that cannot be read, or that restores without exactly one player, begins a fresh run instead, on the seed that stood before the save's rather than the save's.
A game with no `SavePlugin` that is asked to continue is warned and begins afresh, through `begin_afresh`.
A fresh run's world is built from the places' tiles after `NewRun`, not between the schedules, so a start that inserts its places itself still gets one.

## 5. What a place keeps

A place keeps what its builder said of it with its map and its save, rather than in a flag a game keeps in step with whichever map is current: its locks, its secrets, its ambient light and whether it is peaceful.
Each was a game's own system first, Undermere's town's peace and both games' ambient set from the current map every frame, and each moved onto the build when a second thing needed to read it.

## 6. What is not here

- Places streamed in around the player: the open world went on 2026-10-19 with no example using it, and every map is a place.
- A place that rebuilds or decays while nobody is on it: a place is frozen, and what changes it is the game's.
