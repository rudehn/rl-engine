# Engine Usability: the Branch Index

> **For agentic workers:** this is the index. Each branch has its own plan, executable alone; read the branch's plan and this page's "Rules for every branch".

**Goal:** Act on every recommendation in `docs/reviews/engine_usability.md`, "Ranked recommendations", and give each game the public API it stands on, in eighteen small branches that each keep `main` green.

**Spec:** `docs/reviews/engine_usability.md` at `919f6f82`.
It replaces `2026-10-20-engine-usability.md`, which is removed; the review's "The plan" section says where that one was wrong.
The engine is `atglyph` since `edd80eb4`; these plans name its crates, and 15 to 18 were checked again against `86f356ed`.

## Status

- 01 `quick-wins`: done.
- 02 `reflection-chance`: done.
- 03 `content-load`: done.
- 04 `play-begins`: done.
- 05 `tile-light`: done.
- 06 `hello-world`: done.
- 07 `seeds`: done.
- 08 `tty`: done.
- 09 `log-and-factions`: done.
- 10 `content-tier1`: done.
- 11 `one-user-generics`: done.
- 12 `rules-out-of-systems`: done.
- 13 `ambiguity`: done.
- 14 `toolkit-fight`: in progress, rebasing onto the rename.
- 15 to 18: not started.

## Rules for every branch

- Cut the branch with `scripts/worktree.sh usability-<nn>-<name>`, never by switching the main checkout.
- Every commit runs `scripts/gate.sh` and adds its `CHANGELOG.md` lines under `Unreleased`, in the existing "what changed" then "To change:" form.
- The branch ends with `scripts/gate.sh --all`, then a rebase on local `main`, a read of what landed since the base, and `scripts/gate.sh --all` again before the fast-forward merge.
- Guide and system pages are not edited; the release's documentation pass works from the CHANGELOG lines.
  The one exception is a file the guide includes by path that a branch renames, whose include is fixed in the same commit.
- A fingerprint moves only in branch 07; the rename's stream commit `ca38f7ab` moved them once more, outside these plans.
  A fingerprint that moves anywhere else is a bug in that branch, never a re-baseline.
- The fingerprints are the `fingerprint` test binaries of atglyph-bevy, Undermere and Foundry: filter them with `-E 'binary(fingerprint)'`, never `test(/fingerprint/)`, which misses atglyph-bevy's.
- `scripts/gate.sh` runs the fast tier of every package the branch touched and of every package depending on one, fingerprints included.
  A step that runs a package's tests or the fingerprints before the gate is for iterating; skip it when the gate follows at once, since Undermere's fast tier alone is about 260 s of processor time.
- A game (Undermere, Foundry, the tutorial, the template) uses only the engine's public, stable API: its own systems, the engine's system sets, component, resource and message types, plugins, and `atglyph::testing`; it never names an engine system function or a private path, and nothing in the engine is public only so a game's test can reach it.
- A game reaches a sub-crate through the facade's module for it: `atglyph::primitives` (`atglyph-core`), `atglyph::ecs` (`atglyph-bevy`), and `atglyph::grid`, `mapgen`, `rules`, `save`, `render`, `ui` and `tty`.
  A CHANGELOG line names a type by the path a game writes.
- A branch cut before the rename runs `python3 scripts/rename-to-atglyph.py` after its rebase, then `--check`, which CI also runs.
- Every engine crate is published, so its public API is held to semver.
  `cargo-semver-checks` runs only at a release (`docs/RELEASING.md` step 3, `python3 scripts/publish.py --semver`), never per branch: `main` keeps the last release's version while it gathers breaking changes, and nothing is published before `0.4.0`.
  So a branch states its semver impact in each CHANGELOG line: `To change: nothing` for an addition, and `To change: breaking:` then the migration for anything a game on the last release must change.
  On `0.x` a breaking line makes the next release a minor bump (`docs/superpowers/specs/2026-10-22-atglyph-crates-io-design.md`, "Versions").
  A `#[doc(hidden)]` item is outside the check, as cargo-semver-checks treats it.
- Commit messages are one long lowercase sentence saying what is now true, with no attribution lines.
- Where a plan's description of the code disagrees with the code, the code wins; the branch's final report says where.
- A worktree is removed and its branch deleted once merged.

## Order

