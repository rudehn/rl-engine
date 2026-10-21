# Engine API review, 2026-10-16

A review of the public surface of every engine crate, `main` at `911a165`, for clarity, duplication and the review rules in `AGENTS.md`.
Five read-only reviewers split the crates: rl-bevy's gameplay half, rl-bevy's world, turn and AI half, rl-ui, rl-core with rl-grid and rl-rules, and rl-mapgen with rl-world, rl-render, rl-save, rl-overworld and the facade.
Each checked its findings against the code and against how the three games use the API, and left out what `docs/TODO.md` already holds.
About 115 findings came back; overlaps are merged below, and paths are relative to `crates/` unless they name `examples/`.

## Where this stands, 2026-10-20

This review was filed on a branch, at `bf49fa54`, that never reached `main`, and is brought over at `49c4549a` as the record `docs/TODO.md` cites.
Since it was written, `main` had a review of its own surface, `58edc0ed` and the commits beside it, which made 666 items crate-private, removed about 60 nothing used and removed the open world with `rl-world` and `rl-overworld`; and the functional-core refactor lifted the rules inside rl-bevy's systems into `rl-rules`, where they are tested without an `App`.
Section A's bugs are being fixed on the branches `fix-saves`, `fix-projectile`, `fix-rules-rng`, `fix-running` and `fix-ui-keys`, all but the eleventh, which stays open in `docs/TODO.md`; `fix-rules-rng`, bugs 5 and 8, reached `main` at `b19e8850`.
Every finding of sections B to F was checked against `main` that day; what is not named here still stands as written.

Stale, since what they name is gone:

- B, breadth-first steps: rl-world's `distance_field` went with its crate; the others stand.
- C, arriving in formation: the landing search is public as `rl_grid::landing`, and Undermere's mercenaries land through it in `land_beside` rather than through a copy.
- D, region: the world cell went with rl-world, and so did `WorldSettings` and chunk streaming, so `Region` is rl-grid's `Regions` and the mapgen districts, and `Stream` means only a random stream.
- E, the last item: rl-overworld, rl-world and `StreamingPlugin` are gone.
- E, the unused items: `InSight`, `Turns::made_progress`, `TurnHold::launch`, `Occupancy::remove_on`, `xy_idx`, `idx_xy` and `in_bounds_xy` are crate-private or private; `FellAsleep` and `Occupancy::insert_on` are still public.
- F, theme vocabulary: `WorldRules::settlements` went with rl-world.

Reshaped:

- B, how A stands to B: `rl_rules::faction::stance` replaced seven copies of the faction match, but rl-ui's narrator, inspect and nearby views and Undermere's artifacts, bodies and bosses still match by hand, and `Bystanders::relation` still hides in a param with an RNG.
- B, the diagonal cost: turn and bump share `rl_core::turn::step_cost`, and `NORMAL_MOVE_COST` is gone; rl-grid's A* keeps its own `* 1414 / 1000`, and `octile_milli` still counts thousandths.
- B, how an attack travels: worse, since `rl_rules::arms::Reach` now repeats rl-bevy's `combat::Reach` beside `Attempt`, `Delivery` and `Struck.ranged`.
- D, offer: `rl_rules::bump::Offered` is a sixth spelling beside ability's `Offered`.
- E, the public systems: most engine systems are crate-private now, but the roughly forty that Foundry's ambiguity test names stay public for it, so the finding stands for exactly those.
- E, rl-save's prelude: `EntityRemap`, `Versioned`, `encode` and `decode` left it; `EngineSave`, `SaveId` and `Stash` are still there.
- F, a plugin is opt-in: its first clause is bug 2, being fixed on `fix-running`; the rest stands.
- F, no rule numbers: `EQUIP_FROM_GROUND_COST` and `STANDING_ROOM` are crate-private now, which keeps them out of the surface but not out of the engine.

## Verdict

The shape is right.
The turn loop, `Resolution`, `Needs`/`Reads`, the phase sets, the view-collector-presenter split, `ToneId`, facets, `Loadout`, `Marksmanship` and the mapgen chain are each the one answer everything shares, and the games rarely work around them.
Tier 0 is tight: one `Point`, one `Rect`, one `Grid`, one `Direction`, one `DiceRoll`; no theme words, no `HashMap` in gameplay paths, no `Instant`, no generator seeded from a constant outside tests (save one, below).

