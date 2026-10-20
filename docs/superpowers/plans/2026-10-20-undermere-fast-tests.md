# Undermere's Tests Without the Play-Throughs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cut Undermere's test cost by giving each expensive test the cheapest form that still proves what it proves: a pure rule unit-tested without an `App`, a generation property checked on a built place, a staged state instead of a played one, one `App` instead of many, one seed where the property does not depend on the seed, and a rate computed from content instead of sampled.

**Architecture:** Phase 2 of the functional-core roadmap (`docs/superpowers/plans/2026-10-19-functional-core.md`).
The engine already lifted its rules below the line; this phase does the same for the game's tests, guided by AGENTS.md's "Writing fast tests".
Two small engine slices support it: a test helper that lets time pass without playing every turn, and the trade-test follow-up from phase 1.

**Tech Stack:** Rust 2024, Bevy 0.19, cargo-nextest, `scripts/slot` processor-time records, `scripts/test-times.py`.

**Spec:** This document's Measurement section; it records the measurement of 2026-10-20 and the per-test classification it rests on.

## Measurement

Measured on `main` at `55d1a759` with `cargo nextest run --workspace --profile full` and `scripts/test-times.py`.
The suite is 2561 tests and 683 s of processor time; Undermere is 561 tests and 485 s.

| Undermere tier | Tests | Processor time |
|---|---|---|
| Slow tier (branch end and CI) | 46 | 239 s |
| Fast tier (every commit) | 515 | 246 s, of which the scripted-delve fingerprint is 65 s |

The 42 slow-tier tests at 2 s or more (231 s, the fingerprint excluded) were each classified; their cheaper forms come to about 37 s, almost all of it fast enough for the fast tier.
Per-test setup is small: building a headless game `App` costs about 0.05 s of processor time measured alone, so the cost is in what tests generate and play, not in setup.
The fast tier's own expensive tests (about 30 tests at 1 s or more, outside the slow tier) are handled the same way in Task 10.

## Global Constraints

- A rewritten test proves the same property it proved before, or the task names where each dropped assertion now lives (a unit test, a static check, the generator backstop, or an existing test it duplicated). A test is never weakened to pass.
- AGENTS.md "Writing fast tests" is binding: `seeds(full)` for a property over seeds; `first_seeds(full, n)` with its reason for a count; a rare-thing search stops at the first hit and fails loudly if it never finds one; content read once (`Undermere::reseeded`); state set up, not played to; one grown backstop per game in the slow tier; `#[ignore]` only for by-hand spans and tuning aids.
- Behaviour-preserving: game code lifted into pure functions behaves exactly as before, so Undermere's and Foundry's fingerprint tests and rl-bevy's pass unchanged. A moved fingerprint means a rule changed: fix it, never re-baseline.
- Wherever this plan's description of the old code disagrees with the code, the code wins; say where in the report.
- Nothing is public only so a test can reach it. Test helpers live in `examples/undermere/src/testing/` or a test module; engine test helpers live in the engine's public `testing` modules, which games use.
- Test names read as sentences describing the property. Plain dash, never an em dash; every sentence of long Markdown on its own line.
- No `#[allow(clippy::too_many_arguments)]`; never set `CARGO_TARGET_DIR`; a build waiting for a slot is not hung.
- During a task, run the touched crates' tests and the fingerprint tests; `scripts/gate.sh` (the scoped fast gate) before committing. The full `scripts/gate.sh --all` runs once, in Task 11.
- One commit per task, message one long lowercase sentence saying what is now true, no attribution lines. A `CHANGELOG.md` line only for an engine public API change (Task 1).

## Review Focus

1. **A rare-thing search that never hits passes silently.** A search that stops at the first floor or seed holding something must `panic!` or `expect` when none is found, never fall through. Pinned in each task that adds one, by an `expect("...")` on the search's result.
2. **Letting time pass skips what ticks per turn.** A clock jump advances everything that reads elapsed time but not what ticks once per whole turn (statuses, mind decisions). Task 1's helper documents it and its tests show both halves; each later caller uses it only where the property is clock-based.
3. **One seed where the seed matters.** Dropping to one seed is right only where the setup is staged or the town is hand-drawn; a property of generated content keeps `seeds(full)`. Each task says why one seed is enough for each test it narrows.
4. **A fold into the backstop that loses coverage.** Folding a check into `every_place_of_a_run_keeps_every_promise_the_generator_makes_over_a_span_of_seeds` must keep every seed the old test played covered at `seeds(full)` daily and in CI. Pinned in Task 3.
5. **A rate replaced by arithmetic that no longer reads the content.** A computed rate must read the dice, health and percentages from the loaded content (`Armory`, `Bestiary`, spotting rules), so a content change still breaks the test. Pinned in Tasks 7 and 9.