| # | Branch | Review rank | Size | After | Plan |
|---|---|---|---|---|---|
| 01 | `quick-wins` | 1 | S | - | [01](2026-10-21-usability-01-quick-wins.md) |
| 02 | `reflection-chance` | 3 | S | - | [02](2026-10-21-usability-02-reflection-chance.md) |
| 03 | `content-load` | 10 | S | - | [03](2026-10-21-usability-03-content-load.md) |
| 04 | `play-begins` | 2 | S | 01 | [04](2026-10-21-usability-04-play-begins.md) |
| 05 | `tile-light` | 13, half | XS | - | [05](2026-10-21-usability-05-tile-light.md) |
| 06 | `hello-world` | 4 | M | 04 | [06](2026-10-21-usability-06-hello-world.md) |
| 07 | `seeds` | 5 | M | 06 | [07](2026-10-21-usability-07-seeds.md) |
| 08 | `tty` | 14 | M | 06 | [08](2026-10-21-usability-08-tty.md) |
| 09 | `log-and-factions` | 6, two thirds | S | 07 | [09](2026-10-21-usability-09-log-and-factions.md) |
| 10 | `content-tier1` | 7 | M | 07 | [10](2026-10-21-usability-10-content-tier1.md) |
| 11 | `one-user-generics` | 9 | M | 10 | [11](2026-10-21-usability-11-one-user-generics.md) |
| 12 | `rules-out-of-systems` | 17 | M | 09 | [12](2026-10-21-usability-12-rules-out-of-systems.md) |
| 13 | `ambiguity` | 8 | S | 12 | [13](2026-10-21-usability-13-ambiguity.md) |
| 14 | `toolkit-fight` | 15 | S | 11 | [14](2026-10-21-usability-14-toolkit-fight.md) |
| 15 | `names` | 11 | M | 13, 14 | [15](2026-10-21-usability-15-names.md) |
| 16 | `place-model` | 16 | M | 15 | [16](2026-10-21-usability-16-place-model.md) |
| 18 | `game-seams` | public API | M | 15 | [18](2026-10-21-usability-18-game-seams.md) |
| 17 | `prelude` | 12, 18 | M | 16, 18 | [17](2026-10-21-usability-17-prelude.md) |

## Why this order

- The review's items 1 to 3 and 10 are independent and cheap, so they go first.
- The hello world (06) comes before the seeds (07), so the 18 `PlaceRules` impls (12 in games, 6 in engine tests) change signature once.
- The hello world (06) also moves step 1 and the template to `PlayerKeysPlugin`, since it rewrites those very lines.
- Branch 07 is the one branch that moves play, so it re-baselines Undermere's, Foundry's and atglyph-bevy's fingerprints once, in one commit.
- Branch 06 hands every place a `PlaceSeed`, but Undermere and Foundry keep their own derivations until 07, so 06 moves no fingerprint.
- The tier-1 content move (10) comes before removing `BuildContext` (11), which rewrites the same `layout.rs`, and before the renames (15), since it leaves seven of the 26 duplicated names declared once.
- The renames (15) run alone because they touch every crate and both games.
- The place model (16) moves to `atglyph-mapgen` after the renames, so it moves final names; it needs only the serde that branch 10 gives `atglyph-mapgen`.
- The game seams (18) follow the renames (15) and the private systems (13), and come before the prelude, which exports their `PrepareRun`, `RunResumed` and `LaunchArgs`.
- The prelude (17) is last, so it exports final names from final homes.

## What runs in parallel

Branches in one row touch no common file except `CHANGELOG.md`, `docs/PLAN.md`'s progress log and `docs/TODO.md`, whose conflicts are added lines, and the file named beside them, where each adds or edits lines the other does not.
The second of a row to merge resolves the conflict in its rebase.

| Wave | Branches | Shared file to watch |
|---|---|---|
| 1 | 01, 02, 03 | `examples/undermere/src/testing/mod.rs` (01 and 03 each add a function) |
| 2 | 04, 05 | - |
| 3 | 06 | - |
| 4 | 07, 08 | `templates/starter/src/main.rs` (07 a domain constant, 08 the `--tty` front end) |
| 5 | 09, 10 | Undermere's `writings.rs` and `vaults.rs` if they log (09 the log calls, 10 the layout paths) |
| 6 | 11, 12 | `crates/atglyph-bevy/src/lib.rs` (11 removes the `Charts` exports, 12 adds `ItemRules` and `RevivalRules`) |
| 7 | 13, 14 | - |
| 8 | 15 | - |
| 9 | 16, 18 | `crates/atglyph-bevy/src/lib.rs` (16 re-exports the place model, 18 the new schedules and `LaunchArgs`) |
| 10 | 17 | - |

