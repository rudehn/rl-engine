# Undermere Items Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Items are Undermere's only progression and they make builds: enchanting, identification, strength, staffs, wands, rings, potions and scrolls, weapon shapes and curses, on engine seams any game can use.

**Architecture:** Seven engine slices first (a game's typeface; aimed uses; the bag's use-on, unknown and stuck things; shaped and unaware blows; held turns and speed from a stat; ammunition lifted out of Foundry; the farthest exit measured through doors), each in the crate that owns the loop, then the game's systems and content on them.

**Tech Stack:** Rust, Bevy 0.19, `serde`, `ron`, the workspace's own crates.

**Spec:** `docs/superpowers/specs/2026-10-05-undermere-items-design.md`, with `examples/undermere/DESIGN.md` ("Items and builds").

This plan names files, interfaces and the tests to write, and leaves the code to the implementer.
`examples/foundry` and part 1's Undermere are the references for every game-side pattern; `crates/rl-bevy/src/ability.rs` and `crates/rl-ui/src/view/target.rs` for aiming.

## Global Constraints

- Work in `.claude/worktrees/undermere` on branch `undermere`, which is stacked on part 1; never `cd` to the main checkout.
- The constraints of part 1 stand: `.superpowers/sdd/2026-10-02-undermere-skeleton/constraints.md` (copied into this plan's workspace).
- Every engine slice: a test in its crate, doc comments that say why, no theme words, a `CHANGELOG.md` line; Foundry, Corsair, Delve and Heist keep passing, and a fingerprint that moves is explained before it is re-baselined.
- Write in small edits (at most about 120 lines each); a stream silent for ten minutes is stopped.
- The screen can be locked while the user is away; when `RL_CAPTURE` returns black, judge from the painted terminal and list the photographs owed.

## Review Focus

1. A use aimed or chosen and then put away (Escape at the cursor or the bag): nothing is spent and no turn passes. Pinned in tasks 2 and 3.
2. A stuck thing displaced by putting on something that claims its slot, or by a two-hander claiming both hands: refused, not silently removed. Pinned in task 3.
3. A shaped blow next to an ally: a sweep strikes foes only, never the attacker's allies. Pinned in task 4.
4. An unknown thing's name in the log and on every panel before and after it becomes known: never its true name early. Pinned in task 8.
5. Enchanting an item while it is worn: the wearer's armor, attack and charges follow at once, and a save and continue keeps the level. Pinned in task 8.

---

## Tasks

### Task 1: A game's own typeface

**Files:** `crates/rl-render/src/terminal.rs` (the family chosen in `spawn_grid`), `crates/rl-engine/src/lib.rs` (`RoguelikePlugins::typeface`), `examples/undermere/assets/fonts/` (DejaVu Sans Mono and its licence), `examples/undermere/src/main.rs`, `CHANGELOG.md`.
**Interfaces:** `TerminalPlugin::typeface(source)` and `RoguelikePlugins::typeface(path or bytes)`; absent, the system monospace as now.
- [ ] Tests: a terminal given a typeface draws its glyphs from it (the text sections' font is the game's); without one, nothing changes (Foundry's screen tests pass). In Undermere: every glyph the game draws (tiles, props, monsters, items, panels' marks, the title) is one the typeface covers, read from the font file's character map.
- [ ] Download the font (the DejaVu fonts' release on GitHub), check its licence permits bundling, ship the licence beside it.
- [ ] Photograph the town, a floor and the title, and judge the face.
- [ ] Commit: "a game may ship its own typeface, and undermere draws in dejavu sans mono".

### Task 2: A use aimed at somewhere

**Files:** `crates/rl-rules/src/ability.rs` (`TriggerSpec` gains an optional aim), `crates/rl-bevy/src/items.rs` and `effects/triggers.rs` (the use's cell, the footprint), `crates/rl-ui/src/view/target.rs` and `panel/inventory.rs` (`AimUse`), `CHANGELOG.md`.
**Interfaces:** `AimUse { user, item }`; `UseItem` carries an optional cell (or a sibling intent); `Bystanders::land` previews it.
- [ ] Tests: an aimed use lands its effects on the footprint at the chosen cell, not on the user; the preview and the landing agree (a property over seeded layouts, as abilities' is); putting the cursor away spends nothing and passes no turn; an unaimed use is unchanged.
- [ ] Commit: "a thing used from the pack can be aimed, as an ability is".

### Task 3: The bag: a use on another thing, things not yet known, things that will not come off

**Files:** `crates/rl-rules/src/ability.rs` (a trigger asking for a thing), `crates/rl-bevy/src/items.rs` (`UseOn`, `Unknown`, `Stuck`, the refusals), `crates/rl-ui/src/panel/inventory.rs` and `view/inventory.rs`, `view/inspect.rs`, `view/target.rs`, `CHANGELOG.md`.
**Interfaces:** `UseOn { item, on }` and the landing's `on`; `Unknown` and `Stuck` components; `ItemRow` says unknown and stuck.
- [ ] Tests: the bag opens a choosing mode under the trigger's prompt and confirm writes `UseOn`; put away, nothing is spent; an `Unknown` item's row, inspect and banner show no effects and no worth; a `Stuck` item refuses `Unequip`, refuses displacement by an equip and by `EquipFromGround`, and the bag offers no take-off key.
- [ ] Commit: "a thing can be used on another, be not yet known, or refuse to come off".

### Task 4: Shaped blows, and blows on the unaware

**Files:** `crates/rl-bevy/src/combat.rs`, `crates/rl-rules/src/forecast.rs` if the forecast must know a shape, `CHANGELOG.md`.
**Interfaces:** `MeleeAttack::shape`, `RangedAttack::shape` (`Sweep`, `Pierce(n)`); `CombatRules::unaware(percent)`, `MeleeAttack::unaware(percent)`.
- [ ] Tests: a sweep strikes every adjacent foe, each with its own roll, and no ally; a pierce strikes the target and the next cells in line; `Struck` once per swing; an unaware target takes the multiplied blow and an aware one does not; no stealth plugin means no one is unaware.
- [ ] Commit: "a blow can sweep or pierce, and one on the unaware strikes harder".

### Task 5: A status that takes its holder's turns, and speed from a stat

**Files:** `crates/rl-rules/src/status.rs` (`holds`), `crates/rl-bevy/src/status.rs` and the turn loop, `crates/rl-bevy/src/combat.rs` or the turn module (the speed stat), `CHANGELOG.md`.
- [ ] Tests: a held actor's turns pass as waits and its mind is not asked; the player's keys do nothing while held, and the turn passes; held ends when the status does; a speed stat of +100 makes an actor act twice as often, -50 half.
- [ ] Commit: "a status can hold its holder's turns, and a stat can change how fast an actor is".

### Task 6: Ammunition

**Files:** a new `crates/rl-bevy/src/ammo.rs` lifted from `examples/foundry/src/ammo.rs`, `examples/foundry/src/{ammo,plugin,save}.rs`, `CHANGELOG.md`.
- [ ] Tests (moved from Foundry where they test the loop): a shot spends one of the tag from the wielder's bag; none left, the weapon is dry and its shot is not found; reloading on pickup; the rows say dry. Foundry's fingerprint does not move.
- [ ] Commit: "ammunition is the engine's, and foundry spends its slugs through it".

### Task 7: The farthest exit, measured through doors

**Files:** `crates/rl-mapgen/src/dungeon.rs` (`FarthestExit`), `examples/undermere/src/dungeon.rs` (drop its own measuring pass if the engine's now serves), `examples/heist` maps, `docs/TODO.md` (the item leaves), `CHANGELOG.md`.
- [ ] Tests: on a map whose rooms are joined only through doors, the exit is the farthest cell through the doors; Heist's floors over a span of seeds put the way on at least half the map's span from the way in. Heist's and Delve's maps move: say so in the changelog.
- [ ] Commit: "the farthest exit is measured through doors, so a heist's way on is not beside its way in".

### Task 8: Undermere's stats, identification, enchanting and curses

**Files:** `examples/undermere/src/{content,lore,enchant,strength,run,save,armory}.rs`, `assets/lore.ron` (the faces), `CHANGELOG.md` only if the engine changed.
**Interfaces:** `Lore` (`SaveableState`): faces by kind and known kinds; `armory::level_up(item, by)`, re-deriving components from the definition and level; strength requirements and their penalties; `HitRules(Percent)` with accuracy and evasion stats; familiarity, a worn thing's level known after 200 turns.
- [ ] Tests: the Review Focus lines 4 and 5, and the spec's game tests for identification, enchanting, curses and strength.
- [ ] Commit: "undermere's things are unknown until used, enchanting makes them better, the heavy want strength, and the cursed stay on".

### Task 9: Undermere's armory and its effects

**Files:** `examples/undermere/assets/items.ron`, `item_spawns.ron`, `abilities.ron` if a monster's ability moves; `src/effects.rs` (the game's own effects), `src/armory.rs`, `src/population.rs` (the item budget per floor).
- [ ] Tests: every member of the spec's armory loads and does what its row says (a test per kind, not per item, where a kind shares a rule); the game's effects each land and describe themselves; every floor holds a scroll of enchanting over a span of seeds; about fifteen over a run.
- [ ] Commit: "undermere's armory: weapons with shapes, armor with weight, staffs, wands, rings, potions and scrolls".

### Task 10: Play, balance and the pictures

**Files:** whatever playing shows wrong; `examples/undermere/tests/fingerprint.rs` (re-baselined, explained).
- [ ] Play runs to floor five and beyond with different finds, photograph the bag, an unknown potion, a zap, a sweep, a curse, the end screen; fix what reads wrong or plays badly.
- [ ] Commit per fix.

### Task 11: The documentation pass and the full gate

As part 1's task 11b: the system pages for every plugin and type that moved, the overview, the changelog, the README's feature list, `docs/design/items.md` (aimed uses, use on a thing, unknown and stuck things), the combat and statuses pages, `docs/TODO.md`, Undermere's design notes; then everything CI runs.
- [ ] Commit: "docs: aimed and chosen uses, unknown and stuck things, shaped blows, held turns, speed and ammunition, and undermere's armory".
