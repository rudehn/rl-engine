# Undermere Dungeon Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Undermere's ten floors are grown, not stamped: a biome drawn per floor, a layout plan drawn per biome, wild accretion held to metrics and retried, pools of flavor, risk-and-reward and reward pieces larger than a run, six kinds of hidden thing for the ring of awareness to find, and a dungeon that looks lived in.

**Architecture:** Eight engine slices first, each theme-free and in the crate that owns the loop (a cave shape, barriers, pieces grown from prefabs, lakes and bridges, secrets in two halves, the pool, dressings), then the game moved onto them: biomes as data, a layout file of twenty-nine recipes, lairs and thrones as grown pieces, the three pools, hidden things, and the lived-in pass.

**Tech Stack:** Rust, Bevy 0.19, `serde`, `ron`, `rand`, the workspace's own crates.

**Spec:** `docs/superpowers/specs/2026-10-08-undermere-dungeon-design.md`, with `examples/undermere/DESIGN.md` ("Floors"), `docs/design/layout.md`, `docs/design/prefabs.md`, `docs/design/props.md` section 5, and `docs/research/gridsage-mapgen.md`.

This plan names files, interfaces and the tests to write, and leaves the code to the implementer.

## Global Constraints

- Work in `.claude/worktrees/undermere-dungeon` on branch `undermere-dungeon`; never `cd` to the main checkout, which other sessions use.
- **The base.** This branch sits on `main`, which has both the layout work and part 3, so every task builds on it directly.
  Do not start task 9 on a base without part 3: lairs, the boss draw, captives, egg sacs and sleepers are all part 3's.
- Since 2026-10-07 (the lean process): engine slices, tasks 1 to 8, are reviewed per task by a fresh agent; game tasks are not reviewed per task; the whole part runs the full gate at the end and merges as one.
- Every engine slice: tests in its crate, `#![deny(missing_docs)]` doc comments that say why, no theme words, a `CHANGELOG.md` line under `Unreleased`, every game still passing, and a moved fingerprint explained in its commit before it is re-baselined.
- The agent guide's rules hold throughout: randomness through `RunSeed::derive` and registered streams, functions taking `&mut impl Rng`; no `HashMap` or `HashSet` in generation (`BTreeMap`, `Vec` or `BitGrid`, reason stated); registries and traits, never closed taxonomy enums, in engine crates; every RON file opens with a comment listing its full option space; no `TODO` in source.
- A commit runs the fast gate: `cargo fmt --all --check`, clippy on what it touched, and the tests of what it touched.
- Keep each tool call short (read about 200 lines, edit about 120 at a time, long output to a scratchpad file): the API stalls long calls.
- The screen may be locked while the user is away: a black `RL_CAPTURE` means judge from the painted terminal and list the photographs owed.

## Review Focus

1. A grown piece and the retry: a place that cannot lay a required piece fails and the next attempt of the same seed is a different layout with the same picks. Pinned in tasks 3, 7 and 10.
2. A secret on the route: never. The way down is reachable with every secret shut; every cell is reachable with every secret found. Pinned in tasks 5 and 10.
3. A run's picks are the same whichever floor is built first, and no per-run limit is ever exceeded. Pinned in tasks 7 and 12.
4. A dressing never shuts a door or cuts an area, on a cave as on a hall. Pinned in task 8.
5. Spotting: one stream for props and secrets, nearness, `Keen` and stillness each changing only what they say, and a found secret staying found through a save. Pinned in tasks 6 and 14.

---

## Tasks

### Task 1: A cave is a shape [engine slice, reviewed]