## `docs/TODO.md` items closed

Numbers are the table in "The order" at `919f6f82`.
A branch that closes an item removes it in the commit that finishes it and moves its reasoning to `docs/PLAN.md`'s progress log, as `docs/TODO.md` asks.

| TODO | Closed by |
|---|---|
| 11, allowlist entry in Foundry's ambiguity test | 13 |
| 15, two engine types named for a theme word | 15 |
| 27, one type per concept | 10 (the mirrored enums), 15 (`Reach`, `Secret`); loot as `BandedTable<K>` and one actor handle stay open |
| 28, ambiguity tests through the schedule's interface | 13 |
| 30, continuing a run is the engine's | 18 |
| 33, one root-and-prelude rule per crate | 17, for `atglyph`; the per-crate preludes stay open |
| 34, rule numbers in the engine | 02 (reflect chance), 12 (`EQUIP_FROM_GROUND_COST`, `STANDING_ROOM`); the rest stays open |
| 35, the API review's renames | 15 strikes whatever of section D it did |

## Deferred or changed from the review

- **Lean Bevy features, review 2.4 (deferred).** `atglyph-ui` depends on `atglyph-render`, whose `terminal.rs` draws with sprites and text, and `TtyRoguelikePlugins` lives in `atglyph`, which needs the window set; so lean features in `atglyph-bevy` and `atglyph-save` free no terminal build.
  They would also make `cargo test -p atglyph-bevy` resolve Bevy's features differently from the workspace, compiling a second Bevy for every agent iterating on the busiest crate.
  Branch 08 plays in a terminal with the GPU crates compiled but never added.
  Splitting `atglyph-render`'s cell buffer from its window drawing, then leaning the features, is one follow-up; branch 05 adds it to `docs/TODO.md`.
- **4.6, one test app per engine module.** Not in the ranked table; the review says leave it.
- **Engine-wide ambiguity.** Branch 13 adds an atglyph-bevy test over every atglyph-bevy plugin, and the same in `atglyph-save` or `atglyph-ui` only for the pairs Foundry's list held with their systems; a full sweep of those crates waits until their systems conflict.

## Review claims found wrong or incomplete

- **3.4:** Foundry's ambiguity test already compares apps with and without its plugin.
  What names about 45 engine systems is its allow list (21 `pair` entries) and `headless_without_foundry` leaving out engine plugins that `FoundryPlugin` adds.
  Branch 13 is planned on that.
- **1.6:** `Registries` holds only the faction definitions; the relation matrix lives in `CombatRules.factions` and is sized from the registry.
  Branch 09 makes `Factions` a sparse list of named relations that needs no registry, rather than reading `Registries`.
- **1.4:** `TileAppearance` is in `atglyph-render`, which `atglyph-bevy` cannot name, so `places` cannot insert it.
  Branch 06 has `MapViewPlugin` fill the standard tiles' looks by name.
- **1.5:** 10 sites write the default `DamageStages`, not 13; Undermere's and Foundry's lists differ from the default and one test inserts an empty list on purpose.
- **2.4:** lean features alone do not free a terminal build, and cost this workspace a second Bevy build (see above).
- **3.2:** eight twins collapse with 2.1, but `Turn` is also the atglyph-bevy schedule, so 19 names need renaming, not 18.
- **3.6:** the facet filter is near `crates/atglyph-ui/src/view/nearby.rs:147`; line 415 is test content.
- **The hello world:** the engine also needs the tile tables to build `WorldMap`, so branch 06 adds `PlaceRules::tiles`.

## Review

Reviewed 2026-10-21 against the code at `ba4229c5`; the planner's six corrections of the review (3.4, 1.6, 1.4, 1.5, 3.2, 2.4) all hold.

Changed:

- Every plan filtered fingerprints with `test(/fingerprint/)`, which misses atglyph-bevy's (`a_seeded_run_of_minds_combat_and_stealth_...`); now `binary(fingerprint)`.
- Steps that ran Undermere's tests or the fingerprints and then `scripts/gate.sh` ran them twice; the gate already covers dependents, so those steps now run the gate alone.
- Branch 05 is cut to tile light: lean Bevy features free no terminal build while `atglyph` and `atglyph-render` need the window, and would compile a second Bevy for every `-p atglyph-bevy` build; deferred with the `atglyph-render` split.
- Branch 08 no longer waits on 05; `Saves::per_user` returns a `Result`, since a user name is input, and `SavePlugin::for_user` is dropped as redundant.
- Step 1's `PlayerKeysPlugin` moved from 09 to 06, which rewrites the same lines; 09 is now `log-and-factions`.
- Branch 06 missed the sixth engine `PlaceRules` impl (`plugin.rs:1228`) and the engine test rigs that warp with no `Seed`; `places` now declares `needs::<Seed>`.
- Branch 06 had step 1 call `Seed::from_args()`, which does not exist on wasm, where step 1 is a demo; it keeps `Seed(RunSeed(7))`.
- Branch 02 missed `Reflects` in `atglyph-render/src/particles.rs:901`; the walked-past spotting test becomes one seed and the group test four, not twenty.
- Branch 12 keeps `Which` and `ItemAsk` one concept, and `RevivalRules::standing_room` an `i32` as `STANDING_ROOM` is.
- Branch 13 had Foundry's `main.rs` call `add_engine`; in Undermere's form `FoundryPlugin::build` calls it, and a fingerprint that moves means a pair to order.
- Branch 15 renamed the `holds` field the status file reads, which would change RON; the file key stays, and atglyph-bevy's part `holds` field and the stale `Weapons`/`Weapon` lines are fixed.
- Branch 17 also makes `depends_on` an `EngineApp` method, review 3.7, which no branch planned.
- Branch 01 keeps the two engine `DamageStages` test inserts, searches hidden files for step names (`.planning/`), and keeps test timings off the CI profile.

Decided by the author, 2026-10-21:

- The place model (16) goes to `atglyph-mapgen`, not `atglyph-rules`, with `Lock::key` as the raw tag id `Join::Locked` carries.
- A game's ambiguity allowances (13) use only `Allowed::with_any(ours, data)` or `Allowed::among(sets)`; engine-against-engine pairs stay exact, by `TypeId`, in the engine's own tests.

Flagged by the public-API scan, 2026-10-21:

- Fixed: Foundry's allow list named about 45 engine system functions (13); atglyph-bevy's own ambiguity test no longer copies the games' plugin lists (13); `atglyph_bevy::plugin::headless_app` moves to `atglyph_bevy::testing` and the tutorial's tests use `atglyph::testing` (06, 13).
- Fixed: `Reflects::ALWAYS` and `NEVER` (02) and `TtyPlugin::to_writer` (08) are described as API a game uses, not test hooks.
- Closed by branch 18: the games' hand-written `resume` and `clear_run`, their reads of `replay::seed` and `args`, and an unstated testing surface.
- Step 1 on the standard tiles (06) stands; the release's guide pass covers chapter 1's look.
- The log (09) stays as planned: `info`, `muted` and `notice` on `Log`, and `MessageLog` keeps only `push_at`.
- The tutorial and the template (07) draw per place from the place's own seed, `seed.place(map).rng(DOMAIN, 0)`, as Undermere does.

## Re-checked after the rename, 2026-10-23

Checked against `86f356ed`, after the rename to `atglyph` and the crates.io work.
The rename moved crates, paths, metadata keys, environment variables and stream names, and gave the facade its short modules, `atglyph::bevy` and `atglyph::rand`, and every crate a front page.
It renamed no type and left the prelude and the testing surface as they were, so no task in 15 to 18 was done by it.

- 15: still 19 duplicated names, the same 19; two line numbers moved; doc links are checked with `cargo doc` under `-D warnings`, which `gate.sh --all` does not run.
- 16: the moved types' lines, the warp's stair spawn and every `Lock::key` reader re-pinned; `Ladder::stairs` is crate-private today; the published `atglyph-mapgen` keeps its dependencies.
- 17: every app trait's line re-pinned; 26 `Rect` imports, not 19; the README is a facade doc-test the new prelude must keep compiling; module maps go below the front pages' new "Part of `atglyph`" line.
- 18: the games now import `atglyph::ecs::testing` in 13 files, and the template and the tutorial already use `atglyph::testing`; line ranges re-pinned.
- Every plan's CHANGELOG lines name facade paths and say `breaking:`.

Open:

- Whether 15 to 18 land before the `0.4.0` tag, so a game on 0.3 migrates once and the first published API is the final one.
