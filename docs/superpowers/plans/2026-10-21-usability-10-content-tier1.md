# Usability 10: Content Grows Places Without Bevy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `rl-rules` depends on `rl-mapgen`; the eight types declared in both go to one declaration each; the layout translation and the Bevy-free half of the prefab store move from `rl-bevy` to `rl-rules`.

**Architecture:** `rl-mapgen` derives `Deserialize` on its eight types, so `rl-rules`' file types read straight into them and the arm-by-arm translation goes.
`rl-bevy/src/layout.rs` and the plain half of `prefabs.rs` become `rl_rules::grow` and `rl_rules::prefab::PrefabLibrary<M>`; `rl-bevy`'s `Prefabs<M>` stays the resource and derefs to the library.

**Tech Stack:** Rust 2024, serde, ron.

**Spec:** `docs/reviews/engine_usability.md` section 2.1; `docs/TODO.md` item 27; index `docs/superpowers/plans/2026-10-21-engine-usability.md`.

## Global Constraints

- The index's "Rules for every branch" bind this branch: worktree `scripts/worktree.sh usability-10-content-tier1`, after branch 07 merges.
- Every content file in `examples/` and `templates/` loads unchanged: no variant or field is renamed in RON.
- `rl-rules` and `rl-mapgen` stay tier 1, Bevy-free and wasm-clean: `scripts/check-tiers.sh --wasm`.
- No fingerprint moves.
- Plain dash, never an em dash.

## Review Focus

1.
  A file writing `Placement` values the pool never supported (`At`, `InRoom`, `AnyRoom`, which only the mapgen enum has): the pool's loader refuses them by entry name with every other problem; pinned by `a_pool_entry_placed_where_a_pool_cannot_place_is_refused_by_name`.
2.
  A dressing `Thing` from a file has no `stands`: it is `#[serde(skip)]` and filled where the translation filled it; pinned by the dressing tests that moved.
3.
  A RON spelling that changes because serde's derive on the mapgen type differs from the rules twin (`rename_all`, a default): the games' content-loading tests and `scripts/check-template.sh` catch it.
4.
  A default that differs between twins (mapgen `Stairs` has one, the rules twin none): a file that omitted the field before is still refused or still defaulted exactly as before.
5.
  A tool that grows a place without Bevy: pinned by a doc-test on `rl_rules::grow::passes` that compiles against tier-1 crates only.

---

### Task 1: `rl-mapgen`'s eight types deserialize

**Files:**
- Modify: `crates/rl-mapgen/Cargo.toml` (`serde` with `derive`), `crates/rl-mapgen/src/prefab.rs:71,90,424`, `pool.rs:32`, `dungeon.rs:764`, `dress.rs:44,84,155`

| Type | mapgen | rules twin (deleted in Task 2) |
|---|---|---|
| `Turn` | `prefab.rs:71` | `prefab.rs:229` |
| `Mirror` | `prefab.rs:90` | `prefab.rs:241` |
| `Prefer` | `pool.rs:32` | `pool.rs:229` |
| `Placement` | `prefab.rs:424` | `pool.rs:203` |
| `Stairs` | `dungeon.rs:764` | `pool.rs:217` |
| `Spread` | `dress.rs:44` | `dressing.rs:110` |
| `Thing` | `dress.rs:84` | `dressing.rs:132` |
| `DistrictDressings` | `dress.rs:155` | `dressing.rs:282` |

- [ ] **Step 1:** For each pair, read the rules twin's serde attributes and defaults, and put the same on the mapgen type; `Thing::stands` is `#[serde(skip)]`.
- [ ] **Step 2: Write** `each_file_spelling_of_a_turn_mirror_prefer_and_stairs_reads_as_before` in `rl-mapgen`, deserializing every variant from the RON the rules tests use.
- [ ] **Step 3:** `cargo test -p rl-mapgen`, `scripts/check-tiers.sh --wasm`.
  Expected: PASS.
- [ ] **Step 4:** `scripts/gate.sh`, commit.

### Task 2: `rl-rules` reads into them, and the twins go

