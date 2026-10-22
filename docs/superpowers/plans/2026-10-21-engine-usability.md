# Engine Usability: the Branch Index

> **For agentic workers:** this is the index. Each branch has its own plan, executable alone; read the branch's plan and this page's "Rules for every branch".

**Goal:** Act on every recommendation in `docs/reviews/engine_usability.md`, "Ranked recommendations", in seventeen small branches that each keep `main` green.

**Spec:** `docs/reviews/engine_usability.md` at `919f6f82`.
It replaces `2026-10-20-engine-usability.md`, which is removed; the review's "The plan" section says where that one was wrong.

## Rules for every branch

- Cut the branch with `scripts/worktree.sh usability-<nn>-<name>`, never by switching the main checkout.
- Every commit runs `scripts/gate.sh` and adds its `CHANGELOG.md` lines under `Unreleased`, in the existing "what changed" then "To change:" form.
- The branch ends with `scripts/gate.sh --all`, then a rebase on local `main`, a read of what landed since the base, and `scripts/gate.sh --all` again before the fast-forward merge.
- Guide and system pages are not edited; the release's documentation pass works from the CHANGELOG lines.
  The one exception is a file the guide includes by path that a branch renames, whose include is fixed in the same commit.
- A fingerprint moves only in branch 07.
  A fingerprint that moves anywhere else is a bug in that branch, never a re-baseline.
- The fingerprints are the `fingerprint` test binaries of rl-bevy, Undermere and Foundry: filter them with `-E 'binary(fingerprint)'`, never `test(/fingerprint/)`, which misses rl-bevy's.
- `scripts/gate.sh` runs the fast tier of every package the branch touched and of every package depending on one, fingerprints included.
  A step that runs a package's tests or the fingerprints before the gate is for iterating; skip it when the gate follows at once, since Undermere's fast tier alone is about 260 s of processor time.
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
| 17 | `prelude` | 12, 18 | M | 16 | [17](2026-10-21-usability-17-prelude.md) |

## Why this order

- The review's items 1 to 3 and 10 are independent and cheap, so they go first.
- The hello world (06) comes before the seeds (07), so the 18 `PlaceRules` impls (12 in games, 6 in engine tests) change signature once.
- The hello world (06) also moves step 1 and the template to `PlayerKeysPlugin`, since it rewrites those very lines.
- Branch 07 is the one branch that moves play, so it re-baselines Undermere's, Foundry's and rl-bevy's fingerprints once, in one commit.
- Branch 06 hands every place a `PlaceSeed`, but Undermere and Foundry keep their own derivations until 07, so 06 moves no fingerprint.
- The tier-1 content move (10) comes before removing `BuildContext` (11), which rewrites the same `layout.rs`, and before the renames (15), since it leaves seven of the 26 duplicated names declared once.
- The renames (15) run alone because they touch every crate and both games.
- The place model (16) moves after the renames, so it moves final names.
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
| 6 | 11, 12 | `crates/rl-bevy/src/lib.rs` (11 removes the `Charts` exports, 12 adds `ItemRules` and `RevivalRules`) |
| 7 | 13, 14 | - |
| 8 | 15 | - |
| 9 | 16 | - |
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
| 33, one root-and-prelude rule per crate | 17, for `rl-engine`; the per-crate preludes stay open |
| 34, rule numbers in the engine | 02 (reflect chance), 12 (`EQUIP_FROM_GROUND_COST`, `STANDING_ROOM`); the rest stays open |
| 35, the API review's renames | 15 strikes whatever of section D it did |

## Deferred or changed from the review

- **Lean Bevy features, review 2.4 (deferred).** `rl-ui` depends on `rl-render`, whose `terminal.rs` draws with sprites and text, and `TtyRoguelikePlugins` lives in `rl-engine`, which needs the window set; so lean features in `rl-bevy` and `rl-save` free no terminal build.
  They would also make `cargo test -p rl-bevy` resolve Bevy's features differently from the workspace, compiling a second Bevy for every agent iterating on the busiest crate.
  Branch 08 plays in a terminal with the GPU crates compiled but never added.
  Splitting `rl-render`'s cell buffer from its window drawing, then leaning the features, is one follow-up; branch 05 adds it to `docs/TODO.md`.
- **4.6, one test app per engine module.** Not in the ranked table; the review says leave it.
- **Engine-wide ambiguity.** Branch 13 adds an rl-bevy test over every rl-bevy plugin; the same over `rl-ui` and `rl-save` is left to TODO 28's successor if their systems ever conflict.

## Review claims found wrong or incomplete

