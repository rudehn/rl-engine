# Usability 15: One Meaning per Name Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** No public type name is declared twice across `crates/`, "hold" means one thing, the names the review found unguessable say what they are, and the two theme-word types go; a check in the gate keeps it so.

**Architecture:** Mechanical renames the compiler drives, one commit per group, each with its CHANGELOG line so a game migrates by search and replace.
Branch 10 left one declaration of seven of the 26 duplicated names and one fewer `Turn`; this branch renames the other 19 and wires `scripts/check-names.sh` into the gate.

**Tech Stack:** Rust 2024, bash.

**Spec:** `docs/reviews/engine_usability.md` sections 3.2 and 3.3; `docs/TODO.md` items 15, 27 and 35; index `docs/superpowers/plans/2026-10-21-engine-usability.md`.

## Global Constraints

- The index's "Rules for every branch" bind this branch: worktree `scripts/worktree.sh usability-15-names`, after branches 13 and 14 merge; nothing else runs beside it.
- No fingerprint moves.
- No type keyed in a save by its type name (`crates/rl-save/src/run.rs:458`, `short_name`) is renamed; none in the tables is, and the save round-trip tests confirm it.
- No RON spelling changes: only Rust type names move.
- Read each type's doc before renaming; if the doc says something the table's name misses, pick a better name and say so in the commit.
- Plain dash, never an em dash.

## Review Focus

1.
  A new name colliding with an existing one (the review found `Trigger`, `Statuses` and `Placement` in the old plan): `scripts/check-names.sh` fails the commit.
2.
  A name re-exported in two preludes after a rename: `cargo test -p rl-engine --doc` runs the prelude's collision doc-test.
3.
  A doc link to an old name: `scripts/gate.sh --all` builds the docs with warnings denied.
4.
  A guide anchor in `examples/` whose code renames: the include still resolves (`scripts/check-guide.sh`); its prose waits for the release pass.
5.
  A game's own type with a new engine name (Undermere's `Ware` ruled out `Ware` for trade's `Piece`): `rg -w <new name> examples` before each rename.

---

### Task 1: The check

**Files:**
- Create: `scripts/check-names.sh`

- [ ] **Step 1:** Write it: every `pub (struct|enum|type|trait) Name` under `crates/*/src`, outside `tests/` and `benches/`, grouped by name; it prints every name declared twice with its files and exits 1.
- [ ] **Step 2:** Run it.
  Expected: FAIL listing 19 names (26 before branch 10).
- [ ] **Step 3:** Commit, not yet in the gate.

### Task 2: The duplicates

| Name | Keeps it | Renamed |
|---|---|---|
| `Reach` | `rl_rules::arms::Reach` | `rl_bevy::combat::Reach` to `DamageRoute`; `rl_rules::prefab::Reach` to `BandFallback`; `rl_ui::view::target::Reach` to `TargetReach` |
| `Turn` | `rl_bevy::Turn` (the schedule) | `rl_mapgen::prefab::Turn` to `Rotation` |
| `Throw` | `rl_bevy::throwing::Throw` | `rl_rules::parts::Throw` to `ThrowDef` |
| `Spent` | `rl_bevy::consumable::Spent` | `rl_rules::charges::Spent` to `ChargeOutcome` |
| `Source` | `rl_bevy::effects::Source` | `rl_rules::stats::Source` to `ModifierSource` |
| `Sides` | `rl_mapgen::accrete::Sides` | `rl_bevy::testing::Sides` to `TwoSides` |
| `Secret` | `rl_bevy::secrets::Secret` | `rl_mapgen::secrets::Secret` to `SecretCell` |
| `Placed` | `rl_rules::loot::Placed` | `rl_rules::pool::Placed` to `Laid` |
| `Piece` | `rl_mapgen::accrete::Piece` | `rl_rules::trade::Piece` to `TradeItem` |
| `Pick` | `rl_rules::pool::Pick` | `rl_rules::prefab::Pick` to `SlotPick` |
| `Offered` | `rl_rules::bump::Offered` | `rl_bevy::ability::Offered` to `UsableAbilities` |
| `Look` | `rl_rules::prop::Look` | `rl_rules::ability::Look` to `AbilityLook` |
| `Line` | `rl_core::geometry::Line` | `rl_rules::accuracy::Line` to `OddsLine` |
| `Joined` | `rl_rules::pool::Joined` | `rl_bevy::minds::Joined` to `FollowOrder` |
| `Heard` | `rl_bevy::noise::Heard` | `rl_ui::narrate::Heard` to `NarratedEvents` |
| `Change` | `rl_rules::events::quest::Change` | `rl_ui::view::sheet::Change` to `ModifierRow` |
| `Cell` | `rl_render::terminal::Cell` | `rl_mapgen::prefab::Cell` to `PrefabCell` |
| `Asks` | `rl_rules::ability::Asks` | `rl_ui::player_keys::Asks` to `KeyAsks` |
| `Area` | `rl_mapgen::accrete::Area` | `rl_rules::ability::Area` to `EffectArea` |

