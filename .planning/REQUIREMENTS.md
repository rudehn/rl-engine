# Requirements: rl-engine

**Defined:** 2026-09-17
**Core Value:** A game gets a mechanic by adding a plugin, and the engine owns that mechanic's loop, so no game rewrites it on top of engine data structures.

## Roadmap requirements (stealth and lighting)

Each requirement cites the document section it comes from.
Stealth reads `docs/design/stealth.md` section 12b and `docs/PLAN.md` progress 2026-09-16 as the effective contract over earlier sections (`.planning/intel/constraints.md` CON-stealth-*).
Every requirement was checked against `crates/` on 2026-09-17 and is unbuilt.

### Sneak attacks

Source: `docs/design/stealth.md` phase E (section 10) and section 9; `docs/PLAN.md` progress 2026-09-12 ("Deferred: sneak attack damage (phase E)").

- [ ] **SNEAK-01**: `rl_rules::damage::Defender` gains `unaware`, true when the defender keeps an `Aware` and had not noticed the attacker when the blow landed, and false when stealth is not running, the defender keeps no `Aware`, or the hit has no attacker.
  It is the one breaking change to a tier-1 type in the stealth design, recorded as breaking in `CHANGELOG.md`.
- [ ] **SNEAK-02**: Every path that puts a hit with an attacker down the damage pipeline fills `unaware` by the same rule, read before `wake_on_damage` wakes the defender: melee, ranged, a thrown strike, and an ability's `Harm`.
- [ ] **SNEAK-03**: The engine ships no multiplier; the combat docs show a game-written sneak-attack `DamageStage` as a runnable doc-test, and `examples/heist` uses one.

### Two-way stealth

Source: `docs/design/stealth.md` section 9 ("Two-way stealth: monsters hiding from the player ... a render change and a separate slice"); `docs/PLAN.md` progress 2026-09-12 ("Deferred: ... two-way stealth").

- [ ] **SEEN-01**: A player carrying `Notice` keeps an `Aware` of actors carrying `Stealth`, updated on the player's own turn by the same `notices` roll, lit bonus and memory a monster uses, rolled from `StealthRng`.
- [ ] **SEEN-02**: An actor the player has not noticed is not drawn on the map view and does not appear in any player-facing list: the nearby rail, the `InSight` and `Focus` cycle, the targeting cursor, and the inspect panel.
- [ ] **SEEN-03**: A hidden actor that strikes the player, or that the player bumps into, is noticed by the player at once.
- [ ] **SEEN-04**: A player without `Notice` sees every actor its viewshed reaches exactly as today, and every existing test passes unchanged.

### Noise

Source: `docs/design/stealth.md` section 9 ("Noise. A second sense with its own propagation ... should not be smuggled in as a third knob on `Notice`") and section 1 ("distraction"); `docs/design/minds.md` "what waits" (noise, with `TileField<T>` named as the substrate); `docs/PLAN.md` section 3.7 (sound propagation with decay as a Dijkstra map) and section 3.9 (`TileField<T>` for sound); `docs/TODO.md` section 2 (noise part of "Tactics that are missing").

- [ ] **NOISE-01**: A game emits a sound at a cell with a loudness, and the engine propagates it through the map, with walls and closed doors stopping or dampening it by a rule written into the design doc, in integers, deterministically and independent of ECS order.
- [ ] **NOISE-02**: An actor carrying the engine's hearing component that the sound reaches becomes alert to the sound's cell and searches it through `SearchLastKnown`, and the engine writes one message per listener per sound for the game to narrate.
- [ ] **NOISE-03**: A sound with no entity behind it, such as a thing landing, sends listeners to its cell without the game spawning a stand-in entity.
- [ ] **NOISE-04**: Noise is its own opt-in plugin that declares its needs; it adds no field to `Notice` or `NoticeStats`, and without the plugin nothing changes.
- [ ] **NOISE-05**: `examples/heist`'s pebbles and shouts run on the engine sense, and its own earshot loop (`Hears`, `alert_listeners`, the pebble's stand-in `Stealth`) is gone.

### Light in the minds

