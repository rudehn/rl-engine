# Foundry: the lift out and saving a run, Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Foundry can be won, by charging the core and riding the lift out on deck one, and a run left in the middle can be continued from the title screen.

**Architecture:** The engine gains an ordered end of frame (`EndOfFrame` sets in `Last`), an arrival save on `SavePlugin`, and a saved `Counters`. Foundry gains a lift-out entity answered through the engine's refusal of a `GoThrough`, a fifth victory quest, a `save.rs` registering its kinds and resources, a resume path in `run.rs`, and a Continue and a confirm on its title screen.

**Tech Stack:** Rust 2024, Bevy 0.19, RON, the workspace crates `rl-bevy`, `rl-save`, `rl-rules`, `rl-ui`, and `examples/foundry`.

**Spec:** `docs/superpowers/specs/2026-09-23-foundry-lift-out-and-saving-design.md`

## Global Constraints

- Plain dash in all prose, never an em dash; one sentence per line in long Markdown.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`, `cargo test --workspace --no-fail-fast`, `scripts/check-tiers.sh` and `--wasm`, `scripts/check-overview.sh`, `scripts/check-guide.sh` and `python3 scripts/check-systems.py` all pass at the end.
- No theme words in engine crates; `#![deny(missing_docs)]`; doc comments say why at the density of `crates/rl-core/src/turn.rs`.
- Never order a system after another crate's system function; order through a set.
- Foundry's ambiguity test (`examples/foundry/src/plugin/ambiguity.rs`) must pass: every newly conflicting pair is ordered or allowed with a stated reason.
- The lift out's refusal line is exactly "The lift will not move until the core is charged."; the win's line is exactly "The lift climbs out of the foundry, and the core goes up under it."; the confirm is exactly "Abandon the run in progress?" with "Yes" and "No", No picked out.
- The save slot is `"foundry"` and `VERSION` starts at 1.
- Do not commit; the user commits.

## Review Focus

1. **A run continued on a deck other than the one it started on**, with droids mid-hunt and a spent console: nothing is populated twice, no second console appears, the deck's lifts are where they were. Pinned in Task 3 by the round trip on deck three.
2. **The uplink's extra reach on a gun worn when the run was saved**: it comes back once, not zero times and not twice. Pinned in Task 3.
3. **A restart from the in-game menu in the same frame as a deck arrival**: the arrival save must not write a torn-down world after the restart forgot the save. Pinned in Task 1 by the end-of-frame order.
4. **Continue picked with a save this build cannot read**: Continue is dim, and New Game offers to clear it rather than crashing on start. Pinned in Task 4.
5. **Going through the lift out on a deck that is not deck one, or going through anything else on deck one**: only the lift out answers, and only on deck one. Pinned in Task 2.

---

### Task 1: The engine: an ordered end of frame, the arrival save, and saved counters

**Files:**
- Modify: `crates/rl-bevy/src/plugin.rs` (a new `EndOfFrame` set enum, configured in `CorePlugin`; `restart_runs` put in `EndOfFrame::Restart`)
- Modify: `crates/rl-bevy/src/combat.rs:929` (`bury_the_dead` into `EndOfFrame::Bury`)
- Modify: `crates/rl-bevy/src/consumable.rs` (`bury_spent` into `EndOfFrame::Bury`, the `.before(restart_runs)` dropped)
- Modify: `crates/rl-bevy/src/lib.rs` (export `EndOfFrame`)
- Modify: `crates/rl-save/src/run.rs` (`SavePlugin::on_arrival`, `note_arrivals`, `save_on_arrival`, `refresh_stash` into `EndOfFrame::Save`, `SaveableState for Counters`)
- Modify: `crates/rl-save/src/unload.rs:79` (`flush_on_exit` into `EndOfFrame::Save`, after `refresh_stash`)
- Modify: `docs/guide/src/systems/saving.md`, `docs/OVERVIEW.md`, `CHANGELOG.md`

**Interfaces:**
- Produces: `rl_bevy::EndOfFrame::{Save, Bury, Restart}` (a `SystemSet` chained in `Last`); `rl_save::SavePlugin::on_arrival(self) -> Self`; `impl SaveableState for rl_bevy::Counters` with `type Saved = rl_rules::Ledger`.

- [ ] **Step 1: Write the failing tests** in `crates/rl-save/src/run.rs`'s test module, using its existing helpers for an app with a `MemoryBackend` (read the module's `app()`-style helper first and reuse it rather than writing a second).

