# Codebase Structure

**Analysis Date:** 2026-09-17

## Directory Layout

```
rl-engine/
├── crates/                    # the workspace: 11 members, tiers 0-3
│   ├── rl-core/                 # tier 0: grid, geometry, seeds, ids, turn queue
│   ├── rl-grid/                 # tier 1: tiles, terrain, FOV, A*, Dijkstra, light
│   │   └── benches/                # criterion: fov, astar, dijkstra, regions, light, field
│   ├── rl-mapgen/                # tier 1: Chain/Pass pipeline, dungeon builders, prefabs
│   ├── rl-world/                 # tier 1: WorldGraph, hydrology, sites, roads, chunks
│   │   └── examples/                # standalone runnable demos of world generation
│   ├── rl-rules/                 # tier 1: stats, damage, statuses, factions, equip
│   │   ├── ai/                      # Tactic trait, TacticCtx, movement profiles
│   │   ├── content/                 # ContentRegistry<T>, BandedWeightedTable<T>, RON loading
│   │   └── events/                  # facts, trigger registry, named counters
│   ├── rl-bevy/                  # tier 2: turn loop, plugins, components, streaming
│   │   └── tests/                   # cross-crate integration: genres.rs, fingerprint.rs
│   ├── rl-render/                # tier 2: GlyphGrid renderer, world view, capture
│   ├── rl-overworld/             # tier 2: opt-in portal-picker screen (single lib.rs, droppable)
│   ├── rl-ui/                    # tier 2: tones, panel views/collectors/presenters, modals
│   │   ├── panel/                   # presenters: draw one view, take a Rect
│   │   └── view/                    # plain-data view structs + collector systems
│   ├── rl-save/                  # tier 2: save backends, versioned schema, entity remap
│   └── rl-engine/                # tier 3: facade, RoguelikePlugins, curated prelude
├── examples/                  # tier 3: the four worked games
│   ├── corsair/                  # open-world pirate roguelike
│   │   └── assets/                  # RON: bestiary, armory, abilities, tasks
│   ├── delve/                    # five-floor dungeon delve, no surface
│   ├── heist/                    # three-floor stealth heist
│   └── tutorial/                 # ten guide-chapter binaries + a browser harness
│       ├── src/bin/                 # step01_walking.rs ... step10_panels.rs
│       └── dist/                    # wasm build output for the browser demos
├── templates/starter/          # the `cargo generate` template: one file, plays on first build
├── docs/
│   ├── PLAN.md                   # the design: decisions, crate layout, milestones, progress log
│   ├── OVERVIEW.md               # the current inventory, kept current per its own header
│   ├── TODO.md                   # outstanding work (never a `TODO` comment in source)
│   ├── design/                   # per-subsystem design docs: ui, lighting, stealth, minds, abilities, fields
│   ├── guide/src/                # the mdBook: 01-a-map-and-walking.md ... 09-where-to-go-next.md
│   ├── reviews/                  # path:line-cited reviews of the three predecessor repos
│   └── images/                   # screenshots referenced by README and the guide
├── scripts/                    # check-tiers.sh, check-guide.sh, check-template.sh, build-demos.sh
└── .planning/                  # GSD planning artifacts (this analysis lives here)
```

## Directory Purposes

**`crates/rl-core/`:**
- Purpose: the Bevy-free vocabulary every other crate shares.
- Contains: one file per concern - `grid.rs`, `point.rs`, `direction.rs`, `geometry.rs`, `disjoint.rs`, `seed.rs`, `dice.rs`, `id.rs`, `stats.rs`, `turn.rs` - plus `lib.rs` re-exporting all of them at the crate root.
- Key files: `crates/rl-core/src/seed.rs` (`RunSeed`, determinism root), `crates/rl-core/src/turn.rs` (`TurnQueue<Id>`).

**`crates/rl-grid/`:**
- Purpose: tile data, terrain queries, and the spatial algorithms (FOV, pathing, light, fields) that read them.
- Contains: `tile.rs`, `terrain.rs`, `fov.rs`, `astar.rs`, `dijkstra.rs`, `bitgrid.rs`, `region.rs`, `spatial.rs`, `targeting.rs`, `field.rs`, `light.rs`.
- Key files: `crates/rl-grid/src/fov.rs` (shadowcast FOV into a bitset), `crates/rl-grid/benches/grid.rs` (the one bench file, six `criterion_group!` entries).

