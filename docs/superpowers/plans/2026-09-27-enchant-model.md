# One place for what a level does: implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** An item's `enchant` block is the one place that says what a level does; every Foundry weapon and plate takes levels; pulse timing and worn-only refills move off the item's top level; grenades are heard where they land.

**Architecture:** The engine stops knowing about levels: a carrier's `EffectBonus` (written by the game at spawn, level applied) reaches its effects through the `Landing`. `TriggerSpec` gains `every` for pulse triggers. `NoisePlugin` registers a `Noise` effect. Foundry's `ItemDef` gains a single `EnchantDef` and applies it in `spawn_item_at`.

**Tech Stack:** Rust 2024, Bevy ECS, `serde`/`ron`, mdBook guide pages.

**Spec:** `docs/superpowers/specs/2026-09-27-enchant-model-design.md`. Read it first; its sections 2 and 3 are the binding model and numbers.

## Global Constraints

- Never an em dash, code or prose. No `Co-Authored-By` line. Commit messages are lower-case sentences saying what is now true.
- **Per commit (the fast gate, per `AGENTS.md`):** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and the tests of the crates touched, plus the `CHANGELOG.md` line for any change a game would have to follow, written in the same commit. Do not bless guide pages per commit.
- **Once, at the end (Task 5):** everything CI runs, `python3 scripts/check-systems.py` and the guide pass included, over the branch as a whole.
- Tier 0 and 1 crates stay Bevy-free and wasm-clean. No `HashMap`/`HashSet`. No `TODO` in source. Doc comments say why.
- No theme words in engine crates: engine vocabulary is `bonus`, `every`, `noise`; `cloak`, `nanite`, `blast`, `hiss` are Foundry's.
- Every RON schema's top comment lists its full option space, updated in the same commit as the field.
- Tests are named as sentences describing the property; property over a seed range where one exists.
- The numbers (spec section 2): `max` 3 for plain armor and helmets, 5 for weapons, 9 for the nanite and cloak plates; mono-axe and slug rifle `damage: 2`; rangefinder `dark_sight: 1`; nanite `armor: 0, pulse: -100` with its trigger `every: 1000`; cloak `armor: 0, turns: 2`, `Inflict` `turns: 10`, recharge `(every: 4000, while_worn: true)`; grenades `Noise` `blast` 14 (frag, ion, incendiary), `hiss` 6 (smoke).

## Review Focus

1. An item at `+0` is exactly what it was: same armor, rolls, period, no bonus (Task 4).
2. A thrown weapon's strike grows with its level as its blow does (Task 4).
3. A continued run's `+N` items come back with their bonuses (Task 4).
4. A grenade landing out of sight is heard (Task 3, Task 4).
5. A level rule in the wrong place fails at load naming the item: `per_level` left in effect args, `every` on a non-pulse trigger (Tasks 1, 2).

---

## File map

| File | Change | Responsibility |
|---|---|---|
| `crates/rl-bevy/src/effects/mod.rs` | modify | `EffectBonus`; `Landing::bonus` replaces `level`; `Effect::describe` takes the bonus |
| `crates/rl-bevy/src/effects/engine.rs` | modify | `Harm`, `Mend`, `Inflict` add the bonus; `per_level` removed; args refuse unknown fields |
| `crates/rl-bevy/src/effects/triggers.rs` | modify | `land_triggers` reads `EffectBonus`; `Trigger::every`; `Triggers::build` validates `every` |
| `crates/rl-rules/src/ability.rs` | modify | `TriggerSpec::every` |
| `crates/rl-bevy/src/noise.rs` | modify | the `Noise` effect, registered by `NoisePlugin` |
| `crates/rl-bevy/src/ability.rs`, `props.rs`, any `Landing { .. }` | modify | `bonus: EffectBonus::default()` |
| `crates/rl-ui/src/view/inventory.rs` | modify | describe a carrier's effects with its `EffectBonus` |
| `examples/corsair/src/abilities.rs`, `examples/delve/src/effects.rs` | modify | `describe` takes the bonus |
| `examples/foundry/src/gear.rs` | modify | `EnchantDef`, `RechargeDef`; `spawn_item_at` applies the level; validation |
| `examples/foundry/src/main.rs` or `plugin.rs`, `gear::effect_kinds` | modify | declare `blast` and `hiss`; register `Noise` where Foundry builds its effect kinds |
| `examples/foundry/assets/items.ron` | modify | the schema comment; `enchant` on every weapon and armor; pulse `every`; recharge; grenade noise |
| docs (Task 5) | modify | guide pages, design notes, OVERVIEW, CHANGELOG review |

