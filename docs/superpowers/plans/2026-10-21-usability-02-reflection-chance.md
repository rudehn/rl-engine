# Usability 02: Reflection's Chance Is a Rule Value Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `Reflects` carries its chance, a rule value a game sets and a test sets to always or never, and the reflection and spotting tests stop counting outcomes over seeds.

**Architecture:** `Reflects` becomes a component with a `percent` field defaulting to 50, rolled with the same `random_bool(0.5)` at the default so no draw moves.
The rate tests become one app per outcome; the arithmetic they sampled is already unit-tested in tier 1 (`SpotRules::chance`) or is `rand`'s own.

**Tech Stack:** Rust 2024, Bevy 0.19, rand 0.9.

**Spec:** `docs/reviews/engine_usability.md` sections 4.3 and 4.4; index `docs/superpowers/plans/2026-10-21-engine-usability.md`.

## Global Constraints

- The index's "Rules for every branch" bind this branch: worktree `scripts/worktree.sh usability-02-reflection-chance`, `scripts/gate.sh` per commit, `scripts/gate.sh --all` at the end.
- No fingerprint moves: at the default the roll is `random_bool(f64::from(50u8) / 100.0)`, which is `random_bool(0.5)` exactly.
- A rewritten test proves what it proved, or the task names where each dropped assertion lives; never weaken one to pass.
- Plain dash, never an em dash.

## Review Focus

1.
  A body inserted as `Reflects::default()` must roll exactly as the unit `Reflects` did, or the fingerprints move: pinned by running the three fingerprint tests unchanged.
2.
  A percent above 100 written by a game: clamped to 100, never a panic from `random_bool`.
3.
  `Reflects::ALWAYS` on a body in an app with no `AbilitiesPlugin` must still turn a use back, which needs the stream: `an_aimed_use_reflects_off_a_reflecting_body_with_no_abilities_plugin_in_the_app` keeps that check with `ALWAYS`.
4.
  A test that asserted both outcomes keeps both, one app each.
5.
  Undermere's golem keeps reflecting at the default: `examples/undermere/src/bestiary/tests.rs:916` still passes.

---

### Task 1: `Reflects { percent }`

**Files:**
- Modify: `crates/rl-bevy/src/ability.rs:338-339` (the type), `:427` (the query), `:565` (the roll)
- Modify: every `insert(Reflects)` and `spawn((.., Reflects))`: `examples/undermere/src/bestiary.rs:780`, `crates/rl-render/src/particles.rs:901`, `crates/rl-bevy/src/items.rs:1818,1941` (`rg -nw 'Reflects\)' crates examples`)
- Test: `crates/rl-bevy/src/ability.rs` test module

**Interfaces:**
- Produces: `#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)] pub struct Reflects { pub percent: u8 }`; `impl Default` gives `percent: 50`; `pub const ALWAYS: Reflects` (100) and `pub const NEVER: Reflects` (0); `pub fn percent(percent: u8) -> Reflects`, clamped to 100.
- `Bystanders::reflects` becomes `Query<'w, 's, &'static Reflects>`; the roll is `rng.0.random_bool(f64::from(r.percent.min(100)) / 100.0)`.

- [ ] **Step 1: Write the failing tests** `the_default_reflection_chance_is_one_half` (`Reflects::default().percent == 50`) and `a_percent_past_a_hundred_is_held_to_a_hundred` (`Reflects::percent(250) == Reflects::ALWAYS`).
- [ ] **Step 2:** Run `cargo test -p rl-bevy reflection_chance`.
  Expected: FAIL to compile.
- [ ] **Step 3:** Implement; Undermere and the particles test insert `Reflects::default()`.
- [ ] **Step 4:** Run the two tests, then `scripts/gate.sh`, which plays the three fingerprints.
  Expected: PASS, fingerprints unchanged.
- [ ] **Step 5:** Commit.
  CHANGELOG: "`Reflects` carries the chance it turns a use back, `Reflects { percent }`, 50 by default, with `Reflects::ALWAYS` and `Reflects::NEVER` for a body that always or never does. To change: `insert(Reflects)` becomes `insert(Reflects::default())`."