Source: `docs/design/lighting.md` phase E (section 7) and section 1 ("Light-averse and dark-sighted creatures"); `docs/OVERVIEW.md` "Not built yet" ("Lit detection ranges, a light-averse tactic, ...").

- [ ] **LIT-01**: Lit detection ranges are defined and written into `docs/design/lighting.md` phase E before code, stating what they add over `NoticeStats::lit_bonus` (which widens only the certain radius) and the light gate (which caps sight at `Perception`), and then built: an observer detects a lit subject at a distance where the same subject unlit is not detected, for minds and for a player carrying `Notice`.
  If the owner decides in discussion that `lit_bonus` and the gate already are this mechanic, the requirement is met by closing the OVERVIEW item with a PLAN progress entry that says so.
- [ ] **LIT-02**: A light-averse tactic in `rl-rules::ai::tactics` keeps a mind off tiles lit at or above an authored threshold, so a dropped light holds it back, with the light reaching the mind through the snapshot rather than a component query.

### Shadow layer

Source: `docs/design/lighting.md` phase F (section 7, "a shadow layer for negative emitters") and section 1 ("A negative emitter subtracts intensity, which is why a shadow layer is reserved now and not retrofitted").
The rest of phase F, burning tiles that glow, is built through `Lighting::set_glow` (PLAN progress 2026-09-15) and is not re-planned.

- [ ] **SHADE-01**: A negative source on a prop, an actor or an item subtracts intensity in its own layer, cast through the same shadowcast, integer, order-independent, allocation-free per cast, and never persisted; a carried one sheds from its carrier and `Fuel` ends it with `LightEvent::BurntOut`.
- [ ] **SHADE-02**: Everything that reads light reads the shadowed field: the viewshed gate, stealth's `lit`, lit detection and the light-averse tactic.
- [ ] **SHADE-03**: The map view draws a shadowed tile darker and the light overlay shows the reduced intensity.
- [ ] **SHADE-04**: The lighting bench gains a case with negative sources beside `light/20_sources_radius_8`, and with no negative sources the composed field is byte-identical to today's.

## Rules every phase satisfies

These are not requirements of one phase; they are the definition of done for all of them, from `CLAUDE.md` and `docs/PLAN.md` sections 3.13 and 6.

- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass, with no crate-wide allow added.
- `cargo test --workspace` passes, doc-tests included, none fenced `ignore`; `#![deny(missing_docs)]` holds.
- `scripts/check-tiers.sh` and `scripts/check-tiers.sh --wasm` pass; tier 0 and 1 stay Bevy-free.
- `scripts/check-guide.sh` passes when the guide or a quoted example changes; `scripts/check-template.sh` passes when `templates/starter` changes.
- `docs/OVERVIEW.md` changes in the same commit as the system it gains; the design doc's phase is marked built with what the build changed; the PLAN progress log gets a dated entry.
- No theme words in engine crates; no `Custom { id }` or closed content enums; no `TODO` comments in source.
- Randomness only through `Seed`, a stream registered with `add_stream`, or `Seed::stream`; lighting draws from none.
- No `HashMap` or `HashSet` in gameplay paths; `BTreeMap`, `Vec` or `BitGrid` with the reason stated.
- Costs and clocks are integers in hundredths of a step.
- Tests are properties over a seed range where a property exists, fingerprint tripwires labelled as such where none does, named as sentences.
- A change that moves `crates/rl-bevy/tests/fingerprint.rs` re-baselines it on purpose with a `CHANGELOG.md` line.
- Every RON schema a phase adds or extends keeps its top-of-file option-space comment current.

## Later / not in this roadmap

Recorded so nothing is lost.
Each entry names its source; where a captured todo in `.planning/todos/pending/` covers an item, the entry points at the todo file instead of restating it.
Moving an entry into the roadmap is a roadmap update.

### L1. Abilities and encounters

- **LATER-01 Summon effect**: a `Summon` engine effect, to land with the slice that needs it.
  Source: `docs/design/abilities.md` sections 3.5 and 8.
- **LATER-02 Scripted encounters**: an ability's effect list triggered by a fact, without the turn, the cost or the cursor.
  Source: `docs/PLAN.md` "Next" and M5 deferral 2026-09-10; `docs/design/abilities.md` sections 1 and 11; `docs/OVERVIEW.md` "Not built yet".
