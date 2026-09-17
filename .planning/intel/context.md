# Context

Running notes from the 19 DOC-type sources in the ingest set, grouped by topic.
The notes condense each source and point at it for full detail.
Quoting the sources in full would repeat about 5,000 lines that are already in the repo.
DOC sources carry the lowest precedence; where they disagree with `docs/PLAN.md` or a SPEC, the conflicts report says which one wins.

---

## Topic: Built - the engine inventory as of 2026-09-16

source: /Users/nathanrude/Development/rl-engine/docs/OVERVIEW.md (header stamp says 2026-09-15, but the content includes the 2026-09-16 minds work)
corroborated by: /Users/nathanrude/Development/rl-engine/docs/PLAN.md progress log 2026-09-10 to 2026-09-16

Use this list to keep the roadmap from re-planning shipped work.

- Tier 0 `rl-core`: geometry, `Grid<T>`, `DisjointSet`, `RunSeed` and `SeedDomain`, `DiceRoll`, `Id<T>` interning, quantiles, integer-clock `TurnQueue`.
- Tier 1 `rl-grid`: tile registry with doors (`opens_to` and `closes_to`) and burn props, `TileField<T>`, `BitGrid`, symmetric shadowcast FOV, region flood, A*, bounded `DijkstraMap` with rescan, `SpatialGrid`, targeting shapes and `clear_shot`, light field, criterion benches.
- Tier 1 `rl-mapgen`: pass chain with named streams and phases, basic passes, dungeon passes (rooms, BSP, doors, random start, farthest exit), prefab stamping.
- Tier 1 `rl-world`: FBM, elevation banding, priority-flood hydrology, climate, site placement, road router, `WorldGraph`, seam-hashed chunks, per-tile sampling, `world_dump`.
- Tier 1 `rl-rules`: content registries and `BandedTable`, stats with `Source`-tagged modifiers, damage pipeline including negative-hit mends, `Names` and `NameRef<T>`, statuses, faction matrix, slot graph, affixes and enchant, abilities data and gate, AI brain and tactics (melee, flee, hunt, wander, use ability, search last known, throw at range, scavenge, give way, follow) with `Wits`, events and quests, balance, awareness, gas, fire, forecast.
- Tier 2 `rl-bevy`: opt-in plugins with `needs`; the engine-owned turn loop with named sets; replay recording; cues and `TurnHold`; `Seed` and streams; `Decision::Own` and `add_choice`; `TurnSet::React`; action types including `Bump`, `Open`, `Close` and `Swap`; `NewRun`, `Restart`, `RunOver`; chunk streaming; `Registries`; combat and `Loadout`; doors; minds with the perceive stage and goal-keyed `FlowFields`; items, gear folding, throwing; abilities; places; knowledge; statuses; stealth; lighting; gas; fire; `MapFields`; facts; save exports; the `testing` kit; the fingerprint tripwire.
- Tier 2 `rl-render`: diffed terminal back buffer (one sprite per cell), map view, `TileAppearance::load`, Brogue-style shading and memory, light overlay, field appearance, particles, `CapturePlugin`.
- Tier 2 `rl-ui`: `UiPlugin` owning `MessageLog`, the narrator, the game menu and morgue, tones, facets, views and panels (nearby, vitals, gear, inspect, log, scrollback, target, ability, inventory, controls, sheet), `controls` registry, `focus`, shared `cursor`, `ReplayPlugin`, `Modals`, `DirectionKeys`, drawing helpers.
- Tier 2 `rl-overworld`: world view from bands, portal picker, on the shared modal stack.
- Tier 2 `rl-save`: backends (file, memory, browser storage), versioned envelope, entity remap, `Saveable` and `RunSave`, engine state capture including fields and ability state, `Morgue`, `Stash` and `UnloadPlugin`.
- Tier 3 `rl-engine`: facade, `RoguelikePlugins`, curated prelude, `templates/starter` checked by `scripts/check-template.sh`, releases as `v` tags published from `CHANGELOG.md` (0.1.0 and 0.2.0 exist).
- Examples: `examples/delve` (Hollow Whale, five floors, lighting, stealth, knacks, fire and gas), `examples/heist` (Counting House, stealth and light end to end, game-owned `Sense`, `Tactic` and `add_choice`), `examples/corsair` (open world, all content from RON, save, abilities, doors, wits, crews with hands), `examples/tutorial` (Warren).

