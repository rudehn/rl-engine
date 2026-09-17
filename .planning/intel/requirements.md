# Requirements

Synthesized 2026-09-16.

## Provenance note

The ingest set contains no PRD.
No requirement below comes from a product requirements document, and none has PRD-grade acceptance criteria.
With no PRDs there are no competing acceptance variants.

The engine is mostly built (see `/Users/nathanrude/Development/rl-engine/.planning/intel/context.md`, "Built").
So this file lists the outstanding work the ingest set names, as candidate requirements for the roadmapper, from these sources in precedence order:

1. `docs/PLAN.md` (locked ADR): the "Next" line, and locked decisions that are not yet met.
2. `docs/design/*.md` SPEC phases marked proposed (not binding until adopted).
3. `docs/TODO.md` (DOC): the backlog from the 2026-09-15 architecture review, in its recommended order.
4. `docs/OVERVIEW.md` (DOC): the "Not built yet" list.

"Done when" gives only what the source itself says counts as done.
Where the source gives no such test, the entry says so; this file does not make one up.

Section C lists deferred scope that nothing currently owns.
It is the subject of a WARNING in `.planning/INGEST-CONFLICTS.md`, and the user must decide whether it belongs on the roadmap.

---

## A. Tracked outstanding work

### A1. From the architecture review backlog (docs/TODO.md, recommended order)

#### REQ-tactics-missing-and-weights
- source: /Users/nathanrude/Development/rl-engine/docs/TODO.md (section 2); /Users/nathanrude/Development/rl-engine/docs/design/minds.md (section 6)
- scope: `rl-rules::ai::tactics`, minds
- description: Add missing tactics: pack and leader behaviour, keep-at-range for a shooter, patrol or idle routine (patrol needs per-actor state held as a `Sense`), noise and scent (via `DijkstraMap` or a `TileField<T>` rule).
  Make the `UseAbility` footprint scoring weights (hardcoded two for a hit, three against for harm) into fields.
- done when (from source): not stated beyond the item leaving `docs/TODO.md` with its reasoning moved to the PLAN progress log.

#### REQ-movement-profile-costs
- source: /Users/nathanrude/Development/rl-engine/docs/TODO.md (section 3); locked basis /Users/nathanrude/Development/rl-engine/docs/PLAN.md (sections 3.5.3, 3.7)
- scope: `rl-grid` `TileProps`, `FlowFields`
- description: `FlowFields::ensure` keys by `MovementProfile` but builds every map with `PathRules::default()`.
  `TileProps` needs a per-profile walkability mask and the flood must read it, so a swimmer and a walker see different maps and the promised sailing profile works.
- done when (from source): profiles change costs; the sailing profile is wired.

#### REQ-anyone-travels
- source: /Users/nathanrude/Development/rl-engine/docs/TODO.md (section 3); /Users/nathanrude/Development/rl-engine/docs/design/minds.md (section 3.5)
- scope: `rl-bevy` places (`WarpRequest`, `GoThrough`, `resolve_warps`)
- description: Non-players can change maps, so companions, escorts and fleeing monsters can take stairs.
- done when (from source): a non-player can change maps.

#### REQ-attack-cost-and-miss-docs
- source: /Users/nathanrude/Development/rl-engine/docs/TODO.md (section 3); /Users/nathanrude/Development/rl-engine/docs/design/abilities.md (section 11, "Accuracy does not exist")
- scope: combat
- description: Put attack cost on `MeleeAttack` and `RangedAttack` instead of `BASE_ACTION_COST` for every blow.
  Document how a game adds a miss as a `DamageStage`, with an example.
- done when (from source): weapon speed is expressible; the combat docs show a miss stage example.

#### REQ-damage-stages-default
- source: /Users/nathanrude/Development/rl-engine/docs/TODO.md (section 3)
- scope: combat configuration
- description: `DamageStages` must not default to empty, which silently gives raw damage.
  Default to `SubtractArmor`, or declare it with `needs`.
- done when (from source): a game that forgets it is told, or gets armor subtraction.

#### REQ-split-ability-module
- source: /Users/nathanrude/Development/rl-engine/docs/TODO.md (section 4)
- scope: `crates/rl-bevy/src/ability.rs` (1,562 lines)
- description: Split into state, registry and resolver submodules; replace the `Spender` and `Bearing` four-tuple aliases with named `SystemParam`s.
- done when (from source): not stated beyond the split itself.

#### REQ-targetview-pointing
- source: /Users/nathanrude/Development/rl-engine/docs/TODO.md (section 4)
- scope: `crates/rl-ui/src/view/target.rs`
- description: `TargetView` stores `Option<Pointing>` instead of three fields rebuilt every frame, and `Pointing` opens so a game can aim its own thing (a dig direction, a conversation target) through the shared cursor.
- done when (from source): a game can aim its own kind of target through the shared cursor.