The debt is repetition at the seams.
The same pattern is written three or four times, the copies have started to drift into bugs, and a large share of the `pub` surface is internal systems and params no game touches.
Tier 1 crates that cannot name each other have mirrored each other's types, and translate between them arm by arm in rl-bevy.

## A. Bugs

These are wrong today, not merely untidy, and come first.

1. **Every desktop game in one directory shares its settings and records.**
   `Saves::platform_default(name)` (`rl-save/src/backend.rs:203`) discards `name` off wasm and saves beside the executable, so Foundry, Undermere and the tutorial in `target/debug` all write `settings.save.ron` and `records.save.ron`, and settings are written back with only the writing game's keys.
   Fix: a directory per game name on desktop, and refuse a game slot named `settings` or `records`.
2. **Noise and stealth read as running without their plugins.**
   `NoiseRunning` and `StealthRunning` (`rl-bevy/src/noise.rs:233`, `stealth.rs:109`) take the plugin's message resource as proof it was added, but rl-ui's `VitalsViewPlugin` registers `NoiseHeard` and `NarrationPlugin` registers `Noticed`, so `wake_on_sight` reads an `Aware` nothing fills.
   Fix: each plugin inserts a marker, read through one generic `Running<P>`.
3. **A shot and a throw stop on different cells.**
   `combat::shot` stops at `p != to`; `throwing::flight` and `Bystanders::land` at `p != from`, so a shot at a wall lands in it where a throw stops short, against the module's promise that a knife and a spell agree.
   Fix: one `projectile(..) -> Footprint`, with `flight` and `line_of_fire` as wrappers.
4. **Carrying a tag means two things.**
   `doors.rs:84` counts the actor's own `Tagged` or its bag; `props.rs:510` counts the bag only, so a self-tagged actor opens a `Lock` and is refused an `Offering::needing`.
   Fix: one helper.
5. **The forecast and the turn loop disagree on speed 0.**
   `rl-rules/src/forecast.rs:226` treats it as never; `rl-core/src/turn.rs:38` as speed 1.
6. **`Known::learn` and `Known::clear` are undone a pass later** (`rl-bevy/src/ability.rs:73`), since `refresh_known` rebuilds `Known` from `Grants`.
   Fix: `pub(crate)`, and point readers at `Grants`.
7. **The save-slot name is joined into a path unchecked** (`rl-save/src/backend.rs:93`): `../x` or an absolute slot escapes the directory.
   Latent, since every slot is a code constant, but `SaveBackend` is a public seam.
8. **`BaseContext::new` seeds `StdRng::seed_from_u64(0)`** (`rl-mapgen/src/context.rs:128`), so a pass applied outside a `Chain` draws a constant stream.
9. **The inspect panel's health bar is always `Tones::GOOD`** (`rl-ui/src/panel/inspect.rs:257`), where the nearby and party rails use `health_tone`.
10. **Rebinding `CursorKeys.close` misses three screens**, since the sheet, scrollback and controls screens carry their own `close`.
11. **Saved kinds are keyed by short `type_name`** (`rl-save/src/run.rs:474`): renaming a component orphans old saves silently.

## B. One concept, several spellings

### Mirrored across tiers

- **Generation enums named twice.** `rl_rules::{pool::Placement, Stairs, Prefer, Joined, prefab::Turn, Mirror, DistrictDressings}` against rl-mapgen's `Placement`, `dungeon::Stairs`, `Prefer`, `accrete::Join`, `prefab::{Turn, Mirror}`, `dress::DistrictDressings`, translated by hand in `rl-bevy/src/layout.rs:64-223` and `prefabs.rs:167-213`.
  Both crates are tier 1, so one can name the other, or the plain enums move down to rl-grid; the conversions go.
  `layout.rs` and the `Prefabs` RON adapters are Bevy-free and belong in tier 1 as well.
- **Loot is a second banded table.** `rl_rules::loot::{LootRow, LootTable}` repeat `BandedEntry`/`BandedTable`: the same builder, weighted draw and `gaps`, bands stored differently.
  `LootTable` becomes a `BandedTable<K>` with tags per row.
