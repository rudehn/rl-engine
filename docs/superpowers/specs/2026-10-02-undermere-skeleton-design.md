# Undermere, part 1: the skeleton

Status: written 2026-10-02, against `main` at `c10fd9e`.
The game design is `examples/undermere/DESIGN.md`; this is how its first part is built.
The user set the direction (a real game in this repo, skeleton first, victory on the floor ten boss, an itemized score, square tiles, traits) and then asked for the rest to be decided without further questions, so every other call below is the author's, made to be reviewed afterwards.

## Goal

The whole loop, playable and thin: a title menu with Continue, New Game, Records, Settings and Exit; a town with a gate; ten floors; a starter bestiary and armory written in the trait model; the floor ten boss whose death wins; an end screen with an itemized score and the run's numbers; and the records of every run, kept between launches.
Every later part drops into a game that already plays from the title to the end screen.

## Engine work

Two things a second game needs and only Foundry has, or nobody has, so by the house rule they are the engine's.

### 1. The title menu

Foundry's `title.rs` is about three hundred lines of menu logic and eight hundred of art.
A second game would copy the logic, which is the failure `docs/PLAN.md` section 1 was written against, so the logic moves into the engine and the art stays with each game.

What moves, split by crate the way settings already are (the data in `rl-bevy`, the slot in `rl-save`, the screen in `rl-ui`, with no edge between the last two):

- **`rl-bevy`**: the hold on the first run.
  Today `begin_first_run` runs `NewRun` at startup unconditionally, and Foundry's `run::start` returns early while its title is up.
  A resource, `HoldStart`, present while a title screen stands, makes `begin_first_run` do nothing; whoever lowers the hold calls `begin_run` (the same `NewRun` the engine runs at startup and after a restart), so no game's start system checks a title flag.
  `SavedRun`, plain data, is what the run's save slot holds: `Empty`, `Readable` or `Unreadable`.
  `ContinueRun`, a marker resource, asks the game's `NewRun` systems to restore the saved run rather than begin one, replacing Foundry's `Resume`.
  `AbandonRun`, an event triggered on the world, asks whoever keeps the slot to forget it.
- **`rl-save`**: `SavePlugin` fills `SavedRun` once at startup by reading its slot as Foundry's `look_for_save` does, keeps it current as the slot is written and deleted, and observes `AbandonRun` by clearing the `Stash` and deleting the slot, as Foundry's `save::abandon` does.
  An observer, so the deletion has happened by the time the title begins the new run in the same command.
- **`rl-ui`**: `TitlePanel::new(rect)`, a plugin holding the rectangle its menu draws in.
  It inserts `HoldStart` and a `TitleMenu` state, and offers rows in this order, each only when it can be taken: Continue (a `Readable` save), New Game, Records (when `RecordsPanel` was added and there is a record), Settings (when `SettingsPanel` was added and a setting is declared), Exit.
  It behaves as Foundry's does today: the direction keys move and skip a row that cannot be taken, confirm takes it, close exits; Continue inserts `ContinueRun` and begins the run; New Game over any save asks "Abandon the run in progress?" with No picked out, and Yes triggers `AbandonRun` and begins; Settings and Records open their screens over the menu, the settings screen moved to stand over the menu while the title is up and put back when a run begins.
  Its words are the game's to change with `.words(...)`, as `GameMenuPanel`'s are.
  Drawing is two named sets, `TitleSet::Backdrop`, where a game paints its picture, and `TitleSet::Menu`, where the panel draws the rows over it, both run only while the title is up and the engine is idle, outside every play-gated set.
- **Back to the title.**
  A game with a title should be able to get back to it, so `GameMenuPanel` offers a "Main menu" row when a `TitlePanel` was added: from the pause menu it writes the run down first, so Continue picks it up, and from the end screen it simply leaves.
  Either way the run is torn down through the path a `Restart` takes (`EndRun`, `clear_run`, idle), and the title is raised instead of a new run begun.
- **Foundry** moves onto it: its `title.rs` keeps the works, painted in `TitleSet::Backdrop`; its `Title`, `Choice`, key handling, `look_for_save`, `Resume` and `save::abandon` go; `run::start` no longer checks a title.
  Its title tests move to `rl-ui` as tests of the engine's panel, and what is left of Foundry's are the art's.

### 2. Run records

No game keeps its runs, and Undermere's records screen needs them kept between launches.
The same three-way split:

- **`rl-bevy`**, `records`: `RunRecord`, plain data: how it ended (`Won`, `Died`, `Abandoned`), the game's one-line summary, the score, the seed, the turn, when it ended as seconds since the Unix epoch (`None` on wasm, where the engine has no wall clock), and sections of heading and body, the same shape as the ending screen's.
  `RunRecords`, the list, best score first, at most a hundred.
  `RecordRun`, the message a game writes once when its run ends, carrying the score, the summary and the sections; `RecordsPlugin` completes it from the `Ending`, the `Seed` and the clock, files it, and remembers which record is this run's.
- **`rl-save`**: `RecordsSavePlugin`, the records in a `records` slot apart from the run and never deleted with it, read before the first frame and written when one is added, as `SettingsSavePlugin` does for settings.
  RON, with every field defaulted, so a later build reads an earlier build's records.
  A slot that cannot be read is warned about and replaced by the next record, since a list that cannot be parsed cannot be added to.
- **`rl-ui`**: `RecordsPanel::new(rect)`, a modal screen listing the records best first, one row each (rank, score, summary, date), the direction keys walking them and confirm opening the one picked out to show its sections, close going back a level.
  The ending screen shows this run's record: `collect_ending` pushes the sections of the record `RecordsPlugin` filed for the run, so a game that records its run never pushes the same sections a second time, and the end screen and the records page cannot disagree.

`docs/design/records.md` is new, for the reasoning: why a record is the game's words and not the engine's counters, why it lives apart from the run's slot, and why the ending screen reads it.

## The game

`examples/undermere`, a library with a thin binary, in Foundry's shape: one `UndermerePlugin` that the window and the headless tests both add, so a test plays the game that ships.

### The screen

`RoguelikePlugins::new("Undermere", 160, 45).across(2)`: square map tiles two cells wide.
A rail of thirty-four columns down the right (vitals, gear, nearby with what each monster is doing, the controls hint), one blank gutter column, the log's five rows along the bottom of the left, and the map above it, about 62 by 40 tiles.
Inspect, targeting, inventory, controls, menus, settings and records float over the map, cut once in a `Screen` the window and the screen tests share, as Foundry's is.

### Places

The town is `MapId(100)` and floor `n` is `MapId(n)`, one `PlaceRules` building all eleven, so a floor number reads straight off its map.
Every place is built on first arrival and kept whole, as the engine does for any place.

- **Mereholt** is one hand-drawn piece, `assets/prefabs/mereholt.ron`, stamped on a blank map: streets, the tavern, the smithy, the apothecary, the temple and houses as buildings with doors and nothing inside yet, a shore of the black lake, lamps, and the Drowned Gate, a ring of standing stones around a gate tile.
  The piece marks where a run arrives and where the gate is.
  Dusk: a low ambient, lamps and lit windows.
- **Floors** use the engine's passes as they are on `main`, varied by biome so the bands look and play differently from the first build: the Upper Halls (1-2) rooms and doors, the Fungal Deeps (3-4) cellular caves, the Catacombs (5-6) BSP and doors, the Drowned Cathedral (7-8) rooms with scattered deep water, the Forge of Embers (9) caves with lava, and the Throne (10) rooms with the throne piece stamped in one.
  Each biome has its own wall and floor tiles, its ambient light and its fixtures, so a band is told apart at a glance.
  All of it sits behind one `dungeon.rs`; part 4 replaces what is inside it.
- **Ways through**: the gate in town leads to floor one's entry, floor one's way up is the gate back, and every other floor has stairs up at its entry and down at its exit, laid on first arrival as Foundry's lifts are.

### Content, in the trait model

Every file in `assets/` begins with a comment listing every field it may hold.