**Files:** `crates/rl-mapgen/src/accrete.rs` or a new `crates/rl-mapgen/src/cave.rs` (`Cave` implementing `Shape`, reusing `passes::CellularCave`'s rule applied at random cells), `crates/rl-rules/src/layout.rs` (`Cave(w:, h:, fill_pct:, visits_pct:)` in the shape kinds, its load checks, the top-of-file comment), `crates/rl-bevy/src/layout.rs` (the kind built into the shape), `CHANGELOG.md`.
- [ ] Tests (`rl-mapgen`, seeds 0..200, at 84 by 48 and 160 by 90): a cave piece's body is one region; every site is rock just outside the body facing out; a recipe of caves alone and a recipe of caves and rectangles both grow connected to their open share or fail; the survey counts cave areas and rates their seclusion.
- [ ] Tests (`rl-rules`): the file loads a cave; a fill outside 1..100, a visit share of nought and a range written backwards are refused by name.
- [ ] Commit: "a cave is a shape: a piece grown by the automaton, kept to its largest region, joined like any other".

### Task 2: A recipe may lay barriers [engine slice, reviewed]

**Files:** `crates/rl-mapgen/src/accrete.rs` (`Accrete` takes barriers; the fit check treats a barrier cell as the border), `crates/rl-rules/src/layout.rs` (`barriers: [(from: (x_pct, y_pct), to: (x_pct, y_pct), chance_pct:, jitter_pct:)]` on a recipe; refused when one reaches edge to opposite edge, or a share is past a hundred), `crates/rl-bevy/src/layout.rs`, `CHANGELOG.md`.
- [ ] Tests: no piece's body or ring crosses a barrier that is present; a barrier at chance nought is never present and at a hundred always; a place with barriers is still connected over seeds; the walk from the start to the far side of a barrier is on average longer than in the same recipe without it.
- [ ] Commit: "a recipe may lay barriers, rock that growth goes round, present at a chance".

### Task 3: A piece grown from a prefab [engine slice, reviewed]

**Files:** `crates/rl-rules/src/prefab.rs` (legend kind `Entry`; refused when not on the outer wall or when a piece has none), `crates/rl-mapgen/src/prefab.rs` (a prefab as a `Shape`: body inside its own wall, sites at its entries, oriented at random), `crates/rl-mapgen/src/accrete.rs` (`Accrete::pieces`, each with its `Join`, laid once the open share passes the recipe's `late_pct`; a place ending growth with one unlaid fails; `Join::Locked(tag)` and `Join::Sealed`), `crates/rl-mapgen/src/dungeon.rs` (`FarthestExit::clear_of_stamps(n)` and `among_farthest(pct)`), `crates/rl-bevy/src/prefabs.rs` and `layout.rs` (pieces handed through; the locked door and sealed gate as the tiles or props the game names), `CHANGELOG.md`.
- [ ] Tests: a grown piece is entered only through its entries, in all eight orientations; its slots fill as a stamped piece's; a recipe asked for a piece it cannot fit fails, and `run_retrying` returns the first attempt that laid it; a locked join opens only for an opener carrying its tag; `FarthestExit` never lands inside a stamp and, with `among_farthest(33)`, lands on different cells across seeds, each within the farthest third.
- [ ] Commit in stages: the legend entry; the shape; `Accrete` laying it; the exit options.

### Task 4: Lakes and bridges [engine slice, reviewed]

**Files:** a new `crates/rl-mapgen/src/lakes.rs` (`Lakes { tile, count, size, wreath: Option<(TileId, pct)> }` in `Phase::Structures`; `Bridges { across, tile, longest, detour }` in `Phase::Connect`), `crates/rl-mapgen/src/lib.rs`, `CHANGELOG.md`.
- [ ] Tests (seeds 0..200, both sizes): the walkable floor is one region after every lake; a wreath only borders its lake; a bridge spans at most `longest` and is laid only where the walk round was at least `detour`; the same seed lays the same lakes after a pass is added to the chain.
- [ ] Commit: "lakes that never cut a place in two, and bridges where walking round is long".

### Task 5: Cells that hide what they are, the generator's half [engine slice, reviewed]

**Files:** a new `crates/rl-mapgen/src/secrets.rs` (`HideOpenings { loops: (lo, hi), rooms: (lo, hi), spot, at_least_rooms }` in `Phase::Finish` after `Survey`; `Secret { at, is, spot, group, joins }` output, the cell walled with the wall tile beside it), `crates/rl-bevy/src/places.rs` (`PlaceBuild::secrets`), `CHANGELOG.md`.
- [ ] Tests (over seeds, on grown places): the way down is walkable from the way in with every secret shut; every open cell is walkable with every secret open; a hidden room's only way in is its secret and it holds neither way; no secret is on the route; counts fall within the bounds asked; a place with no safe opening to hide yields none and does not fail unless rooms were required.
- [ ] Commit: "a place may hide some of its openings: loops' shortcuts and dead ends' doors, never the way on".

### Task 6: Cells that hide what they are, finding them [engine slice, reviewed]

**Files:** a new `crates/rl-bevy/src/secrets.rs` (`Secret` component spawned with the place; found sets the tile, finds its group, despawns), `crates/rl-bevy/src/props.rs` (`spot_hidden_props` becomes one roll over props and secrets; nearness `(near, far)`, `Keen(u8)` on the finder, stillness multiplier, set on `PropsPlugin`; `Found` replaces `Spotted` for both, with `Spotted` kept only if a game reads it), `crates/rl-save` (secrets saved with their map), `docs/design/props.md` section 5 (the refusal of a search key kept, the reasons for stillness), `CHANGELOG.md`.
- [ ] Tests: a secret in sight is found at its chance and becomes its tile, out of sight never; half the chance past `near`, none past `far`; `Keen` adds; a waited turn multiplies and a moved turn does not; one cell of a group found finds all; props and secrets draw from one stream so adding a secret moves no later blow; a found secret stays found and an unfound one stays hidden through a save; a found secret door opens like any door.
- [ ] Commit in stages: secrets as entities and saved; the roll's three changes; `Found`.

### Task 7: The pool [engine slice, reviewed]

**Files:** a new `crates/rl-rules/src/pool.rs` (the file: named weights, entries with name, kind, piece or dressing, floors, where-tags, weight, per-place and per-run limits, group, placement `Grown(join)` / `Inside(shapes)` / `Dressed`, preference `Secluded` / `DeadEnd` / `Hidden` / `Junction` / `Anywhere`; budgets per band and kind with `required`; load checks by name; `Pool::plan_run(rng, places) -> Vec<Vec<Pick>>`, pure), a new `crates/rl-mapgen/src/pool.rs` (`StampPicks`, the inside and dressed picks into areas by preference, reading `Surveyed` and `Secret`; a required pick that fits nowhere fails the place, a flavor pick is dropped), `crates/rl-bevy/src/layout.rs` (a place's picks become `Accrete::pieces` and `StampPicks`), `prefab::coverage` run over pool entries, `CHANGELOG.md`.
- [ ] Tests (`rl-rules`, no `App`, seeds 0..500): no per-run limit exceeded and no group shared; an entry never outside its floors or tags; budgets met whenever the pool can meet them; every entry drawn over the span; the same seed and places plan the same picks; the load refuses an unknown weight name, an unknown piece, a band backwards, a kind with no budget.
- [ ] Tests (`rl-mapgen`): a `Secluded` pick lands in the most secluded area it fits; `Hidden` only behind a secret; `DeadEnd` only in a dead end; a required pick that fits nowhere fails the place, a flavor one does not.
- [ ] Commit in stages: the file; the run plan; the placing pass.

### Task 8: Dressing an area of any size [engine slice, reviewed]

**Files:** a new `crates/rl-rules/src/dressing.rs` (the file: a legend as the prefab's, placement rules `Walls`, `Corners`, `Center`, `Rows { aisle }`, `Scatter`, a density, a repaint `(tile, pct)`; load checks), a new `crates/rl-mapgen/src/dress.rs` (`Dress`, per area no stamp took, a weighted list by district including nothing; slots emitted as a stamp's), `crates/rl-bevy/src/layout.rs`, `docs/TODO.md` (items 24 to 26 struck), `CHANGELOG.md`; Foundry may adopt it later, not here.
- [ ] Tests (seeds 0..200, rectangles, crosses and caves): nothing stands on or beside an opening; every area stays one walkable region with everything standing; one dressing dresses a 5 by 5 room and a 20 by 12 hall to its density within a tolerance, with things along the walls of both; rows leave their aisles; slots fill and save as a prefab's.
- [ ] Commit in stages: the file; the pass.

### Task 9: Biomes as data

**Files:** a new `examples/undermere/assets/biomes.ron` (twelve biomes and the three throne floors: floors, weight, tiles, recipes and weights, ambient and light color, lakes, decor, dressings, favored monsters), `src/dungeon.rs` (the `Biome` enum becomes a loaded `BiomeDef`; `Undermere::biome_of(floor)`, pure over the seed), `src/light.rs` (ambient of the drawn biome), `src/population.rs` (favored kinds at three times their weight), `assets/tiles.ron` and `registry()` (the new biomes' tiles).
- [ ] Tests: each floor draws a biome of its band; over seeds 0..200 every biome is drawn and none takes more than two thirds of its band's floors; the draw is the same whichever floor is built first; a favored kind is drawn about three times as often on its biome's floors; the log names the drawn biome on arrival.
- [ ] Commit in stages.

### Task 10: The layout file and the floor chain

**Files:** a new `examples/undermere/assets/layout.ron` (shapes, districts, twenty-nine recipes per the spec), `src/dungeon.rs` (the floor chain of the spec's "How a floor is grown", `run_retrying` at forty attempts; the painting pass by shape and district; `Rooms`, `Bsp`, `CellularCave`, `JoinCaves`, `Rim`, `ThroneHall` and `Lair` passes retired; `Patches` kept for decor), `src/main.rs` (`--floor N --seed S` prints a floor as glyphs), `tests/fingerprint.rs`.
- [ ] Tests (seeds 0..200): every floor of every biome and recipe builds within forty attempts and meets its recipe; every recipe is drawn; the way down is never inside a piece and meets the recipe's route; every open cell is walkable from the way in with every secret found and lock opened, the way down with none; floor one's layout differs between any two of twenty seeds; the fingerprint re-baselined with the reason in the commit.
- [ ] Print three seeds of every recipe and read them by eye before calling the recipes done (research 4.5); write down what changed.
- [ ] Commit in stages: the file; the chain; the printer.

### Task 11: Lairs and thrones as grown pieces

**Files:** `assets/prefabs/` (the five lairs and three thrones gain `Entry` cells), `src/dungeon.rs` (part 3's lair draw kept; a lair and a throne handed to `Accrete::pieces`; the way in on floor ten drawn far from the throne by `RandomStart` clear of stamps and the route bound).
- [ ] Tests: part 3's lair, boss and population tests pass, unchanged in what they assert; each lair and throne is entered only through its entries; the throne's walk from the way in is at least half the floor's longer side.
- [ ] Commit.

### Task 12: The pool file and the flavor pieces

**Files:** a new `assets/pool.ron` (weights, budgets, every entry of all three pools), a new `assets/dressings.ron` (the flavor and reward dressings, and each biome's room dressings with their share of nothing), `assets/prefabs/flavor/*.ron` (the fixed flavor pieces), `assets/props.ron` (the scenery they stand), `src/prefabs.rs` (the new files compiled in), `src/dungeon.rs` (the run's plan drawn once from the seed).
- [ ] Tests: the pool, the dressings and every piece load; the coverage report finds every slot at every band it may stand at; over seeds 0..500 no run exceeds a limit, every entry appears on some run, and no run sees more than half the flavor pool; a floor's picks are the same whichever floor is built first.
- [ ] Commit in stages.

### Task 13: Risk-and-reward and reward pieces, and what they do

**Files:** `assets/prefabs/risk/*.ron` and `assets/prefabs/reward/*.ron` (the pieces), `assets/props.ron` (cages, pedestals, the seal, the vault door, grates, glyphs, the spring, the anvil, the well), `assets/items.ron` (the vault key), a new `src/vaults.rs` (the mechanisms no effect says: cages shut when one is taken; water rises through grates; the floor gives way and the taker falls to the floor below off its stairs; the seal lifts; glyphs cry out; the circle's dead rise; the gallery collapses behind), effects in `src/effects/` where they are effects.
- [ ] Tests, one a mechanism: taking one caged thing shuts the others; the flood reaches deep water at the vault's far wall within its turns; the rotten floor drops the taker onto an open cell of the floor below, not its stairs; the key opens its own vault and no other; the lever raises its floor's seal; a glyph crossed wakes every sleeper on the floor; the circle's bodies rise when it is crossed; the gallery's way back falls in.
- [ ] Tests (seeds 0..200): every prize is walkable from the way in once its key, lever or secret is found; each piece builds in every biome it may stand in.
- [ ] Commit in stages.

### Task 14: Hidden things

**Files:** `assets/props.ron` (the seven traps, the lever, the loose flagstone), `assets/biomes.ron` (secret counts and chances), a new `src/traps.rs` (traps by depth on corridors, chokepoints and doorways from the survey, never within six of the way in nor on a way through, no trapdoor on floor nine; flagstones; the causeway's group), `src/stats.rs` (`keep_awareness` replaced by `Keen` from the awareness stat), `src/plugin.rs` (nearness two and seven, stillness three, on `PropsPlugin`), the log's words for a find.
- [ ] Tests: counts by depth over seeds; every trap on a walkable cell off the stairs; every hidden thing reachable once the secrets are found; a monster springing a trap in sight shows it; the ring of awareness adds fifteen a level through `Keen`; beside a secret door, a waiting adventurer finds it within twenty turns on at least nine seeds in ten at base chances.
- [ ] Commit in stages.

### Task 15: A lived-in dungeon

**Files:** `src/dungeon.rs` or a new `src/decor.rs` (decor per biome and area: rubble, moss creep, ash, cobwebs in corners, bones, a blood trail from an opening into a dead end ending at remains), a new `assets/writings.ron` (last words, inscriptions, level feelings, with the full option space at its top), `src/lore.rs` or a new `src/writings.rs` (remains carry a line, never one twice a run; inscriptions offer "read"; a floor's feeling logged on arrival), `assets/monsters.ron` and `assets/traits.ron` (moths and blind cave rats, harmless, of no side, worth no score).
- [ ] Tests: decor on every floor at its biome's rates over seeds; a remains line never repeats within a run; reading an inscription logs its line; a floor holding a piece with a feeling logs it on arrival and one without logs none; small life never attacks and never counts in the score.
- [ ] Commit in stages.

### Task 16: Play, balance and the pictures

As part 3's task 11: the soak over fifty seeds against the new floors, counting what is found, what is missed, how many secrets are found and how, and how long a floor takes; retune the chances, budgets and floor sizes with the numbers before and after; real-window photographs of a floor of every biome, a grown vault, a hidden room found, a sprung trap, and a lake with its bridge; fixes in further commits; engine faults reported, not fixed in this task.

### Task 17: The documentation pass and the full gate

Over the branch as a whole, per `CLAUDE.md`:
- [ ] System pages: `docs/guide/src/systems/layout.md` (caves, barriers, grown pieces, the exit's options), `mapgen.md` (lakes and bridges, secrets), `prefabs.md` (`Entry`), `props.md` (secrets, nearness, `Keen`, stillness, `Found`), and a page for the pool and one for dressing if either adds a plugin; each confirmed against the code that moved and blessed with `python3 scripts/check-systems.py --bless <system>`, and `scripts/check-systems-style.sh <page>` run on each.
- [ ] Design notes: `docs/design/layout.md` (sections 2.1, 3 and 9: caves and grown pieces built), new `docs/design/pools.md` and `docs/design/dressing.md` (reasoning and rejected alternatives), `docs/design/props.md` section 5; each new note listed in `CLAUDE.md`'s layout section and in `docs/README.md`.
- [ ] `docs/OVERVIEW.md` (the inventory and the `rl-bevy` plugin table), `CHANGELOG.md` (every line a game on the last release would change), `README.md`'s feature list (caves, pools, secrets, dressings), `docs/TODO.md` (items 24 to 26 struck).
- [ ] `examples/undermere/DESIGN.md`: the floors table replaced by the spec's, "flavour" made "flavor", the pools named with their counts, part 4's entry under "The parts" written as part 3's is.
- [ ] Everything CI runs: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `scripts/check-tiers.sh` and `--wasm`, `scripts/check-guide.sh`, `scripts/check-overview.sh`, `python3 scripts/check-systems.py`.
