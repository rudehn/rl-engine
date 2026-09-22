# Foundry: the full loop, implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a Foundry run a whole run: ten decks down setting four charges, back up with the foundry awake, out through the lift you came down on, or dead with a morgue file to show for it.

**Architecture:** Everything here is game-side, in `examples/foundry`. No engine crate changes, because the engine already has the parts: `RunOver`/`Outcome`/`Ending` for endings, `Morgue` for the summary, `ActionRefused` as the seam a game answers a failed `GoThrough` in, and `BandedTable` sampled at a band for depth. The four existing pieces that change shape are the deck builder (three decks to ten), the mission (one charge to four), the population planner (first arrival only to first arrival plus revisit at the deepest band) and the drop roller (a kind's table to a kind's table filtered by the deck).

**Tech Stack:** Rust 2024, Bevy, `rl-engine` workspace crates, RON content in `examples/foundry/assets/`, `criterion` unused here, tests are `#[cfg(test)]` modules in the file under test plus `examples/foundry/tests/fingerprint.rs`.

**Spec:** `docs/superpowers/specs/2026-09-21-foundry-full-run-design.md`. The plan argues from the spec; read both. This plan covers spec slices 1 to 4 only. Spec slices 5 to 8 (unified noise loudness, the title screen, save and resume, the upgrade pool) are independent subsystems and get their own plans, per the scope rule that each plan must produce working, testable software on its own. After Task 10 here, a run plays from deck one to a won or lost ending.

## Global Constraints

Copied from `CLAUDE.md`; every task's requirements implicitly include these.

- Every workspace member declares `tier` under `[package.metadata.rl-engine]`. Tier 0 and 1 crates must not depend on Bevy, and no crate depends on a higher tier. `scripts/check-tiers.sh` checks it. No crate changes tier in this plan.
- `#![deny(missing_docs)]` on every crate. Every new public item needs a doc comment. Doc-tests compile and run; never fence an example as `ignore`.
- `cargo fmt --all --check` must pass. `rustfmt.toml` pins the width, so do not hand-wrap; let `cargo fmt` do it.
- `cargo clippy --workspace --all-targets -- -D warnings` must pass. Do not add a crate-wide `allow` for `too_many_arguments`; split the system instead.
- No `HashMap` or `HashSet` in gameplay or generation paths. `BTreeMap`, `Vec` or a `BitGrid`, with the reason stated.
- No `TODO` comments in source. Outstanding work goes in `docs/TODO.md` or the plan's progress log.
- Randomness comes through `Seed::stream(domain, index)`. Never construct a generator from a constant or from entropy in game code. Never draw a game's roll from the engine's combat stream.
- Doc comments say why and why-not, at the density of `crates/rl-core/src/turn.rs`, not more.
- Test names read as a sentence describing the property. Property-over-seed-range where a property exists; fingerprint tripwires labelled as such where none does.
- Every RON schema carries a top-of-file comment listing the full option space. If a task adds a field, it adds the line documenting it.
- Prose style: plain dash, never an em dash. American spelling in identifiers.
- `docs/OVERVIEW.md` is the inventory of what the engine has. No task here adds or removes an engine system, so no task here touches it. `examples/foundry/DESIGN.md` status marks move as tasks land, named per task below.
- Costs and clocks are integers, hundredths of a step, the same unit everywhere.

## A note on the fingerprint tripwire

`examples/foundry/tests/fingerprint.rs` asserts a scripted 200-turn run on seed 7 comes to one number.

Task 1 moves it: the spawn bands change what decks 1 to 3 hold, which is all the scripted run sees. Task 7 may move it, since filtering a drop table before the roll changes how many rolls the drop stream spends. No other task here should move it, and each task below says what to expect.

When a task says to re-baseline it:

1. Run `cargo test -p foundry --test fingerprint` and read the `left` value from the failure.
2. Put that value in the `assert_eq!` at the bottom of `fingerprint_tripwire_a_scripted_two_hundred_turn_run_on_seed_seven_comes_to_the_same_run_every_time`, with underscores every three digits as the existing literal has.
3. Run it again and confirm it passes twice in a row (the test itself runs the seed twice).
4. Say so in `CHANGELOG.md`, in that task's commit, naming the old and new values. The test's own comment requires this.

Never re-baseline it in a task whose steps do not say to. If it moves in Task 2, 3, 4, 5, 6, 8, 9 or 10, something unintended changed, and that is a bug to find rather than a number to update.

## File structure

Files created:

- `examples/foundry/src/climb.rs` - the run's depth memory and the way out. Owns `Deepest`, the system that updates it, `LiftOut`, the system that plants it on deck one, and the system that answers a refused `GoThrough` on it with a win or a line. One file because these four things change together and are meaningless apart: all of them exist only because the run goes back up.

Files modified:

- `examples/foundry/src/decks.rs` - `DECKS`, the core prefab, the reactor stamped on three decks instead of one.
- `examples/foundry/src/lifts.rs` - `deck_line` for decks 4 to 10.
- `examples/foundry/src/mission.rs` - the console at every reactor mark; the pick opened by any charge quest.
- `examples/foundry/src/upgrades.rs` - `Choosing` holds what is offered rather than always three; `Taken` remembers what the run has; the placeholder `RunOver::won()` goes.
- `examples/foundry/src/droids/spawns.rs` - the revisit branch.
- `examples/foundry/src/loot.rs` - the deck-banded drop filter.
- `examples/foundry/src/plugin.rs` - registers the new systems.
- `examples/foundry/src/lib.rs` - declares the new module.
- `examples/foundry/src/main.rs` - inserts the `Morgue`; the menu's ending lines.
- `examples/foundry/assets/monsters.ron` - spawn bands to deck 10.
- `examples/foundry/assets/quests.ron` - four tasks.
- `examples/foundry/DESIGN.md`, `CHANGELOG.md` - as each task lands.

`climb.rs` is the only new file. Everything else is a change to the module that already owns the concern, which is why there is no restructuring in this plan.

---

### Task 1: Ten decks

**Files:**
- Modify: `examples/foundry/src/decks.rs:20` (`DECKS`), `:115-129` (`reactor`), `:153-157` (the chain), `:175-196` (two tests)
- Modify: `examples/foundry/assets/monsters.ron` (spawn bands)
- Modify: `examples/foundry/src/lifts.rs:22-30` (`deck_line`)

**Interfaces:**
- Consumes: nothing.
- Produces: `crate::decks::DECKS == 10`. Decks 3, 6, 9 and 10 each report exactly one `Spot` with `tag == 'R' as u32`. `Foundry::build` succeeds for `map_of(1..=10)`. Every deck 1 to 10 has a non-empty band in `Roster::table`.

- [ ] **Step 1: Change the two tests to the ten-deck properties**

In `examples/foundry/src/decks.rs`, replace `only_deck_three_holds_the_reactor_and_it_holds_exactly_one` with the property for four charge decks. Keep the other test's body; it already loops `1..=DECKS`.

```rust
    /// The charge decks, and only they, hold a reactor: three, six and
    /// nine, and the core on ten. One each, since two consoles on a deck
    /// would let one run set the same charge twice.
    #[test]
    fn only_the_charge_decks_hold_a_reactor_and_each_holds_exactly_one() {
        for s in 0..20 {
            let foundry = Foundry::new(RunSeed(s));
            for deck in 1..=DECKS {
                let built = foundry.build(map_of(deck), None).unwrap();
                let reactors = built.spots.iter().filter(|s| s.tag == 'R' as u32).count();
                assert_eq!(reactors, usize::from(matches!(deck, 3 | 6 | 9 | 10)), "deck {deck}, seed {s}");
            }
        }
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p foundry --lib decks::`
Expected: FAIL. `only_the_charge_decks_hold_a_reactor_and_each_holds_exactly_one` fails at deck 6 with `0 != 1`, because `DECKS` is still 3 so the loop never reaches 6, or, once `DECKS` is 10, because only deck 3 stamps one.

- [ ] **Step 3: Raise `DECKS` and stamp the reactor on three decks**

In `examples/foundry/src/decks.rs`, change the constant and its doc:

```rust
/// How many decks the foundry has. Charges are set on three, six and
/// nine, and on the core on ten.
pub const DECKS: u32 = 10;
```

In `Foundry::generate`, replace the `if deck == 3` block:

