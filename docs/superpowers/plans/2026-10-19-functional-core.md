# Functional Core: Rules Tested Without an App Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the rule decisions that live inside `rl-bevy` systems into Bevy-free pure functions in `rl-rules` (or `rl-core`/`rl-grid` where they are arithmetic or grid search), so each rule is unit-tested without an `App`, and each system only gathers, calls and applies.

**Architecture:** Functional core, imperative shell.
A system reads its queries and resources into plain values, calls one function in a tier 1 crate, and writes the result back.
`Entity` becomes a generic `A: Copy` (or `A: Copy + Ord` where order matters), as `Hit<A>`, `Brain<A>` and `Work<A>` already do; Bevy resources become plain arguments or closures, as `TacticCtx` already does.
Every task is behaviour-preserving, so the fingerprint tripwires are the regression net.

**Tech Stack:** Rust 2024, Bevy 0.19 (`rl-bevy` only), `rand` 0.9, cargo-nextest.

**Spec:** The Roadmap section below is the spec; it records the architecture review of 2026-10-19.
Line numbers in this plan are at commit `0ad89eb`; if code has moved, find tests by name.

## Roadmap

The engine stays a framework: Bevy ECS is the one runtime, and the ordered systems are its value.
What it gains is a line below which everything is plain Rust: the rules, the grids, and later the cell buffer presenters draw into.
Four phases, each its own plan; this plan is phase 1.

1. **Rules out of `rl-bevy` systems** (this plan).
   `rl-bevy` has 556 tests and almost all of them build an `App` (combat 44 of 46, status 22 of 22, consumable 19 of 19); each costs about 70 ms of processor time and 240 ms of wall time, where a unit test of a pure function costs well under 1 ms.
   Every rule lifted below the line is tested in microseconds and is callable by a game that wires its loop differently.
2. **Game rules out of Undermere systems.**
   The same pattern in `examples/undermere`, which spends 486 s of the suite's 696 s of processor time; 153 of its 560 tests drive a full game `App`.
   Its plan starts by measuring per-test cost with `scripts/test-times.py` and lifting the most expensive rules first.
3. **`rl-term`, a Bevy-free cell buffer.**
   A tier 1 crate holding `Cell { glyph, fg: Rgb, bg: Rgb }`, the buffer, and the drawing helpers now in `crates/rl-ui/src/panel/mod.rs`; `rl-render` becomes the Bevy backend that blits it.
   A presenter becomes a function of a view, a rectangle, a palette and a buffer, so presenters are tested without an `App` too.
4. **Input boundary and a crossterm backend.**
   Backends translate keys into `rl-ui`'s `EngineKey`/`Chord`; a crossterm plugin runs a headless Bevy app and writes the buffer to a real terminal, which behind `sshd`'s `ForceCommand` is SSH play.

Not doing: abstracting over the ECS (hecs, shipyard), and macroquad support until a game needs it.

## Global Constraints

- `rl-core`, `rl-grid` and `rl-rules` stay Bevy-free and build on `wasm32-unknown-unknown`: `scripts/check-tiers.sh` and `scripts/check-tiers.sh --wasm` pass. No `std::time::Instant`.
- `#![deny(missing_docs)]`; doc comments say why and why-not at the density of `crates/rl-core/src/turn.rs`; plain dash, never an em dash; no theme words in engine code or tests.
- Randomness arrives as `&mut impl Rng`; never construct a generator. Every draw happens in the same order and under the same conditions as before.
- No `HashMap`/`HashSet`; costs and clocks are integers, in hundredths of a step.
- Test names read as sentences describing the property.
- Behaviour-preserving: `rl-bevy`'s `fingerprint` test and Undermere's and Foundry's fingerprint tests pass unchanged. A task that moves a fingerprint has changed a rule; fix the task, never re-baseline.
- New tier 1 functions are `pub` (another crate calls them) and documented; they join a prelude only if a game would call them.
- Work happens in a worktree from `scripts/worktree.sh functional-core`; each task ends with `scripts/gate.sh` passing and one commit.
- A `CHANGELOG.md` line under Unreleased per task that adds public functions or moves a public type, saying what a game may now call.

## The Pattern

Every task below follows the same seven steps; tasks 1 and 2 spell them out in full, and the rest give only what differs.

