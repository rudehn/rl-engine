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
| 05 | `lean-bevy` | 13 | S | - | [05](2026-10-21-usability-05-lean-bevy.md) |
| 06 | `hello-world` | 4 | M | 04 | [06](2026-10-21-usability-06-hello-world.md) |
| 07 | `seeds` | 5 | M | 06 | [07](2026-10-21-usability-07-seeds.md) |
| 08 | `tty` | 14 | M | 05, 06 | [08](2026-10-21-usability-08-tty.md) |
| 09 | `first-steps` | 6 | S | 07 | [09](2026-10-21-usability-09-first-steps.md) |
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
- The hello world (06) comes before the seeds (07), so the 17 `PlaceRules` impls change signature once.
- Branch 07 is the one branch that moves play, so it re-baselines Undermere's, Foundry's and rl-bevy's fingerprints once, in one commit.
- Branch 06 hands every place a `PlaceSeed`, but Undermere and Foundry keep their own derivations until 07, so 06 moves no fingerprint.
- The tier-1 content move (10) comes before removing `BuildContext` (11), which rewrites the same `layout.rs`, and before the renames (15), since it leaves seven of the 26 duplicated names declared once.
- The renames (15) run alone because they touch every crate and both games.
- The place model (16) moves after the renames, so it moves final names.
- The prelude (17) is last, so it exports final names from final homes.

## What runs in parallel

Branches in one row touch no common file except `CHANGELOG.md`, whose conflicts are two added lines, and the file named beside them, where each adds or edits lines the other does not.
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

- **The window half of `rl-render` (new, deferred).** `rl-ui` depends on `rl-render`, whose `terminal.rs` draws with sprites and text, so branch 05 cannot keep the GPU stack out of a terminal build.
  Branch 08 plays in a terminal with those crates compiled but never added.
  Splitting the window half into its own crate is a follow-up once branch 08 shows the compile time matters; 05 adds it to `docs/TODO.md`.
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
- **2.4:** lean features alone do not free a terminal build (see above).
- **3.2:** eight twins collapse with 2.1, but `Turn` is also the rl-bevy schedule, so 19 names need renaming, not 18.
- **3.6:** the facet filter is near `crates/rl-ui/src/view/nearby.rs:147`; line 415 is test content.
- **The hello world:** the engine also needs the tile tables to build `WorldMap`, so branch 06 adds `PlaceRules::tiles`.
