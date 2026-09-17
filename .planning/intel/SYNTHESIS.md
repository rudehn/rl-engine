# Synthesis

Entry point for `gsd-roadmapper`.
Ingest of `/Users/nathanrude/Development/rl-engine/docs/` on 2026-09-16, mode `new`, precedence ADR > SPEC > PRD > DOC.

## Status

AWAITING USER: no blockers, 2 warnings need a decision before routing.
Details: `/Users/nathanrude/Development/rl-engine/.planning/INGEST-CONFLICTS.md`.

## The situation in one paragraph

`rl-engine` is a multi-crate roguelike engine that is largely built.
Milestones M0 to M6 and most of their deferrals shipped between 2026-09-10 and 2026-09-16.
It has opt-in plugins, minds, combat, items, abilities, stealth, lighting, fire and gas, UI panels, save and replay, three example games, a starter template, and a tutorial guide.
The roadmap should cover what remains: the architecture-review backlog in `docs/TODO.md`, the PLAN "Next" line, the inventory's "Not built yet" list, unmet locked decisions, some doc drift, and deferred scope the user has not yet scoped.
Do not re-plan shipped work; `context.md` "Built" is the inventory.

## Doc counts by type

- ADR: 1 (`docs/PLAN.md`, locked)
- SPEC: 4 (`docs/design/lighting.md`, `stealth.md`, `ui.md`, `abilities.md`)
- PRD: 0
- DOC: 19 (`docs/OVERVIEW.md`, `docs/TODO.md`, `docs/design/fields.md`, `docs/design/minds.md`, four `docs/reviews/*.md`, eleven `docs/guide/src/*.md`)
- UNKNOWN: 0

## Decisions

Locked: 1 ADR, recorded as 19 separate decision entries (owner pre-decisions, sections 1, 2, 3.1 to 3.13, 4, 5 and 6, 8, and the "Next" line), each with its effective state after the progress log.
Source: /Users/nathanrude/Development/rl-engine/docs/PLAN.md.
Also recorded: 3 owner or as-built decisions from DOC sources, each checked against the ADR (`docs/design/minds.md` x2, `docs/design/fields.md` x1).
File: `/Users/nathanrude/Development/rl-engine/.planning/intel/decisions.md`.

## Requirements

No PRDs, so there are no PRD requirements and no competing variants.
39 candidate requirements come from higher-precedence and backlog sources, each with its source:

- A1, architecture review backlog (`docs/TODO.md`, recommended order), 18: REQ-tactics-missing-and-weights, REQ-movement-profile-costs, REQ-anyone-travels, REQ-attack-cost-and-miss-docs, REQ-damage-stages-default, REQ-split-ability-module, REQ-targetview-pointing, REQ-onmap-required, REQ-streams-out-of-prelude, REQ-guide-second-half, REQ-overview-plugin-table, REQ-frame-diagram, REQ-doc-comment-density, REQ-start-helper, REQ-split-delve-main, REQ-crate-family-name (open owner decision), REQ-publish-readiness, REQ-publish-staging.
- A2, PLAN "Next" line, 4: REQ-corsair-nights, REQ-scripted-encounters, REQ-ui-node-presenters (gated on a game asking), REQ-lwr-conversion (gated on the engine being done; flagged).
- A3, unmet locked decision, 1: REQ-instanced-terminal-rendering.
- A4, OVERVIEW "Not built yet", 4: REQ-cursed-items, REQ-heat-cold-liquids-wind, REQ-light-mechanics, REQ-mouse-to-tile.
- B, doc drift found by the conflict pass, 3: REQ-doc-drift-lighting-opt-in, REQ-doc-drift-design-docs, REQ-doc-drift-overview-and-guide.
- C, deferred scope with no owner, pending user scoping (WARNING), 9: REQ-UNSCOPED-m7-corsair-sea, REQ-UNSCOPED-m4-worldgen, REQ-UNSCOPED-generated-victory-and-encounters, REQ-UNSCOPED-auto-explore-and-travel-to, REQ-UNSCOPED-stealth-beyond-phase-d, REQ-UNSCOPED-lighting-shadow-layer, REQ-UNSCOPED-summon-effect, REQ-UNSCOPED-benches, REQ-UNSCOPED-speed-modifiers.

File: `/Users/nathanrude/Development/rl-engine/.planning/intel/requirements.md`.

## Constraints

27 effective contracts from the 4 SPECs, with each SPEC's own as-built section and the ADR applied:

- api-contract: 13
- protocol: 7
- nfr: 6
- schema: 1

By subsystem: lighting 7, stealth 6, UI 7, abilities 7.
File: `/Users/nathanrude/Development/rl-engine/.planning/intel/constraints.md`.

## Context topics

8 topics: the built inventory; the inventory's not-built list; the outstanding backlog; the minds design; the fire and gas design; the Warren guide; the four predecessor reviews (evidence for PLAN); the project rules these docs restate.
File: `/Users/nathanrude/Development/rl-engine/.planning/intel/context.md`.

## Conflicts

- Blockers: 0
- Warnings: 2
  - Cross-reference cycles (17-doc component of mutual hyperlinks). Downgraded from the contract's BLOCKER because synthesis never follows references recursively; the user should confirm.
  - Deferred scope with no owner (requirements.md section C). The user should mark each group in scope, later, or dropped.
- Info (auto-resolved or noted): 18. Mostly PLAN progress-log revisions superseding PLAN section 3, SPEC sections superseded by their own as-built sections or the ADR, and stale DOC statements.
  One entry to read before scoping the living-world-rogue conversion: the "Next" line reverses the locked overworld-travel wording.

Report: `/Users/nathanrude/Development/rl-engine/.planning/INGEST-CONFLICTS.md`.

## Guidance for the roadmapper

- Sequence A1 in the order `docs/TODO.md` gives; that order is the review's recommendation.
- Hard constraints for any phase: tiers 0 and 1 stay Bevy-free and wasm-clean; subsystems stay opt-in plugins with `needs`; no theme words, `Custom { id }` or closed content enums; randomness only through `Seed` and streams; no hash containers in gameplay paths; `docs/OVERVIEW.md` updated in the same commit as any system change; a finished `docs/TODO.md` item leaves that file and its reasoning moves to the PLAN progress log.
- Publishing (REQ-crate-family-name, REQ-publish-readiness, REQ-publish-staging) starts with an owner decision on the crate family name.
- REQ-ui-node-presenters and REQ-lwr-conversion are gated by conditions in their sources; do not schedule them as unconditional work.
- Section C items stay off the roadmap until the user scopes them.