```rust
        // Decks three, six and nine each feed a section of the plant, and
        // deck ten is the core: one reactor chamber on each, so the four
        // charges have somewhere to be set. The core is its own shape and
        // marks `R` like the others, so one system plants all four
        // consoles.
        chain = match deck {
            3 | 6 | 9 => chain.then(StampPrefab { name: "reactor", prefab: self.reactor()?, at: Placement::AnyRoom, orient: Orient::Fixed }),
            DECKS => chain.then(StampPrefab { name: "core", prefab: self.core()?, at: Placement::AnyRoom, orient: Orient::Fixed }),
            _ => chain,
        };
```

Note the existing line is `let mut chain = Chain::new()...;` followed by `if deck == 3 { chain = chain.then(...) }`. Keep `chain` mutable and assign the `match` to it.

- [ ] **Step 4: Add the core prefab**

Beside `reactor` in `examples/foundry/src/decks.rs`:

```rust
    /// The core chamber, deck ten only: the same five by five as a
    /// reactor so the room-size guarantee in `generate` still holds, with
    /// machinery down both sides rather than one, which is the whole of
    /// how the core reads as bigger without needing a bigger room.
    /// Marks `R`, since what stands on it is a console like the other
    /// three and nothing in the mission distinguishes them.
    fn core(&self) -> Result<Prefab, BuildError> {
        let (bulkhead, console, hatch) = (self.bulkhead, self.console, self.hatch);
        let legend = |c: char| match c {
            '#' => Some(bulkhead),
            'c' => Some(console),
            'h' => Some(hatch),
            _ => None,
        };
        Prefab::parse(&["#####", "#ccc#", "#.R.#", "#c.c#", "##h##"], legend).map_err(|e| BuildError::new("core", e))
    }
```

- [ ] **Step 5: Run the deck tests**

Run: `cargo test -p foundry --lib decks::`
Expected: PASS.

If `every_deck_builds_over_a_span_of_seeds_with_an_entry_and_an_exit` takes longer than about thirty seconds, lower its seed span from `0..300` to `0..100`. At ten decks that is 1000 builds against the 900 it did at three decks, so coverage does not drop. Say so in the test's comment if you change it.

- [ ] **Step 6: Extend the monster spawn bands to deck ten**

In `examples/foundry/assets/monsters.ron`, the bands stop at deck 8, so decks 9 and 10 would hold nothing at all. Extend the existing four kinds; add no new kind, which is content and out of scope.

- `line droid`: `spawn: [(1, 2, 100, 1, 2), (2, 4, 90, 2, 3)]` becomes `spawn: [(1, 2, 100, 1, 2), (2, 4, 90, 2, 3), (4, 10, 60, 2, 4)]`
- `probe droid`: `spawn: [(1, 6, 20, 1, 1)]` becomes `spawn: [(1, 10, 20, 1, 1)]`
- `heavy droid`: `spawn: [(3, 8, 40, 1, 1)]` becomes `spawn: [(3, 8, 40, 1, 1), (8, 10, 60, 1, 2)]`
- `coolant rat`: `spawn: [(1, 7, 50, 1, 3)]` becomes `spawn: [(1, 10, 50, 1, 3)]`

- [ ] **Step 7: Widen the roster's gap test**

In `examples/foundry/src/droids.rs`, `every_deck_in_the_slice_has_something_to_spawn` asserts `roster.table.gaps(1..=3).is_empty()`. Change it to the full run and rename it, since "the slice" is no longer three decks:

```rust
    #[test]
    fn every_deck_of_the_run_has_something_to_spawn() {
        let roster = Roster::load(&crate::content::registries());
        assert!(roster.table.gaps(1..=crate::decks::DECKS as i32).is_empty());
    }
```

- [ ] **Step 8: Run it**

Run: `cargo test -p foundry --lib droids::every_deck_of_the_run`
Expected: PASS.

- [ ] **Step 9: Name decks 4 to 10 in the log**

In `examples/foundry/src/lifts.rs`, `deck_line` names decks 1, 2 and `DECKS`. With `DECKS` now 10 the `DECKS =>` arm claims deck ten, and decks 3 to 9 fall to the bare `n => format!("Deck {n}.")`. Replace the whole function body:

```rust
pub fn deck_line(deck: u32) -> String {
    match deck {
        1 => "Deck 1: the upper assembly hall. The work lights are still on.".to_string(),
        2 => "Deck 2: the lower assembly hall. The lights are out down here.".to_string(),
        3 => "Deck 3: the first reactor deck, dark. Find the console and set the charge.".to_string(),
        4 | 5 => format!("Deck {deck}: fabrication. Furnace glow, and the air is worse."),
        6 => "Deck 6: the second reactor deck.".to_string(),
        7 | 8 => format!("Deck {deck}: the reactor ring. Nothing down here was built for people."),
        9 => "Deck 9: the third reactor deck.".to_string(),
        DECKS => "Deck 10: the core.".to_string(),
        n => format!("Deck {n}."),
    }
}
```

The `n` arm is now unreachable for `1..=10` but stays, because `deck_line` takes a `u32` and a caller can pass anything.

- [ ] **Step 10: Check the deck-line test still holds**

Run: `cargo test -p foundry --lib lifts::`
Expected: PASS. If a test asserts on deck three's old wording, update the expected string to the new line.

- [ ] **Step 11: Run the whole crate and re-baseline the fingerprint**

Run: `cargo test -p foundry`
Expected: every test passes except the fingerprint, which fails because the line droid's and the rat's bands changed weight on decks 1 to 3, so the scripted run's population differs. Re-baseline it by the procedure at the top of this plan.

- [ ] **Step 12: Update DESIGN.md**

In `examples/foundry/DESIGN.md`, the Progression section says "Decks 1-3, the assembly zone, **exist**; decks 4-10 are **planned**." Change it to say all ten decks are built and that the zones are not:

```markdown
All ten decks **exist** and are played; the four zones are **planned**, so every deck is still built as an assembly hall and they differ only in population and depth.
```

- [ ] **Step 13: Commit**

```bash
git add examples/foundry/src/decks.rs examples/foundry/src/droids.rs examples/foundry/src/lifts.rs examples/foundry/assets/monsters.ron examples/foundry/tests/fingerprint.rs examples/foundry/DESIGN.md CHANGELOG.md
git commit -m "foundry: ten decks, with a reactor on three, six and nine and the core on ten"
```

---

### Task 2: A console at every reactor

**Files:**
- Modify: `examples/foundry/src/mission.rs:121-142` (`spawn_console_on_arrival` and its doc)

**Interfaces:**
- Consumes: `crate::decks::DECKS` from Task 1, and the `R` spots decks 3, 6, 9 and 10 report.
- Produces: a `reactor console` prop standing on the `R` mark of every charge deck, on that deck's first arrival.

- [ ] **Step 1: Write the failing test**

In `examples/foundry/src/mission.rs`'s test module:

```rust
    /// Every charge deck gets its console on the arrival that builds it,
    /// which is what makes one system serve four reactors instead of the
    /// one deck three used to name.
    #[test]
    fn every_charge_deck_stands_a_console_on_its_reactor_mark_over_a_span_of_seeds() {
        for s in 0..4u64 {
            for deck in [3u32, 6, 9, 10] {
                let mut app = crate::testing::headless(RunSeed(s));
                crate::testing::arrive_on(&mut app, deck);
                let world = app.world_mut();
                let mut q = world.query_filtered::<&OnMap, With<PropKind>>();
                let here = crate::decks::map_of(deck);
                let consoles = q.iter(world).filter(|on| on.0 == here).count();
                assert_eq!(consoles, 1, "seed {s}, deck {deck}: one console");
            }
        }
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p foundry --lib mission::tests::every_charge_deck_stands_a_console`
Expected: FAIL at deck 6 with `0 != 1`, because the system still tests `deck_of(ev.map) != 3`.

- [ ] **Step 3: Stop naming deck three**

In `examples/foundry/src/mission.rs`, replace the guard in `spawn_console_on_arrival`:

```rust
    for ev in entered.read() {
        if !ev.first {
            continue;
        }
```

and delete the `crate::decks::deck_of(ev.map) != 3` clause. The `place.spots.iter().find(|s| s.tag == 'R' as u32)` below already leaves a deck with no reactor mark alone, so the deck number never needs testing: the map says whether a deck has a reactor, and only the charge decks stamp one.

Update the doc comment's first line, which says "deck three's `R` mark":

