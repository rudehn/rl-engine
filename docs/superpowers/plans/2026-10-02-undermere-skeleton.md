# Undermere Skeleton Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Undermere plays from a title menu through a town, ten floors and the Drowned King to an end screen with an itemized score, and every run is kept in records between launches.

**Architecture:** Two engine slices first, each split as settings already are (plain data in `rl-bevy`, the slot in `rl-save`, the screen in `rl-ui`): the title menu lifted out of Foundry, and run records.
Then `examples/undermere`, a library with a thin binary in Foundry's shape, its content written as monsters of a core plus named traits.

**Tech Stack:** Rust, Bevy 0.19, `serde`, `ron`, the workspace's own crates.

**Spec:** `docs/superpowers/specs/2026-10-02-undermere-skeleton-design.md`, with the game design in `examples/undermere/DESIGN.md`.

This plan names files, interfaces and the tests to write, and leaves the code to the implementer, who reads the reference files each task points at.
`examples/foundry` is the reference game for everything in the Undermere tasks: when a task says "as Foundry's", open that file and follow its pattern, its doc density and its test style.

## Global Constraints

- Engine tasks 1 to 5 work in `.claude/worktrees/undermere` on branch `undermere`; game tasks 6 to 8 may run in a second worktree, `.claude/worktrees/undermere-game` on branch `undermere-game` branched from `undermere`, and are merged back before task 9. Never `cd` to the main checkout.
- `CARGO_TARGET_DIR` stays unset. Long output goes to a file in the scratchpad and is read by its tail.
- Scoped runs while iterating: `cargo test -p <crate> <filter>`. The whole workspace once, in task 11.
- Each commit runs the fast gate first: `cargo fmt --all --check`, `cargo clippy -p <crates touched> --all-targets -- -D warnings`, and the tests of what it touched.
- Each commit adds its line to `CHANGELOG.md` under `Unreleased` when a game author would see the change.
- Commit messages are one lowercase sentence in the repo's style, with no co-author line.
- `#![deny(missing_docs)]`: every public item gets a doc comment that says why, at the density of `crates/rl-core/src/turn.rs`.
- No `TODO` comments, no em dash anywhere, no `HashMap` or `HashSet` in gameplay or generation, no `#[non_exhaustive]`, no theme words in engine crates.
- `rustfmt.toml` pins the width; do not hand-wrap.
- A test's name reads as a sentence describing the property; property tests run over a span of seeds.
- In Markdown, one sentence per line.
- Never order a system after another crate's system function; order against sets.
- `rl-ui` must not gain a dependency on `rl-save`, nor `rl-save` on `rl-ui`.
- Randomness only through `Seed`, `RunSeed::derive` and engine streams; never a generator from a constant or from entropy in engine code.
- Every content RON file starts with a comment listing every field it may hold.

## Review Focus

1. The key that takes a title row is read again by the screen it opens in the same frame (Enter on Settings also changes the first setting, Enter on New Game also answers the question): nothing beyond the one action happens. Pinned in task 3.
2. A save slot that cannot be read: Continue is not offered, New Game still asks before clearing it, and nothing panics. Pinned in tasks 1 and 3.
3. "Main menu" from the pause menu, then Continue: the run comes back where it was left, not a turn earlier. Pinned in task 3.
4. A records slot that is not RON, or comes from a build with more fields: nothing panics; unknown fields are ignored; an unreadable list is replaced by the next record. Pinned in task 2.
5. A monster whose two traits both set one value (two `speed`s): the game refuses to start and names both traits and the field. Pinned in task 6.

---

## File structure

| File | Responsibility |
| --- | --- |
| `crates/rl-bevy/src/title.rs` (new) | `HoldStart`, `begin_run`, `SavedRun`, `ContinueRun`, `AbandonRun`, `LeaveRun`: the run's start as something a screen can hold. |
| `crates/rl-bevy/src/plugin.rs` (modify) | `begin_first_run` respects the hold; `restart_runs` answers `LeaveRun`. |
| `crates/rl-bevy/src/records.rs` (new) | `RunRecord`, `RecordOutcome`, `RecordSection`, `RunRecords`, `RecordRun`, `RecordsPlugin`. Plain data and the one system that files a record. |
| `crates/rl-save/src/run.rs` (modify) | `SavePlugin` keeps `SavedRun`, observes `AbandonRun`, writes on `LeaveRun`. |
| `crates/rl-save/src/records.rs` (new) | `RecordsSavePlugin`: the `records` slot. |
| `crates/rl-ui/src/panel/title.rs` (new) | `TitlePanel`, `TitleMenu`, `TitleWords`, `TitleSet`, `title_is_up`. |
| `crates/rl-ui/src/panel/records.rs` (new) | `RecordsPanel`, `RecordsScreen`, `records_modal`. |
| `crates/rl-ui/src/game_menu.rs` (modify) | The "Main menu" row. |
| `crates/rl-ui/src/view/ending.rs` (modify) | The ending shows this run's record. |
| `crates/rl-engine/src/lib.rs` (modify) | The prelude exports what a game names. |
| `examples/foundry/src/{title,run,save,plugin,main}.rs` (modify) | Foundry onto the engine's title. |
| `examples/undermere/**` (new) | The game. |
| `Cargo.toml` (modify) | `examples/undermere` in the workspace members. |