- `traits.ron`: a list of `(name, ...)` where every other field is optional: `wits` ("mindless", "animal", "sapient" or a list as the engine's `Wits` reads), `speed` (percent), `perception`, `dark_sight`, `flee_at`, `notice`, `hearing`, `light` (radius, colour, flicker), `resists` (list of kind and percent), `armor`, `abilities` (names in `abilities.ron`), `statuses` (held for life), and `says`, the examine panel's words for it.
- `monsters.ron`: `name`, `glyph`, `color`, `hp`, `attack` written `"1d4 slash"`, `faction`, `traits`, and optionally `ranged` written `"1d6 pierce @6"`, `armor`, `boss` and `drops`.
- `items.ron`: Foundry's item schema trimmed to what the skeleton uses (slot, either, also, tags, melee, ranged, throw, armor, resists, effects, triggers, consumable, light), plus `lends`, the traits it gives its wearer while worn.
- `abilities.ron`, `props.ron`, `tiles.ron`, `monster_spawns.ron`, `item_spawns.ron`, `quests.ron` and the prefabs, each in the engine's own schema where one exists.

A trait resolves at load: a monster's core and its traits fold into one definition, every name checked through `Names`, and a single value set by two of one monster's traits refused at load naming both.
Lists add up.
The resolved definition keeps its trait names, which the examine panel lists with each trait's `says`.

The starter content, enough to play to the bottom and no more, since parts 2 and 3 are where the depth goes:

- Factions: the adventurer, beasts, goblins, undead; the adventurer at war with all three, and beasts with goblins, so the dungeon fights itself.
- Damage kinds: slash, pierce, crush, fire, cold, poison.
- About a dozen monsters across the bands, one trick each above the first floors and none on them, and the Drowned King on floor ten with a court of skeletons placed by the throne piece's monster slots.
- About twenty items: dagger, sword, axe, mace, spear, a bow and arrows, leather, chain and plate, a shield, a helm, a lantern and torches, potions of healing, a fire flask that bursts into flame where it lands, and gold.

### The run

`run::start` builds the dungeon, spawns the adventurer with a sword, leather armour, a lit lantern and two potions of healing, and warps them into Mereholt.
`run::resume` is Foundry's, answering the engine's `ContinueRun`.
The quest file has one quest, slaying the Drowned King, marked `victory`; its fact is written when a monster marked `boss` dies.

### Score, stats and the record

`tally.rs` keeps a `Tally` of the run (saved with it): kills, the threat slain (the engine's `balance::threat` of each kill, summed), the deadliest kill, gold found (counted when a stack of gold is first picked up, never twice), damage dealt and taken, potions drunk, items found, mini-bosses slain, and the deepest floor.
When the run ends, `tally::record` writes one `RecordRun`: the score as the design's five lines, the summary ("Slew the Drowned King", "Slain by a goblin on floor 3"), and two sections, Score and The run.

### Saving

Foundry's pattern: the slot `undermere`, written on the way out and on every arrival, deleted by death and by victory; kinds for the adventurer, a monster by definition name, an item by definition name with its stack, and a stair or gate; and the `Tally`, the deepest floor, `Quests` and `Counters` as states.
A scripted run (replay, record, capture) saves in memory, as Foundry's does.

### The title

`title.rs` is only the picture, painted in `TitleSet::Backdrop`: the black lake under stars, a moon and its broken reflection, mist, Mereholt's windows on the left, the gate's stones on the right with a violet glow, and UNDERMERE lettered large.
Three things move and only three: the stars twinkle, the gate's glow breathes, the mist drifts.

## Testing

- Content: every file loads; every trait and every monster's trait list resolves; two traits setting one value are refused, by name; every spawn row and every drop names something real.
- Places: every floor and the town build over a span of seeds, with an entry and a way on; every open cell of every floor can be walked to from its entry; floor ten holds the throne and the king.
- The loop, headless through `UndermerePlugin`: a run starts in Mereholt; the gate leads to floor one and back; stairs connect every floor; the king's death wins; the adventurer's death ends the run with a record whose score lines add up to its score.
- The screen: the rail, the log and the map where `Screen` cuts them, square tiles drawn two cells wide.
- The engine's title panel: the rows offered for each kind of slot, skipping, the abandon question, Continue inserting `ContinueRun`, Exit.
- Records: a record round-trips through its slot; a slot that cannot be read is replaced, not crashed on; the list keeps the best hundred in order; the ending screen shows the run's record.
- A fingerprint tripwire over a scripted run, labelled as such, as Foundry's.

## Documentation, paid once at the end of the branch

- `docs/guide/src/systems/records.md` and `title.md`, the reference for the new plugins, and the existing pages whose files moved.
- `docs/design/records.md`, listed in `AGENTS.md`'s layout and `docs/README.md`.
- `docs/OVERVIEW.md`: the new plugins in each crate's list and the `rl-ui` table, Foundry's title paragraph, and an Undermere section.
- `README.md`: Undermere under the example games, and the title and records in the feature list.
- `CHANGELOG.md`, under Unreleased: the hold, `ContinueRun`, `AbandonRun`, the panels, the records, and what Foundry dropped.
- The guide's last chapter names Undermere.

## Not in this part

Enchanting, identification, strength and everything else in part 2; monster tricks past one and the mini-bosses of part 3; biomes per run, prefab pools and a lived-in dungeon of part 4; shops, townsfolk and gold as money of part 5; mercenaries of part 6.
