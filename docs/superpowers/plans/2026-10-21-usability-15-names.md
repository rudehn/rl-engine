# Usability 15: One Meaning per Name Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** No public type name is declared twice across `crates/`, "hold" means one thing, the names the review found unguessable say what they are, and the two theme-word types go; a check in the gate keeps it so.

**Architecture:** Mechanical renames the compiler drives, one commit per group, each with its CHANGELOG line so a game migrates by search and replace.
Branch 10 left one declaration of seven of the 26 duplicated names and one fewer `Turn`; this branch renames the other 19 and wires `scripts/check-names.sh` into the gate.
The rename to `atglyph` moved crates and paths only, no type name, so none of this was done by it; 19 duplicates still stand at `86f356ed`.

**Tech Stack:** Rust 2024, bash.

**Spec:** `docs/reviews/engine_usability.md` sections 3.2 and 3.3; `docs/TODO.md` items 15, 27 and 35; index `docs/superpowers/plans/2026-10-21-engine-usability.md`.

## Global Constraints

- The index's "Rules for every branch" bind this branch: worktree `scripts/worktree.sh usability-15-names`, after branches 13 and 14 merge; nothing else runs beside it.
- No fingerprint moves.
- No type keyed in a save by its type name (`crates/atglyph-save/src/run.rs:458`, `short_name`) is renamed; none in the tables is, and the save round-trip tests confirm it.
- No RON spelling changes: only Rust type names move.
- Every rename is breaking: each CHANGELOG line's `To change:` opens with `breaking:` and lists old and new names by the path a game writes (`atglyph::ecs::combat::Reach`).
- Read each type's doc before renaming; if the doc says something the table's name misses, pick a better name and say so in the commit.
- Plain dash, never an em dash.

## Review Focus

1.
  A new name colliding with an existing one (the review found `Trigger`, `Statuses` and `Placement` in the old plan): `scripts/check-names.sh` fails the commit.
2.
  A name re-exported in two preludes after a rename: `cargo test -p atglyph --doc` runs the prelude's collision doc-test.
  `atglyph_bevy::prelude` exports `Reach`, `Strikes`, `Stowed`, `TurnHold`, `HeldWhileWorn`, `Pulse`, `AddMoment` and `Moments`, so the preludes rename with them.
3.
  A doc link to an old name: `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`, as CI runs it, since `scripts/gate.sh --all` builds no docs.
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
  Expected: FAIL listing 19 names (26 before branch 10; 19 at `86f356ed`).
- [ ] **Step 3:** Commit, not yet in the gate.

### Task 2: The duplicates

| Name | Keeps it | Renamed |
|---|---|---|
| `Reach` | `atglyph_rules::arms::Reach` | `atglyph_bevy::combat::Reach` to `DamageRoute`; `atglyph_rules::prefab::Reach` to `BandFallback`; `atglyph_ui::view::target::Reach` to `TargetReach` |
| `Turn` | `atglyph_bevy::Turn` (the schedule) | `atglyph_mapgen::prefab::Turn` to `Rotation` |
| `Throw` | `atglyph_bevy::throwing::Throw` | `atglyph_rules::parts::Throw` to `ThrowDef` |
| `Spent` | `atglyph_bevy::consumable::Spent` | `atglyph_rules::charges::Spent` to `ChargeOutcome` |
| `Source` | `atglyph_bevy::effects::Source` | `atglyph_rules::stats::Source` to `ModifierSource` |
| `Sides` | `atglyph_mapgen::accrete::Sides` | `atglyph_bevy::testing::Sides` to `TwoSides` |
| `Secret` | `atglyph_bevy::secrets::Secret` | `atglyph_mapgen::secrets::Secret` to `SecretCell` |
| `Placed` | `atglyph_rules::loot::Placed` | `atglyph_rules::pool::Placed` to `Laid` |
| `Piece` | `atglyph_mapgen::accrete::Piece` | `atglyph_rules::trade::Piece` to `TradeItem` |
| `Pick` | `atglyph_rules::pool::Pick` | `atglyph_rules::prefab::Pick` to `SlotPick` |
| `Offered` | `atglyph_rules::bump::Offered` | `atglyph_bevy::ability::Offered` to `UsableAbilities` |
| `Look` | `atglyph_rules::prop::Look` | `atglyph_rules::ability::Look` to `AbilityLook` |
| `Line` | `atglyph_core::geometry::Line` | `atglyph_rules::accuracy::Line` to `OddsLine` |
| `Joined` | `atglyph_rules::pool::Joined` | `atglyph_bevy::minds::Joined` to `FollowOrder` |
| `Heard` | `atglyph_bevy::noise::Heard` | `atglyph_ui::narrate::Heard` to `NarratedEvents` |
| `Change` | `atglyph_rules::events::quest::Change` | `atglyph_ui::view::sheet::Change` to `ModifierRow` |
| `Cell` | `atglyph_render::terminal::Cell` | `atglyph_mapgen::prefab::Cell` to `PrefabCell` |
| `Asks` | `atglyph_rules::ability::Asks` | `atglyph_ui::player_keys::Asks` to `KeyAsks` |
| `Area` | `atglyph_mapgen::accrete::Area` | `atglyph_rules::ability::Area` to `EffectArea` |

