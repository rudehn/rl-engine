# Item Triggers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make effects a subsystem of their own, give props and items one `Triggers` component keyed by an open registry of moments, turn `Consumable` into a count of charges, and take ability-lending off items.

**Architecture:** `effects.rs` owns everything that lands an effect list and a new `EffectsPlugin`; a subsystem that owns a moment writes `Fired` and nothing else; one system, `land_triggers`, lands every entity's triggers for that moment; `ConsumablesPlugin` spends charges from the same messages. Abilities, props and items are peers over it.

**Tech Stack:** Rust 2024, Bevy 0.19, RON content, `cargo test`, the repo's `scripts/check-*.sh` and `scripts/check-systems.py`.

**Spec:** `docs/superpowers/specs/2026-09-23-item-triggers-design.md`. Read it first; this plan argues from it and does not repeat its reasoning.

## Global Constraints

- Tier 0 and 1 crates (`rl-core`, `rl-grid`, `rl-rules`, `rl-world`) take no Bevy dependency; `scripts/check-tiers.sh` and `scripts/check-tiers.sh --wasm` must pass after every task.
- `#![deny(missing_docs)]`: every public item gets a doc comment saying why, at the density of `crates/rl-core/src/turn.rs`.
- No `HashMap` or `HashSet` in gameplay paths; `BTreeMap`, `Vec` or an `Interner`.
- No `TODO` comments in source; open work goes in `docs/TODO.md`.
- Plain dash, never an em dash, in code, comments and docs. American spelling in identifiers.
- Every RON schema carries a top-of-file comment listing the full option space.
- Randomness only from a registered stream; every effect rolls from `EffectRng`, whose derivation domain stays `b"ability"` so seeds replay.
- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass at the end of every task.
- Tests are named as sentences describing the property; a fingerprint is labelled "fingerprint tripwire".
- Commit only if the user asks; each task ends at a green, committable state.

**Two deliberate departures from the spec, to confirm with the user before Task 2:**

1. **Spec section 7 puts `land_triggers` in `ResolveSet::Effects`.** Fire and gas step in `ResolveSet::Fields`, which runs before `Effects`, so a grenade's fire or smoke would start a pass late while an ability's starts at once. This plan adds `ResolveSet::Triggers` between `Act` and `Fields` instead, which keeps every "same pass" the spec promises.
2. **Spec section 4 has `AbilitiesPlugin`, `PropsPlugin` and `ConsumablesPlugin` require `EffectsPlugin` via `depends_on`.** That adds a line to every tutorial step, the template and four games. This plan has each of them add `EffectsPlugin` when it is not already added (`if !app.is_plugin_added::<EffectsPlugin>()`), so no game changes.

## Review Focus

1. **A stack of consumables thrown one at a time:** throwing splits one unit off the stack; the thrown unit must be despawned where it lands, and the stack left in the bag must keep its count minus one, not lose two. Pinned in Task 6.
2. **A `destroyed` trigger whose burst kills another prop with its own `destroyed` trigger** (a chain of fuel drums): each must fire exactly once, a pass apart, never recursing in one pass. Pinned in Task 7.
3. **An item with both a `use` and a `land` trigger that is thrown**: only `land` fires, and a used one only `use`. Pinned in Task 4.
4. **Saving a run with a half-recharged wand and loading it:** `left` and `progress` return, and `max`, `when_empty` and `every` come from the definition, so an old save cannot hold a stale maximum. Pinned in Task 8.
5. **A monster wielding an empty `Kept` wand:** `ShootAtRange` must not pick a shot `Loadout` will not fire, or the mind stalls on a refused action every turn. Pinned in Task 6.

---

## File map

- `crates/rl-rules/src/ability.rs`: `EffectSpec` stays; `Area`, `TriggerSpec` added; `Cost::Charge` and `Purse::charges` removed.
- `crates/rl-rules/src/prop.rs`: `trigger: Option<TriggerDef>` becomes `triggers: Vec<TriggerSpec>`; `TriggerDef` and `TriggerOn` removed.
- `crates/rl-bevy/src/effects.rs`: the subsystem: moved machinery, `Source`, moments, `Triggers`, `Fired`, `land_triggers`, `EffectsPlugin`. It will pass 800 lines; split it into `effects/mod.rs` (plugin, `Effects`, the landing types), `effects/engine.rs` (the nine engine effects and `AddEngineEffects`) and `effects/triggers.rs` (moments, `Triggers`, `Fired`, `land_triggers`) in Task 3.
- `crates/rl-bevy/src/ability.rs`: loses the moved machinery, item sources, `Charges`, `redirect_item_uses`, `Cost::Charge` handling.
- `crates/rl-bevy/src/consumable.rs`: `Consumable` with charges, `WhenEmpty`, `Recharge`, spending moments, `spend_charges`, `recharge_charges`.
- `crates/rl-bevy/src/items.rs`: reports `use`; refuses a use of an empty thing.
- `crates/rl-bevy/src/throwing.rs`: reports `land`.
- `crates/rl-bevy/src/combat.rs`: reports `fire` and `hit`; `Loadout` skips an empty consumable's attack.
- `crates/rl-bevy/src/props.rs`: props arm `Triggers` on spawn and report `entered` and `destroyed`; `Triggered` and the `Fired(u32)` component removed.
- `crates/rl-bevy/src/plugin.rs`: `ResolveSet::Triggers`.
- `crates/rl-bevy/src/lib.rs`: re-exports.
- `crates/rl-save/src/engine.rs`: saves `Consumable` and trigger fires instead of `Charges`.
- `crates/rl-ui/src/view/inventory.rs`, `panel/inventory.rs`, `view/ability.rs`, `view/gear.rs`: triggers and charges on screen; lent abilities gone.
- `examples/foundry/**`, `examples/corsair/**`: migration.
- Docs listed in Task 12.

---

### Task 1: The authored form of a trigger

**Files:**
- Modify: `crates/rl-rules/src/ability.rs` (beside `EffectSpec`)
- Modify: `crates/rl-rules/src/lib.rs` (re-export)