```rust
/// With `on_arrival()`, arriving somewhere writes the run: a deck entered
/// is a save point, and a crash loses at most the deck in hand.
#[test]
fn a_run_is_saved_on_arrival_when_asked_and_not_otherwise() {
    for (asked, expected) in [(true, true), (false, false)] {
        let mut app = saving_app(asked); // the module's app, SavePlugin with or without .on_arrival()
        let player = spawn_saved_player(&mut app);
        app.world_mut().write_message(rl_bevy::WarpRequest::into_place(player, rl_bevy::MapId::new(1)));
        app.update();
        app.update();
        assert_eq!(app.world().resource::<Saves>().exists("test"), expected, "asked: {asked}");
    }
}

/// The fact ledger is the engine's, so its saving is too.
#[test]
fn counters_come_back_as_they_were_counted() {
    let mut ledger = rl_rules::Ledger::default();
    // count something the way FactsPlugin would; read `rl_rules::events::ledger` for the call
    let saved = rl_bevy::Counters(ledger.clone()).capture();
    let mut fresh = rl_bevy::Counters(rl_rules::Ledger::default());
    fresh.restore(saved);
    assert_eq!(fresh.0, ledger);
}
```

If the module has no place-building helper, build the place the way `crates/rl-bevy/src/places.rs`'s own tests do (`PlaceRulesRes` with a test builder) and say so in the ledger.

- [ ] **Step 2: Run them to see them fail.**
Run: `cargo test -p rl-save --lib -- on_arrival counters_come_back`
Expected: compile errors, `on_arrival` and the `Counters` impl do not exist.

- [ ] **Step 3: The end of frame.** In `crates/rl-bevy/src/plugin.rs`:

```rust
/// The stages of `Last`, in order: what the run is written down as, then
/// what the frame is done with, then a restart.
///
/// A save written after a restart tore the run down would write a world
/// with no map in it, and a burial after the restart would despawn what
/// the new run just spawned; so the three are named here, where the
/// schedule is, and each crate puts its own system in its stage.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EndOfFrame {
    /// The run written to the stash and the slot.
    Save,
    /// The dead and the spent despawned.
    Bury,
    /// A restart asked for this frame.
    Restart,
}
```

In `CorePlugin::build`: `.configure_sets(Last, (EndOfFrame::Save, EndOfFrame::Bury, EndOfFrame::Restart).chain())` and `restart_runs.in_set(EndOfFrame::Restart)`.
`bury_the_dead.in_set(EndOfFrame::Bury)` in `combat.rs`; `bury_spent.in_set(EndOfFrame::Bury)` in `consumable.rs`, dropping its `.before(restart_runs)` and the comment above it, replaced by one saying the stage does it.
Export `EndOfFrame` beside `EngineSet` in `lib.rs`, in both the root and the prelude lists.

- [ ] **Step 4: The arrival save.** In `crates/rl-save/src/run.rs`:

```rust
pub struct SavePlugin {
    slot: String,
    version: u32,
    on_arrival: bool,
}

impl SavePlugin {
    /// Also writes the run to the slot on every frame in which the player
    /// arrived somewhere, so each place entered is a save point and a
    /// crash loses at most the place in hand. Off by default: a game
    /// without places, or one that saves on a key, is unchanged.
    pub fn on_arrival(mut self) -> Self {
        self.on_arrival = true;
        self
    }
}

/// Whether the frame held an arrival, for [`save_on_arrival`].
#[derive(Resource, Default)]
struct ArrivedThisFrame(bool);

/// Notes an arrival. A reader of its own rather than a cursor inside the
/// exclusive system, so the message is read the way every other reader
/// reads it.
fn note_arrivals(mut entered: MessageReader<PlaceEntered>, mut arrived: ResMut<ArrivedThisFrame>) {
    arrived.0 |= entered.read().count() > 0;
}

/// Writes the run once for a frame that held an arrival, while playing.
/// A save that fails is logged and the run goes on, as a stash that fails is.
pub fn save_on_arrival(world: &mut World) {
    if !std::mem::take(&mut world.resource_mut::<ArrivedThisFrame>().0) {
        return;
    }
    if *world.resource::<State<EngineState>>().get() != EngineState::Playing {
        return;
    }
    if let Err(e) = save_run(world) {
        error!("the run could not be saved on arrival: {e}");
    }
}
```

In `build`: `SavePlugin::new` sets `on_arrival: false`; `refresh_stash.in_set(EndOfFrame::Save)`; when `self.on_arrival`, `init_resource::<ArrivedThisFrame>()` and `(note_arrivals, save_on_arrival).chain().in_set(EndOfFrame::Save).before(refresh_stash)`.
The first arrival of a run happens on the frame `NewRun` set `Playing`, which the state only reaches the next frame; if the first test's first arrival is skipped for that reason, keep `ArrivedThisFrame` set until a frame in which the state is `Playing` rather than clearing it, and note the ruling.
In `crates/rl-save/src/unload.rs`: `flush_on_exit.in_set(EndOfFrame::Save).after(crate::run::refresh_stash)`, so the stash written on exit is the freshest.

