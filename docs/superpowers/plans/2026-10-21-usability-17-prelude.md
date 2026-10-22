# Usability 17: A Prelude Sized by Use Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `rl_engine::prelude` holds what the first three tutorial steps use and no more, every other name a game uses sits in exactly one topic module, each crate's app extension methods come through one trait, and `rl-bevy` and `rl-render` open with a module map.

**Architecture:** The prelude moves to its own file and lists names explicitly, so a script can count them and find a name exported twice.
`prelude::all` re-exports every module, so a game migrates in one line and narrows later.
The grid `Rect` is renamed `GridRect`, so it can enter the prelude beside Bevy's.

**Tech Stack:** Rust 2024, Python 3 for the check.

**Spec:** `docs/reviews/engine_usability.md` sections 3.1, 3.5, 3.7, 3.8 and "Features with one user"; `docs/TODO.md` item 33; index `docs/superpowers/plans/2026-10-21-engine-usability.md`.

## Global Constraints

- The index's "Rules for every branch" bind this branch: worktree `scripts/worktree.sh usability-17-prelude`, after branch 16 merges; nothing else runs beside it.
- No fingerprint moves.
- No cargo feature and no new crate for the extras: an unused plugin costs nothing at run time.
- Method names of the merged traits do not change; only `use` lines do.
- Plain dash, never an em dash.

## Review Focus

1.
  A game that globs two topic modules: a name in both is an ambiguity error at every use; `scripts/check-prelude.py` fails on any name exported twice.
2.
  A name in a topic module that collides with Bevy's prelude: the prelude doc-test globs `bevy::prelude::*` with `prelude::all::*` and names one type from each module.
3.
  A game that globs only the core and calls `app.add_action`: the core exports `EngineApp`, `UiApp` and `SaveApp`, so every extension method resolves.
4.
  The guide's chapters glob `rl_engine::prelude::*`: each tutorial step compiles with the core plus the module its chapter introduces, and `scripts/check-guide.sh` passes.
5.
  The template, generated outside the workspace, compiles with the new prelude: `scripts/check-template.sh`.

---

### Task 1: One app trait per crate

**Files:**
- Modify: `crates/rl-bevy/src/{seed.rs:81,work.rs:80,effects/engine.rs:495,minds.rs:815,effects/mod.rs:453,consumable.rs:116,effects/triggers.rs:103,478,turn.rs:795,settings.rs:170,props.rs:180,cue.rs:271,plugin.rs:522,753,799,noise.rs:105,places.rs}`
- Modify: `crates/rl-ui/src/{modal.rs:168,controls.rs:710,tone.rs:184}`, `crates/rl-save/src/run.rs:395`
- Modify: every `use` of the old traits

**Interfaces:**
- Produces: `pub trait EngineApp` in `rl_bevy::app`, holding the methods of `AddStream`, `AddWork`, `AddEngineEffects`, `AddChoice`, `AddEffect`, `AddSpending`, `AddTriggerKind`, `KeepOnRemnant`, `AddAction`, `AddSettings`, `AddVerb`, `AddAirborne`, `ResetsOnNewRun`, `Reads`, `Needs`, `AddSound` and `AddPlaces`, each keeping its doc comment; a method no game calls stays `pub(crate)` through a crate-private trait.
- Produces: `pub trait UiApp` (`AddModal`, `AddControls`, `AddTone`) and `pub trait SaveApp` (`AddSaveable`).
- `SpawnPlayer`, on `Commands`, stays its own trait.
- Changes: the free function `rl_bevy::plugin::depends_on::<P>(app, plugin)` (`plugin.rs:865`) becomes `EngineApp::depends_on::<P>(&self, plugin)`, as `needs` and `reads` are methods (review 3.7).

- [ ] **Step 1:** Merge; the old traits go.
- [ ] **Step 2:** `scripts/gate.sh`.
  Expected: PASS.
- [ ] **Step 3:** Commit.
  CHANGELOG: "Each crate's `App` methods come through one trait: `EngineApp`, `UiApp`, `SaveApp`. To change: `use` lines naming `AddAction`, `Needs`, `AddModal`, `AddSaveable` and the rest name the crate's one trait."

### Task 2: `Rect` is `GridRect`

**Files:**
- Modify: `crates/rl-core/src/point.rs:92` and every use (`rg -nw 'Rect' crates examples templates`, telling Bevy's from the grid's by import)
- Modify: `crates/rl-engine/src/lib.rs:186-191` (the note that left it out)

- [ ] **Step 1:** Rename; the 19 example sites that imported it by path drop the path.
- [ ] **Step 2:** `scripts/gate.sh`, `scripts/check-template.sh`, commit.
  CHANGELOG: "`rl_core::Rect` is `GridRect`, and is in the prelude. To change: the name."