- **An actor is a generic `A` in some types and a bare `u64` in others** (`rl-rules/src/status.rs:405-450`, `stats.rs:84`), so `Entity::try_from_bits` appears in five crates.
- **Two `Secret`s** (`rl-bevy/src/secrets.rs:41`, `rl-mapgen/src/secrets.rs:40`); `PlaceBuild` holds one and `PlaceMap` the other, and Undermere invents `JOINS_NO_AREA` to fill indices a place does not keep.
- **Tile glow is parsed in rl-render** (`rl-render/src/looks.rs:104`), a second read of the look file, so a headless tool cannot light tiles.

### Written three or four times

- **How A stands to B**: the `(Some(rules), Some(mine), Some(theirs))` faction match about 15 times across rl-bevy, rl-ui and four Undermere files, while `Bystanders::relation` hides inside a param holding an RNG.
  A `Factions` resource out of `CombatRules`, and a `Sides` param with `relation` and `hostile`.
- **Interners**: `Sounds`, `Verbs`, `WorkKinds`, `Moments` with identical `declare`/`get`/`name` and four `Add*` traits; `Interner::get(name)` reads backwards beside `Registry::get(id)`.
  One `Interned<K>` and `app.declare::<K>(name)`.
- **A place**: `PlaceBuild`, `PlaceMap`, `PlaceSave`, copied field by field.
- **Current map plus stash**: `Occupancy::switch`, `Knowledge::switch`, `WorldMap::switch_to`, which must be called together.
- **Moving an entity**: `relocate` plus `pos.0 =` plus `viewshed.dirty = true` in turn, bump, effects and two tutorial steps the guide quotes.
- **A list cursor**: nine `selected: usize`, six wrap-around `move_by`, six scroll windows in rl-ui, while `ListMenu` exists to own this and no engine panel uses it.
- **Key binding**: game keys go through `Controls`; each panel's keys are their own resource behind the closed `EngineKey` enum and a field per panel in `Bindings`.
- **Stack split and merge**: `trade::hand_over`, `items.rs:571`, `throwing.rs:190`, and a fourth in Undermere's `mercenaries/knowing.rs` that never merges.
- **Breadth-first steps**: rl-mapgen `walk.rs`, rl-world `distance_field`, mapgen `dress.rs:428`, and Undermere's `vaults.rs` because the mapgen one is private.
- **Opening queries**: `o.areas.0 == i || o.areas.1 == i` ten times, the area-graph walk three times; `Opening::joins`, `other`, `Join::shuts` and one `AreaGraph`.
- **Weighted picks**: `draw_weighted`, `StampOneOf`, `dress.rs:233`.
- **Chebyshev reach**: `is_adjacent`, `awareness::within_reach`, `work::in_reach`, `Thinking::within_reach`, an open-coded `SLEEPER_REACH`; `Perception`'s doc says a disc where the check is a square.
- **The diagonal cost** `* 1414 / 1000` in rl-grid, turn and bump, with `octile_milli` in thousandths against the hundredths rule; `NORMAL_MOVE_COST` repeats `BASE_ACTION_COST`.
- **Lingering then despawned in `Last`**: `Dead`, `Spent`, `Remnant`, each with its own systems; `Recharge` and `Pulse` are the same clock.

### Parallel types in one crate

- **How an attack travels**: `Reach`, `Attempt`, `rl_rules::Delivery`, `Struck.ranged: bool`.
- **Range**: `RangedAttack` and `Throwable` each carry `range`, `effective` and two accessors defaulting to a third.
- **A roll of a kind**: a bare `(DamageKindId, DiceRoll)`, reversed as `(DiceRoll, NameRef)` in `parts::Throw`.
- **A stat change**: `StatusModifier { stat, op }` against `(StatId, Op)` tuples in `Bestows` and `WhileHolding`.
- **Targeting**: `AbilityDef` keeps `mode`, `aim`, `sight` loose and rebuilds a `Targeting`; `Usable` spreads two of them.
- **Steps**: `Steps::{Four, Eight}` and `PathRules.diagonals`, each with `directions()`; `PathRules::EIGHT_WAY == default()`.
- **Sight range**: `Perception` overwrites `Viewshed.range` every cast.
- **Dark sight**: five components folding one number, a hand copy of the gear stat fold.
- **Benching an attack**: an empty `Consumable` filtered in `Loadout`, a dry gun moved to `Stowed`, a game's heat also in `Stowed`, told apart by markers.
- **`Buy` and `Sell`** are the same struct; `Traded` already carries a `Deal`.
- **`ItemRow` redeclares `Row`'s fields** rather than embedding it as `TradeRow` does.
- **`AbilityRow`** holds `blocked` and `why` for one fact, the second in English; turns are phrased four ways across panels.
- **Yes or no** is modelled three times in rl-ui.
- **`RecordOutcome`** re-declares `state::Outcome` with a hand match.
- **`Ending`** duplicates `RunOver`'s fields.