- [ ] **Step 5: The saved counters.** Beside `impl SaveableState for Quests`:

```rust
/// The fact ledger is the engine's, so its saving is too: a game with a
/// [`Counters`] adds `save_state::<Counters>()` and nothing more.
impl SaveableState for Counters {
    type Saved = rl_rules::Ledger;

    fn capture(&self) -> rl_rules::Ledger {
        self.0.clone()
    }

    fn restore(&mut self, saved: rl_rules::Ledger) {
        self.0 = saved;
    }
}
```

- [ ] **Step 6: Run the tests.**
Run: `cargo test -p rl-save -p rl-bevy --lib`
Expected: all pass, the two new ones included.

- [ ] **Step 7: Docs.** `docs/guide/src/systems/saving.md`: `on_arrival()` and the saved `Counters` in `The model`, `EndOfFrame` where the page says when the stash is refreshed; add `crates/rl-bevy/src/plugin.rs` to its manifest only if the page now claims where a set sits in its chain. `docs/OVERVIEW.md`: the set list gains `EndOfFrame::{Save, Bury, Restart}` in `Last`, and the saving line the arrival save and `Counters`. `CHANGELOG.md` under `Unreleased`: `SavePlugin::on_arrival()`, `Counters` saved, and `EndOfFrame`, with the note that a game that ordered its own `Last` system against `restart_runs` orders against the set instead.
Run `scripts/check-systems-style.sh docs/guide/src/systems/saving.md`, re-read every page `python3 scripts/check-systems.py` flags, and bless each.

- [ ] **Step 8: Clippy, then the task's tests.**
Run: `cargo clippy --workspace --all-targets -- -D warnings && cargo test -p rl-save -p rl-bevy -p foundry`
Expected: clean, and all pass, Foundry's ambiguity test included (Foundry does not add `SavePlugin` yet).

---

### Task 2: The lift out, and the run won

**Files:**
- Modify: `examples/foundry/src/lifts.rs` (`Lift`, `LiftOut`, the lift out laid in `link_decks`, `ride_out`)
- Modify: `examples/foundry/src/mission.rs` (`Facts::lift_out`, `On::LiftOut`, `answer_victory`)
- Modify: `examples/foundry/assets/quests.ron` (the fifth quest, the schema comment)
- Modify: `examples/foundry/src/plugin.rs` (register `ride_out` in `TurnSet::React`, `answer_victory` in `Update` before `EngineSet::Input`)
- Modify: `examples/foundry/src/climb.rs:1-7` (the header), `examples/foundry/TODO.md`, `examples/foundry/DESIGN.md`
- Test: `examples/foundry/src/lifts.rs` tests module

**Interfaces:**
- Produces: `pub struct Lift;` and `pub struct LiftOut;` (both `Component, Debug, Clone, Copy, Default`) in `crate::lifts`; `crate::mission::Facts { charge_set, lift_out }`; `crate::mission::CORE_QUEST: &str = "core_charge"`.

- [ ] **Step 1: Write the failing tests** in `lifts.rs`:

```rust
#[cfg(test)]
mod tests {
    use rl_engine::rl_core::RunSeed;

    use super::*;
    use crate::mission::Facts;

    fn me(app: &mut App) -> Entity {
        app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap()
    }

    fn go_through(app: &mut App) {
        let player = me(app);
        app.world_mut().write_message(Intent::new(player, GoThrough));
        crate::testing::settle(app);
    }

    fn said(app: &App, line: &str) -> bool {
        app.world().resource::<MessageLog>().iter().any(|e| e.text == line)
    }

    /// Sets every charge, the way the consoles report them.
    fn charge_everything(app: &mut App) {
        let kind = app.world().resource::<Facts>().charge_set;
        for deck in [3u64, 6, 9, 10] {
            app.world_mut().write_message(Happened(Fact::new(kind).about(deck)));
            app.update();
        }
    }

    #[test]
    fn the_lift_out_stands_where_the_commando_came_in() {
        let mut app = crate::testing::headless(RunSeed(4));
        crate::testing::settle(&mut app);
        let player = me(&mut app);
        let at = app.world().get::<Position>(player).unwrap().0;
        let outs: Vec<Point> = app.world_mut().query_filtered::<&Position, With<LiftOut>>().iter(app.world()).map(|p| p.0).collect();
        assert_eq!(outs, [at], "one lift out, under the commando's feet at the start");
    }

    #[test]
    fn before_the_core_the_lift_out_refuses_with_a_line_and_costs_no_turn() {
        let mut app = crate::testing::headless(RunSeed(4));
        crate::testing::settle(&mut app);
        let before = crate::testing::clock(&app);
        go_through(&mut app);
        assert!(said(&app, "The lift will not move until the core is charged."));
        assert_eq!(crate::testing::clock(&app), before, "no turn spent");
        assert_eq!(*app.world().resource::<State<EngineState>>().get(), EngineState::Playing, "and the run goes on");
    }

    #[test]
    fn after_the_core_the_lift_out_wins_the_run() {
        let mut app = crate::testing::headless(RunSeed(4));
        crate::testing::settle(&mut app);
        charge_everything(&mut app);
        go_through(&mut app);
        crate::testing::settle(&mut app);
        assert_eq!(*app.world().resource::<State<EngineState>>().get(), EngineState::Over, "the run is over");
        assert!(app.world().get_resource::<Ending>().is_some_and(|e| e.won), "and won");
    }

    #[test]
    fn going_through_nothing_elsewhere_is_the_engines_refusal_alone() {
        let mut app = crate::testing::headless(RunSeed(4));
        crate::testing::settle(&mut app);
        crate::testing::pass_turns(&mut app, 1);
        let player = me(&mut app);
        let beside = crate::testing::key_toward_an_open_cell(&app, player); // add to testing/ if missing: the direction key of any walkable neighbour
        rl_engine::rl_bevy::testing::press(&mut app, beside);
        go_through(&mut app);
        assert!(!said(&app, "The lift will not move until the core is charged."), "only the lift out answers");
    }
}
```

Read `crates/rl-bevy/src/state.rs` for the field on `Ending` that says won, and use its real name; the `key_toward_an_open_cell` helper goes in `examples/foundry/src/testing/mod.rs` if nothing like it exists (look at `key_toward_the_console` in `testing/` first).

- [ ] **Step 2: Run them to see them fail.**
Run: `cargo test -p foundry --lib lifts::`
Expected: compile errors for `LiftOut`, then, once it exists, assertion failures.

- [ ] **Step 3: The lift out.** In `lifts.rs`, the two markers with doc comments, and in `link_decks` after the `deck > 1` branch:

```rust
if deck == 1 {
    // The way out, where the commando came in: no `Transition`, so the
    // engine refuses a `GoThrough` on it, and [`ride_out`] answers.
    commands.spawn((Position(ev.entry), OnMap(ev.map), Lift, LiftOut, Name::new("lift out"), Glyph::new('<', LIFT).on_layer(1)));
}
```

and `Lift` added to the tuple of both existing lift spawns.
Update the module doc: deck one's lift out, and why it has no `Transition`.

- [ ] **Step 4: The fact, the objective and the quest.** In `mission.rs`: `Facts` gains `lift_out: FactKind`, the registry built from `["charge_set", "lift_out"]`; `On` gains `/// The lift out on deck one was ridden. LiftOut,` and `matcher` answers `On::LiftOut => Matcher::any(facts.lift_out)`; `pub const CORE_QUEST: &str = "core_charge";` beside `CHARGE_QUESTS`.
In `quests.ron`, the fifth quest exactly as spec section 3 writes it, and the schema comment: `on` lists `ChargeSet(deck) | LiftOut`, and `victory` reads "finishing this wins the run: the last quest, the lift out".

- [ ] **Step 5: `ride_out`.**

```rust
/// The player standing on the lift out, as [`ride_out`] reads them.
type Rider<'w, 's> = Query<'w, 's, (Entity, &'static Position, Option<&'static OnMap>), With<Player>>;

/// Answers a `GoThrough` on the lift out, which the engine has already
/// refused and given the turn back for, since the lift out leads nowhere
/// the engine could take anyone.
///
/// Before the core is charged it says so; after, it reports the fact the
/// last quest counts, and finishing that quest is what wins. Reads the
/// pass's own `Intent<GoThrough>` beside the refusal, because a refusal
/// does not say what was refused and the player may be standing on the
/// lift out when a pickup of nothing is refused.
pub fn ride_out(mut refused: MessageReader<ActionRefused>, mut going: MessageReader<Intent<GoThrough>>, rider: Rider, outs: Query<(&Position, Option<&OnMap>), With<LiftOut>>, mission: Mission) {
    let asked: Vec<Entity> = going.read().map(|i| i.actor).collect();
    for r in refused.read() {
        let Ok((me, at, on)) = rider.get(r.actor) else { continue };
        if !asked.contains(&me) {
            continue;
        }
        let map = on.map(|m| m.0).unwrap_or(MapId::SURFACE);
        if !outs.iter().any(|(p, m)| p.0 == at.0 && m.map(|m| m.0).unwrap_or(MapId::SURFACE) == map) {
            continue;
        }
        mission.answer();
    }
}
```

