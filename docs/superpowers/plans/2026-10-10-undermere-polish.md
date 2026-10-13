# Undermere Polish Implementation Plan

**Goal:** Undermere's look, lines and balance brought to a finish, with the engine gaps the game found closed for every game.
**Spec:** `docs/superpowers/specs/2026-10-10-undermere-polish-design.md`.

## How the work runs (parallel by file ownership)

- Each task runs in its own worktree and branch, cut from `main` (`git switch -c <branch> main` inside the agent's own worktree).
- Each task owns the files listed under it; a file outside its list is touched only if the task cannot work without it, and the report names it.
- Shared files: `CHANGELOG.md` is owned by the controller; a task writes its changelog line into its report, and the controller adds the lines at merge. `crates/rl-bevy/src/lib.rs` and the preludes are append-only; the controller resolves their merge.
- Tripwires (every game's fingerprint test) are not in a task's gate: a task runs its crates' tests and the example games' tests with fingerprint failures noted, not re-baselined. The controller re-baselines once per wave, at merge, with the reason.
- Gate per task: `cargo fmt --all --check`; `cargo clippy` on the crates touched with `--all-targets -- -D warnings`; their tests; the example games' tests. Every cargo run in the background. No docs pass (docs wait for a release); doc comments and a changelog line in the report.
- A task with engine changes gets a review before it merges; a game-only task does not, the wave's review covers it.

## Wave A: the engine (starts now; no dependencies between its tasks)

| Task | Branch | Owns |
| --- | --- | --- |
| A1 Light warms and darkens | `polish-light` | `crates/rl-render/src/shade.rs`, `crates/rl-bevy/src/lighting.rs` (light sources), `crates/rl-rules` lighting math if any |
| A2 The player's long action | `polish-long-action` | `crates/rl-bevy/src/work.rs`, the turn hold for the player in `crates/rl-bevy/src/turn.rs` or wherever player input is held, `crates/rl-ui`'s stop key |
| A3 Narration reads right | `polish-narration` | `crates/rl-ui/src/narrate.rs`, `crates/rl-core/src/noun.rs` |
| A4 Uses and flights | `polish-uses` | `crates/rl-bevy/src/items.rs` and `stealth.rs` (an aimed use ends unseen), the flight drawing in `crates/rl-render` (a reflected flight's path) and `crates/rl-bevy/src/ability.rs`'s landing path it reads |

Merge order: A3, A4, A1, A2 (smallest first); each merges `main` in before its fast-forward, and the controller re-baselines fingerprints once after the last.

## Wave B: the game (starts when parts 4 and 6 are on main; depends on wave A merged)

| Task | Branch | Owns | Waits for |
| --- | --- | --- | --- |
| B1 Adopt the engine's half | `polish-adopt` | Undermere's darkness potion effect, `src/captives.rs`, the artifacts' names | wave A |
| B2 Weapons with a manner | `polish-weapons` | Undermere's mace and rapier rows and their effects (a new `src/manners.rs`) | wave A |
| B3 Charms | `polish-charms` | a new `assets/charms.ron` if the item loader takes a second file (else after B1 and B2, in `items.ron`), a new `src/charms.rs` | wave A |
| B4 Balance | `polish-balance` | every RON number, the soak | B1, B2, B3 |
| B5 The look | `polish-look` | whatever the photographs show is wrong; lair tiles in `src/dungeon/lairs.rs` | B4 |

B1, B2 and B3 run side by side; B4 then B5 after them.
`assets/items.ron` is shared by B1 and B2: B1 owns the potion and artifact rows, B2 the mace and rapier rows; each edits only its rows, and the controller resolves the merge.
The wave's whole-part review runs after B5, then one fix wave, then the merge.

## Review Focus

1. A light of negative intensity never makes a tile brighter than unlit, and never darkens the player's own sight of a cell it cannot see.
2. The player's long action never swallows a key the player meant (a stop key always stops it), and a save taken mid-action resumes or drops it cleanly.
3. Nothing the player cannot see is named in the log, by any phrase.
4. A charm's recharge survives a save and is never shortened past one turn.
5. No plan's win rate falls below half the best plan's over 150 delves.

## Progress

Written 2026-10-10 while parts 4 and 6 were built by other sessions, with main at 9735d9e.
Process: lean (engine tasks reviewed, game tasks covered by the wave's review), and parallel by file ownership: each task its own worktree and branch from main, CHANGELOG lines collected by the controller, fingerprints re-baselined once per wave at merge.
Wave A dispatched 2026-10-10: A1 polish-light, A2 polish-long-action, A3 polish-narration, A4 polish-uses (opus, isolated worktrees). Wave B waits for parts 4 and 6 on main and wave A merged.
Wave A interrupted 2026-10-10 at about 22:30: the controlling session and its four agents stopped mid-build, every task's code written and none committed.
Wave A finished 2026-10-11 by another session, one task at a time in the plan's merge order, each rebased onto main, gated (fmt, clippy on the workspace, its crates' tests, every example game's tests) and fast-forwarded: A3 polish-narration c3bbfe2, A4 polish-uses 48cf940, A1 polish-light a418a48, A2 polish-long-action 37d177a. No fingerprint moved, so none was re-baselined. CHANGELOG lines written at each merge; docs/TODO.md items for the coloured light and the player's long action removed.
Fixes made while finishing: A3 a test's expected order, and Undermere's leaving.rs marks a fallen mercenary named as {whom} too; A4 rustfmt only; A2 work_for_player's sight inputs bundled as Lookout for clippy, and pace_player_work moved before EngineSet::Input so a game's exclusive input system needs no order against it (Foundry's ambiguity test).
A2 chose any key, not one key, to stop the player's long action, listed as "any key"; Undermere's waystone then still spent the kneeling's turns with its own `kneel`.
The play notes (`docs/superpowers/plans/2026-10-12-play-notes.md`) merged on 2026-10-12 and changed some of what wave B's balance and look tasks would have: the mercenaries' fees, the bow and arrows, the props' colours and the shops' tag draws. Phase 1 of `docs/reviews/engine_and_games_duplication.md` merged the same day and made the waystone's kneeling and a captive's freeing the engine's long action, which was B1's first task. Wave B is unblocked and should be re-read against main before it starts.