#### REQ-onmap-required
- source: /Users/nathanrude/Development/rl-engine/docs/TODO.md (section 4)
- scope: `rl-bevy` components
- description: Require `OnMap` on `Position` so `on.map(|m| m.0).unwrap_or(MapId::SURFACE)` disappears from turn, items, minds, places, status and both games.
- done when (from source): the `Option` is gone everywhere.

#### REQ-streams-out-of-prelude
- source: /Users/nathanrude/Development/rl-engine/docs/TODO.md (section 4); locked basis /Users/nathanrude/Development/rl-engine/docs/PLAN.md (section 3.3)
- scope: preludes, `examples/corsair/src/items.rs` `drop_loot`
- description: Remove `CombatRng` and `AbilityRng` from the preludes and move Corsair's loot drop onto `Seed::stream`.
- done when (from source): no game draws from a subsystem's stream.

#### REQ-guide-second-half
- source: /Users/nathanrude/Development/rl-engine/docs/TODO.md (section 5)
- scope: `docs/guide`
- description: Four chapters in the guide's style: lights out, being noticed, an ability in RON, saving the run.
  Today these get one paragraph each in chapter 9.
- note: chapter 5 of the current guide already teaches an ability in RON (/Users/nathanrude/Development/rl-engine/docs/guide/src/05-a-knack.md), and chapter 2 introduces a carried light (/Users/nathanrude/Development/rl-engine/docs/guide/src/02-sight-and-light.md); the roadmapper should scope this item against the restructured nine-chapter guide.
- done when (from source): not stated beyond the four chapters existing.

#### REQ-overview-plugin-table
- source: /Users/nathanrude/Development/rl-engine/docs/TODO.md (section 5)
- scope: `docs/OVERVIEW.md` rl-bevy section
- description: A table of plugin, what it needs, what it adds and what it emits; prose stays for the why.

#### REQ-frame-diagram
- source: /Users/nathanrude/Development/rl-engine/docs/TODO.md (section 5)
- scope: guide
- description: One picture of the frame: `EngineSet`, the `Turn` passes and their sets, on one page of the guide.
- note: chapter 1 now has a table of the turn-pass stages (/Users/nathanrude/Development/rl-engine/docs/guide/src/01-a-map-and-walking.md, "One pass of the turn loop"), but it does not show `EngineSet`; the item was still listed at the last `docs/TODO.md` commit (e38d120).

#### REQ-doc-comment-density
- source: /Users/nathanrude/Development/rl-engine/docs/TODO.md (section 5)
- scope: all crates
- description: Doc comments at `turn.rs` density; history belongs in the PLAN progress log, and comments say what and why-not.

#### REQ-start-helper
- source: /Users/nathanrude/Development/rl-engine/docs/TODO.md (section 5)
- scope: `rl-bevy` run start, guide chapter 1
- description: A `start_in_place(player, map)` command to replace the warp plus state flip, shrinking chapter 1.

#### REQ-split-delve-main
- source: /Users/nathanrude/Development/rl-engine/docs/TODO.md (section 5)
- scope: `examples/delve/src/main.rs` (1,280 lines)
- description: Split into input, narration and content modules, as Corsair is.

#### REQ-crate-family-name
- source: /Users/nathanrude/Development/rl-engine/docs/TODO.md (section 6)
- scope: every crate name, 297 references across 153 files, the public path `rl_engine::rl_core::Rect`
- description: `rl-core` is taken on crates.io, so the family needs a name before the first crates.io release.
  Candidate families with the base, `-core` and `-grid` names all free: `roguelike`, `dungeoneer`, `torchlit`, `runedeep`, `vaults`, `morgue`.
  Shape to copy: bracket-lib (one word is the facade crate and the prefix).
  Reserve the whole family the same day.
- decision status: open, owner decision required (INFO in conflicts report; does not contradict the locked repo name).

#### REQ-publish-readiness
- source: /Users/nathanrude/Development/rl-engine/docs/TODO.md (section 6)
- scope: workspace `Cargo.toml`, per-crate readmes, release tooling
- description: Add version requirements to the eleven `[workspace.dependencies]` path entries (51 path dependencies flow through them); add readmes to the ten crates without one; fix `rl-engine`'s `readme = "../../README.md"`, which `cargo package` refuses; publish in tier order, dry-running each; adopt `cargo-release` or `release-plz` before the first release.
- done when (from source): `cargo publish` is no longer blocked.