with a `Mission` `SystemParam` holding `Res<Quests>`, `Res<Facts>`, `MessageWriter<Happened>` and `MessageWriter<Tell>`, whose `answer(&mut self)` writes the `Tell` when `quests.tracker.state(quests.defs.expect(CORE_QUEST)) != QuestState::Done` and the `Happened(Fact::new(facts.lift_out))` otherwise.
Register in `plugin.rs`: `crate::lifts::ride_out.in_set(TurnSet::React)`.

- [ ] **Step 6: The win.** In `mission.rs`:

```rust
/// Ends the run won when the tracker reports the victory quest done, the
/// way Corsair's quests end its runs: the win is the mission's, written
/// in `quests.ron`, not a system that knows which quest is last.
pub fn answer_victory(mut changes: MessageReader<QuestChange>, mut over: MessageWriter<RunOver>) {
    for change in changes.read() {
        if let Change::QuestDone { victory: true, .. } = change.0 {
            over.write(RunOver::won().saying("The lift climbs out of the foundry, and the core goes up under it."));
        }
    }
}
```

Registered in `Update`, `.before(EngineSet::Input)`, beside `offer_the_pick`, with the same comment's reasoning in one line.

- [ ] **Step 7: Run the tests.**
Run: `cargo test -p foundry --lib`
Expected: all pass, the ambiguity test included; if `ride_out` or `answer_victory` conflict with another Foundry system, order them or allow the pair with its reason.

- [ ] **Step 8: The words.** `climb.rs`'s header: the lift out on deck one ends the run once the core is charged. `examples/foundry/TODO.md`: the "No way to win" line gone. `examples/foundry/DESIGN.md`: the lift out and the way out marked **exists**.

- [ ] **Step 9: Clippy, then the task's tests.**
Run: `cargo clippy -p foundry --all-targets -- -D warnings && cargo test -p foundry`
Expected: clean; all pass; the fingerprint tripwire does not move (the lift out is an entity, not a roll). If it moves, re-baseline it and say why in `CHANGELOG.md`.

---

### Task 3: What Foundry saves, and continuing a run

**Files:**
- Create: `examples/foundry/src/save.rs`
- Modify: `examples/foundry/src/lib.rs` (`pub mod save;`)
- Modify: `examples/foundry/src/gear.rs` (`ItemKind` on every item `spawn_item` makes)
- Modify: `examples/foundry/src/run.rs` (`Commando`, `spawn_commando`, `prepare`, `Resume`, `resume`)
- Modify: `examples/foundry/src/upgrades.rs:42` (`Upgrade` derives `Serialize, Deserialize`), `examples/foundry/src/climb.rs` (`SaveableState for Deepest`)
- Modify: `examples/foundry/src/plugin.rs` (`crate::save::register(app)`; `run::resume` in `NewRun`)
- Modify: `examples/foundry/src/main.rs` (`Saves::platform_default("foundry")` inserted)
- Modify: `examples/foundry/src/testing/mod.rs` (`Saves::new(MemoryBackend::default())` in `headless_without_foundry`; a `continued(text: &str) -> App` helper)
- Modify: `examples/foundry/src/plugin/ambiguity.rs` (whatever the new systems conflict with)
- Modify: `examples/foundry/Cargo.toml` if `serde` is not already a dependency

**Interfaces:**
- Consumes: `SavePlugin::on_arrival()`, `SaveableState for Counters` (Task 1); `Lift`, `LiftOut` (Task 2).
- Produces: `crate::save::{SLOT, VERSION, register, abandon}`; `crate::gear::ItemKind(pub Id<ItemDef>)`; `crate::run::{Commando, Resume, spawn_commando, prepare, resume}`; `crate::testing::continued(text: &str) -> App`.

- [ ] **Step 1: Write the failing round trip** in `save.rs`'s test module:

```rust
#[cfg(test)]
mod tests {
    use rl_engine::rl_core::RunSeed;
    use rl_engine::rl_save::{Saves, save_run};

    use super::*;

    /// What a run looks like from outside, for comparing a run with the
    /// same run continued.
    #[derive(Debug, PartialEq)]
    struct Looks {
        deck: u32,
        at: Point,
        health: i32,
        lamp: bool,
        taken: Vec<crate::upgrades::Upgrade>,
        deepest: u32,
        first_charge_done: bool,
        pack: Vec<(String, u32)>,
        heat: Vec<u32>,
        reach: Vec<i32>,
        spent_consoles: usize,
        droids: usize,
        lifts: usize,
    }

    fn looks(app: &mut App) -> Looks { /* read each field from the world; `pack` sorted by name */ }

    /// A run on deck three, with a gun half hot and worn under the uplink,
    /// a spent console, two upgrades, the lamp off, slugs in the pack and a
    /// wounded droid, comes back as it was.
    #[test]
    fn a_run_saved_on_deck_three_comes_back_as_it_was() {
        let mut app = crate::testing::headless(RunSeed(2));
        let me = crate::testing::beside_the_console(&mut app); // deck three
        // set the charge through the real verb, pick Uplink and Servos through crate::testing::pick,
        // equip a hand blaster and fire it twice (crate::testing::fire_at_a_target), give slugs
        // (crate::testing::give_slugs), switch the lamp off (press shift+L), and wound a droid
        crate::testing::settle(&mut app);
        let before = looks(&mut app);
        save_run(app.world_mut()).expect("the run saves");
        let text = app.world().resource::<Saves>().load(SLOT).unwrap().expect("a save");

        let mut continued = crate::testing::continued(&text);
        assert_eq!(looks(&mut continued), before);
        crate::testing::pass_turns(&mut continued, 5); // and it plays on
    }

    #[test]
    fn the_uplinks_reach_comes_back_once_on_a_gun_worn_when_saved() { /* reach before == reach after, not +1 */ }

    #[test]
    fn a_deck_arrival_writes_the_save() { /* crate::testing::arrive_on(&mut app, 2); the slot exists */ }

    #[test]
    fn a_death_deletes_the_save() { /* arrive, kill the commando (Health 0 through a DamageEvent), settle; the slot is gone */ }

    #[test]
    fn a_win_deletes_the_save() { /* charge everything, ride out, settle; the slot is gone */ }
}
```

Every elided body is written out in full in this step, reading each value from the world the way the existing Foundry tests do; the plan names what each asserts.

- [ ] **Step 2: Run them to see them fail.**
Run: `cargo test -p foundry --lib save::`
Expected: compile errors (`save.rs` does not exist).

- [ ] **Step 3: `ItemKind`.** In `gear.rs`:

```rust
/// Marks an item with the definition it was made from, which is what a
/// save writes down and a continued run makes it again from.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemKind(pub Id<ItemDef>);
```

inserted in `spawn_item`'s first `commands.spawn` tuple.

- [ ] **Step 4: The commando, split out of `start`.** In `run.rs`: `pub struct Commando;` (a marker with a doc comment); `pub fn spawn_commando(commands: &mut Commands, registries: &Registries) -> Entity` holding the spawn tuple and the inventory and slots insert now inline in `start`, with `Commando` added to the tuple; `pub fn prepare(commands: &mut Commands, seed: RunSeed, registries: &Registries)` holding every `insert_resource` `start` makes before the player, and `PlaceRulesRes`. `start` becomes `prepare`, `spawn_commando`, the log line, the warp and `Playing`, and returns at once when `Resume` exists as well as when the title is up.

- [ ] **Step 5: `save.rs`.** Module doc in the manner of Corsair's; then:

```rust
/// Bump when a kind's shape below changes so an old save would parse wrongly.
// v1: the first shape.
pub const VERSION: u32 = 1;

/// The slot every run saves to.
pub const SLOT: &str = "foundry";

/// What the save is made of.
pub fn register(app: &mut App) {
    app.add_plugins((SavePlugin::new(SLOT).version(VERSION).on_arrival(), UnloadPlugin))
        .save_kind::<ItemKind>()
        .save_kind::<crate::droids::Kind>()
        .save_kind::<Commando>()
        .save_kind::<Lift>()
        .save_state::<Taken>()
        .save_state::<Deepest>()
        .save_state::<Quests>()
        .save_state::<Counters>();
}

/// Deletes the saved run, for a New Game that abandons it.
pub fn abandon(world: &mut World) {
    world.resource::<Stash>().clear();
    if let Err(e) = world.resource::<Saves>().delete(SLOT) {
        error!("could not delete the save: {e}");
    }
}
```

