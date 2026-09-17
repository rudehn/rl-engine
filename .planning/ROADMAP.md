# Roadmap: rl-engine - stealth and lighting

## Overview

The engine already notices, remembers and searches (stealth phases A to D) and casts, gates and draws light (lighting phases A to C).
This roadmap finishes both subsystems as their design docs describe and nothing else.
Stealth goes first because each step builds on the one before: a blow learns whether its target knew (sneak attacks), the player learns to notice what hides (two-way stealth), and then sound becomes a sense both sides use (noise).
Lighting follows: light starts to matter to the minds beyond the gate (lit detection ranges, a light-averse tactic), and last a shadow layer lets a game subtract light, which everything built before it then reads.
Every phase ends with the engine owning the loop and a worked example exercising it, so no game keeps its own copy.

Scope was set by the user on 2026-09-17; everything else outstanding is in `.planning/REQUIREMENTS.md` "Later / not in this roadmap".

## Phase gate

Every phase's last success criterion refers to this gate, from `CLAUDE.md` and `docs/PLAN.md` sections 3.13 and 6:

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` with no crate-wide allow, and `cargo test --workspace` with doc-tests run and none fenced `ignore`, all pass.
- `scripts/check-tiers.sh` and `scripts/check-tiers.sh --wasm` pass, and `#![deny(missing_docs)]` holds on every crate touched.
- `scripts/check-guide.sh` passes if a quoted example changed, and `scripts/check-template.sh` if `templates/starter` changed.
- `docs/OVERVIEW.md` changes in the same commit as the system it gains; the design doc marks its phase built with what the build changed; `docs/PLAN.md` gets a dated progress entry; `CHANGELOG.md` records breaking changes and any deliberate fingerprint re-baseline.
- No theme words in engine crates, no `Custom { id }` or closed content enums, no `TODO` comments, randomness only through registered streams or `Seed::stream`, no `HashMap` or `HashSet` in gameplay paths, integer costs and clocks.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: Sneak attacks** - A damage stage knows whether the defender had noticed its attacker, so a game can write its own sneak attack
- [ ] **Phase 2: The player notices** - Actors can hide from the player, and nothing on screen gives an unnoticed actor away
- [ ] **Phase 3: Noise** - Sound is an engine sense that sends listeners to look, and Heist stops carrying its own
- [ ] **Phase 4: Light in the minds** - A lit subject is detected from farther, and a light-averse mind treats light as a wall
- [ ] **Phase 5: Shadow layer** - Negative sources subtract light, and everything that reads light reads the shadow

## Phase Details

### Phase 1: Sneak attacks
**Goal**: A game can make a blow on a defender that had not noticed its attacker count for more, with a damage stage of its own, and the engine tells every stage truthfully whether the defender knew.
**Depends on**: Nothing (first phase); builds on stealth phases A to D, already built
**Source**: `docs/design/stealth.md` phase E and section 9; `docs/PLAN.md` progress 2026-09-12
**Requirements**: SNEAK-01, SNEAK-02, SNEAK-03
**Success Criteria** (what must be TRUE):
  1. Over a seed range of headless fights with `StealthPlugin`, a damage stage sees `Defender::unaware` true exactly when the defender kept an `Aware` and had not noticed the attacker as the blow landed, and false on the next blow, because the first one woke it.
  2. A melee blow, a ranged shot, a thrown strike and an ability's `Harm` on the same unaware defender all report `unaware` true, and a status tick, fire, or any hit with no attacker reports false.
  3. Without `StealthPlugin`, `unaware` is always false and every existing test passes unchanged, the fingerprint tripwire included.
  4. In `examples/heist`, the thief's blow on a watchman who has not noticed them goes through a heist-owned sneak-attack stage and says so in the log, proven by a test through the real wiring, and the combat docs carry the same kind of stage as a runnable doc-test while the engine ships no multiplier.
  5. The phase gate passes, with the `Defender` change recorded as breaking in `CHANGELOG.md` and stealth.md phase E marked built.
**Plans**: TBD