- **LATER-03 Generated victory condition and generated encounters**, as data over events.
  Source: `docs/PLAN.md` M5 and progress 2026-09-10 ("Deferred from M5").
- **LATER-04 Darkness as an ability effect**: a lighting-registered effect that places a negative source, the way fire registers `Ignite`; the shadow layer in Phase 5 is its prerequisite.
  Source: `docs/design/lighting.md` section 1 ("darkness as a spell").

### L2. The PLAN "Next" line

Source for all: `docs/PLAN.md` progress log, "Next" (Nate, 2026-09-10).

- **LATER-05 Nights on Corsair's surface**: a Corsair-side system writes ambient from the turn clock, and a lantern the player can douse or run out of; content in the example, not an engine feature.
  Also `docs/design/lighting.md` phase D and `docs/OVERVIEW.md` "Not built yet".
- Scripted encounters are also on the "Next" line; see LATER-02.
- **LATER-06 Bevy UI node presenters** over the panel views (UI phase H): wrapping, proportional text, hover, sub-cell bars.
  Gated in the source: deferred until a game asks.
  Also `docs/design/ui.md` phase H and section 5; `docs/OVERVIEW.md` "Not built yet".
- **LATER-07 Living-world-rogue conversion**, once the engine is done; it keeps overworld token movement, so `rl-overworld` regains travel on the map beside the portal picker, and its maps stream as chunks.
  Gated in the source: "once the engine is done".
  Note: this reverses locked owner decisions in `docs/PLAN.md` ("Decisions Nate has already made": travel does not simulate time, no route-travel command) and sections 3.5.1 and 3.5.2 (the overworld never owns the player's position; `lwr`'s Scale and Travel state machine is not ported); the phase that takes it must restate which overworld invariants still hold (`.planning/INGEST-CONFLICTS.md` INFO entry).

### L3. docs/TODO.md sections 2 to 5

Source for all: `docs/TODO.md`, in its recommended order.

- **LATER-08 Missing tactics and fixed weights** (section 2): pack and leader behaviour, keep-at-range for a shooter, patrol or idle routine with a post, scent, and `UseAbility`'s hardcoded footprint weights as fields.
  Noise, from the same item, is in this roadmap (Phase 3); the TODO item is edited when Phase 3 lands.
  Also `docs/design/minds.md` section 6.
- **LATER-09 Movement profiles that change costs** (section 3): see `.planning/todos/pending/2026-09-17-make-movement-profiles-change-pathing-costs.md`.
- **LATER-10 Anyone travels** (section 3): non-players change maps through `WarpRequest` and `GoThrough`.
- **LATER-11 Attack cost, and a place for a miss** (section 3): cost on `MeleeAttack` and `RangedAttack`; combat docs show a miss as a `DamageStage`.
- **LATER-12 `DamageStages` must not default to empty** (section 3): default to `SubtractArmor`, or declare it with `needs`.
- **LATER-13 Split `crates/rl-bevy/src/ability.rs`** (section 4): see `.planning/todos/pending/2026-09-17-split-ability-rs.md`.
- **LATER-14 `TargetView` holds `Option<Pointing>`, and `Pointing` opens to a game's own aim** (section 4).
- **LATER-15 `OnMap` as a required component** (section 4): see `.planning/todos/pending/2026-09-17-require-onmap-on-position.md`.
- **LATER-16 Streams out of the prelude** (section 4): see `.planning/todos/pending/2026-09-17-route-corsair-draws-through-seed-stream.md`.
- **LATER-17 Guide chapters for the second half** (section 5): lights out, being noticed, an ability in RON, saving the run; scope against the restructured nine-chapter guide, where chapters 2 and 5 already cover a carried light and an ability in RON.
  The "being noticed" and "lights out" chapters would be written against the mechanics this roadmap finishes.
