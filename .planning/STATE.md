# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-09-17)

**Core value:** A game gets a mechanic by adding a plugin, and the engine owns that mechanic's loop, so no game rewrites it.
**Current focus:** None - the roadmap is empty

## Current Position

Phase: none (the roadmap has no phases)
Plan: none
Status: Idle - nothing planned, nothing in flight
Last activity: 2026-09-17 - The user deferred the whole stealth and lighting roadmap before planning; the 18 requirements moved to REQUIREMENTS.md "Later", group L0

Progress: [░░░░░░░░░░] 0%

## Performance Metrics

**Velocity:**
- Total plans completed: 0
- Average duration: -
- Total execution time: -

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| - | - | - | - |

**Recent Trend:**
- Last 5 plans: -
- Trend: -

*Updated after each plan completion*

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions; the repo's `docs/PLAN.md` progress log stays the primary record.
Recent decisions affecting current work:

- Roadmap: emptied (user, 2026-09-17). Stealth and lighting were scoped in and then deferred the same day, before any planning; group L0 in REQUIREMENTS.md holds them, and commit 324d903 holds the phase shapes.
- Roadmap: stealth before lighting, ordered by dependency (sneak attacks, then the player notices, then noise; lit detection and light-averse, then the shadow layer).
- Roadmap: noise is pulled out of `docs/TODO.md` section 2; the rest of that item stays Later.
- Roadmap: sconces as prefab marks and the ranged penalty in the dark are not in (no engine machinery needed, and gated on accuracy, respectively).

### Pending Todos

11 pending, in `.planning/todos/pending/`.
With no roadmap running, these are the available work. `2026-09-17-make-movement-profiles-change-pathing-costs.md` and `2026-09-17-take-color-out-of-log-spans.md` are the two the review rated highest.

### Blockers/Concerns

- Phase 4: LIT-01 needs an owner design decision in discussion; no doc defines a lit detection range beyond `NoticeStats::lit_bonus` and the light gate.
- Phase 3: the propagation substrate is open (`DijkstraMap` per PLAN 3.7, or `TileField<T>` per PLAN 3.9 and minds.md); decide it in the design doc first.
- Phases 1 to 3 may move rolls or ordering; the fingerprint tripwire is re-baselined on purpose with a `CHANGELOG.md` line, never silently.

## Deferred Items

Items acknowledged and carried forward from previous milestone close:

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| *(none)* | | | |

## Session Continuity

Last session: 2026-09-17
Stopped at: GSD set up and the roadmap emptied at the user's request; 11 todos pending
Resume file: None