**`crates/rl-mapgen/`:**
- Purpose: the generation pipeline itself, generic over both world passes and dungeon builders.
- Contains: `chain.rs` (`Chain`/`Pass<C>`/`Phase`), `context.rs` (`BuildContext`), `dungeon.rs` (rooms, BSP, doors, start/exit), `prefab.rs` (ASCII prefab stamping).

**`crates/rl-world/`:**
- Purpose: the coarse open-world layer and on-demand chunk generation.
- Contains: `graph.rs` (`WorldGraph`), `hydrology.rs`, `climate.rs`, `elevation.rs`, `noise.rs` (`Fbm`), `sites.rs`, `roads.rs`, `chunk.rs`, `fine.rs` (tile-scale interpolation), `facts.rs`.
- Key files: `crates/rl-world/examples/` holds standalone runnable demos separate from the workspace's `examples/` games - do not confuse the two.

**`crates/rl-rules/`:**
- Purpose: rules shapes (the "engine ships the shape, game ships the instances" split from PLAN section 3.10).
- Contains: `damage.rs`, `status.rs`, `equip.rs`, `affix.rs`, `faction.rs`, `forecast.rs` (the duel-preview math the inspect panel calls), `balance.rs`, `names.rs`, `fire.rs`, `gas.rs`; submodules `ai/` (tactics, movement profiles), `content/` (registries, RON loading), `events/` (facts, quests, counters) - these were folded in from four formerly-separate crates on 2026-09-11 because they weighed the same on the dependency-weight rule.

**`crates/rl-bevy/`:**
- Purpose: the Bevy plugins that run the game: the turn loop, components, messages, streaming.
- Contains: `plugin.rs` (the system-set wiring - read this file first), `turn.rs` (`Action`, `Intent<A>`, `Acting`), `seed.rs` (`Seed`, `Stream`, `AddStream`), `components.rs`, `combat.rs`, `ability.rs` (largest file in the workspace at 1717 lines), `minds.rs` (AI, 1245 lines), `items.rs`, `places.rs`, `world.rs` (chunk streaming), `stealth.rs`, `lighting.rs`, `fire.rs`, `gas.rs`, `doors.rs`, `bump.rs`, `status.rs`, `throwing.rs`, `cue.rs`, `knowledge.rs`, `state.rs`, `registries.rs`, `replay.rs`, `testing.rs` (the `TestApp`/`headless_app` builder).
- Key files: `crates/rl-bevy/tests/genres.rs` (loads five RON content sets differing only in vocabulary into one registry - the theme-agnosticism proof); `crates/rl-bevy/tests/fingerprint.rs`.

**`crates/rl-render/`:**
- Purpose: drawing, owned once so both games and the two glyph games never spawn one entity per tile.
- Contains: `terminal.rs` (`GlyphGrid`, instanced mesh over a font atlas), `map_view.rs`, `looks.rs`, `shade.rs` (lit/remembered/flicker/shimmer), `particles.rs`, `capture.rs` (screenshot + scripted-key playback, refuses a black frame).

**`crates/rl-overworld/`:**
- Purpose: the opt-in coarse-world screen and portal picker.
- Contains: a single `lib.rs` (346 lines) - deliberately one file, since the whole crate is meant to be removable from a game's `Cargo.toml` without touching anything else.

