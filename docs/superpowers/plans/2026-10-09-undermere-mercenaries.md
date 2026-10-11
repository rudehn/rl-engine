# Undermere Mercenaries Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The adventurer walks out of Mereholt with whoever they can pay: candidates at the Sunken Lantern with rolled kits and drawn names, hired for a priced offer, following, fighting, drinking their own potions, wearing better gear, coming down every stair and through the gate, ordered by one key, and remembered by name when they die.

**Architecture:** One engine slice first (the party on the rail), then the game on part 3's minds and part 5's town: the kind and its kit roll, the candidates, hiring and the three brains, the order key, what a mercenary knows, travelling, leaving and death, saving, and the rail wired with photographs.

**Tech Stack:** Rust, Bevy 0.19, `serde`, `ron`, the workspace's own crates.

**Spec:** `docs/superpowers/specs/2026-10-08-undermere-mercenaries-design.md`, with `examples/undermere/DESIGN.md` ("Mercenaries").

This plan names files, interfaces and the tests to write, and leaves the code to the implementer.

## Global Constraints

- Work in `.claude/worktrees/undermere-mercenaries` on branch `undermere-mercenaries`, branched from `main` at `0b8394d` (part 5 merged); never `cd` to the main checkout or another worktree.
- Part 4 is being built at the same time on `undermere-dungeon`; touch `dungeon.rs` and `dungeon/` only where this part must, and say so.
- Every engine slice: a test in its crate, doc comments that say why, no theme words, a `CHANGELOG.md` line; every game keeps passing. The guide and the overview are written at a release, not per branch.
- Lean process: a per-task review for the engine slice only, one fresh agent per task, the touched crates' tests and `clippy -p` during a task, the full gate once at the end, then a whole-part review.
- Commit in stages; keep each tool call short (read about 150 lines, edit about 120 at a time, every long output to a scratchpad file read by its tail).
- The screen may be locked while the user is away: a black capture means judge from the painted terminal in a test and list the photographs owed.

## Review Focus

1. A mercenary never tells the adventurer what they do not know: it uses a found unknown thing only once its kind is known, counts unknown gear at plain numbers, and teaches a kind only by using it in the adventurer's sight. Pinned in task 5.
2. A mercenary is never lost for a reason the player did not choose: it never travels alone, and one left behind waits and follows again. Pinned in task 6.
3. Gold and score stay honest: a fee is paid as part 5's priced offer, never on a step, and a mercenary's kill counts on Slain and nothing else moves. Pinned in tasks 3 and 7.
4. A run's candidates and names are the seed's, distinct across the whole run, through a save. Pinned in tasks 3 and 8.

---

## Tasks

### Task 1: The party on the rail

**Files:** `crates/rl-ui/src/view/party.rs` and `crates/rl-ui/src/panel/party.rs` (new: `PartyView`, its collector in `ViewSet::Collect` from `Leader`, `Health`, `Name` and where each stands; `PartyPanel::new(rect)`), `crates/rl-ui/src/lib.rs`, `CHANGELOG.md`.
- [ ] Tests: a follower on another map has a row marked elsewhere; a non-follower has none; rows keep the order they joined in; the panel draws nothing with nobody following and a health bar in the vitals' tones otherwise; a game's facet shows on a row.
- [ ] Commit: "the party on the rail: who follows the player, how each is, and where".

### Task 2: The kind, the names and the kit roll