---

### Task 1: The hold on the first run, and what the save slot holds

**Files:**
- Create: `crates/rl-bevy/src/title.rs`
- Modify: `crates/rl-bevy/src/lib.rs`, `crates/rl-bevy/src/plugin.rs` (`begin_first_run`, `restart_runs`), `crates/rl-save/src/run.rs` (`SavePlugin`), `crates/rl-engine/src/lib.rs` (prelude), `CHANGELOG.md`
- Reference: `examples/foundry/src/title.rs` (`look_for_save`, `begin`), `examples/foundry/src/save.rs` (`abandon`), `examples/foundry/src/run.rs` (`Resume`)

**Interfaces:**
- Produces, in `rl_bevy::title` and re-exported at the crate root and in the prelude:
  - `#[derive(Resource, Default)] pub struct HoldStart;` While present, `begin_first_run` runs nothing.
  - `pub fn begin_run(world: &mut World)`: runs `NewRun`; the one door a title uses.
  - `#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)] pub enum SavedRun { #[default] Empty, Readable, Unreadable }`
  - `#[derive(Resource, Default)] pub struct ContinueRun;` A game's `NewRun` systems restore the saved run while it is present; whoever restores removes it.
  - `#[derive(Event, Debug, Clone, Copy)] pub struct AbandonRun;` Triggered on the world; whoever keeps the slot forgets it.
  - `#[derive(Message, Debug, Clone, Copy)] pub struct LeaveRun;` Answered in `EndOfFrame::Restart` by `restart_runs`: `EndRun`, `clear_run`, `HoldStart` inserted, state to `Idle`, no new run begun.
- `CorePlugin` registers `LeaveRun` and initialises `SavedRun`.
- `SavePlugin` (rl-save): in `finish` or a `PreStartup` system, reads its slot as Foundry's `look_for_save` does and sets `SavedRun`; sets `Readable` after each write and `Empty` after each delete; `app.add_observer` on `AbandonRun` clears the `Stash`, deletes the slot and sets `Empty`; in `EndOfFrame::Save`, when a `LeaveRun` was written this frame and the state is `Playing`, captures and writes the run (`app.reads::<LeaveRun>()`).

- [ ] **Step 1: Write the failing tests** in `crates/rl-bevy/src/title.rs` and `crates/rl-save/src/run.rs`:
  - `with_the_start_held_startup_begins_no_run`: an app with `CorePlugin`, a counting system in `NewRun`, and `HoldStart` inserted before the first update; after two updates the count is 0; `begin_run` makes it 1.
  - `without_a_hold_startup_begins_the_run_as_before`: the same app without `HoldStart` counts 1.
  - `leaving_a_run_tears_it_down_and_holds_the_start`: a playing test world (`rl_bevy::testing::surface`), write `LeaveRun`, update; `EngineState::Idle`, no `WorldMap`, `HoldStart` present, and the `NewRun` counter unchanged.
  - `an_empty_slot_reads_empty`, `a_saved_run_reads_readable`, `a_slot_of_another_version_reads_unreadable` (memory backend; for the last, persist text with a wrong version).
  - `abandoning_deletes_the_slot_and_clears_the_stash`: persist a run, trigger `AbandonRun`; the slot is gone, the stash empty, `SavedRun::Empty`.
  - `leaving_a_run_writes_it_down_first`: a playing run with `SavePlugin`, take a turn, write `LeaveRun`, update; the slot holds a run whose turn is the run's current turn, and `SavedRun::Readable`.
- [ ] **Step 2: Run them and see them fail**: `cargo test -p rl-bevy title` and `cargo test -p rl-save run`.
- [ ] **Step 3: Implement** as the interfaces say. `begin_first_run` checks `HoldStart` before `run_schedule(NewRun)`.
- [ ] **Step 4: Run the tests and see them pass**, then `cargo test -p rl-bevy -p rl-save` whole.
- [ ] **Step 5: Commit** with a `CHANGELOG.md` line: "the start of a run can be held by a title screen, and the save slot says what it holds".

