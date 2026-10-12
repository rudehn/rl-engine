# Play notes: fixes, tuning and the rail

> **For agentic workers:** five tasks run at once, each in its own worktree and branch. A task touches only the files it owns below; a file it needs that another task owns is a stop-and-report, not an edit.

**Goal:** the user's notes from playing Undermere (`todo.md`, 2026-10-12): three bugs, five pieces of tuning and content, and three engine UI changes.

**Out of scope, by the user's direction:** the mercenary not taking up a better copy of what it holds; rocks; the straw fire trap; grouping the inventory.

## Rules for every task

- Bugs start by reproducing the bug as a player meets it, in a test that plays the real app (Undermere's `src/testing`, an `App` test, or the capture tool), and the fix ends with that test passing.
- The gate per task is the code gate of what it touched: `cargo fmt --all --check`, `cargo clippy -p <crate> --all-targets -- -D warnings`, `cargo test -p <crate>`. No documentation work: no system pages, no `check-systems.py`, no overview; docs are paid at the release.
- `examples/undermere/tests/fingerprint.rs` and `examples/foundry`'s fingerprint are tripwires every task may move: a task does not re-baseline them; it reports that one moved and why. They are re-baselined once, after the merge.
- `CHANGELOG.md` is owned by nobody: an engine task writes its `Unreleased` line(s) in its final report, and they are added at the merge.
- Each task commits on its own branch, one commit per finished item, with no co-author line.
- The project's rules in `AGENTS.md` hold: no theme words in engine crates, a panel is a view, a collector and a presenter, a widget takes a `ToneId`, what the engine cannot know is a `Facet`.

## Task A: a death is not a notice (engine)

The log reads "You kill the kobold; the kobold notices you!".
`crates/rl-bevy/src/stealth.rs` `wake_on_damage` writes `Noticed { observer: target, .. }` for every `DamageDealt` on an unaware target, the killing blow included, since `Dead` lands only in cleanup.
Fix: a target the blow killed notices nothing; reproduce first with a one-blow kill on an unaware monster and assert no `Noticed` for it, and keep the existing waking of a target that survives.
`examples/foundry/src/plugin/ambiguity.rs` names this system; touch it only if the signature change forces it.
Owns: `crates/rl-bevy/src/stealth.rs`, `examples/foundry/src/plugin/ambiguity.rs`.

## Task B: the inventory by letter, and held keys (engine)

Rows are lettered `a`, `b`, `c` ... in the order they are listed, drawn beside each row; rows past `z` have no letter and are reached by the cursor.
Brogue's two steps, so no key means two things on one screen: on the list, a letter selects its row and opens it, and Enter opens the row under the cursor; an opened row takes the screen's action keys (equip, drop, use, throw, and the rest of `InventoryKeys`) and Esc goes back to the list.
The cursor keys move the selection as now, and held, they repeat at the pace the look cursor repeats at, through `RepeatPace`/`Repeats` in `crates/rl-ui/src/controls.rs` as `cursor.rs` uses them.
Every game with the inventory screen gets this; its `CHANGELOG.md` line says how a player's keys changed.
Owns: `crates/rl-ui/src/panel/inventory.rs`, `crates/rl-ui/src/view/inventory.rs`, `crates/rl-ui/src/controls.rs` (only if a helper is missing), and any engine test of the inventory screen.

## Task C: the nearby rail and the party (engine, then Undermere)

1. The nearby rail lists a prop only when there is something to do with it or about it: it offers a verb, holds things, or is a hidden thing the player has found. A prop that can only be broken is not listed. Tab-cycling and the rest of `InSight` are unchanged.
2. The party panel's health bar spans the row, as the nearby rail's does, rather than a fixed width at its right edge; Undermere's `.bars(4)` in `main.rs` goes or changes to match.
3. What a nearby enemy or a follower carries is shown on its row: Undermere pushes `KitSeen::of`'s words ("wields ..., wears ...") as facets on nearby rows and party rows in `ViewSet::Annotate`, named as the adventurer would know each thing; the engine's presenters show a row's facets if they do not already, wrapping or clipping as the inspect panel does.
Owns: `crates/rl-ui/src/view/nearby.rs`, `crates/rl-ui/src/panel/nearby.rs`, `crates/rl-ui/src/panel/party.rs`, the party view and collector, `crates/rl-ui/src/focus.rs`, `crates/rl-bevy/src/props.rs` (only a predicate on a prop's definition), `examples/undermere/src/main.rs`, `examples/undermere/src/mercenaries/orders.rs`, `examples/undermere/src/mercenaries/tavern.rs`. Register Undermere systems in the mercenaries plugin or `main.rs`, never `plugin.rs`.

## Task D: Undermere's content and tuning

1. **The window** (`dungeon.rs:287`, `TileProps::wall("lit window")`) blocks sight and light. It is impassable and stops projectiles, and is seen and lit through, as the lake's `blocks_projectiles` pattern beside it. Reproduce with a sight test first.
2. **The staff of blinking** aims a bolt of 12 and blinks 2 a level, so the cursor offers cells the blink never reaches. The cursor must never offer a cell the blink will not reach: make the aim's range follow the reach at every level if the targeting can, else make the two one number; the `describe` text and the inventory's aim line agree with what happens.
3. **Arrows are not thrown**, and **the bow is clearly better than throwing**: arrows lose `throw:` and stay found and sold as ammunition; the bow out-damages a javelin and reaches farther, with an accuracy edge, so a bow with arrows is the ranged choice and darts and javelins are the stopgap. Keep `t` throwing what is meant to be thrown.
4. **Mercenaries cost more**: raise `fee` in `mercenaries.ron` so the cheapest candidate on the first evening costs about what the smithy's best tier-one weapon does, and a stacked one several times that; check against the soak's gold estimate (`tests/soak.rs`, `estimated_gold`) and say the numbers chosen.
5. **Props stand out from the floor**: every prop in `props.ron` gets a colour clearly apart from the floors it stands on (planks, flagstone and the rest in `tiles.ron`) and a glyph that is not floor-like (`.`, `,`, `:`, `-`); the table first. A prop meant to be found by looking closely, such as the loose flagstone, may stay subtle, with the reason in the file's comment.
6. **Wares and the mercenary kit draw by tag**: a part's row may name an item or a tag, `(tag: "potion", bands:, weight:)`, drawn through the armory's `LootTable::pick_tagged` at the row's band, read by one function shared by `shops.rs` and `mercenaries/kit.rs`; `wares.ron` moves most shelves to tag rows so a run's stock comes from everything the dungeon can hold, keeping the shelf's no-duplicates rule; each file's top comment lists the new field.
Owns: everything under `examples/undermere/assets/` except `tiles.ron`; `examples/undermere/src/dungeon.rs`, `effects/moving.rs`, `armory.rs` and `armory/`, `finds.rs`, `content.rs`, `shops.rs`, `mercenaries/kit.rs`, `tests/soak.rs`.

## Task E: Undermere reads its map better

1. **Doors stand out**: the door and open door tiles in `tiles.ron` get a look apart from the walls and floors around them.
2. **Doors not yet passed**: Undermere records every door cell the adventurer has stepped through, per floor, saved with the run; a key (free in the controls registry, under "Look") marks, on the map, every explored door not in that record until the next key is pressed, through `rl_ui::cursor`'s `CursorMap::mark` or an overlay of the same kind.
3. **Corpses have their own glyph**: a body left by `RemainsPlugin` is drawn as `%` in a dimmed shade of its kind's colour, so what died is still read off it; put on in `TurnSet::React` on `RemainsLeft` and again when a continued run lays bodies back down; the feeding-ground prey's `REMAINS_GLYPH` in `population.rs` becomes the one constant both use.
Owns: `examples/undermere/assets/tiles.ron`, `examples/undermere/src/input.rs`, `plugin.rs`, `save.rs`, `population.rs`, and new modules for doors and corpses.

## Merge

Each branch merges into one branch, `play-notes`, from `main`, in the order A, B, C, D, E: engine first, then the game on top.
Then the fingerprints are re-baselined once, the `CHANGELOG.md` lines from the reports are added, the full code gate runs (fmt, workspace clippy and tests, `cargo doc`, `scripts/check-tiers.sh` and `--wasm`), one review reads the whole branch, its findings are fixed, and `main` fast-forwards.