```rust
/// Puts a reactor console on the `R` mark of any deck that reports one,
/// the moment it is first entered, beside the loot
/// `loot::scatter_on_arrival` plants on the same arrival: reads the same
/// [`PlaceEntered`] the way that system and `droids::populate_deck` do,
/// and is unordered against both, since none of the three ever shares a
/// tile-claiming concern with either of the others.
///
/// Which decks have one is the builder's business, not this system's: it
/// asks the map rather than the deck number, so adding or moving a
/// reactor is a change to `decks.rs` alone.
```

Also fix the module doc at the top of the file, which says `[`spawn_console_on_arrival`] plants [`Console`] at deck three's `R` mark`.

- [ ] **Step 4: Run it to verify it passes**

Run: `cargo test -p foundry --lib mission::`
Expected: PASS, all of them.

- [ ] **Step 5: Run the crate**

Run: `cargo test -p foundry`
Expected: PASS. The fingerprint must not move: this task changes nothing on decks 1 to 3, which is all the scripted run visits.

- [ ] **Step 6: Commit**

```bash
git add examples/foundry/src/mission.rs
git commit -m "foundry: a console stands on whatever deck reports a reactor mark"
```

---

### Task 3: Four charges

**Files:**
- Modify: `examples/foundry/assets/quests.ron`
- Modify: `examples/foundry/src/mission.rs:190-209` (`offer_the_pick`)

**Interfaces:**
- Consumes: the consoles from Task 2.
- Produces: quests named `first_charge`, `second_charge`, `third_charge`, `core_charge`, chained by `after`. Each raises `Change::QuestDone` when its own charge is set, and each opens a pick.

- [ ] **Step 1: Write the four tasks**

Replace the list in `examples/foundry/assets/quests.ron`, keeping the header comment and updating the `victory` line in it:

```ron
//   victory:    optional; finishing this wins the run. No charge wins it: the run is won
//               by reaching deck one's lift out with the core charge set, which `climb.rs`
//               answers, so every task here is false.
#![enable(implicit_some)]
[
    (
        name: "first_charge",
        title: "The first reactor",
        text: "The reactor on the third deck feeds the assembly lines above it. Set a charge on its console.",
        objectives: [(text: "Set a charge on the reactor", on: ChargeSet(3), need: Total(1))],
    ),
    (
        name: "second_charge",
        title: "The second reactor",
        text: "Fabrication runs on the sixth deck's reactor. Three decks down, and darker.",
        after: ["first_charge"],
        objectives: [(text: "Set a charge on the reactor", on: ChargeSet(6), need: Total(1))],
    ),
    (
        name: "third_charge",
        title: "The third reactor",
        text: "The reactor ring's own feed, on the ninth deck. Below it there is only the core.",
        after: ["second_charge"],
        objectives: [(text: "Set a charge on the reactor", on: ChargeSet(9), need: Total(1))],
    ),
    (
        name: "core_charge",
        title: "The core",
        text: "The tenth deck. Set the last charge, then climb all ten decks back to the lift you came down on.",
        after: ["third_charge"],
        objectives: [(text: "Set a charge on the core", on: ChargeSet(10), need: Total(1))],
    ),
]
```

- [ ] **Step 2: Write the failing test**

In `examples/foundry/src/mission.rs`'s test module:

```rust
    /// The four charges are one chain: each opens only once the one above
    /// it is done, so a commando cannot charge the core first by riding
    /// the lifts straight down.
    #[test]
    fn the_core_charge_stays_shut_until_the_three_reactors_above_it_are_charged() {
        let mut app = crate::testing::headless(RunSeed(2));
        crate::testing::settle(&mut app);
        let quests = app.world().resource::<Quests>();
        assert_eq!(quests.tracker.state(quests.defs.expect("first_charge")), QuestState::Open, "the first charge is open from the start");
        assert_eq!(quests.tracker.state(quests.defs.expect("core_charge")), QuestState::Locked, "the core waits on the three above it");
    }
```

`Quests` carries both halves: `defs` names the quests and `tracker.state(id)` returns `QuestState::{Locked, Open, Done}`. `crate::testing::quest_done` in `examples/foundry/src/testing/mission.rs:58` reads it the same way.

- [ ] **Step 3: Run it to verify it fails**

Run: `cargo test -p foundry --lib mission::tests::the_core_charge_stays_shut`
Expected: FAIL. Before Step 1 lands, `core_charge` does not exist and `expect` panics.

- [ ] **Step 4: Open a pick on any charge, not only the first**

In `examples/foundry/src/mission.rs`, `offer_the_pick` matches `quest == first_charge`. `Change::QuestDone { quest, victory }` is raised per quest, so the chain already reports four times; the system just has to accept all four.

```rust
/// Reacts to the tracker's own report of a charge quest finishing: never
/// a mere `Progress`, and never a quest that is not one of the four
/// charges, so a later mission of any kind cannot open this pick. One
/// frame behind the fact that finished it (`rl_bevy::events`'s own doc on
/// [`QuestChange`]), which is why this is a plain `Update` system rather
/// than anything in `TurnSet`: nothing here is itself a reaction to a
/// turn, only to what the tracker made of one after the fact. Runs before
/// `EngineSet::Input`, so the key handlers find the pick open.
pub fn offer_the_pick(
    mut changes: MessageReader<QuestChange>,
    quests: Res<Quests>,
    mut modals: ResMut<Modals>,
    mut screen: ResMut<crate::upgrades::ChoiceScreen>,
    mut choosing: ResMut<crate::upgrades::Choosing>,
) {
    let charges = CHARGE_QUESTS.map(|name| quests.defs.expect(name));
    for change in changes.read() {
        if let Change::QuestDone { quest, .. } = change.0
            && charges.contains(&quest)
        {
            crate::upgrades::offer(&mut modals, &mut screen, &mut choosing);
        }
    }
}
```

and, beside `CHARGE` near the top of the file:

```rust
/// The four charge quests, in the order `quests.ron` chains them. Named
/// once here so the pick and the climb's own win condition read the same
/// list rather than each spelling it out.
pub const CHARGE_QUESTS: [&str; 4] = ["first_charge", "second_charge", "third_charge", "core_charge"];
```

This keeps `offer`'s current three-argument signature. Task 4 adds the fourth argument and comes back to this call, which is why that task and not this one owns the pool.

At the end of this task, four charges each open a pick offering the same three upgrades, and picking one applies it again. That is a real bug and Task 4 is the fix; it is left standing for one task because splitting the chain from the pool keeps each task's test honest about what it proves.

- [ ] **Step 5: Run it to verify it passes**

Run: `cargo test -p foundry --lib mission::`
Expected: PASS.

- [ ] **Step 6: Run the crate**

Run: `cargo test -p foundry`
Expected: PASS. The fingerprint must not move: the scripted run never reaches a console.

- [ ] **Step 7: Update DESIGN.md**

The Progression section says setting a charge "**exists** on deck 3". Change it to all four.

- [ ] **Step 8: Commit**

```bash
git add examples/foundry/assets/quests.ron examples/foundry/src/mission.rs examples/foundry/DESIGN.md
git commit -m "foundry: four charges, chained, each opening a pick"
```

---

### Task 4: The pick offers what is left

**Files:**
- Modify: `examples/foundry/src/upgrades.rs:39-50` (`Upgrade`, `OFFERED`), `:196-233` (`Choosing`, `offer`), `:240-271` (`choice_keys`)
- Modify: `examples/foundry/src/plugin.rs` (register `Taken`)

**Interfaces:**
- Consumes: `CHARGE_QUESTS` from Task 3.
- Produces: `pub struct Taken(pub Vec<Upgrade>)` as a resource; `Choosing(pub Option<Vec<Upgrade>>)`; `pub fn offer(&mut Modals, &mut ChoiceScreen, &mut Choosing, &Taken)`. `choice_keys` reads the offer out of `Choosing` rather than out of `OFFERED`, pushes the pick onto `Taken`, and no longer ends the run.

**Why:** four picks from a pool of three would offer the same three every time and apply a duplicate. Offering what is left makes four picks correct with the three upgrades that exist, and the pool can grow later without touching this logic. Spec section 5.2.

- [ ] **Step 1: Write the failing test**

In `examples/foundry/src/upgrades.rs`'s test module:

```rust
    /// A pick never offers what the run already has, so four charges
    /// across a run give four different upgrades rather than the same
    /// three over and over, and the pick after the pool runs dry offers
    /// nothing rather than a repeat.
    #[test]
    fn a_pick_offers_only_what_the_run_has_not_taken() {
        let all = Taken(Vec::new());
        assert_eq!(on_offer(&all).len(), 3, "nothing taken: the whole pool, capped at three");
        let two_left = Taken(vec![Upgrade::Stims]);
        let offer = on_offer(&two_left);
        assert!(!offer.contains(&Upgrade::Stims), "what is taken is not offered again");
        assert_eq!(offer.len(), 2);
        let empty = Taken(vec![Upgrade::Stims, Upgrade::Uplink, Upgrade::Servos]);
        assert!(on_offer(&empty).is_empty(), "a dry pool offers nothing");
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p foundry --lib upgrades::tests::a_pick_offers_only_what`
Expected: FAIL to compile: `Taken` and `on_offer` do not exist.

- [ ] **Step 3: Add `Taken` and `on_offer`**

In `examples/foundry/src/upgrades.rs`, beside `OFFERED`:

```rust
/// What this run has already fitted, in the order it was picked.
///
/// A resource rather than components read off the player, because what an
/// upgrade did to the player is not always something to read back: servos
/// raised a number that armor could raise too, and stims pushed an id
/// onto a list that an implant will push onto later. The pick needs to
/// know what it offered before, which is its own question.
#[derive(Resource, Debug, Clone, Default)]
pub struct Taken(pub Vec<Upgrade>);

/// The upgrades a pick offers given what the run has: the pool in
/// [`OFFERED`] order, minus what is taken, at most three rows because
/// three is what the screen and the design both say a pick is.
///
/// Empty is a real answer, not a bug: four charges against a pool of
/// three means the fourth pick has nothing to give, and a pick with no
/// rows closes itself rather than offering a repeat.
pub fn on_offer(taken: &Taken) -> Vec<Upgrade> {
    OFFERED.iter().copied().filter(|u| !taken.0.contains(u)).take(3).collect()
}
```

- [ ] **Step 4: Run it to verify it passes**

Run: `cargo test -p foundry --lib upgrades::tests::a_pick_offers_only_what`
Expected: PASS.

- [ ] **Step 5: Make `Choosing` hold the offer**

Change the declaration:

```rust
/// The upgrades on screen while a pick is open, or `None` while none is.
///
/// A `Vec` rather than `[Upgrade; 3]`: a pick late in a run offers fewer
/// than three, since it never offers what the run already has.
#[derive(Resource, Default)]
pub struct Choosing(pub Option<Vec<Upgrade>>);
```

Then `offer`:

```rust
/// Opens the pick on whatever is left to offer. A pick with nothing left
/// opens nothing, which is what the fourth charge does against a pool of
/// three: the charge still counts, and the screen does not appear to
/// offer a choice that is not there.
pub fn offer(modals: &mut Modals, screen: &mut ChoiceScreen, choosing: &mut Choosing, taken: &Taken) {
    let rows = on_offer(taken);
    if rows.is_empty() {
        return;
    }
    screen.menu.title = "The charge is set. Choose one upgrade".to_string();
    screen.menu.hints = "\u{2191}\u{2193} pick \u{2022} enter choose".to_string();
    screen.menu.set_rows(
        rows.iter()
            .map(|u| {
                let (name, text) = blurb(*u);
                MenuRow::new(name).detail(text)
            })
            .collect(),
    );
    choosing.0 = Some(rows);
    let id = modal(modals);
    modals.open(id);
}
```

- [ ] **Step 6: Read the pick out of `Choosing` and stop ending the run**

In `choice_keys`, replace the tail from `let selected = screen.menu.selected;`:

```rust
    let selected = screen.menu.selected;
    let Some(upgrade) = world.resource::<Choosing>().0.as_ref().and_then(|rows| rows.get(selected).copied()) else { return };
    let Some(player) = world.query_filtered::<Entity, With<Player>>().iter(world).next() else { return };
    apply(upgrade, player, world);
    world.resource_mut::<Taken>().0.push(upgrade);
    world.resource_mut::<Modals>().close_one(id);
    world.resource_mut::<Choosing>().0 = None;
```

The `RunOver::won()` write at the end goes. It was the first slice's placeholder ending; Task 9 owns the real one. Update `choice_keys`'s doc comment, which says "the run ends the moment one is made", to say the pick costs no turn and the run carries on.

- [ ] **Step 7: Pass `Taken` in from `offer_the_pick`**

`offer` now takes a fourth argument, so `mission::offer_the_pick` grows a `taken: Res<crate::upgrades::Taken>` parameter and passes `&taken`:

```rust
pub fn offer_the_pick(
    mut changes: MessageReader<QuestChange>,
    quests: Res<Quests>,
    mut modals: ResMut<Modals>,
    mut screen: ResMut<crate::upgrades::ChoiceScreen>,
    mut choosing: ResMut<crate::upgrades::Choosing>,
    taken: Res<crate::upgrades::Taken>,
) {
    let charges = CHARGE_QUESTS.map(|name| quests.defs.expect(name));
    for change in changes.read() {
        if let Change::QuestDone { quest, .. } = change.0
            && charges.contains(&quest)
        {
            crate::upgrades::offer(&mut modals, &mut screen, &mut choosing, &taken);
        }
    }
}
```

- [ ] **Step 8: Register `Taken`**

In `examples/foundry/src/plugin.rs`, beside where `Choosing` and `ChoiceScreen` are initialised, add `.init_resource::<crate::upgrades::Taken>()`.

- [ ] **Step 9: Fix the tests that asserted the old ending**

`examples/foundry/src/input.rs` has a test reading `Ending` and asserting `Outcome::Won` after a pick (around line 168 and 241). That behaviour is gone. Change the test to assert the run is still playing after a pick, and rename it to say so; Task 9 adds the test for the real victory.

- [ ] **Step 10: Run the crate**

Run: `cargo test -p foundry`
Expected: PASS. The fingerprint must not move.

- [ ] **Step 11: Commit**

```bash
git add examples/foundry/src/upgrades.rs examples/foundry/src/plugin.rs examples/foundry/src/input.rs examples/foundry/src/mission.rs
git commit -m "foundry: a pick offers what the run has not taken, and no longer ends it"
```

---

### Task 5: The run remembers its deepest deck

**Files:**
- Create: `examples/foundry/src/climb.rs`
- Modify: `examples/foundry/src/lib.rs` (declare the module)
- Modify: `examples/foundry/src/plugin.rs` (register the resource and system)

**Interfaces:**
- Consumes: `crate::decks::deck_of`, the engine's `PlaceEntered`.
- Produces: `pub struct Deepest(pub u32)` as a resource, starting at 1; `pub fn remember_depth(...)`, a system that raises it on arrival and never lowers it.

- [ ] **Step 1: Write the module with its failing test**

Create `examples/foundry/src/climb.rs`:

```rust
//! The way back out: how deep the run has been, and the lift it ends on.
//!
//! Four things that only exist because the run goes back up, and that are
//! meaningless apart. [`Deepest`] is the run's depth memory, which is what
//! the climb's population is drawn at rather than the deck's own band.
//! [`LiftOut`] is the lift on deck one the commando came down on, planted
//! with no `Transition`, so the engine refuses a `GoThrough` on it and
//! leaves the player its turn; [`answer_the_lift_out`] is the game
//! answering that refusal, which is the whole of the run's victory.

use bevy::prelude::*;
use rl_engine::prelude::*;

/// The deepest deck this run has stood on, one at the start.
///
/// The climb's difficulty is drawn at this band rather than at the deck's
/// own, the way NetHack's ascension run takes its monsters from the
/// deepest level reached: a deck one revisited after the core holds what
/// deck ten holds. Only ever raised, so walking back up does not walk it
/// back down.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deepest(pub u32);

impl Default for Deepest {
    fn default() -> Self {
        Self(1)
    }
}

/// Raises [`Deepest`] on every arrival that goes deeper than the run has
/// been. Reads `PlaceEntered` rather than the player's `OnMap`, since the
/// arrival is the event the rest of the deck's setup hangs off too.
pub fn remember_depth(mut entered: MessageReader<PlaceEntered>, mut deepest: ResMut<Deepest>) {
    for ev in entered.read() {
        deepest.0 = deepest.0.max(crate::decks::deck_of(ev.map));
    }
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_core::RunSeed;

    use super::*;

    /// Climbing back up does not make the run shallower: the climb is
    /// drawn at the deepest band, so a memory that fell back to the
    /// current deck would make the way up easier than the way down.
    #[test]
    fn the_run_remembers_its_deepest_deck_and_climbing_back_up_does_not_lower_it() {
        let mut app = crate::testing::headless(RunSeed(3));
        crate::testing::arrive_on(&mut app, 3);
        assert_eq!(app.world().resource::<Deepest>().0, 3);
        crate::testing::arrive_on(&mut app, 1);
        assert_eq!(app.world().resource::<Deepest>().0, 3, "deck one again, but the run has been to three");
    }
}
```