### Task 2: The reflection tests set the outcome instead of counting it

**Files:**
- Modify: `crates/rl-bevy/src/ability.rs:1391-1620` (seven tests)
- Modify: `crates/rl-bevy/src/items.rs:1808` and `:1933`

Each test below builds one app with `Reflects::ALWAYS` for the turned-back assertions and one with `Reflects::NEVER` for the landed ones, and drops its seed loop and its `both outcomes over N seeds` count.

| Test now | Becomes |
|---|---|
| `a_reflecting_body_turns_a_bolt_back_on_its_caster_about_half_the_time` | `a_reflecting_body_turns_a_bolt_back_on_its_caster_at_its_chance` |
| `a_reflected_bolt_is_seen_to_fly_to_the_body_that_turned_it_and_back_to_its_caster` | same name, two apps |
| `a_reflection_writes_what_turned_back_and_a_landing_writes_none` | same name, two apps |
| `a_reflecting_body_never_turns_back_what_an_ally_aims_at_it` | same name, one app with `ALWAYS` |
| `a_reflected_bolt_lands_its_cells_where_its_caster_stands` | same name, two apps |
| `a_ball_turned_back_by_the_only_body_it_caught_bursts_round_its_caster` | same name, one app with `ALWAYS` |
| `a_body_in_a_burst_turns_back_only_its_own_share` | same name, two apps |
| `watched_an_aimed_use_turned_back_is_seen_to_fly_to_the_body_and_back` (items) | same name, two apps |
| `an_aimed_use_reflects_off_a_reflecting_body_with_no_abilities_plugin_in_the_app` (items) | same name, one app with `ALWAYS`, asserting the user is hurt and the body is not |

- [ ] **Step 1:** Rewrite the tests as the table says; the `ReflectRng` reseeding lines go.
- [ ] **Step 2:** Run `cargo nextest run -p rl-bevy -E 'test(/reflect/)'`.
  Expected: PASS.
- [ ] **Step 3:** Break the roll on purpose (always `false`) and run again.
  Expected: the `ALWAYS` halves FAIL.
  Restore it.
- [ ] **Step 4:** `scripts/gate.sh`, commit.

### Task 3: The spotting tests stop counting finds over seeds

**Files:**
- Modify: `crates/rl-bevy/src/props.rs:2852-2920`

`SpotRules::chance` is unit-tested in `crates/rl-rules/src/prop.rs:705`; the `App` tests only need to show the system asks it with the right distance and waiting.

| Assertion now | Becomes |
|---|---|
| 50% in sight found on 15 to 45 of 60 seeds | dropped; `the_spotting_chance_is_whole_near_half_out_to_far_and_none_beyond_with_keen_added_and_stillness_multiplying` covers it |
| within near, a certainty found on 10 of 10 seeds | one seed |
| past near, found on 15 to 45 of 60 seeds | dropped for the same unit test; the past-far case already shows the distance reaches the rule |
| waited on a hundredfold, found on 20 of 20 seeds | one seed |
| walked past, found on at most 3 of 20 seeds | one seed on which a 1 in 100, `still: 100`, is not found after a step: a `waited` passed wrongly would make it certain on every seed |
| a group found in one roll, over 20 seeds | kept over 4 seeds, not 20, with the reason beside it: any one seed shows a second roll unless both rolls agree; certainty would hide it |

- [ ] **Step 1:** Rewrite as the table says; `found_at_once` takes a single seed or goes if unused.
- [ ] **Step 2:** Run `cargo nextest run -p rl-bevy -E 'test(/secret|spot|keen|waited|group/)'`.
  Expected: PASS.
- [ ] **Step 3:** `scripts/gate.sh`, commit.

### Task 4: Branch end

- [ ] Remove "the reflect chance" from TODO 34 in `docs/TODO.md` and add one line to `docs/PLAN.md`'s progress log.
- [ ] `scripts/gate.sh --all`; compare rl-bevy's processor time with the last run in `rl-test-times.tsv` if branch 01 has merged.
  Rebase, re-gate, merge.