1. Write the unit tests for the new function in the tier 1 crate, porting the values of the `App` tests the task lists.
2. Run them and see them fail to compile.
3. Implement the function.
4. Run them and see them pass.
5. Rewrite the `rl-bevy` system to gather, call the function, and apply its result.
6. Delete each listed `App` test whose every assertion now lives in a unit test; keep each listed **keep** test, which proves the system calls the rule.
7. Run the module's tests, the three fingerprint tests and `scripts/gate.sh`; commit.

The fingerprint tests are run with:

```bash
cargo nextest run --workspace --profile full -E 'test(/fingerprint/)'
```

## Review Focus

1. **A missing side or missing rules falls back differently per call site.** `stealth` and `work` treat "unknown" as at odds, while `bump`, `minds`, `props` and `status` treat it as not hostile; one shared function must return `Option<Relation>` and leave each fallback at its call site. Pinned by Task 1's `stance_is_none_when_the_rules_or_either_side_is_missing` and the call sites' kept `App` tests.
2. **Who is asked about whom.** `props` asks how *they* stand to *me*, `work` asks both ways round, the rest ask *me* to *them*; with an asymmetric table the order decides the answer. Pinned by Task 1's `stance_reads_the_table_from_the_first_side_to_the_second`.
3. **A random draw that moves.** `stumble` draws only when its chance is between 0 and 100 and some way is open; a lifted version that draws in other cases shifts every later roll. Pinned by Task 6's `a_stumble_draws_nothing_when_it_is_certain_impossible_or_walled_in`.
4. **Entity order becomes `A: Ord` order.** Followers through a way are ordered by distance and then entity; a generic version must order by the id it is handed, not by insertion. Pinned by Task 10's `followers_are_nearest_first_and_ties_go_to_the_lower_id`.
5. **Integer edges in health.** An absorbed hit at full health reports nothing; a blow past the last health reports the whole blow as dealt but only what was left as lost; a heal near `i32::MAX` saturates. Pinned by Task 2's tests.

---

### Task 1: One stance rule for the seven copies of it

**Files:**
- Modify: `crates/rl-rules/src/faction.rs` (new function and tests)
- Modify: `crates/rl-rules/src/lib.rs:119,153` (export `stance`)
- Modify: `crates/rl-bevy/src/stealth.rs:253`, `crates/rl-bevy/src/bump.rs:297`, `crates/rl-bevy/src/minds.rs:770,995`, `crates/rl-bevy/src/work.rs:418`, `crates/rl-bevy/src/props.rs:490`, `crates/rl-bevy/src/status.rs:396`
- Modify: `AGENTS.md`, the "Writing fast tests" list (one bullet stating the pattern)

**Interfaces:**
- Produces: `rl_rules::stance(factions: Option<&Factions>, a: Option<FactionId>, b: Option<FactionId>) -> Option<Relation>`, which is `Some(factions.relation(a, b))` when all three are present and `None` otherwise.

- [ ] **Step 0: Record the baseline.** In the new worktree run `scripts/gate.sh --all`, then `python3 scripts/test-times.py --top 0`, and keep the per-package table (tests, processor seconds) for the final task's commit message.

- [ ] **Step 1: Write the failing tests** in `faction.rs`'s test module:

```rust
#[test]
fn stance_is_none_when_the_rules_or_either_side_is_missing() {
    let defs = Registry::from_defs(vec![FactionDef::new("a"), FactionDef::new("b")]).unwrap();
    let (a, b) = (defs.expect("a"), defs.expect("b"));
    let f = Factions::new(&defs);
    assert_eq!(stance(None, Some(a), Some(b)), None);
    assert_eq!(stance(Some(&f), None, Some(b)), None);
    assert_eq!(stance(Some(&f), Some(a), None), None);
}

#[test]
fn stance_reads_the_table_from_the_first_side_to_the_second() {
    let defs = Registry::from_defs(vec![FactionDef::new("a"), FactionDef::new("b")]).unwrap();
    let (a, b) = (defs.expect("a"), defs.expect("b"));
    let mut f = Factions::new(&defs);
    f.set(a, b, Relation::Hostile);
    f.set(b, a, Relation::Neutral);
    assert_eq!(stance(Some(&f), Some(a), Some(b)), Some(Relation::Hostile));
    assert_eq!(stance(Some(&f), Some(b), Some(a)), Some(Relation::Neutral));
}
```