## C. Loops the games own

- **Continuing a run.** `ContinueRun` is a flag, so `foundry/src/run.rs:134-200` and `undermere/src/run.rs:398-470` are the same code: load, fall back to a fresh start, reseed, prepare, restore, clear on failure.
  The engine owns continue-or-fresh; the game supplies prepare, a fresh start and a post-restore hook.
- **Generation queries kept private.** Undermere's `dungeon.rs:846-895` rebuilds `walk_through_doors`, a cut check `Dress` already applies, and `ways`.
- **Arriving in formation.** `mercenaries/travel.rs:118` copies the private `landing()`; an `Arrive::Beside(point, offset)` would do.
- **`Spot.tag` is a `u32` holding a char**, unpacked by the same `zip(char::from_u32(..))` eight times across both games; `Spot::glyph()` and `Prefabs::slot_of(&Spot)`.
- **`Afflict` has no constructor**: 37 literals in 26 files ending `by: None, held_by: None`.
- **`Stack { key: id.index() as u64, .. }`** is built by hand in both games; `Stack::of(id, count)`.
- **`PlaceRulesRes(Box::new(x))`** in every game and every tutorial step; `Places::new(impl PlaceRules)`.

## D. Names

- **`Struck` fires on misses**, and is easily read beside the `STRUCK` moment, which means the opposite; `Attacked`, carrying a `Delivery`.
- **Landings**: `Aimed`, `Landed`, `Landing`, `LandedOn`, `UseLanding`, `ShotLanding` (also for melee), `ThrowLanding`.
- **`on`** is a moment in `Trigger`, the carrier in `Fired`, the thing used on in `FiredOn` and `Landing`; `moment`, `carrier`, `used_on`.
- **Hold** is the turn loop waiting, a status taking turns, a status kept by worn gear and stat changes under a status; the worn lenders are `HeldWhileWorn`, `LitWhenWorn`, `WornDarkSight`, `Bestows`.
- **Offer** is props' `Offer`, `Offering`, `Offers`, `OfferedHere` and ability's `Offered`; `Offer` repeats four fields of `Offering`.
- **Use** is `Use` (an ability) beside `UseItem`, `UseItemAt`, `UseOn`.
- **Clashes in rl-rules**: two `Placed`, two `Pick`, `loot::Layout` beside the `layout` module, `accuracy::Line` beside `geometry::Line`; `Percent` is a hit model.
- **Region** is a connected component in rl-grid, a world cell in rl-world, a district's taste in rl-mapgen; `WorldMap` is the map one is on in a game with no world; `WorldSettings` holds only a streaming radius.
- **Stream** is an RNG stream and chunk streaming; `RunSeed::stream` has no caller and means something else than `Seed::stream`.
- **Starts and exits**: eight builder types across two modules, `StartPoint` and `ExitPoint` in different ones, and `RandomStart::beyond()` returning a `ClearStart`.
- **Pass names**: some passes take a `name`, others hard-code one, and two hard-coded ones in a chain panic.
- **rl-ui builders**: `title()` against `titled()`, `called()` setting a noun, words as a struct on some panels and setters on others; `MenuItem::offered` takes three bools; `TitleRow` and `MenuItem` name the same rows differently.
- **Modal ids**: 14 constants no game uses, 14 accessors of which two return `Option` and the rest panic, split between `view::` and `panel::`.
- **System sets**: `ResolveSet::Effects` holds the ticks while effects run in `Triggers`; `EndOfFrame` lacks the `Set` suffix.
- **The app extensions**: `depends_on` is a free function where `needs` and `reads` are methods; plugin names are strings; `add_work` declares a kind; `add_setting` returns an id.
- **The run's lifecycle**: messages, an observer event and resource flags for one life; `AbandonRun` means forgetting the save, not `Outcome::Abandoned`.
- **Smaller**: `Trading::open()` is a getter; `TileField::set_cells()` is a getter; `rl_core::stats` holds float numerics beside `rl_rules::stats`; the `idx`/`index` mix; `position_hash(seed, x, y)` beside `Point`; `footprint`'s two same-typed predicates passed the wrong way round once; `may_use(.., bool, bool)`; `headings: [String; 5]`; `Turns::export` returns a bare tuple; `events.rs` holds facts; `MapId` derives `Component` and so inserts silently in place of `OnMap`.