- [ ] **Step 2: Declare the module and register the system**

In `examples/foundry/src/lib.rs`, add `pub mod climb;` in alphabetical order among the other `pub mod` lines.

In `examples/foundry/src/plugin.rs`, add `.init_resource::<crate::climb::Deepest>()` and register `crate::climb::remember_depth`. It must run before the deck's population is planned, since Task 6 reads `Deepest` on the same arrival. Put it in the same set as the other arrival systems and order it before `crate::droids::spawns::populate_deck` by set membership or an explicit `.before(...)`, and say in a comment why: the depth memory has to be current before anything draws from it. Find how `populate_deck`, `scatter_on_arrival` and `spawn_console_on_arrival` are registered and follow that pattern.

- [ ] **Step 3: Run it to verify it passes**

Run: `cargo test -p foundry --lib climb::`
Expected: PASS.

If `arrive_on` does not accept a deck the run has already visited, read `examples/foundry/src/testing/loot.rs:14` and extend it or write the second arrival with a `WarpRequest` the way `revisiting_a_deck_scatters_nothing_new` in `loot.rs` does.

- [ ] **Step 4: Run the crate**

Run: `cargo test -p foundry`
Expected: PASS. The fingerprint must not move: nothing reads `Deepest` yet.

- [ ] **Step 5: Commit**

```bash
git add examples/foundry/src/climb.rs examples/foundry/src/lib.rs examples/foundry/src/plugin.rs
git commit -m "foundry: the run remembers the deepest deck it has stood on"
```

---

### Task 6: A revisited deck repopulates at the deepest band

**Files:**
- Modify: `examples/foundry/src/droids/spawns.rs:78-104` (`populate_deck` and its doc)

**Interfaces:**
- Consumes: `crate::climb::Deepest` from Task 5.
- Produces: a revisit spawns groups at `band = deepest` and `target = BASE_GROUPS + deepest * GROUPS_PER_DECK`. A first arrival is unchanged.

**Why:** spec section 3.2. One banded table sampled at a different band, which is how NetHack and DCSS both do the return trip.

- [ ] **Step 1: Write the failing test**

In `examples/foundry/src/droids/spawns.rs`'s test module:

```rust
    /// The climb is drawn at the deepest band the run reached, so a deck
    /// one revisited after the core holds what the core's neighbours hold
    /// rather than what deck one held on the way down. One table, sampled
    /// deeper, which is how NetHack's ascension run works.
    #[test]
    fn a_revisited_deck_repopulates_at_the_deepest_band_the_run_reached() {
        let mut app = crate::testing::headless(RunSeed(5));
        crate::testing::arrive_on(&mut app, 1);
        let shallow = crate::testing::monsters_on(&mut app, 1);
        crate::testing::arrive_on(&mut app, 9);
        crate::testing::arrive_on(&mut app, 1);
        let after = crate::testing::monsters_on(&mut app, 1);
        assert!(after > shallow, "deck one after deck nine holds more than deck one did: {shallow} then {after}");
    }
```

`monsters_on(&mut App, deck) -> usize` does not exist. Add it to `examples/foundry/src/testing/droids.rs`:

```rust
/// How many living monsters stand on `deck`, for a test about a deck's
/// population rather than about any one of them.
pub fn monsters_on(app: &mut App, deck: u32) -> usize {
    let here = crate::decks::map_of(deck);
    let world = app.world_mut();
    let mut q = world.query_filtered::<&OnMap, (With<Mind>, Without<Dead>)>();
    q.iter(world).filter(|on| on.0 == here).count()
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p foundry --lib spawns::tests::a_revisited_deck_repopulates`
Expected: FAIL. `after` equals whatever survived from the first visit, because `populate_deck` returns early on `!ev.first`.

- [ ] **Step 3: Add the revisit branch**

In `examples/foundry/src/droids/spawns.rs`, `populate_deck` currently does `if !ev.first { continue; }`. Replace that and the two lines that derive the band and target:

```rust
pub fn populate_deck(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, stock: Stock, deepest: Res<crate::climb::Deepest>) {
    let Stock { roster, map, seed, registries } = &stock;
    for ev in entered.read() {
        let Some(place) = map.place(ev.map) else { continue };
        let deck = crate::decks::deck_of(ev.map);
        // A first arrival is the deck's own band. A revisit is the climb,
        // and the climb is drawn at the deepest band the run reached, so
        // the way up is the harder half rather than a walk through decks
        // the commando already emptied.
        let band = if ev.first { deck } else { deepest.0 };
        let mut rng = seed.stream(if ev.first { b"foundry.spawns" } else { b"foundry.climb" }, u64::from(deck));
        let bounds = place.terrain.bounds();
        let target = BASE_GROUPS + band * GROUPS_PER_DECK;
        let groups = plan_population(&roster.table, band as i32, bounds, ev.entry, target, &mut |p| map.is_walkable(p), &mut rng);
        for (id, p) in groups.into_iter().flatten() {
            spawn_monster(&mut commands, roster, id, p, ev.map, registries);
        }
    }
}
```

The stream domain differs for the climb, so a deck's second population is not its first one over again. Note `rng` is derived fresh per arrival from the deck, which means a deck revisited twice gets the same groups twice; that is acceptable and deliberate, because a run has no reason to bounce and the alternative is a persistent generator per deck. Say that in the doc comment.

Update the doc comment, which currently says "A revisit is not a first arrival, so `PlaceEntered::first` being false leaves it alone: nobody new."

- [ ] **Step 4: Run it to verify it passes**

Run: `cargo test -p foundry --lib spawns::`
Expected: PASS.

- [ ] **Step 5: Run the crate and re-baseline the fingerprint**

Run: `cargo test -p foundry`
Expected: PASS, and think before touching the fingerprint.

`u64::from(deck)` and `deck as u64` are the same value for a `u32`, and gaining a `Res` parameter changes no roll, so this task moves the fingerprint only if the scripted run actually revisits a deck. Read `examples/foundry/tests/fingerprint.rs`: the script walks a square and shoots, and never takes a lift, so it should not revisit and the number should hold.

If it moves anyway, do not re-baseline yet. Something is entering a deck a second time that you did not expect, and finding out what is worth more than the number.

- [ ] **Step 6: Update DESIGN.md**

The Progression section's climb bullets say every deck is repopulated at the deepest band. Mark that part **exists**; the bigger groups and hunters-anywhere bullets stay **planned**.

- [ ] **Step 7: Commit**

```bash
git add examples/foundry/src/droids/spawns.rs examples/foundry/src/testing/droids.rs examples/foundry/tests/fingerprint.rs examples/foundry/DESIGN.md CHANGELOG.md
git commit -m "foundry: a revisited deck repopulates at the deepest band the run reached"
```

---

### Task 7: A deck drops only its own band's loot

**Files:**
- Modify: `examples/foundry/src/loot.rs:199-221` (`drop_on_death` and its doc)

**Interfaces:**
- Consumes: nothing from earlier tasks; `ItemDef::spawn` is `Option<(i32, i32, u32)>`, already on the struct.
- Produces: a drop entry whose item declares a `spawn` band is kept only when the deck lies inside it. An item with no band always drops.

**Why:** spec section 3.3. This is what makes Task 6 safe. Without it, bouncing between two decks farms deepest-band drops at no reward risk, which is the loop DCSS deleted in 0.6.0.

- [ ] **Step 1: Write the failing test**

In `examples/foundry/src/loot.rs`'s test module:

```rust
    /// A deck pays its own band's loot, whatever died on it. The climb
    /// puts deep droids on shallow decks, and a deep droid's gear on deck
    /// one would make walking up and down a way to farm: deepest-band
    /// danger for deepest-band reward, on a deck whose own loot was
    /// already taken.
    #[test]
    fn a_deck_drops_only_what_its_own_band_could_have_scattered() {
        let registries = crate::content::registries();
        let armory = Armory::load(&registries);
        let carbine = armory.defs.expect("blaster carbine");
        let plate = armory.defs.expect("composite plate");
        // The heavy droid's whole table, both entries certain.
        let table = [(carbine, 100), (plate, 100)];
        // Deck one: the carbine bands at 3-10 and the plate at 2-10, so a
        // heavy droid killed up here leaves nothing.
        assert!(banded_for(&armory, &table, 1).is_empty());
        // Deck three: both are in band.
        assert_eq!(banded_for(&armory, &table, 3).len(), 2);
        // Deck two: the plate only.
        assert_eq!(banded_for(&armory, &table, 2), vec![(plate, 100)]);
    }

    /// An item that lies on no deck is not depth-gated, it is simply not
    /// of the decks: hunter's plate is designed to drop from bounty
    /// hunters and to be found nowhere, so a band filter that dropped it
    /// would delete it from the game.
    #[test]
    fn an_item_with_no_spawn_band_drops_wherever_it_is_carried() {
        let registries = crate::content::registries();
        let armory = Armory::load(&registries);
        // Every item in the file bands today, so this asserts the rule
        // against a synthetic id rather than against content that would
        // have to be added to test it.
        let unbanded = armory.defs.iter().find(|(_, d)| d.spawn.is_none()).map(|(id, _)| id);
        if let Some(id) = unbanded {
            assert_eq!(banded_for(&armory, &[(id, 100)], 1).len(), 1);
        }
    }
```