- [ ] **Step 2: Run them to see them fail.** `cargo test -p rl-rules stance` fails: `cannot find function stance`.

- [ ] **Step 3: Implement `stance`** in `faction.rs` with the signature above, and add it to both `pub use faction::{...}` lists in `lib.rs`.

- [ ] **Step 4: Run them to see them pass.** `cargo test -p rl-rules stance` passes.

- [ ] **Step 5: Replace the seven copies.** Each call site passes `rules.map(|r| &r.factions)` and the two `FactionId`s, and keeps its own fallback and its own argument order:

| Site | Becomes |
|---|---|
| `stealth.rs:253` `at_odds` | `stance(..mine, theirs).map_or(true, \|r\| r == Relation::Hostile)` |
| `bump.rs:297` `relation` | `stance(..me, other)` |
| `minds.rs:995` `neutral` | `stance(..me, them) == Some(Relation::Neutral)` |
| `minds.rs:770` roster | `stance(..mine, theirs)` |
| `work.rs:418` foes | `stance(..mine, theirs).zip(stance(..theirs, mine)).map_or(true, \|(x, y)\| x == Relation::Hostile \|\| y == Relation::Hostile)` |
| `props.rs:490` `hostile` | `stance(..theirs, mine) == Some(Relation::Hostile)` |
| `status.rs:396` | `stance(..mine, theirs) == Some(Relation::Hostile)` |

Delete the private helpers `at_odds` and `relation` that are left unused.

- [ ] **Step 6: Add the pattern to `AGENTS.md`.** One bullet in "Writing fast tests", after "A property of generation is tested without an `App`": a rule inside a system is a rule only an `App` can test, so a system gathers, calls a function in `rl-rules`, and applies; the `App` test that is left proves the call, and the rule's cases are unit tests.

- [ ] **Step 7: Verify and commit.** `cargo nextest run -p rl-bevy -E 'test(/stealth|bump|minds|work|props|status/)'`, the fingerprint tests, then `scripts/gate.sh`. Commit with a `CHANGELOG.md` line naming `rl_rules::stance`.

### Task 2: What a hit and a heal do to health

**Files:**
- Modify: `crates/rl-rules/src/damage.rs` (new types, functions and tests)
- Modify: `crates/rl-rules/src/lib.rs` (export them)
- Modify: `crates/rl-bevy/src/combat.rs:1312-1319` (`Mitigation::lands`), `:1336-1361` (`apply_damage`), `:1368-1378` (`apply_healing`)
- Test: delete ported tests at `combat.rs:1843, 1861, 2426, 2465`; **keep** `an_invulnerable_actor_takes_no_harm_and_is_still_healed` (2441)

**Interfaces:**
- Consumes: `rl_rules::Vitals { current: i32, max: i32 }`.
- Produces:
  - `pub struct Applied { pub current: i32, pub dealt: i32, pub lost: i32, pub died: bool }`
  - `pub fn apply_hit(health: Vitals, amount: i32) -> Applied`: `current = (health.current - amount).min(health.max)`; `dealt` is `amount` for a harmful hit and `health.current - current` for an absorbed one (`amount < 0`); `lost = health.current - current.max(0)`; `died = current <= 0`.
  - `pub fn heal(health: Vitals, amount: i32, mending_pct: i32) -> i32` returns the new current: the offer is `amount.max(0) * mending_pct / 100` computed in `i64`, added saturating, stopped at `max`, never below the old current.
  - `pub fn mitigate(amount: i32, invulnerable: bool, peaceful: bool) -> i32`: `0` when `peaceful`; otherwise `amount.min(0)` when `invulnerable`; otherwise `amount`.

- [ ] **Step 1: Write the failing tests** in `damage.rs`:

```rust
#[test]
fn a_hit_takes_health_and_a_blow_past_the_last_reports_all_of_it_dealt_and_only_what_was_left_lost() {
    let v = |current| Vitals { current, max: 10 };
    assert_eq!(apply_hit(v(10), 4), Applied { current: 6, dealt: 4, lost: 4, died: false });
    assert_eq!(apply_hit(v(3), 8), Applied { current: -5, dealt: 8, lost: 3, died: true });
}

#[test]
fn an_absorbed_hit_restores_up_to_full_and_reports_what_it_restored() {
    let v = |current| Vitals { current, max: 10 };
    assert_eq!(apply_hit(v(10), -5), Applied { current: 10, dealt: 0, lost: 0, died: false });
    assert_eq!(apply_hit(v(6), -3), Applied { current: 9, dealt: -3, lost: -3, died: false });
}

#[test]
fn a_heal_is_scaled_by_mending_stopped_at_full_and_never_takes_health_away() {
    let v = |current| Vitals { current, max: 10 };
    assert_eq!(heal(v(5), 4, 100), 9);
    assert_eq!(heal(v(9), 4, 100), 10);
    assert_eq!(heal(v(5), 4, 50), 7);
    assert_eq!(heal(v(5), 4, 0), 5);
    assert_eq!(heal(v(5), -4, 100), 5);
    assert_eq!(heal(Vitals { current: i32::MAX - 1, max: i32::MAX }, i32::MAX, 100), i32::MAX);
}

#[test]
fn an_invulnerable_target_keeps_what_it_absorbs_and_a_peaceful_place_takes_nothing_either_way() {
    assert_eq!(mitigate(7, false, false), 7);
    assert_eq!(mitigate(7, true, false), 0);
    assert_eq!(mitigate(-3, true, false), -3);
    assert_eq!(mitigate(7, false, true), 0);
    assert_eq!(mitigate(-3, false, true), 0);
}
```

- [ ] **Step 2: Run them to see them fail.** `cargo test -p rl-rules damage` fails to compile: `apply_hit`, `heal`, `mitigate` and `Applied` are not defined.

- [ ] **Step 3: Implement** the three functions and `Applied` (derive `Debug, Clone, Copy, PartialEq, Eq`) with the semantics in Interfaces, and export them.

- [ ] **Step 4: Run them to see them pass.** `cargo test -p rl-rules damage` passes.

- [ ] **Step 5: Rewrite the systems.** `Mitigation::lands` keeps the stage pipeline and returns `mitigate(amount, invulnerable, self.map.is_peaceful(map))`. `apply_damage` keeps its `health.current <= 0` skip, calls `apply_hit(Vitals { current, max }, amount)`, and writes `current`, `DamageDealt { dealt, lost, .. }` and the `DeathEvent` from `died`. `apply_healing` calls `heal(.., loadout.mending(target))` and reports `new - before`.

- [ ] **Step 6: Delete the ported `App` tests** listed under Files, keeping the **keep** test.

- [ ] **Step 7: Verify and commit.** `cargo nextest run -p rl-bevy -E 'test(/combat::/)'`, the fingerprint tests, and `scripts/gate.sh`. Commit with a `CHANGELOG.md` line naming `rl_rules::{apply_hit, heal, mitigate, Applied}`.

### Tasks 3 to 13

Each of these follows the Pattern.
For each one, the table gives the new functions and their file, the `rl-bevy` code they replace, the `App` tests whose assertions become the unit tests (ported with their values), and the **keep** test that stays to prove the wiring.
The implementer names each unit test as a sentence describing the case it ports.

#### Task 3: The weapon an actor strikes with, and what it is worth

- **New** in `crates/rl-rules/src/arms.rs` (export from `lib.rs`):
  - `pub enum Reach { Melee, Ranged }`
  - `pub fn choose_reach(adjacent: bool, line_of_fire: bool, has_melee: bool, has_ranged: bool) -> Option<Reach>`
  - `pub fn first_charged<I: Copy, W: Copy>(worn: impl IntoIterator<Item = (I, Option<W>, bool)>, own: Option<W>) -> Option<(Option<I>, W)>`
  - `pub fn armor_total(own: i32, worn: impl IntoIterator<Item = i32>, stat: i32) -> i32`
  - `pub fn mending_pct(stat: i32) -> i32`
  - `pub fn scale_unaware(amount: i32, pct: u32) -> i32`