#### REQ-publish-staging
- source: /Users/nathanrude/Development/rl-engine/docs/TODO.md (section 6)
- scope: release policy
- description: Publish the five Bevy-free crates first; keep the Bevy layer on a git dependency until its API stops moving.
- note: the source's claim that there is no `CHANGELOG.md` is stale (INFO in conflicts report).

### A2. From the PLAN "Next" line (locked ADR)

#### REQ-corsair-nights
- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md ("Next"); /Users/nathanrude/Development/rl-engine/docs/design/lighting.md (phase D); /Users/nathanrude/Development/rl-engine/docs/OVERVIEW.md ("Not built yet")
- scope: `examples/corsair` (content, not engine)
- description: A Corsair-side system writes ambient from the turn clock so the open water has nights; a lantern the player can douse or run out of.
- constraint: a day cycle is game content, not an engine feature (CON-light-opt-in).

#### REQ-scripted-encounters
- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md ("Next", and M5 deferral 2026-09-10); /Users/nathanrude/Development/rl-engine/docs/design/abilities.md (sections 1, 11); /Users/nathanrude/Development/rl-engine/docs/OVERVIEW.md ("Not built yet")
- scope: abilities, events
- description: An ability with no user: the effect list triggered by a fact, without the turn, the cost or the cursor.

#### REQ-ui-node-presenters
- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md ("Next", section 3.12); /Users/nathanrude/Development/rl-engine/docs/design/ui.md (phase H); /Users/nathanrude/Development/rl-engine/docs/OVERVIEW.md ("Not built yet")
- scope: `rl-ui` (feature or `rl-ui-node` crate)
- description: Bevy UI presenters over the existing views, adding wrapping, proportional text, hover and sub-cell bars.
- gating condition (from source): deferred until a game asks for one of those.

#### REQ-lwr-conversion
- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md ("Next", Nate 2026-09-10)
- scope: living-world-rogue port; `rl-overworld`; chunk streaming
- description: After the engine is done, convert living-world-rogue onto the engine; it keeps overworld token movement, so `rl-overworld` regains travel on the map beside the portal picker, and its maps stream as chunks.
- gating condition (from source): "once the engine is done".
- flag: reverses locked section 3.5.1 and 3.5.2 wording (INFO in conflicts report); the phase should restate which overworld invariants still hold.

### A3. Locked ADR decisions not yet met

#### REQ-instanced-terminal-rendering
- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (sections 2 item 6, 3.12 original text, 6); /Users/nathanrude/Development/rl-engine/docs/OVERVIEW.md ("Not built yet")
- scope: `rl-render` terminal
- description: Replace one sprite per cell, the known scaling limit, with instanced rendering.

### A4. From docs/OVERVIEW.md "Not built yet" (not otherwise tracked)

#### REQ-cursed-items
- source: /Users/nathanrude/Development/rl-engine/docs/OVERVIEW.md ("Not built yet")
- scope: items
- description: Items that resist removal or carry a deliberate penalty.

#### REQ-heat-cold-liquids-wind
- source: /Users/nathanrude/Development/rl-engine/docs/OVERVIEW.md ("Not built yet"); /Users/nathanrude/Development/rl-engine/docs/design/fields.md (section 6)
- scope: tile fields
- description: Heat and cold that put fire out, liquids that flow, gas moved by wind.
- constraint: new rules over `TileField<T>`, not new field types (DEC-FIELDS-01, ADR-PLAN-3.9).

#### REQ-light-mechanics
- source: /Users/nathanrude/Development/rl-engine/docs/OVERVIEW.md ("Not built yet"); /Users/nathanrude/Development/rl-engine/docs/design/lighting.md (phase E)
- scope: minds, lighting, combat
- description: Lit detection ranges for minds, a light-averse tactic, a ranged penalty in the dark once accuracy exists.

#### REQ-mouse-to-tile
- source: /Users/nathanrude/Development/rl-engine/docs/OVERVIEW.md ("Not built yet"); /Users/nathanrude/Development/rl-engine/docs/design/ui.md (section 8)
- scope: `rl-render`
- description: A mouse-to-tile query, which hover, tooltips and click-to-travel wait on; it should land before any panel depends on hover.

---

## B. Documentation drift to correct

These come from contradictions the conflict pass resolved by precedence (INFO entries).
They are small fixes that keep the docs consistent with the locked ADR and the code.

#### REQ-doc-drift-lighting-opt-in
- source: /Users/nathanrude/Development/rl-engine/docs/guide/src/09-where-to-go-next.md ("Lighting: Add one resource"); /Users/nathanrude/Development/rl-engine/docs/OVERVIEW.md ("Lighting, opt-in by inserting `Lighting`"); /Users/nathanrude/Development/rl-engine/docs/design/lighting.md (section 3 "Inserting it turns lighting on")
- description: Say that lighting is turned on by adding `LightingPlugin`, which inserts `Lighting::dark()` (crates/rl-bevy/src/lighting.rs:306-315), as chapter 3 and the examples do.