### Phase 2: The player notices
**Goal**: Monsters can hide from the player the way the player hides from monsters, by the same rules, and the map and every panel show only what the player has noticed.
**Depends on**: Phase 1 (a hidden actor's first blow on an unaware player is a sneak attack)
**Source**: `docs/design/stealth.md` section 9 ("Two-way stealth"), section 12b, section 13 ("Stealth that is invisible to the player"); `docs/PLAN.md` progress 2026-09-12 and 2026-09-16
**Requirements**: SEEN-01, SEEN-02, SEEN-03, SEEN-04
**Success Criteria** (what must be TRUE):
  1. A player carrying `Notice` notices actors carrying `Stealth` by the same `notices` roll, lit bonus and memory a monster uses, on the player's own turn and from `StealthRng`: over a seed range, a still hidden actor inside the certain radius is noticed at once, and one beyond it with a non-zero chance is noticed within a bounded number of turns.
  2. Until the player notices it, a hidden actor is absent from the map view, the nearby rail, the `InSight` and `Focus` cycle, the targeting cursor and the inspect panel, and it appears in all of them on the frame it is noticed, as exact-text presenter tests show.
  3. A hidden actor that strikes the player, or that the player bumps into, is noticed at once, and its first blow on the unaware player reports `Defender::unaware` true.
  4. A player without `Notice` sees everything its viewshed reaches exactly as today, every existing test passes, and in `examples/heist` a watcher authored with `Stealth` in `watch.ron` can be walked past unseen and then noticed, in a test that plays keys through the real input.
  5. The phase gate passes, with two-way stealth marked built in stealth.md and any fingerprint movement re-baselined on purpose.
**Plans**: TBD
**UI hint**: yes

### Phase 3: Noise
**Goal**: A game says where a sound happened and how loud it was, and the engine decides who hears it and sends them to look, so no game writes its own earshot loop.
**Depends on**: Phase 2 (the player's `Aware` exists, so a sound can reach either side)
**Source**: `docs/design/stealth.md` sections 1 and 9 ("Noise"); `docs/design/minds.md` "what waits"; `docs/PLAN.md` sections 3.7 and 3.9; `docs/TODO.md` section 2 (the noise part only)
**Requirements**: NOISE-01, NOISE-02, NOISE-03, NOISE-04, NOISE-05
**Success Criteria** (what must be TRUE):
  1. A game emits a sound at a cell with a loudness, and every listener it reaches becomes alert to that cell, walks there through `SearchLastKnown`, and gets one message per sound, while a sound with no entity behind it needs no stand-in entity.
  2. Over a seed range of generated maps, the set of listeners a sound reaches is identical across repeated runs and across shuffled spawn order, and a listener behind a wall or a shut door hears less or nothing compared with one at the same distance in the open, by the propagation rule the design doc states.
  3. Noise is its own opt-in plugin declaring its needs, `Notice` and `NoticeStats` gain no field, and without the plugin every existing test passes unchanged.
  4. In `examples/heist`, pebbles and shouts run on the engine sense with `Hears`, `alert_listeners` and the pebble's stand-in `Stealth` deleted, and a test throws a pebble so a guard in earshot walks to it while a guard behind a shut door does not.
  5. The phase gate passes, with noise written into `docs/design/stealth.md` as a built section, the noise part removed from `docs/TODO.md` section 2 with its reasoning in the PLAN progress log.
**Plans**: TBD

### Phase 4: Light in the minds
**Goal**: Light matters to the minds beyond the gate: a lit subject is detected from farther than an unlit one, and a light-averse mind will not walk into light.
**Depends on**: Phase 2 (a player carrying `Notice` reads the same lit detection rule as a monster)
**Source**: `docs/design/lighting.md` phase E and section 1; `docs/OVERVIEW.md` "Not built yet"
**Requirements**: LIT-01, LIT-02
**Success Criteria** (what must be TRUE):
  1. `docs/design/lighting.md` phase E states, before any code, what a lit detection range adds over `NoticeStats::lit_bonus` and the light gate, or records the owner's decision that those two already are the mechanic and closes the OVERVIEW item with a PLAN entry.
  2. Where it is built, over a seed range an observer detects a lit subject at a distance where the same subject unlit is not detected, for a monster and for a player carrying `Notice`, and a game without `LightingPlugin` sees no change.
  3. A mind with the light-averse tactic never steps onto a tile lit at or above its authored threshold: over a seed range of maps with a light between it and its goal it stops short, and once the light is taken away it goes on, with the tactic tested in `rl-rules` without an `App`.
  4. In `examples/heist` or `examples/delve`, whichever the creature fits, a light-averse creature authored in RON is held back by a light the player sets down, in a test through the real wiring.
  5. The phase gate passes, with lighting.md phase E marked built and the OVERVIEW "Not built yet" line reduced to the ranged penalty in the dark.
**Plans**: TBD

### Phase 5: Shadow layer
**Goal**: A game can make darkness: a negative source subtracts light in its own layer, so a shadow hides what a light would show, and the map draws it.
**Depends on**: Phase 4 (lit detection and the light-averse tactic read the shadowed field)
**Source**: `docs/design/lighting.md` phase F and section 1; burning-tile glow from the same phase is built and not re-planned
**Requirements**: SHADE-01, SHADE-02, SHADE-03, SHADE-04
**Success Criteria** (what must be TRUE):
  1. A negative source on a prop, an actor or a carried item subtracts intensity through the same shadowcast, and over shuffled source orders the composed field is byte-identical, while with no negative sources it is byte-identical to today's field on every existing fixture.
  2. A lit subject standing in a shadow is not seen by a mind that saw it the turn before, is not counted as lit by stealth or lit detection, and no longer holds back a light-averse mind, in headless tests.
  3. A carried negative source sheds from its carrier, `Fuel` ends it with one `LightEvent::BurntOut`, nothing about it is persisted, and the map view draws a shadowed tile darker while the light overlay shows the lower intensity.
  4. A bench with negative sources sits beside `light/20_sources_radius_8`, and in `examples/heist` a game-side source of darkness hides the thief inside a lamp's light, proven by a test through the real wiring.
  5. The phase gate passes, with lighting.md phase F marked built.
**Plans**: TBD
**UI hint**: yes

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> 4 -> 5.
Phases 4 and 5 do not depend on Phase 3, so lighting could start after Phase 2 if the order needs to change.

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Sneak attacks | 0/TBD | Not started | - |
| 2. The player notices | 0/TBD | Not started | - |
| 3. Noise | 0/TBD | Not started | - |
| 4. Light in the minds | 0/TBD | Not started | - |
| 5. Shadow layer | 0/TBD | Not started | - |