and the four `Saveable` impls:
- `Commando`: `type Saved = CommandoSave { lamp: bool }`; capture reads whether `LightSource` is on the entity; restore runs `spawn_commando` through a `CommandQueue` (as Corsair's `ItemKind::restore` does) and removes `LightSource` when `!lamp`.
- `crate::droids::Kind`: `type Saved = String`, the definition's name from `Roster`; restore runs `droids::spawn_monster(&mut commands, roster, id, Point::ZERO, MapId::SURFACE, &registries)` through a `CommandQueue`, since the engine puts back where it stands and on which map.
- `ItemKind`: `type Saved = ItemSave { def: String, heat: Option<u32> }`; restore runs `gear::spawn_item` with an armory loaded from `Registries`, `EffectKinds` and `Moments`, then sets `Heat::now` (and `locked` from the heat reaching its limit, reading `heat.rs` for how a lock is decided).
- `Lift`: `type Saved = (char, bool)`, the glyph and whether it is the lift out; restore spawns `Lift`, `Name` ("lift out", "lift up" or "lift down" by the glyph and the flag), the glyph in `LIFT`'s colour on layer 1, and `LiftOut` when the flag says; the engine restores the `Transition`.

`impl SaveableState for Taken` (`Saved = Vec<Upgrade>`) and for `Deepest` (`Saved = u32`).

- [ ] **Step 6: Resuming.** In `run.rs`:

```rust
/// Asks the next `NewRun` to continue the saved run rather than start one.
#[derive(Resource, Debug, Clone, Copy)]
pub struct Resume;

/// Continues the saved run: the world built again from the saved seed,
/// the run restored into it, and what the run did to the commando put
/// back after the engine has given it back its gear. After `start` and
/// `mission::start` in `NewRun`, so the resources a restore writes into
/// exist.
pub fn resume(world: &mut World) {
    if world.remove_resource::<Resume>().is_none() {
        return;
    }
    let saved = match rl_engine::rl_save::load_run(world) {
        Ok(Some(s)) => s,
        Ok(None) | Err(_) => { /* log it, and start a fresh run: run `start` through `world.run_system_once(start)` */ return; }
    };
    world.insert_resource(Seed(saved.engine.seed));
    let registries = world.resource::<Registries>().clone();
    let mut queue = CommandQueue::default();
    prepare(&mut Commands::new(&mut queue, world), saved.engine.seed, &registries);
    queue.apply(world);
    saved.restore(world).unwrap_or_else(|e| panic!("the save could not be restored: {e}"));
    let player = /* the Player */;
    for upgrade in world.resource::<Taken>().0.clone() {
        crate::upgrades::apply(upgrade, player, world);
    }
    let deck = /* deck_of the player's OnMap */;
    world.resource_mut::<MessageLog>().notice(format!("Continuing on deck {deck}."), saved.turn());
    world.resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
}
```

Registered in `plugin.rs`: `app.add_systems(NewRun, crate::run::resume.after(crate::run::start).after(crate::mission::start))`.
`crate::testing::continued(text)`: `headless_without_foundry` plus `FoundryPlugin`, the title down, the text persisted to the memory backend's `SLOT`, `Resume` inserted, and settled.

- [ ] **Step 7: Registered for the game and the tests.** `crate::save::register(app)` in `FoundryPlugin::build`; `Saves::new(MemoryBackend::default())` in `testing::headless_without_foundry`; `Saves::platform_default("foundry")` in `main.rs` beside the other resources it inserts.

- [ ] **Step 8: Run the tests.**
Run: `cargo test -p foundry --lib`
Expected: the round trip and the rest pass. The ambiguity test names the pairs `SavePlugin`'s and `UnloadPlugin`'s systems and `resume` conflict in; order each through the engine's sets, or allow it with a reason that holds (an exclusive `Last` system in `EndOfFrame::Save` is ordered against the burial and the restart by the set; `forget_save` on `OnEnter(Over)` runs once per ending).

- [ ] **Step 9: Clippy, then the task's tests.**
Run: `cargo clippy --workspace --all-targets -- -D warnings && cargo test -p foundry`
Expected: clean; all pass; if the fingerprint tripwire moved, the stash and the arrival save roll nothing, so find why before re-baselining.

---

### Task 4: The title screen: Continue, and the confirm

**Files:**
- Modify: `examples/foundry/src/title.rs`
- Test: `examples/foundry/src/title.rs` tests module

**Interfaces:**
- Consumes: `crate::save::{SLOT, abandon}`, `crate::run::Resume` (Task 3).
- Produces: `Title { up, picked, save: SaveOnDisk, confirm: Option<bool> }`, `pub enum SaveOnDisk { None, Readable, Unreadable }`.

- [ ] **Step 1: Write the failing tests.**

```rust
/// Continue is taken only when there is a save this build can read.
#[test]
fn continue_is_offered_only_with_a_readable_save() { /* SaveOnDisk::None and Unreadable: step() never lands on Continue; Readable: it does, and it is the row picked at first */ }

/// New Game over a save asks first; No keeps the save and the title up.
#[test]
fn new_game_over_a_save_asks_and_no_keeps_it() { /* persist a save; title up; pick New Game; Enter; confirm is Some(false); Enter; the title is still up, the slot still exists */ }

/// Yes abandons it and starts a fresh run on deck one.
#[test]
fn new_game_over_a_save_and_yes_starts_fresh() { /* as above, Up to Yes, Enter; the slot is gone; the run is playing on deck one */ }

/// Continue resumes the saved run.
#[test]
fn continue_resumes_the_saved_run() { /* a run saved on deck two; a fresh app with the save persisted and the title up; Enter on Continue; the commando is on deck two */ }

/// A damaged save leaves Continue dim, and New Game still asks, so it can be cleared.
#[test]
fn a_damaged_save_leaves_continue_dim_and_new_game_clears_it() { /* persist "not a save"; SaveOnDisk::Unreadable; New Game asks; Yes clears it */ }
```

Each body written out in full, pressing keys through `ButtonInput<KeyCode>` the way `taking_up_the_offer_starts_the_run` does, and building its apps with `crate::testing::headless` with `Title::default()` reinserted and the save persisted before the first update.

- [ ] **Step 2: Run them to see them fail.**
Run: `cargo test -p foundry --lib title::`
Expected: compile errors for `SaveOnDisk` and `confirm`.

- [ ] **Step 3: What is on disk.** A `Startup` system in `TitlePlugin`, `look_for_save(world: &mut World)`, sets `Title::save` from `load_run`: `Ok(Some(_))` is `Readable`, `Ok(None)` is `None`, `Err(_)` is `Unreadable`; then sets `picked` to the first available row. `Choice::available(self, save: SaveOnDisk) -> bool` replaces `available(self)`, and `step` takes the save too.

- [ ] **Step 4: The confirm.** In `read_title_keys`: while `confirm` is `Some(yes)`, up and down (and left and right) flip it, Enter takes it and Escape is No; No closes the confirm with New Game picked; Yes closes it, lowers the title and queues `abandon` then `begin`. Enter on New Game opens the confirm with `Some(false)` when `save != SaveOnDisk::None`, and begins at once otherwise. Enter on Continue lowers the title, inserts `Resume` and queues `begin`.

- [ ] **Step 5: Drawing it.** `paint_menu`: while the confirm is up, the line "Abandon the run in progress?" in `Tones::TEXT` centred two rows under the last menu row, and under it "No" and "Yes" side by side, the picked one with the `>` mark in `Tones::TITLE` and the other in `Tones::TEXT`; Continue drawn in `Tones::TEXT` when available. The picture and the rows above do not move.

- [ ] **Step 6: Run the tests.**
Run: `cargo test -p foundry --lib`
Expected: all pass.

- [ ] **Step 7: Clippy, then the task's tests.**
Run: `cargo clippy -p foundry --all-targets -- -D warnings && cargo test -p foundry`
Expected: clean; all pass.

---

### Task 5: Docs, screenshots, and every check

**Files:**
- Modify: `CHANGELOG.md`, `docs/OVERVIEW.md`, `docs/PLAN.md`, `examples/foundry/DESIGN.md`, `examples/foundry/TODO.md`, `README.md` only if the saving bullet names games that save

- [ ] **Step 1: The words.** `CHANGELOG.md` under `Unreleased`: Foundry can be won by the lift out, and saves on the way out and on each deck, with Continue and the confirm on its title screen. `docs/OVERVIEW.md`: Foundry's section, its ending and its save. `examples/foundry/DESIGN.md`: saving marked **exists**; `TODO.md`: the Continue line gone. `docs/PLAN.md`: a progress-log entry quoting Nate: "let's do the lift out (it should be where the player came in the first floor at), and saving a game".

- [ ] **Step 2: In the game.** With `RL_CAPTURE` (see `crates/rl-render/src/capture.rs`), capture and read: the refused lift out on a new run (`enter .` then Enter on the lift); the title with Continue lit after a run was left (run once with keys that walk a few steps and then `\` `n` for the next deck, which saves on arrival; then a second run on the title); the confirm over New Game; and the continued run on deck two. Fix whatever looks off, and look at each image before moving on.

- [ ] **Step 3: Every check.**
Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace --no-fail-fast && scripts/check-tiers.sh && scripts/check-tiers.sh --wasm && scripts/check-overview.sh && scripts/check-guide.sh && python3 scripts/check-systems.py`
Expected: every command passes.