---

## Task 1: An effect grows by its carrier's bonus, not by a level

**Files:** `crates/rl-bevy/src/effects/{mod.rs,engine.rs,triggers.rs}`, every `Landing { .. }` literal (grep), `crates/rl-bevy/src/ability.rs`, `crates/rl-ui/src/view/inventory.rs`, `examples/corsair/src/abilities.rs`, `examples/delve/src/effects.rs`, `crates/rl-bevy/src/consumable.rs` tests, `CHANGELOG.md`.

**Interfaces produced:**

```rust
/// What a carrier's effects land stronger by: turns added to every status
/// they inflict and an amount added to every harm or mend roll. Written by
/// the game when it spawns the thing, whatever made it stronger (an enchant
/// level, a blessing) already applied, as `Bestows` and `Armor` are; the
/// engine never learns why.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EffectBonus {
    /// Whole turns added to every status an effect inflicts.
    pub turns: u32,
    /// Added to every harm or mend roll's bonus.
    pub amount: i32,
}
```

- `Landing::bonus: EffectBonus` replaces `Landing::level`. `land_triggers` copies the carrier's `EffectBonus` (default when absent). Abilities and `land_on` pass `EffectBonus::default()`.
- `Effect::describe(&self, registries: &Registries, bonus: EffectBonus) -> String`; `Effects::describe(registries, bonus)`.
- `Harm` and `Mend`: `pub fn roll_with(&self, bonus: EffectBonus) -> DiceRoll` (roll bonus + `bonus.amount`); `Inflict`: `pub fn turns_with(&self, bonus: EffectBonus) -> u32`. Remove `per_level` fields and the old `roll_at`/`turns_at`.
- The three effects' `Args` structs gain `#[serde(deny_unknown_fields)]`, so a `per_level` left in a file is a load error naming the field. Check every engine effect's `Args` and apply the same to all of them (an unknown argument is always a typo).

