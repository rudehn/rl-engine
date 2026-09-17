# Constraints

Synthesized from the four SPEC documents in the ingest set: lighting, stealth, UI and abilities design docs.
Each of these docs records its own as-built revisions (a "what the build changed" section or a revised preamble), and those revisions supersede the doc's earlier sections.
Where a SPEC statement contradicts the locked ADR (`docs/PLAN.md` progress log) or the code, the ADR wins and the stale statement is listed in `.planning/INGEST-CONFLICTS.md` as INFO.
Entries below state the effective contract.

Types used: api-contract, schema, nfr, protocol.

---

## Lighting

source: /Users/nathanrude/Development/rl-engine/docs/design/lighting.md
status in source: phases A to C built 2026-09-11; D to F proposed (not binding)

### CON-light-gate-single-writer
- type: protocol
- source: /Users/nathanrude/Development/rl-engine/docs/design/lighting.md (section 0, decision 1; section 3 "the gate")
- content: The light gate lives in `update_viewsheds` and nowhere else.
  `Viewshed::line` is the geometric shadowcast and `Viewshed::visible` is what is seen; `visible = line ∩ (lit ≥ threshold ∪ within DarkSight ∪ adjacent)`.
  `Knowledge::mark` runs over `visible`.
  Every mind carries its own `Viewshed`, gated by the same code as the player's.

### CON-light-channels
- type: api-contract
- source: /Users/nathanrude/Development/rl-engine/docs/design/lighting.md (section 0 decision 2, section 2)
- content: Brightness (`intensity: u8`) and colour are separate channels; gameplay reads intensity only.
  A `waver` channel is read only by the renderer, so flicker never changes what is seen.

### CON-light-opt-in
- type: api-contract
- source: /Users/nathanrude/Development/rl-engine/docs/design/lighting.md (section 0 decision 3); effective form per /Users/nathanrude/Development/rl-engine/docs/PLAN.md progress 2026-09-11 and crates/rl-bevy/src/lighting.rs:306-315
- content: Lighting is opt-in by adding `LightingPlugin`, which inserts `Lighting::dark()`; without the plugin there is no gate and no cost.
  The engine names no torch, sun or lava; ambient is one `Light` per map that the game writes; no day cycle in the engine.

### CON-light-integer-determinism
- type: nfr
- source: /Users/nathanrude/Development/rl-engine/docs/design/lighting.md (sections 2, 5)
- content: Integer falloff, full within `radius / 3`, linear to exactly zero at the rim.
  Screen blending per channel.
  Emitters are sorted by origin then intensity before casting so the field is byte-identical regardless of ECS order (property test shuffles input).
  Lighting draws from no RNG and is never persisted; a load rebuilds it.

### CON-light-cost
- type: nfr
- source: /Users/nathanrude/Development/rl-engine/docs/design/lighting.md (sections 3, 6)
- content: No allocation per cast (caller-owned scratch `BitGrid` and field).
  Bench `light/20_sources_radius_8` in `crates/rl-grid/benches/grid.rs`; measured 70 µs cast and 52 µs compose over 96x64 on an M1.
  Static and dynamic layers recast only when their sorted emitter lists differ (as built, phase B note).
  Opacity changes bump `opacity_epoch`, which refreshes light and every viewshed.

### CON-light-sources
- type: api-contract
- source: /Users/nathanrude/Development/rl-engine/docs/design/lighting.md (section 3)
- content: One `LightSource { intensity, radius, color }` component serves props, actors and items; a carried item sheds from its carrier.
  `DarkSight(i32)`, absent means adjacency only.
  `Fuel(u32)` removes the `LightSource` at zero and writes `LightEvent::BurntOut` once.
  Fuel and item light state are the game's to save.

### CON-light-tests
- type: nfr
- source: /Users/nathanrude/Development/rl-engine/docs/design/lighting.md (section 8)
- content: Headless ASCII-fixture tests for falloff, commutativity, occlusion, epoch bumps, adjacency floor, dark sight, lit versus doused detection by a mind, carried light shedding, fuel exhaustion, and every existing test unchanged without lighting.

---

## Stealth and awareness

source: /Users/nathanrude/Development/rl-engine/docs/design/stealth.md
status in source: phases A to D built 2026-09-12; E proposed; section 12b supersedes earlier sections

### CON-stealth-types
- type: api-contract
- source: /Users/nathanrude/Development/rl-engine/docs/design/stealth.md (sections 2, 12b)
- content: Tier 1 (`rl-rules::ai::awareness`): `NoticeStats { certain, chance_pct, lit_bonus, memory }`, `StealthStats { quiet, subtlety }`, pure `notices(distance, notice, stealth, lit: bool, roll)` which does not take perception, and `Awareness`.
  Tier 2 (`rl-bevy::stealth`): components `Notice`, `Stealth`, `Aware(BTreeMap<Entity, Awareness>)`, message `Noticed { observer, subject, at }`, `StealthPlugin`, `StealthRunning`.
  `quiet` floors the certain radius at one.

