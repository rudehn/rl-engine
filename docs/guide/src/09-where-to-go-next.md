# Where to go next

Warren is a complete roguelike in about 450 lines and uses maybe a third of the engine.
The Systems pages are the reference for the rest, one per system, and each teaser below links to the page that covers it.

## Lighting

- **Add** one resource: `commands.insert_resource(Lighting::dark());`
- **You supply** a `LightSource` on a prop, an actor or an item, `Fuel` if it burns down, and `DarkSight` on whoever sees without one.
- **You get** the `Viewshed` split from [chapter 2](02-sight-and-light.md) starting to matter: `line` stays geometric, `visible` shrinks to what is lit, and a monster that sheds nothing is found only where a light reaches it.
  `Fuel` reports `LightEvent::BurntOut`.
- **Worked examples** `undermere`, where every place is seen dimly by its own ambient and a lantern, a brazier or a clump of glowcaps is what makes it bright.
  `foundry` below deck one, where the lights are out and the commando sees by a shoulder lamp and a helmet's dark sight.
- **Reference** [Sight and lighting](systems/sight.md).
- **Design** [`docs/design/lighting.md`](https://github.com/rudehn/rl-engine/blob/main/docs/design/lighting.md).

## Stealth

- **Add** `StealthPlugin`.
- **You supply** `Notice` on an observer, which is a certain radius, a chance beyond it, a bonus while the subject stands in light, and a memory; `Stealth` on a subject narrows both.
- **You get** a roll between being seen and being noticed.
  A monster that has not noticed you does not act on you, one that loses you searches where it last saw you, and `Watchers` answers who is watching whom for the panels.
- **Worked examples** `foundry`: a commando the droids have to notice, and a probe that signals and then sounds an alarm that brings the rest.
  `undermere` gives every monster a `Notice`, and a potion's darkness shrouds whoever stands in it.
- **Reference** [Stealth](systems/stealth.md).
- **Design** [`docs/design/stealth.md`](https://github.com/rudehn/rl-engine/blob/main/docs/design/stealth.md).

## Abilities

- **Add** `AbilitiesPlugin`.
- **You supply** abilities as data: an aim, a shape from the targeting footprints, costs, requirements, a cooldown and a list of named effects.
  The engine ships ten effects, seven registered together by `add_engine_effects` and three by the fire, gas and noise plugins, and a game registers its own with `add_effect`.
- **You get** a key that writes `AimAt`, a cursor the engine opens, a preview of what the shot would cover, and the turn spent.
  An item never lends an ability: what a potion does is its own triggers, which the engine lands when it is drunk or thrown, so a potion is a line of RON.
- **Worked examples** `undermere` has fourteen, `foundry` one, and `crates/rl-bevy/tests/genres.rs` loads five genres of them into one registry.
- **Reference** [Abilities](systems/abilities.md).
- **Design** [`docs/design/abilities.md`](https://github.com/rudehn/rl-engine/blob/main/docs/design/abilities.md).

## Statuses, stats and gear

- **Add** nothing for statuses, which are `CorePlugin`'s, and put the registries a game needs in `Registries`.
- **You supply** what wearing a thing does, on the thing: a worn blade carries the `MeleeAttack` it is swung with, a coat its `Armor`, and an affix what it `Bestows` on a stat.
- **You get** afflictions ticked by the turn through the damage pipeline, with stacking rules and cures; a stat block with a modifier accumulator; an equipment slot graph with displacement; and an affix model of prefixes and suffixes with level-scaled grants, weighted rolling and per-instance state.
  `Loadout` sums the gear at the blow and `fold_gear` keeps the stats current, so nothing is ever copied onto the wearer.
- **Worked example** `undermere` uses all of it.
- **Reference** [Statuses](systems/statuses.md) for the afflictions and the stat block, [Items and equipment](systems/items.md) for the slots, the affixes and the `Loadout`, [Uses and charges](systems/uses.md) for what a thing does when it is used and what that costs it, and [Ammunition](systems/ammunition.md) for what a shot spends.

## Quests

- **Add** `FactsPlugin`.
- **You supply** facts with a kind, a subject, an object and an amount, and objectives over them.
- **You get** `Quests` with prerequisite chains and a victory flag, and `Counters`, a ledger of named tallies.
  It is the grown-up version of [chapter 6](06-two-floors.md)'s `if the king died`.
- **Reference** [Statuses](systems/statuses.md), where `FactsPlugin` sits beside the statuses it shares a page with.

## Saving

- **Add** `SavePlugin` and a backend resource: files, memory or browser storage, all behind one trait.
- **You supply** `Saveable` on the component that marks each kind of thing your game spawns, saying how to write one down and spawn it again, registered with `save_kind`.
- **You get** the rest of the walk: where each thing stands, its health, its bag, its slots and its statuses, along with the scheduler's queue, the world's edits and places, and what has been explored.
  The envelope is versioned and refuses a mismatch instead of guessing, entities are remapped on the way back in, and `SavePlugin` keeps the save a turn behind the run so a closed window saves.
  It forgets the save when the run ends.
- **Worked example** Foundry's `save.rs`: four kinds and four resources, in about two hundred and fifty lines before its tests.
- **Reference** [Saving and the ending screen](systems/saving.md).

## The run's beginning and end

- **Add** nothing.
  `CorePlugin` has it.
- **You supply** a start system in `NewRun`, and whatever your game keeps of a run that the engine does not, forgotten in `EndRun`.
- **You get** the engine running your start again after every `Restart`, on a fresh seed or the same one, with the old run torn down first.
  `RunOver` ends a run, from the player's death unless `CombatRules` say otherwise, or from any condition of your own.
  `GameMenuPanel` opens over the ending and offers the next run, under whatever your game pushed onto `EndingView` about what the run came to.
- **Reference** [The turn loop](systems/turn-loop.md) for the schedules, and [Saving and the ending screen](systems/saving.md) for what is kept and what is shown at the end.

## A title screen

- **Add** `TitlePanel::new(rect)`, with the rectangle its rows stand in.
- **You supply** your picture, painted in `TitleSet::Backdrop`, and a start in `NewRun` that stands aside while `ContinueRun` is present, for the system of yours that restores the saved run.
- **You get** the first run held back until the player picks: Continue, in a game with `SavePlugin`, over a save this build can read, New Game, which asks before it gives a saved run up, Records and Settings when there is something behind them, and Exit.
  The pause and end menus offer Main menu, and with `SavePlugin` a run left that way is written down for Continue.
- **Worked examples** `undermere`, a night over a black lake, and `foundry`, a foundry's works.
- **Reference** [The title screen](systems/title.md).

## Records of every run

- **Add** `RecordsPlugin`, kept between launches by your `SavePlugin`, and `RecordsPanel::new(rect)` for the screen.
- **You supply** one `RecordRun` when your run is over: its score, a line saying what it came to, and the sections you want shown.
- **You get** every run filed best first and kept between launches in a slot of its own, the run's own record on the screen it ends on, and a screen listing them all, which the title opens.
- **Worked example** `undermere`, whose score is five lines and a total.
- **Reference** [Run records](systems/records.md).
- **Design** [`docs/design/records.md`](https://github.com/rudehn/rl-engine/blob/main/docs/design/records.md).

## A narrator

- **Add** `NarratorPlugin`.
- **You supply** nothing, or a `Phrasebook` with the phrases you would rather it used.
- **You get** every engine event spoken, split by who did what to whom, with names in the colours of the things they name.
  Reword a phrase, silence one, or read the `NarrationView` and say it your own way.
  A line of your own told from inside a turn is a `Tell`, written in `TurnSet::React`, and the narrator speaks it after what it answers and before whatever the next actor does.
- **Reference** [Narration](systems/narration.md).

## A world above the dungeon

- **Add** `StreamingPlugin`, and `rl-overworld` for a map screen.
- **You supply** the world's parameters.
- **You get** FBM noise, elevation banding, priority-flood hydrology, climate, scored site placement and a road router, streamed around the player with a seam hash so chunk edges agree, keeping your edits across unload and reload.
  `rl-overworld` draws it with a portal picker.
- **Worked example** none yet: no example game streams a surface.
- **Reference** [Places and streaming](systems/places.md) for the two kinds of map and the streaming, and [The overworld](systems/overworld.md) for the screen over them.

## Balance

`rl-rules::balance` scores threat and prints a band report over the `BandedTable` from [chapter 7](07-content-in-files.md).
No example game has a flag that prints it yet.

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
  The keys every roguelike reads alike, walking, the stairs, waiting, picking up, firing and throwing, are `PlayerKeysPlugin`, named in your own words; step 10 takes four of them and keeps the shove and the crusts in its own system.
- **Two presenters over one view**: `LogPanel` draws the last few lines along the bottom and `ScrollbackPanel` draws all of them on a screen, over the same log, and neither knows the other exists.
- **The forecast** in the look cursor is not the panel's arithmetic. `rl_rules::forecast` runs the average roll through the same mitigation pipeline a real blow goes through, so it cannot drift from the fight.
- **The screen** is cut once, and `RailLayout` cuts the usual one, a map with the log under it and a rail down the right, from a few numbers; its `RailScreen` hands back every rectangle, and `panels()` the rail's panels as a group you edit.

`examples/tutorial/src/bin/step10_panels.rs` is the worked example, with the rail, the look cursor, tones and a controls screen, and [`docs/design/ui.md`](https://github.com/rudehn/rl-engine/blob/main/docs/design/ui.md) is why it is shaped that way.
[Panels](systems/panels.md) is the reference for the split and for every view the engine ships, and [Controls, modals and cursors](systems/controls.md) for the declaration, the stack and the cursor.

## Testing without a window

The engine ships the test kit it uses itself, in `rl_engine::rl_bevy::testing`, and a game's tests use the same copy.

A headless app is `MinimalPlugins`, states and `CorePlugin`, plus the engine plugins your game uses and your own systems, exactly as `main` does minus the three that draw.
Two `update` calls start a run and deal the player its first turn; after that it is one per action.
Writing an intent is how the game plays itself, and a suite like Warren's runs in about forty milliseconds.

- `KeyScriptPlugin` and `press(&mut app, key)` play a key the way a keyboard does, so a test drives your real input system instead of writing intents by hand.
- `surface(&mut app)` stands an open test world up for a test that needs a map and not the game's own.
- `two_sides(&mut app)` inserts combat rules for two sides at war.
- `rl_engine::testing` is the same kit with the headless game beside it: `headless(seed, narrator, game)` builds the app above with your narrator and your plugin and saves kept in memory, `settle`, `pass_turns`, `stand_on`, `equip` and their kin drive it as the engine's own intents would, and `row`, `written` and `said` read the screen and the log back.

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
| `foundry` | Ten decks of a droid foundry; combat depth, with guns that run hot or dry, droids that shoot back, and a probe that shouts for them |
| `undermere` | A delve from a lakeside town down ten floors to a drowned king, written as a whole game; the engine's title and records, monsters written as traits, and square tiles |

## Reading further

- [`docs/OVERVIEW.md`](https://github.com/rudehn/rl-engine/blob/main/docs/OVERVIEW.md), the inventory of what exists and what does not.
- [`docs/PLAN.md`](https://github.com/rudehn/rl-engine/blob/main/docs/PLAN.md), why, decision by decision.
- [`docs/design/`](https://github.com/rudehn/rl-engine/tree/main/docs/design), one file per subsystem: how it works and why it is shaped that way.
  Twenty of them, and among those this chapter has not sent you to already are
  [fields](https://github.com/rudehn/rl-engine/blob/main/docs/design/fields.md) (fire and gas),
  [minds](https://github.com/rudehn/rl-engine/blob/main/docs/design/minds.md) (how a non-player decides),
  [noise](https://github.com/rudehn/rl-engine/blob/main/docs/design/noise.md) (what a sound is and who hears it),
  [props](https://github.com/rudehn/rl-engine/blob/main/docs/design/props.md) (crates, levers, containers and traps) and
  [remains](https://github.com/rudehn/rl-engine/blob/main/docs/design/remains.md) (what is left where something died).
- [`docs/TODO.md`](https://github.com/rudehn/rl-engine/blob/main/docs/TODO.md), the work that has been found and not started, if you would rather build the engine than a game on it.
- [`AGENTS.md`](https://github.com/rudehn/rl-engine/blob/main/AGENTS.md), the rules the build enforces and the rules review enforces.