---

### Task 1: Letting time pass in a test (engine slice)

**Files:**
- Modify: `crates/rl-bevy/src/testing.rs` (the helper and its tests)
- Modify: `crates/rl-engine/src/testing.rs` only if it re-exports rl-bevy's helpers by name
- Modify: `CHANGELOG.md`

**Interfaces:**
- Produces: `pub fn let_time_pass(app: &mut App, turns: u32)` in `rl_bevy::testing` (and so `rl_engine::testing`): advances the turn clock and every queued actor's due time by `turns * BASE_ACTION_COST`, then runs one frame so systems that read the elapsed time (recharge, regeneration, fading, familiarity) see the jump. It does not tick what counts whole turns.

- [ ] **Step 1: Write the failing tests** in `rl-bevy/src/testing.rs`'s test module:
  - `letting_time_pass_moves_the_clock_and_every_queued_actor_by_the_same_span`: with two actors queued at known times, after `let_time_pass(app, 50)` the clock is `5000` later and each entry's due time is `5000` later, in the same order.
  - `letting_time_pass_recharges_what_recharges_by_the_clock`: a `Consumable` with a `Recharge` of one charge per `2500` and one charge spent is full again after `let_time_pass(app, 25)` and not after `let_time_pass(app, 24)` on a fresh app.
- [ ] **Step 2: Run them to see them fail.** `cargo test -p rl-bevy letting_time_pass` fails to compile.
- [ ] **Step 3: Implement** `let_time_pass`, reading the turn queue's export and import paths (`Turns`); read the systems that use the clock first and document in the helper's doc comment exactly what it advances and what it does not (Review Focus 2).
- [ ] **Step 4: Run them to see them pass**, then the rl-bevy fingerprint test and `scripts/gate.sh`.
- [ ] **Step 5: Commit** with a `CHANGELOG.md` line: games may call `rl_engine::testing::let_time_pass` in their tests.

### Task 2: A sale is weighed against the trader's purse (engine slice, phase 1 follow-up)

**Files:**
- Modify: `crates/rl-bevy/src/trade.rs` (the test `short_currency_..._are_each_refused_and_say_which` and its neighbours)

The restored `dear` case asks 250 of a trader holding 100 from a customer holding 50, so it is short against either purse and cannot tell whose is read.

- [ ] **Step 1: Add a discriminating case:** a sale whose price falls between the two purses, for example a ware worth `320` at the house's 25% (`80`), sold to a trader holding `100` by a customer holding `50`; assert the sale goes through at `80` and both purses move by it. Reading the customer's purse would refuse it.
- [ ] **Step 2: Run** `cargo nextest run -p rl-bevy -E 'test(/trade::/)'`, the fingerprint tests and `scripts/gate.sh`; commit.

### Task 3: The generator backstop absorbs the generation sweeps

**Files:**
- Modify: `examples/undermere/src/dungeon.rs` tests: the backstop `every_place_of_a_run_keeps_every_promise_the_generator_makes_over_a_span_of_seeds` (~1348), `every_recipe_grows_within_forty_attempts_and_meets_itself_over_a_few_seeds` (~1983), `floor_ones_layout_differs_between_any_two_of_twenty_seeds` (~2003), `every_floor_is_the_same_whichever_floor_is_built_first` (~1664)

