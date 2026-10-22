# Usability 12: Rules Out of Systems Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Two fixed rule numbers become rule values a game sets, stealth's `at_odds` becomes a tier-1 rule beside `stance`, and `resolve_items` becomes gather, a tier-1 `resolve_item`, and apply.

**Architecture:** The pattern `may_use`, `SpotRules::chance` and `faction::stance` already follow: the system gathers plain facts, calls one function in `rl-rules`, and applies what it returns.
The rule's cases become unit tests; the `App` tests that remain prove the call.

**Tech Stack:** Rust 2024, Bevy 0.19.

**Spec:** `docs/reviews/engine_usability.md` section 4.4; `docs/TODO.md` item 34; index `docs/superpowers/plans/2026-10-21-engine-usability.md`.

## Global Constraints

- The index's "Rules for every branch" bind this branch: worktree `scripts/worktree.sh usability-12-rules-out-of-systems`, after branch 09 merges.
- No fingerprint moves: the defaults are today's numbers and `resolve_item` decides exactly what the system decided.
- Costs and distances are integers.
- An `App` test removed names the unit test that now holds its case.
- Plain dash, never an em dash.

## Review Focus

1.
  Equipping from the ground still costs half again a step by default: `ItemRules::default().equip_from_ground == BASE_ACTION_COST * 3 / 2`.
2.
  A game with no sides: `at_odds` answers yes, as stealth did; pinned by `with_no_sides_everyone_is_at_odds`.
3.
  A worn `Stuck` thing under every arm (drop, unequip, equip over it): each refusal is a unit case of `resolve_item`.
4.
  A use aimed where the aim is refused, and a thing the player does not know: the gather reports both, and the rule refuses both; unit cases.
5.
  The order of events an equip that displaces two things writes (`Unequipped` each, then `Equipped`): `resolve_item` returns them in that order; a unit case.

---

### Task 1: `at_odds` in tier 1

**Files:**
- Modify: `crates/rl-rules/src/faction.rs` (beside `stance`)
- Modify: `crates/rl-bevy/src/stealth.rs:267-272`

**Interfaces:**
- Produces: `pub fn at_odds(factions: Option<&Factions>, a: Option<FactionId>, b: Option<FactionId>) -> bool`: `stance(..)` is `None` or `Some(Hostile)`.

- [ ] **Step 1: Write the failing tests** `with_no_sides_everyone_is_at_odds`, `hostile_sides_are_at_odds_and_neutral_or_allied_ones_are_not`.
- [ ] **Step 2:** `cargo test -p rl-rules faction`.
  Expected: FAIL.
- [ ] **Step 3:** Implement; stealth calls it and its private copy goes.
- [ ] **Step 4:** `scripts/gate.sh`, commit.
  CHANGELOG: "`rl_rules::faction::at_odds`, whether one side has anything against another, the rule stealth asks. To change: nothing."

### Task 2: The two rule numbers are rule values

**Files:**
- Modify: `crates/rl-bevy/src/items.rs:357-365,448-455` (`EQUIP_FROM_GROUND_COST`, `Which::cost`)
- Modify: `crates/rl-bevy/src/remains.rs:178,202-262` (`STANDING_ROOM`, `revive`, `standing_room`)

**Interfaces:**
- Produces: `#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)] pub struct ItemRules { pub equip_from_ground: u32 }`, `Default` gives `BASE_ACTION_COST * 3 / 2`; `ItemsPlugin` runs `init_resource::<ItemRules>()`.
- Produces: `#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)] pub struct RevivalRules { pub standing_room: u32 }`, `Default` gives 2; the plugin that adds revival inits it, and `revive` reads it.

- [ ] **Step 1: Write the failing tests** `equipping_from_the_ground_costs_what_the_item_rules_say` (set 250, assert the actor's next turn is 250 later) and `a_body_is_revived_no_further_off_than_the_revival_rules_allow` (room 1, the ring at 2 free and the ring at 1 full: refused).
- [ ] **Step 2:** Run them.
  Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run them and the fingerprints.
  Expected: PASS, unchanged.
  `scripts/gate.sh`, commit.
  CHANGELOG: "`ItemRules` and `RevivalRules` hold what equipping from the ground costs and how far off a revived body may stand, the numbers the engine fixed. To change: nothing; insert either to change it."

### Task 3: `resolve_items` is gather, `resolve_item`, apply

**Files:**
- Create: `crates/rl-rules/src/items.rs` (and its `lib.rs` entry and module-map line)
- Modify: `crates/rl-bevy/src/items.rs:540-757` (`resolve_items`)
- Test: `crates/rl-rules/src/items.rs`; `crates/rl-bevy/src/items.rs` test module (29 tests)

**Interfaces:**
- Produces: `pub enum ItemAsk<I> { PickUp, Drop(I), Equip(I), EquipFromGround(I), Unequip(I), Use(I), UseAt(I, Point), UseOn(I, I) }`.
- Produces: `pub struct ItemFacts<I>`: plain facts the system gathers for one ask, such as what lies under the actor for `PickUp`, whether the actor carries the thing, whether it lies under them, whether it is stuck, whether it may be used (`may_use`), whether an aim was refused, the wearable shape, and which worn things may come off.
  The exact fields are whatever the 217 lines read; nothing in it is an `Entity` query.
- Produces: `pub enum ItemOutcome<I> { PickedUp(I), Dropped(I), Equipped(I), Unequipped(I), WouldNotComeOff(I), Used(I), Refused(ItemRefusal) }` with `ItemRefusal` naming each reason the system refuses today.
- Produces: `pub fn resolve_item<I: Copy + Eq>(ask: ItemAsk<I>, facts: &ItemFacts<I>, worn: Option<&mut Equipment<I>>) -> Vec<ItemOutcome<I>>`, outcomes in the order the events are written today.

- [ ] **Step 1:** Read `resolve_items` and list each arm's refusals and outcomes; that list is the unit-test list.
- [ ] **Step 2: Write the failing unit tests** in `rl-rules`, one per refusal and outcome per arm, named as sentences (for example `a_stuck_thing_worn_is_not_dropped`, `equipping_over_two_things_takes_both_off_before_putting_it_on`).
- [ ] **Step 3:** `cargo test -p rl-rules items`.
  Expected: FAIL to compile.
- [ ] **Step 4:** Implement `resolve_item`; rewrite the system as gather, call, apply (merging stacks, moving to the ground, spawning, writing `ItemEvent`, charging the cost), each under clippy's argument limit.
- [ ] **Step 5:** Run every rl-bevy items test unchanged.
  Expected: PASS.
  Then remove each `App` test whose case is now a unit test, keeping at least one per arm that proves the call and every test of an entity effect; the commit lists each removed test beside its unit test.
- [ ] **Step 6:** Fingerprints and `scripts/gate.sh`.
  Expected: PASS, unchanged.
- [ ] **Step 7:** Commit.
  CHANGELOG: "`rl_rules::items::resolve_item` decides what picking up, dropping, wearing and using a thing does, the rule `ItemsPlugin` applies, so a terminal game uses the same rule. To change: nothing."

### Task 4: Branch end

- [ ] `docs/TODO.md` item 34 drops `EQUIP_FROM_GROUND_COST` and `STANDING_ROOM`; one line in `docs/PLAN.md`'s progress log.
- [ ] `scripts/gate.sh --all`, rebase, re-gate, merge.