The second test is conditional because no item in `items.ron` lacks a band yet. Rather than leave it vacuous, add the assertion for the rule directly against `banded_for`'s contract by giving the function a unit test of its own if `unbanded` is `None`: assert that `banded_for` keeps an entry whose def has `spawn: None`. If that needs a constructed `ItemDef`, build one in the test.

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p foundry --lib loot::tests::a_deck_drops_only`
Expected: FAIL to compile: `banded_for` does not exist.

- [ ] **Step 3: Add the filter**

In `examples/foundry/src/loot.rs`, beside `roll_drops`:

```rust
/// The entries of `drops` a deck can actually pay out: an item that
/// declares a `spawn` band drops only inside it, and an item with no band
/// drops anywhere.
///
/// The second half is the half that makes it correct rather than a
/// special case. A band says where an item lies on the decks; an item
/// with none does not lie on the decks at all, which is what hunter's
/// plate is for, and it must still drop wherever the hunter carrying it
/// dies.
///
/// Filtered before the roll, not after, so a drop the deck cannot pay
/// does not consume a roll and shift every later kill's luck.
pub fn banded_for(armory: &Armory, drops: &[(Id<ItemDef>, u32)], deck: u32) -> Vec<(Id<ItemDef>, u32)> {
    drops
        .iter()
        .copied()
        .filter(|(id, _)| match armory.defs.get(*id).spawn {
            Some((min, max, _)) => (min..=max).contains(&(deck as i32)),
            None => true,
        })
        .collect()
}
```

- [ ] **Step 4: Use it in `drop_on_death`**

In `drop_on_death`, between building `table` and rolling it:

```rust
        let armory = armory.get_or_insert_with(|| Armory::load(&registries));
        let table: Vec<(Id<ItemDef>, u32)> = def.drops.iter().map(|(name, pct)| (armory.defs.expect(name), *pct)).collect();
        let deck = crate::decks::deck_of(on_map.0);
        let table = banded_for(armory, &table, deck);
        for id in roll_drops(&table, &mut drops.0) {
```

`deck_of` is already imported at the top of the file. Add to `drop_on_death`'s doc comment why the deck matters:

```rust
/// What it pays is filtered to the deck's own band first ([`banded_for`]):
/// the climb puts deep droids on shallow decks, and a deck that paid a
/// deep droid's gear would make walking up and down worth doing.
```

- [ ] **Step 5: Run it to verify it passes**

Run: `cargo test -p foundry --lib loot::`
Expected: PASS.

- [ ] **Step 6: Run the crate and re-baseline the fingerprint**

Run: `cargo test -p foundry`
Expected: the fingerprint may move, since filtering before the roll changes how many rolls the drop stream spends. If it moves, re-baseline; if it does not, do not touch it.

`kill_with_a_guaranteed_drop` in `examples/foundry/src/testing/loot.rs` may now kill something whose drop the deck filters out. If a test using it fails, it is asserting the old behaviour: make it kill on a deck in the item's band, and say in the helper's doc that the deck decides what can drop.

- [ ] **Step 7: Update DESIGN.md**

In the Progression section, "killing a droid is worth what it drops and nothing more" gains the band rule, since it is now part of the design rather than an implementation detail:

```markdown
What a kill drops is the deck's own band, never the band the commando has reached, so a droid killed on a deck above its own leaves nothing and the climb is danger without reward.
```

- [ ] **Step 8: Commit**

```bash
git add examples/foundry/src/loot.rs examples/foundry/src/testing/loot.rs examples/foundry/DESIGN.md CHANGELOG.md
git commit -m "foundry: a deck drops only its own band's loot, so the climb cannot be farmed"
```

---

### Task 8: The lift out on deck one

**Files:**
- Modify: `examples/foundry/src/climb.rs`
- Modify: `examples/foundry/src/plugin.rs` (register the system)

**Interfaces:**
- Consumes: the engine's `PlaceEntered`, `Position`, `OnMap`, `Glyph`.
- Produces: `pub struct LiftOut` as a marker component, on one entity at deck one's entry, spawned on first arrival, carrying no `Transition`.

**Why:** spec sections 2.4 and 5.3. No `Transition` is the mechanism, not an omission: the engine's `resolve_warps` takes the player through whatever transition is under it, so an entity with none makes the engine write `ActionRefused` and leave the player its turn at no cost. Task 9 answers that refusal. The alternative, adding the transition only once the charges are set, changes the map under the player.

- [ ] **Step 1: Write the failing test**

In `examples/foundry/src/climb.rs`'s test module:

```rust
    /// Deck one has the lift the commando came down on from the first
    /// turn, and it carries no `Transition`: the engine would otherwise
    /// take the player through it, and where it leads is the end of the
    /// run rather than another deck.
    #[test]
    fn deck_one_stands_the_lift_out_at_its_entry_with_nowhere_to_go() {
        let mut app = crate::testing::headless(RunSeed(1));
        crate::testing::settle(&mut app);
        let world = app.world_mut();
        let mut q = world.query_filtered::<(Entity, &Position, Has<Transition>), With<LiftOut>>();
        let found: Vec<(Entity, Point, bool)> = q.iter(world).map(|(e, p, t)| (e, p.0, t)).collect();
        assert_eq!(found.len(), 1, "one lift out");
        assert!(!found[0].2, "and it goes nowhere, so a GoThrough on it is refused");
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p foundry --lib climb::tests::deck_one_stands_the_lift_out`
Expected: FAIL to compile: `LiftOut` does not exist.

- [ ] **Step 3: Add `LiftOut` and the system that plants it**

In `examples/foundry/src/climb.rs`:

```rust
/// The lift on deck one the commando came down on: the run's way out.
///
/// Carries no `Transition`, deliberately. The engine's own warp resolver
/// takes the player through whatever transition it stands on, and this
/// one does not lead to a deck, it ends the run. With none, a `GoThrough`
/// on this cell is refused, the player keeps its turn at no cost, and
/// [`answer_the_lift_out`] decides what the refusal meant.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct LiftOut;

/// What the lift out is drawn in: the same amber as the lifts between
/// decks, since it is the same machinery.
const LIFT_OUT: Color = Color::srgb(0.95, 0.8, 0.35);

/// Plants the lift out at deck one's entry, the first time deck one is
/// entered. Deck one alone, since the run began there and ends there.
pub fn plant_the_lift_out(mut commands: Commands, mut entered: MessageReader<PlaceEntered>) {
    for ev in entered.read() {
        if !ev.first || crate::decks::deck_of(ev.map) != 1 {
            continue;
        }
        commands.spawn((Position(ev.entry), OnMap(ev.map), LiftOut, Name::new("lift out"), Glyph::new('<', LIFT_OUT).on_layer(1)));
    }
}
```

`lifts::link_decks` spawns no lift up on deck one, so nothing else claims that cell. Check that while implementing: if `link_decks` changed, one of the two has to give way, and the lift out wins.

- [ ] **Step 4: Register it**

In `examples/foundry/src/plugin.rs`, register `crate::climb::plant_the_lift_out` beside `crate::lifts::link_decks`, in the same set and with the same ordering, since both react to the same arrival and neither reads the other's work.

- [ ] **Step 5: Run it to verify it passes**

Run: `cargo test -p foundry --lib climb::`
Expected: PASS.

- [ ] **Step 6: Run the crate**

Run: `cargo test -p foundry`
Expected: PASS. The fingerprint must not move: an entity with no `Blocks` and no `Actor` at the entry changes no roll and no order. If it does move, something about the new entity is being seen by the map view or the nearby panel in a way that feeds the fingerprint, and that is worth understanding before re-baselining.

- [ ] **Step 7: Commit**

```bash
git add examples/foundry/src/climb.rs examples/foundry/src/plugin.rs
git commit -m "foundry: deck one keeps the lift the commando came down on"
```

---

### Task 9: Victory, and the lift that will not move

**Files:**
- Modify: `examples/foundry/src/climb.rs`
- Modify: `examples/foundry/src/plugin.rs` (register the system in `TurnSet::React`)
- Modify: `examples/foundry/src/main.rs` (the menu's ending lines)

**Interfaces:**
- Consumes: `LiftOut` from Task 8, `CHARGE_QUESTS` from Task 3, `Facts` from `mission.rs`, the engine's `ActionRefused`, `RunOver`, `Tell`.
- Produces: `pub fn answer_the_lift_out(...)`, a `TurnSet::React` system. A refused action by a player standing on a `LiftOut` either wins the run or says why not.

**Why:** spec section 5.4.

- [ ] **Step 1: Write the two failing tests**

In `examples/foundry/src/climb.rs`'s test module:

```rust
    /// The lift will not move without the charges, and saying so costs
    /// nothing: the engine leaves a player its turn on a refused action,
    /// and a run should not be taxed for trying the door.
    #[test]
    fn the_lift_out_refuses_without_the_core_charge_and_costs_no_turn() {
        let mut app = crate::testing::headless(RunSeed(1));
        crate::testing::settle(&mut app);
        let player = crate::testing::on_the_lift_out(&mut app);
        let before = crate::testing::clock(&app);
        app.world_mut().write_message(Intent::new(player, GoThrough));
        crate::testing::settle(&mut app);
        assert_eq!(crate::testing::clock(&app), before, "trying the lift costs nothing");
        assert!(app.world().get_resource::<Ending>().is_none(), "and the run is not over");
    }

    /// The whole of the run's victory: the core charge set, and the
    /// commando back on the lift it came down on.
    #[test]
    fn the_lift_out_wins_the_run_once_the_core_charge_is_set() {
        let mut app = crate::testing::headless(RunSeed(1));
        crate::testing::settle(&mut app);
        let player = crate::testing::on_the_lift_out(&mut app);
        crate::testing::report_charge(&mut app, 10);
        crate::testing::settle(&mut app);
        app.world_mut().write_message(Intent::new(player, GoThrough));
        crate::testing::settle(&mut app);
        let ending = app.world().resource::<Ending>();
        assert_eq!(ending.outcome, Outcome::Won);
    }
```

Two helpers are needed in `examples/foundry/src/testing/mission.rs`:

```rust
/// Stands the player on deck one's lift out, for a test about what taking
/// it does. Returns the player.
pub fn on_the_lift_out(app: &mut App) -> Entity {
    let at = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<&Position, With<crate::climb::LiftOut>>();
        q.iter(world).next().expect("deck one stands a lift out").0
    };
    let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
    app.world_mut().get_mut::<Position>(player).unwrap().0 = at;
    player
}

/// Reports the charge on `deck` as set, without walking a commando to a
/// console: the fact is what the mission counts, so a test about what a
/// set charge unlocks does not have to play three decks first.
pub fn report_charge(app: &mut App, deck: u32) {
    let kind = app.world().resource::<crate::mission::Facts>().charge_set;
    app.world_mut().write_message(Happened(Fact::new(kind).about(u64::from(deck))));
}
```

Moving the player by writing `Position` sidesteps the spatial grid the engine keeps. That is acceptable here because the test never asks anything about occupancy, only about what a `GoThrough` on that cell does. If a later test needs the grid consistent, walk the player instead.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p foundry --lib climb::tests::the_lift_out`
Expected: FAIL to compile: `answer_the_lift_out` and the two helpers do not exist.

- [ ] **Step 3: Answer the refusal**

In `examples/foundry/src/climb.rs`:

```rust
/// What answering the lift out reads.
#[derive(bevy::ecs::system::SystemParam)]
pub struct LiftOutAnswer<'w, 's> {
    refusals: MessageReader<'w, 's, ActionRefused>,
    standing: Query<'w, 's, &'static Position, With<Player>>,
    lifts: Query<'w, 's, &'static Position, With<LiftOut>>,
    quests: Res<'w, Quests>,
    over: MessageWriter<'w, RunOver>,
    tell: MessageWriter<'w, Tell>,
}

/// Answers a refused action by a player standing on the lift out: with
/// the core charge set it wins the run, and without it says why the lift
/// will not move.
///
/// The engine refuses a `GoThrough` that finds no `Transition` under the
/// actor and leaves the player its turn at no cost, so this reads that
/// refusal rather than the intent. Reading the refusal rather than
/// claiming the intent first is what keeps this system from having to
/// order itself against `resolve_warps`, which is another crate's system
/// function and none of this game's business.
///
/// A refusal anywhere else is not this system's: the engine's own refusal
/// stands, and whatever else in the game reads refusals says its own
/// piece.
pub fn answer_the_lift_out(mut answer: LiftOutAnswer) {
    let refused = answer.refusals.read().count() > 0;
    if !refused {
        return;
    }
    let Ok(here) = answer.standing.single() else { return };
    if !answer.lifts.iter().any(|p| p.0 == here.0) {
        return;
    }
    let core = answer.quests.defs.expect("core_charge");
    if answer.quests.tracker.state(core) == QuestState::Done {
        answer.over.write(RunOver::won().saying("The lift takes you up. Ten decks below, the core is armed and waiting."));
    } else {
        answer.tell.write(Tell::new("The lift will not move. Not with the reactors still feeding the plant.", Tones::MUTED));
    }
}
```

`Quests` carries the tracker, so this needs no second resource: `quests.tracker.state(id) == QuestState::Done` is the same read `crate::testing::quest_done` does at `examples/foundry/src/testing/mission.rs:58`. Reading the quest rather than the fact is deliberate: the quest chain is what defines "the last charge", and reading the fact would duplicate that rule in a second place.

The `refusals.read().count() > 0` reads every refusal of the pass and asks only whether there was one, because a refusal names its actor and this system has already established which cell the player is on. If a later game system needs to tell refusals apart, filter on `actor` instead.

- [ ] **Step 4: Register it in `TurnSet::React`**

In `examples/foundry/src/plugin.rs`, register `crate::climb::answer_the_lift_out` in `TurnSet::React`. It reacts to what a turn caused, which is what `React` is for, and it must not go in a drawing phase.

- [ ] **Step 5: Run them to verify they pass**

Run: `cargo test -p foundry --lib climb::`
Expected: PASS.

- [ ] **Step 6: Say the right thing on the ending screen**

In `examples/foundry/src/main.rs`, `GameMenuPanel::new(screen.menu).title("Foundry").died("The foundry keeps you.").won("The first charge is set.")` still describes the first slice. Change the victory line:

```rust
        GameMenuPanel::new(screen.menu).title("Foundry").died("The foundry keeps you.").won("You climbed out. The foundry did not."),
```

- [ ] **Step 7: Run the crate**

Run: `cargo test -p foundry`
Expected: PASS. The fingerprint must not move: the scripted run never stands on the lift out with a charge set.

- [ ] **Step 8: Update DESIGN.md**

The Goals section's primary goal and the Losing section both describe this. Mark the primary goal **exists**.

- [ ] **Step 9: Commit**

```bash
git add examples/foundry/src/climb.rs examples/foundry/src/plugin.rs examples/foundry/src/main.rs examples/foundry/src/testing/mission.rs examples/foundry/DESIGN.md
git commit -m "foundry: the lift out wins the run, or says why it will not move"
```

---

### Task 10: The morgue

**Files:**
- Modify: `examples/foundry/src/main.rs` (insert the `Morgue`)
- Modify: `examples/foundry/src/climb.rs` (push Foundry's sections)
- Modify: `examples/foundry/src/plugin.rs` (register the system)

**Interfaces:**
- Consumes: `Deepest` from Task 5, `Taken` from Task 4, `CHARGE_QUESTS` and `Facts` from Task 3, the engine's `RunOver` and `Morgue`.
- Produces: a `Morgue` resource in the real binary, and a `TurnSet::React` system that pushes Foundry's own sections onto it in reaction to `RunOver`.

**Why:** spec section 5.4. `GameMenuPanel::run_ended` files an obituary only when the game inserted a `Morgue`, and Foundry never has, so no run has ever left a file. The engine supplies the header, the character sheet and the last of the log; the sections are the game's.

- [ ] **Step 1: Write the failing test**

In `examples/foundry/src/climb.rs`'s test module:

```rust
    /// A dead run leaves a summary worth reading: how deep it got, what
    /// it armed, and what it was carrying when it stopped. The engine
    /// writes the header and the last of the log; these are the four
    /// things only Foundry knows.
    #[test]
    fn a_run_that_ends_files_how_deep_it_got_and_what_it_armed() {
        let mut app = crate::testing::headless(RunSeed(1));
        crate::testing::settle(&mut app);
        app.insert_resource(Morgue::new(MemoryBackend::default(), "Foundry"));
        crate::testing::arrive_on(&mut app, 3);
        crate::testing::report_charge(&mut app, 3);
        crate::testing::settle(&mut app);
        app.world_mut().write_message(RunOver::died(None));
        crate::testing::settle(&mut app);
        let sections = app.world().resource::<Morgue>().take_sections();
        let headings: Vec<&str> = sections.iter().map(|(h, _)| h.as_str()).collect();
        assert!(headings.contains(&"The run"), "{headings:?}");
        let body = &sections.iter().find(|(h, _)| h == "The run").unwrap().1;
        assert!(body.contains("Deck 3"), "how deep it got: {body}");
        assert!(body.contains("1 of 4"), "what it armed: {body}");
    }
```

`MemoryBackend` is `Default` and saves in memory expressly "for tests and for a run that must not touch disk" (`crates/rl-save/src/backend.rs:125`). It is re-exported from `rl_save`, so the test imports `rl_engine::rl_save::{MemoryBackend, Morgue}`. No temp file, and nothing to clean up.

`take_sections` drains, so call it once and keep the result.

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p foundry --lib climb::tests::a_run_that_ends_files`
Expected: FAIL: no sections, because nothing pushes any.

- [ ] **Step 3: Push Foundry's sections**

In `examples/foundry/src/climb.rs`:

```rust
/// What the morgue's Foundry section reads.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Summary<'w, 's> {
    over: MessageReader<'w, 's, RunOver>,
    morgue: Option<ResMut<'w, Morgue>>,
    deepest: Res<'w, Deepest>,
    taken: Res<'w, crate::upgrades::Taken>,
    quests: Res<'w, Quests>,
    worn: Query<'w, 's, &'static Equipped, With<Player>>,
    names: Query<'w, 's, &'static Name>,
}

/// Pushes what only Foundry knows onto the morgue, in reaction to the run
/// ending: how deep it got, how many charges it set, what it fitted, and
/// what it had in its hands.
///
/// `Option<ResMut<Morgue>>` because the morgue is optional data, not an
/// optional subsystem: a headless test inserts none and a run still ends
/// properly, it simply leaves no file. The sections are pushed rather
/// than written to a file here, because `GameMenuPanel::run_ended` files
/// the obituary on the frame the run ends and takes whatever the game
/// left for it.
pub fn file_the_summary(mut summary: Summary) {
    if summary.over.read().next().is_none() {
        return;
    }
    let Some(morgue) = summary.morgue.as_deref_mut() else { return };
    let set = crate::mission::CHARGE_QUESTS
        .iter()
        .filter(|name| summary.quests.tracker.state(summary.quests.defs.expect(name)) == QuestState::Done)
        .count();
    let mut lines = vec![format!("Deck {} was the deepest.", summary.deepest.0), format!("Charges set: {set} of {}.", crate::mission::CHARGE_QUESTS.len())];
    lines.push(match summary.taken.0.as_slice() {
        [] => "Nothing fitted.".to_string(),
        picked => format!("Fitted: {}.", picked.iter().map(|u| crate::upgrades::name_of(*u)).collect::<Vec<_>>().join(", ")),
    });
    if let Ok(worn) = summary.worn.single() {
        let held: Vec<&str> = worn.0.worn().filter_map(|(_, item)| summary.names.get(item).ok()).map(Name::as_str).collect();
        lines.push(if held.is_empty() { "Empty handed.".to_string() } else { format!("Carrying: {}.", held.join(", ")) });
    }
    morgue.section("The run", lines.join("\n"));
}
```

`Morgue::section(&mut self, heading, body)` is the pushing half of `take_sections` (`crates/rl-save/src/morgue.rs:126`). Note `Obituary::section` at line 54 is a different, consuming builder method on a different type; the one wanted here takes `&mut self`.

`crate::upgrades::name_of` does not exist. `blurb` is private and returns a name and a pitch; add beside it:

```rust
/// An upgrade's name on its own, for anything that lists what a run
/// fitted rather than offering it.
pub fn name_of(upgrade: Upgrade) -> &'static str {
    blurb(upgrade).0
}
```

- [ ] **Step 4: Register it**

In `examples/foundry/src/plugin.rs`, register `crate::climb::file_the_summary` in `TurnSet::React`, before `answer_the_lift_out` is irrelevant but it must run in the same pass the `RunOver` is written in, and `end_runs` is a `PostUpdate` system, so a `React` system reading `RunOver` sees it in time. Verify that ordering holds by running the test in Step 5 rather than by reasoning about it: if the section never arrives, the read is happening after the message was dropped, and the system belongs in `Update` before `EngineSet::Turns` instead.

- [ ] **Step 5: Run it to verify it passes**

Run: `cargo test -p foundry --lib climb::`
Expected: PASS.

- [ ] **Step 6: Insert the morgue in the real binary**

In `examples/foundry/src/main.rs`, beside the other resources:

```rust
        .insert_resource(Morgue::platform_default("foundry", "Foundry"))
