<!-- refreshed: 2026-09-17 -->
# Architecture

**Analysis Date:** 2026-09-17

## System Overview

```text
┌──────────────────────────────────────────────────────────────────────┐
│  Tier 3: facade and games                                            │
│  `crates/rl-engine` (facade + prelude), `examples/corsair`,          │
│  `examples/delve`, `examples/heist`, `examples/tutorial`,            │
│  `templates/starter`                                                 │
└───────────────────────────────┬────────────────────────────────────-┘
                                 │ depends on
                                 ▼
┌──────────────────────────────────────────────────────────────────────┐
│  Tier 2: the Bevy layer                                              │
│  `rl-bevy` (turn loop, plugins, components, chunk streaming)         │
│  `rl-render` (glyph grid, world view, capture)                       │
│  `rl-ui` (tones, panel views/collectors/presenters, modals, log)     │
│  `rl-overworld` (opt-in portal-picker screen, droppable)             │
│  `rl-save` (save backends, versioned schema, entity remap)           │
└───────────────────────────────┬────────────────────────────────────-┘
                                 │ depends on
                                 ▼
┌──────────────────────────────────────────────────────────────────────┐
│  Tier 1: Bevy-free algorithms and rules                              │
│  `rl-grid` (tiles, terrain, FOV, A*, Dijkstra, spatial index, light) │
│  `rl-mapgen` (pass pipeline, dungeon builders, prefabs)              │
│  `rl-world` (world graph, hydrology, sites, roads, chunk gen)        │
│  `rl-rules` (stats, damage, statuses, factions, equip, ai, events,   │
│  content registries, balance) - modules: `ai/`, `content/`,          │
│  `events/`                                                            │
└───────────────────────────────┬────────────────────────────────────-┘
                                 │ depends on
                                 ▼
┌──────────────────────────────────────────────────────────────────────┐
│  Tier 0: the core                                                    │
│  `rl-core`: `Grid<T>`, `Point`, `Rect`, `Direction`, geometry,        │
│  `DisjointSet`, `RunSeed` + `SeedDomain` + derive, `Dice`, `Id<T>`,   │
│  `TurnQueue<Id>`                                                     │
└──────────────────────────────────────────────────────────────────────┘
```

Verified against `cargo metadata --no-deps --format-version 1` on 2026-09-17: every workspace member declares `[package.metadata.rl-engine] tier`, the declared tiers match the table above and the README's, and `scripts/check-tiers.sh` (which reads tiers from `cargo metadata`, not from itself) passes clean: `ok: no crate depends on a higher tier` and every tier-0/1 crate plus `rl-save` reports `ok: <crate> is bevy-free` under `--wasm`.