## Topic: Not built - the inventory's own gap list

source: /Users/nathanrude/Development/rl-engine/docs/OVERVIEW.md ("Not built yet")

Cursed items; heat, cold, liquids and wind; nights on Corsair's open water and a lantern the player can douse or run out of; lit detection ranges, light-averse tactic, ranged penalty in the dark; scripted encounters; Bevy UI presenters (UI phase H); mouse-to-tile; instanced terminal rendering.
Carried into `/Users/nathanrude/Development/rl-engine/.planning/intel/requirements.md` sections A2 to A4.

## Topic: Outstanding work backlog

source: /Users/nathanrude/Development/rl-engine/docs/TODO.md (written 2026-09-15 from an architecture review of `main` at `cf5f0a9`; last commit e38d120, 2026-09-16)

- Framing: "the structure is right, and the debt is behaviour the engine ships as data and every game rewrites on top of it".
- Process rule: an item leaves `docs/TODO.md` in the commit that finishes it, and its reasoning moves to the PLAN progress log.
- Section 1 (own the loops the games keep rewriting): empty; built 2026-09-16.
- Section 2 (open the minds): the five opening items built on 2026-09-16 through `docs/design/minds.md`; missing tactics and weights remain.
- Sections 3 to 6: make what exists real, simplify, documentation, publish.
  Each item is carried into requirements.md section A1 in the source's recommended order.
- "Tracked elsewhere" points at the PLAN "Next" line and the OVERVIEW "Not built yet" list.
- One item in section 6, the crate family rename, is a decision still to be made.

## Topic: Minds design (built 2026-09-16)

source: /Users/nathanrude/Development/rl-engine/docs/design/minds.md

- `rl-rules` decides and never acts; `rl-bevy` perceives and acts and never decides.
- Every actor carries its own `Viewshed` (owner decision), recast in `DecideSet::Sense` for the actor holding the turn.
- `DecideSet::{Sense, Notice, Offer, Perceive, Minds, Game}` with `PerceiveSet::{Begin, Roster, Filter, Annotate}`; `Thinking` is a core resource; the snapshot is sorted once at the head of `Minds`.
- A game's knowledge is a typed `Sense`; a game's choice is `Decision::Own(Box<dyn Choice>)` routed by `add_choice`.
  A `Brain<A, C>` type parameter was declined so games without choices pay nothing.
- Minds work without combat: optional health and faction on `ActorView`, `GiveWay` tactic, `MindRng` and `StealthRng` streams.
- `FlowFields` keyed by `(goals, profile, opens_doors, away)` and kept by cost epoch; `Follow { keep_within, no_closer_than }` is a companion.
- Risks accepted: replays recorded before stage 6 desync; stealth got harder and content was retuned.
- What waits: pack and leader, keep-at-range, patrol with a post, noise and scent.

## Topic: Fire and gas design (built 2026-09-15)

source: /Users/nathanrude/Development/rl-engine/docs/design/fields.md

- `TileField<T>` is double-buffered, stepped whole, allocation-free; new kinds are new rules, not new types.
- Gas is content (`GasDef`: spread, fade of at least one unit, veils_at, burns, inflicts); a cloud always clears; tiles that stop thrown things stop gas.
- Fire is engine rules: tile `burn` with a required `leaves`, `Flammable` entities, burning gas; rolls hash seed, turn and cell; `FireRules::inflicts` and `smoke`; `FireEvent`.
- Integration: veil into opacity, glow through `Lighting::set_glow`, minds avoid fire, `FieldAppearance`, saved per map.
- Placement: one field per map; on the streamed surface the field follows the window; both plugins opt-in in `ResolveSet::Fields`, fire before gas.
- Not here: heat and cold, liquids, wind.