### CON-stealth-state-machine
- type: protocol
- source: /Users/nathanrude/Development/rl-engine/docs/design/stealth.md (sections 4, 12b)
- content: `Awareness` has two variants, `Unaware` and `Alert { at, stale_turns }`.
  A sighting resets staleness; losing takes `memory` turns out of sight; an alert observer keeps its subject while it can perceive it, and the roll only decides whether an unaware observer becomes aware.
  `Noticed` is written once on the flip.

### CON-stealth-scheduling
- type: protocol
- source: /Users/nathanrude/Development/rl-engine/docs/design/stealth.md (sections 3, 6, 12b)
- content: `update_awareness` runs in `DecideSet::Notice` before the minds, for the actor holding the turn.
  `wake_on_damage` reads `DamageDealt` in `TurnSet::React`.
  Whether stealth runs is decided by `StealthRunning` (the plugin), not by components.
  The observer's own `Viewshed` is the one line-of-sight answer for perceive, noticing and `Watchers`.
  Rolls come from `StealthRng` (per /Users/nathanrude/Development/rl-engine/docs/design/minds.md section 3.4).

### CON-stealth-ui
- type: api-contract
- source: /Users/nathanrude/Development/rl-engine/docs/design/stealth.md (sections 8, 12b)
- content: `Row::aware: Option<bool>` (None when no stealth runs) and `VitalsView::seen`, both reading `rl_bevy::Watchers`.

### CON-stealth-boundaries
- type: api-contract
- source: /Users/nathanrude/Development/rl-engine/docs/design/stealth.md (sections 6, 9)
- content: Squad propagation is the game's, over `Noticed`.
  The engine ships no sneak-attack multiplier; phase E adds only `Defender::unaware` so a game writes its own damage stage.
  Noise is a separate sense, not a knob on `Notice`.

### CON-stealth-tests-cost
- type: nfr
- source: /Users/nathanrude/Development/rl-engine/docs/design/stealth.md (sections 11, 12)
- content: Pure property tests over seed ranges for certain radius, lit bonus, quiet floor, bounded noticing time, and exact forgetting turn; headless tests for dark versus lit hunting, waking on a blow, last-known search, single `Noticed`, and unchanged behaviour without the plugin.
  One roll per stealthed subject per actor-turn; nothing per frame.

---

## UI

source: /Users/nathanrude/Development/rl-engine/docs/design/ui.md
status in source: phases A to G built 2026-09-12; H proposed; section 11b supersedes earlier sections

### CON-ui-three-way-split
- type: protocol
- source: /Users/nathanrude/Development/rl-engine/docs/design/ui.md (section 0, 11b); effective set names per crates/rl-ui/src/lib.rs:139-147 and /Users/nathanrude/Development/rl-engine/docs/PLAN.md progress 2026-09-12
- content: Every panel is a view (a resource of plain data: no `Color`, no rect, no string the game did not supply), a collector (refills the view every frame in `ViewSet::Collect`), and a presenter (draws in a `PresentSet` layer, taking its rectangle in its constructor).
  Game annotations run in `ViewSet::Annotate`; narration in `ViewSet::Speak`.
  Views are tier 2 (they carry `Entity`); derivations are tier 1 in `rl-rules::forecast`.
  Opt-in is per panel.

### CON-ui-tones
- type: api-contract
- source: /Users/nathanrude/Development/rl-engine/docs/design/ui.md (section 4, 11b)
- content: A widget takes a `ToneId`, never a `Color`.
  `Tones` interner plus `Palette` indexed by dense id, extended by games with `add_tone(name, colour)`.
  An uncoloured tone is reported by name at startup, not on use.

### CON-ui-facets-and-names
- type: api-contract
- source: /Users/nathanrude/Development/rl-engine/docs/design/ui.md (sections 2, 11b, 12)
- content: `Facet { key, text, tone }` pushed onto a row for what the engine cannot know.
  Two games pushing the same facet key signals the field belongs in the view.
  Rows use Bevy's `Name` (not an engine `Label`); no `Describe` trait.

### CON-ui-modals-input
- type: protocol
- source: /Users/nathanrude/Development/rl-engine/docs/design/ui.md (section 7)
- content: `Modals` is a stack of interned ids with `modal_is`, `modal_open`, `no_modal`; engine input gated on an empty stack.
  `InSight` and `Focus` are the one list both cursors cycle.
  Keys are declared once in `Controls` and read through `ControlInput`; `ControlsPanel` draws that registry.

### CON-ui-scope-limits
- type: api-contract
- source: /Users/nathanrude/Development/rl-engine/docs/design/ui.md (section 8, 12)
- content: The engine does not own main menus, settings, save-slot screens, rebinding screens, text entry, scrollbars, drag or focus traversal.
  Hover and tooltips wait on a mouse-to-tile query in `rl-render`.
  Test for engine ownership: the panel needs engine state to build.
  No layout resource (`ChromeLayout` removed; `panel::split_*` helpers).