`rl-ui`'s `Cargo.toml` (`crates/rl-ui/Cargo.toml:19-33`) lists `rl-mapgen` and `rl-world` only under `[dev-dependencies]`, for view tests that need a real generated map; `scripts/check-tiers.sh`'s `jq` filter explicitly excludes dev-dependencies (`select(.kind == null or .kind == "build")`), so this is a deliberate, correctly-scoped exception, not a boundary leak.

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| `TurnQueue<Id>` | integer-clock scheduling, generic over actor id, no ECS | `crates/rl-core/src/turn.rs` |
| `RunSeed` / `SeedDomain` | one root seed, named per-subsystem streams, position/pair hashes for seams | `crates/rl-core/src/seed.rs` |
| `TileRegistry` / `Terrain` / `OpacitySource` / `CostSource` | tile data as opaque ids, terrain queries, FOV and pathing traits | `crates/rl-grid/src/tile.rs`, `crates/rl-grid/src/terrain.rs` |
| `fov::compute`, `AStar`, `DijkstraMap` | shadowcast FOV into a bitset, A* with optional turn cost, multi-goal flood | `crates/rl-grid/src/fov.rs`, `crates/rl-grid/src/astar.rs`, `crates/rl-grid/src/dijkstra.rs` |
| `Chain` / `Pass<C>` / `BuildContext` | the generation pipeline, per-pass RNG streams keyed by pass name | `crates/rl-mapgen/src/chain.rs`, `crates/rl-mapgen/src/context.rs` |
| `WorldGraph`, hydrology, sites, roads | the coarse open-world layer and chunk generation | `crates/rl-world/src/graph.rs`, `crates/rl-world/src/hydrology.rs`, `crates/rl-world/src/chunk.rs` |
| `Registries`, `ContentRegistry<T>`, `BandedWeightedTable<T>` | data-driven content loaded from RON, validated at load | `crates/rl-rules/src/content.rs`, `crates/rl-rules/src/content/` |
| Damage pipeline, `StatusDef`, `Tactic`, factions | staged, game-composed rules mechanics | `crates/rl-rules/src/damage.rs`, `crates/rl-rules/src/status.rs`, `crates/rl-rules/src/ai.rs`, `crates/rl-rules/src/faction.rs` |
| `CorePlugin`, `TurnSet`/`EngineSet`/`PresentSet` | the turn loop, the frame's system-set wiring, `Requirements`/`Needs` | `crates/rl-bevy/src/plugin.rs` |
| `Seed`, `Stream`, `AddStream` | the run's seed resource and per-plugin derived RNG streams | `crates/rl-bevy/src/seed.rs` |
| `Action` trait, `Intent<A>`, `Acting` | actions as types a game registers, not a closed enum | `crates/rl-bevy/src/turn.rs` |
| `GlyphGrid`, `Renderable`, `Cell` | the instanced glyph renderer and the world view | `crates/rl-render/src/terminal.rs`, `crates/rl-render/src/map_view.rs` |
| `View` structs, collectors, presenters | panel data, the systems that refill it, the systems that draw it | `crates/rl-ui/src/view/`, `crates/rl-ui/src/panel/` |
| `Facet`, `Facets` | the escape hatch for what the engine cannot know about a game's content | `crates/rl-ui/src/facet.rs` |
| `Tone`, `ToneId`, `Palette` | the interned semantic colour registry every widget indexes by | `crates/rl-ui/src/tone.rs` |
| `SaveBackend`, `Versioned`, `EntityRemap` | pluggable save storage, schema versioning, stable ids across a save/restore | `crates/rl-save/src/backend.rs`, `crates/rl-save/src/versioned.rs`, `crates/rl-save/src/remap.rs` |
| `RoguelikePlugins` | the facade's minimal always-on group: window, `CorePlugin`, `FovPlugin`, map view, `UiPlugin`, capture | `crates/rl-engine/src/lib.rs` |

## Pattern Overview

**Overall:** a Bevy ECS application composed from opt-in plugins, layered over a Bevy-free algorithmic and rules core, with content (tiles, monsters, statuses, quests) supplied entirely by the game as data loaded into registries.

**Key Characteristics:**
- Every subsystem beyond the always-on `CorePlugin` is a plugin a game names explicitly; nothing turns itself on because a resource happens to exist.
- Determinism is structural: one `RunSeed`, named domains, `StdRng` everywhere gameplay draws, so a seed replays identically on native and wasm.
- Extension is by registry-plus-trait, never by enum variant: `TileId`, `StatusId`, `FactionId`, `DamageTypeId` are opaque ids the game populates from RON; `Pass<C>`, `Tactic`, `OpacitySource`, `CostSource`, `Renderable`, `SaveBackend` are traits taken as parameters.
- The UI is deliberately split into three temporal phases per frame (collect, annotate, present) so the reusable half (queries) and the game-specific half (vocabulary, colour) never touch the same code.

## Layers

**Tier 0 - `rl-core`:**
- Purpose: geometry, ids, seeds and the pure turn queue; the vocabulary every other crate shares.
- Location: `crates/rl-core/src/`
- Contains: `Grid<T>`/`Grid2D`, `Point`, `Rect`, `Direction`/`DirectionSet`, Bresenham and other geometry, `DisjointSet`, `RunSeed`/`SeedDomain`, `Dice`, `Id<T>` interning, quantile stats, `TurnQueue<Id>`.
- Depends on: `rand` only.
- Used by: every other crate in the workspace.

**Tier 1 - `rl-grid`, `rl-mapgen`, `rl-world`, `rl-rules`:**
- Purpose: everything that must run headless, test in milliseconds, and build for wasm: map data, generation, pathfinding, FOV, lighting math, and rules shapes.
- Location: `crates/rl-grid/src/`, `crates/rl-mapgen/src/`, `crates/rl-world/src/`, `crates/rl-rules/src/` (with `ai/`, `content/`, `events/` submodules - these were once separate crates, folded in on 2026-09-11 because they weighed the same as `rl-rules` on the dependency-weight rule).
- Contains: tile registry and terrain, shadowcast FOV, A*, `DijkstraMap`, `TileField<T>`, the `Chain`/`Pass<C>` generation pipeline, `WorldGraph` and chunk generation, the damage/status/equip/faction/ai/content/events rules layer.
- Depends on: `rl-core`, and each other in the direction the table in README/PLAN section 4 states (`rl-mapgen` and `rl-rules` depend on `rl-grid`; `rl-world` depends on `rl-mapgen`).
- Used by: tier 2 exclusively; never depends on Bevy, checked by `scripts/check-tiers.sh --wasm`.