## Topic: Game-author guide (Warren tutorial)

sources:
- /Users/nathanrude/Development/rl-engine/docs/guide/src/SUMMARY.md
- /Users/nathanrude/Development/rl-engine/docs/guide/src/introduction.md
- /Users/nathanrude/Development/rl-engine/docs/guide/src/01-a-map-and-walking.md
- /Users/nathanrude/Development/rl-engine/docs/guide/src/02-sight-and-light.md
- /Users/nathanrude/Development/rl-engine/docs/guide/src/03-blows-and-the-log.md
- /Users/nathanrude/Development/rl-engine/docs/guide/src/04-things-and-minds.md
- /Users/nathanrude/Development/rl-engine/docs/guide/src/05-a-knack.md
- /Users/nathanrude/Development/rl-engine/docs/guide/src/06-two-floors.md
- /Users/nathanrude/Development/rl-engine/docs/guide/src/07-content-in-files.md
- /Users/nathanrude/Development/rl-engine/docs/guide/src/08-an-action-of-your-own.md
- /Users/nathanrude/Development/rl-engine/docs/guide/src/09-where-to-go-next.md

- Structure: an introduction and nine chapters.
  Chapters 1 to 6 are the "six steps", each a runnable binary playable in the browser; chapters 7 and 8 move content into RON and add a custom action; chapter 9 tours the rest of the engine, panels and headless testing.
- The introduction states two API rules: own the loop or leave it out; the engine never names your content.
- Chapter 1: `RoguelikePlugins`, `Seed`, `NewRun`, tiles as registry ids, `TileAppearance`, pass chains, `WarpRequest`, intents, and a table of turn-pass stages (Schedule, Decide, Resolve, Sweep, React, Cleanup).
- Chapter 2: separate walkable and opaque flags, `Viewshed::line` and `visible`, `Knowledge`, `Memory`, a carried `LightSource`.
- Chapter 3: `CombatPlugin`, `MindsPlugin`, `LightingPlugin`, `VitalsPanel`, `LogPanel`, `NarratorPlugin` phrase override, `GameMenuPanel`, `Morgue`, damage stages, factions, brains.
- Chapter 4: items as components, `Throwable`, `ThrowingPlugin`, the aiming cursor, `Bump`, game-defined item use in `TurnSet::React`, `Wits`.
- Chapter 5: abilities in RON (with the full option-space header), `Abilities::load`, `Grants`, `AimAt`, monsters using abilities.
- Chapter 6: places per `MapId`, `PlaceRules`, stairs as transitions, per-floor seeded spawning, a way out that wins.
- Chapter 7: bestiary and tile looks in RON, `NameRef<DamageKind>`, `Registries::names()`, depth-banded tables.
- Chapter 8: a custom `Shove` action and `Choice`, `add_choice`, `Resolution` duties.
- Chapter 9: lighting, stealth, abilities, statuses and gear, quests, saving, run lifecycle, narrator, streaming world, balance, panels (view, collector in `ViewSet::Collect`, presenter), testing kit, crate tiers, examples.
- Build rule from OVERVIEW: chapters quote sources through mdBook anchors, and `scripts/check-guide.sh` fails on broken anchors, images or contents entries.

## Topic: Evidence - reviews of the predecessor codebases

These reviews are the evidence `docs/PLAN.md` cites ("Every claim below is backed by a `path:line` citation in one of those reports").
Their recommendations fed the locked decisions and are not requirements in their own right.

### roguelike_engine (prior extraction attempt, about 15.7k LOC, Bevy 0.17)
- source: /Users/nathanrude/Development/rl-engine/docs/reviews/roguelike_engine.md
- Headline: fantasy-rogue did not depend on it and reimplemented its modules; the engine shipped data structures, not behaviour.
- Findings: `Custom { id }` variants are inert; combat has no extension point; `AbilityDef` describes only a fireball; generation 18.5 ms per small floor; quadratic choke map; O(r^3) lighting; FOV serialized on `ResMut<Map>`; no Dijkstra map; benches measure the wrong things; all doc-tests ignored; two determinism holes.
- Ranked recommendations (F1 to F15): ship behaviour or nothing; lift the turn phase machine; registries over `Custom`; default content crate; split crates by dependency weight; solve world serialization; `Send` fallible chains; fantasy-rogue's seed model; no hash containers in gameplay; fix complexity classes; bench hot paths; `PathingRules` trait; bitset GOAP state; doc-tests and missing_docs; correct scheduling by default.