- [ ] Tests first, in `effects/engine.rs`'s tests: a bonus adds its amount to harm and mend rolls and its turns to an inflicted status, and a default bonus adds nothing; `per_level` in any of the three effects' args is refused with an error naming it; a typo'd arg on another engine effect is refused.
- [ ] In `consumable.rs` tests, replace the enchanted-landing test: a carrier with `EffectBonus { amount: 6, .. }` mends 4 + 6.
- [ ] In `crates/rl-ui/src/view/inventory.rs` tests: a row whose item has `EffectBonus { turns: 4, .. }` and an `Inflict` of 10 turns reads `for 14 turns`.
- [ ] Implement; update every `Landing { .. }` literal and every `describe` override in the workspace (Corsair `Plunder`, Delve `Drain`).
- [ ] Foundry still compiles: in `examples/foundry/src/gear.rs`, where the cloak plate relied on `Enchant` for its landing level, insert `EffectBonus { turns: 2 * level, amount: 0 }` for the cloak plate for now (Task 4 replaces this with the general rule), and remove `per_level` from `items.ron`'s cloak `Inflict` (its `turns` stays 10). Foundry's cloak-duration test must still pass.
- [ ] CHANGELOG under Unreleased: replace the earlier "effect lands at the enchant level" entry's mechanism: effects grow by the carrier's `EffectBonus`; `per_level` is gone from `Harm`, `Mend` and `Inflict`; `describe` takes an `EffectBonus` (a game's own effect updates its signature); effect args refuse unknown fields.
- [ ] Fast gate; commit: "an effect grows by what its carrier's bonus adds, and the engine no longer knows what a level is".

## Task 2: A pulse's period is its trigger's

**Files:** `crates/rl-rules/src/ability.rs` (`TriggerSpec`), `crates/rl-bevy/src/effects/triggers.rs` (`Trigger`, `Triggers::build`), `CHANGELOG.md`.

- `TriggerSpec::every: Option<u32>` (`#[serde(default)]`), doc: hundredths of a step between pulses, for a `pulse` trigger only.
- `Trigger::every: Option<u32>` carried through `Triggers::build`; `Triggers::pulse_every(&self) -> Option<u32>` returns the first pulse trigger's period.
- `Triggers::build` errors: a `pulse` trigger with no `every` ("a pulse trigger needs `every`, the hundredths between pulses"); an `every` on any other moment ("`every` is only for a pulse trigger"); `every: 0`.
- [ ] Tests first (triggers.rs tests): each of the three errors, reported together with the item's other errors; a pulse trigger with `every` builds and `pulse_every` returns it.
- [ ] Implement. Update every `TriggerSpec { .. }` literal.
- [ ] Foundry still loads: its nanite plate's pulse trigger gains `every: 1000` now; Foundry keeps its old `pulse:` field until Task 4 but reads the period from the trigger if you prefer; the nanite tests must pass.
- [ ] CHANGELOG: `TriggerSpec` gains `every`; a pulse trigger must carry it; a `TriggerSpec` literal adds `every: None`.
- [ ] Fast gate; commit: "a pulse trigger carries its own period, and one without it, or an every on any other trigger, is refused at load".

## Task 3: A trigger can make a noise

**Files:** `crates/rl-bevy/src/noise.rs`, `crates/rl-bevy/src/effects/` only if the sound-name resolution needs it, `CHANGELOG.md`.

- `pub struct Noise { pub sound: SoundId, pub loudness: i32 }`, `FromArgs` kind `"Noise"`, args `(sound: "name", loudness: N)` with unknown fields refused; `apply` writes one `MakeNoise { at: landing.landed_at.unwrap_or(landing.aim), loudness, sound, maker: Some(landing.user) }`; `describe` returns an empty string (a noise is not what a thing does to anyone).
- Registered by `NoisePlugin::build` with `add_effect::<Noise>()`, as `FirePlugin` registers `Ignite`, so a file naming it works exactly when the game has noise.
- The sound name must resolve when effects are built, and an unknown sound must be a load error naming it. `FromArgs::from_args` receives `&Names`; if `Names` cannot see the `Sounds` interner today, extend it the smallest clean way (for example `Names::sounds(&Sounds)` as `gases` is carried) and make every place that builds effects pass it; say what you chose in the report.
- [ ] Tests first: a trigger with `Noise` landing makes one `MakeNoise` at the landing cell, by the user, at the given loudness, heard by a listener in range and not by one out of range; an unknown sound fails to build; a game without `NoisePlugin` naming `Noise` fails to build (the kind is unregistered).
- [ ] Implement.
- [ ] CHANGELOG: the `Noise` effect.
- [ ] Fast gate; commit: "a trigger can make a noise where it lands, through a noise effect the noise plugin registers".

## Task 4: Foundry's items take one enchant block

**Files:** `examples/foundry/src/gear.rs`, `examples/foundry/src/save.rs` (only if needed), where Foundry declares sounds and builds effect kinds (`gear::effect_kinds`, `main.rs`/`plugin.rs`, `testing`), `examples/foundry/assets/items.ron`, `CHANGELOG.md`.

- `EnchantDef { max: i32, armor: Option<i32>, damage: Option<i32>, dark_sight: i32, pulse: i32, turns: u32, amount: i32 }` with serde defaults (`armor`/`damage` `None` meaning the inferred default), doc per spec section 2. `RechargeDef { every: u32, while_worn: bool }`; `ConsumableDef::recharge: Option<RechargeDef>`. Remove `ItemDef::pulse`, `ItemDef::attuned`, `PulseDef`, the old `EnchantDef { most }`.
- `EnchantDef` helpers: `armor_per_level(&self, def: &ItemDef) -> i32` (explicit, else 1 when `def.armor != 0`, else 0) and `damage_per_level(&self, def: &ItemDef) -> i32` (explicit, else 1 when the item has a melee, ranged or throw strike, else 0).
- `spawn_item_at` at `level` (clamped to `0..=max`, and 0 for a thing with no `enchant`): `Armor(def.armor + armor_per_level * level)` (inserted when nonzero); each of `MeleeAttack`, `RangedAttack` and the throw strike's dice gets `bonus + damage_per_level * level`; `WornDarkSight(def.dark_sight + enchant.dark_sight * level)`; `Pulse::every((trigger_every + enchant.pulse * level).max(100))` from the armory's built triggers' `pulse_every()`; `EffectBonus { turns: enchant.turns * level, amount: enchant.amount * level }` when either is nonzero; `Attuned` when `recharge.while_worn`. Remove the Task 1 cloak stopgap.
- Validation refuses (each its own message): `enchant` on a thing with no slot or that stacks; `max < 1`; `dark_sight` in `enchant` without the item's own `dark_sight`; `pulse` without a pulse trigger; `turns` or `amount` on an item with no effects; `recharge.while_worn` without a slot.
- `items.ron`: the schema comment documents `enchant` (every key, its default, "base numbers stay where they are; everything a level changes lives here"), `recharge: (every:, while_worn:)`, the pulse trigger's `every`, and the `Noise` effect; every weapon gets `enchant: (max: 5)` (mono-axe and slug rifle `damage: 2`); every plain armor and helmet `enchant: (max: 3)` (rangefinder `dark_sight: 1`); the nanite and cloak plates as in spec section 3; the four grenades' land triggers gain `Noise` (`blast` 14 for frag, ion, incendiary; `hiss` 6 for smoke). Declare the `blast` and `hiss` sounds wherever Foundry sets up noise, and register `Noise` in `gear::effect_kinds` and every place Foundry builds an armory outside the app.
- [ ] Tests first, in `gear.rs`:
  - every item at `+0` is what it was before this task: for each definition, its `Armor`, attack rolls, `WornDarkSight`, `Pulse::every` and absence of `EffectBonus` equal the values the pre-task file gave (write the expected table from the current `items.ron`);
  - a `+3` composite plate has armor 5; a `+2` mono-axe swings `2d6+5`; a `+2` monoblade's thrown strike and blow both gain 2; a `+2` rangefinder sees 8 in the dark; a `+9` nanite plate pulses every 100 and a `+12` one no faster; a `+2` cloak plate cloaks for 14 turns (keep the Unseen-duration measurement from the earlier cloak test);
  - an item found at a band above its `max` is spawned at `max`;
  - each validation rule refuses its case;
  - a thrown frag grenade landing out of the commando's sight makes a noise a droid within 14 hears, and the commando's noise meter reads it;
  - a continued run keeps a `+2` mono-axe's roll and a `+2` rangefinder's sight.
- [ ] Implement.
- [ ] CHANGELOG: Foundry's item schema: `enchant` is one block with its keys and defaults; every weapon and piece of armor is enchantable; `pulse` and `attuned` moved onto the pulse trigger and the recharge; grenades are heard.
- [ ] Fast gate; commit: "foundry: an item's enchant block says everything a level does, every weapon and plate takes levels, and grenades are heard".

## Task 5: The branch's documentation pass and the running game

- [ ] Run everything CI runs: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `scripts/check-tiers.sh`, `scripts/check-tiers.sh --wasm`, `scripts/check-overview.sh`, `python3 scripts/check-systems.py`, `scripts/check-guide.sh`.
- [ ] For every page `check-systems.py` names, read the `git diff` it prints against the page, fix what is untrue (effects.md's argument lists and `Landing`, items.md's worn things and enchant, noise.md's effect, abilities.md's `describe`), then bless; `scripts/check-systems-style.sh` each edited page.
- [ ] Design notes: `docs/design/effects.md`'s "Effects at a level" section rewritten for `EffectBonus`; `docs/design/items.md` on the enchant block's rule; `docs/design/noise.md` on the `Noise` effect. `docs/OVERVIEW.md` and the spec of the previous branch (`docs/superpowers/specs/2026-09-26-worn-gear-and-the-unseen-design.md`) where they name `per_level`, `Landing::level`, `pulse:` or `attuned:` as current.
- [ ] Re-read the whole `CHANGELOG.md` Unreleased section for entries this branch made untrue (the earlier `per_level` and `Landing::level` lines) and correct them.
- [ ] In the running game (the capture harness in `crates/rl-render/src/capture.rs`, `FOUNDRY_START`, the cheat menu): the bag rows of a `+N` weapon, plate and helmet show their raised numbers; the cloak plate's row reads its level's turns; a grenade thrown out of sight shows on the commando's noise meter. Fix anything that looks off, with a test.
- [ ] Commit each fix and the documentation pass: "the reference and the design notes say how a level is written, once, in an item's enchant block".