- **Replaces:** `combat.rs:736-829` (`Loadout` arithmetic), `:1000-1018` (weapon choice), `:1054-1064` (unaware worth, scaling, floor); `accuracy.rs:123-133` `at_distance`; `forecast::Arms::at`. All three weapon-choice copies call `choose_reach`.
- **Port:** `combat.rs:1922`, `:1991`, `:2759`.
- **Keep:** `a_worn_blade_replaces_the_fist_and_worn_strikes_add_up`.

#### Task 4: What bumping into a cell does

- **New:**
  - in `crates/rl-rules/src/bump.rs`: `pub enum Offered<O> { None, One(O), Several }`, `pub enum BumpOutcome<A, O> { Step, Open, Attack(A), Swap(A), Interact(O), Ask(A), Refuse(A) }`, `pub fn classify_offers<O: Copy>(offers: &[O], plain: impl Fn(&O) -> bool) -> Offered<O>`, and `pub fn resolve_bump<A: Copy, O>(someone: Option<A>, stance: Option<Relation>, gives_way: bool, allies_swap: bool, offered: Offered<O>, door: bool) -> BumpOutcome<A, O>`
  - in `crates/rl-core/src/turn.rs`: `pub fn step_cost(tile: u32, diagonal: bool) -> u32`, which is `tile * 1414 / 1000` on a diagonal
- **Replaces:** `bump.rs:193-279` (`redirect_bumps` decision), `:283-295` (`offers_at` classification), `:351-352`; `turn.rs:617-618` (`step_cost`).
- **Port:** `bump.rs:444, 479, 505, 543, 557, 570, 592, 617, 649, 686`.
- **Keep:** `bump.rs:809`.

#### Task 5: How a mind's choice becomes a step

- **New** in `crates/rl-rules/src/ai/`:
  - `pub enum StepAs { Swap, Open(Direction), Step(Direction), Wait }`
  - `pub fn step_as(from: Point, to: Point, gives_way: bool, door: bool) -> StepAs`
  - `pub fn sort_roster<V>(seen: impl IntoIterator<Item = (V, Option<Relation>)>) -> (Vec<V>, Vec<V>, Vec<V>)`, returning enemies, allies and others in the order seen
  - Move the `Recent` cache (`minds.rs:235-279`) to `crates/rl-rules/src/ai/recent.rs` unchanged.
- **Replaces:** `minds.rs:1048-1065` (decision to intent), `:770-778` (roster); `Thinking::offer_trail` `:629-634` becomes `Trail::offer` in `rl-rules`.
- **Port:** `minds.rs:2484`; `bump.rs:776, 786, 796`.
- **Keep:** `minds.rs:2597`.

#### Task 6: Charges, recharging and pulses

- **New** in `crates/rl-rules/src/charges.rs`:
  - `pub enum Spent { Left(u16), NextUnit, Destroyed, Empty }`
  - `pub fn spend(left: u16, max: u16, stack: Option<u32>, when: WhenEmpty) -> Spent`
  - `pub fn recharge(left: u16, max: u16, progress: u32, every: u32, passed: u32) -> (u16, u32)`
  - `pub fn periods(progress: u32, every: u32, passed: u32) -> (u32, u32)`, returning the times fired and the progress left
- **Replaces:** `consumable.rs:151-163, 224-238, 251-254`; `items.rs:792-797` (`pulse_worn` counting).
- **Port:** `consumable.rs:409, 416, 424, 448, 472, 511`.
- **Keep:** `consumable.rs:527`.

#### Task 7: The rest of a status's rules

- **New** in `crates/rl-rules/src/status.rs`:
  - `pub fn stumble(pct: Option<u32>, open: &[Direction], rng: &mut impl Rng) -> Option<Direction>`. It draws from `rng` only when `0 < pct < 100` and `open` is non-empty; it takes the percent gate first, then a uniform pick, in that order.
  - `pub fn wakes_from_harm(ticked_by_its_own_sleep: bool, fell_asleep_this_pass: bool) -> bool`
  - `pub fn side_change(held: Option<FactionId>, now: FactionId, was: Option<FactionId>) -> Option<(FactionId, Option<FactionId>)>`
