# rl-engine

## What This Is

`rl-engine` is a multi-crate Rust roguelike engine.
Tier 0 and 1 crates (`rl-core`, `rl-grid`, `rl-mapgen`, `rl-world`, `rl-rules`) are engine-agnostic, Bevy-free and build for `wasm32-unknown-unknown`; tier 2 crates (`rl-bevy`, `rl-render`, `rl-ui`, `rl-overworld`, `rl-save`) are the Bevy layer; tier 3 `rl-engine` is the facade.
Its consumers live in the workspace: `examples/corsair` (open world), `examples/delve` (dungeon), `examples/heist` (stealth and light), `examples/tutorial` (the guide's Warren), and `templates/starter`.
It targets native desktop and the wasm32 browser.

This planning set covers one slice of remaining work: the unbuilt parts of stealth and lighting.
Everything else outstanding is recorded, with its source, in `.planning/REQUIREMENTS.md` under "Later / not in this roadmap".

## Core Value

A game gets a mechanic by adding a plugin, and the engine owns that mechanic's loop, so no game rewrites it on top of engine data structures.

## Source of truth

The repo's own documents are authoritative; this file points at them rather than restating them.

- `docs/PLAN.md` - locked design decisions (sections 1 to 8) and the dated progress log, where later entries supersede section text.
- `docs/design/stealth.md` - stealth and awareness; section 12b and PLAN progress 2026-09-16 are the effective contract over earlier sections.
- `docs/design/lighting.md` - lighting; phases A to C built, D to F proposed.
- `docs/design/minds.md`, `docs/design/fields.md`, `docs/design/ui.md`, `docs/design/abilities.md` - the neighbouring subsystems these phases touch.
- `docs/OVERVIEW.md` - the inventory of what the engine has, and its "Not built yet" list.
- `docs/TODO.md` - the architecture-review backlog.
- `CLAUDE.md` (symlink to `AGENTS.md`) - rules the build enforces and rules only review enforces.
- `.planning/intel/*.md` and `.planning/INGEST-CONFLICTS.md` - the ingest synthesis these planning files were built from.
- `.planning/codebase/*.md` - the committed architecture review of the code.

## Requirements

### Validated

Milestones M0 to M6 and most of their deferrals shipped between 2026-09-10 and 2026-09-16.
`docs/OVERVIEW.md` is the inventory; `.planning/intel/context.md` "Built" condenses it.
The parts this roadmap builds on:

- Stealth phases A to D: `NoticeStats`, `StealthStats`, `notices`, `Awareness`, `Notice`, `Stealth`, `Aware`, `Noticed`, `StealthPlugin`, `StealthRunning`, `StealthRng`, `Watchers`, `SearchLastKnown`, `Row::aware`, `VitalsView::seen` - 2026-09-12, revised 2026-09-13 and 2026-09-16.
- Lighting phases A to C: `rl-grid::light`, `Lighting`, `LightingPlugin`, `LightSource`, `DarkSight`, `Fuel`, `LightEvent`, the opacity epoch, the gate in `update_viewsheds`, Brogue-style shading - 2026-09-11.
- Burning-tile glow through `Lighting::set_glow` in the dynamic layer - 2026-09-15 (the as-built form of part of lighting phase F; not re-planned).
- Every actor carries its own `Viewshed` - 2026-09-16.

### Active

- [ ] Sneak attacks: `Defender::unaware`, with no engine multiplier (stealth.md phase E).
- [ ] Two-way stealth: the player notices hidden actors, and nothing on screen gives an unnoticed actor away (stealth.md section 9).
- [ ] Noise as its own engine sense (stealth.md section 9; minds.md "what waits").
- [ ] Light in the minds: lit detection ranges and a light-averse tactic (lighting.md phase E; OVERVIEW "Not built yet").
- [ ] A shadow layer for negative emitters (lighting.md phase F).

Detailed, checkable requirements are in `.planning/REQUIREMENTS.md`.

### Out of Scope

- Everything outside stealth and lighting - the user scoped this roadmap to those two subsystems on 2026-09-17; the rest is recorded in `.planning/REQUIREMENTS.md` "Later / not in this roadmap" so nothing is lost.
- An engine sneak-attack multiplier - balance is the game's (stealth.md phase E).
- Squad alerting or shout propagation rules - the `Noticed` message is the seam and propagation is content (stealth.md sections 6 and 9).
- A day and night cycle in the engine - ambient is data the game writes (lighting.md section 0, CON-light-opt-in).
- A ranged penalty in the dark - gated on accuracy, which is deliberately absent (abilities.md "Accuracy does not exist").
- Sconces as prefab marks as engine work - prefab marks already become `Spot`s, so a lit prefab is game content.

## Context

- Examples are the proof: `examples/heist` is the worked example of stealth and light, and today implements its own sound (`Hears`, `alert_listeners`, a stand-in pebble entity) because the engine has none.
- `examples/heist` has no test of its stealth-and-light mechanic; captured as `.planning/todos/pending/2026-09-17-split-heist-main-and-test-its-stealth.md`.
- `crates/rl-bevy/tests/fingerprint.rs` is a fingerprint tripwire over minds, combat, stealth and items; a phase that moves a roll or an order re-baselines it on purpose with a `CHANGELOG.md` line.
- Property tests are hand-rolled loops over a small seed range, not a property-testing crate (`.planning/codebase/TESTING.md`).
- 11 captured todos from the architecture review sit in `.planning/todos/pending/`; they stay todos.

## Constraints

- **Tiers**: tier 0 and 1 never depend on Bevy, no crate depends on a higher tier, and tier 0, tier 1 and `rl-save` build on wasm32 with no `std::time::Instant` - enforced by `scripts/check-tiers.sh` and `--wasm`.
- **Build gates**: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` with no crate-wide allows, `#![deny(missing_docs)]`, doc-tests that compile and run and are never fenced `ignore`.
- **Inventory**: `docs/OVERVIEW.md` is updated in the same commit as any system it gains or loses; the design doc and the PLAN progress log record what the build changed.
- **Vocabulary**: no fantasy, sci-fi or pirate words in engine crate types, docs or constants; content is an id in a registry.
- **Extension**: registries and traits only; no `#[non_exhaustive]` + `Custom { id }`, no closed taxonomy enums for content.
- **Randomness**: only through `Seed`, a `Stream` registered with `add_stream`, or `Seed::stream`; functions take `&mut impl Rng`; lighting draws from no RNG.
- **Containers**: no `HashMap` or `HashSet` in gameplay or generation paths.
- **Integers**: costs and clocks are integers in hundredths of a step; light is integer falloff and screen blending.
- **Plugins**: a subsystem is an opt-in plugin that declares what it needs with `app.needs::<R>(plugin, hint)`; whether it runs is decided by the plugin, never by a resource happening to exist.
- **Panels**: view, collector and presenter; widgets take a `ToneId`, never a `Color`.
- **Scheduling**: reactions in `TurnSet::React`; never order after another crate's system function.
- **Source hygiene**: no `TODO` comments in source; plain dash, never an em dash; American spelling in identifiers.

## Key Decisions

The locked decisions below come from `docs/PLAN.md` (the one ADR, status adopted) and the owner decisions it corroborates.
Each block gives the effective state after the progress log; the cited section is the full text.

<decisions>
ADR-PLAN-01 (PLAN section 1, locked): Own the loop or leave the subsystem out.
A struct plus a `SystemSet` marker is not a subsystem, and the engine is developed against a real game.
</decisions>

<decisions>
ADR-PLAN-02 and ADR-PLAN-3.1 (PLAN sections 2 and 3.1, locked): Themes live in the game and engine crates are lexically theme-free.
Lighting, factions, stealth, statuses, tile fields and quest triggers are engine mechanics; which ones exist is game content.
</decisions>

<decisions>
ADR-PLAN-3.2 (PLAN section 3.2, locked): Extension through registries and traits, never enum variants.
Data extensions are opaque ids in registries loaded from RON with validate-on-load; code extensions are traits; actions and effects are types.
</decisions>

<decisions>
ADR-PLAN-3.3 (PLAN section 3.3, locked; progress 2026-09-14 and 2026-09-16): `rand` 0.9, `StdRng` for gameplay, `RunSeed::derive(domain, index)`.
The engine owns `Seed`; a subsystem registers a `Stream` with `add_stream`; a game draws from `Seed::stream`; minds and stealth roll from `MindRng` and `StealthRng`.
Determinism within one build is an invariant, not across versions.
</decisions>

<decisions>
ADR-PLAN-3.6 (PLAN section 3.6, locked; progress 2026-09-10 to 2026-09-16): The engine owns the turn loop on an integer clock.
`TurnSet::{Decide, Resolve, Sweep, React, Cleanup}` with `DecideSet`, `ResolveSet` and `CleanupSet`; new actors are admitted player first, then in spawn order.
</decisions>

<decisions>
ADR-PLAN-3.7 (PLAN section 3.7, locked; minds.md 3.5): Dijkstra maps per pather class, A* for unique goals.
`FlowFields` are keyed by goals, profile, door opening and direction and kept by cost epoch; PLAN 3.7 names sound propagation with decay as a Dijkstra map.
</decisions>

<decisions>
ADR-PLAN-3.9 (PLAN section 3.9, locked): Data layout for performance.
`TileField<T>` is the one type for fire, gas, sound, scent, heat and blood; no hash containers in gameplay paths; reusable scratch buffers; shadowcast lighting; benches on hot paths.
</decisions>

<decisions>
ADR-PLAN-3.10 (PLAN section 3.10, locked): Shapes in the engine, vocabulary in the game.
The engine ships a staged damage pipeline and a tactic trait; tactic-priority AI is the only AI; attacking is its own intent.
</decisions>

<decisions>
ADR-PLAN-3.11 (PLAN section 3.11, locked; progress 2026-09-11 and 2026-09-13): Subsystems are opt-in plugins that declare needs with `app.needs::<R>(plugin, hint)`, reported together on entering play.
Named phases so no plugin orders after a concrete system; messages are the engine and game seam; `rl_bevy::testing` is the test kit.
</decisions>

<decisions>
ADR-PLAN-3.12 (PLAN section 3.12 as revised in-section, locked): Every panel is a view, a collector and a presenter; terminal presenters ship; the map view stays a glyph grid.
</decisions>

<decisions>
ADR-PLAN-3.13 (PLAN section 3.13, locked): `#![deny(missing_docs)]`, running doc-tests, no blanket clippy allows, why-and-why-not doc comments, RON schemas with a top-of-file option-space comment, property-over-seed-range tests, labelled fingerprint tripwires, headless `App` tests.
</decisions>

<decisions>
ADR-PLAN-4 (PLAN section 4 and progress 2026-09-11, locked): Crate layout by dependency weight.
Tier 0 `rl-core`; tier 1 `rl-grid`, `rl-mapgen`, `rl-world`, `rl-rules`; tier 2 `rl-bevy`, `rl-render`, `rl-ui`, `rl-overworld`, `rl-save`; tier 3 `rl-engine`.
</decisions>

<decisions>
ADR-PLAN-5-6 (PLAN sections 5 and 6, locked): Not ported includes `roguelike_engine`'s `squad/` and `stealth/noise.rs`.
Not carried includes `Custom { id }`, closed taxonomy enums, hash containers in gameplay paths, ungated `Update` systems, `std::time::Instant` on wasm, unseeded constructors, and `TODO` comments in source.
</decisions>

<decisions>
DEC-MINDS-01 (minds.md 3.1, Nate 2026-09-16; PLAN progress 2026-09-16): Every actor carries its own `Viewshed`.
The observer's own viewshed is the one line-of-sight answer for perceive, noticing and `Watchers`; `perceivable` is removed.
</decisions>

<decisions>
CON-stealth (stealth.md sections 6, 9, 12b; effective contract): The engine ships no sneak-attack multiplier, only `Defender::unaware`.
Squad propagation is the game's, over `Noticed`.
Noise is a separate sense, not a knob on `Notice`.
`notices` does not take perception; `memory` lives on `NoticeStats`; an alert observer keeps its subject while it can perceive it.
</decisions>

<decisions>
CON-light (lighting.md sections 0, 2, 3, 5; PLAN progress 2026-09-11; effective contract): The gate lives in `update_viewsheds` and nowhere else.
Gameplay reads `intensity` only; `waver` is the renderer's.
Lighting is opt-in by adding `LightingPlugin`, which inserts `Lighting::dark()`; the engine names no torch, sun or lava and runs no day cycle.
Falloff and blending are integer, emitters are sorted before casting so the field is order-independent, lighting draws from no RNG and is never persisted.
</decisions>

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Roadmap scoped to stealth and lighting only (2026-09-17) | The user chose these two subsystems; everything else is recorded as Later with its source | - Pending |
| Noise is pulled from `docs/TODO.md` section 2 into this roadmap; pack, leader, keep-at-range, patrol, scent and the `UseAbility` weights stay Later | stealth.md section 9 places noise in stealth; the rest of that TODO item is minds work | - Pending |
| Lit detection ranges and the light-averse tactic are in; the ranged penalty in the dark and sconces as prefab marks are out | lighting.md phase E places all four in lighting, but the penalty is gated on accuracy and sconces need no engine machinery | - Pending |
| Burning-tile glow is not re-planned | Built 2026-09-15 through `Lighting::set_glow` in the dynamic layer | ✓ Good |

## Evolution

After each phase transition: move shipped requirements to Validated with the phase, move invalidated ones to Out of Scope with the reason, log decisions here, and check that "What This Is" still holds.
The repo's own record stays primary: the design doc, `docs/OVERVIEW.md` and the PLAN progress log are updated in the phase's commits.

---
*Last updated: 2026-09-17 after project initialization from ingest*
