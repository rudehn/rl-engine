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
- Particles: `rl-render` gains `Particles`, an `Animation` of `Trail` and `Burst` steps drawn from the wall clock over the map, and `ParticlesPlugin`, in `RoguelikePlugins`, which plays a flight along the path and a burst over the cells for every ability used, in the ability's new optional `look: (glyph:, color:)`, and a flight for every item thrown in the item's glyph. `AbilityEvent::Used` and `ItemEvent::Thrown` carry the `path` (and the cells) for it.
- An aim past where its shape reaches, or stopped short by something in the way, is refused as `Blocked::OutOfReach` rather than landed where it stopped. The targeting overlay paints the part of the line past the stop in the bad tone (`TargetView::beyond`), and the flight and that segment carry a pulse that runs from the user towards the cursor. The shot cursor says out of reach rather than no target when that is what is wrong.
- Fixed: both games narrated a use of an ability after the blows it landed; narration is chained in the order things happen.
- The ability menu is the engine's end to end: `AbilityKeys` opens it (`a`), the direction keys walk it, confirming a row writes `AimAt` for it, and the row picked out is described from its data: `AbilityDef::description` (new, optional in RON), its aim and shape in a phrase, what it costs and needs with the registries' names, how long it takes and until it is ready again, what each effect does through `Effect::describe` (new, with a default that says nothing), and why it cannot be used in words. `AbilityRow` gains `description`, `mode`, `time`, `cooldown`, `costs`, `requires`, `effects` and `why`; `AbilityPanel::hints` is gone and `called` names what the list lists on the controls screen. The delve's and Corsair's hotkeys work over the open list.
- Fixed: the key that closes a screen was read again as the world's in the same frame, so the Enter that confirmed an aim could also be the Enter that takes the stairs, and the stairs, refused, spent the turn the aim was for. `Modals::any_open` stays true for the rest of the frame a screen closes on, and the targeting cursor is not steered on the frame it opens.
- `SheetView` and `SheetPanel`, the character sheet: every registered stat with its base, its value and each modifier between them tagged by source, resists, the blows and the shot, statuses with their turns left and what they do, and every slot. A status names its own modifiers; a game names its items' with `SheetView::name_source`. Opened by `SheetKeys`, `c` by default, and listed on the controls screen as `EngineKey::OpenSheet`. Corsair has it on `@` and the delve on `c`. A Shift and digit chord now labels as the symbol it types on a US layout, `@` for Shift and 2.
- `Stash` and `UnloadPlugin`: the last save a game encoded, written through `Saves` when a browser page is hidden or unloaded and on the frame the app exits, so a closed tab or window keeps the run. `Saves` now holds its backend in an `Arc` (`Saves::new(backend)`), so the page's handler writes through the same one. Corsair stashes the run once a turn and clears the stash on death.
- Corsair's sea chest and ledger write their bottom-border hints from the registry when they open.
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