#### REQ-doc-drift-design-docs
- source: /Users/nathanrude/Development/rl-engine/docs/design/ui.md (section 2 `PresentSet::Narrate`, section 6 `EquipView`, section 12 "warns once", references to `EnginePlugins` and Lamplight); /Users/nathanrude/Development/rl-engine/docs/design/abilities.md (RON header effect list, section 2 table, section 8 remaining effects); /Users/nathanrude/Development/rl-engine/docs/design/stealth.md (section 0 player-viewshed wording)
- description: Bring superseded sections in line with their own as-built sections and the PLAN progress log, or mark them as history.

#### REQ-doc-drift-overview-and-guide
- source: /Users/nathanrude/Development/rl-engine/docs/OVERVIEW.md ("Last updated: 2026-09-15" while describing 2026-09-16 work); /Users/nathanrude/Development/rl-engine/docs/guide/src/09-where-to-go-next.md ("The crates": "the tests in chapter 9" points at itself); /Users/nathanrude/Development/rl-engine/docs/PLAN.md (progress 2026-09-12 names `10-panels.md` and chapters 11 and 12; no progress entry for the guide restructure or `examples/heist`); /Users/nathanrude/Development/rl-engine/docs/TODO.md (section 6 "there is no `CHANGELOG.md`")
- description: Fix the stale stamp, the self-reference, the missing progress-log entries and the stale changelog claim.

---

## C. Deferred scope with no current owner (pending user scoping)

These items were promised by a locked PLAN section, a milestone, or a SPEC phase, and later deferred.
None of them appears in the PLAN "Next" line, `docs/TODO.md`, or the OVERVIEW "Not built yet" list.
The ingest set does not say whether they are still wanted.
See the WARNING "Deferred scope with no owner" in `.planning/INGEST-CONFLICTS.md`.

#### REQ-UNSCOPED-m7-corsair-sea
- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (section 8, M7)
- description: Ships as vehicles with a sailing profile, boarding that pushes a deck map, navy, pirates and merchants as factions, a treasure-map dig quest as data over events, weather as a `TileField<T>`.

#### REQ-UNSCOPED-m4-worldgen
- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (section 3.4, M4, progress 2026-09-10 M4 deferrals); /Users/nathanrude/Development/rl-engine/docs/design/lighting.md (phase E sconces)
- description: River channels and bridges inside chunks beyond the channel pass, fords, cullers beyond keep-largest, a choke map with worklist pruning, decoration rules, settlement passes richer than huts, sconces as prefab marks.

#### REQ-UNSCOPED-generated-victory-and-encounters
- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (M5, progress 2026-09-10 M5 deferrals)
- description: A generated victory condition and generated encounters as data over events.

#### REQ-UNSCOPED-auto-explore-and-travel-to
- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (sections 3.5.2, 3.7, 5)
- description: Auto-explore and travel-to-a-seen-tile within the active region, simulating every step, with an `ExploreInterrupt` trait for stop rules.
  No implementation was found in `crates/`.

#### REQ-UNSCOPED-stealth-beyond-phase-d
- source: /Users/nathanrude/Development/rl-engine/docs/design/stealth.md (phase E, section 9); /Users/nathanrude/Development/rl-engine/docs/PLAN.md (progress 2026-09-12 deferrals)
- description: Phase E sneak attacks (`Defender::unaware`, no engine multiplier), two-way stealth (monsters hidden from the player, including the map view), noise as its own sense.
  Squad alerting stays the game's by design.

#### REQ-UNSCOPED-lighting-shadow-layer
- source: /Users/nathanrude/Development/rl-engine/docs/design/lighting.md (phase F)
- description: A shadow layer for negative emitters (darkness as an effect).
  Burning-tile glow from phase F is already built through `Lighting::set_glow` (see INFO in conflicts report).

#### REQ-UNSCOPED-summon-effect
- source: /Users/nathanrude/Development/rl-engine/docs/design/abilities.md (section 3.5, section 8)
- description: A `Summon` engine effect, to land with the slice that needs it.

#### REQ-UNSCOPED-benches
- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (section 3.9)
- description: Benches for the choke map, each builder, a full world chain at three sizes and the render sweep.
  Only fov, astar, dijkstra, regions, light and tile_field exist, in `crates/rl-grid/benches/grid.rs`.

#### REQ-UNSCOPED-speed-modifiers
- source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md (section 3.6)
- description: Speed through a `SpeedModifier` trait or an `ActionCost` message, not by reading named statuses.
  Neither name was found in `crates/`; check the code before scoping, since speed may already work another way.