### Task 2: Run records: the record, the list and the slot

**Files:**
- Create: `crates/rl-bevy/src/records.rs`, `crates/rl-save/src/records.rs`
- Modify: `crates/rl-bevy/src/lib.rs`, `crates/rl-save/src/lib.rs`, `crates/rl-engine/src/lib.rs`, `CHANGELOG.md`
- Reference: `crates/rl-save/src/settings.rs` (the slot pattern, end to end), `crates/rl-bevy/src/state.rs` (`Ending`, `Outcome`)

**Interfaces:**
- Produces, in `rl_bevy::records` and the prelude:
  - `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)] #[serde(default)] pub struct RunRecord { pub outcome: RecordOutcome, pub summary: String, pub score: i64, pub seed: u64, pub turn: u64, pub when: Option<u64>, pub sections: Vec<RecordSection> }` with `Default`.
  - `pub enum RecordOutcome { Won, #[default] Died, Abandoned }` (serde).
  - `pub struct RecordSection { pub heading: String, pub body: String }` (serde).
  - `#[derive(Resource, Default)] pub struct RunRecords { pub records: Vec<RunRecord>, pub this_run: Option<RunRecord> }` with `pub const MAX_RECORDS: usize = 100;` and `RunRecords::file(&mut self, record: RunRecord)` keeping best score first (ties: the newer first), capped.
  - `#[derive(Message, Debug, Clone)] pub struct RecordRun { pub score: i64, pub summary: String, pub sections: Vec<RecordSection> }`
  - `pub struct RecordsPlugin;` registers the message and the resource, and `record_runs`, which answers each `RecordRun` once an `Ending` exists: outcome from `Ending`'s `Outcome`, seed from `Seed`, turn from the clock the ending carries, `when` from `SystemTime` off wasm and `None` on it; files it; sets `this_run`. `this_run` is reset on a new run (`reset_on_new_run` or `EndRun`).
- Produces, in `rl_save`: `pub const RECORDS_SLOT: &str = "records";` and `pub struct RecordsSavePlugin;`, reading the slot into `RunRecords` before the first frame and writing it whenever the list changed, as `SettingsSavePlugin` does.

- [ ] **Step 1: Write the failing tests**:
  - rl-bevy: `a_record_is_filed_best_score_first`, `a_tie_files_the_newer_record_first`, `the_list_keeps_the_best_hundred`, `a_record_run_written_before_the_ending_waits_for_it`, `a_record_takes_its_outcome_seed_and_turn_from_the_ending`, `this_run_is_forgotten_when_a_new_run_begins`.
  - rl-save: `records_round_trip_through_their_slot`, `a_record_from_a_build_with_more_fields_still_reads` (persist RON with an extra field), `a_slot_that_is_not_ron_is_warned_about_and_replaced_by_the_next_record` (Review Focus 4), `records_are_never_deleted_with_the_run`.
- [ ] **Step 2: Run them and see them fail**: `cargo test -p rl-bevy records`, `cargo test -p rl-save records`.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them and see them pass**, then both crates whole.
- [ ] **Step 5: Commit**: "a run ends in a record, filed best first and kept between launches in a slot of its own".

### Task 3: The title panel, and the way back to it

**Files:**
- Create: `crates/rl-ui/src/panel/title.rs`
- Modify: `crates/rl-ui/src/panel/mod.rs` (or wherever panels are declared), `crates/rl-ui/src/lib.rs` (module doc names it), `crates/rl-ui/src/game_menu.rs` (the "Main menu" row), `crates/rl-engine/src/lib.rs`, `CHANGELOG.md`
- Reference: `examples/foundry/src/title.rs` lines 35-325 (the behaviour to reproduce), `crates/rl-ui/src/game_menu.rs` (words, keys, drawing), `crates/rl-ui/src/panel/settings.rs` (`SettingsLayout`, `settings_modal`)