**Tier 2 - `rl-bevy`, `rl-render`, `rl-ui`, `rl-overworld`, `rl-save`:**
- Purpose: the Bevy plugins that turn the tier-1 shapes into a running game: the turn loop, rendering, chrome, the opt-in portal-picker screen, and persistence.
- Location: `crates/rl-bevy/src/`, `crates/rl-render/src/`, `crates/rl-ui/src/`, `crates/rl-overworld/src/`, `crates/rl-save/src/`.
- Contains: `CorePlugin` and the turn schedule, components (`Position`, `Actor`, `Viewshed`, ...), chunk streaming, mutation messages, the `GlyphGrid` renderer, panel views/collectors/presenters, the tone palette, save backends and the versioned schema.
- Depends on: tier 0 and 1 crates, `bevy`.
- Used by: `rl-engine` (tier 3) and the example games through it; nothing in tier 2 depends on `rl-overworld`, confirmed by `grep -rl rl_overworld crates/rl-bevy/src crates/rl-render/src crates/rl-ui/src crates/rl-save/src` returning nothing, which is what makes it genuinely droppable per PLAN section 3.5.1.

**Tier 3 - `rl-engine` and the games:**
- Purpose: the facade a game depends on, and the games that prove the API is enough on its own.
- Location: `crates/rl-engine/src/lib.rs`, `examples/corsair`, `examples/delve`, `examples/heist`, `examples/tutorial`, `templates/starter`.
- Contains: `RoguelikePlugins` (the minimal always-on group), a curated `prelude`, and every crate re-exported so a game writes one dependency line.
- Depends on: everything.
- Used by: nothing; this is the leaf.

## Data Flow

### The turn loop (one frame, `crates/rl-bevy/src/plugin.rs:15-118`, `:197-215`)