- **Replaces:** `status.rs:552-564, 315, 434-444`.
- **Port:** `status.rs:1270, 1291, 1088, 1107, 1200`.
- **Keep:** `status.rs:1128`.
- **Review Focus test:** `a_stumble_draws_nothing_when_it_is_certain_impossible_or_walled_in` seeds two `StdRng`s alike, calls `stumble` on one with `pct` `Some(0)`, `Some(100)` and `None`, and with `open` empty, then asserts `a.random::<u64>() == b.random::<u64>()`.

#### Task 8: Noticing and staying aware

- **New** in `crates/rl-rules/src/ai/awareness.rs`:
  - `pub fn observe(state: Awareness, in_view: bool, spots: impl FnOnce() -> bool, at: Point, memory: u32) -> (Awareness, bool)`, where `spots` is called exactly when the old code rolled
  - `pub fn unaware(asleep: bool, running: bool, hides: bool, at_odds: bool, knows: bool) -> bool`
- **Replaces:** `stealth.rs:443-498` (transitions; the subject order stays in the system), `:215-222`.
- **Port:** `stealth.rs:659, 675, 825`.
- **Keep:** `stealth.rs:864`.

#### Task 9: Why work breaks off

- **New** in `crates/rl-rules/src/work.rs`:
  - Move `BreakReason` from `rl-bevy/src/work.rs:134` here, with `rl-bevy` re-exporting it from the same path so games compile unchanged.
  - `pub fn break_reason(died: bool, hurt: bool, done_by_another: bool, reachable: bool) -> Option<BreakReason>`, taking the first that applies in the order Died, Hurt, DoneByAnother, OutOfReach
  - `pub fn newly_sighted<A: PartialEq>(seen: &[A], now: &[A]) -> bool`
- **Replaces:** `work.rs:270-301, 513-516`.
- **Port:** `work.rs:772, 780, 806`.
- **Keep:** `work.rs:939`.

#### Task 10: Who travels behind whom, and where they land

- **New:**
  - in `crates/rl-rules/src/travel.rs`: `pub fn followers<A: Ord + Copy>(candidates: impl IntoIterator<Item = (A, Point, bool)>, at: Point) -> Vec<A>`, keeping those that follow and ordering them by Chebyshev distance to `at` and then by id
  - in `crates/rl-grid/src/landing.rs`: `pub fn landing(want: Point, free: impl Fn(Point) -> bool, walkable: impl Fn(Point) -> bool, reach: usize) -> Option<Point>`, the breadth-first ring search with `places.rs`'s `LANDING_REACH` passed as `reach`
- **Replaces:** `places.rs:565-578, 622-643`.
- **Port:** `places.rs:965, 1095`.
- **Keep:** `places.rs:1264`.
- **Review Focus test:** `followers_are_nearest_first_and_ties_go_to_the_lower_id` hands in ids `[5, 2, 9]`, all at distance 1 except `9` at distance 2, and expects `[2, 5, 9]`.

#### Task 11: Gear upgrades, use rules and dark sight

- **New** in `crates/rl-rules/src/equip.rs`:
  - `pub fn gain<I: Copy + Eq>(worn: &Equipment<I>, item: I, shape: &EquipShape, stays: impl Fn(I) -> bool, score: i32, score_of: impl Fn(I) -> i32, has_blow: impl Fn(I) -> bool, own_blow: i32) -> Option<i32>`
  - `pub fn may_use(in_bag: bool, known_empty: bool, wearable: bool, worn: bool) -> bool`
  - `pub fn dark_sight(own: Option<i32>, worn: impl IntoIterator<Item = i32>, suppressed: bool) -> Option<i32>`
- **Replaces:** `items.rs:1087-1102, 494-518, 938-942`.
- **Port:** `items.rs:1327`; `minds.rs:1907, 1952`.
- **Keep:** `items.rs:1505`, `minds.rs:1997`.

#### Task 12: Prices, purses and refusals

- **New:**
  - `crates/rl-rules/src/trade.rs`: `pub fn quote(...)`, carrying `Trader::pays/each/quote` (`trade.rs:75-111`) as plain functions over `Ware`'s fields. The `Trader` and `Ware` components stay in `rl-bevy` and delegate. Also `pub fn draw_down<I: Copy>(pieces: &[(I, u32)], amount: u32) -> Vec<(I, u32)>`.
  - `crates/rl-rules/src/prop.rs`: move `Refused` here from `rl-bevy/src/props.rs:216`, with `rl-bevy` re-exporting it from the same path so games compile unchanged. Add `pub fn refusal(needs: Option<TagId>, carries: bool, price: Option<u32>, purse: u32) -> Option<Refused>`. Move `SpotRules` (`props.rs:1134-1149`) here with its existing unit test.