**Interfaces:**
- Produces: `rl_rules::Area { Here, Burst { radius: i32 } }` (`Deserialize`, `Default` = `Here`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Debug`); `rl_rules::TriggerSpec { on: String, area: Area, fires: Option<u32>, effects: Option<Vec<EffectSpec>> }` (`Deserialize`, `Clone`, `Debug`).

- [ ] **Step 1: Write the failing test** in `crates/rl-rules/src/ability.rs`'s test module:

```rust
/// A trigger reads as its moment and nothing else when it takes the
/// thing's shared list at its own feet, and as much more as it needs.
#[test]
fn a_trigger_reads_bare_or_with_an_area_a_fire_count_and_a_list_of_its_own() {
    let bare: TriggerSpec = ron::from_str(r#"(on: "use")"#).unwrap();
    assert_eq!((bare.on.as_str(), bare.area, bare.fires), ("use", Area::Here, None));
    assert!(bare.effects.is_none(), "no list of its own: it takes the shared one");
    let full: TriggerSpec = ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(r#"(on: "land", area: Burst(radius: 2), fires: 3, effects: [(kind: "Harm", args: (kind: "fire", roll: "1d4"))])"#)
        .unwrap();
    assert_eq!((full.area, full.fires), (Area::Burst { radius: 2 }, Some(3)));
    assert_eq!(full.effects.as_ref().map(|e| e[0].kind.as_str()), Some("Harm"));
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p rl-rules --lib a_trigger_reads`
Expected: FAIL to compile, `cannot find type TriggerSpec`.

- [ ] **Step 3: Implement**, after `EffectSpec`:

```rust
/// Where a trigger's effects land, around the cell its moment happened on.
///
/// `Here` is the one cell, which is a stim in the arm or a plate underfoot.
/// `Burst` is the same wall-bounded footprint an ability's `Ball` covers,
/// so a grenade and a fireball of one radius reach the same cells. Shapes
/// that need a direction, a cone or a line, have none to take here, since
/// a moment happens at a cell rather than along an aim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub enum Area {
    /// The cell it happened on.
    #[default]
    Here,
    /// Every cell within `radius` of it that a projectile could reach.
    Burst {
        /// How far it reaches, in cells.
        radius: i32,
    },
}

/// A trigger as a content file writes it: `(on: "land", area: Burst(radius: 1))`.
///
/// Read by any game's file and by the engine's prop loader alike, so a
/// grenade and a trap are written the same way. `on` stays a word until
/// the Bevy layer resolves it against the moments registered for the run,
/// the way an offer's verb does. `effects` is optional because a thing
/// may say what it contains once and let each trigger deliver it: a trigger
/// with no list of its own lands its carrier's shared list.
#[derive(Debug, Clone, Deserialize)]
pub struct TriggerSpec {
    /// The moment that sets it off: `use`, `land`, `hit`, `entered`,
    /// `destroyed`, `fire`, or one a game registered.
    pub on: String,
    /// Where its effects land, around where the moment happened.
    #[serde(default)]
    pub area: Area,
    /// How many times it may go off; absent, every time.
    #[serde(default)]
    pub fires: Option<u32>,
    /// What it lands; absent, its carrier's shared list.
    #[serde(default)]
    pub effects: Option<Vec<EffectSpec>>,
}
```

Re-export both from `crates/rl-rules/src/lib.rs` beside `EffectSpec`, and name them in that file's module docs.

- [ ] **Step 4: Run the test**

Run: `cargo test -p rl-rules --lib a_trigger_reads`
Expected: PASS.

- [ ] **Step 5: Check tiers and lint**

Run: `scripts/check-tiers.sh && cargo clippy -p rl-rules --all-targets -- -D warnings`
Expected: clean.

---

### Task 2: Effects as a subsystem

Behaviour-neutral: every existing test and fingerprint passes unchanged at the end of this task. **Confirm the two departures in Global Constraints with the user before starting.**

**Files:**
- Modify: `crates/rl-bevy/src/effects.rs`, `crates/rl-bevy/src/ability.rs`, `crates/rl-bevy/src/props.rs`, `crates/rl-bevy/src/consumable.rs`, `crates/rl-bevy/src/lib.rs`, and every file `grep -rln "ability::\(EffectWorld\|Landing\|Effect\b\|FromArgs\|EffectKinds\|AddEffect\|AbilityRng\)" crates examples templates` lists.

**Interfaces:**
- Produces, in `rl_bevy::effects`: `EffectRng` (was `AbilityRng`), `Source { Ability(AbilityId), Trigger { on: Entity, moment: MomentId }, Offer(Entity) }`, `Landing { user, source, origin, aim, cells, path, landed_at, targets }` with `Landing::ability(&self) -> Option<AbilityId>`, `EffectWorld`, `Effect`, `FromArgs`, `EffectKinds`, `AddEffect`, `EffectsPlugin`. `Source::Trigger` is declared here with `MomentId` from Task 3; in this task declare `pub type MomentId = rl_core::Id<Moment>;` and `pub struct Moment;` in `effects.rs` so it compiles, and Task 3 gives them their registry.

- [ ] **Step 1: Write the failing tests** in `effects.rs`'s test module:

```rust
/// Fingerprint tripwire: the effect stream is the ability stream renamed,
/// derived from the same domain, so every seed rolls the dice it rolled
/// before the move. The number is what `AbilityRng` drew first for seed 7
/// on the day it moved.
#[test]
fn fingerprint_tripwire_the_effect_stream_draws_what_the_ability_stream_drew() {
    use rand::RngCore;
    use crate::seed::Stream;
    let mut rng = EffectRng::for_run(rl_core::RunSeed(7));
    assert_eq!(rng.next_u64(), FIRST_DRAW_FOR_SEED_SEVEN);
}

/// A game with props and no abilities lands a trap's effects: effects
/// belong to no one subsystem.
#[test]
fn a_game_with_effects_and_no_abilities_still_lands_a_trap() {
    let mut app = crate::plugin::headless_app();
    app.add_plugins((crate::combat::CombatPlugin, crate::props::PropsPlugin));
    app.add_engine_effects();
    // A prop registry with one plate that harms whoever steps on it,
    // loaded with `rl_rules::prop::load`, an actor beside it, a step onto
    // it through `Intent::new(actor, Move(dir))`, two updates, and a
    // `DamageDealt` for the actor.
    let dealt = crate::testing::step_onto_a_plate(&mut app);
    assert!(dealt > 0, "the plate harmed whoever stepped on it");
}
```

Before writing the first test, record the number: on the untouched tree, add a scratch test printing `AbilityRng::for_run(RunSeed(7)).next_u64()`, run it with `--nocapture`, delete the scratch test, and put the value in a `const FIRST_DRAW_FOR_SEED_SEVEN: u64` in the test module.

For the second test, add `pub fn step_onto_a_plate(app: &mut App) -> i32` to `crates/rl-bevy/src/testing.rs`. Build it from the prop test `spring_on_entered` already has in `props.rs`'s test module: same registry, same actor and step, returning the `dealt` summed from `Messages<DamageDealt>`.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p rl-bevy --lib effects::tests`
Expected: FAIL to compile, `cannot find type EffectRng`.

- [ ] **Step 3: Move the machinery.** Cut these from `ability.rs` and paste them into `effects.rs` above `pub struct Effects`, keeping every doc comment:
  - `AbilityRng` and its `Stream` impl, renamed `EffectRng`; the domain stays `SeedDomain::new(b"ability")`, with a line in its doc saying why;
  - `Landing`;
  - `EffectWorld` and its `impl`;
  - `Effect`, `FromArgs`, `Builder`, `EffectKinds`, `AddEffect`.

Replace `Landing::ability: Option<AbilityId>` with `pub source: Source` and add:

```rust
/// What landed an effect list.
///
/// So an effect can tell a spell from a trap without the landing type
/// assuming every list is an ability's, which is what `Option<AbilityId>`
/// used to say by leaving it empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// An ability resolved.
    Ability(AbilityId),
    /// A trigger went off on `on`, at `moment`.
    Trigger {
        /// What carried the trigger: a prop, a thing in a bag, a thrown
        /// thing where it came down, a weapon.
        on: Entity,
        /// What set it off.
        moment: MomentId,
    },
    /// A prop's offer was taken up.
    Offer(Entity),
}

impl Landing {
    /// The ability that landed this, when an ability did.
    pub fn ability(&self) -> Option<AbilityId> {
        match self.source {
            Source::Ability(id) => Some(id),
            _ => None,
        }
    }
}
```

In `ability.rs`, `Bystanders::land` builds `source: Source::Ability(ability)`; every reader of `landing.ability` calls `landing.ability()`. `Effects::land_on` gains a `source: Source` parameter, and its two callers in `props.rs` (the offer resolver and the two springers) pass `Source::Offer(prop)` and `Source::Trigger { on: prop, moment: Moments::ENTERED }` or `DESTROYED` once Task 3 exists. Until then, pass `Source::Offer(prop)` for both springers and fix it in Task 7. `consumable.rs`'s `land_uses` passes `Source::Offer(item)` until Task 4 deletes it.

- [ ] **Step 4: Add `EffectsPlugin`** at the end of `effects.rs`'s non-test code:

```rust
/// Effects: what every carrier of an effect list shares.
///
/// The registry of effect kinds, the stream every effect rolls from, and
/// the messages an effect writes. Added by whatever lands effects,
/// abilities, props or consumables, when a game has not already added it,
/// so a game names the subsystems it wants and this comes with them.
pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        use crate::plugin::Reads;
        use crate::seed::AddStream;
        app.init_resource::<EffectKinds>()
            .add_stream::<EffectRng>("EffectsPlugin")
            .add_message::<crate::status::Afflict>()
            .add_message::<crate::status::Cure>()
            .add_message::<crate::combat::DamageEvent>()
            .reads::<crate::cue::Cued>();
    }
}
```

In `AbilitiesPlugin::build`, `PropsPlugin::build` and `ConsumablesPlugin::build`, first line:

```rust
if !app.is_plugin_added::<crate::effects::EffectsPlugin>() {
    app.add_plugins(crate::effects::EffectsPlugin);
}
```

Then delete from each of those three what `EffectsPlugin` now does: `init_resource::<EffectKinds>()`, `add_stream::<AbilityRng>`, and the `Afflict`, `Cure` and `DamageEvent` registrations, along with the comments explaining them. `AddEngineEffects::add_engine_effects` gets the same `is_plugin_added` guard, so a test that calls it before adding a plugin still works.

- [ ] **Step 5: Re-point every import**

Run: `grep -rn "ability::\(EffectWorld\|Landing\|Effect\b\|FromArgs\|EffectKinds\|AddEffect\|AbilityRng\)\|AbilityRng" crates examples templates --include=*.rs`

Change each one to `crate::effects::` inside `rl-bevy` and to `rl_engine::rl_bevy::` elsewhere. In `crates/rl-bevy/src/lib.rs`, move the names out of the `ability` re-export lists into the `effects` lists, at line 50 and in the prelude at line 110; add `EffectsPlugin`, `EffectRng` and `Source`; drop `AbilityRng`.

- [ ] **Step 6: Run everything**

Run: `cargo test --workspace 2>&1 | grep -E "test result|FAILED|panicked"`
Expected: every result `ok`; both new tests pass; Foundry's `tests/fingerprint.rs` passes unchanged.

- [ ] **Step 7: Lint**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

---

### Task 3: Moments, `Triggers`, `Fired` and `land_triggers`

**Files:**
- Split: `crates/rl-bevy/src/effects.rs` into `effects/mod.rs`, `effects/engine.rs` and `effects/triggers.rs`, as the file map says; `mod.rs` re-exports what it moved.
- Modify: `crates/rl-bevy/src/plugin.rs` (`ResolveSet::Triggers`), `crates/rl-bevy/src/lib.rs`.

**Interfaces:**
- Consumes: `rl_rules::{Area, TriggerSpec, EffectSpec}` (Task 1); `Effects`, `EffectKinds`, `EffectWorld`, `Landing`, `Source` (Task 2).
- Produces:
  - `pub struct Moment;` `pub type MomentId = Id<Moment>;`
  - `#[derive(Resource, Debug, Clone)] pub struct Moments(Interner<Moment>)` with consts `USE = 0`, `LAND = 1`, `FIRE = 2`, `HIT = 3`, `ENTERED = 4`, `DESTROYED = 5`, `BUILT_IN: [&str; 6] = ["use", "land", "fire", "hit", "entered", "destroyed"]`, and `declare(&mut self, &str) -> MomentId`, `get(&self, &str) -> Option<MomentId>`, `name(&self, MomentId) -> &str`.
  - `pub trait AddMoment { fn add_moment(&mut self, name: &str) -> &mut Self; }` for `App`.
  - `#[derive(Clone)] pub struct Trigger { pub on: MomentId, pub area: Area, pub fires: Option<u32>, pub effects: Arc<Effects> }`
  - `#[derive(Component, Clone, Default)] pub struct Triggers(pub Vec<Trigger>)` with `pub fn build(specs: &[TriggerSpec], shared: &[EffectSpec], moments: &Moments, kinds: &EffectKinds, names: &Names<'_>) -> Result<Triggers, Vec<String>>` and `pub fn on(&self, moment: MomentId) -> impl Iterator<Item = &Trigger>`.
  - `#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)] pub struct Fired { pub on: Entity, pub moment: MomentId, pub by: Option<Entity>, pub at: Point }`
  - `pub fn land_triggers(...)` in `ResolveSet::Triggers`.
  - `ResolveSet::Triggers`, chained between `ResolveSet::Act` and `ResolveSet::Fields` in `plugin.rs:243`.

- [ ] **Step 1: Write the failing tests** in `effects/triggers.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{CombatPlugin, DamageDealt, Health};
    use crate::components::{Actor, Blocks, Position};
    use crate::plugin::headless_app;

    /// A world with combat and the engine's effects, an open floor, and a
    /// damage kind called `blunt`, as `crate::testing::surface` and
    /// `two_sides` build it for combat's own tests.
    fn floor() -> (App, Point, rl_rules::damage::DamageKindId) {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, crate::world::StreamingPlugin));
        app.add_engine_effects();
        let start = crate::testing::surface(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        (app, start, sides.kind)
    }

    fn harm(roll: &str) -> Vec<rl_rules::EffectSpec> {
        ron::from_str(&format!(r#"[(kind: "Harm", args: (kind: "blunt", roll: "{roll}"))]"#)).unwrap()
    }

    fn build(app: &App, specs: &str, shared: &[rl_rules::EffectSpec]) -> Result<Triggers, Vec<String>> {
        let specs: Vec<rl_rules::TriggerSpec> = ron::from_str(specs).unwrap();
        let world = app.world();
        let registries = world.resource::<Registries>();
        Triggers::build(&specs, shared, world.resource::<Moments>(), world.resource::<EffectKinds>(), &registries.names())
    }

    fn fire(app: &mut App, on: Entity, moment: MomentId, at: Point) {
        app.world_mut().write_message(Fired { on, moment, by: None, at });
        app.world_mut().run_schedule(crate::plugin::Turn);
    }

    fn health(app: &App, e: Entity) -> i32 {
        app.world().get::<Health>(e).unwrap().current
    }

    #[test]
    fn a_moment_nobody_registered_fails_the_build_naming_it() {
        let (app, _, _) = floor();
        let errs = build(&app, r#"[(on: "explode")]"#, &harm("1")).err().expect("an unknown moment refuses");
        assert!(errs.iter().any(|e| e.contains("\"explode\"")), "{errs:?}");
    }

    #[test]
    fn a_trigger_with_neither_a_list_nor_a_shared_one_fails_the_build() {
        let (app, _, _) = floor();
        assert!(build(&app, r#"[(on: "use")]"#, &[]).is_err(), "a trigger that does nothing is a typo");
    }

    #[test]
    fn here_lands_on_the_one_cell_and_nobody_beside_it() {
        let (mut app, at, _) = floor();
        let triggers = build(&app, r#"[(on: "use")]"#, &harm("3")).unwrap();
        let under = app.world_mut().spawn((Actor, Blocks, Position(at), Health::full(10))).id();
        let beside = app.world_mut().spawn((Actor, Blocks, Position(at.offset(1, 0)), Health::full(10))).id();
        let carrier = app.world_mut().spawn(triggers).id();
        fire(&mut app, carrier, Moments::USE, at);
        assert_eq!((health(&app, under), health(&app, beside)), (7, 10));
    }

    /// A burst reaches exactly what an ability's ball of the same radius
    /// reaches, because it is the same call.
    #[test]
    fn a_burst_covers_the_cells_an_abilitys_ball_of_that_radius_covers() {
        let (app, at, _) = floor();
        let map = app.world().resource::<WorldMap>();
        let occupancy = app.world().resource::<crate::turn::Occupancy>();
        let ball = rl_grid::footprint(rl_grid::TargetMode::Ball { range: 0, radius: 2 }, at, at, map.window_tiles(), |p| {
            p != at && (map.blocks_projectiles(p) || occupancy.is_occupied(p))
        });
        assert_eq!(area_cells(Area::Burst { radius: 2 }, at, map, occupancy), ball.cells);
    }

    #[test]
    fn a_trigger_fires_as_often_as_it_says_per_carrier_and_not_per_definition() {
        let (mut app, at, _) = floor();
        let triggers = build(&app, r#"[(on: "entered", fires: 1)]"#, &harm("1")).unwrap();
        let victim = app.world_mut().spawn((Actor, Blocks, Position(at), Health::full(10))).id();
        let one = app.world_mut().spawn(triggers.clone()).id();
        let two = app.world_mut().spawn(triggers).id();
        fire(&mut app, one, Moments::ENTERED, at);
        fire(&mut app, one, Moments::ENTERED, at);
        assert_eq!(health(&app, victim), 9, "the first carrier went off once");
        fire(&mut app, two, Moments::ENTERED, at);
        assert_eq!(health(&app, victim), 8, "and springing it spent nothing of the second");
    }

    #[test]
    fn a_moment_a_game_registered_lands_like_one_of_the_engines() {
        let (mut app, at, _) = floor();
        app.add_moment("overheat");
        let overheat = app.world().resource::<Moments>().get("overheat").unwrap();
        let triggers = build(&app, r#"[(on: "overheat")]"#, &harm("2")).unwrap();
        let wielder = app.world_mut().spawn((Actor, Blocks, Position(at), Health::full(10))).id();
        let gun = app.world_mut().spawn(triggers).id();
        fire(&mut app, gun, overheat, at);
        assert_eq!(health(&app, wielder), 8);
    }

    #[test]
    fn two_triggers_on_one_moment_land_in_the_order_they_were_written() {
        let (mut app, at, _) = floor();
        let triggers = build(&app, r#"[(on: "use", effects: [(kind: "Harm", args: (kind: "blunt", roll: "2"))]), (on: "use", effects: [(kind: "Harm", args: (kind: "blunt", roll: "5"))])]"#, &[]).unwrap();
        let victim = app.world_mut().spawn((Actor, Blocks, Position(at), Health::full(10))).id();
        let carrier = app.world_mut().spawn(triggers).id();
        app.init_resource::<DealtLog>().add_systems(bevy::app::PostUpdate, keep_dealt);
        fire(&mut app, carrier, Moments::USE, at);
        app.update();
        assert_eq!(app.world().resource::<DealtLog>().0, vec![2, 5]);
        let _ = victim;
    }

    #[derive(Resource, Default)]
    struct DealtLog(Vec<i32>);

    fn keep_dealt(mut dealt: MessageReader<DamageDealt>, mut log: ResMut<DealtLog>) {
        log.0.extend(dealt.read().map(|d| d.dealt));
    }
}
```

`run_schedule(Turn)` runs one pass whatever the state; if the headless app needs `EngineState::Playing` for `Turn` systems to run, set it and `app.update()` twice first, the way `combat.rs`'s `arena` tests do.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p rl-bevy --lib effects::triggers`
Expected: FAIL to compile, `cannot find type Moments`.

- [ ] **Step 3: Split the file** as the file map says, with a module doc on each part. `mod.rs` keeps `Effects`, `Landing`, `Source`, `EffectWorld`, `Effect`, `FromArgs`, `EffectKinds`, `AddEffect`, `EffectRng` and `EffectsPlugin`, and does `pub use engine::*; pub use triggers::*;`.

- [ ] **Step 4: Moments** in `triggers.rs`, modelled line for line on `Verbs` and `AddVerb` in `crates/rl-bevy/src/props.rs:80-146`:

```rust
/// What sets a trigger off: a thing used, a throw come to rest, an attack
/// made or landed, a cell stepped on, a prop broken, or one a game names.
pub struct Moment;

/// A registered moment.
pub type MomentId = Id<Moment>;

/// Every moment in play, in the order they were first named.
///
/// Interned rather than a closed enum so a game adds its own, an
/// overheating gun or a charged console, with one line, and reports it
/// from a system of its own. The engine's are interned first, so their ids
/// are the constants on this type.
#[derive(Resource, Debug, Clone)]
pub struct Moments(Interner<Moment>);

impl Moments {
    /// A thing in the bag was used.
    pub const USE: MomentId = MomentId::from_raw(0);
    /// A thrown thing came to rest.
    pub const LAND: MomentId = MomentId::from_raw(1);
    /// An attack was made with a worn thing.
    pub const FIRE: MomentId = MomentId::from_raw(2);
    /// An attack made with a worn thing struck someone.
    pub const HIT: MomentId = MomentId::from_raw(3);
    /// Somebody stepped onto a prop's cell.
    pub const ENTERED: MomentId = MomentId::from_raw(4);
    /// A prop was broken.
    pub const DESTROYED: MomentId = MomentId::from_raw(5);

    /// The engine's moments, in the order their ids are handed out.
    pub const BUILT_IN: [&'static str; 6] = ["use", "land", "fire", "hit", "entered", "destroyed"];

    /// The id for `name`, assigning a new one if it is unseen.
    pub fn declare(&mut self, name: &str) -> MomentId {
        self.0.intern(name)
    }

    /// The id for `name`, if it has been declared.
    pub fn get(&self, name: &str) -> Option<MomentId> {
        self.0.get(name)
    }

    /// The name behind `id`.
    pub fn name(&self, id: MomentId) -> &str {
        self.0.name(id)
    }
}

impl Default for Moments {
    fn default() -> Self {
        let mut names = Interner::new();
        for name in Self::BUILT_IN {
            names.intern(name);
        }
        Self(names)
    }
}

/// Declares a moment while the app is being built.
pub trait AddMoment {
    /// Declares the moment `name`. Look its id up again with [`Moments::get`].
    fn add_moment(&mut self, name: &str) -> &mut Self;
}

impl AddMoment for App {
    fn add_moment(&mut self, name: &str) -> &mut Self {
        self.init_resource::<Moments>();
        self.world_mut().resource_mut::<Moments>().declare(name);
        self
    }
}
```

`EffectsPlugin::build` gains `.init_resource::<Moments>().add_message::<Fired>()`.

- [ ] **Step 5: `Trigger`, `Triggers` and `Fired`** in `triggers.rs`:

```rust
/// One thing a carrier does at one moment.
///
/// The list behind an `Arc`, built once per definition and shared by every
/// copy, and the fire count beside it, which is each carrier's own.
#[derive(Clone)]
pub struct Trigger {
    /// What sets it off.
    pub on: MomentId,
    /// Where it lands, around where the moment happened.
    pub area: Area,
    /// Times it may still go off; `None` for every time.
    pub fires: Option<u32>,
    /// What it lands.
    pub effects: Arc<Effects>,
}

/// Everything a prop or a thing does at its moments.
///
/// One component for both, since a trap and a grenade differ in which
/// moments they answer and never in how the answer lands.
#[derive(Component, Clone, Default)]
pub struct Triggers(pub Vec<Trigger>);

impl Triggers {
    /// Builds a carrier's triggers from content, once per definition.
    ///
    /// A spec with no list of its own takes `shared`, which is how a thing
    /// says what it contains once and lets each trigger deliver it. Every
    /// problem is reported, not the first: an unknown moment by its name,
    /// an unregistered effect by `Effects::build`'s own message, and a
    /// trigger with neither a list nor a shared one as the typo it is.
    pub fn build(specs: &[TriggerSpec], shared: &[EffectSpec], moments: &Moments, kinds: &EffectKinds, names: &Names<'_>) -> Result<Self, Vec<String>> {
        let mut errors = Vec::new();
        let shared_built = if shared.is_empty() { None } else { Effects::build(shared, kinds, names).map(Arc::new).map_err(|e| errors.extend(e)).ok() };
        let mut out = Vec::new();
        for spec in specs {
            let Some(on) = moments.get(&spec.on) else {
                let known: Vec<&str> = Moments::BUILT_IN.to_vec();
                errors.push(format!("no moment is registered as {:?}; the engine's are {}", spec.on, known.join(", ")));
                continue;
            };
            let effects = match (&spec.effects, &shared_built) {
                (Some(own), _) => match Effects::build(own, kinds, names) {
                    Ok(e) => Arc::new(e),
                    Err(e) => {
                        errors.extend(e);
                        continue;
                    }
                },
                (None, Some(shared)) => shared.clone(),
                (None, None) => {
                    errors.push(format!("the trigger on {:?} has no effects of its own and nothing shared to deliver", spec.on));
                    continue;
                }
            };
            out.push(Trigger { on, area: spec.area, fires: spec.fires, effects });
        }
        if errors.is_empty() { Ok(Self(out)) } else { Err(errors) }
    }

    /// The triggers on `moment`, in the order they were written.
    pub fn on(&self, moment: MomentId) -> impl Iterator<Item = &Trigger> {
        self.0.iter().filter(move |t| t.on == moment)
    }
}

/// Something happened to a carrier at a moment.
///
/// The one thing a subsystem that owns a moment writes. What lands is
/// [`land_triggers`]' business, and what a use costs is the consumables'.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fired {
    /// The carrier: a prop, a thing used or thrown, a weapon.
    pub on: Entity,
    /// Which moment.
    pub moment: MomentId,
    /// Who set it off, when anyone did.
    pub by: Option<Entity>,
    /// Where it happened.
    pub at: Point,
}
```

- [ ] **Step 6: `area_cells` and `land_triggers`** in `triggers.rs`:

```rust
/// The cells `area` covers around `at`: the one cell, or the same footprint
/// an ability's ball of that radius covers, by the same call.
pub fn area_cells(area: Area, at: Point, map: &WorldMap, occupancy: &Occupancy) -> Vec<Point> {
    match area {
        Area::Here => vec![at],
        Area::Burst { radius } => {
            let stops = |p: Point| p != at && (map.blocks_projectiles(p) || occupancy.is_occupied(p));
            rl_grid::footprint(rl_grid::TargetMode::Ball { range: 0, radius }, at, at, map.window_tiles(), stops).cells
        }
    }
}

/// Lands every carrier's triggers for each moment reported this pass.
///
/// In [`ResolveSet::Triggers`](crate::plugin::ResolveSet::Triggers), after
/// every system that reports a moment in the same pass and before fire,
/// gas, statuses and damage, so whatever a trigger starts is resolved in
/// the pass that set it off. Every actor under the footprint is a target,
/// the one who set it off included: a grenade does not ask whose it was.
pub fn land_triggers(
    mut fired: MessageReader<Fired>,
    mut carriers: Query<&mut Triggers>,
    alive: Query<(), (With<crate::combat::Health>, Without<crate::combat::Dead>)>,
    occupancy: Res<Occupancy>,
    map: Res<WorldMap>,
    mut world: EffectWorld,
) {
    for f in fired.read() {
        let Ok(mut triggers) = carriers.get_mut(f.on) else { continue };
        for trigger in triggers.0.iter_mut().filter(|t| t.on == f.moment) {
            if trigger.fires == Some(0) {
                continue;
            }
            let cells = area_cells(trigger.area, f.at, &map, &occupancy);
            let targets: Vec<Entity> = cells.iter().flat_map(|c| occupancy.at(*c).iter().copied()).filter(|e| alive.contains(*e)).collect();
            let landing = Landing {
                user: f.by.unwrap_or(f.on),
                source: Source::Trigger { on: f.on, moment: f.moment },
                origin: f.at,
                aim: f.at,
                cells,
                path: Vec::new(),
                landed_at: Some(f.at),
                targets,
            };
            trigger.effects.land(&landing, &mut world);
            if let Some(left) = trigger.fires.as_mut() {
                *left -= 1;
            }
        }
    }
}
```

If `EffectWorld` already holds `Occupancy` or `WorldMap` mutably or immutably, Bevy refuses the system for a conflicting borrow; read them through `EffectWorld` instead. Add `pub fn map(&self) -> &WorldMap` and `pub fn occupancy(&self) -> &Occupancy` accessors to `EffectWorld` and drop those two parameters. `Effects::land_on` becomes a thin wrapper that builds the one-cell `Landing` the same way, and stays for offers.

- [ ] **Step 7: The set.** In `crates/rl-bevy/src/plugin.rs`, add `Triggers` to `ResolveSet` with a doc line ("Every carrier's triggers for the moments reported this pass, after the actions that reported them and before fire, gas, statuses and damage"), and put it in the chain at `plugin.rs:243`, between `ResolveSet::Act` and `ResolveSet::Fields`. `EffectsPlugin::build` adds `.add_systems(Turn, land_triggers.in_set(ResolveSet::Triggers))`.

- [ ] **Step 8: Run the tests**

Run: `cargo test -p rl-bevy --lib effects && cargo test --workspace 2>&1 | grep -E "test result|FAILED"`
Expected: the new tests PASS; the whole workspace still passes, since nothing reports a moment yet.

- [ ] **Step 9: Lint**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

---

### Task 4: `Consumable` with charges, and the `use` moment

**Files:**
- Rewrite: `crates/rl-bevy/src/consumable.rs`
- Modify: `crates/rl-bevy/src/items.rs` (report `use`; refuse an empty one), `crates/rl-bevy/src/lib.rs`

**Interfaces:**
- Consumes: `Fired`, `Moments`, `Triggers` (Task 3).
- Produces:
  - `#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)] pub struct Consumable { pub left: u16, pub max: u16, pub when_empty: WhenEmpty, pub recharge: Option<Recharge> }` with `pub fn new(max: u16, when_empty: WhenEmpty) -> Self` (full) and `pub fn recharging(self, every: u32) -> Self`, plus `pub fn is_empty(&self) -> bool`.
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)] pub enum WhenEmpty { Destroyed, Kept }`
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub struct Recharge { pub every: u32, pub progress: u32 }`
  - `#[derive(Resource, Debug, Clone)] pub struct SpendingMoments(Vec<MomentId>)`, defaulting to `[USE, LAND, FIRE]`, and `pub trait AddSpending { fn spends_on(&mut self, moment: MomentId) -> &mut Self; }`.
  - `pub fn spend_charges(...)` and `pub fn recharge_charges(...)`.
  - Removed: `OnUse`, `land_uses`, `Spending`.

- [ ] **Step 1: Write the failing tests** in `consumable.rs`'s test module, replacing the tests that used `OnUse`. Keep a helper `bag_with(app, (Consumable, Option<Stack>, Triggers)) -> (Entity user, Entity item)` that spawns a player on the surface with an `Inventory` holding the item, built from the setup the existing tests in this module use:

```rust
#[test]
fn a_use_lands_the_use_trigger_on_the_user_and_takes_one_charge() {
    let (mut app, user, item) = bag_with(Consumable::new(3, WhenEmpty::Kept), None, mend_on_use("4"));
    wound(&mut app, user, 10);
    use_it(&mut app, user, item);
    assert_eq!(health(&app, user), health_max(&app, user) - 10 + 4);
    assert_eq!(app.world().get::<Consumable>(item).map(|c| c.left), Some(2));
}

#[test]
fn the_last_charge_of_a_stack_takes_one_off_the_stack_and_the_next_unit_starts_full() {
    let (mut app, user, item) = bag_with(Consumable::new(1, WhenEmpty::Destroyed), Some(3), mend_on_use("1"));
    use_it(&mut app, user, item);
    assert_eq!(app.world().get::<Stack>(item).map(|s| s.count), Some(2));
    assert_eq!(app.world().get::<Consumable>(item).map(|c| c.left), Some(1), "the next unit starts full");
}

#[test]
fn the_last_of_a_destroyed_thing_is_gone_and_the_last_of_a_kept_one_stays_at_nothing() {
    let (mut app, user, grenade) = bag_with(Consumable::new(1, WhenEmpty::Destroyed), None, mend_on_use("1"));
    use_it(&mut app, user, grenade);
    assert!(app.world().get_entity(grenade).is_err(), "destroyed");
    let (mut app, user, wand) = bag_with(Consumable::new(1, WhenEmpty::Kept), None, mend_on_use("1"));
    use_it(&mut app, user, wand);
    assert_eq!(app.world().get::<Consumable>(wand).map(|c| c.left), Some(0), "kept, empty");
}

#[test]
fn using_an_empty_thing_is_refused_and_the_turn_is_kept() {
    let (mut app, user, wand) = bag_with(Consumable { left: 0, ..Consumable::new(3, WhenEmpty::Kept) }, None, mend_on_use("1"));
    let before = app.world().resource::<Turns>().now();
    use_it(&mut app, user, wand);
    assert_eq!(app.world().resource::<Turns>().now(), before, "refused, so no time passed");
    let refused = app.world_mut().resource_mut::<Messages<ActionRefused>>().drain().any(|r| r.actor == user);
    assert!(refused);
}

#[test]
fn a_recharge_returns_a_charge_every_period_on_the_clock_and_keeps_the_remainder() {
    let (mut app, user, wand) = bag_with(Consumable { left: 0, ..Consumable::new(3, WhenEmpty::Kept).recharging(250) }, None, mend_on_use("1"));
    wait_turns(&mut app, user, 3);
    let c = app.world().get::<Consumable>(wand).copied().unwrap();
    assert_eq!((c.left, c.recharge.map(|r| r.progress)), (1, Some(50)), "three turns of 100 is one charge and 50 over");
}

#[test]
fn a_thing_with_a_use_and_a_land_trigger_used_from_the_bag_answers_only_use() {
    let triggers = build_triggers(r#"[(on: "use", effects: [(kind: "Mend", args: (kind: "care", roll: "3"))]), (on: "land", effects: [(kind: "Harm", args: (kind: "blunt", roll: "9"))])]"#);
    let (mut app, user, item) = bag_with(Consumable::new(2, WhenEmpty::Kept), None, triggers);
    wound(&mut app, user, 5);
    use_it(&mut app, user, item);
    assert_eq!(health(&app, user), health_max(&app, user) - 5 + 3, "mended, and the land trigger did not go off");
}
```

`use_it` writes `Intent::new(user, UseItem(item))` and updates twice. `wait_turns` writes `Wait` and updates once per turn. `mend_on_use(roll)` and `build_triggers(ron)` call `Triggers::build` against the app's resources.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p rl-bevy --lib consumable`
Expected: FAIL to compile, `no function or associated item named new`.

- [ ] **Step 3: Rewrite `consumable.rs`.** Keep the module doc's reasoning, updated: the carriers are now abilities, props' offers and triggers; a used thing is a `use` trigger; `Consumable` counts charges. Then:

```rust
impl Consumable {
    /// A full one holding `max`.
    pub fn new(max: u16, when_empty: WhenEmpty) -> Self {
        Self { left: max, max, when_empty, recharge: None }
    }

    /// The same, regaining one charge every `every` hundredths of a step.
    pub fn recharging(mut self, every: u32) -> Self {
        self.recharge = Some(Recharge { every, progress: 0 });
        self
    }

    /// Whether there is nothing left to spend in the unit in hand.
    pub fn is_empty(&self) -> bool {
        self.left == 0
    }
}

/// Spends one charge from every consumable whose moment spends.
///
/// After [`land_triggers`](crate::effects::land_triggers), so what the
/// last charge did has landed before the thing is gone. A moment with no
/// triggers on the carrier still spends, which is how a plain wand's
/// ordinary shot costs a charge.
pub fn spend_charges(mut commands: Commands, mut fired: MessageReader<Fired>, spends: Res<SpendingMoments>, mut things: Query<(&mut Consumable, Option<&mut Stack>)>) {
    for f in fired.read() {
        if !spends.0.contains(&f.moment) {
            continue;
        }
        let Ok((mut c, stack)) = things.get_mut(f.on) else { continue };
        c.left = c.left.saturating_sub(1);
        if c.left > 0 {
            continue;
        }
        match stack {
            Some(mut s) if s.count > 1 => {
                s.count -= 1;
                c.left = c.max;
            }
            _ if c.when_empty == WhenEmpty::Destroyed => {
                commands.entity(f.on).despawn();
            }
            _ => {}
        }
    }
}

/// Counts every recharging consumable's progress up by the time that
/// passed this pass, and returns a charge for each full period.
pub fn recharge_charges(turns: Res<Turns>, mut last: Local<Option<u32>>, mut things: Query<&mut Consumable>) {
    let now = turns.now();
    let passed = now.saturating_sub(last.unwrap_or(now));
    *last = Some(now);
    if passed == 0 {
        return;
    }
    for mut c in &mut things {
        let (left, max) = (c.left, c.max);
        let Some(r) = c.recharge.as_mut() else { continue };
        if left >= max {
            r.progress = 0;
            continue;
        }
        r.progress += passed;
        let gained = (r.progress / r.every.max(1)) as u16;
        r.progress %= r.every.max(1);
        c.left = (left + gained).min(max);
        if c.left == max {
            if let Some(r) = c.recharge.as_mut() {
                r.progress = 0;
            }
        }
    }
}
```

`SpendingMoments` defaults to `vec![Moments::USE, Moments::LAND, Moments::FIRE]`. `AddSpending::spends_on` pushes a moment if it is absent. `ConsumablesPlugin::build`, after the `EffectsPlugin` guard:

```rust
app.init_resource::<SpendingMoments>()
    .add_systems(Turn, spend_charges.in_set(ResolveSet::Triggers).after(crate::effects::land_triggers))
    .add_systems(Turn, recharge_charges.in_set(TurnSet::React));
```

- [ ] **Step 4: Report `use`, and refuse an empty one** in `items.rs`. Give `ItemWorld` `consumables: Query<'w, 's, &'static Consumable>` in place of `lends`, and delete the `lends` skip at the top of `resolve_items`. In the `Which::Use(item)` arm (around `items.rs:395`), before the use is accepted:

```rust
if world.consumables.get(item).is_ok_and(|c| c.is_empty()) {
    // An empty wand is still a wand, and using one is a mistake the
    // player should hear about rather than pay a turn for.
    resolution.refuse(actor, rl_core::turn::BASE_ACTION_COST);
    continue;
}
```

`Resolution::refuse` is the existing method at `turn.rs:240`; use its real name if it differs. Next to the existing `events.write(ItemEvent::Used { actor, item })`, add `fired.write(Fired { on: item, moment: Moments::USE, by: Some(actor), at: pos })`, where `pos` is the actor's position, already read in that arm. Add `fired: MessageWriter<'w, Fired>` to `resolve_items`' parameters, and register `Fired` in `ItemsPlugin` with `.add_message::<Fired>()`.

- [ ] **Step 5: Re-export** `Consumable`, `WhenEmpty`, `Recharge`, `SpendingMoments`, `AddSpending` and `ConsumablesPlugin` from `lib.rs` and the prelude, and drop `OnUse`.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p rl-bevy --lib consumable items`
Expected: PASS. `rl-ui` and the games do not compile yet; Task 5 and Tasks 9 to 11 fix them.

---

### Task 5: Items stop lending abilities

**Files:**
- Modify: `crates/rl-bevy/src/ability.rs`, `crates/rl-rules/src/ability.rs`, `crates/rl-ui/src/view/ability.rs`, `crates/rl-ui/src/view/inventory.rs`, `crates/rl-ui/src/panel/inventory.rs`, `crates/rl-bevy/src/lib.rs`

**Interfaces:**
- Produces: `Known(BTreeSet<AbilityId>)` with `learn(AbilityId)`, `has`, `iter() -> impl Iterator<Item = AbilityId>`, `len`, `is_empty`, `clear`; `refresh_known(Query<(&mut Known, Option<&Grants>)>)`; `ItemRow` without `lends`; `Lent` removed.
- Removed: `Charges`, `redirect_item_uses`, `Known::source_of`, `Cost::Charge`, `Purse::charges`, `charges_of`, `spend_charges` in `ability.rs`, `UserState::charges`, and the `source` parameters of `gate` and `pay`.

- [ ] **Step 1: Write the failing test** in `crates/rl-bevy/src/ability.rs`'s tests:

```rust
/// What an actor knows is what it is, never what it carries: a thing in
/// the bag with `Grants` on it teaches nothing.
#[test]
fn an_actor_knows_its_own_grants_and_nothing_its_bag_carries() {
    let (mut app, user, bolt) = one_ability_user();
    let charm = app.world_mut().spawn((crate::items::Item, Grants(vec![bolt]))).id();
    app.world_mut().get_mut::<crate::items::Inventory>(user).unwrap().items.push(charm);
    app.world_mut().get_mut::<Grants>(user).unwrap().0.clear();
    app.update();
    assert!(!app.world().get::<Known>(user).unwrap().has(bolt), "carrying it taught nothing");
}
```

`one_ability_user` is the setup the existing tests in this module build from `Content`: a player with `Grants(vec![bolt])`, an `Inventory`, and `AbilitiesPlugin` with `ItemsPlugin`.

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p rl-bevy --lib an_actor_knows_its_own_grants`
Expected: FAIL, `carrying it taught nothing`.

- [ ] **Step 3: Remove item sources** in `ability.rs`:
  - `Known` becomes a `BTreeSet<AbilityId>`, and `learn` loses its `source` argument.
  - `refresh_known` reads only the actor's own `Grants`.
  - Delete `redirect_item_uses` and its `add_systems` line, `Charges`, `charges_of`, `spend_charges`, `UserState::charges`, and the `Cost::Charge` arm of `pay`.
  - `gate` and `pay` lose their `source: Option<Entity>` parameters, and every caller stops passing one.
  - Rewrite `Grants`' doc: it is what an actor knows of itself; items no longer carry it.

In `crates/rl-rules/src/ability.rs`, delete `Cost::Charge`, `Purse::charges`, the `CostRon::Charge` arm and every test line that sets `charges`. In `crates/rl-ui/src/view/ability.rs:159`, delete the `Cost::Charge` arm.

- [ ] **Step 4: The pack screen.**
  - In `view/inventory.rs`, delete `Lent` and `ItemRow::lends`, and replace the `Lends` query with `(Option<&'static Triggers>, Option<&'static Consumable>)`.
  - `used` becomes one line per trigger: `use: {line}` for a `use` trigger, `thrown: {line}` for a `land` trigger, with ` in a burst of {radius}` appended for `Area::Burst`, and `on a hit: {line}` for a `hit` trigger, where each `{line}` is from `Effects::describe`. Triggers on game moments are listed by their moment's name. The prefix moves from the panel into the view, so the panel prints `row.used` lines as they are.
  - `uses_something` becomes "has a `use` trigger".
  - Add `pub charges: Option<(u16, u16)>` to `ItemRow`, set to `Some((left, max))` when `max > 1`, and `pub empty: bool`.
  - `usable()` is `uses_something && !empty`.
- In `panel/inventory.rs`:
  - The use key writes `UseItem` when `row.usable()`; delete the `AimAt` branch, and `add_message::<AimAt>()` if nothing else uses it.
  - `hints` asks only `row.usable()`.
  - The detail prints `row.used` as the view wrote it, and `{left}/{max} charges` in the muted tone when `charges` is set.
  - Rewrite the two tests that used `Grants` or `OnUse`, `an_item_that_lends_an_ability_...` (line 573) and the poultice test (line 643), as a thing with a `use` trigger and one with a `land` trigger. Assert the lines `use: mends 2d4` and `thrown: 1d6 fire in a burst of 1`, and that an empty wand shows no use key.

- [ ] **Step 5: Run everything that compiles**

Run: `cargo test -p rl-rules -p rl-bevy -p rl-ui 2>&1 | grep -E "test result|FAILED|error"`
Expected: PASS. The save and the games are fixed in Tasks 8 to 11.

- [ ] **Step 6: Lint the three crates**

Run: `cargo clippy -p rl-rules -p rl-bevy -p rl-ui --all-targets -- -D warnings`
Expected: clean.

---

### Task 6: Throwing reports `land`, combat reports `fire` and `hit`

**Files:**
- Modify: `crates/rl-bevy/src/throwing.rs`, `crates/rl-bevy/src/combat.rs`

**Interfaces:**
- Consumes: `Fired`, `Moments`, `Consumable`.
- Produces: `Throwable { range, strike: Option<(DamageKindId, DiceRoll)> }` (unchanged; a strike-less throw is already allowed); `ShotLanding` gains `attacker: Entity`, `with: Option<Entity>`; `Loadout::melee_with` and `ranged_with` skip a worn item whose `Consumable` is empty.

- [ ] **Step 1: Write the failing tests.** In `throwing.rs`:

```rust
#[test]
fn a_thrown_consumable_lands_its_trigger_where_it_comes_down_and_is_gone() {
    let (mut app, thrower, grenade, at) = thrower_with(Consumable::new(1, WhenEmpty::Destroyed), burst_harm("4", 1), 3);
    let victim = stand_at(&mut app, at);
    throw_at(&mut app, thrower, grenade, at);
    assert!(health(&app, victim) < 10, "it burst on whoever stood there");
    assert!(app.world().get_entity(grenade).is_err(), "and is gone, not lying on the floor");
}

#[test]
fn throwing_one_of_a_stack_spends_that_one_and_leaves_the_rest_in_the_bag() {
    let (mut app, thrower, grenades, at) = thrower_with(Consumable::new(1, WhenEmpty::Destroyed), burst_harm("1", 1), 3);
    throw_at(&mut app, thrower, grenades, at);
    assert_eq!(app.world().get::<Stack>(grenades).map(|s| s.count), Some(2), "one thrown, two left, not one");
}

#[test]
fn a_thrown_blade_with_no_trigger_and_no_consumable_lands_and_waits_to_be_picked_up() {
    let (mut app, thrower, knife, at) = thrower_with_knife();
    throw_at(&mut app, thrower, knife, at);
    assert_eq!(app.world().get::<Position>(knife).map(|p| p.0), Some(at));
}
```

In `combat.rs`:

```rust
#[test]
fn a_wand_spends_a_charge_per_shot_even_when_the_shot_finds_nobody() {
    let (mut app, shooter, wand) = shooter_with_wand(Consumable::new(3, WhenEmpty::Kept));
    shoot_at_empty_cell(&mut app, shooter);
    assert_eq!(app.world().get::<Consumable>(wand).map(|c| c.left), Some(2));
}

#[test]
fn a_hit_trigger_lands_on_the_struck_actor_where_it_stands() {
    let (mut app, shooter, gun, target) = shooter_facing(hit_harm("3"));
    let before = health(&app, target);
    shoot(&mut app, shooter, target);
    // The shot's own roll plus the trigger's three.
    assert!(before - health(&app, target) >= 3);
    let _ = gun;
}

#[test]
fn an_empty_kept_wand_offers_no_shot_so_a_mind_never_chooses_one() {
    let (mut app, shooter, wand) = shooter_with_wand(Consumable { left: 0, ..Consumable::new(3, WhenEmpty::Kept) });
    let mut state: bevy::ecs::system::SystemState<Loadout> = bevy::ecs::system::SystemState::new(app.world_mut());
    let loadout = state.get(app.world()).unwrap();
    assert!(loadout.ranged(shooter).is_none(), "nothing to fire, so ShootAtRange has nothing to pick");
    let _ = wand;
}
```

Build the helpers from each module's existing test setup: `throwing.rs`'s tests throw a knife, and `combat.rs`'s `arena` and `struck_by_two_guns` shoot. A wand is `(Item, RangedAttack::new(kind, DiceRoll::flat(0), 6), Consumable, Wearable)`, equipped in the main hand.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p rl-bevy --lib throwing combat::tests::a_wand combat::tests::a_hit combat::tests::an_empty`
Expected: FAIL.

- [ ] **Step 3: `land`.** In `throwing.rs::land`, after `events.write(ItemEvent::Thrown { .. })`, add `fired.write(Fired { on: item, moment: Moments::LAND, by: Some(actor), at: rests })`, passing `fired: &mut MessageWriter<Fired>` from both callers. The item is still inserted at `rests` first, so a thing that survives landing lies there; `spend_charges` despawns a consumable one afterwards.

- [ ] **Step 4: `fire` and `hit`.** In `combat.rs::resolve_attacks`, next to `struck.write(Struck { .. })`:

```rust
if let Some(item) = from {
    fired.write(Fired { on: item, moment: Moments::FIRE, by: Some(actor), at: pos.0 });
}
```

Give `ShotLanding` `attacker: Entity` and `with: Option<Entity>`, set them where it is built, and pass `fired` and a `positions: &Query<&Position>` into `land`, which writes `Fired { on: with, moment: Moments::HIT, by: Some(attacker), at: target's position now }` when `with` is `Some` and the target still stands. `land_shots` gains the same two parameters. Add `fired: MessageWriter<Fired>` to `Arena` or to `resolve_attacks`, whichever keeps the parameter count under clippy's limit.

- [ ] **Step 5: `Loadout` skips an empty consumable's attack.** Add `spent: Query<'w, 's, &'static Consumable>` to `Loadout`. In `melee_with` and `ranged_with`, skip a worn item where `self.spent.get(item).is_ok_and(|c| c.is_empty())`. Armor and resistances from the same item still count.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p rl-bevy --lib`
Expected: PASS.

---

### Task 7: Props onto `Triggers`

**Files:**
- Modify: `crates/rl-rules/src/prop.rs`, `crates/rl-bevy/src/props.rs`, `examples/foundry/assets/props.ron`, and every other `props.ron` that `grep -rln "trigger:" examples templates` lists.

**Interfaces:**
- Consumes: `TriggerSpec`, `Triggers::build`, `Fired`, `Moments`.
- Produces: `PropDef::triggers: Vec<TriggerSpec>`; props carry `Triggers` from the frame they are armed; `report_entered` and `report_destroyed` in place of `spring_on_entered` and `spring_on_destroyed`.
- Removed: `TriggerDef`, `TriggerOn`, `Triggered`, and the `Fired(u32)` component in `props.rs:520`, whose name the message now takes.

- [ ] **Step 1: Write the failing tests** in `props.rs`:

```rust
/// A plate harms in the pass the step was taken, not a pass later.
#[test]
fn a_plate_harms_whoever_steps_on_it_in_the_pass_they_stepped() {
    let (mut app, walker) = beside_a_plate(r#"triggers: [(on: "entered", fires: 3, effects: [(kind: "Harm", args: (kind: "blunt", roll: "2"))])]"#);
    let before = health(&app, walker);
    step_onto_the_plate_and_run_one_pass(&mut app, walker);
    assert_eq!(health(&app, walker), before - 2);
}

#[test]
fn a_drum_that_bursts_sets_alight_every_cell_in_its_radius_the_pass_after_it_breaks() {
    let (mut app, drum, at) = a_drum(r#"triggers: [(on: "destroyed", area: Burst(radius: 1), effects: [(kind: "Ignite", args: (turns: 3))])]"#);
    break_it(&mut app, drum);
    run_one_pass(&mut app);
    let fire = app.world().resource::<crate::fire::Fire>();
    for dx in -1..=1 {
        for dy in -1..=1 {
            assert!(fire.is_burning(at.offset(dx, dy)), "{dx},{dy}");
        }
    }
}

/// A drum that bursts into another drum sets it off once, a pass later,
/// and never inside its own pass.
#[test]
fn a_chain_of_drums_goes_off_one_pass_at_a_time_and_each_once() {
    let (mut app, first, second) = two_drums_side_by_side(r#"triggers: [(on: "destroyed", area: Burst(radius: 1), effects: [(kind: "Harm", args: (kind: "blunt", roll: "99"))])]"#);
    let mut reports = record_fired(&mut app);
    break_it(&mut app, first);
    run_one_pass(&mut app);
    run_one_pass(&mut app);
    run_one_pass(&mut app);
    let destroyed: Vec<Entity> = reports.drain(&mut app).into_iter().filter(|f| f.moment == Moments::DESTROYED).map(|f| f.on).collect();
    assert_eq!(destroyed, vec![first, second], "each once, in order");
}
```

The helpers build from the existing prop tests in this module: a prop registry from `rl_rules::prop::load`, a `FirePlugin` for the drum test, and `record_fired` copying `Fired` out in `PostUpdate` the way `crate::testing` copies `Struck`.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p rl-bevy --lib props`
Expected: FAIL to compile on `triggers:`.

- [ ] **Step 3: The schema.** In `crates/rl-rules/src/prop.rs`:
  - `Authored` gets `#[serde(default)] triggers: Vec<TriggerSpec>` in place of `trigger`.
  - `PropDef` gets `pub triggers: Vec<TriggerSpec>`.
  - `load` checks each `fires` is not `Some(0)` with the existing message, and reads `effects` through `read_effects` when present.
  - Delete `TriggerDef` and `TriggerOn`.
  - Update the module doc: a moment stays a word until the Bevy layer interns it, as a verb does.

- [ ] **Step 4: Arming and reporting** in `crates/rl-bevy/src/props.rs`:
  - `PropEffects` keeps `offers`, and replaces `triggers: Vec<Effects>` with `triggers: Vec<Triggers>`, one per prop id, built in `build_prop_effects` with `Triggers::build(&def.triggers, &[], &moments, kinds, &names)`. Errors are logged with the prop's name as today.
  - New `arm_props(mut commands, built: Option<Res<PropEffects>>, fresh: Query<(Entity, &PropKind), (With<Prop>, Without<Triggers>)>)` inserts `built.triggers[kind.0.index()].clone()` on each prop whose list is non-empty. It is chained after `build_prop_effects` in `PropSet::Stock`.
  - `report_entered(mut steps: MessageReader<Stepped>, props: Query<(Entity, &Position), (With<Prop>, With<Triggers>)>, mut fired: MessageWriter<Fired>)` writes `Fired { on: prop, moment: Moments::ENTERED, by: Some(step.actor), at: step.to }` for each prop on `step.to`, in `ResolveSet::Act`.
  - `report_destroyed(mut deaths: MessageReader<DeathEvent>, props: Query<(), (With<Prop>, With<Triggers>)>, mut fired: MessageWriter<Fired>)` writes `Fired { on: death.entity, moment: Moments::DESTROYED, by: death.credit, at: death.at }`, in `TurnSet::React`.
  - A dead prop is despawned at the end of the frame, after `land_triggers` has read it the next pass. If `process_deaths` despawns a prop before then, the burst finds no carrier. In that case `report_destroyed` writes the `Fired` onto a fresh entity carrying a clone of the dead prop's `Triggers`, and `land_triggers` despawns such a carrier after landing it. Mark it with a `Remnant` component whose doc says why.
  - Offers land through `Effects::land_on(.., Source::Offer(prop))`.
  - Delete `Triggered`, the `Fired(u32)` component, `spring_on_entered`, `spring_on_destroyed` and the `Trap` alias. Register neither `Triggered` nor `Fired` here; `EffectsPlugin` registers `Fired`.

- [ ] **Step 5: Content.** In every `props.ron` the grep lists, replace `trigger: (on: Entered, fires: N, effects: [...])` with `triggers: [(on: "entered", fires: N, effects: [...])]`, and `Destroyed` with `"destroyed"`. Rewrite each file's header comment for `triggers`: `on` is `"entered" | "destroyed"` or a game's own moment, `area` is `Here | Burst(radius:)`, `fires` is optional, and `effects` is as `abilities.ron` writes them.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p rl-rules -p rl-bevy`
Expected: PASS.

---

### Task 8: Saving charges and fire counts

**Files:**
- Modify: `crates/rl-save/src/engine.rs`, `examples/corsair/src/save.rs:40`

**Interfaces:**
- Produces: `AbilityState { pools, cooldowns }` without `charges`; `EffectState { consumable: Option<(u16, u32)>, fires: Vec<Option<u32>> }` saved per entity beside it; `EngineSave` restores onto the components the game respawned.

- [ ] **Step 1: Write the failing test**, replacing the `Charges` round trip at `engine.rs:270-297`:

```rust
/// A half-recharged wand and a cable with one fire left come back as they
/// were; the wand's maximum and period come from what the game respawned,
/// never from the save.
#[test]
fn a_wand_mid_recharge_and_a_cable_with_one_fire_left_come_back_as_they_were() {
    let (mut app, wand, cable) = saved_world_with(
        Consumable { left: 2, max: 5, when_empty: WhenEmpty::Kept, recharge: Some(Recharge { every: 400, progress: 130 }) },
        vec![Some(1), None],
    );
    let (mut loaded, wand2, cable2) = round_trip(&mut app, wand, cable, |respawned| {
        respawned.insert(Consumable::new(5, WhenEmpty::Kept).recharging(400));
    });
    let c = loaded.world().get::<Consumable>(wand2).copied().unwrap();
    assert_eq!((c.left, c.max, c.recharge.map(|r| r.progress)), (2, 5, Some(130)));
    let fires: Vec<Option<u32>> = loaded.world().get::<Triggers>(cable2).unwrap().0.iter().map(|t| t.fires).collect();
    assert_eq!(fires, vec![Some(1), None]);
    let _ = &mut loaded;
}
```

Build `saved_world_with` and `round_trip` from the existing wand test's capture-and-restore setup in this module. The cable's `Triggers` is two triggers built with `Triggers::build`, the first with `fires: 3` then set to `Some(1)`.

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p rl-save --lib a_wand_mid_recharge`
Expected: FAIL to compile.

- [ ] **Step 3: Implement.** Remove `charges` from `AbilityState` and its `of` and `restore`. Add `EffectState`:
  - `of` reads `Consumable` as `(left, progress)` and every trigger's `fires` in list order.
  - `restore` writes them onto the entity's existing `Consumable` and `Triggers`, and ignores a fire count past the respawned list's length.

  Save `EffectState` wherever `AbilityState` is saved per entity. Its doc says why only these two are saved: the lists and the maximums are content, and a save that held them would outlive a change to the file.

- [ ] **Step 4: Bump Corsair's save version** at `examples/corsair/src/save.rs:40` from `3` to `4`, with a line saying the engine save's per-entity state changed shape.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p rl-save`
Expected: PASS.

---

### Task 9: A wand's charges on the gear panel

**Files:**
- Modify: `crates/rl-ui/src/view/gear.rs`, `crates/rl-ui/src/panel/gear.rs`

**Interfaces:**
- Produces: `GearRow::charges: Option<(u16, u16)>`, drawn as a right-aligned `left/max` in the muted tone ahead of any facets.

- [ ] **Step 1: Write the failing test** in `view/gear.rs`:

```rust
#[test]
fn a_worn_wand_reads_its_charges_on_its_row_and_a_blade_reads_none() {
    let mut stage = worn(&[("wand", Some(Consumable { left: 3, ..Consumable::new(5, WhenEmpty::Kept) })), ("blade", None)]);
    stage.tick();
    let view = stage.app.world().resource::<GearView>();
    let charges: Vec<Option<(u16, u16)>> = view.rows.iter().filter(|r| r.item.is_some()).map(|r| r.charges).collect();
    assert_eq!(charges, vec![Some((3, 5)), None]);
}
```

Build `worn` from the existing gear view tests' setup.

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p rl-ui --lib a_worn_wand`
Expected: FAIL to compile.

- [ ] **Step 3: Implement.** The collector reads `Option<&Consumable>` off each worn item and sets `charges` when `max > 1`. The presenter draws it. Add one assertion to an existing gear panel test that the text `3/5` appears on the row.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rl-ui`
Expected: PASS.

---

### Task 10: Foundry

**Files:**
- Modify: `examples/foundry/assets/items.ron`, `assets/abilities.ron`, `assets/props.ron` (done in Task 7), `src/gear.rs`, `src/input.rs`, `src/testing/mod.rs`, `src/plugin/ambiguity.rs`, `src/upgrades.rs` (only if it names `Charges`), `tests/fingerprint.rs`, `DESIGN.md`.

**Interfaces:**
- Consumes: `Triggers::build`, `Consumable`, `WhenEmpty`, `Moments`, `Fired`.
- Produces: `ItemDef` fields `triggers: Vec<TriggerSpec>`, `effects: Vec<EffectSpec>`, `consumable: Option<ConsumableDef { charges: u16, when_empty: WhenEmpty, recharge: Option<u32> }>`, `throw: Option<ThrowDef { range: i32, strike: Option<(DiceRoll, NameRef<DamageKind>)> }>`; removed: `grants`, `on_use`, `uses`.

- [ ] **Step 1: Rewrite the grenade tests** in `src/gear.rs` as real throws. Replace `throw_grenade` with:

```rust
/// Throws one of the grenades `player` carries at `aim`, through the
/// engine's own throw action, the way the pack's throw key does.
fn throw_grenade(app: &mut App, player: Entity, grenade: Entity, aim: Point) {
    app.world_mut().write_message(Intent::new(player, Throw { item: grenade, at: aim }));
    crate::testing::settle(app);
}
```

Each of the four grenade tests passes the carried stack instead of the ability name. The frag test asserts the stack went from 2 to 1 and that no grenade lies at `aim`. Add:

```rust
/// A stim is used, never thrown: it has a use trigger and no land trigger,
/// so the pack offers the one and the item file offers no throw.
#[test]
fn a_stim_has_a_use_trigger_and_no_throw() {
    let r = crate::content::registries();
    let armory = crate::testing::armory(&r);
    let stim = armory.defs.get(armory.defs.expect("stim"));
    assert!(stim.throw.is_none());
    assert_eq!(stim.triggers.iter().map(|t| t.on.as_str()).collect::<Vec<_>>(), vec!["use"]);
}
```

Use the engine's real throw action; its name is in `crates/rl-bevy/src/throwing.rs` above `resolve_throws`.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p foundry --lib gear`
Expected: FAIL to compile.

- [ ] **Step 3: `items.rs`'s content.** Replace:
  - the medical pair's `on_use`/`uses` with `consumable: (charges: 1, when_empty: Destroyed), triggers: [(on: "use", effects: [...])]`, keeping each list as it is;
  - each grenade's `grants` with `throw: (range: 6), consumable: (charges: 1, when_empty: Destroyed), triggers: [(on: "land", area: Burst(radius: 1), effects: [...])]`, with radius 2 for the ion grenade and the effects moved over from its `abilities.ron` entry;
  - the monoblade's `throw: (4, "1d6", "kinetic")` with `throw: (range: 4, strike: ("1d6", "kinetic"))`.

  Rewrite the header's option list for `triggers`, `effects`, `consumable`, `throw`, and delete `grants`, `on_use` and `uses` from it.

- [ ] **Step 4: `abilities.ron`** loses the four grenade entries and the comment above them, and its header drops the grenade sentence.

- [ ] **Step 5: `gear.rs`.**
  - `ItemDef` takes the new fields.
  - `validate_def` drops the `uses` and `grants` rules, and adds: a `consumable` with `charges: 0` is an error, and a `land` trigger on a thing with no `throw` is an error ("a land trigger on a thing that cannot be thrown never lands").
  - `Armory` holds `triggers: Vec<Triggers>`, built once per definition with `Triggers::build(&d.triggers, &d.effects, moments, kinds, &names)`. `Armory::load` and `Content` take `Moments` beside `EffectKinds`.
  - `spawn_item` inserts `Triggers` when non-empty; `Consumable::new(c.charges, c.when_empty)`, plus `.recharging(every)`, when set; and `Throwable { range, strike }`.
  - In `src/testing/mod.rs`, `effect_kinds()` stays, `armory()` passes `&Moments::default()`, and the doc comments say why.

- [ ] **Step 6: The `t` key** in `src/input.rs`: `t` opens the pack (`rl_engine::rl_ui::INVENTORY_MODAL`), where the pack's own `t` throws the row picked. Rename the control to "throw something from the pack". Rewrite `t_with_a_monoblade_in_the_pack_asks_the_cursor_to_throw_it` as `t_opens_the_pack_and_t_on_a_row_throws_it`: press `t`, assert the inventory modal is on top, press `t` again, and assert an `AimThrow` for the first row's item.

- [ ] **Step 7: The ambiguity test.** In `src/plugin/ambiguity.rs`, `ids()` replaces `Messages<Triggered>` with `Messages<Fired>`, and every allowed entry naming `spring_on_entered` or `spring_on_destroyed` names `report_entered` or `report_destroyed`. Run the test and name any new unordered pair in `names()`; order it rather than allowing it, as the file's doc asks.

- [ ] **Step 8: Run the tests**

Run: `cargo test -p foundry`
Expected: every test passes except, possibly, the fingerprint tripwire. The live cables now harm a pass sooner (spec section 7). If the tripwire moves, update the number in `tests/fingerprint.rs` and note the old and new values for the CHANGELOG line in Task 12.

- [ ] **Step 9: Play it.** Build, then capture a thrown frag grenade, the pack, and the gear panel with Foundry's capture tool, as the earlier grenade work did:

```bash
cargo build -p foundry
FOUNDRY_START=2 RL_CAPTURE=/tmp/throw.png RL_CAPTURE_KEYS="enter t" ./target/debug/foundry --seed 7
```

Read the images: the pack shows `thrown: 3d6 kinetic in a burst of 1` and a `t throw` hint. Fix anything that reads wrong before moving on.

---

### Task 11: Corsair, and every other game

**Files:**
- Modify: `examples/corsair/assets/items.ron`, `examples/corsair/src/items.rs`, and whatever `cargo build --workspace --all-targets` still reports in `examples/` and `templates/`.

- [ ] **Step 1: Run Corsair's rum test to see it fail**

Run: `cargo test -p corsair`
Expected: FAIL to compile on `OnUse`.

- [ ] **Step 2: Migrate.**
  - The rum's `on_use: [...]` becomes `consumable: (charges: 1, when_empty: Destroyed), triggers: [(on: "use", effects: [...])]`, with the same four effects.
  - The throwing knife's `thrown: (6, "1d4+1", "cutlass")` becomes `throw: (range: 6, strike: ("1d4+1", "cutlass"))`.
  - `items.rs` builds `Triggers` and `Consumable` the way Foundry's `gear.rs` does in Task 10, and drops `OnUse`.
  - The header comment lists the new fields.

- [ ] **Step 3: Every other game.** Fix what the build reports, which is renamed imports only: Delve, Heist, the tutorial steps and the template carry no item effects.

- [ ] **Step 4: Run everything**

Run: `cargo test --workspace 2>&1 | grep -E "test result|FAILED|panicked"`
Expected: all `ok`.

---

### Task 12: Documentation, and the whole check

**Files:**
- Create: `docs/design/effects.md`, `docs/guide/src/systems/effects.md`
- Modify: `docs/design/items.md`, `docs/guide/src/systems/{items,props,abilities}.md`, `docs/guide/src/SUMMARY.md`, `docs/OVERVIEW.md`, `README.md`, `CHANGELOG.md`, `docs/TODO.md`, `docs/PLAN.md`, `CLAUDE.md`, `docs/README.md`, `examples/foundry/DESIGN.md`

- [ ] **Step 1: `docs/design/effects.md`**, written from the spec, sections 1 to 8. Cover what the subsystem owns, moments as a registry and why not an enum, `Triggers` and the potion model, `land_triggers` and `ResolveSet::Triggers`, with both departures and their reasons, `Consumable`, and approaches A and C as the roads not taken. One sentence per line.

- [ ] **Step 2: `docs/design/items.md`**, rewritten:
  - the carriers table becomes abilities, offers and triggers;
  - section 1's dividing line becomes "an item never lends an ability";
  - the potion model;
  - `Consumable` with charges;
  - `on_equip` still deferred, with the standing-state reason kept.

  Delete everything about `Grants` on items and `Charge(1)`.

- [ ] **Step 3: The guide.**
  - `systems/effects.md` follows the six fixed parts in `CLAUDE.md`'s "Writing a system page". Its manifest names `EffectsPlugin` and lists `crates/rl-bevy/src/effects/mod.rs`, `effects/triggers.rs`, `effects/engine.rs`, `crates/rl-rules/src/ability.rs` and `crates/rl-bevy/src/consumable.rs`. Every code block is an `expand-guide.py` include of a real anchor: add `// ANCHOR: triggers` around Foundry's grenade loading in `gear.rs` and quote it.
  - Update `items.md`, `props.md` and `abilities.md` wherever they mention `OnUse`, `Grants` on items, `Charges`, `Triggered` or `trigger:`.
  - Add the page to `SUMMARY.md`.
  - Run `scripts/check-systems-style.sh` on each changed page, then `python3 scripts/check-systems.py --bless` for each after re-reading it.

- [ ] **Step 4: Inventory and history.**
  - `OVERVIEW.md`: an `EffectsPlugin` row in the `rl-bevy` plugin table, and the effects, items, consumables and props lines rewritten.
  - `README.md`: the "Items that do things" bullet.
  - `CLAUDE.md` layout: `effects` in the design docs list.
  - `docs/README.md`: the new design doc.
  - `docs/TODO.md`: three items, a tactic for a mind to use a thing from its bag, a shape for a shot, and `on_equip`.
  - `docs/PLAN.md`: a progress-log entry.
  - `examples/foundry/DESIGN.md`: the grenade paragraph, "a grenade is a throwable item with a land trigger", and `t`.

- [ ] **Step 5: `CHANGELOG.md`, under `Unreleased`.** One entry per breaking change, each with its migration:
  - `Triggered` becomes `Fired`, filtered by moment;
  - `OnUse` becomes a `use` trigger;
  - `Charges` becomes `Consumable`;
  - `Cost::Charge` and item `Grants` are removed;
  - `AbilityRng` becomes `EffectRng`, same seeds;
  - `trigger:` becomes `triggers:` in `props.ron`;
  - the trap and destroyed timings of spec section 7;
  - Corsair's save version 4;
  - the fingerprint re-baseline from Task 10, with both numbers, if it moved.

- [ ] **Step 6: Every check**

Run:
```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings \
  && cargo test --workspace 2>&1 | grep -E "FAILED|panicked"; \
  scripts/check-tiers.sh && scripts/check-tiers.sh --wasm && scripts/check-overview.sh \
  && scripts/check-guide.sh && python3 scripts/check-systems.py
```
Expected: no `FAILED` or `panicked` lines, and every script prints its ok line.