**Files:**
- Modify: `crates/rl-rules/Cargo.toml` (`rl-mapgen.workspace = true`)
- Modify: `crates/rl-rules/src/{prefab.rs,pool.rs,dressing.rs,lib.rs}`
- Modify: `crates/rl-bevy/src/layout.rs`, `crates/rl-bevy/src/prefabs.rs` (the arm-by-arm translations go)
- Modify: game imports of the twins (`rg -n 'rl_rules::(prefab|pool|dressing)::(Turn|Mirror|Prefer|Placement|Stairs|Spread|Thing|DistrictDressings)' examples templates`)

- [ ] **Step 1: Write the failing test** `a_pool_entry_placed_where_a_pool_cannot_place_is_refused_by_name` in `pool.rs`.
- [ ] **Step 2:** Delete the eight twins; the file types name the `rl_mapgen` types; the pool loader refuses `At`, `InRoom` and `AnyRoom`.
- [ ] **Step 3:** `cargo nextest run --workspace -E 'package(rl-rules) | package(rl-bevy) | package(undermere) | package(foundry) | test(/fingerprint/)'`, `scripts/check-template.sh`.
  Expected: PASS, fingerprints unchanged.
- [ ] **Step 4:** `scripts/gate.sh`, commit.
  CHANGELOG: "`rl-rules` reads prefab, pool and dressing files straight into `rl-mapgen`'s `Turn`, `Mirror`, `Prefer`, `Placement`, `Stairs`, `Spread`, `Thing` and `DistrictDressings`; the copies `rl-rules` declared are gone. To change: an import of one from `rl_rules` names `rl_mapgen` instead; files are unchanged."

### Task 3: The layout translation and the prefab library move to tier 1

**Files:**
- Create: `crates/rl-rules/src/grow.rs` (from `crates/rl-bevy/src/layout.rs`, with its tests)
- Modify: `crates/rl-rules/src/prefab.rs` (`PrefabLibrary<M>`, from `crates/rl-bevy/src/prefabs.rs:104-230`)
- Modify: `crates/rl-bevy/src/prefabs.rs` (`Prefabs<M>` wraps `Arc<PrefabLibrary<M>>` and derefs to it), `crates/rl-bevy/src/lib.rs` (the `layout` module and its re-exports go)
- Modify: `examples/undermere/src/{dungeon.rs,vaults.rs,writings.rs}`, `examples/foundry/src/decks.rs` (`rl_bevy::layout` paths)

**Interfaces:**
- Produces: `pub struct PrefabLibrary<M>` with `new`, `with_dressings`, `dressings`, `defs`, `roles`, `piece`, `grown`, `slot`, the signatures `Prefabs<M>` has today.
- Produces: `rl_rules::grow::{passes, picked, dress, LayoutPasses, LayoutTiles, Picked}`, the signatures `rl_bevy::layout` has today with `&PrefabLibrary<M>` where they took `&Prefabs<M>`.

- [ ] **Step 1:** Move, keeping every test; the moved tests now run without an `App`.
- [ ] **Step 2: Write** a doc-test on `grow::passes` that grows a place from a layout string with only `rl_core`, `rl_grid`, `rl_mapgen` and `rl_rules` in scope.
- [ ] **Step 3:** `cargo test -p rl-rules grow`, `cargo nextest run -p undermere -p foundry`, `scripts/check-tiers.sh --wasm`.
  Expected: PASS.
- [ ] **Step 4:** `scripts/gate.sh`, commit.
  CHANGELOG: "Growing a place from layout, pool and dressing files is tier 1: `rl_bevy::layout` is now `rl_rules::grow`, and what `Prefabs` held is `rl_rules::prefab::PrefabLibrary`, which `Prefabs` derefs to. To change: the paths; a tool or a terminal game grows the same places without Bevy."

### Task 4: Branch end

- [ ] `docs/TODO.md` item 27 keeps only loot as a `BandedTable<K>`, one actor handle, and what branch 15 renames; one line to `docs/PLAN.md`'s progress log.
- [ ] `scripts/gate.sh --all`, rebase, re-gate, merge.