- [ ] **Step 1: The backstop builds content once:** `Undermere::new` once, then `reseeded(s)` per seed.
- [ ] **Step 2: Fold the recipe check in.** Seed `s` grows the recipes whose index `i` satisfies `i % QUICK == s % QUICK`, so the daily seeds cover every recipe once and CI's full range covers each several times. Keep only the checks the backstop does not already make (the grown size, the required route length, the exit not on a slot); delete the standalone test, keeping its existing `#[ignore]`d 0..200 tuning span if there is one.
- [ ] **Step 3: Fold the floor-one check in.** The backstop collects floor one's terrain per seed and asserts no two seeds share one; delete the standalone test.
- [ ] **Step 4: Narrow the build-order test** to one seed and floors `[1, 5, 10]`, built shallow-first on one instance and deep-first on a fresh `reseeded` one; say in its doc why one seed suffices (the property is per-place streams, not content).
- [ ] **Step 5: Verify** every seed the deleted tests played is still covered daily or in CI (Review Focus 4), run `cargo nextest run -p undermere --profile full -E 'test(/dungeon::/)'`, the fingerprints and `scripts/gate.sh`; commit.

### Task 4: Writings and feelings as rules

**Files:**
- Modify: `examples/undermere/src/writings.rs` (lift and tests: `a_remains_reads_the_same_line_twice_and_no_two_remains_in_a_run_share_one` ~274, `a_floor_with_a_feeling_piece_says_it_on_arrival_and_one_without_says_none` ~313)
- Modify: `examples/undermere/src/testing/mod.rs` (shared helpers)

**Interfaces:**
- Produces, in `testing/mod.rs`: `slot_of(undermere: &Undermere, spot: &Spot) -> Option<&Slot>` (moved from dungeon's tests' private `slot()`), and `first_floor_holding(piece: &str) -> (RunSeed, u32)`, extending `dungeon::drawn(name)`, which `expect`s a hit (Review Focus 1). Later tasks use both.
- Produces, in `writings.rs`: the loop body of `assign_writings` as a pure `assign(...)`, and the inner match of `tell_floor_feeling` as `feeling_of(...) -> Option<&str>`, each with the signature the code needs; both crate-private.

- [ ] **Step 1: Unit-test the two lifted rules** on made-up input: `assign` never gives two places one line while the pool lasts and keeps a key's line when reassigned; `feeling_of` returns the feeling for a spot with a feeling prop, nothing for a hidden-tag spot of the same prop, nothing for neither.
- [ ] **Step 2: Lift them**, leaving the systems as gather, call, apply.
- [ ] **Step 3: Rewrite each App test to one arrival** on the floor `first_floor_holding` finds for a piece that writes (`"A Fallen Adventurer"` or `"Last Words"`) and for a feeling piece (`"The Tinder Room"`), plus one floor without a feeling piece for the second test.
- [ ] **Step 4: Verify and commit.**

### Task 5: Population, captives and rumours

**Files:**
- Modify: `examples/undermere/src/population.rs` tests: `whichever_lair_a_floor_draws_holds_its_boss...` (~1059), `a_feeding_ground_stands_a_beast_over_its_preys_own_engine_remains...` (~1197), `a_floor_is_populated_with_more_of_its_own_biomes_favored_kinds` (~816), `a_floor_is_populated_on_first_arrival_away_from_the_entry` (~893), `the_dar_trio_is_placed_together...` (~999)
- Modify: `examples/undermere/src/captives.rs` test `exactly_one_floor_a_run_draws_holds_one_shackled_captive...` (~271)
- Modify: `examples/undermere/src/rumours.rs` test `the_innkeeper_tells_what_this_runs_floors_hold...` (~281)

- [ ] **Step 1: Lairs.** A static check over the prefabs: each mini-boss piece has a monster slot for its boss and between one and four other monster slots, and the Widow's has none. The App test then picks seeds with the pure `crate::dungeon::lairs(seed)`, stopping once it has one ordinary lair and the Widow's, two arrivals in all.
- [ ] **Step 2: Feeding ground:** stop at the first seed holding one, with `expect` (Review Focus 1).
- [ ] **Step 3: Favored kinds:** delete the sampled-share test (a spread, which AGENTS.md says is computed from the weights or not tested); lift the selection at ~615 into a crate-private `favored_on(...)` and unit-test that it returns the drawn biome's favored list.
- [ ] **Step 4: First arrival:** one seed, arriving at floor 1, a lair floor, floor 10 and `captive_floor(seed)`, then re-arriving at floor 1. Absorb the captive test's "exactly one shackled captive on the captive floor and none elsewhere" into this walk, and delete the captive test; add a pure test that `captive_floor` falls in its range over `seeds(full)`.
- [ ] **Step 5: Dar trio:** one or two seeds, saying why the placement does not depend on the seed.
- [ ] **Step 6: Rumours:** a pure test that the order told is the final boss, then the lairs shallowest first, then the biomes drawn, then the strata, then the last line, for `seeds(full)`; keep one App test that talks until the last line comes twice, with no arrivals. Name, in the report, the existing tests that cover what the floors hold (the throne boss in the backstop, the biomes in `the_biomes_a_rumour_is_told_of_are_the_ones_the_floors_draw...`).
- [ ] **Step 7: Verify and commit.**