- **LATER-18 A plugin table in the overview** (section 5).
- **LATER-19 One picture of the frame** (section 5): `EngineSet`, the `Turn` passes and their sets; chapter 1 has the turn-pass table but not `EngineSet`.
- **LATER-20 Doc comments at `turn.rs` density** (section 5).
- **LATER-21 A start helper** (section 5): `start_in_place(player, map)`.
- **LATER-22 Split the delve's `main.rs`** (section 5); the same debt in Heist is `.planning/todos/pending/2026-09-17-split-heist-main-and-test-its-stealth.md`.

### L4. docs/OVERVIEW.md "Not built yet", other than what this roadmap pulls in

Source for all: `docs/OVERVIEW.md` "Not built yet".
Pulled into this roadmap: lit detection ranges and the light-averse tactic (LIT-01, LIT-02).

- **LATER-23 Cursed items**: items that resist removal or carry a deliberate penalty.
- **LATER-24 Heat and cold, liquids that flow, wind**: new rules over `TileField<T>`, not new field types.
  Also `docs/design/fields.md` section 6.
- **LATER-25 A ranged penalty in the dark**: gated in the source on accuracy existing, which `docs/design/abilities.md` rules out ("Accuracy does not exist"); it would follow LATER-11's miss-as-a-damage-stage pattern.
  Also `docs/design/lighting.md` phase E.
- **LATER-26 Mouse-to-tile in `rl-render`**, which hover, tooltips and click-to-travel wait on.
  Also `docs/design/ui.md` section 8.
- **LATER-27 Instanced terminal rendering**: see `.planning/todos/pending/2026-09-17-instanced-terminal-rendering.md`.
  Also `docs/PLAN.md` sections 2 and 6 (a locked decision not yet met).
- Nights on Corsair, scripted encounters and Bevy UI presenters are also on this list; see LATER-05, LATER-02 and LATER-06.

### L5. Documentation drift

Source: `.planning/INGEST-CONFLICTS.md` INFO entries and `.planning/intel/requirements.md` section B.

- **LATER-28 Lighting opt-in wording**: `docs/design/lighting.md` section 3, `docs/OVERVIEW.md` and `docs/guide/src/09-where-to-go-next.md` say inserting `Lighting` turns lighting on; the code and PLAN progress 2026-09-11 say adding `LightingPlugin`, which inserts `Lighting::dark()`.
  Phases 4 and 5 edit these same paragraphs, so the fix may ride along with them.
- **LATER-29 Superseded design-doc sections**: `docs/design/ui.md` (`PresentSet::Narrate`, `EquipView`, "warns once", `EnginePlugins`, Lamplight), `docs/design/abilities.md` (effect list, section 2 table, section 8), `docs/design/stealth.md` section 0 (player viewshed as the one oracle).
  Phase 2 edits `docs/design/stealth.md`, so its section 0 fix may ride along.
- **LATER-30 OVERVIEW stamp, guide self-reference, missing PLAN entries, stale changelog claim**: the stamp and the camera clamp are `.planning/todos/pending/2026-09-17-overview-camera-clamp-drift.md`; the rest are chapter 9's "the tests in chapter 9", no progress entry for the guide restructure or `examples/heist`, and `docs/TODO.md` section 6 saying there is no `CHANGELOG.md`.
- **LATER-31 Theme words in engine doc comments**: see `.planning/todos/pending/2026-09-17-theme-words-in-engine-doc-comments.md`.

### L6. Auto-explore and travel-to

- **LATER-32 Auto-explore and travel to a seen tile** within the active region, simulating every step, with an `ExploreInterrupt` trait for stop rules; no implementation exists in `crates/`.
  Source: `docs/PLAN.md` sections 3.5.2, 3.7 and 5.

### L7. World generation and Corsair's sea

- **LATER-33 M7 Corsair sea scope**: ships as vehicles with a sailing profile, boarding that pushes a deck map, navy, pirates and merchants as factions, a treasure-map dig quest as data over events, weather as a `TileField<T>`.
  Source: `docs/PLAN.md` section 8, M7.
- **LATER-34 M4 world-generation deferrals**: river channels and bridges inside chunks beyond the channel pass, fords, cullers beyond keep-largest, a choke map with worklist pruning, decoration rules, settlement passes richer than huts.
  Source: `docs/PLAN.md` section 3.4, M4, progress 2026-09-10 ("Deferred from M4").