**Files:** `examples/undermere/assets/monsters.ron` (the mercenary), `traits.ron` (`mercenary`), `names.ron` (the `mercenary` set with bynames), `mercenaries.ron` (new: fee, parts, levels, with its header comment), `examples/undermere/src/mercenaries.rs` (new: the pure kit roll at a band and the fee from part 5's `Worth`).
- [ ] Tests: the set draws a hundred distinct names on every seed of a range; a kit has a weapon, armor at about four in five and a consumable at about one in two over a range of seeds; a later band reaches rows the first evening's cannot; a fee is its base plus its share of its kit's worth; a level below nought is cursed.
- [ ] Commit: "a mercenary is one kind with a rolled kit and a drawn name, and costs what its kit is worth".

### Task 3: Candidates at the Sunken Lantern

**Files:** `examples/undermere/src/mercenaries.rs` (three candidates a visit from `Seed::stream("tavern", visit)`, standing at the tavern's tables on the `townsfolk` faction, named against `GivenNames`; their kit as look facets with unknown kinds by their face; `hire` as a priced offer at the fee and `talk` saying kit and fee; `hire` taken off every candidate while two follow), `examples/undermere/assets/prefabs/mereholt.ron` (marks at the tables).
- [ ] Tests: three candidates a visit, drawn again on the next visit and the same for the same seed; no name repeats a townsperson's or an earlier candidate's; a bump opens the offers screen and spends nothing; hiring spends the fee, refused when short and absent while two follow.
- [ ] Commit: "three candidates wait at the sunken lantern's tables each visit, each with a kit, a name and a fee".

### Task 4: Hired, the three brains and the order key

**Files:** `examples/undermere/src/mercenaries.rs` (hiring through `allies.rs`, `Leader`, the `Mercenary` component, `give` and `dismiss` offers; the follow, hold and fight-freely brains built once at load; `o` under "Act" cycling every mercenary's order with one log line and no turn; the order as a facet on the party panel and the look panel), `examples/undermere/src/input.rs`.
- [ ] Tests: a follower keeps within three of the adventurer across a floor with nothing to fight; on hold it keeps its post while a foe is in sight beyond reach; fighting freely it chases and comes back; `o` cycles all orders and costs no turn; its name reads as a proper noun in the log.
- [ ] Commit: "a hired mercenary follows, holds or fights freely as one key orders the party".

### Task 5: What a mercenary carries and knows

**Files:** `examples/undermere/src/mercenaries.rs` (`OwnKit`; a `DecideSet::Perceive` system dropping unknown carried things not `OwnKit` and recounting unknown gear in sight at plain numbers; a kind learned when a mercenary uses it in the adventurer's sight; `give` through the pack's chooser).
- [ ] Tests: it drinks its own potion when hurt, unknown or not, and the adventurer learns the kind when in sight and not out of it; it never drinks a found potion not identified; it puts on a better sword lying in sight and passes over an unknown one at plain numbers; a given sword is worn on its next turn; a cursed thing it puts on stays on.
- [ ] Commit: "a mercenary uses its own kit freely and anything else only once the adventurer knows it".

### Task 6: Travelling

**Files:** `examples/undermere/src/mercenaries.rs` (on the adventurer's `Travelled`, every hired mercenary on the map left, alive, not holding, within three cells and in sight is warped beside the landing, nearest first then by entity; one with no free cell stays and says so; one left behind keeps its `Leader` and follows again when the adventurer comes back), with the stairs and the waystone's warp alike.
- [ ] Tests: it comes down the stair from three cells away, and is left behind from four or on hold; it comes through a waystone's warp as through a stair; one left behind follows when the adventurer returns to its floor and leaves again.
- [ ] Commit: "a mercenary goes where the adventurer goes, from beside them, and waits where it was left".

### Task 7: Leaving and death

**Files:** `examples/undermere/src/mercenaries.rs` (`dismiss` asks once, then townsfolk again with its kit dropped, `hire` offered at the fee of what it now carries), `examples/undermere/src/tally.rs` (`mercenaries_lost` and a fallen list; a hired mercenary's kill on the Slain line), the end screen and the records.
- [ ] Tests: dismissing asks, drops the kit and offers `hire` again; a mercenary's death is counted, named on the end screen and in the records; its kill counts on Slain; nothing else in the score moves.
- [ ] Commit: "a dismissed mercenary goes back to the tavern, and a fallen one is named on the end screen".

### Task 8: Saving

**Files:** `examples/undermere/src/save.rs` (`Mercenary`, `OwnKit`, candidates; brains and offers put back on load; the save version raised if older saves would read wrongly).
- [ ] Tests: a hired party survives a save and continue with its leader, orders, offers and own kit; a run saved in the tavern finds the same three candidates; the names a run has given survive.
- [ ] Commit: "a hired party and the tavern's candidates are saved with the run".

### Task 9: The rail, and photographs

**Files:** `examples/undermere/src/main.rs` (the `PartyPanel` in the rail and its order facet), `UNDERMERE_NEAR` taking `candidates`.
- [ ] Tests: the rail shows the party with its order and a follower elsewhere marked so.
- [ ] Photographs: the tavern with its candidates and a look at one; a party of two in a fight on a floor; the end screen naming a fallen mercenary. Read and judge each; fix what looks off.
- [ ] Commit: "undermere's rail shows the party, each with its order".

### Task 10: The full gate, the whole-part review, and the merge

- [ ] The code half of CI: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --no-fail-fast`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`, `scripts/check-tiers.sh --wasm`, `cargo bench --workspace --no-run`.
- [ ] Whole-part review (opus) against the spec and the Review Focus; one fix wave and its re-review.
- [ ] Merge `main` in if it moved, then the fast-forward of `main`, which the user runs.