## E. Surface wider than the API

- About 45 `pub` systems and 20 `pub` `SystemParam`s in rl-ui, and most systems in rl-bevy, are public so the games' ambiguity tests can name them: Foundry's names 45 engine system functions to allow a pair or label a failure.
  A game's test is thereby written against the engine's private functions, breaks when one is renamed or split, and a public system invites the ordering the rules forbid.
  The ambiguity check is a property of the schedule, so it is tested through the schedule's interface: the engine checks its own systems against each other in its own tests, where private access is ordinary; a game's test judges only pairs with one of its own systems in them, and allows a pair by the game's system and the data it shares (`Allowed::with_any`), never by an engine function; and the table of names goes, since a failure reads by the game's system and the data.
  Then the engine's systems are `pub(crate)`, and what stays public is plugins, the sets a game orders against, messages, components and resources.
- The roots and preludes of rl-rules, rl-grid, rl-mapgen, rl-ui and rl-render are two hand-kept lists that have drifted; Undermere writes `rl_ui::view::inspect::inspect_modal` eight times.
  One rule: the root re-exports every type a game names, the prelude is a subset.
- rl-save's prelude carries `EntityRemap`, `SaveId`, `Versioned`, `EngineSave`, `Stash`, `encode`, `decode`, none used outside the crate.
- `InSight`, `FellAsleep`, `Turns::made_progress`, `TurnHold::launch`/`land`, `Occupancy::insert_on`/`remove_on`, `xy_idx`, `idx_xy`, `in_bounds_xy` and a handful of rl-rules helpers have no outside user.
- rl-overworld (346 lines) has no game user, draws and queries in one system, and takes `Color`; it folds into rl-ui as an opt-in panel or waits until a game wants it.
  rl-world and `StreamingPlugin` likewise have no game consumer.

## F. Rules

- **A plugin is opt-in**: bug 2 above; `CorePlugin` also builds the minds' `FlowFields`, `Thinking`, `Knowledge`, two intents, statuses and work; effect registration is inconsistent, since `add_engine_effects` registers nothing by default while Fire, Gas and Noise silently register theirs.
- **Never order after another crate's function**: rl-ui orders against `rl_render::particles::skip_on_key`.
- **No rule numbers in the engine**: `EQUIP_FROM_GROUND_COST`, a 50% reflect chance, effective range a third of range, `STANDING_ROOM`, `SLEEPER_REACH`, the `NoticeStats` and `HearingStats` defaults.
- **A closed list every subsystem edits**: `WasLiving` in `remains.rs:424`, as `RemnantKeeps` already is a registry.
- **No `Color` in a widget**: `print_rich(fg: Color, bg: Color)` and `Segment = (String, Option<Color>)`.
  `Span.color` itself is the named thing's own glyph colour, as `Row.glyph` is, and stands.
- **English the game did not supply**: `MenuItem`'s labels, inspect and sheet lines, `AbilityRow::why`.
- **`BTreeMap` over `HashMap`**: `rl-save/src/remap.rs:22`, deterministic in use, against the letter of the rule.
- **Theme vocabulary**: `WorldRules::settlements`.

## Order of work

1. The bugs in section A, each reproduced in a test first; fixed on a branch of its own each, all but the eleventh, which is `docs/TODO.md` item 31.
2. The tier-1 mirrors: one enum per concept, loot on banded tables, one actor handle, one `Secret`.
3. The patterns written four times: factions, interners, the list cursor, key binding, moving, the place struct, the per-map stash, the projectile.
4. The loops in section C, starting with continue-or-fresh.
5. The surface: systems to `pub(crate)`, one root-and-prelude rule per crate.
6. The renames in section D, batched by crate so each game migrates once, with `CHANGELOG.md` lines.