- **LATER-35 Sconces as prefab marks**: a room lit by authored light in a prefab.
  The mechanism exists (prefab marks become `Spot`s, `crates/rl-bevy/src/places.rs:113`), so what remains is content in a game's prefabs, not engine lighting work.
  Source: `docs/design/lighting.md` phase E and section 1 ("Authored light in prefabs").
- Generated victory and encounters: see LATER-03.

### L8. Speed and benches

- **LATER-36 Speed through a `SpeedModifier` trait or an `ActionCost` message**, not by reading named statuses; neither name exists in `crates/`, and every actor already requires a `Speed`, so check the code before scoping.
  Source: `docs/PLAN.md` section 3.6.
- **LATER-37 Missing benches**: the choke map, each builder, the world chain at three sizes and the render sweep, plus the per-frame loop: see `.planning/todos/pending/2026-09-17-bench-the-per-frame-loop.md`.
  Source: `docs/PLAN.md` section 3.9.

### L9. Crate family name and publishing

Source for all: `docs/TODO.md` section 6.

- **LATER-38 The crate family name**: `rl-core` is taken on crates.io; choose a family (candidates in the source), reserve it the same day, and rename 297 references across 153 files.
  Open owner decision; it does not contradict the locked repo name `rl-engine`.
- **LATER-39 Publish readiness**: version requirements on the eleven `[workspace.dependencies]` path entries, readmes for the ten crates without one, `rl-engine`'s out-of-package readme path, tier-order dry runs, `cargo-release` or `release-plz`.
- **LATER-40 Publish staging**: the five Bevy-free crates first; the Bevy layer stays a git dependency until its API settles.
  The source's "no `CHANGELOG.md`" is stale.

### L10. Other captured todos

- **LATER-41 Sanitize save slot names**: see `.planning/todos/pending/2026-09-17-sanitize-save-slot-names.md`.
- **LATER-42 Take `Color` out of log spans**: see `.planning/todos/pending/2026-09-17-take-color-out-of-log-spans.md`.

## Out of Scope

Excluded by locked decisions or the design docs, not deferred.

| Feature | Reason |
|---------|--------|
| An engine sneak-attack damage multiplier | Balance is the game's; the engine ships only `Defender::unaware` (stealth.md phase E) |
| Squad alerting and shout propagation rules in the engine | `Noticed` is the seam; what a shout carries and who it reaches are content (stealth.md sections 6 and 9) |
| Noise as a knob on `Notice` | Noise is a separate sense with its own propagation (stealth.md section 9) |
| Porting `roguelike_engine`'s `stealth/noise.rs` or `squad/` | Not ported (PLAN section 6) |
| A day and night cycle or an ambient rules trait in the engine | Ambient is data the game writes (lighting.md sections 0 and 3) |
| Accuracy or a to-hit roll in the engine | "Accuracy does not exist; a to-hit roll would be a damage stage" (abilities.md) |
| Flicker affecting what is seen | `waver` is the renderer's alone (lighting.md section 5) |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| SNEAK-01 | Phase 1 | Pending |
| SNEAK-02 | Phase 1 | Pending |
| SNEAK-03 | Phase 1 | Pending |
| SEEN-01 | Phase 2 | Pending |
| SEEN-02 | Phase 2 | Pending |
| SEEN-03 | Phase 2 | Pending |
| SEEN-04 | Phase 2 | Pending |
| NOISE-01 | Phase 3 | Pending |
| NOISE-02 | Phase 3 | Pending |
| NOISE-03 | Phase 3 | Pending |
| NOISE-04 | Phase 3 | Pending |
| NOISE-05 | Phase 3 | Pending |
| LIT-01 | Phase 4 | Pending |
| LIT-02 | Phase 4 | Pending |
| SHADE-01 | Phase 5 | Pending |
| SHADE-02 | Phase 5 | Pending |
| SHADE-03 | Phase 5 | Pending |
| SHADE-04 | Phase 5 | Pending |

**Coverage:**
- Roadmap requirements: 18 total
- Mapped to phases: 18
- Unmapped: 0

---
*Requirements defined: 2026-09-17*
*Last updated: 2026-09-17 after roadmap creation*