### Task 6: Light, the run's start, traps, the town and the stairs walk

**Files:**
- Modify: `examples/undermere/src/light.rs` tests (~303, ~193, ~396, ~436), `examples/undermere/src/population.rs` test `a_goblin_camp_holds_a_fire...` (~1145), `examples/undermere/src/run.rs` tests (~699, ~589), `examples/undermere/src/traps.rs` test (~737), `examples/undermere/src/townsfolk.rs` test (~951), `examples/undermere/src/stairs.rs` test (~264)

- [ ] **Step 1: Lit on arrival:** a static brazier check over the prefabs (no piece that may stand above floor 10 has a cold brazier, no throne piece a warm one), and the App test on one seed at a floor with glowing tiles (chosen through the pure biome draw) and floor 10.
- [ ] **Step 2: Seen dimly:** find one dim floor and one Spider Warrens floor through `reseeded(s).biome_of(f)`, stopping at the first hit, and arrive at those two.
- [ ] **Step 3: The camp's fire:** fold its assertions into `a_goblin_camp_holds_a_fire...`, which searches the same floor; both stop at the first camp found, with `expect`.
- [ ] **Step 4: The lantern:** arrive at floor 4 and floor 10, then back to town.
- [ ] **Step 5: Starting near a post:** one App on one seed (the town is hand-drawn); for each role insert `StartNear`, call the run's stand-near system, settle and check.
- [ ] **Step 6: Starting on a floor:** floors 1 and `FLOORS` only.
- [ ] **Step 7: Secret doors found:** replace the sampled rate with arithmetic read from the spotting rules: the per-turn chance for a secret door's spot, and `1 - (1 - p)^20 >= 0.9` (Review Focus 5). Name the engine tests that cover the spotting roll's wiring.
- [ ] **Step 8: The wanderers:** unit-test the lamplighter's `rounds()` (every waypoint beside a stop and open, in angular order); keep the App test on one seed for as many turns as one round needs.
- [ ] **Step 9: The stairs walk** stays a play-through (it is the walk backstop): `first_seeds(full, 1)`.
- [ ] **Step 10: Verify and commit.**

### Task 7: Mercenaries and how unknown things read

**Files:**
- Modify: `examples/undermere/src/mercenaries/orders.rs` tests (~373, ~427, ~394), `mercenaries/travel.rs` (~440; lift), `mercenaries/knowing.rs` (~411, ~463; lift), `examples/undermere/src/seeming.rs` tests (~505, ~472)

- [ ] **Step 1: Keeping within the leash** (`a_follower_keeps_within_three...`): one seed, or a staged course of about 30 cells with one corner and one door built with `wall_off`/`lay_floor`; the gap rule is already unit-tested in rl-rules.
- [ ] **Step 2: Fighting freely and holding:** one seed each (the setups are staged), one App per test where both branches fit, each branch stopping at its first hit.
- [ ] **Step 3: Landing beside the adventurer:** lift the sort by distance and entity and the beside-loop from `follow_through` into a crate-private pure function; unit-test distinct cells, the nearer first with one cell free, and the lower entity at equal distance; keep one App run on one seed.
- [ ] **Step 4: Drinking its own potion:** one seed, one App covering the walled-off, seen-unknown and healing cases in turn.
- [ ] **Step 5: A better weapon:** `SeenWorth` calls `rl_rules::equip::gain` instead of its own copy; lift the rule scoring a thing of unknown level into a crate-private pure function tested against content read once; keep one seed of the App test.
- [ ] **Step 6: How unknown things read:** each seeming test becomes one App that gives every thing and reads every row (the pack) or equips each in turn (the vitals, the cursed one last), reading the armory from the app's resource instead of loading the library again.
- [ ] **Step 7: Verify and commit.**

### Task 8: Staffs, rings, potions and the necromancer

