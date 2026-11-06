# Changelog

All notable changes to atglyph are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
atglyph is not on crates.io yet, so a game depends on a release by its git tag, as the README shows.

## [Unreleased]

### Added

- `RoguelikePlugins::fit_screen` and `FitScreenPlugin`: the window opens as large as the screen allows rather than at the cells' declared size, keeping the grid's shape, centred in what the system leaves to windows (on macOS the screen less its menu bar and dock), and is sized before it is shown; a window that opens fullscreen, a capture and the web are left as they were; under reactive updates it asks for each frame it needs, so a hidden window never waits on input, and a window made on a monitor of another scale than the primary's is placed in its own scale.
- `Fields::descents_away_within` and `TacticCtx::step_away_within`: a field away from goals flooded only over the square a radius round the mind, any goal beyond it standing on its edge, for a flight; the whole field by default.
- `Grounded(inner)` holds any tactic to the mind's ground, as `Idle` and `WalkRounds` keep to it: a step it would take from the ground off it becomes a wait, so a watchman whose hunt or search is wrapped holds the line facing a foe past its edge rather than chasing it or pacing in and out; a blow over the line goes through, a mind with no ground is unaffected, and one already off its ground is let back on.
- `BumpRules::offers_first`: a bump into a neutral of a giving-way side that offers something is dealt with (taken up as an `Interact`, or a `Bumped` with `offers` when it offers several) before it gives way, so walking into a store keeper opens the trade; one offering nothing is still swapped with.
- `dress::dress_room` dresses one room's cells on a terrain with no chain, by the rules `Dress` keeps, for a game that lays its own buildings.
- Dressing rules for furnishing houses: a rule lays an exact `count` (`count: 1` or `count: (fewest, most)`) in place of `density_pct`, keeps `from_door` `Near` or `Far` by the path in from the nearest opening, or `Behind('c')` another thing by the path round the things laid as `'c'`, so a shop whose counter ends in the far corner has shelves along the whole far wall, and the spreads `Beside('c')`, `Across(gap: n)` for a line across the room with a gap, as a shop's counter, and `OutsideDoor` for a cell flanking the way out, never cutting the ground outside, read five cells round the area so a barrel by a door with a post two cells out stands where the street goes round it, and never joined to the street only through the room and out by another door.
- `Keep` can keep away from the others: `Station::Others` is the actors neither allied nor hostile, and `Keep::away(station, distance)` backs off inside `distance` and never closes in, named `keep_away`; a keep on the others steps by near fields, as `GiveWay` does.
- `Keep::watching`: a keep that waits in its band while the nearest of its station is in sight, so a wary animal that has backed off far enough stands and watches rather than wandering back in.
- A mind keeps to its ground: an actor's `Ground` id, read through the game's `GroundRule`, holds its idle steps to the cells the game names, and one off its ground walks back. Herds: actors sharing a `Herd` id keep together with no leader, by `Keep::herd` on the middle of their living mates.
- Carving a place out of solid rock a cell at a time: `DrunkardsWalk` opens a broad hall with rock standing in it, and `Aggregate`, diffusion-limited aggregation, grows branching tunnels; both open the share of the map asked for in one piece, from an `Origin`, with a brush width, a margin and, for aggregation, a pull toward the middle.
- `CellBrightness`: a game draws each cell of a map brighter or darker than its tile, as high ground brighter than low, in sight and in memory alike.
- Near fields: `Fields::descents_toward_near` and `Fields::descents_away_near` flood only the box round a mind and its goals, a margin wider, for a step among those a few cells off; the engine floods them afresh and keeps them out of the shared fields, and a `Fields` of a game's own answers them from its whole fields unless it says otherwise.
- `RangeRules::check` names every divisor under one and multiplier under nought, and `StealthPlugin::by_range` refuses rules that fail it, naming each; a multiplier too large for a range saturates.
- Cover: a tile may be `cover: true` in a tiles file, `TileProps::cover` and `TileTables::cover`, for a rule that makes an actor standing in it harder to notice.
- Stealth by range: `StealthPlugin::by_range(RangeRules)` judges noticing by the subject's own `StealthRange`, within which anything not allied may notice it by the game's chance a turn and beyond which nothing does, an alert observer holding it in view within `hold` times the range; the plugin gives every actor a range at the base and every mind an `Aware` as each appears, and a neutral is hidden from a mind's others until noticed. `Hides` marks whatever can hide under either rule. Each `StealthRange` is worked out every turn from the actor's `LastDeed` and where it stands: waiting divides it, attacking multiplies it, the dark band and cover each divide it.
- `namegen::reads_well` holds a name put together outside a set to a set's rejections.
- `CombatRules::spares(by, spared)` and `spares_side`: a side harms nobody of another, whatever the two are to each other; an attack by one of `by` on one of `spared` is refused before it is made, as a bump into a neutral is, a throw, a launch, a shot or a bolt of theirs flies past one as if nobody stood there, no area of theirs lands on one but what is aimed at allies, and a mind of `by` never makes one its target. `Bystanders::spares` answers it for two actors.
- Rounds: an actor's `Rounds { route, next }` walks to each point of its route in turn and from the last back to the first, a route of one point a post it keeps, by the `WalkRounds` tactic named `walk_rounds` through the `OnRounds` sense; its steps keep to its `Ground`, a point somebody else stands on is passed once beside it, a brain that hunts above it breaks off and comes back to the point it was walking to, and a save keeps it with the actor.
- `Streamed::slack`: a streamed place's window stays where it is while the player stands within that many tiles of the chunk it was made round, and moves only once the player is that far into another, so pacing along a chunk's edge loads and lets go of nothing; `Streamed::window_of` and `Streamed::centre_following` answer the window and the chunk it follows from.
- `GearView::lines`: a game pushes `GearLine`s in `ViewSet::Annotate` and the gear panel draws them under the slots, in the slots' column, as something kept ready that no slot holds: a second weapon a turn away, or what a throw will take.
- The bag and the trade screen weigh gear before it goes on: under the row picked out, a thing that could be worn says what it would replace in the slot it goes in, or that nothing is worn there, the hand a two-hander takes from what holds it and the hand a one-hander frees, a stuck thing in the way, and each number that changes, old to new and by how much (`armor 1 -> 2 (+1)`, `blow dagger 1d4 -> own 1d6 (+1 avg)`, `arrow roll +1d4 -> none (-2.5 avg)`), a gain in the good tone and a loss in the bad, and `no change` for a thing put on in place of its like. What adds up (armor, extra rolls, resistances, stats) is weighed thing against thing; the blow, the shot, each launcher and sight in the dark are weighed by what the wearer would have before and after, as a fight decides them, the wearer's own blow named by `InventoryPanel::own` and `TradeWords::own`. The trade screen draws the same under its two lists, after a blank row, for a ware or a thing of the pack, against what the customer wears, in a pane as tall as the longest description on the screen. `Equipment::swap` works out what putting a thing on would change without putting it on, by the rule `equip_keeping` equips by, as a `Swap`; the view's `Replacing`, `TakenOff`, `Kit`, `Wielding`, `LaunchLine` and `Bestowed` carry it; the bag also describes what a launcher adds and the sight in the dark a thing lends.
- `StatDef::lower_is_better`: a stat less of which is better, so a screen weighing gear tones its rise as a loss.
- `TradeView::described_mut`: a game annotates what the trade screen's pane describes in `ViewSet::Annotate`, as it annotates the bag's rows.
- The trade screen lists things alike and not stacked as one counted row, `3 daggers`, priced for one: confirm deals one of them and the whole-row key each of them; a thing with `MassNoun` is counted as written, `2 leather leggings`. Things are counted in the new `ViewSet::Settle`, after the game annotates, so a thing it notes (the other weapon among daggers) stays a row of its own; a counted row's price always says `each`, giving up the refusal and then the end of its name for room, never the word.
- A blow can be versatile: `Melee::versatile` rolls a bigger die while a named slot is free, in the fight, the pack and the inspect panel alike.
- `ItemParts::check` refuses a versatile blow on a thing that takes the slot it needs free, a two-hander or a thing held only in that slot, since it could never strike its bigger roll.
- `AddsOwnBlow`: an actor whose worn weapon's blow adds to its own, as a monster's does in DCSS, the player untouched.
  `Loadout::melee` puts its own roll in the blow's new `MeleeAttack::also` (a versatile weapon's bigger roll and its own together), the fight rolls the two as one hit so armor meets them once, a shot and a throw carry none of it, the forecast counts it, the sheet and the pack say it as `1d4+1d2`, and a mind weighing gear for one gives up none of its `OwnBlowScore` by putting a blade on; `arms::own_blow_added` is the one rule all of them read.