```

`platform_default` picks browser storage on wasm and a file elsewhere, which is what `crates/rl-save/src/morgue.rs:103` is for.

- [ ] **Step 7: Run the crate and the binary**

Run: `cargo test -p foundry`
Expected: PASS, fingerprint unmoved.

Run: `cargo run -p foundry` and die on purpose, then confirm the ending screen names the morgue file and that the file has a "The run" section in it. This is the one task in this plan whose result is only visible in the real binary, so do not skip it.

- [ ] **Step 8: Update DESIGN.md**

The Losing section says the morgue "**exists** in its general form". Change it to say Foundry files one, and name the four things in it.

- [ ] **Step 9: Commit**

```bash
git add examples/foundry/src/climb.rs examples/foundry/src/upgrades.rs examples/foundry/src/main.rs examples/foundry/src/plugin.rs examples/foundry/DESIGN.md CHANGELOG.md
git commit -m "foundry: a run that ends leaves a morgue file worth reading"
```

---

## When this plan is done

Run a real run and confirm the loop closes: `cargo run -p foundry`, then `FOUNDRY_START=9 cargo run -p foundry` to reach the core quickly and check the climb and the win without playing ten decks by hand.

Then add one integration test in `examples/foundry/tests/` that plays the whole loop scripted: down to deck ten setting four charges, back to deck one, onto the lift, `Outcome::Won`. It belongs in `tests/` rather than in a module because it crosses every system in the game, and it is the test that would have caught any of the ten tasks above regressing.

Write the `CHANGELOG.md` entry for the loop as a whole at that point, rather than ten separate ones: the per-task entries above are only for the fingerprint re-baselines, which have to be recorded in the commit that moves them.

## What this plan does not do

From the spec's section 7, and its slices 5 to 8, each of which gets its own plan:

- Unified noise loudness (`Footfall` to `Loudness`), which is an engine change and independent of everything here.
- The title screen, its ASCII art, and the engine's menu backdrop.
- Save and resume, and the menu's Continue row.
- The upgrade pool beyond the three that exist. Task 4 makes four picks correct against three upgrades; it does not add a fourth.
- Every zone, hazard, faction and monster kind the spec's section 7 lists.