**`crates/rl-ui/`:**
- Purpose: chrome, panels, the tone palette, the modal stack, the message log.
- Contains: `lib.rs` (the crate's own architecture doc - read this before adding a panel), `tone.rs` (`ToneId`/`Palette`), `facet.rs`, `modal.rs`, `log.rs`, `narrate.rs`, `controls.rs`/`keys.rs` (bindings and repeat pacing), `cursor.rs`/`focus.rs` (the look/target cursors), `menu.rs`, `game_menu.rs`, `replay.rs`, `harness.rs` (test-only `Stage` builder); `panel/` (one file per presenter: `vitals.rs`, `gear.rs`, `nearby.rs`, `inspect.rs`, `log.rs`, `scrollback.rs`, `sheet.rs`, `target.rs`, `ability.rs`, `inventory.rs`, `controls.rs`); `view/` (one file per view+collector, same names).
- Key files: `crates/rl-ui/src/lib.rs:5-13` (the view/collector/presenter table), `crates/rl-ui/src/facet.rs`.

**`crates/rl-save/`:**
- Purpose: persistence, independent of any one storage medium.
- Contains: `backend.rs` (`SaveBackend` trait, native/memory/browser backends), `versioned.rs` (schema policy), `remap.rs` (`EntityRemap`, `SaveId`), `engine.rs` (`EngineSave`), `morgue.rs` (`Morgue`/`Obituary`, the death recap `rl-ui` reads), `unload.rs` (the wasm `beforeunload` bridge), `run.rs`.

**`crates/rl-engine/`:**
- Purpose: the one dependency a game adds.
- Contains: a single `lib.rs` re-exporting every crate and defining `RoguelikePlugins` and `prelude`.

**`examples/*` and `templates/starter/`:**
- Purpose: proof the public API is sufficient on its own; every mechanic the engine has is exercised by at least one of `corsair`, `delve`, or `heist` (README's own claim, and borne out by the `TurnSet`/`ResolveSet` hooks each one adds).
- Contains: each game's `main.rs` plus a handful of theme-specific modules (content tables, places/floor population, the one ability effect it adds beyond the engine's shared set); an `assets/` directory of RON files per game.

**`docs/`:**
- Purpose: the design record and the player/contributor-facing guide.
- `docs/PLAN.md`: decisions with citations into `docs/reviews/`, the crate-layout table, and a dated progress log - read the progress log for what actually landed, since the plan is written before the fact and the log after.
- `docs/OVERVIEW.md`: the current inventory, explicitly kept current in the same commit as any system add/remove.
- `docs/design/`: one file per subsystem (`ui.md`, `lighting.md`, `stealth.md`, `minds.md`, `abilities.md`, `fields.md`); `ui.md` section 11b is a model of recording where a build diverged from its own design doc, with reasons.
- `docs/guide/src/`: the mdBook, one chapter per tutorial step; `docs/guide/book/` (git-ignored per this analysis' scope) is the built output.

## Key File Locations

**Entry Points:**
- `crates/rl-engine/src/lib.rs`: the facade; `RoguelikePlugins` is where a new game's `App::new()` starts.
- `examples/corsair/src/main.rs`, `examples/delve/src/main.rs`, `examples/heist/src/main.rs`: the three full games.
- `templates/starter/src/main.rs`: the smallest complete game, generated by `cargo generate`.

**Configuration:**
- `Cargo.toml` (workspace root): members, shared dependency versions, the dev/release opt-level split (`docs/PLAN.md` section 4's "`lwr`'s profile trick").
- `crates/*/Cargo.toml`: each declares `[package.metadata.rl-engine] tier = N`, read by `scripts/check-tiers.sh`.
- `rustfmt.toml`: `max_width = 160`, `use_small_heuristics = "Max"`, `edition = "2024"` - do not hand-wrap lines, `cargo fmt --all --check` is what CI enforces.

**Core Logic:**
- `crates/rl-bevy/src/plugin.rs`: the turn loop and every top-level `SystemSet`; read this before anything else in `rl-bevy`.
- `crates/rl-mapgen/src/chain.rs`: the generation pipeline every map and world builder runs through.
- `crates/rl-rules/src/damage.rs`, `status.rs`: the staged rules mechanics.

**Testing:**
- Inline `#[cfg(test)] mod tests` at the bottom of the file under test, in every crate (the dominant pattern - e.g. `crates/rl-core/src/seed.rs:143-253`).
- `crates/rl-bevy/tests/`: cross-crate integration tests that need more than one module (`genres.rs`, `fingerprint.rs`).
- `crates/rl-grid/benches/grid.rs`: the one criterion bench file in the workspace, covering FOV, A*, Dijkstra, regions, light, and `TileField`. `rl-mapgen` and `rl-world` have no `benches/` directory yet, despite PLAN section 3.9 calling for "each builder, a full world chain at three sizes" from day one - a gap worth flagging if a future phase touches generation performance.
- `crates/rl-ui/src/harness.rs`: a test-only `Stage` builder used across `rl-ui`'s own `#[cfg(test)]` modules to spin up a headless app with a map and an actor.

## Naming Conventions

**Files:**
- One file per major type or concern, named after it in `snake_case`: `seed.rs` holds `RunSeed`/`SeedDomain`, `tone.rs` holds `Tone`/`ToneId`/`Palette`.
- A crate's `panel/` and `view/` subdirectories mirror each other file-for-file (`panel/vitals.rs` ↔ `view/vitals.rs`), which is the visible trace of the view/collector/presenter split.

**Directories:**
- Workspace crates are named `rl-<concern>` (kebab-case for the crate/directory, `rl_<concern>` for the Rust module path - e.g. directory `crates/rl-bevy`, `use rl_bevy::...`).
- A crate that used to be a standalone workspace member and became a module keeps a subdirectory of its old name: `rl-rules/src/ai/`, `content/`, `events/`.

**Tests and tutorials:**
- Tutorial binaries are `stepNN_<name>.rs` under `examples/tutorial/src/bin/`, but the numbering is not contiguous: there is no `step07_*.rs` (guide chapter `07-content-in-files.md` quotes `step08_content.rs`). Do not assume `stepNN` corresponds to guide chapter `NN` when adding a new step - check `docs/guide/src/SUMMARY.md` and the chapter's `<!-- include: -->` line for the actual binary it names.

## Where to Add New Code

**New engine mechanic (algorithm, generation pass, rules shape):**
- Bevy-free logic: the matching tier-1 crate (`rl-grid` for spatial algorithms, `rl-mapgen` for a new `Pass`, `rl-world` for worldgen, `rl-rules` for a rules shape). Add a bench in that crate's `benches/` if the path is hot (PLAN 3.9's rule, even though `rl-mapgen`/`rl-world` don't yet have one).
- Bevy wiring (components, systems, messages): `crates/rl-bevy/src/`, as a new module plus a `Plugin` that declares its `app.needs::<R>()` and slots into `TurnSet`/`DecideSet`/`ResolveSet`/`CleanupSet` by name, never `.after()` a concrete function.

**New UI panel:**
- Follow `crates/rl-ui/src/lib.rs:52-72` literally: a view struct in `view/<name>.rs` (no `Color`, no `Rect`, no game-supplied string), a collector system in `ViewSet::Collect` behind a plugin declaring its `Needs`, any real arithmetic pushed to `rl-rules` and tested without an `App`, then a presenter in `panel/<name>.rs` taking its `Rect` in its constructor and colouring only through `ToneId`/`Palette`.
- Add both files even for a tiny panel - the split is the point, not a formality.

**New action a game resolves itself:**
- Define the action type, implement `Action`, register with `app.add_action::<T>()`, resolve it in `TurnSet::Resolve` (or a sub-stage of `ResolveSet` if it interacts with movement/effects/damage ordering), and claim the actor through `Acting` - follow the pattern in `crates/rl-bevy/src/turn.rs` and the example games' own action modules (e.g. `examples/corsair/src/items.rs`'s item actions).

**New game content (tiles, monsters, statuses, quests, abilities):**
- Data, not code: a RON file under the game's `assets/`, loaded into the matching `Registries`/`ContentRegistry<T>`. Never add a variant to an engine enum - there are none to add to.
- A genuinely new mechanic an existing hook can't express is the one case for engine code; check `docs/design/` for the subsystem first, since PLAN section 3.10's table says which hooks already exist.

**New example game or template:**
- A new top-level directory under `examples/` (a workspace member, `tier = 3` in its `Cargo.toml`) or a fork of `templates/starter/`. Add its RON under its own `assets/`; do not add a fifth theme's vocabulary to any engine crate.

## Special Directories

**`crates/rl-world/examples/`:**
- Purpose: standalone runnable demonstrations of world-generation internals (distinct from the top-level `examples/` games).
- Generated: No.
- Committed: Yes.

**`examples/tutorial/dist/`:**
- Purpose: the wasm/browser build output for the guide's in-browser demos.
- Generated: Yes.
- Committed: check `.gitignore` before assuming; treat as build output regardless.

**`docs/guide/book/`:**
- Purpose: the built mdBook output.
- Generated: Yes.
- Committed: excluded from this analysis' scope per the mapping instructions; treat as generated output, not source.

**`.planning/`:**
- Purpose: GSD planning artifacts, including this document.
- Generated: partially (by GSD tooling).
- Committed: per repository convention - excluded from this analysis' scope.

---

*Structure analysis: 2026-09-17*
