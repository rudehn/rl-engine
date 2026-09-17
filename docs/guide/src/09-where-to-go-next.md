# Where to go next

Warren is a complete roguelike in about 450 lines and uses maybe a third of the engine.

## Lighting

- **Add** one resource: `commands.insert_resource(Lighting::dark());`
- **You supply** a `LightSource` on a prop, an actor or an item, `Fuel` if it burns down, and `DarkSight` on whoever sees without one.
- **You get** the `Viewshed` split from [chapter 2](02-sight-and-light.md) starting to matter: `line` stays geometric, `visible` shrinks to what is lit, and a monster that sheds nothing is found only where a light reaches it.
  `Fuel` reports `LightEvent::BurntOut`.
- **Worked examples** `heist`, where wall lamps are the only light, snuffing one is how you cross a room, and the watch light them again.
  `delve` below the Maw: a brand that can be smothered, a torch to set down, and `v` to see the light as digits.
- **Design** `docs/design/lighting.md`.

## Stealth

- **Add** `StealthPlugin`.
- **You supply** `Notice` on an observer, which is a certain radius, a chance beyond it, a bonus while the subject stands in light, and a memory; `Stealth` on a subject narrows both.
- **You get** a roll between being seen and being noticed.
  A monster that has not noticed you does not act on you, one that loses you searches where it last saw you, and `Watchers` answers who is watching whom for the panels.
- **Worked examples** `heist`: a thief the watch have to notice, a pebble that draws them to the wrong corner, and a shout that brings the rest.
  `delve` is built around it and Corsair's caves use it.
- **Design** `docs/design/stealth.md`.

## Abilities

- **Add** `AbilitiesPlugin`.
- **You supply** abilities as data: an aim, a shape from the targeting footprints, costs, requirements, a cooldown and a list of named effects.
  The engine ships seven effects and a game registers its own with `add_effect`.
- **You get** a key that writes `AimAt`, a cursor the engine opens, a preview of what the shot would cover, and the turn spent.
  An item that `Grants` an ability lends it to whoever carries it, and a `Charge` cost is spent from the item, so a potion is a line of RON.
- **Worked examples** `delve` has five, `corsair` four, and `crates/rl-bevy/tests/genres.rs` loads five genres of them into one registry.
- **Design** `docs/design/abilities.md`.

## Statuses, stats and gear

- **Add** `StatusPlugin`, and put the registries a game needs in `Registries`.
- **You supply** what wearing a thing does, on the thing: a worn blade carries the `MeleeAttack` it is swung with, a coat its `Armor`, and an affix what it `Bestows` on a stat.
- **You get** afflictions ticked by the turn through the damage pipeline, with stacking rules and cures; a stat block with a modifier accumulator; an equipment slot graph with displacement; and an affix model of prefixes and suffixes with level-scaled grants, weighted rolling and per-instance state.
  `Loadout` sums the gear at the blow and `fold_gear` keeps the stats current, so nothing is ever copied onto the wearer.
- **Worked example** `corsair` uses all of it.

## Quests

- **Add** `FactsPlugin`.
- **You supply** facts with a kind, a subject, an object and an amount, and objectives over them.
- **You get** `Quests` with prerequisite chains and a victory flag, and `Counters`, a ledger of named tallies.
  It is the grown-up version of [chapter 6](06-two-floors.md)'s `if the king died`.

## Saving

- **Add** `SavePlugin` and a backend resource: files, memory or browser storage, all behind one trait.
- **You supply** `Saveable` on the component that marks each kind of thing your game spawns, saying how to write one down and spawn it again, registered with `save_kind`.
- **You get** the rest of the walk: where each thing stands, its health, its bag, its slots and its statuses, along with the scheduler's queue, the world's edits and places, and what has been explored.
  The envelope is versioned and refuses a mismatch instead of guessing, entities are remapped on the way back in, and `SavePlugin` keeps the save a turn behind the run so a closed window saves.
  It forgets the save when the run ends.
- **Worked example** Corsair's `save.rs`: four kinds and four resources, in about four hundred lines.

## The run's beginning and end

- **Add** nothing.
  `CorePlugin` has it.
- **You supply** a start system in `NewRun`, and whatever your game keeps of a run that the engine does not, forgotten in `EndRun`.
- **You get** the engine running your start again after every `Restart`, on a fresh seed or the same one, with the old run torn down first.
  `RunOver` ends a run, from the player's death unless `CombatRules` say otherwise, or from any condition of your own.
  `GameMenuPanel` opens over the ending and offers the next run, and `Morgue` writes the run down.

## A narrator

- **Add** `NarratorPlugin`.
- **You supply** nothing, or a `Phrasebook` with the phrases you would rather it used.
- **You get** every engine event spoken, split by who did what to whom, with names in the colours of the things they name.
  Reword a phrase, silence one, or read the `NarrationView` and say it your own way.

## A world above the dungeon