### Task 3: The check

**Files:**
- Create: `scripts/check-prelude.py`
- Modify: `scripts/gate.sh`

- [ ] **Step 1:** Write it over `crates/rl-engine/src/prelude.rs`: the core module's explicit names are at most 80; no name appears in two of the core and the topic modules; only `all` may glob.
  It prints each offending name with its modules.
- [ ] **Step 2:** Run it on today's prelude.
  Expected: FAIL, since the prelude is still `lib.rs`'s globs.
- [ ] **Step 3:** Add it to `scripts/gate.sh`'s fast checks once Task 4 passes it.

### Task 4: The prelude by use

**Files:**
- Create: `crates/rl-engine/src/prelude.rs` (from `crates/rl-engine/src/lib.rs:212-224`)
- Modify: every `use rl_engine::prelude::*` in `examples/` and `templates/`

| Module | Holds |
|---|---|
| `prelude` | what steps 1 to 3 and the template name: `RoguelikePlugins`, the turn sets and schedules, `Seed`, `RunSeed`, `PlaceSeed`, `MapId`, `PlaceRules`, `PlaceBuild`, `TileRegistry`, `BaseContext`, `Chain`, `dungeon`, `Position`, `Player`, `Viewshed`, `RevealsMap`, `Glyph`, `Cell`, `PlayerKeysPlugin`, `Log`, `Tones`, `CombatRules`, `Health`, `EngineApp`, `UiApp`, `SaveApp`, `SpawnPlayer`, `GridRect`, and whatever else steps 1 to 3 fail to compile without |
| `prelude::combat` | combat, statuses, damage, abilities, effects, throwing, ammo |
| `prelude::items` | items, loot, consumables, equipment, charges |
| `prelude::minds` | minds, tactics, stealth, noise |
| `prelude::world` | lighting, fire, gas, props, doors, secrets, remains, work, the place model beyond the core |
| `prelude::mapgen` | `rl_mapgen` passes, `rl_rules::grow`, pools, prefabs, dressings |
| `prelude::ui` | panels, views, presenters, palette, modals, cursors |
| `prelude::save` | `rl-save` |
| `prelude::extras` | trade, records, the party and sheet panels, `Lakes`, namegen, threat, `Reflects` |
| `prelude::all` | every module above, by glob |

Plumbing a game never names (`Airborne`, `Lands`, `Marksmanship`, `SideWas` and their like) is in none.

- [ ] **Step 1:** Switch steps 1 to 3 to `use rl_engine::prelude::*;` alone and add names to the core until they compile.
- [ ] **Step 2:** Sort the rest into the topic modules; each later step adds the module its chapter introduces; Undermere and Foundry use `prelude::all::*`.
- [ ] **Step 3:** Update the prelude doc-test: glob `bevy::prelude::*` and `rl_engine::prelude::all::*`, and name one type from each module.
- [ ] **Step 4:** `scripts/check-prelude.py`, `cargo test -p rl-engine --doc`, `scripts/check-guide.sh`, `scripts/check-template.sh`, `scripts/gate.sh`.
  Expected: PASS.
  Put the core's size in the commit message beside the review's 722.
- [ ] **Step 5:** Commit.
  CHANGELOG: "`rl_engine::prelude` holds the names a first game uses; the rest are in `prelude::combat`, `items`, `minds`, `world`, `mapgen`, `ui`, `save` and `extras`, and `prelude::all` is every one. To change: `use rl_engine::prelude::*;` becomes `use rl_engine::prelude::all::*;`, then narrow at leisure."

### Task 5: Module maps

**Files:**
- Modify: `crates/rl-bevy/src/lib.rs:1-11`, `crates/rl-render/src/lib.rs:1-14`

- [ ] **Step 1:** Each crate's docs open with a module map in the form of `crates/rl-rules/src/lib.rs:1-89`: one bullet per group of modules, saying what each is for.
- [ ] **Step 2:** `cargo doc -p rl-bevy -p rl-render --no-deps` with warnings denied, `scripts/gate.sh`, commit.
  No CHANGELOG line.

### Task 6: Branch end

- [ ] `docs/TODO.md` item 33 keeps only the per-crate preludes; a line in `docs/PLAN.md`'s progress log.
- [ ] Re-measure what the review measured (core prelude size, step 1's code lines and engine names, `scripts/check-names.sh`) and append the before and after to `docs/reviews/engine_usability.md`, one line each.
- [ ] `scripts/gate.sh --all`, rebase, re-gate, merge.