### CON-ui-backend
- type: api-contract
- source: /Users/nathanrude/Development/rl-engine/docs/design/ui.md (section 5)
- content: Terminal presenters ship; Bevy UI node presenters (phase H) only when a game asks, behind a feature or a separate crate if dependency weight justifies it.
  The map view stays a glyph grid.

### CON-ui-tests
- type: nfr
- source: /Users/nathanrude/Development/rl-engine/docs/design/ui.md (section 10)
- content: Headless row tests, facet targeting, seed-range property on row distance and viewshed membership, exact terminal text presenter tests, palette override, modal input refusal, and arithmetic tested only in `rl-rules`.

---

## Abilities

source: /Users/nathanrude/Development/rl-engine/docs/design/abilities.md
status in source: phases A to F built 2026-09-12, revised 2026-09-13; preamble records deviations that supersede the body

### CON-ability-def-schema
- type: schema
- source: /Users/nathanrude/Development/rl-engine/docs/design/abilities.md (sections 3.1, 3.6); /Users/nathanrude/Development/rl-engine/docs/guide/src/05-a-knack.md (as-built RON header)
- content: `AbilityDef` fields: `name`, optional `description`, optional `look`, `aim` (`Foe` default, `Ally`, `SelfOnly`, `Ground`, `Anyone`), `mode` (`Own`, `Adjacent`, `Bolt`, `Ball`, `Beam`, `Cone`), optional `sight`, optional `requires` (`Has`, `Lacks`, `Wielding`, `InSlot`, `Above`), optional `costs` (`Pool`, `Charge`, `Health`, `Item`), optional `time` (hundredths, default 100), optional `cooldown` (hundredths, default 0), `effects` as `(kind, chance, args)` with args as raw RON parsed by the registered effect.
  Every name resolves through `Names` at load; unknown names are reported together at startup.

### CON-ability-closed-enums-justified
- type: api-contract
- source: /Users/nathanrude/Development/rl-engine/docs/design/abilities.md (sections 3.2, 3.3)
- content: `Aim` and `Cost` are deliberately closed: they enumerate what the engine's own faction matrix can answer and what engine subsystems can decrement, not content.
  A new fuel is a registered stat used through `Pool`.

### CON-ability-effects-are-types
- type: api-contract
- source: /Users/nathanrude/Development/rl-engine/docs/design/abilities.md (sections 0, 3.5); effective list per /Users/nathanrude/Development/rl-engine/docs/PLAN.md progress 2026-09-13 and 2026-09-15
- content: `trait Effect` in `rl-bevy` with `apply(&Landing, &mut EffectWorld)` and `describe`; registered with `add_effect`; no `Custom { id }`, no effect enum.
  Engine effects: `Harm`, `Mend`, `Inflict`, `Cleanse`, `Shove`, `Pull`, `Teleport` (in `effects`, via `AddEngineEffects`), plus `Ignite` and `Emit` registered by the fire and gas plugins.
  `EffectWorld` exposes requests and `Commands`, never component queries.
  Combat and statuses do not depend on abilities.

### CON-ability-one-landing
- type: protocol
- source: /Users/nathanrude/Development/rl-engine/docs/design/abilities.md (revision 2026-09-13, sections 4, 6)
- content: `Aim::hits`, `Aim::worth_aiming_at` and `aim_blocked` in `rl-rules` are the rules; `Bystanders::land` in `rl-bevy` is the one call both the resolver and the targeting preview use; a property test over seeded layouts holds the preview to the resolver.
  An aim past the shape's reach is refused as `Blocked::OutOfReach` (PLAN progress 2026-09-15).

### CON-ability-costs-and-state
- type: protocol
- source: /Users/nathanrude/Development/rl-engine/docs/design/abilities.md (sections 3.3, 4, 5, 7)
- content: Costs are all-or-nothing, paid once after the gate and before the effects; a refused use is free and keeps the player's turn.
  Cooldowns are absolute times on the turn clock and count from the moment of use.
  `Known` is rebuilt, not edited.
  Pools, cooldowns and charges are saved in `EngineSave`.
  `AbilityRng` is derived from `Seed` on its own domain; chances rolled once per effect per use, in order.
  Effects run inside `Resolve`, not `React`.

### CON-ability-minds
- type: api-contract
- source: /Users/nathanrude/Development/rl-engine/docs/design/abilities.md (section 6)
- content: A mind must be able to use abilities: the gate filters usable abilities into the snapshot, the `UseAbility` tactic scores footprints by `Aim`, and `Decision::Ability { id, aim }` is an engine decision.
  Accuracy does not exist; a to-hit roll would be a damage stage.

### CON-ability-tests
- type: nfr
- source: /Users/nathanrude/Development/rl-engine/docs/design/abilities.md (section 9)
- content: Pure gate and payment tests; seed-range properties for sight and cooldown; refusal costs; save round trip of cooldowns and pools; a labelled fingerprint tripwire; content refusal naming every bad name at once; `crates/rl-bevy/tests/genres.rs` loads five genres into one registry.