- `GearPanel::held_as`, and a versatile item's free slot shown as held: a slot another slot's item holds, a two-hander's claimed hand or the free hand a versatile blade is swung in, reads the game's label in the muted tone, `╰ both hands`, in place of the item a second time; `GearView` fills a versatile blade's free slot with its row, not primary, as it fills a two-hander's, the slot still free to fill.

### Changed

- `Names::load` names an entry that does not parse by its position and the `name:` it writes, `entry 3 (rapier): ...`, rather than its position alone, so a misspelt variant deep in a long file says which definition it is in; an entry with no readable name is still named by position.
- **Breaking:** A launcher's range, effective range and roll are the shot's, in place of the launched thing's own; `LaunchDef` takes `effective`, `range` and `kind`, and `ThrowAs::launched` names only the launcher.
- **Breaking:** `Melee` has a public `versatile` field, a `VersatileDef`, and `MeleeAttack` a public `versatile` field, an `arms::Versatile`; a game that writes either as a struct literal adds `versatile: None`.
- **Breaking:** `MeleeAttack` and the view's `Strike` have a public `also` field, the wielder's own roll landing in a worn blow's hit, `forecast::Combatant` an `also` and `forecast::Arms` a `melee_also` for the same; a game that writes one as a struct literal adds `also: None` or `melee_also: None`.
- **Breaking:** `Strike::also` is no longer a launcher's roll beside a launched thing's, since that strike is the launcher's alone, and `LaunchLine` holds the launcher's shot, its `kind` and an `i32` `range`; the bag says a launcher as `launches arrow 1d4 pierce to 10` and weighs one against another as roll and range, with no `+`.
- **Breaking:** `ItemRow` has public `launches`, `dark_sight` and `replacing` fields and its `bestows` is a `Vec<Bestowed>` rather than `(name, op)` pairs; `TradeRow` has public `described` and `items` fields; `TradeWords` a public `own` field; `StatDef` a public `lower_is_better` field. A game that writes one as a struct literal adds `launches: None, dark_sight: None, replacing: None`, `described: None, items: vec![entity]`, `own: "own".into()` or `lower_is_better: false`, and one that reads `bestows` reads `stat` and `op` off each.
- `WorldMap::view` and `opening_view` find the window and the current place once, when made, rather than for every cell a shadowcast or a flood reads, which was most of what reading a cell cost.
- **Breaking:** `FleeWhenHurt` has a public `within` field, how far round itself a mind looks for its way away, and runs down a field over that square alone rather than the whole loaded window, so a flight costs the same however much is loaded; `FleeWhenHurt::at(pct)` gives `FleeWhenHurt::WITHIN`, 20 cells, and a game that writes the struct literal adds `within: FleeWhenHurt::WITHIN`.
- **Breaking:** `Streamed` has a public `slack` field; a game that writes one as a struct literal adds `slack: 0` to keep its window moving at every chunk's edge, and a save written before reads it as 0: a run restored from such a save keeps the slack of 0 its saved layout carries, safe with any sight a game's reach allows, its window only moving at every edge as before.
- A streamed place keeps the chunks that left its window last whole, twice a window's worth, so walking back over ground just left builds nothing; `PlaceRules::build_chunk` is asked again only for a chunk left longer ago, and its edits are laid back on it as before.
- A streamed place's window moving no longer moves the map's opacity and cost epochs: a viewshed whose sight lies in what the window holds before and after is kept as it was cast, the lighting marks only the viewers whose sight a move lit or darkened, and the rest are cast again as before, so a move no longer recasts every viewer in the window.
- A mind's flow field toward its goals outlives a move of a streamed place's window wherever it provably leads as one flooded over the new window would, its cost from the mind's cell and those round it below the least any way through the ground that moved could cost, and is flooded again elsewhere; a field leading away is flooded again at once. Nothing a mind decides changes because the window moved under it.
- **Breaking:** `CombatRules` has a public `spares` field, `BumpRules` a public `offers_first` field and `atglyph_save::EntityState` a public `rounds` field; a game that writes one of them as a struct literal adds `spares: Vec::new()`, `offers_first: false` or `rounds: None` to keep it as it was, and a save written before reads `rounds` as `None`.
- **Breaking:** `shot`, `line_of_fire` and `flight` take `past`, whoever the flight goes by, the shooter's spared as `Bystanders::spares` answers; a game that calls one passes `|_| false` to keep anyone in the way stopping it.
- **Breaking:** `dress::Thing` has `count` and `from_door` fields, and `dress::Spread` has the variants `Beside`, `Across` and `OutsideDoor`; a game that writes a `Thing` literal adds `count: None, from_door: None` to lay as before, and one that matches a `Spread` names the new variants.
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
- The look cursor forecasts no fight, no duel and no odds, with one of a side the player's `CombatRules::spares`, whose blows never land on them, as with an ally.
- The aim's Tab passes over one the user spares when aiming harm, as the cursor's opening already did, so cycling a shot or a throw goes from foe to foe.
- The trade screen lists the rows of one name together, where the first of them stands, and otherwise in the bag's order, so a thing sold to a trader goes in beside the trader's own of its name rather than to the foot of the list.

### Fixed

- A mind that opens doors no longer panics reading a flow field kept through a move of a streamed place's window, and a fleer whose foe is beyond its reach flees from the nearest open cell of its reach's edge toward it rather than from one that may be a wall or under a prop.
- An ambient a game writes in the middle of a streamed place stands while the window moves round the player, as it does on a place built whole, rather than going back to the place's own at every move.
- A glowing tile on a chunk of a streamed place that left the window and came back is lit once, not once more for every return.
- The terminal's cells reach the screen before Bevy lays the new text out, works out what is shown and moves what moved in `PostUpdate`, so a step no longer leaves last frame's glyphs standing in the new colour for a frame, white where a cell turned blank, nor a picture where it was.
- A herd member standing on its herd's middle steps off it, as its `no_closer_than` says, rather than asking for a field and then doing nothing.
- A trade screen row's price says "each" only where the word fits beside the whole of the row's name, so a long name, `5 loaves of bread` at a purse that cannot pay, is never cut short for it.

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