**Interfaces:**
- Consumes: task 1's `HoldStart`, `begin_run`, `SavedRun`, `ContinueRun`, `AbandonRun`, `LeaveRun`.
- Produces, in `rl_ui` and the prelude:
  - `pub struct TitlePanel { .. }` with `TitlePanel::new(menu: Rect) -> Self` and `.words(words: TitleWords) -> Self`.
  - `#[derive(Clone, Debug)] pub struct TitleWords { pub continue_run: String, pub new_game: String, pub records: String, pub settings: String, pub exit: String, pub abandon: String, pub yes: String, pub no: String }` with `Default` ("Continue", "New Game", "Records", "Settings", "Exit", "Abandon the run in progress?", "Yes", "No").
  - `#[derive(Resource)] pub struct TitleMenu { pub picked: usize, pub confirm: Option<bool>, pub covered: bool }` and `pub enum TitleRow { Continue, NewGame, Records, Settings, Exit }` with `TitleMenu::rows(&self, ...) -> Vec<TitleRow>` being the rows that can be taken.
  - `#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)] pub enum TitleSet { Backdrop, Menu }`, chained, in `Update`, running only under `title_is_up`.
  - `pub fn title_is_up(hold: Option<Res<HoldStart>>, state: Res<State<EngineState>>) -> bool`.
  - Records: the row is offered when a modal named `records` has been declared (task 4's `RecordsPanel` declares it under that name) and `RunRecords` (task 2) holds a record, so task 4 adds nothing here.
- `GameMenuPanel`: `MenuItem::MainMenu` offered when `TitleMenu` exists, in the pause menu above Quit and on the end screen; taking it writes `LeaveRun`.

- [ ] **Step 1: Write the failing tests** in `crates/rl-ui/src/panel/title.rs`, through the headless key player (`rl_bevy::testing::press` or the crate's harness):
  - `with_nothing_saved_the_title_offers_new_game_first_and_never_continue`
  - `with_a_readable_save_the_title_picks_continue_out_first`
  - `with_an_unreadable_save_continue_is_not_offered_and_new_game_still_asks`
  - `the_direction_keys_skip_a_row_that_cannot_be_taken_and_wrap`
  - `continue_inserts_continue_run_and_begins_the_run`
  - `new_game_over_a_save_asks_with_no_picked_out_and_no_puts_the_question_away`
  - `yes_triggers_abandon_and_begins_one_run`: an observer counts `AbandonRun`; a `NewRun` counter reads 1 after.
  - `the_key_that_opens_settings_is_not_read_again_by_the_settings_screen` (Review Focus 1): with one declared setting, Enter on Settings opens the screen and the setting keeps its choice that frame.
  - `close_on_the_title_exits` (an `AppExit` is written).
  - `the_settings_screen_stands_over_the_menu_while_the_title_is_up_and_goes_back_when_a_run_begins`.
  - In `game_menu.rs`: `with_a_title_the_pause_menu_offers_main_menu_and_taking_it_leaves_the_run`; `without_a_title_no_main_menu_row_is_offered`.
  - Review Focus 3, in `crates/rl-save` or the panel's tests with `SavePlugin` added: `main_menu_from_the_pause_menu_then_continue_resumes_the_turn_the_run_was_left_on`.
- [ ] **Step 2: Run them and see them fail**: `cargo test -p rl-ui title` and `cargo test -p rl-ui game_menu`.
- [ ] **Step 3: Implement.** The panel inserts `HoldStart` and `TitleMenu` in `build`. Keys: `DirectionKeys` up and down, `CursorKeys` confirm and close, read the way `menu_keys` reads them so a rebound key is honoured. Taking a row that runs a schedule queues an exclusive command (`commands.queue`), as Foundry's `begin` does. Draw the rows centred in the panel's rectangle in `TitleSet::Menu` with `Tones::SELECT` for the picked row and `Tones::MUTED` for what cannot be taken, and the question in place of the rows while asked. Moving and restoring `SettingsLayout` is Foundry's `look_for_settings` and `begin`.
- [ ] **Step 4: Run the tests and see them pass**, then `cargo test -p rl-ui` whole and `cargo test -p rl-save`.
- [ ] **Step 5: Commit** with a `CHANGELOG.md` line: "a title screen is the engine's: `TitlePanel` holds the first run back and offers Continue, New Game, Settings and Exit, and the pause and end menus offer the way back to it".

### Task 4: Run records: the screen, the ending and the title's row

**Files:**
- Create: `crates/rl-ui/src/panel/records.rs`
- Modify: `crates/rl-ui/src/view/ending.rs` (`collect_ending` pushes `this_run`'s sections when `RunRecords` exists), `crates/rl-ui/src/lib.rs`, `crates/rl-engine/src/lib.rs`, `CHANGELOG.md`
- Reference: `crates/rl-ui/src/panel/settings.rs` and `crates/rl-ui/src/menu.rs` (a modal list), `crates/rl-ui/src/game_menu.rs` (drawing sections)

**Interfaces:**
- Produces: `pub struct RecordsPanel` with `RecordsPanel::new(rect: Rect) -> Self` and `.title(String)`, `.empty(String)`; `pub fn records_modal(modals: &Modals) -> Option<ModalId>`; `#[derive(Resource, Default)] pub struct RecordsScreen { pub picked: usize, pub open: bool, pub scroll: usize }`.
- A row reads `rank. score  summary  date` with the date as `YYYY-MM-DD` from `when`, or nothing; the detail view is the record's sections drawn as the ending screen draws them, scrolled when longer than the rectangle.

- [ ] **Step 1: Write the failing tests**: `the_records_screen_lists_records_best_first_with_their_scores`, `confirm_opens_the_record_picked_out_and_close_goes_back_to_the_list`, `close_on_the_list_closes_the_screen`, `with_no_records_the_screen_says_so`, `the_ending_screen_shows_the_sections_of_this_runs_record`, `the_title_offers_records_once_there_is_one`.
- [ ] **Step 2: Run them and see them fail**: `cargo test -p rl-ui records`.
- [ ] **Step 3: Implement.** Dates: compute the civil date from Unix seconds with the standard days-from-civil arithmetic; no new dependency.
- [ ] **Step 4: Run them and see them pass**, then `cargo test -p rl-ui` whole.
- [ ] **Step 5: Commit**: "the records are a screen, and the ending screen shows the run's own record".

### Task 5: Foundry onto the engine's title

**Files:**
- Modify: `examples/foundry/src/title.rs` (keep the art, its `paint*` functions and its art tests; drop `Title`, `SaveOnDisk`, `Choice`, `look_for_save`, `look_for_settings`, `read_title_keys`, `begin`, `abandon_and_begin`), `examples/foundry/src/run.rs` (`start` stops checking a title; `Resume` replaced by `ContinueRun`), `examples/foundry/src/save.rs` (`abandon` removed), `examples/foundry/src/plugin.rs`, `examples/foundry/src/main.rs` (`TitlePanel::new(screen.title_menu)` beside the other panels, at the rectangle the old menu drew in), `examples/foundry/src/testing/mod.rs` if it raised the title, `examples/foundry/DESIGN.md` and `CHANGELOG.md`.

**Interfaces:**
- Consumes: tasks 1 and 3.
- Foundry's picture is painted by one system in `TitleSet::Backdrop`; its menu is the engine's.

- [ ] **Step 1:** Move each of Foundry's title tests that tests menu behaviour into task 3's suite if it is not already covered there, and delete it here; keep the tests of the picture.
- [ ] **Step 2:** Make the change. The run's start no longer needs a title flag, since the engine holds the first run.
- [ ] **Step 3:** `cargo test -p foundry` whole, the fingerprint included, which must not move: the title changes nothing a turn does. If it moves, find out why before re-baselining.
- [ ] **Step 4:** Photograph the title to compare against the old one: `RL_CAPTURE=<scratchpad>/foundry-title.png cargo run -p foundry -- --seed 7` and read the picture; the menu must sit where the old one sat and read as well over the art.
- [ ] **Step 5: Commit**: "foundry's title screen is the engine's menu over its own picture".

### Task 6: Undermere: the crate, its content and the trait model

**Files:**
- Create: `examples/undermere/Cargo.toml` (lib and bin, as `examples/foundry/Cargo.toml`), `examples/undermere/src/lib.rs`, `src/content.rs`, `src/traits.rs`, `src/bestiary.rs`, `src/armory.rs`, `assets/traits.ron`, `assets/monsters.ron`, `assets/items.ron`, `assets/abilities.ron`, `assets/statuses.ron` if Foundry's content has statuses in a file (else in `content.rs` as Foundry does), `assets/monster_spawns.ron`, `assets/item_spawns.ron`
- Modify: workspace `Cargo.toml` members
- Reference: `examples/foundry/src/content.rs`, `src/droids.rs` and `src/droids/spawns.rs` (definitions through `NameRef`, `Names::load`, the roster as `ActorMaker`, `spawn_monster`), `src/gear.rs` (the armory as `ItemMaker`, `spawn_item_at`), `src/loot.rs`, and their assets

**Interfaces:**
- `content::registries() -> Registries`: factions `adventurer`, `beasts`, `goblins`, `undead`; damage kinds `slash`, `pierce`, `crush`, `fire`, `cold`, `poison`; the statuses the content names (at least `poisoned`, `scorched`, `mending`); slots `main hand`, `off hand`, `head`, `body`, `light`; tags `weapon`, `armor`, `potion`, `gold`, `arrow`, `light`.
- `content::combat_rules(&Registries) -> CombatRules`: adventurer hostile to all three, beasts hostile to goblins.
- `traits::TraitDef` (serde, every field but `name` optional, as the spec lists) and `traits::load(text: &str, names: &Names) -> Result<Registry<TraitDef>, LoadErrors>`.
- `traits::Folded { wits, speed, perception, dark_sight, flee_at, notice, hearing, light, resists, armor, abilities, statuses, says: Vec<(String, String)> }` and `traits::fold(traits: &[TraitId], all: &Registry<TraitDef>) -> Result<Folded, Clash>` where `Clash { field: &'static str, first: String, second: String }` displays as "`<field>` is set by both `<first>` and `<second>`".
- `bestiary::MonsterDef` (the resolved monster: name, glyph, colour, hp, attack, ranged, armor, faction, boss, drops, traits by name, and a `Folded`), `bestiary::Bestiary` (`Resource`: `defs: Registry<MonsterDef>`, `spawns`, implementing `ActorMaker` with `type Def = MonsterDef`), `bestiary::spawn_monster(commands: &mut Commands, def: &MonsterDef, at: Point, map: MapId, registries: &Registries) -> Entity`.
- `attack` and `ranged` parse as `"NdS+B kind"` and `"NdS+B kind @range"`, errors naming the monster.
- `armory::ItemDef`, `armory::Armory` (`Resource`, `ItemMaker`), `armory::spawn_item(commands, armory, def, registries) -> Entity`; `lends` resolves to trait ids and is folded onto the wearer while worn by a system in task 8.
- Content per the spec: twelve or so monsters across the bands and `the Drowned King` with `boss: true`; about twenty items including `gold` (tag `gold`, stacks).

- [ ] **Step 1: Write the failing tests** in the modules:
  - `every_content_file_loads_and_every_name_in_it_resolves` (one test per file is fine).
  - `a_monster_is_its_core_and_its_traits_folded_together` (a test monster with `swift` reads speed 150).
  - `lists_add_up_across_a_monsters_traits` (resists from two traits are both there).
  - `two_traits_setting_one_value_are_refused_naming_both_and_the_field` (Review Focus 5).
  - `an_unknown_trait_is_refused_by_name`.
  - `every_band_of_the_spawn_table_can_draw_a_monster_and_only_floor_ten_has_the_king`.
  - `every_drop_and_every_item_spawn_names_a_real_item`.
  - `a_spawned_monster_carries_what_its_traits_gave_it`: a headless world, spawn a monster with `glows` and `cowardly`; it has a `LightSource` and its profile flees at the trait's threshold.
- [ ] **Step 2: Run them and see them fail**: `cargo test -p undermere`.
- [ ] **Step 3: Implement**, writing each asset's field comment first.
- [ ] **Step 4: Run them and see them pass.**
- [ ] **Step 5: Commit**: "undermere's content: monsters as a core and named traits, an armory, and the spawn tables".

### Task 7: Undermere: the town, the floors, the ways through, and the light

**Files:**
- Create: `examples/undermere/src/dungeon.rs`, `src/stairs.rs`, `src/light.rs`, `assets/tiles.ron`, `assets/props.ron`, `assets/prefabs/mereholt.ron`, `assets/prefabs/throne.ron`, `src/prefabs.rs` if loading the pieces wants its own module as Foundry's does
- Reference: `examples/foundry/src/decks.rs`, `src/lifts.rs`, `src/light.rs`, `src/prefabs.rs`, `examples/delve/src/floors.rs` (caves, water, a lit floor), `examples/foundry/assets/tiles.ron` (the `Vary` jitter and shimmer)

**Interfaces:**
- `dungeon::FLOORS: u32 = 10`, `dungeon::TOWN: MapId = MapId(100)`, `dungeon::map_of(floor: u32) -> MapId`, `dungeon::floor_of(map: MapId) -> Option<u32>` (`None` for the town), `dungeon::Biome` (a closed enum is right in a game) with `Biome::of(floor) -> Biome`, and `dungeon::Undermere` implementing `PlaceRules`, with `new(seed: RunSeed) -> Self`, `tiles()`, `appearance()`, `prefabs()`.
- Floors as the spec's table, each biome with its own wall and floor tiles in `tiles.ron`, every tile authored with both colours and a `Vary`, water and lava shimmering.
- Mereholt: a hand-drawn piece about 60 by 36 tiles: the square, the tavern, smithy, apothecary, temple and houses with doors, the lake shore, lamp posts, and the gate's ring of stones; marks for where a run arrives and where the gate stands.
- `stairs::link_places`: on a first arrival lays the way up at the entry and the way down at the exit (the gate in town leads to floor one's entry; floor one's way up leads to the town's gate; floor ten has no way down), and on every arrival tells the place's line ("Floor 3: the Fungal Deeps. Something glows in the dark.").
- `light::set_ambient`: per place, as Foundry's; `light::light_fixtures`: lamps, braziers and glowing fungus lit on first arrival, as Foundry's `light_the_lamps`.

- [ ] **Step 1: Write the failing tests** in `dungeon.rs`, as Foundry's `decks.rs` tests: `every_floor_and_the_town_build_over_a_span_of_seeds_with_an_entry_and_a_way_on`, `every_open_cell_of_every_floor_can_be_walked_to_from_its_entry_with_everything_standing`, `floor_ten_holds_the_throne_and_the_king`, `the_town_has_one_gate_and_the_arrival_is_not_on_it`, `each_biome_draws_its_own_walls_and_floors`; in `stairs.rs`: `the_gate_leads_to_floor_one_and_floor_ones_way_up_leads_back_to_the_gate`, `floor_ten_has_no_way_down`.
- [ ] **Step 2: Run them and see them fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them and see them pass.**
- [ ] **Step 5: Commit**: "undermere's places: Mereholt and its gate, ten floors in five biomes, and the light in each".

### Task 8: Undermere: the run, play, the screen and saving

**Files:**
- Create: `examples/undermere/src/plugin.rs`, `src/run.rs`, `src/input.rs`, `src/save.rs`, `src/main.rs`, `src/testing/mod.rs`, and a population module (`src/population.rs`) placing each floor's monsters on first arrival as Foundry's `populate_deck` does
- Reference: `examples/foundry/src/plugin.rs`, `run.rs`, `input.rs`, `save.rs`, `main.rs`, `testing/mod.rs`, `droids/spawns.rs`

**Interfaces:**
- `plugin::UndermerePlugin` (every game system, added by `main.rs` and `testing::headless` alike) and `plugin::narrator() -> NarratorPlugin`.
- `run::prepare`, `run::start` (the adventurer, a sword, leather armour, a lit lantern, two potions of healing; warped into `TOWN`), `run::resume` answering `ContinueRun` (task 1's), `run::Adventurer` marker.
- Worn items lend their traits: a system folds an item's `lends` onto its wearer while worn and takes it off again, using the engine's `Bestows` and status holds where the trait is a stat or a status, and the components (`Resists`, `LightSource`, `DarkSight`) where it is one of those.
- `save::SLOT = "undermere"`, `save::VERSION = 1`, `save::register`: kinds for the adventurer, monsters, items and stairs; states `Quests`, `Counters`, and the deepest floor (task 9 adds `Tally`).
- `main.rs`: `Screen` as the spec cuts it at 160 by 45 with `across(2)`; every panel Foundry adds, the nearby rail's activity words ("sleeping" where a mind has noticed nothing, "hunting", "searching", "fleeing", "guarding"); `saves_for(scripted)` as Foundry's.
- Keys as Foundry's, with `>` `<` and Enter for stairs and the gate.

- [ ] **Step 1: Write the failing tests**: `a_new_run_puts_the_adventurer_in_mereholt_with_their_kit` (span of seeds), `walking_through_the_gate_arrives_on_floor_one` and `taking_every_way_down_reaches_floor_ten` (warp-driven, as Foundry's lift tests), `a_floor_is_populated_on_first_arrival_away_from_the_entry`, `the_rail_the_log_and_the_map_are_where_the_screen_cuts_them`, `a_map_tile_is_drawn_two_cells_wide`, `a_ring_that_lends_a_trait_gives_it_while_worn_and_takes_it_back`, `a_run_saved_on_arrival_continues_on_the_same_floor_with_the_same_kit`, `a_scripted_run_never_touches_the_players_save`.
- [ ] **Step 2: Run them and see them fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them and see them pass**, then `cargo run -p undermere -- --seed 7` and photograph the town and a floor of each biome with `RL_CAPTURE` (warp with a debug start variable as Foundry's `FOUNDRY_START`, here `UNDERMERE_START=<floor>`), read each picture, and fix what looks wrong before committing.
- [ ] **Step 5: Commit**: "undermere plays: the adventurer, the town, the floors and their people, the screen, and saving".

### Task 9: Undermere: victory, the tally, the score and the record

**Files:**
- Create: `examples/undermere/src/tally.rs`, `assets/quests.ron`, the mission loading (in `tally.rs` or `src/quest.rs`, after Foundry's `mission.rs`)
- Modify: `src/plugin.rs`, `src/save.rs` (`Tally` as a saved state), `src/main.rs` (`RecordsPanel`, and `GameMenuPanel` words for death and victory)

**Interfaces:**
- `tally::Tally` (`Resource`, `SaveableState`): `kills: u32`, `threat_slain: f32`, `deadliest: Option<(String, f32)>`, `gold_found: u32`, `damage_dealt: u32`, `damage_taken: u32`, `potions_drunk: u32`, `items_found: u32`, `minibosses: u32`, `deepest: u32`.
- `tally::score(t: &Tally, won: bool) -> Vec<(String, i64)>`: the five lines of the design's table, in order, and `tally::total(&lines) -> i64`.
- `tally::record`: on the run's end writes one `RecordRun` (task 2) with the summary and two sections, "Score" (each line and the total) and "The run" (turns, deepest floor, kills and the deadliest, damage dealt and taken, potions drunk, items found, gold found, the seed).
- A monster with `boss: true` dying writes the fact the victory quest counts.

- [ ] **Step 1: Write the failing tests**: `slaying_the_king_wins_the_run`, `the_score_is_the_sum_of_its_lines` (property over a span of tallies), `a_death_ends_in_a_record_naming_what_killed_and_where`, `gold_is_counted_once_however_often_it_is_picked_up`, `a_kill_adds_its_threat_and_the_deadliest_is_kept`, `the_tally_survives_a_save_and_continue`.
- [ ] **Step 2: Run them and see them fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them and see them pass.**
- [ ] **Step 5: Commit**: "undermere's run ends in a score: depth, the slain, treasure, mini-bosses and victory, line by line, and a record of it".

### Task 10: Undermere: the title

**Files:**
- Create: `examples/undermere/src/title.rs`
- Modify: `src/main.rs` (`TitlePanel` at the menu's rectangle), `src/plugin.rs`

**Interfaces:**
- `title::paint_backdrop`, one system in `TitleSet::Backdrop` painting the whole terminal: the lake, the stars, the moon and its broken reflection, mist, Mereholt's windows, the gate's stones and their violet glow, and UNDERMERE lettered large, with only the stars, the glow and the mist moving.

- [ ] **Step 1: Write the failing tests**: `the_title_is_up_at_launch_and_no_run_has_begun`, `the_backdrop_paints_every_cell_of_the_terminal`, `new_game_from_the_title_puts_the_adventurer_in_mereholt`.
- [ ] **Step 2: Run them and see them fail.**
- [ ] **Step 3: Implement**, then photograph it (`RL_CAPTURE`) at two moments and read both: the menu must be legible over the picture and the picture worth looking at.
- [ ] **Step 4: Run them and see them pass.**
- [ ] **Step 5: Commit**: "undermere opens on the black lake and the gate, with the engine's menu over it".

### Task 11: The fingerprint, the full gate, the look, and the documentation pass

**Files:**
- Create: `examples/undermere/tests/fingerprint.rs` (as Foundry's), `docs/design/records.md`, `docs/guide/src/systems/records.md`, `docs/guide/src/systems/title.md`
- Modify: `docs/OVERVIEW.md`, `README.md`, `CHANGELOG.md`, `AGENTS.md` (layout list), `docs/README.md`, the guide's last chapter (`docs/guide/src/09-where-to-go-next.md`), `docs/guide/src/SUMMARY.md`, `examples/undermere/DESIGN.md` (what part 1 built)

- [ ] **Step 1:** The fingerprint tripwire, labelled as one, over a scripted run of a few seeds.
- [ ] **Step 2:** Play it: a run from the title through the town and down at least three floors, photographing each new thing, and fix what looks off.
- [ ] **Step 3:** The documentation, as `AGENTS.md` lays it out: each new plugin on a system page with a `documents:` manifest, `scripts/check-systems-style.sh <page>` clean, `python3 scripts/check-systems.py --bless <system>` for each page confirmed against the code, every page whose files moved re-read and blessed.
- [ ] **Step 4:** Everything CI runs: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `scripts/check-tiers.sh`, `scripts/check-tiers.sh --wasm`, `scripts/check-overview.sh`, `scripts/check-guide.sh`, `python3 scripts/check-systems.py`, `scripts/check-template.sh` if the API it uses moved.
- [ ] **Step 5: Commit** the documentation pass.