- **3.4:** Foundry's ambiguity test already compares apps with and without its plugin.
  What names about 45 engine systems is its allow list (21 `pair` entries) and `headless_without_foundry` leaving out engine plugins that `FoundryPlugin` adds.
  Branch 13 is planned on that.
- **1.6:** `Registries` holds only the faction definitions; the relation matrix lives in `CombatRules.factions` and is sized from the registry.
  Branch 09 makes `Factions` a sparse list of named relations that needs no registry, rather than reading `Registries`.
- **1.4:** `TileAppearance` is in `rl-render`, which `rl-bevy` cannot name, so `places` cannot insert it.
  Branch 06 has `MapViewPlugin` fill the standard tiles' looks by name.
- **1.5:** 10 sites write the default `DamageStages`, not 13; Undermere's and Foundry's lists differ from the default and one test inserts an empty list on purpose.
- **2.4:** lean features alone do not free a terminal build, and cost this workspace a second Bevy build (see above).
- **3.2:** eight twins collapse with 2.1, but `Turn` is also the rl-bevy schedule, so 19 names need renaming, not 18.
- **3.6:** the facet filter is near `crates/rl-ui/src/view/nearby.rs:147`; line 415 is test content.
- **The hello world:** the engine also needs the tile tables to build `WorldMap`, so branch 06 adds `PlaceRules::tiles`.

## Review

Reviewed 2026-10-21 against the code at `ba4229c5`; the planner's six corrections of the review (3.4, 1.6, 1.4, 1.5, 3.2, 2.4) all hold.

Changed:

- Every plan filtered fingerprints with `test(/fingerprint/)`, which misses rl-bevy's (`a_seeded_run_of_minds_combat_and_stealth_...`); now `binary(fingerprint)`.
- Steps that ran Undermere's tests or the fingerprints and then `scripts/gate.sh` ran them twice; the gate already covers dependents, so those steps now run the gate alone.
- Branch 05 is cut to tile light: lean Bevy features free no terminal build while `rl-engine` and `rl-render` need the window, and would compile a second Bevy for every `-p rl-bevy` build; deferred with the `rl-render` split.
- Branch 08 no longer waits on 05; `Saves::per_user` returns a `Result`, since a user name is input, and `SavePlugin::for_user` is dropped as redundant.
- Step 1's `PlayerKeysPlugin` moved from 09 to 06, which rewrites the same lines; 09 is now `log-and-factions`.
- Branch 06 missed the sixth engine `PlaceRules` impl (`plugin.rs:1228`) and the engine test rigs that warp with no `Seed`; `places` now declares `needs::<Seed>`.
- Branch 06 had step 1 call `Seed::from_args()`, which does not exist on wasm, where step 1 is a demo; it keeps `Seed(RunSeed(7))`.
- Branch 02 missed `Reflects` in `rl-render/src/particles.rs:901`; the walked-past spotting test becomes one seed and the group test four, not twenty.
- Branch 12 keeps `Which` and `ItemAsk` one concept, and `RevivalRules::standing_room` an `i32` as `STANDING_ROOM` is.
- Branch 13 had Foundry's `main.rs` call `add_engine`; in Undermere's form `FoundryPlugin::build` calls it, and a fingerprint that moves means a pair to order.
- Branch 15 renamed the `holds` field the status file reads, which would change RON; the file key stays, and rl-bevy's part `holds` field and the stale `Weapons`/`Weapon` lines are fixed.
- Branch 17 also makes `depends_on` an `EngineApp` method, review 3.7, which no branch planned.
- Branch 01 keeps the two engine `DamageStages` test inserts, searches hidden files for step names (`.planning/`), and keeps test timings off the CI profile.

Left for the author:

- Branch 16 puts the place model in `rl-rules`; `rl-mapgen` may be the better home, since `PlaceBuild::from_context` reads only mapgen and a map-only user would get places without the rules crate.
- Branch 13 turns each `pair(ours, engine_fn)` into `with_any(ours, data)` as TODO 28 says, which widens it to any system on that data; naming the engine system by its Bevy path string would keep it exact without `pub`.
- Branch 06 moves step 1 to the standard tiles and looks, so chapter 1's screenshots change and chapter 2 introduces custom tiles; the release's guide pass must say so.
- Branch 09 drops `MessageLog::info`, `muted` and `notice`, so an exclusive system writes `push_at(text, Tones::NOTICE, turn)`; keeping the shorthands with a turn is the alternative.
- Branch 07 keeps the tutorial's `seed.rng(RATS, map)` rather than the place's seed, so the tutorial and Undermere key per-place draws two ways.