`EngineSet` orders one frame: `Stream` (chunk streaming) → `Input` (read the player's keys once) → `Turns` (run the `Turn` schedule) → `Light` → `Fov` → `Present`. `run_turns` (`crates/rl-bevy/src/plugin.rs:136-151`) repeats the `Turn` schedule - up to `MAX_PASSES = 512` - until the player holds a turn, a pass changes nothing, or a `TurnHold` is raised, so one player step costs one frame no matter how many actors are due.

Inside one `Turn` pass, `TurnSet` chains six stages:

1. `TurnSet::Schedule` - deal a turn (`turn::start_pass`, `turn::schedule`, `crates/rl-bevy/src/plugin.rs:225`).
2. `TurnSet::Decide` - sub-staged as `DecideSet::Notice → Offer → Perceive → Minds → Game` (`plugin.rs:205`); `DecideSet::Perceive` further sub-stages as `PerceiveSet::Begin → Roster → Filter → Annotate` (`plugin.rs:207`) and only runs `.run_if(a_mind_holds_the_turn)` (`plugin.rs:206`).
3. `TurnSet::Resolve` - sub-staged as `ResolveSet::Redirect → Travel → Act → Fields → Effects → Damage` (`plugin.rs:208-213`), with `FieldSet::Fire → Gas` chained inside `ResolveSet::Fields` (`plugin.rs:214`). Games add their own action resolvers here, claiming the actor through `Acting`.
4. `TurnSet::Sweep` - refuses any `Intent` no resolver claimed, so a forgotten resolver is a loud warning rather than a frozen game.
5. `TurnSet::React` - where a game answers what the turn caused: loot drops, floor population on first entry, status infliction. Runs inside the pass, so effects land before the next actor acts; anything that scans the world every frame belongs in `PresentSet::Narrate` instead.
6. `TurnSet::Cleanup` - sub-staged `CleanupSet::Remove → Requeue` (`plugin.rs:215`): dead actors leave the queue and the occupancy index before requeuing, so nothing killed this pass is dealt another turn.

**Actions are types, not an enum.** `Action` is a trait, `Intent<A>` the message carrying one; `Step`, `Wait`, `Bump`, `Swap`, `GoThrough`, `Open`, `Close` are the engine's, registered with `add_action::<T>()`; a game adds its own (`Attack`, item actions) the same way and resolves them in `TurnSet::Resolve` alongside the engine's (`crates/rl-bevy/src/turn.rs`, PLAN progress log, 2026-09-11 entry on "actions stop being an enum").

### The view/present pipeline (`crates/rl-ui/src/lib.rs:127-152`, `crates/rl-bevy/src/plugin.rs:46-56, 203`)

`ViewSet::Collect → Annotate → Speak` runs every frame inside `PresentSet::Narrate`, itself the first of four ordered layers inside `EngineSet::Present`: `PresentSet::Narrate → Map → Chrome → Overlay`.

1. `ViewSet::Collect` - the engine's collector systems rebuild every panel's view (a plain-data resource: `Row`s, `Bar`s, numbers, an `Entity` for hover) from the world, every frame rather than on a turn boundary (PLAN's `docs/design/ui.md` section 11b: a frame already rewrites the whole map, so a stale-by-one-frame panel was judged a worse bug than the cost of recollecting).
2. `ViewSet::Annotate` - the game pushes what the engine cannot know: a wielded weapon, an AI state, onto a row as a `Facet { key, text, tone }` (`crates/rl-ui/src/facet.rs`).
3. `ViewSet::Speak` - the narrator turns typed `Said` events into log lines, after the game has had its say about the rows.
4. `PresentSet::Map`, `PresentSet::Chrome`, `PresentSet::Overlay` - drawing proper: the glyph grid, the status/log chrome, then whatever covers it (overworld screen, menus, modals).

Every panel is authored in three layers, in this order (`crates/rl-ui/src/lib.rs:52-72`): a view struct (no `Color`, no `Rect`, no game-supplied string) in `view/`; a collector system in `ViewSet::Collect` behind a plugin that declares its resource needs with `Needs::needs`; rules arithmetic pushed down to `rl-rules` where it is tested without an `App` (`rl_rules::forecast`, used by the inspect panel at `crates/rl-ui/src/view/inspect.rs:22`); a presenter plugin in `panel/` that takes its `Rect` in its constructor and colours only by `ToneId` through the `Palette`.

## Key Abstractions

**Plugins and `Needs`/`app.needs` (`crates/rl-bevy/src/plugin.rs:451-509`):**
- Purpose: make "is this subsystem on" a fact about which plugins were added, never about whether a resource happens to exist.
- Examples: `CorePlugin::needs::<WorldMap>(...)` (`plugin.rs:242`); `InspectViewPlugin::needs::<Registries>(...)` (`crates/rl-ui/src/view/inspect.rs:74`).
- Pattern: a plugin calls `app.needs::<R>(plugin_name, hint)` while building; `check_requirements` runs once on `OnEnter(EngineState::Playing)` and panics listing every missing piece together, naming the hint for each, rather than crashing on the first missing resource it happens to touch.

**Registries (`crates/rl-rules/src/content.rs`, `crates/rl-grid/src/tile.rs`):**
- Purpose: the sole non-trait extension mechanism; a game adds content by inserting rows into a registry keyed by an opaque `Id<T>`, never by adding an enum variant.
- Examples: `TileRegistry`, `Registries` (damage kinds, statuses, factions, gases), `ContentRegistry<T>` and `BandedWeightedTable<T>` for RON-loaded, depth-banded content.
- Pattern: `validate()` at load time plus guard tests over live assets recover the exhaustiveness a closed enum used to give at compile time.

**`RunSeed` / `SeedDomain` / `Seed` / `Stream` (`crates/rl-core/src/seed.rs`, `crates/rl-bevy/src/seed.rs`):**
- Purpose: one root seed per run, with every subsystem's randomness deterministically and independently derived from it.
- Examples: `RunSeed::derive(domain, index)` and `RunSeed::stream(domain)` in `rl-core`, which any Bevy-free code (mapgen, worldgen) uses directly; `Seed` (a `Resource` wrapping `RunSeed`) and the `Stream` trait plus `AddStream`/`add_stream::<S>()` in `rl-bevy`, which a plugin uses to own a named, auto-rederiving RNG resource (`CombatRng`, `AbilityRng`, `MindRng`, `StealthRng`, each registered via `.add_stream::<T>("PluginName")` - `crates/rl-bevy/src/combat.rs:527`, `ability.rs:1137`, `minds.rs:626`, `stealth.rs:213`).
- Pattern: `add_stream::<S>` itself calls `app.needs::<Seed>(...)`, so a stream that is never given a seed is reported the same way any other missing requirement is, not a silent `rand::rng()` fallback.

**`Facet` (`crates/rl-ui/src/facet.rs`):**
- Purpose: the one place a game's vocabulary is allowed to reach an engine-owned view, without the view type ever naming it.
- Examples: `Facet { key: FacetId, text, tone: ToneId }`; the engine's own use is the status badge key, declared once in `UiPlugin::build` (`crates/rl-ui/src/lib.rs:169`).
- Pattern: pushed in `ViewSet::Annotate`; two games wanting the same key is documented as the signal the field belongs in the view instead (`crates/rl-ui/src/facet.rs:14-15`, `docs/design/ui.md:290-292`).

**`ToneId` / `Palette` / `Tone` (`crates/rl-ui/src/tone.rs`):**
- Purpose: every widget colours by an interned semantic tone, never a literal `Color`, so restyling the whole UI is one palette edit.
- Examples: `Tones::BAD`, `readable()` helper, `AddTone` for a game's own tones.
- Pattern: an unset tone is reported once at `OnEnter(Playing)` (`tone::report_unset_tones`, `lib.rs:172`) rather than warned on first use, because interior mutability would be needed to warn-once inside a `Res` read.

## Entry Points

**`examples/corsair` (open-world pirate roguelike):**
- Location: `examples/corsair/src/main.rs`
- Exercises: `rl-world` streaming, factions, ranged attacks (`f`), the ability system, quests/facts, saves, the full panel set, `rl-overworld`'s portal picker.
- Structure: `content.rs` (RON-backed bestiary/armory), `places.rs` (`populate_places`, cave/treasure logic), `items.rs`, `monsters.rs`, `quests.rs`, `save.rs`, `abilities.rs` (the one effect Corsair adds beyond the engine's seven), `statuses.rs`, `rules.rs`, `input.rs`.

**`examples/delve` (dungeon delve, no surface):**
- Location: `examples/delve/src/main.rs`
- Exercises: lighting fully on (a carried brand, dark sight, glowing bile), `rl-mapgen` chains with no `WorldGraph` at all (`PlaceRules::build` takes `Option<&WorldGraph>`), five floors built lazily on first entry.
- Structure: `floors.rs` (the whole multi-floor map builder in one file, per README), `effects.rs` (the one effect delve adds).

**`examples/heist` (stealth heist, three floors):**
- Location: `examples/heist/src/main.rs`
- Exercises: the stealth/`Sense`/`Aware` model, a game-owned `Tactic` reading an engine-pushed `Sense`, and a game-owned `Choice` the engine routes through - the clearest existing proof that a game can extend AI without the engine knowing its vocabulary.
- Structure: `main.rs` (1076 lines - the largest single game-layer file in the repo), `floors.rs`.

**`examples/tutorial` (the guide's ten runnable steps):**
- Location: `examples/tutorial/src/bin/step01_walking.rs` … `step10_panels.rs`
- Exercises: one engine capability at a time, each binary quoted verbatim by a `docs/guide/src/NN-*.md` chapter via `<!-- include: -->` markers, checked by `scripts/check-guide.sh`.
- Note: filenames are not sequential - there is no `step07_*.rs`; guide chapter `07-content-in-files.md` quotes `step08_content.rs` and chapter `08-an-action-of-your-own.md` quotes `step09_shove.rs`. Intentional (chapters and step numbers were decoupled at some point) but worth knowing before assuming a 1:1 mapping.

**`templates/starter` (the `cargo generate` template):**
- Location: `templates/starter/src/main.rs` (562 lines - one file, per the README's "it plays from the first build")
- Exercises: the smallest complete game: a floor of rooms, a torch and braziers, goblins that hunt by sight, bump-to-attack, a status row, a log, a look cursor, three headless tests.

## Architectural Constraints

- **Threading:** single-threaded Bevy `Update`/`Turn` schedule; `Chain` is `Send` (PLAN 3.4) so generation *can* move off the main thread, but no example or the engine itself currently spawns generation on another thread - it is a capability, not a used one.
- **Global state:** none observed as module-level statics beyond the one `AtomicU64` process-lifetime run counter in `RunSeed::fresh` (`crates/rl-core/src/seed.rs:85`), which is explicitly not used for gameplay determinism (only to avoid two `fresh()` calls colliding within one process).
- **Circular imports:** none possible by construction - `scripts/check-tiers.sh` fails any crate that depends on a higher tier, and `cargo metadata` confirms zero such edges as of this analysis.
- **Frame-vs-turn coupling:** UI collectors run every frame, not on a turn boundary (`docs/design/ui.md` section 11b), which means a game that wants panel data as of a specific turn (for a replay scrubber, say) has no such seam yet; today's use cases don't need it.
- **`MAX_PASSES = 512`:** a turn loop with more than 512 actors due before the player's next turn defers the rest to the following frame rather than stalling (`crates/rl-bevy/src/plugin.rs:120-123, 140-150`); this is a soft ceiling on how many awake actors one frame of the active region can carry, undocumented as a tunable outside this constant.

## Anti-Patterns

### Reading `Option<Res<T>>` to decide whether a system runs at all

**What happens:** none found. Every `Option<Res<T>>` sampled (`crates/rl-bevy/src/fov.rs:64-65`, `stealth.rs:92,135-136,258,261`, `minds.rs:333,396`, `world.rs:575-576`, `gas.rs:200`, `fire.rs:360`, `rl-ui/src/controls.rs:578-582`, `rl-ui/src/game_menu.rs:176,219,222,298-299`, `rl-ui/src/replay.rs:105,168`, `rl-ui/src/narrate.rs:247`) reads optional data *inside* a system that already runs unconditionally because its plugin was added, and treats absence as meaningful game state (no lighting means a lit world; no `CombatRules` means no combat math to run this tick) rather than as "is this subsystem installed."
**Why this would be wrong if found:** CLAUDE.md's plugin rule specifically forbids deciding *whether a system runs* by resource presence - that decision belongs to whether the plugin was added.
**What the codebase actually does:** exactly the sanctioned pattern from CLAUDE.md: "a resource may still be optional data... read an optional one as `Option<Res<_>>` and say in the docs what its absence means." Recorded here because it was worth verifying, not because it is a problem.

### Colour reaching a "view" type through the game's own data, not a literal

**What happens:** `Span { start, len, color: Color }` (`crates/rl-ui/src/log.rs:26-32`) carries a `bevy::color::Color`, and `MessageLog::push_spans` stores it directly on a log entry. The colour originates in `narrate::render` (`crates/rl-ui/src/narrate.rs:566-598, 630-636`), which reads it off `glyph.map(|g| g.fg)` - the foreground colour the *game* already set on that entity's `Glyph` component for map rendering - so a monster's name in the log matches its glyph on the map.
**Why it's in tension with the stated rule:** `crates/rl-ui/src/lib.rs:73-75` says a view holds "No `Color`, no `Rect`, no string the game did not supply", and the log/narration view is the one place a `Color` (not a `ToneId`) sits directly in engine-owned view data.
**Why it is not actually a violation:** the colour is never authored by the engine - it is forwarded verbatim from a component the *game* already owns (`Glyph::fg`), the same way a game-supplied string is allowed through. It is a narrower, more defensible exception than a widget picking its own literal, but it does mean `rl-ui/src/log.rs` and `narrate.rs` are the one spot where "restyle everything through the `Palette`" does not hold: recolouring a game's glyphs does not recolour its log names, because the log line already baked the `Color` in at write time. A game that repaints its `Glyph`s at runtime (a status effect that tints a monster, say) will see stale colours on old log lines, which is arguably correct (the line described that moment) but is not documented anywhere as a chosen behaviour.

### Ordering by concrete function vs by named `SystemSet`

**What happens:** searched every `.after(` / `.before(` call outside `crates/rl-bevy` and `crates/rl-ui` system-set enums; the only three hits (`crates/rl-render/src/capture.rs:130`, `crates/rl-bevy/src/testing.rs:154`, `crates/rl-ui/src/replay.rs:77`) order after Bevy's own built-in `InputSystems` label, not after another engine crate's function.
**Why this would be wrong if found:** PLAN 3.11 and `rl-ui/src/lib.rs:71-72` both forbid a system ordering itself after another crate's concrete function, precisely so a crate can split or rename an internal system without breaking a downstream one.
**What the codebase actually does:** consistently orders by named `SystemSet` (`TurnSet`, `DecideSet`, `ResolveSet`, `CleanupSet`, `FieldSet`, `PerceiveSet`, `EngineSet`, `PresentSet`, `ViewSet`) everywhere it matters, including inside the three example games (`examples/corsair/src/main.rs:196-202`, `examples/delve/src/main.rs:88`, `examples/heist/src/main.rs:81-86`, `templates/starter/src/main.rs:92`). This is the rule holding, cited here as the positive control for the check above.

## Error Handling

**Strategy:** `Result<(), BuildError>` for anything that can fail during generation (`Pass::apply`, PLAN 3.4), so a `Chain` can retry; a hard `panic!` for setup mistakes the game must fix before shipping (`check_requirements`, `crates/rl-bevy/src/plugin.rs:502-508`); a `warn!` for a likely-but-not-certain setup mistake the game can legitimately intend (`warn_if_play_never_began`, `plugin.rs:516-524`).

**Patterns:**
- Missing engine setup is reported once, all-at-once, with a hint per piece - never a crash on the first resource a system happens to touch.
- A refused `Intent` (no resolver claimed it) is a warning plus a named type, not a silently frozen actor (`TurnSet::Sweep`).
- Out-of-bounds writes to a `Grid` are ignored and reported rather than panicking (`crates/rl-core/src/grid.rs:178`).

## Cross-Cutting Concerns

**Logging:** `bevy::log` macros (`debug!`, `warn!`) for engine-internal diagnostics; a separate, player-facing `MessageLog`/`Narrator` pipeline (`crates/rl-ui/src/log.rs`, `crates/rl-ui/src/narrate.rs`) for in-game text, driven by typed `Said` events rather than string matching (PLAN 3.10 explicitly replaces the old approach of "classifying log lines by matching about thirty English substrings").

**Validation:** every RON-loaded registry runs a `validate()` hook on load (PLAN 3.2); every RON schema file carries a top-of-file comment listing its full option space (CLAUDE.md rule, spot-checked present on the asset files under `examples/*/assets/`).

**Authentication:** not applicable - a local, single-process game engine with no network layer.

## Architectural Assessment

This section evaluates where the built code honours or diverges from the intended architecture in CLAUDE.md, README.md, and `docs/PLAN.md`. Overall the codebase is unusually faithful to its stated design: the mechanisms described in the plan (plugins-are-opt-in, registries-not-enums, named-stream determinism, view/collector/presenter) all exist, are used consistently by the engine's own code, and are exercised correctly by all four example games. The findings below are the specific places worth a reader's attention - most are honoured-with-caveats rather than violations.

**Tier boundaries: honoured, mechanically enforced, and verified.**
`cargo metadata` plus `scripts/check-tiers.sh` (run live for this analysis) confirm zero crates depend on a higher tier and every tier-0/1 crate plus `rl-save` builds for `wasm32-unknown-unknown`. The one place a tier-2 crate (`rl-ui`) lists tier-1 crates (`rl-mapgen`, `rl-world`) it does so only as `[dev-dependencies]` for view tests (`crates/rl-ui/Cargo.toml:27-33`), which the tier checker correctly excludes. This is the strongest-verified claim in the whole document.

**Registry-over-enum and no-theme-words rules: honoured.**
No fantasy/sci-fi/pirate vocabulary was found in any `crates/*/src/*.rs` file (a grep for common theme words returned only an unrelated false positive, `stash`). No `#[non_exhaustive]` appears anywhere in `crates/`. No `HashMap`/`HashSet` appears in any gameplay or generation path: the only two hits in the whole workspace are `crates/rl-core/src/id.rs:178` (inside a `#[cfg(test)]` block) and `crates/rl-save/src/remap.rs:18` (`EntityRemap`, a save/restore-time structure keyed by a runtime `Entity`, arguably outside "gameplay or generation paths" as the rule means them, but worth a maintainer's eye if `rl-save` ever moves onto a hot path).

**Plugin-is-opt-in rule: honoured, and `rl-overworld` is its strongest expression.**
`Needs`/`app.needs` (`crates/rl-bevy/src/plugin.rs:451-509`) is used consistently by every plugin sampled, including `rl-ui`'s panel plugins and `rl-bevy`'s `add_stream`. No system was found that gates *whether it runs* on `resource_exists`; every `Option<Res<T>>` found reads optional data inside an already-scheduled system, exactly as CLAUDE.md prescribes. `rl-overworld` takes the opt-in principle to the crate level - it is a leaf crate nothing else depends on (confirmed by grep) - so a game removes it from `Cargo.toml` entirely rather than merely not adding its plugin, which is the strongest form of "no subsystem is a mode a game can't decline."

**Determinism/streams rule: honoured, and cleanly layered.**
`rl-core::RunSeed` is the Bevy-free primitive any generation code uses directly; `rl-bevy::Seed`/`Stream`/`AddStream` is a thin Bevy wrapper that auto-rederives a named stream whenever the seed resource changes and reports a missing seed through the same `Requirements` mechanism as any other missing piece, rather than falling back to `rand::rng()`. Four plugins (`CombatPlugin`, `AbilitiesPlugin`, `MindsPlugin`, `StealthPlugin`) each own exactly one stream, matching the "a subsystem owns its stream" model precisely.

**UI view/collector/presenter split: honoured, with one narrow, load-bearing exception.**
The three-layer split described in `crates/rl-ui/src/lib.rs` and `docs/design/ui.md` is real: `view/inspect.rs` pushes its arithmetic down into `rl_rules::forecast` exactly as documented, and every panel presenter takes its `Rect` in its constructor. The one place the "no `Color` in a view" rule bends is `Span::color` in `crates/rl-ui/src/log.rs:26-32`, fed from a game's own `Glyph::fg` in `crates/rl-ui/src/narrate.rs:587,595` - defensible (it forwards the game's own data rather than inventing a literal) but undocumented as a deliberate exception, and it means recolouring a `Palette` does not recolour named entities in old log lines, only new ones.

**`docs/design/ui.md` section 11b as a model of honest drift-tracking.** Five decisions in the shipped UI differ from the original design doc (`Name` instead of a bespoke `Label` component to avoid colliding with `bevy_ui`'s; views moved from tier 1 to tier 2 because a `Row` carries an `Entity`; collectors run every frame instead of gated to a turn boundary; unset-tone reporting moved from first-use to startup; `ChromeLayout` was dropped as unnecessary). Each is recorded with its reason rather than silently diverging from the plan - this is the pattern worth holding up as the template for how the rest of the plan's inevitable future drift should be recorded.

**Dependency direction: no violations found; a small number of tier-2 crates carry more surface than their name implies.**
`rl-ui` depends on `rl-save` in production (`crates/rl-ui/Cargo.toml:25`) for the death-screen recap (`Morgue`, `Obituary` in `crates/rl-ui/src/game_menu.rs:26`) - a UI crate reaching into the save crate is unusual on first read but is the correct direction (save data flowing up into a display), not a boundary violation. `rl-save` itself has grown beyond PLAN section 4's original "core, bevy" dependency list to include `rl-grid` and `rl-rules` as the run's captured state grew (map, occupancy, statuses); this is organic growth within tier 2, not a tier violation, but it means `rl-save`'s actual dependency footprint is wider than the plan's crate-layout table currently documents - `docs/OVERVIEW.md` (kept current per its own header) is the source of truth here, not PLAN section 4, which is explicitly historical.

**Games copying engine code: none found.**
Every mechanism the plan singled out as previously "left to the game" (the turn loop, the `TurnQueue`, resolver dispatch, RNG streams) is owned once by the engine and merely extended by the game through the documented seams (`add_action`, `add_stream`, `TurnSet::React`/`Resolve`, `Facet`, `Tactic`). The apparent duplication of a `Stock` `SystemParam` name across `examples/corsair/src/places.rs:166`, `examples/delve/src/main.rs:384`, and `examples/heist/src/main.rs:481` is coincidental naming, not copied code - each bundles a completely different set of game-specific resources (bestiary/armory vs. beasts/bile vs. watch/tiles) for spawning that game's own content, which is exactly the game-owns-vocabulary split the plan calls for, not a seam the engine should absorb.

**Size and complexity distribution: no crate is doing structurally too much, but `rl-bevy/src/ability.rs` is worth a maintainer's attention.**
At 1717 lines it is the largest source file in the workspace by a wide margin (the next largest, `minds.rs`, is 1245). No `#![allow(clippy::too_many_arguments)]` or `type_complexity` escape hatch exists anywhere in the workspace (checked directly), so its size is width of surface area (targeting, effects, cooldowns, the ability registry) rather than a system evading the parameter-count lint the plan specifically calls out (PLAN 3.10, replacing a 615-line, 16-parameter combat system). It has not yet been split the way `docs/PLAN.md:778` anticipates `rl-grid` might split into `rl-fov`/`rl-path`/`rl-field`, and nothing in the codebase suggests it must be - but it is the single largest concentration of one subsystem's logic in one file, and a phase that touches abilities should expect to read most of it.

---

*Architecture analysis: 2026-09-17*