### living-world-rogue (about 17.6k LOC, Bevy 0.19)
- source: /Users/nathanrude/Development/rl-engine/docs/reviews/living_world_rogue.md
- Findings: the Bevy-free `lwr-world` crate is the model (249 tests in 4 s); closed biome, tile and site enums tear across themes; the road router allocates about 2.6 MB per candidate pair; `Caverns` clones the grid per round; strongest tests and determinism of the three repos.
- Ranked recommendations: enforce the engine-free core in CI; adopt LWR's determinism model; themes in the game but ship one worked theme as a crate, using generic parameters with trait bounds; base map building on LWR's `Chain`; primitives crate first; one distance-field function; a general `PathRouter`; RNG-free network without petgraph; change of scale as a first-class concept (seam hash as a primitive, `Surroundings<F>` generic); take the turn scheduler as-is.

### fantasy-rogue core, map, render, save, infra (about 30k LOC)
- source: /Users/nathanrude/Development/rl-engine/docs/reviews/fantasy_rogue_core.md
- Findings: fantasy `SeedDomain` enum and tile enum; room accretion O(attempts x W x H); `populate_blocked_tiles` defeats change detection; `HashSet<Point>` viewsheds; one `Text2d` entity per tile; full-map promotion scan; full Dijkstra per auto-explore step; `FireRng` seeded from a constant; promotion draws from the combat stream.
- Recommendations include `RngSource`, a tile semantics registry, `ContentRegistry<T>`, a renderer back-end seam, a world lifecycle hook, a save-schema trait.
- Note: its cross-references to `CLAUDE.md` and `TODO.md` mean the reviewed fantasy-rogue repo's files, not this repo's.

### fantasy-rogue combat, actors, items, ui, assets (about 62k LOC)
- source: /Users/nathanrude/Development/rl-engine/docs/reviews/fantasy_rogue_game.md
- Findings: no spatial index; per-monster A*; hash sets in hot paths; per-frame brain allocation; string-keyed AI lookups; tests and determinism strong; a 615-line damage system.
- Ranked recommendations (F1 to F16): seed-domain determinism; composable mitigation chain; modifier stack; `ContentRegistry<T>` and banded tables; open the tactic-priority AI and drop GOAP; spatial index and shared flow field; semantic log with a narration trait; separate move and attack intents; keep the SystemSet ownership contract; port affixes and enchant; generalize the balance checker; faction relation matrix; statuses as data; missing_docs, doc-tests and examples; vocabulary machinery in the engine; a crate split.

## Topic: Project rules the ingest set restates

sources: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (sections 3.11, 3.13, 6); /Users/nathanrude/Development/rl-engine/docs/guide/src/introduction.md; /Users/nathanrude/Development/rl-engine/docs/guide/src/09-where-to-go-next.md

The repo's `CLAUDE.md` (a symlink to `AGENTS.md`, not in the ingest set) states the enforced and review-only rules these docs repeat.
The main ones: tier metadata and Bevy-free tiers 0 and 1; wasm builds with no `Instant`; missing_docs; runnable doc-tests; fmt and clippy clean with no blanket allows; `docs/OVERVIEW.md` updated in the same commit as a system change; no theme words; no `Custom { id }` or closed content enums; randomness only through `Seed`, streams and `&mut impl Rng`; no hash containers in gameplay; opt-in plugins with `needs`; the panel split; `ToneId` not `Color`; facets; game reactions in `TurnSet::React`; integer costs in hundredths; RON option-space headers; no `TODO` comments in source; plain dash and American spelling in identifiers.
