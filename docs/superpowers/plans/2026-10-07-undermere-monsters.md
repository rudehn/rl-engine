# Undermere Monsters Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Undermere's monsters are characters: tricks by depth, a look panel that tells what is known and keeps the rest a mystery, mini-bosses with their lairs and artifacts, a floor ten boss drawn from three, and a dungeon that is lived in.

**Architecture:** Six engine slices first (a mind uses what it carries; a status that sleeps; an actor's own triggers; actors that offer verbs; the look panel wraps; following one leader), each in the crate that owns the loop, then the game's armor rule, bestiary, monster knowledge, mini-bosses, boss pool and lived-in floors on them.

**Tech Stack:** Rust, Bevy 0.19, `serde`, `ron`, the workspace's own crates.

**Spec:** `docs/superpowers/specs/2026-10-07-undermere-monsters-design.md`, with `examples/undermere/DESIGN.md` ("Monsters") and the research at `.superpowers/sdd/2026-10-05-undermere-items/research-monsters.md` (copied into this plan's workspace).

This plan names files, interfaces and the tests to write, and leaves the code to the implementer.

## Global Constraints

- Work in `.claude/worktrees/undermere` on branch `undermere`, stacked on part 2; never `cd` to the main checkout.
- Part 2's constraints stand (copied into this plan's workspace as `constraints.md`).
- Every engine slice: a test in its crate, doc comments that say why, no theme words, a `CHANGELOG.md` line; every game keeps passing; a moved fingerprint is explained before it is re-baselined.
- Commit in stages; keep each tool call short (read about 200 lines, edit about 120 at a time, every long output to a scratchpad file): the API stalls long calls.
- The screen may be locked while the user is away: a black `RL_CAPTURE` means judge from the painted terminal and list the photographs owed.

## Review Focus

1. A sleeping monster struck by a sneak attack: it takes the unaware blow and then wakes, in that order. Pinned in tasks 2 and 7.
2. A monster's `died` trigger landing after its body is gone: where it died, once, credited to it. Pinned in task 3.
3. A captive freed while its captors watch: the verb takes its turns, can be broken off by a blow, and the freed captive takes the adventurer's side for good, through a save. Pinned in tasks 4 and 11.
4. A trick the player has not seen: never named on the look panel or in the log before it is shown. Pinned in task 8.
5. A war band whose leader dies: it does not freeze; it falls back to its allies. Pinned in tasks 6 and 9.

---

## Tasks

### Task 1: A mind uses what it carries

**Files:** `crates/rl-rules/src/ai/` (`Snapshot` gains the carried usable things; a `UseCarried` tactic; `Wits::USES`), `crates/rl-bevy/src/minds.rs` and the items snapshot contributor, `CHANGELOG.md`.
- [ ] Tests: a hurt mind with a healing thing drinks it and only when hurt past its threshold; a mind with an aimed thing zaps a foe through `Aim::worth_aiming_at`, refused for exactly what the player's use would be; a mind without `USES` does neither.
- [ ] Commit: "a mind uses what it carries: it drinks to mend when hurt and zaps what it holds at a foe".

### Task 2: A status that sleeps

**Files:** `crates/rl-rules/src/status.rs` (`sleeps`), `crates/rl-bevy/src/status.rs` and the wakers (a noise heard above the sleeper's hearing, a blow landing, a foe noticed close), `CHANGELOG.md`.
- [ ] Tests: a sleeper's turns pass held; a noise loud enough at its cell wakes it and a quiet one does not; a blow wakes it after the blow lands (unaware); noticing a foe within two cells wakes it.
- [ ] Commit: "a status can sleep: it holds its holder's turns until a noise, a blow or a foe close by wakes it".

### Task 3: An actor's own triggers

**Files:** `crates/rl-bevy/src/combat.rs` (`hit` fired on the attacking actor too; `struck`), `crates/rl-bevy/src/effects/triggers.rs` (the `struck` and `died` moments; `died` relayed through `Remnant` as a prop's `destroyed`), `CHANGELOG.md`.
- [ ] Tests: an actor's `hit` trigger lands on its target when its own blow lands; `struck` lands when a blow lands on it; `died` lands where it died, once, credited to it, after its body is gone.
- [ ] Commit: "an actor answers its own moments: its blow landing, a blow on it, and its death".

### Task 4: Actors that offer verbs

**Files:** `crates/rl-bevy/src/props.rs` (the three props-only filters widened; an `Offers` component on an actor), the bump redirect, `crates/rl-ui`'s offers screen if it names props, `CHANGELOG.md`.
- [ ] Tests: an actor beside the one holding the turn offers its verb; `Interact` on it resolves through the same gate and refusals; a bump into a non-hostile actor offering one thing comes to it; a hostile actor's bump is still an attack.
- [ ] Commit: "an actor can offer verbs, as a prop does".

### Task 5: The look panel wraps, and a mind follows one leader

**Files:** `crates/rl-ui/src/panel/inspect.rs` (facets wrapped with `wrap`, a "more" line), `crates/rl-rules/src/ai/tactics.rs` (`Station::Leader(Entity)`, falling back to allies when the leader is gone), `CHANGELOG.md`.
- [ ] Tests: five long facets wrap within the rectangle and a sixth says more; a mind keeps within its distances of its leader, not its nearest ally, and keeps its allies when the leader dies.
- [ ] Commit per change.

### Task 6: Undermere's armor rule and bestiary

**Files:** `examples/undermere/src/content.rs` or a `damage.rs` (armor absorbs at most half a blow), `assets/traits.ron`, `assets/monsters.ron`, `assets/abilities.ron`, `assets/monster_spawns.ron`, `src/bestiary.rs` (actor triggers and offers in the trait model), the game's systems for tricks no effect says (splitting, stealing).
**Numbers:** this plan's content sheet (`content-sheet-monsters.md` in the workspace).
- [ ] Tests: armor never takes more than half a blow; every kind loads with its traits; each new trait does what its row says (a bite poisons, a jelly splits on a blow, a myconid bursts into spores where it died, a monkey takes a thing and runs, a necromancer raises a body as its own); sleepers placed asleep take the unaware blow.
- [ ] Commit in stages.

### Task 7: What is known about a monster

**Files:** `examples/undermere/src/bestiary_lore.rs` (or within `lore.rs`), `src/main.rs` (the look panel's facets), `src/save.rs`.
- [ ] Tests: a monster's plain traits are named on sight; a trick is "?" until its effect lands in the adventurer's sight, then named; knowledge survives a save; the log never names a trick before it shows (Review Focus 4).
- [ ] Commit.

### Task 8: Mini-bosses and their artifacts

**Files:** `assets/prefabs/` (five lairs), `assets/monsters.ron` (five named monsters), `assets/roles.ron` (entourages), `assets/items.ron` (five artifacts), `src/dungeon.rs` (two lairs drawn per run between floors three and nine), `src/tally.rs` (the mini-boss line counts them).
- [ ] Tests: two lairs per run over a span of seeds, never the same twice in a run; each lair builds, holds its boss and entourage (following its leader), and the boss drops its artifact; each artifact does what its row says, cost included; the score's mini-boss line counts a kill.
- [ ] Commit in stages.

### Task 9: The floor ten boss, drawn from three

**Files:** `assets/prefabs/` (three thrones), `assets/monsters.ron`, `src/dungeon.rs` (the run draws one), `src/quest.rs` (killing whichever is drawn wins).
- [ ] Tests: each boss is drawn over a span of seeds; each throne builds and is reachable; killing each wins; the Mother Below's spawnlings stop when she dies.
- [ ] Commit.

### Task 10: A lived-in dungeon

**Files:** `src/population.rs` (sleepers by a share that falls with depth; patrols; camps), `assets/prefabs/` (a goblin camp, a feeding ground), captives (a shackled monster offering "free", freed after a few turns, then an ally following the adventurer).
- [ ] Tests: shares over a span of seeds; a patrol walks its loop; a camp holds a fire, sleepers and a watch; a captive freed follows the adventurer and fights for them, through a save (Review Focus 3).
- [ ] Commit in stages.

### Task 11: Play, balance and the pictures

As part 2's task 10b: the soak over 50 seeds against the new bestiary, tuning with the numbers before and after, real-window photographs of a sleeping camp, a mini-boss lair, each boss, the look panel naming a trick; fixes in further commits; engine faults reported, not fixed.

### Task 12: The documentation pass and the full gate

As part 2's task 11: every page the checker flags, new pages for every new plugin or type that needs one, `docs/design/minds.md` (using what it carries, following a leader), the statuses and props pages and notes (sleeping, actors offering verbs), the effects page (actor moments), the overview, the changelog, the README's feature list, Undermere's design notes; then everything CI runs.