- [ ] **Step 1:** Rename by crate, lowest tier first, one commit per crate, each with `scripts/gate.sh` and a CHANGELOG line listing old and new names under "To change".
- [ ] **Step 2:** `scripts/check-names.sh`.
  Expected: PASS.
- [ ] **Step 3:** Add it to `scripts/gate.sh` and to `.github/workflows/ci.yml` beside the tier check and the old-names check; commit.

### Task 3: "Hold" means one thing

| Now | After |
|---|---|
| `TurnHold` (`atglyph-bevy/src/cue.rs:117`) | `CueWait` |
| `HoldStart` | kept: the one "hold" left |
| `StatusDef::holds()` (`atglyph-rules/src/status.rs:185`) and its field (`:88`) | `skips_turns()`; the status file's `holds:` key (`Authored`, `status.rs:252`) stays, since no RON spelling changes |
| `holds` on atglyph-bevy's part (`atglyph-bevy/src/parts.rs:141`), statuses held while worn | `worn_statuses`, keeping any RON key with `#[serde(rename = "holds")]` |
| `HeldWhileWorn` (`atglyph-bevy/src/items.rs:875`) | `WornStatuses` |
| `held_for_life` (`atglyph-bevy/src/parts.rs:801`) | `innate_statuses` |

- [ ] **Step 1:** Rename; key repeat's "the engine's hold" in docs becomes "key repeat".
- [ ] **Step 2:** `rg -n 'hold' crates --type rust -i` reads only `HoldStart`, the RON keys kept above, "holds" as plain English, and key words for holding a key.
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
| `Weapons` (`atglyph-ui/src/narrate.rs:551`, a private type alias) | `Wielded` |
| `Weapon` (`atglyph-bevy/src/combat.rs:942`, private) | `ChosenAttack` |

Kept, each a defined term for the glossary: `Loadout`, `Facet`, `Cue`, `Bestows`, `Provenance`, and the place words.

- [ ] **Step 1:** One commit per row group, `scripts/gate.sh` each, with the CHANGELOG line.
- [ ] **Step 2:** `scripts/check-names.sh`, `cargo test -p atglyph --doc`, `scripts/check-template.sh`.
  Expected: PASS.

### Task 5: Branch end

- [ ] `docs/TODO.md`: remove item 15; item 27 loses `Reach` and `Secret`; item 35 loses whatever of `docs/reviews/engine_api.md` section D this branch did; add to section 5 a guide glossary page for the release pass (`MapId` and what hangs off it, the turn sets, cues, facets, loadouts, and every rename here).
  A line each in `docs/PLAN.md`'s progress log.
- [ ] `scripts/gate.sh --all`, rebase, re-gate, merge.