**Files:**
- Modify: `examples/undermere/src/armory/magic.rs` tests (~539, ~574, ~252, ~480, ~44, ~597, ~176), `armory/draughts.rs` tests (~293, ~76), `examples/undermere/src/bestiary/tests.rs` (~783)

**Interfaces:**
- Consumes: Task 1's `rl_engine::testing::let_time_pass(app, turns)`.

- [ ] **Step 1: One App per test** where a test builds one per case: every potion drunk in one App (paralysis waited off late, descent last); every staff and wand zapped in one App (a fresh held goblin before each, blinking last); every blinking level and aim in one App.
- [ ] **Step 2: Let time pass** instead of playing hundreds of turns, where the property reads the clock: the ring of regeneration, both Choirmaster tests, the ring of wisdom's recharge, the firebolt's charge (240 then 12). Unit-test the arithmetic that is the game's own (e.g. `staffs::paced`) beside it.
- [ ] **Step 3: Confusion:** one seed and one App, afflicting both the adventurer and the goblin (the stumble rule is unit-tested in rl-rules).
- [ ] **Step 4: The necromancer with no body:** one arena asserting the ability would not act, then that it would once a body lies in reach.
- [ ] **Step 5: Verify and commit.**

### Task 9: Rates from the content, and the screen without the turns

**Files:**
- Modify: `examples/undermere/src/armory/play.rs` test `a_dagger_kills_the_unaware_in_one_blow_far_more_often_than_a_sword...` (~115), `examples/undermere/src/main.rs` test `a_found_chain_mail_is_unidentified_and_says_how_long_until_its_level_is_known` (~1095)

- [ ] **Step 1: The dagger and the sword:** compute from the content with no draw (Review Focus 5): read the dagger's and the sword's dice from the armory, the unaware percentages, and the goblin's and rat's health from the bestiary; assert every dagger roll scaled with `rl_rules::arms::scale_unaware` fells both, and that the share of sword rolls leaving a goblin standing lies in the old bounds. Confirm goblins wear no armor; name the engine test covering the unaware blow's wiring.
- [ ] **Step 2: The chain mail:** set the worn count instead of playing it (the 200-turn count is proven by `lore::tests::armor_found_is_called_unidentified_until_it_has_been_worn_two_hundred_turns`): set it to show the countdown text, then to one short of known and play one turn.
- [ ] **Step 3: Verify and commit.**

### Task 10: The fast tier's own expensive tests

**Files:** the Undermere tests outside the slow tier at 1 s or more of processor time, listed by `python3 scripts/test-times.py --top 80` at the start of the task (on 2026-10-20 about 30, among them `population::...a_goblin_camp...`, `quest::...killing_whichever_boss_was_drawn...`, `shops::...a_run_saved_and_continued...`, `shops::...the_same_seed_restocks...`, `waystone::...finished_the_call...`, `population::...a_goblin_war_band...`, `mercenaries::tavern::...each_visit_seats...`, `bestiary::...a_necromancer_left_to_its_own_mind...`, `enchant::...a_clinging_curse...`, `mercenaries::travel::...one_left_behind...`, `vaults::...the_paralysis_vestibules_vent...`, the `mercenaries::saved::` tests, `armory::magic::polymorph...`).

- [ ] **Step 1: Classify each** as rule, generation promise, content presence or spread, or wiring, the way the tasks above were, and write the classification and the chosen form per test into the report before changing code.
- [ ] **Step 2: Apply** the levers of the tasks above to each where a cheaper form keeps the property; leave a test unchanged where none does and say why.
- [ ] **Step 3: Verify and commit.**

### Task 11: Re-tier, measure, and the full gate

**Files:**
- Modify: `.config/nextest.toml` (the slow tier's `default-filter`)

- [ ] **Step 1: Re-tier:** run `cargo nextest run --workspace --profile full` and `python3 scripts/test-times.py --profile full --top 80`; remove from the slow tier every Undermere test now under 2 s of processor time, and keep there every test still over it.
- [ ] **Step 2: The full gate:** `scripts/gate.sh --all`; it passes, with the budgets.
- [ ] **Step 3: Measure and record:** in the commit message and the report, Undermere's tests and processor time per tier before (the Measurement section's numbers) and after, from the same command; processor time is the comparable number, wall time is not.
- [ ] **Step 4: Commit.**