- [ ] **Step 1:** Rename by crate, lowest tier first, one commit per crate, each with `scripts/gate.sh` and a CHANGELOG line listing old and new names under "To change".
- [ ] **Step 2:** `scripts/check-names.sh`.
  Expected: PASS.
- [ ] **Step 3:** Add it to `scripts/gate.sh` and to `.github/workflows/ci.yml` beside the tier check; commit.

### Task 3: "Hold" means one thing

| Now | After |
|---|---|
| `TurnHold` (`rl-bevy/src/cue.rs:117`) | `CueWait` |
| `HoldStart` | kept: the one "hold" left |
| `StatusDef::holds()` (`rl-rules/src/status.rs:185`) and its field | `skips_turns()` |
| `HeldWhileWorn` (`rl-bevy/src/items.rs:865`) | `WornStatuses` |
| `held_for_life` (`rl-bevy/src/parts.rs:801`) | `innate_statuses` |

- [ ] **Step 1:** Rename; key repeat's "the engine's hold" in docs becomes "key repeat".
- [ ] **Step 2:** `rg -n 'hold' crates --type rust -i` reads only `HoldStart`, "holds" as plain English, and key words for holding a key.
- [ ] **Step 3:** `scripts/gate.sh`, commit with the CHANGELOG line.

### Task 4: The names that mislead, and the theme words

| Now | After |
|---|---|
| `Moment`, `Moments`, `MomentId`, `AddMoment` | `TriggerKind`, `TriggerKinds`, `TriggerKindId`, `AddTriggerKind` |
| `Afflict`, `Cure`, `Afflicted` | `ApplyStatus`, `RemoveStatus`, `ActiveStatuses` |
| `Strikes` | `BonusDamage` |
| `Among`, `Chooser` | `OfferFilter`, `OfferChooser` |
| `Keen` | `SearchBonus` |
| `Pulse` | `WornTimer` |
| `Stowed` | `Unusable` |
| `Life` | `RevivalTwin` |
| `Doing` | `CurrentTactic` |
| `Weapons` (`rl-ui/src/narrate.rs:376`, private) | `Wielded` |
| `Weapon` (`rl-bevy/src/combat.rs:510`, private) | `ChosenAttack` |

Kept, each a defined term for the glossary: `Loadout`, `Facet`, `Cue`, `Bestows`, `Provenance`, and the place words.

- [ ] **Step 1:** One commit per row group, `scripts/gate.sh` each, with the CHANGELOG line.
- [ ] **Step 2:** `scripts/check-names.sh`, `cargo test -p rl-engine --doc`, `scripts/check-template.sh`.
  Expected: PASS.

### Task 5: Branch end

- [ ] `docs/TODO.md`: remove item 15; item 27 loses `Reach` and `Secret`; item 35 loses whatever of `docs/reviews/engine_api.md` section D this branch did; add to section 5 a guide glossary page for the release pass (`MapId` and what hangs off it, the turn sets, cues, facets, loadouts, and every rename here).
  A line each in `docs/PLAN.md`'s progress log.
- [ ] `scripts/gate.sh --all`, rebase, re-gate, merge.