- **Replaces:** `trade.rs:75-111, 380-391`; `props.rs:468-470`.
- **Port:** `trade.rs:706`; `props.rs:1690, 1996`.
- **Keep:** `trade.rs:728`, `props.rs:2092`.

#### Task 13: Noise, effects geometry, and the measurement

- **New:**
  - in `crates/rl-rules/src/hearing.rs`: `pub fn loudest(arrivals: impl IntoIterator<Item = (i32, Point)>) -> Option<Point>`, keeping the existing tie-break at `noise.rs:426-429`
  - in `crates/rl-grid/src/slide.rs`: `pub fn slide(from: Point, toward: Point, cells: i32, free: impl Fn(Point) -> bool) -> Point`
  - in `crates/rl-rules/src/effects.rs`: `pub fn landing_user<A: Copy>(as_itself: bool, is_actor: bool, carrier: A, by: Option<A>) -> A`
- **Replaces:** `noise.rs:426-429`; `effects/mod.rs:343-363` (`EffectWorld::slide`); `effects/triggers.rs:667`.
- **Port:** `noise.rs:612`.
- **Keep:** `noise.rs:629`.
- **Final step:** run `scripts/gate.sh --all` and `python3 scripts/test-times.py --top 0`, and put the before and after per-package table (tests, processor seconds) for `rl-bevy` and `rl-rules` in the commit message.

### Task 14: A terminal walk on the toolkit alone

A small example that uses only the Bevy-free crates, the way bracket-lib is used: the game owns its loop, and the engine hands it a map, a field of view and a terminal-sized picture of both.

**Files:**
- Create: `examples/terminal/Cargo.toml`, package `terminal`, `publish = false`, `[package.metadata.rl-engine] tier = 3`, depending on `rl-core`, `rl-grid` and `rl-mapgen` from the workspace, `rand` as the workspace names it, and `crossterm = "0.29"`. It does not depend on Bevy, `rl-bevy` or `rl-engine`.
- Modify: root `Cargo.toml` (`members` gains `"examples/terminal"`)
- Create: `examples/terminal/src/lib.rs` (the game: pure, unit-tested)
- Create: `examples/terminal/src/main.rs` (the terminal loop only)
- Modify: `docs/OVERVIEW.md` (one line describing the example) and `docs/guide/src/09-where-to-go-next.md` (one line naming it), because `scripts/check-overview.sh` requires every example to be in both

**Interfaces:**
- Produces, in `lib.rs`:
  - `pub struct Walk { pub terrain: Terrain, pub tiles: TileRegistry, pub at: Point, pub seen: BitGrid, pub explored: BitGrid }`
  - `pub const WIDTH: i32 = 60; pub const HEIGHT: i32 = 24; pub const SIGHT: i32 = 8;`
  - `pub fn generate(seed: u64) -> Walk`. It builds `BaseContext::blank(WIDTH, HEIGHT, TileRegistry::standard(), wall)` and runs the chain `Rooms { floor, ..Default::default() }`, then `RandomStart`, with `RunSeed(seed)`. It stands the player on the chain's `StartPoint`, then calls `look`.
  - `pub fn step(walk: &mut Walk, dir: Direction) -> bool` moves one cell when the target is walkable, then calls `look`, and returns whether it moved.
  - `pub fn look(walk: &mut Walk)` recomputes `seen` with `rl_grid::fov::compute` at range `SIGHT` and unions it into `explored`.
  - `pub fn render(walk: &Walk) -> Vec<String>`, `HEIGHT` rows of `WIDTH` characters: `@` at the player; a cell in sight drawn as `#` (wall) or `.` (floor); an explored cell out of sight drawn the same, and a cell never seen drawn as a space. Colour is the terminal loop's business, not `render`'s.
  - `pub fn direction_of(key: char) -> Option<Direction>` maps `h j k l y u b n` (vi keys) to the eight directions.
