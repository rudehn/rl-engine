# Roadmap: rl-engine

## Overview

The roadmap is empty.
It held five phases finishing stealth and lighting until the user deferred all of them on 2026-09-17, before any planning or execution.
Those 18 requirements are kept in full in `.planning/REQUIREMENTS.md` under "Later / not in this roadmap", group L0, and the phase shapes they had, with their goals, dependencies and success criteria, are in commit `324d903`.

Nothing is in flight.
The next roadmap starts by moving a group out of "Later" and into phases, whether that is stealth and lighting again or something else.
`.planning/todos/pending/` holds 11 items from the 2026-09-17 architecture review that can be worked without a roadmap.

## Phase gate

Every phase's last success criterion refers to this gate, from `CLAUDE.md` and `docs/PLAN.md` sections 3.13 and 6:

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` with no crate-wide allow, and `cargo test --workspace` with doc-tests run and none fenced `ignore`, all pass.
- `scripts/check-tiers.sh` and `scripts/check-tiers.sh --wasm` pass, and `#![deny(missing_docs)]` holds on every crate touched.
- `scripts/check-guide.sh` passes if a quoted example changed, and `scripts/check-template.sh` if `templates/starter` changed.
- `docs/OVERVIEW.md` changes in the same commit as the system it gains; the design doc marks its phase built with what the build changed; `docs/PLAN.md` gets a dated progress entry; `CHANGELOG.md` records breaking changes and any deliberate fingerprint re-baseline.
- No theme words in engine crates, no `Custom { id }` or closed content enums, no `TODO` comments, randomness only through registered streams or `Seed::stream`, no `HashMap` or `HashSet` in gameplay paths, integer costs and clocks.

## Phases

None.
See `.planning/REQUIREMENTS.md` "Later / not in this roadmap" for what is available to plan.

## Phase Details

None.

## Progress

No phases, so nothing to execute or track.

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| *(none)* | - | - | - |

---
*Roadmap created 2026-09-17 from the docs ingest; emptied the same day at the user's request.*