- **Add** `StreamingPlugin`, and `rl-overworld` for a map screen.
- **You supply** the world's parameters.
- **You get** FBM noise, elevation banding, priority-flood hydrology, climate, scored site placement and a road router, streamed around the player with a seam hash so chunk edges agree, keeping your edits across unload and reload.
  `rl-overworld` draws it with a portal picker.
- **Worked example** `corsair`.

## Balance

`rl-rules::balance` scores threat and prints a band report over the `BandedTable` from [chapter 7](07-content-in-files.md).

```sh
cargo run -p corsair -- --balance
```

## Panels, past the two you have

Warren uses a status strip and a log, added in [chapter 3](03-blows-and-the-log.md) when there was something to put in them.
The engine has four more views and the machinery behind all of them.

A panel is split in three, and the split is why any of it belongs in an engine.
The **view** is a resource of plain data, rows and bars and numbers, with no colour and no string the game did not supply.
The **collector** refills it in `ViewSet::Collect`.
The **presenter** draws one, in a `PresentSet` layer, taking its rectangle in its constructor.
"Every actor in the viewshed, nearest first, with a health fraction and a relation" is the same sentence in every roguelike; a gold-ruled rail with small-caps headings is one game's taste.

That gives five places to stop, and you can stop at any of them: add the panel and be done, change the `Palette` and restyle everything at once, push a `Facet` for what the engine cannot know, keep the view and draw it yourself, or add neither.

- **What the engine cannot know** is a `Facet`: a key, some words and a tone, pushed onto a row in `ViewSet::Annotate`. Warren already does this for the crusts in your bag and the floor you are on.
- **Colours** are never passed to a widget. Every widget takes a `ToneId`, a semantic role the `Palette` turns into a colour, and `add_tone` declares a role and colours it in one call.
- **Screens** are a stack of interned ids in `Modals`, with `no_modal` and `modal_is` as run conditions, so one gate on your input covers every screen you ever add.
- **Keys** can be declared once in a `Controls` registry and read back by name, which is what lets `ControlsPanel` show exactly the keys the game reads.
- **Two presenters over one view**: `LogPanel` draws the last few lines along the bottom and `ScrollbackPanel` draws all of them on a screen, over the same log, and neither knows the other exists.
- **The forecast** in the look cursor is not the panel's arithmetic. `rl_rules::forecast` runs the average roll through the same mitigation pipeline a real blow goes through, so it cannot drift from the fight.

`examples/tutorial/src/bin/step10_panels.rs` is the worked example, with the rail, the look cursor, tones and a controls screen, and `docs/design/ui.md` is why it is shaped that way.

## Testing without a window

The engine ships the test kit it uses itself, in `rl_engine::rl_bevy::testing`, and a game's tests use the same copy.

A headless app is `MinimalPlugins`, states and `CorePlugin`, plus the engine plugins your game uses and your own systems, exactly as `main` does minus the three that draw.
Two `update` calls start a run and deal the player its first turn; after that it is one per action.
Writing an intent is how the game plays itself, and a suite like Warren's runs in about forty milliseconds.

- `KeyScriptPlugin` and `press(&mut app, key)` play a key the way a keyboard does, so a test drives your real input system instead of writing intents by hand.
- `surface(&mut app)` stands an open test world up for a test that needs a map and not the game's own.
- `two_sides(&mut app)` inserts combat rules for two sides at war.

Where a property exists, assert it over a range of seeds: that every generated floor has somewhere to stand and somewhere to go is forty-eight floors of evidence that costs milliseconds, because generation is tier 1 and never builds an `App`.
Where no property exists, use a fingerprint test and say so in its name, so a change reads as a change rather than a failure.
Test the refusal as well as the action: a wrong refusal crashes nothing, it silently eats a turn or freezes the loop.

The test module of `step10_panels.rs` is the worked example.


## The crates

| Tier | Crates | Bevy |
|---|---|---|
| 0 | `rl-core` | no |
| 1 | `rl-grid`, `rl-mapgen`, `rl-world`, `rl-rules` | no |
| 2 | `rl-bevy`, `rl-render`, `rl-ui`, `rl-overworld`, `rl-save` | yes |
| 3 | `rl-engine` | facade |

Map generation, field of view, pathfinding, the damage pipeline and the AI brains are tier 1.
They run headless, test in milliseconds and build for WebAssembly, so the tests in [chapter 9](09-where-to-go-next.md) cost milliseconds, and CI enforces the boundary.
A tool that needs only one of them can depend on that crate alone and never compile Bevy.

## The examples

| Example | What it shows |
|---|---|
| `delve` | Five floors of a beached whale, no surface at all; lighting, stealth and five knacks; `floors.rs` is the whole map builder |
| `corsair` | An open-world pirate roguelike with a pirate's abilities, built only on the public API |
| `heist` | Three floors of a counting house in the dark; stealth and light end to end, with a score to carry out |

## Reading further

- `docs/OVERVIEW.md`, the inventory of what exists and what does not.
- `docs/PLAN.md`, why, decision by decision.
- `docs/design/`, one file per subsystem: how it works and why it is shaped that way.
- `AGENTS.md`, the rules the build enforces and the rules review enforces.