- `main.rs`:
  - enters raw mode and the alternate screen, and hides the cursor;
  - draws `render`'s rows each turn, dimming rows of remembered cells is optional;
  - reads `crossterm::event::read()`, mapping arrow keys and `direction_of` to `step`, and quits on `q` or `Esc`;
  - restores the terminal on quit and on panic, using a guard whose `Drop` leaves raw mode and the alternate screen;
  - takes an optional seed as its first argument, defaulting to `1`.

- [ ] **Step 1: Write the failing tests** in `lib.rs`:
  - `the_player_starts_on_floor_over_a_span_of_seeds`: for seeds `0..8`, the tile at `generate(seed).at` is walkable.
  - `a_step_into_a_wall_is_refused_and_leaves_the_player_where_it_was`: on a `generate(1)` walk, find a direction whose neighbour is wall; `step` returns `false` and `at` is unchanged.
  - `a_step_onto_floor_moves_the_player_and_what_it_saw_stays_explored`: find a walkable neighbour; `step` returns `true` and moves `at` there, and every cell explored before the step is still explored after it.
  - `the_picture_is_the_map_size_and_shows_the_player_once`: `render` returns `HEIGHT` rows of `WIDTH` chars, with exactly one `@`, at `at`.
  - `a_cell_never_seen_is_blank`: some cell outside `explored` renders as `' '`.
- [ ] **Step 2: Run them to see them fail.** `cargo test -p terminal` fails to compile.
- [ ] **Step 3: Implement `lib.rs`** with the signatures above, and `main.rs` as described. `main.rs` holds no rules: everything it decides is a key mapping.
- [ ] **Step 4: Run them to see them pass.** `cargo test -p terminal` passes. `cargo run -p terminal` opens the map in the terminal; walking with the arrows and `hjklyubn` moves `@` and reveals the rooms, and `q` restores the shell. That is checked by hand and stated in the report.
- [ ] **Step 5: Verify and commit.** `scripts/check-tiers.sh` passes, which confirms the example reaches no Bevy crate through its dependencies (`cargo tree -p terminal | grep -c bevy` prints `0`). `scripts/check-overview.sh` and `scripts/gate.sh` pass. Commit with a `CHANGELOG.md` line naming the example.

### Task 15: The README says how the engine is split

**Files:**
- Modify: `README.md`, a new section titled "Two ways to use it", placed after the feature list and before the crate table

**Interfaces:**
- Consumes: Task 14's example (`examples/terminal`), and the rule functions Tasks 1 to 13 added to `rl-rules`.

The section explains the split to someone choosing how to use the engine.

- **The toolkit.** `rl-core`, `rl-grid`, `rl-mapgen` and `rl-rules` contain no Bevy and build for the browser. They play the role bracket-lib plays: grids, field of view, light, pathfinding, map generation chains, content registries, and the rules (combat, statuses, charges, AI tactics). They are plain functions a game calls from a loop it owns, tested with no `App`.
  - Point at `examples/terminal` as the worked example of a game on the toolkit alone, in a terminal, with no Bevy.
  - Name which crates to depend on, and say that depending on them never compiles Bevy.
- **The framework.** `rl-bevy`, `rl-render`, `rl-ui` and `rl-save`, behind the `rl-engine` facade, own the loop: the turn order, the systems in a fixed sequence, the plugins, the panels, the saves.
  - A game adds `RoguelikePlugins` and the subsystems it wants by name.
  - Point at the tutorial, Undermere and Foundry as worked examples.
- **How the two meet.** A framework system gathers what it needs from the world, calls a toolkit function, and applies the result. So a rule behaves the same in either, and a game can start on the framework and drop to the toolkit wherever it wants different wiring.
  - The ways to change the framework without forking it are: leave out a plugin, join a system set, replace a presenter, and insert a resource of your own.
- **What it does not do.** It does not abstract over the ECS: the framework is Bevy's. A terminal or SSH front end for the framework is planned; for now the toolkit example is the terminal path.

Every sentence goes on its own line, with a plain dash and never an em dash. The section names only crates, types and examples that exist when it is written.

- [ ] **Step 1: Write the section** as above.
- [ ] **Step 2: Verify and commit.** `scripts/check-overview.sh` and `scripts/check-guide.sh` pass, and every crate, type, function and example the section names exists in the tree (confirm each with `grep`). Commit.
