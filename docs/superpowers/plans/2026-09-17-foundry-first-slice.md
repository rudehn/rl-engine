# Foundry, the first slice - implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A playable first slice of Foundry: decks 1 to 3 of a droid foundry, six weapons with heat and ammunition, six armor pieces, droids that shoot, loot on the floor and from the dead, and a first reactor charge with a pick-one-of-three upgrade that ends the run in victory.

**Architecture:** Two small engine changes land first, because the game cannot be built without them: a `Struck` message naming the weapon a blow came from, and a `ShootAtRange` tactic so a mind can fire a ranged weapon. Then `examples/foundry` is built module by module in Delve's and Corsair's shape: content registries, three deck chains, a gear loader, heat, ammunition, the droid roster, loot, the mission, and the play surface. Every mechanic that is really about the rules is a plain struct tested without an `App`; the Bevy systems around it are thin.

**Tech Stack:** Rust 2024, Bevy 0.17 through `rl-engine`, RON content files compiled in with `include_str!`, `rand` through the engine's seeded streams.

**Spec:** `docs/superpowers/specs/2026-09-17-foundry-design.md`. Section 13 is the slice this plan builds; sections 10.4 and 10.5 are the two engine changes it lands first.

## Global Constraints

These apply to every task, and every task's requirements include them.

From `CLAUDE.md`, enforced by the build:

- Every workspace member declares `tier` under `[package.metadata.rl-engine]`. `examples/foundry` is tier 3. No crate depends on a higher tier; tier 0 and 1 crates never depend on Bevy.
- `#![deny(missing_docs)]` on every crate, and doc-tests compile and run; never fence an example as `ignore`.
- `docs/OVERVIEW.md` changes in the same commit as a system the engine gains.
- `cargo fmt --all --check` passes; `rustfmt.toml` pins the width, so never hand-wrap.
- `cargo clippy --workspace --all-targets -- -D warnings` passes, with no crate-wide allow.
- `scripts/check-tiers.sh` and `scripts/check-tiers.sh --wasm` pass. `scripts/check-guide.sh` passes: run it whenever a change touches a type the tutorial constructs.

From `CLAUDE.md`, enforced by review:

- The engine owns the loop: behaviour belongs in the engine, and a game never copies engine logic.
- No theme words in engine crates: no fantasy, sci-fi or pirate vocabulary in their types, docs, constants or comments.
- Randomness: a game's own draws come from `Seed::stream(domain, index)`; functions take `&mut impl Rng`; never `ResMut<CombatRng>` or `ResMut<AbilityRng>` in game code, and never a generator from a constant or entropy.
- No `HashMap` or `HashSet` in gameplay or generation paths.
- A panel is a view, a collector and a presenter; something the engine cannot know is a `Facet` pushed in `ViewSet::Annotate`.
- A game's rules that react to a turn go in `TurnSet::React`. Never order a system after another crate's system function.
- Costs and clocks are integers in hundredths of a step; `BASE_ACTION_COST` is 100.
- Doc comments say why and why-not, at the density of `crates/rl-core/src/turn.rs`.
- Tests are properties over a seed range where a property exists, fingerprint tripwires labelled as such where none does, named as sentences.
- Every RON file carries a top-of-file comment listing its full option space.
- No `TODO` comments in source. Plain dash, never an em dash. American spelling in identifiers.
- Long Markdown: one full sentence per physical line.

From the spec:

- No Star Wars names in any committed file. Ids and Rust names are neutral (`line_droid`, `heavy_droid`, `commando`); display names in `assets/` are neutral too ("line droid", "heavy droid").
- No file in `examples/foundry/src/` passes 400 lines.
- The damage table, verbatim from spec section 5, as `Resistances` percentages removed:

| Profile | kinetic | energy | ion |
|---|---|---|---|
| `chassis` (droids) | 50 | 0 | -100 |
| `organic` (the player, vermin) | 0 | 25 | 75 |

- Heat, verbatim from spec section 6.2: a shot adds heat; a weapon at 100 or more locks and stays locked until its heat reaches zero; a weapon vents only on a turn its wielder did not fire it.

A practical note for every task: builds are slow when the target directory is cold. Prefix cargo commands with `CARGO_TARGET_DIR=<repo root>/target` when working in a worktree.

## Decisions this plan makes beyond the spec

Each one is a ruling the spec did not settle; the executing controller should know them.

1. **Two more engine changes**, now spec sections 10.4 and 10.5, found while planning. Without 10.4 the game cannot tell which weapon fired; without 10.5 no droid can shoot.
2. **The slice's reactor is on deck 3**, not deck 4. The slice ends at deck 3 (spec section 13), and a reactor it cannot reach is not a playable slice. Setting its charge, then choosing an upgrade, ends the run in victory. The later plan moves it to deck 4 and chains the other two.
3. **Composed encounters are out of the slice.** Spec section 13 lists them as absent; spec section 16 had left it open.
4. **The private asset pack is out of the slice.** The slice ships neutral names only, compiled in.
5. **Four modules the spec's layout did not list:** `heat.rs`, `ammo.rs`, `loot.rs` and `testing.rs`. Without them `gear.rs` would pass the 400-line ceiling.
6. **The targeting uplink** (+1 range) is applied by the game to the ranged weapons the player wears, on equip and unequip, since the engine has no range stat.

---

## File Structure

| File | Responsibility |
|---|---|
| `crates/rl-bevy/src/combat.rs` | Modify: `Struck` message, `Loadout::melee_with` / `ranged_with`, written by `resolve_attacks` |
| `crates/rl-rules/src/ai/snapshot.rs` | Modify: `Snapshot::reach` |
| `crates/rl-rules/src/ai/tactics.rs` | Modify: `ShootAtRange` |
| `crates/rl-bevy/src/items.rs` | Modify: fill `snapshot.reach` beside `snapshot.missiles` |
| `Cargo.toml` | Modify: `examples/foundry` joins the workspace |
| `examples/foundry/Cargo.toml` | Create: tier 3, publish false |
| `examples/foundry/src/main.rs` | Create: plugin wiring and the run's start, nothing else |
| `examples/foundry/src/content.rs` | Create: registries, resistance profiles, the `Content` resource |
| `examples/foundry/src/decks.rs` | Create: tiles, the three deck chains, prefabs |
| `examples/foundry/src/gear.rs` | Create: the item loader and item spawning |
| `examples/foundry/src/heat.rs` | Create: heat, pure and Bevy |
| `examples/foundry/src/ammo.rs` | Create: ammunition |
| `examples/foundry/src/droids.rs` | Create: the monster loader, spawn table, brains, the probe alarm |
| `examples/foundry/src/loot.rs` | Create: floor scatter and drops on death |
| `examples/foundry/src/mission.rs` | Create: the reactor charge and its quest |
| `examples/foundry/src/upgrades.rs` | Create: the pick-one-of-three and applying a pick |
| `examples/foundry/src/input.rs` | Create: keys |
| `examples/foundry/src/testing.rs` | Create: a headless Foundry for the crate's tests |
| `examples/foundry/assets/*.ron` | Create: tiles, items, monsters, quests, abilities |
| `docs/OVERVIEW.md`, `CHANGELOG.md`, `docs/TODO.md`, `README.md` | Modify: in the tasks that change what they describe |

---

### Task 1: A blow says what struck it

**Files:**
- Modify: `crates/rl-bevy/src/combat.rs` (the `Loadout` system param, `resolve_attacks`, `CombatPlugin::build`)
- Modify: `crates/rl-bevy/src/lib.rs` (export `Struck` beside `DamageEvent`, in both the crate root and the prelude)
- Modify: `docs/OVERVIEW.md`, `CHANGELOG.md`
- Test: `crates/rl-bevy/src/combat.rs`, its test module

**Interfaces:**
- Consumes: `Loadout::melee(who) -> Option<MeleeAttack>`, `Loadout::ranged(who) -> Option<RangedAttack>`, and the private `Loadout::worn(who)` iterator they are built on.
- Produces:
  - `pub struct Struck { pub attacker: Entity, pub target: Entity, pub with: Option<Entity>, pub ranged: bool }`, a Bevy `Message`, registered by `CombatPlugin`.
  - `Loadout::melee_with(&self, who: Entity) -> Option<(Option<Entity>, MeleeAttack)>` and `Loadout::ranged_with(&self, who: Entity) -> Option<(Option<Entity>, RangedAttack)>`: the attack and the worn item it came from, `None` when it is the actor's own.

- [ ] **Step 1: Write the failing tests**

Read the existing tests in `crates/rl-bevy/src/combat.rs` first; the melee and ranged cost tests there (`melee_turn_cost`, `shot_turn_cost`) show how to build a headless fight. Follow that idiom. The assertions below are the requirement; the setup follows the file.

```rust
#[test]
fn a_blow_names_the_worn_weapon_it_came_from_and_nothing_when_it_came_from_the_attacker() {
    // Worn: the blow names the item. Bare-handed: it names nothing. A game
    // that heats or spends a weapon learns which one from this, rather than
    // working out for itself which item the engine would have picked.
    let (worn, bare) = (struck_by_melee(true), struck_by_melee(false));
    assert!(worn.with.is_some(), "a worn weapon's blow names it");
    assert_eq!(worn.ranged, false);
    assert_eq!(bare.with, None, "a bare-handed blow names nothing");
}

#[test]
fn a_shot_names_the_item_it_was_fired_from_and_the_one_loadout_would_pick() {
    // Two ranged items worn: the shot names whichever the loadout picks,
    // which is the first in slot order, so the report and the resolver can
    // never disagree about which weapon fired.
    let (struck, first, _second) = struck_by_two_guns();
    assert!(struck.ranged);
    assert_eq!(struck.with, Some(first));
}
```

Write the two helpers beside them. `struck_by_melee(worn: bool) -> Struck` sets up an attacker adjacent to a target, with a `MeleeAttack` either on a worn item (`Item`, `Wearable`, equipped into a slot through `Equipped`) or on the attacker itself, sends one `Intent<Attack>`, runs the app, and returns the single `Struck` written. `struck_by_two_guns() -> (Struck, Entity, Entity)` equips two items each carrying a `RangedAttack`, puts the target three tiles off with a clear line, sends one attack, and returns the `Struck` and both item entities in slot order. Read `Struck` messages the way the file's other tests read `DamageEvent`s.

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p rl-bevy a_blow_names_the_worn_weapon`
Expected: compile error, `cannot find type Struck`.

- [ ] **Step 3: Add the message**

In `crates/rl-bevy/src/combat.rs`, beside `DamageEvent`:

```rust
/// That an attack found something to strike with, and what: written once
/// per attack that lands a blow or fires a shot, before its damage.
///
/// A game hangs what a weapon does to itself on this: heat, ammunition,
/// wear. It names the worn item rather than leaving the game to work out
/// which one the loadout would have chosen, which is the engine's decision
/// and would be copied, and drift, in every game that needed it.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Struck {
    /// Who attacked.
    pub attacker: Entity,
    /// At whom.
    pub target: Entity,
    /// The worn item the attack came from; `None` for the attacker's own,
    /// a fist or a claw, which has nothing to heat or to spend.
    pub with: Option<Entity>,
    /// Whether it was a shot rather than a blow in reach.
    pub ranged: bool,
}
```

Register it in `CombatPlugin::build` with `app.add_message::<Struck>()`, beside the existing `add_message::<DamageEvent>()`.

- [ ] **Step 4: Teach the loadout to say where an attack came from**

`Loadout::melee` and `Loadout::ranged` find the attack with `self.worn(who).find_map(...)`, falling back to the actor's own. Read `worn` to see what its iterator yields; if it does not yield the item entity, extend it so it does. Then add the two `_with` methods, and rebuild `melee` and `ranged` on top of them so there is one place the choice is made:

```rust
    /// The blow `who` strikes, as [`Loadout::melee`], with the worn item it
    /// comes from, or `None` when it is `who`'s own.
    pub fn melee_with(&self, who: Entity) -> Option<(Option<Entity>, MeleeAttack)> {
        let wielded = self.worn(who).find_map(|(item, melee, ..)| melee.copied().map(|m| (Some(item), m)));
        let (from, base) = wielded.or_else(|| self.own.get(who).ok().and_then(|(_, melee, ..)| melee.copied()).map(|m| (None, m)))?;
        let bonus = self.stat(who, |r| r.attack);
        Some((from, MeleeAttack { dice: DiceRoll { bonus: base.dice.bonus + bonus, ..base.dice }, ..base }))
    }

    /// The blow `who` strikes: the first worn item's in slot order, or its
    /// own, with the attack stat added to the roll.
    pub fn melee(&self, who: Entity) -> Option<MeleeAttack> {
        self.melee_with(who).map(|(_, m)| m)
    }
```

Write `ranged_with` and rebuild `ranged` the same way, over `RangedAttack`. Keep `melee` and `ranged`'s existing doc comments.

- [ ] **Step 5: Write it from the resolver**

In `resolve_attacks`, add `mut struck: MessageWriter<Struck>` to the parameters, use the `_with` methods to pick the weapon, and write one `Struck` when a weapon is found, before the damage:

```rust
        let weapon = if geometry::is_adjacent(pos.0, target_pos.0) {
            loadout.melee_with(intent.actor).map(|(from, m)| (from, false, m.kind, m.dice, m.cost))
        } else {
            loadout
                .ranged_with(intent.actor)
                .filter(|(_, r)| line_of_fire(&map, &occupancy, pos.0, target_pos.0, r.range))
                .map(|(from, r)| (from, true, r.kind, r.dice, r.cost))
        };
        let mut spent = rl_core::turn::BASE_ACTION_COST;
        if let Some((from, ranged, kind, dice, cost)) = weapon {
            spent = cost.unwrap_or(rl_core::turn::BASE_ACTION_COST);
            struck.write(Struck { attacker: intent.actor, target, with: from, ranged });
```

and leave the damage writes after it unchanged. If the system now has more parameters than clippy allows, fold `struck` into the existing `Arena` system param rather than allowing the lint.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p rl-bevy`
Expected: PASS, the fingerprint tripwire in `crates/rl-bevy/tests/fingerprint.rs` unmoved: this change writes a new message and changes no roll.

- [ ] **Step 7: Record it**

`docs/OVERVIEW.md`, in the rl-bevy combat entry, one sentence on its own line:

```markdown
  Every attack that finds a weapon writes `Struck`, naming the worn item it came from, so a game can heat or spend a weapon without working out which one the loadout chose.
```

`CHANGELOG.md`, under Unreleased, one sentence per line:

```markdown
- `Struck` names the worn item each attack came from.
  `Loadout::melee_with` and `Loadout::ranged_with` answer the same question for a caller.
```

- [ ] **Step 8: Verify and commit**

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && scripts/check-guide.sh
git add crates/rl-bevy docs/OVERVIEW.md CHANGELOG.md
git commit -m "feat(combat): a blow says what struck it"
```

---

### Task 2: A mind can shoot

**Files:**
- Modify: `crates/rl-rules/src/ai/snapshot.rs` (`Snapshot`, a new field)
- Modify: `crates/rl-rules/src/ai/tactics.rs` (`ShootAtRange`)
- Modify: `crates/rl-rules/src/lib.rs` (export `ShootAtRange` wherever `ThrowAtRange` is exported)
- Modify: `crates/rl-bevy/src/items.rs:515-540`, or wherever `snapshot.missiles` is filled: fill `snapshot.reach` beside it
- Modify: `docs/OVERVIEW.md`, `docs/TODO.md`, `CHANGELOG.md`
- Test: `crates/rl-rules/src/ai/tactics.rs`, its test module

**Interfaces:**
- Consumes: `Loadout::ranged(who)` from rl-bevy, `ThrowAtRange`'s shape, `clear_shot` from `rl_grid::targeting`, `Decision::Attack(A)`.
- Produces:
  - `Snapshot::reach: Option<i32>`, how far the actor's own shot carries; `None` when it has none.
  - `pub struct ShootAtRange { pub chance_pct: u32 }`, a `Tactic`, `Default` 100.

- [ ] **Step 1: Write the failing tests**

Read the test module in `crates/rl-rules/src/ai/tactics.rs` first, particularly the tests for `ThrowAtRange`, and build the snapshots the same way. These run without an `App`.

```rust
#[test]
fn a_shooter_fires_down_a_clear_line_at_an_enemy_in_reach() {
    let s = shooter_snapshot(Some(5), Point::new(4, 0));
    let d = decide(&ShootAtRange::default(), &s, |_| false);
    assert!(matches!(d, Some(Decision::Attack(_))), "an enemy four off, reach five, clear: shoot");
}

#[test]
fn a_shooter_holds_fire_when_the_line_is_blocked_the_enemy_is_out_of_reach_or_at_its_elbow() {
    // Each is its own reason to decline, and each must decline on its own.
    let blocked = decide(&ShootAtRange::default(), &shooter_snapshot(Some(5), Point::new(4, 0)), |p| p == Point::new(2, 0));
    let far = decide(&ShootAtRange::default(), &shooter_snapshot(Some(5), Point::new(7, 0)), |_| false);
    let close = decide(&ShootAtRange::default(), &shooter_snapshot(Some(5), Point::new(1, 0)), |_| false);
    let unarmed = decide(&ShootAtRange::default(), &shooter_snapshot(None, Point::new(4, 0)), |_| false);
    assert!(blocked.is_none(), "a wall in the way");
    assert!(far.is_none(), "seven off with a reach of five");
    assert!(close.is_none(), "adjacent is MeleeAdjacent's");
    assert!(unarmed.is_none(), "nothing to shoot with");
}
```

`shooter_snapshot(reach, enemy_at)` is a snapshot with the actor at the origin, `reach` set, and one enemy at `enemy_at`. `decide(tactic, snapshot, blocks_shot)` evaluates the tactic with that `blocks_shot` predicate and bounds wide enough for the test. Build both from whatever helpers the file's `ThrowAtRange` tests already use.

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p rl-rules a_shooter`
Expected: compile error, `cannot find struct ShootAtRange` or `no field reach`.

- [ ] **Step 3: Add the field**

On `Snapshot`:

```rust
    /// How far the actor's own shot carries, if it has one. Read from
    /// whatever it wields, so a mind that picks up a rifle can shoot
    /// without its brain changing.
    pub reach: Option<i32>,
```

Give it `None` wherever `Snapshot` is constructed.

- [ ] **Step 4: Add the tactic**

In `crates/rl-rules/src/ai/tactics.rs`, after `ThrowAtRange`:

```rust
/// Shoot the nearest enemy in reach down a clear line, when that enemy is
/// not already at its elbow.
///
/// For any mind whose snapshot has a [`Snapshot::reach`]: firing what it
/// holds takes no wits, so a mindless sentry shoots as readily as a
/// sapient one. It leaves an adjacent enemy to [`MeleeAdjacent`], which
/// belongs above it. It shoots when there is a shot and does not back off
/// to keep one; holding a distance is a separate tactic. The line is judged
/// by `blocks_shot`, the predicate a shot flies by, so a mind never fires
/// into a wall it thought was clear.
#[derive(Debug, Clone, Copy)]
pub struct ShootAtRange {
    /// Percentage chance of shooting on a turn there is a shot to take.
    /// Below a hundred so a shooter sometimes closes in instead.
    pub chance_pct: u32,
}

impl Default for ShootAtRange {
    fn default() -> Self {
        Self { chance_pct: 100 }
    }
}

impl<A: Copy> Tactic<A> for ShootAtRange {
    fn name(&self) -> &'static str {
        "shoot_at_range"
    }

    fn evaluate(&self, ctx: &mut TacticCtx<'_, A>) -> Option<Decision<A>> {
        let s = ctx.snapshot;
        let reach = s.reach?;
        if s.enemies.is_empty() {
            return None;
        }
        if self.chance_pct < 100 && !ctx.rng.random_ratio(self.chance_pct.min(100), 100) {
            return None;
        }
        let me = s.me.pos;
        s.enemies
            .iter()
            .filter(|e| (2..=reach).contains(&geometry::chebyshev(me, e.pos)))
            .find(|e| clear_shot(me, e.pos, reach, ctx.bounds, |p| p != me && (ctx.blocks_shot)(p)))
            .map(|e| Decision::Attack(e.id))
    }
}
```

The `enemies` list's order is the nearest-first order `ThrowAtRange` relies on; confirm that by reading where it is built, and say so in the doc comment if it is not.

- [ ] **Step 5: Fill the reach**

Where rl-bevy sets `snapshot.missiles` (around `crates/rl-bevy/src/items.rs:538`), set `snapshot.reach` from the thinker's loadout, `loadout.ranged(thinker).map(|r| r.range)`. If `Loadout` is not reachable from that system, add the fill in `combat.rs` as a sibling system in the same set and say why in its doc comment.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p rl-rules && cargo test -p rl-bevy`
Expected: PASS. No existing brain includes `ShootAtRange`, so the fingerprint tripwire does not move.

- [ ] **Step 7: Record it**

`docs/OVERVIEW.md`, in the rl-rules `ai` entry: `ShootAtRange` fires what the mind wields down a clear line at an enemy two or more tiles off.

`docs/TODO.md` section 2: remove "keep-at-range for a shooter" from the missing-tactics list only if the list reads that way; otherwise add one line saying a mind now shoots when there is a shot, and that holding a distance is what remains.

`CHANGELOG.md`, Unreleased: `ShootAtRange`, and `Snapshot::reach`.

- [ ] **Step 8: Verify and commit**

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && scripts/check-tiers.sh --wasm && scripts/check-guide.sh
git add crates docs CHANGELOG.md
git commit -m "feat(ai): a mind can shoot what it wields"
```

---

### Task 3: The Foundry crate, its registries and a headless harness

**Files:**
- Modify: `Cargo.toml` (workspace `members`)
- Create: `examples/foundry/Cargo.toml`
- Create: `examples/foundry/src/main.rs`, `content.rs`, `testing.rs`
- Test: `examples/foundry/src/content.rs`

**Interfaces:**
- Consumes: `Registry::from_defs`, `DamageKind::new(..).unarmored()`, `SlotDef`, `Resistances::set(kind, pct)`, the `Registries` struct as `examples/delve/src/main.rs::registries` assembles it.
- Produces:
  - `content::registries() -> Registries`, with damage kinds `kinetic`, `energy`, `ion`; slots `main hand`, `off hand`, `head`, `torso`, `arms`, `legs`; factions `commando`, `droids`, `vermin`; tags `weapon`, `armor`, `slug`; statuses `sensors down`.
  - `content::Profile` enum, `Chassis` and `Organic`, and `content::resistances(profile: Profile, registries: &Registries) -> Resistances`.
  - `testing::headless(seed: RunSeed) -> App`.

- [ ] **Step 1: The crate**

`examples/foundry/Cargo.toml`, copying Corsair's shape:

```toml
[package]
name = "foundry"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
rust-version.workspace = true
publish = false
description = "A worked example: ten decks of a droid foundry on rl-engine."

[package.metadata.rl-engine]
# Read by scripts/check-tiers.sh: 0 core, 1 no Bevy, 2 the Bevy layer, 3 the facade and games.
tier = 3

[dependencies]
rl-engine.workspace = true
bevy.workspace = true
rand = { workspace = true, features = ["std", "std_rng"] }
serde.workspace = true
```

Add `"examples/foundry"` to the workspace `members` in the root `Cargo.toml`.

- [ ] **Step 2: Write the failing test**

In `examples/foundry/src/content.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_damage_table_is_the_designs_to_the_percent() {
        // Spec section 5: a droid shrugs off half of a slug and takes double
        // from ion; flesh takes a quarter less from a bolt and nearly nothing
        // from ion. A profile that drifts from this rebalances every fight.
        let r = registries();
        let (kinetic, energy, ion) = (r.damage_kinds.expect("kinetic"), r.damage_kinds.expect("energy"), r.damage_kinds.expect("ion"));
        let chassis = resistances(Profile::Chassis, &r);
        assert_eq!((chassis.get(kinetic), chassis.get(energy), chassis.get(ion)), (50, 0, -100));
        let organic = resistances(Profile::Organic, &r);
        assert_eq!((organic.get(kinetic), organic.get(energy), organic.get(ion)), (0, 25, 75));
    }

    #[test]
    fn every_slot_the_armor_names_is_registered() {
        let r = registries();
        for slot in ["main hand", "off hand", "head", "torso", "arms", "legs"] {
            assert!(r.slots.get_by_name(slot).is_some(), "{slot}");
        }
    }
}
```

Adjust `get_by_name` to whatever lookup `Registry` provides; `expect` panics, so a test that wants a boolean needs the non-panicking one.

- [ ] **Step 3: Run it and watch it fail**

Run: `cargo test -p foundry the_damage_table`
Expected: compile errors, `registries` and `resistances` not found.

- [ ] **Step 4: Write `content.rs`**

```rust
//! What Foundry's world is made of: its damage kinds, slots, sides and
//! the two ways a body takes a hit.
//!
//! Every name here is content the engine knows nothing about; the engine
//! sees ids. The two resistance profiles are the design's damage table,
//! kept in one place so a droid and a commando cannot drift from it.

use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_rules::Resistances;

/// How a body takes a hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// Plated: shrugs off half a slug, takes a bolt in full, and is undone by ion.
    Chassis,
    /// Flesh in plate: a bolt is blunted, ion barely registers.
    Organic,
}

/// The resistance table for `profile`, as percentages removed.
pub fn resistances(profile: Profile, registries: &Registries) -> Resistances {
    let k = &registries.damage_kinds;
    let (kinetic, energy, ion) = (k.expect("kinetic"), k.expect("energy"), k.expect("ion"));
    let mut r = Resistances::new();
    let (a, b, c) = match profile {
        Profile::Chassis => (50, 0, -100),
        Profile::Organic => (0, 25, 75),
    };
    r.set(kinetic, a);
    r.set(energy, b);
    r.set(ion, c);
    r
}
```

Then `pub fn registries() -> Registries`, assembled exactly the way `examples/delve/src/main.rs::registries` assembles Delve's (read lines 144 onward), with these definitions:

- Damage kinds: `DamageKind::new("kinetic")`, `DamageKind::new("energy")`, `DamageKind::new("ion").unarmored()`, since plate does not stop a charge, and `DamageKind::new("care").unarmored()`, the kind a `Mend` is dealt as, as Corsair's and Delve's are. No profile resists `care`, so a heal lands in full.
- Statuses: `StatusDef { badge: Some('~'), ..StatusDef::new("sensors down") }`, cured by time.
- Slots: `main hand`, `off hand`, `head`, `torso`, `arms`, `legs`.
- Factions: `commando`, `droids`, `vermin`.
- Tags: `weapon`, `armor`, `slug`.
- No gases, no stats beyond what `Registries` requires.

- [ ] **Step 5: A minimal `main.rs` and the harness**

`main.rs`, for now, declares the modules and opens a window with `RoguelikePlugins::new("Foundry", 100, 40)`; Task 11 fills it in. Its `//!` doc says what Foundry is in two sentences.

`testing.rs`, modelled on `examples/corsair/src/testing.rs`: `pub fn headless(seed: RunSeed) -> App` adding `FovPlugin, CombatPlugin, MindsPlugin, StatusPlugin, ItemsPlugin, ThrowingPlugin, LightingPlugin, StealthPlugin, FactsPlugin, AbilitiesPlugin`, `add_engine_effects()`, `Seed(seed)` and `UiPlugin`. Later tasks add their systems to it.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p foundry && scripts/check-tiers.sh`
Expected: PASS; the tier check sees `foundry` at tier 3.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock examples/foundry
git commit -m "feat(foundry): a crate, its damage table, and a headless harness"
```

---

### Task 4: Three decks, with vaults that vary

**Files:**
- Create: `examples/foundry/src/decks.rs`, `examples/foundry/assets/tiles.ron`
- Modify: `examples/foundry/src/main.rs` (the `start` system), `testing.rs`
- Test: `examples/foundry/src/decks.rs`

**Interfaces:**
- Consumes: `PlaceRules`, `PlaceBuild::from_context`, `Chain`, `Rooms`, `Doors`, `FarthestExit`, `RandomStart`, `StampOneOf`, `Orient`, `Placement`, `Prefab::parse`, `TileAppearance::load`, `WarpRequest::into_place`, `PlaceRulesRes`.
- Produces:
  - `decks::DECKS: u32 = 3`, `decks::map_of(deck: u32) -> MapId`, `decks::deck_of(map: MapId) -> u32`, maps counting from 1 as Delve's do.
  - `decks::Foundry::new(seed: RunSeed) -> Foundry`, implementing `PlaceRules`, with `tiles()` and `appearance()`.
  - Prefab marks: `A` an armory's guaranteed item, `L` a floor item, `R` the reactor console. Only deck 3 carries `R`.
  - `main::start`, spawning the player as `commando` with `Profile::Organic` resistances, and warping to deck 1.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rl_engine::rl_mapgen::Stamped;

    #[test]
    fn every_deck_builds_over_a_span_of_seeds_with_an_entry_and_an_exit() {
        for s in 0..40 {
            let foundry = Foundry::new(RunSeed(s));
            for deck in 1..=DECKS {
                let built = foundry.build(map_of(deck), None).unwrap_or_else(|e| panic!("deck {deck}, seed {s}: {e}"));
                assert_ne!(built.entry, built.exit, "deck {deck}, seed {s}: the way in is the way out");
            }
        }
    }

    #[test]
    fn only_deck_three_holds_the_reactor_and_it_holds_exactly_one() {
        for s in 0..20 {
            let foundry = Foundry::new(RunSeed(s));
            for deck in 1..=DECKS {
                let built = foundry.build(map_of(deck), None).unwrap();
                let reactors = built.marks.iter().filter(|(c, _)| *c == 'R').count();
                assert_eq!(reactors, usize::from(deck == 3), "deck {deck}, seed {s}");
            }
        }
    }

    #[test]
    fn a_vault_is_not_laid_the_same_way_on_every_seed() {
        // The armory's mark, measured from its stamp's corner, is its
        // facing. Forty seeds laying it one way means orientation did
        // nothing.
        let facings: std::collections::BTreeSet<_> = (0..40)
            .map(|s| Foundry::new(RunSeed(s)).build(map_of(1), None).unwrap())
            .filter_map(|b| b.stamped.iter().find(|st| st.marks.iter().any(|(c, _)| *c == 'A')).map(|st| {
                let (_, at) = st.marks.iter().find(|(c, _)| *c == 'A').unwrap();
                (at.x - st.bounds.x, at.y - st.bounds.y)
            }))
            .collect();
        assert!(facings.len() > 1, "forty seeds laid the armory exactly one way");
    }
}
```

Read `PlaceBuild` in `crates/rl-bevy/src/places.rs` first: if it does not expose `entry`, `exit`, `marks` and the `Stamped` outputs under those names, use the names it does expose and keep the assertions.

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p foundry every_deck_builds`
Expected: compile error, `Foundry` not found.

- [ ] **Step 3: Tiles**

Register, in `Foundry::new`, as Delve's `Whale::new` does: `hull` (wall), `deck` (floor), `grating` (floor, `move_cost(120)`), `bulkhead` (wall), `hatch` (floor, opaque, the doors pass's door), `console` (wall, the reactor's), `lamp` (wall, lit). `assets/tiles.ron` gives each its glyph and colours, with the option-space comment `examples/delve/assets/tiles.ron` carries; read it and match.

- [ ] **Step 4: The assembly-hall chain, and the vaults**

The band for decks 1 to 3 is rooms and doors, per spec section 9:

```rust
impl PlaceRules for Foundry {
    fn build(&self, map: MapId, _: Option<&WorldGraph>) -> Result<PlaceBuild, BuildError> {
        let deck = deck_of(map);
        let mut ctx = BaseContext::blank(70, 40, self.tiles.clone(), self.hull);
        // One stream per deck, so building deck 2 first does not change deck 1.
        let seed = RunSeed(self.seed.0 ^ (deck as u64) << 32);
        let mut chain = Chain::new()
            .then(Rooms { floor: self.deck, min_size: 5, max_size: 11, ..Default::default() })
            .then(Doors { door: self.hatch, ..Default::default() })
            .then(StampOneOf { name: "armory", choices: self.armories()?, at: Placement::AnyRoom, orient: Orient::TurnedOrMirrored })
            .then(StampOneOf { name: "store", choices: self.stores()?, at: Placement::AnyRoom, orient: Orient::TurnedOrMirrored });
        if deck == 3 {
            // The reactor meets a corridor through its hatch, so it keeps its facing.
            chain = chain.then(StampPrefab { name: "reactor", prefab: self.reactor()?, at: Placement::AnyRoom, orient: Orient::Fixed });
        }
        chain.then(RandomStart).then(FarthestExit).run(&mut ctx, seed)?;
        Ok(PlaceBuild::from_context(&ctx))
    }
}
```

Check `Doors`' real fields in `crates/rl-mapgen/src/dungeon.rs` before using them. `armories()` returns two or three `(Prefab, u32)` candidates drawn as `vault` is in Delve's `heart()`, each asymmetric and each holding one `A`; `stores()` two candidates each holding two `L`s; `reactor()` one piece with one `R` beside a `console` wall. Keep each piece under 11 cells on a side, so `Rooms`' largest room holds it.

- [ ] **Step 5: The start**

In `main.rs`, a `start` system for `NewRun`, following `examples/delve/src/main.rs:277-365`: build `Foundry`, insert `WorldMap::new(foundry.tiles().tables())`, `foundry.appearance()`, `PlaceRulesRes(Box::new(foundry))`, `CombatRules::new(&factions).hostile(commando, droids).hostile(commando, vermin).hostile(droids, vermin)`, `DamageStages(vec![Box::new(SubtractArmor)])`, and `Lighting::dark()`. Spawn the player: `Actor, Player, Blocks, Position(Point::ZERO), Viewshed::new(20), RevealsMap, Health::full(30), Armor(0), Faction(commando), Resists(resistances(Profile::Organic, &registries)), MeleeAttack { kind: kinetic, dice: DiceRoll::new(1, 3), cost: None }, Name::new("you"), Glyph::new('@', Color::WHITE).on_layer(10), Inventory::default(), Equipped(Equipment::with_slot_count(registries.slots.len()))`. Warp to `map_of(1)`. Set `EngineState::Playing`. Add `start` to `testing::headless`.

- [ ] **Step 6: A headless run lands on deck 1**

In `main.rs`'s tests:

```rust
#[test]
fn a_new_run_puts_the_commando_on_deck_one() {
    let mut app = crate::testing::headless(RunSeed(3));
    for _ in 0..5 {
        app.update();
    }
    let map = app.world().resource::<WorldMap>().current();
    assert_eq!(crate::decks::deck_of(map), 1);
}
```

- [ ] **Step 7: Run and commit**

Run: `cargo test -p foundry`
Expected: PASS.

```bash
git add examples/foundry
git commit -m "feat(foundry): three decks of assembly halls, with vaults that turn"
```

---

### Task 5: Gear, from a file

**Files:**
- Create: `examples/foundry/src/gear.rs`, `examples/foundry/assets/items.ron`
- Test: `examples/foundry/src/gear.rs`

**Interfaces:**
- Consumes: `Names::load`, `NameRef<T>`, `EquipShape::{in_slot, in_any, and_claims}`, `Item`, `Wearable`, `Tagged`, `Stack`, `MeleeAttack`, `RangedAttack`, `Resists`, `Armor`, `DarkSight`, `ItemEvent::{Equipped, Unequipped}`.
- Produces:
  - `gear::ItemDef`, `gear::Armory { defs: Registry<ItemDef>, table: BandedTable<Id<ItemDef>> }`, `Armory::load(registries: &Registries) -> Armory`.
  - `gear::spawn_item(commands: &mut Commands, armory: &Armory, id: Id<ItemDef>, registries: &Registries) -> Entity`: an item entity with every component its definition implies, including `Heat` and `Ammo` from Tasks 6 and 7, which this task declares as fields only.
  - `gear::grant_dark_sight`, a `TurnSet::React` system: wearing an item with `dark_sight` gives the wearer `DarkSight`, and taking it off takes it away.

- [ ] **Step 1: The file**

`examples/foundry/assets/items.ron`, the slice's six weapons, six armor pieces, and slugs. Numbers are spec sections 6.3, 6.4 and 7, verbatim:

```ron
// Foundry's gear: what lies on the decks and what the dead leave behind.
//
// Every field (the ones marked "optional" may be left out):
//   name:      unique; spawn tables and drops refer to it
//   glyph:     one character
//   color:     (r, g, b) in 0..=1
//   slot:      optional; where it is worn: "main hand" | "off hand" | "head" | "torso" | "arms" | "legs"
//   either:    optional; true for a one-handed weapon that fits either hand, so two can be dual wielded
//   also:      optional; slots it also claims when worn, for two-handers
//   tags:      optional; "weapon" | "armor" | "slug"
//   armor:     optional; flat armor while worn
//   resists:   optional; [(damage kind, percent removed)] while worn: "kinetic" | "energy" | "ion"
//   melee:     optional; (roll, kind) while wielded, roll as "NdS+B"
//   ranged:    optional; (range, roll, kind) while wielded
//   cost:      optional; what one blow or shot costs, in hundredths of a step; absent is 100
//   heat:      optional; (per shot, vent per quiet turn); the weapon locks at 100 until it cools to 0
//   ammo:      optional; the tag of what one shot spends, as "slug"
//   dark_sight: optional; tiles seen without light while worn
//   stack:     optional; whether copies merge into one counted entry
//   spawn:     optional; (min deck, max deck, weight) for lying on a deck
#![enable(implicit_some)]
[
    (name: "vibroblade", glyph: '/', color: (0.8, 0.85, 0.9), slot: "main hand", either: true, tags: ["weapon"], melee: ("1d6+1", "kinetic"), cost: 100, spawn: (1, 10, 3)),
    (name: "vibro-axe", glyph: '/', color: (0.7, 0.75, 0.8), slot: "main hand", also: ["off hand"], tags: ["weapon"], melee: ("2d6+1", "kinetic"), cost: 140, spawn: (2, 10, 2)),
    (name: "hand blaster", glyph: '}', color: (0.95, 0.4, 0.3), slot: "main hand", either: true, tags: ["weapon"], ranged: (5, "1d6", "energy"), cost: 80, heat: (15, 20), spawn: (1, 10, 3)),
    (name: "blaster carbine", glyph: '}', color: (0.9, 0.3, 0.25), slot: "main hand", also: ["off hand"], tags: ["weapon"], ranged: (9, "1d8+1", "energy"), cost: 100, heat: (22, 22), spawn: (2, 10, 2)),
    (name: "ion pistol", glyph: '}', color: (0.4, 0.7, 1.0), slot: "main hand", either: true, tags: ["weapon"], ranged: (5, "1d6", "ion"), cost: 90, heat: (25, 20), spawn: (1, 10, 2)),
    (name: "slugthrower pistol", glyph: '}', color: (0.6, 0.55, 0.45), slot: "main hand", either: true, tags: ["weapon"], ranged: (6, "1d8", "kinetic"), cost: 100, ammo: "slug", spawn: (1, 10, 2)),
    (name: "slugs", glyph: ',', color: (0.7, 0.65, 0.5), tags: ["slug"], stack: true, spawn: (1, 10, 4)),
    (name: "commando helmet", glyph: '[', color: (0.85, 0.85, 0.8), slot: "head", tags: ["armor"], armor: 1, spawn: (1, 10, 2)),
    (name: "rangefinder helmet", glyph: '[', color: (0.6, 0.85, 0.7), slot: "head", tags: ["armor"], armor: 1, dark_sight: 6, spawn: (2, 10, 1)),
    (name: "scrap plate", glyph: '[', color: (0.55, 0.5, 0.45), slot: "torso", tags: ["armor"], armor: 1, spawn: (1, 10, 3)),
    (name: "phase one plate", glyph: '[', color: (0.9, 0.9, 0.85), slot: "torso", tags: ["armor"], armor: 2, resists: [("energy", 10)], spawn: (2, 10, 2)),
    (name: "combat gauntlets", glyph: '[', color: (0.7, 0.7, 0.7), slot: "arms", tags: ["armor"], armor: 1, spawn: (1, 10, 2)),
    (name: "armored greaves", glyph: '[', color: (0.65, 0.65, 0.6), slot: "legs", tags: ["armor"], armor: 1, spawn: (1, 10, 2)),
]
```

- [ ] **Step 2: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_item_loads_and_every_spawn_band_on_the_first_three_decks_has_something() {
        let r = crate::content::registries();
        let armory = Armory::load(&r);
        assert_eq!(armory.defs.len(), 13);
        assert!(armory.table.gaps(1..=3).is_empty(), "a deck with nothing to find");
    }

    #[test]
    fn a_weapons_file_numbers_reach_its_attack_and_a_two_hander_claims_the_off_hand() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (axe, carbine) = spawn_two(&mut app, "vibro-axe", "blaster carbine");
        let world = app.world();
        let melee = world.get::<MeleeAttack>(axe).expect("the axe swings");
        assert_eq!(melee.cost, Some(140));
        let ranged = world.get::<RangedAttack>(carbine).expect("the carbine fires");
        assert_eq!((ranged.range, ranged.cost), (9, Some(100)));
        let shape = &world.get::<Wearable>(axe).unwrap().0;
        assert_eq!(shape.slots().count(), 2, "main hand and off hand");
    }

    #[test]
    fn wearing_the_rangefinder_gives_dark_sight_and_taking_it_off_takes_it_away() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (player, helmet) = wear(&mut app, "rangefinder helmet");
        assert_eq!(app.world().get::<DarkSight>(player).map(|d| d.0), Some(6));
        unwear(&mut app, player, helmet);
        assert_eq!(app.world().get::<DarkSight>(player), None);
    }
}
```

`spawn_two`, `wear` and `unwear` are small helpers in the test module over `spawn_item` and the engine's `Equip` and `Unequip` intents; read `crates/rl-bevy/src/items.rs` for how an actor equips. `Registry::len` may be named otherwise; use its real name.

- [ ] **Step 3: Run them and watch them fail**

Run: `cargo test -p foundry every_item_loads`
Expected: compile error.

- [ ] **Step 4: The loader and the spawner**

`ItemDef` mirrors `examples/corsair/src/items.rs:34-59`, with the extra fields the file's header lists: `either: bool`, `melee: Option<(DiceRoll, NameRef<DamageKind>)>`, `cost: Option<u32>`, `heat: Option<(u32, u32)>`, `ammo: Option<NameRef<TagDef>>`, `dark_sight: Option<i32>`, `resists: Vec<(NameRef<DamageKind>, i32)>`. `Armory::load` reads `items.ron` through `Names::load`, validates as Corsair's does (`also` without `slot`, `stack` with `slot`, `either` with `also` are errors), and builds the spawn table with `BandedEntry::new(id).bands(lo, hi).weight(w)`.

The shape: `either` becomes `EquipShape::in_any([main hand, off hand])`, `also` becomes `EquipShape::in_slot(slot).and_claims(..)`, otherwise `EquipShape::in_slot(slot)`.

`spawn_item` spawns `Item`, `Name`, `Glyph`, `Tagged`, and, when present, `Wearable`, `MeleeAttack { kind, dice, cost }`, `RangedAttack { kind, dice, range, cost }`, `Armor`, `Resists`, `Stack { key: id.index() as u64, count: 1 }`, `Heat::new(per_shot, vent)` from Task 6's module and `Ammo { tag }` from Task 7's module, and `WornDarkSight(n)`, a component declared here. Declare `Heat` in `heat.rs` and `Ammo` in `ammo.rs` now, each with its fields, a doc comment and, for `Heat`, the `Heat::new` constructor Task 6 specifies, so this task compiles; Tasks 6 and 7 give them behaviour.

`grant_dark_sight` reads `ItemEvent::Equipped { actor, item }` and `ItemEvent::Unequipped { actor, item }` and inserts or removes `DarkSight` on the actor when the item has `WornDarkSight`. If the player could wear two such items, it takes the largest; say so in its doc comment.

- [ ] **Step 5: Run and commit**

Run: `cargo test -p foundry`
Expected: PASS.

```bash
git add examples/foundry
git commit -m "feat(foundry): six weapons, six pieces of armor and slugs, from a file"
```

---

### Task 6: Heat

**Files:**
- Modify: `examples/foundry/src/heat.rs`
- Modify: `examples/foundry/src/main.rs`, `testing.rs` (register the systems)
- Test: `examples/foundry/src/heat.rs`

**Interfaces:**
- Consumes: `Struck` from Task 1, `TurnEnd`, `MeleeAttack`, `RangedAttack`, `MessageLog`.
- Produces:
  - `heat::CAPACITY: u32 = 100`.
  - `heat::Heat { per_shot: u32, vent: u32, now: u32, locked: bool, fired: bool }`, `Heat::new(per_shot, vent)`, `Heat::fire(&mut self) -> bool` (whether this shot locked it), `Heat::turn_end(&mut self) -> bool` (whether this turn unlocked it).
  - `heat::Stowed`, an enum holding a `MeleeAttack` or a `RangedAttack` taken off a locked weapon.
  - Systems `heat_on_struck` and `vent_heat`, both in `TurnSet::React`.

- [ ] **Step 1: Write the failing rules tests**

These test the struct alone, with no `App`, since heat is rules arithmetic:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_weapon_fires_exactly_its_burst_before_it_locks() {
        // Spec sections 6.3 and 6.4: burst is the shots to reach 100.
        for (per_shot, burst) in [(15, 7), (22, 5), (25, 4), (20, 5), (30, 4), (35, 3), (40, 3), (45, 3)] {
            let mut h = Heat::new(per_shot, 20);
            let shots = (1..=10).find(|_| h.fire()).expect("locks within ten");
            assert_eq!(shots, burst, "per shot {per_shot}");
        }
    }

    #[test]
    fn a_locked_weapon_stays_locked_until_it_has_vented_all_the_way_to_zero() {
        let mut h = Heat::new(25, 20);
        while !h.fire() {}
        assert_eq!(h.now, 100);
        // 100 at 20 a quiet turn is five turns; unlocked on the fifth, not before.
        let unlocked_on = (1..=10).find(|_| h.turn_end()).unwrap();
        assert_eq!(unlocked_on, 5);
        assert_eq!(h.now, 0);
    }

    #[test]
    fn a_turn_it_fired_on_does_not_vent_and_heat_never_goes_below_zero() {
        let mut h = Heat::new(15, 20);
        h.fire();
        h.turn_end();
        assert_eq!(h.now, 15, "fired this turn, so nothing vented");
        h.turn_end();
        assert_eq!(h.now, 0, "a quiet turn vents, and 15 less 20 is zero, not less");
    }
}
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p foundry a_weapon_fires_exactly_its_burst`
Expected: compile error, `Heat::fire` not found.

- [ ] **Step 3: The rules**

```rust
/// How hot a weapon may run before it locks.
pub const CAPACITY: u32 = 100;

/// A weapon's heat: what a shot adds, what a quiet turn sheds, and where
/// it stands.
///
/// It locks at [`CAPACITY`] and stays locked until it has vented all the
/// way to zero, so emptying a weapon is a commitment to cool it rather
/// than a pause. It vents only on a turn it did not fire, which is what
/// lets heat build on a fast weapon at all.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Heat {
    /// Heat one shot or swing adds.
    pub per_shot: u32,
    /// Heat a turn without firing sheds.
    pub vent: u32,
    /// Heat now.
    pub now: u32,
    /// Whether it has locked and not yet cooled to zero.
    pub locked: bool,
    /// Whether it fired this turn.
    pub fired: bool,
}

impl Heat {
    /// Cold and unlocked.
    pub fn new(per_shot: u32, vent: u32) -> Self {
        Self { per_shot, vent, now: 0, locked: false, fired: false }
    }

    /// Records a shot, and answers whether this shot locked it.
    pub fn fire(&mut self) -> bool {
        self.now = self.now.saturating_add(self.per_shot);
        self.fired = true;
        let locks = !self.locked && self.now >= CAPACITY;
        self.locked |= locks;
        locks
    }

    /// Ends a turn: vents if it did not fire, and answers whether this
    /// turn unlocked it.
    pub fn turn_end(&mut self) -> bool {
        if !self.fired {
            self.now = self.now.saturating_sub(self.vent);
        }
        self.fired = false;
        let unlocks = self.locked && self.now == 0;
        if unlocks {
            self.locked = false;
        }
        unlocks
    }
}
```

- [ ] **Step 4: Run the rules tests**

Run: `cargo test -p foundry heat`
Expected: PASS.

- [ ] **Step 5: Write the failing wiring test**

```rust
#[test]
fn an_overheated_weapon_stops_firing_and_the_next_one_in_hand_takes_over() {
    // Two hand blasters, both hands. The first fires its burst of seven and
    // locks; the loadout then finds the second, because the locked one no
    // longer carries a ranged attack. That is dual wielding under heat.
    let mut app = crate::testing::headless(RunSeed(1));
    let (player, first, second) = crate::testing::dual_blasters(&mut app);
    let struck: Vec<_> = crate::testing::fire_at_a_target(&mut app, player, 8);
    assert!(struck[..7].iter().all(|s| s.with == Some(first)));
    assert_eq!(struck[7].with, Some(second), "the eighth shot comes from the other hand");
    assert!(app.world().get::<RangedAttack>(first).is_none(), "locked: stowed");
}
```

Add the two helpers to `testing.rs`: `dual_blasters(&mut App) -> (Entity, Entity, Entity)` spawns and equips two hand blasters, and `fire_at_a_target(&mut App, shooter: Entity, shots: usize) -> Vec<Struck>` places a target three tiles off with a clear line and sends that many attacks, collecting the `Struck` messages in order.

- [ ] **Step 6: The systems**

```rust
/// An attack a locked weapon had, put by until it cools.
#[derive(Component, Debug, Clone, Copy)]
pub enum Stowed {
    /// A blow.
    Melee(MeleeAttack),
    /// A shot.
    Ranged(RangedAttack),
}
```

`heat_on_struck`: for each `Struck { with: Some(item), attacker, .. }` whose item has `Heat`, call `fire`; when it locks, remove the item's `RangedAttack` or `MeleeAttack`, insert it as `Stowed`, and log that the weapon has locked. Taking the attack off the item is what makes the engine's loadout pass over it, so no engine knows about heat.

`vent_heat`: for each whole turn in `TurnEnd`, call `turn_end` on every `Heat`; when one unlocks, take its `Stowed` off and put the attack back, and log that it is cool.

Register both in `TurnSet::React` in `main.rs` and in `testing::headless`.

- [ ] **Step 7: A facet**

The gear panel cannot know about heat, so push it as a `Facet` in `ViewSet::Annotate`: for each row of the gear view whose item has `Heat`, a facet keyed `heat` reading `NN%`, or `locked` while locked, with a warning tone at or above 70. Read `crates/rl-ui/src/facet.rs` and how Corsair pushes its wielded-weapon facet before writing it.

- [ ] **Step 8: Run and commit**

Run: `cargo test -p foundry`
Expected: PASS.

```bash
git add examples/foundry
git commit -m "feat(foundry): blasters run hot, lock at a hundred and cool to nothing"
```

---

### Task 7: Ammunition

**Files:**
- Modify: `examples/foundry/src/ammo.rs`
- Modify: `main.rs`, `testing.rs` (register the systems)
- Test: `examples/foundry/src/ammo.rs`

**Interfaces:**
- Consumes: `Struck`, `Inventory`, `Tagged`, `Stack`, `Stowed` from Task 6, `RangedAttack`.
- Produces: `ammo::Ammo { tag: TagId }`; systems `spend_ammo` in `TurnSet::React` and `reload`, which puts a dry weapon's shot back once its wielder carries ammunition again.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_shot_spends_one_slug_and_the_last_slug_leaves_the_weapon_dry() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (player, pistol) = crate::testing::slug_pistol_with(&mut app, 2);
        let struck = crate::testing::fire_at_a_target(&mut app, player, 3);
        assert_eq!(struck.len(), 2, "two slugs, two shots; the third finds nothing to fire");
        assert!(app.world().get::<RangedAttack>(pistol).is_none(), "dry");
    }

    #[test]
    fn a_dry_weapon_fires_again_once_its_wielder_picks_up_slugs() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (player, pistol) = crate::testing::slug_pistol_with(&mut app, 0);
        crate::testing::give_slugs(&mut app, player, 3);
        app.update();
        assert!(app.world().get::<RangedAttack>(pistol).is_some(), "loaded again");
    }
}
```

`slug_pistol_with(&mut App, slugs: u32) -> (Entity, Entity)` spawns the player with an equipped slugthrower pistol and a stack of `slugs`, and runs whatever makes an empty pistol dry at the start. `give_slugs` adds a stack to the inventory.

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p foundry each_shot_spends_one_slug`

- [ ] **Step 3: The systems**

`spend_ammo`: for each `Struck { ranged: true, with: Some(item), attacker, .. }` whose item has `Ammo { tag }`, find the first item in the attacker's `Inventory` tagged `tag` with a `Stack`, take one, and despawn it at zero. If none of that tag is left, stow the weapon's `RangedAttack` as `Stowed::Ranged`, with a `Dry` marker so `vent_heat` never restores it, and log that it is empty.

`reload`: for each weapon with `Ammo` and `Dry`, whose wearer's inventory holds a stack of its tag, restore the stowed attack and remove `Dry`. Also make a weapon that is equipped with no ammunition carried go dry at once, so the first shot is never free; run it on `ItemEvent::Equipped` and on picking up.

Check `vent_heat` from Task 6 skips a `Dry` weapon, and that a weapon never carries both `Heat` and `Ammo`: validate that in `Armory::load` and make it a load error.

- [ ] **Step 4: Run and commit**

Run: `cargo test -p foundry`
Expected: PASS.

```bash
git add examples/foundry
git commit -m "feat(foundry): a slugthrower spends slugs and runs dry"
```

---

### Task 8: Droids and vermin

**Files:**
- Create: `examples/foundry/src/droids.rs`, `examples/foundry/assets/monsters.ron`
- Modify: `main.rs`, `testing.rs`
- Test: `examples/foundry/src/droids.rs`

**Interfaces:**
- Consumes: `BandedTable`, `BandedEntry`, `Brain`, `MeleeAdjacent`, `ShootAtRange` from Task 2, `FleeWhenHurt`, `Hunt`, `SearchLastKnown`, `Wander`, `Wits`, `Mind`, `Perception`, `DarkSight`, `Notice`, `Aware`, `Noticed`, `Resists`, `resistances` from Task 3, `DamageDealt { target, hit, dealt }`, `TurnEnd`.
- Produces:
  - `droids::MonsterDef`, `droids::Roster { defs, table, brains }`, `Roster::load(registries: &Registries) -> Roster`.
  - `droids::spawn_monster(commands, roster, id, at, map, registries) -> Entity`.
  - `droids::Alarm`, a marker for a monster whose noticing wakes its whole deck, and the system `sound_alarm` in `TurnSet::React`.
  - `droids::Jammed { sight: i32, turns: u32 }`, and the systems `jam_sensors` and `unjam_sensors` in `TurnSet::React`.

- [ ] **Step 1: The file**

`examples/foundry/assets/monsters.ron`. A monster may carry several spawn entries; that is how a group grows with depth, as in the sibling project's spawn system. Numbers from spec sections 8.1 and 8.5:

```ron
// Foundry's roster: what walks the decks.
//
// Every field (the ones marked "optional" may be left out):
//   name:       unique
//   glyph:      one character
//   color:      (r, g, b) in 0..=1
//   hp:         maximum health
//   armor:      flat armor
//   profile:    how it takes a hit: "chassis" | "organic"
//   faction:    "droids" | "vermin"
//   wits:       "mindless" | "animal" | "sapient", or a list as in the engine's Wits
//   perception: how far it sees, in tiles
//   dark_sight: optional; tiles it sees without light: radar, for the droids that have it
//   melee:      (roll, kind)
//   ranged:     optional; (range, roll, kind); a droid with one shoots
//   speed:      percent, 100 normal
//   flee_at:    percent health at or below which it runs; 0 never flees
//   alarm:      optional; true when noticing an enemy wakes every droid on the deck
//   spawn:      one or more (min deck, max deck, weight, min group, max group);
//               several entries let a group grow with depth
//   drops:      optional; [(item name, percent chance)] rolled on death
#![enable(implicit_some)]
[
    (name: "line droid", glyph: 'b', color: (0.85, 0.75, 0.55), hp: 8, armor: 0, profile: "chassis", faction: "droids", wits: "mindless",
     perception: 8, melee: ("1d2", "kinetic"), ranged: (5, "1d4", "energy"), speed: 100, flee_at: 0,
     spawn: [(1, 2, 100, 1, 2), (2, 4, 90, 2, 3)], drops: [("hand blaster", 5), ("slugs", 5)]),
    (name: "probe droid", glyph: 'p', color: (0.5, 0.5, 0.55), hp: 6, armor: 0, profile: "chassis", faction: "droids", wits: "mindless",
     perception: 10, dark_sight: 4, melee: ("1d2", "kinetic"), ranged: (4, "1d4", "energy"), speed: 120, flee_at: 0, alarm: true,
     spawn: [(1, 6, 20, 1, 1)], drops: [("slugs", 10)]),
    (name: "heavy droid", glyph: 'B', color: (0.6, 0.6, 0.7), hp: 18, armor: 2, profile: "chassis", faction: "droids", wits: "mindless",
     perception: 8, melee: ("1d4", "kinetic"), ranged: (6, "1d6+1", "energy"), speed: 90, flee_at: 0,
     spawn: [(3, 8, 40, 1, 1)], drops: [("blaster carbine", 10), ("phase one plate", 10)]),
    (name: "coolant rat", glyph: 'r', color: (0.6, 0.7, 0.75), hp: 4, armor: 0, profile: "organic", faction: "vermin", wits: "animal",
     perception: 6, melee: ("1d3", "kinetic"), speed: 110, flee_at: 50,
     spawn: [(1, 7, 50, 1, 3)]),
]
```

The spec's drop rates, section 12, are line droid 10%, probe droid 10% and heavy droid 20%, as a chance of dropping something. Each entry above is rolled on its own, so two 5% entries give a little under 10% of dropping anything; that is close enough to the design, and simpler than a table that picks one.

- [ ] **Step 2: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn every_deck_in_the_slice_has_something_to_spawn() {
        let roster = Roster::load(&crate::content::registries());
        assert!(roster.table.gaps(1..=3).is_empty());
    }

    #[test]
    fn line_droids_come_alone_or_in_pairs_on_deck_one_and_never_more() {
        // Spec section 8.5: lone sentries first, bigger groups deeper.
        let roster = Roster::load(&crate::content::registries());
        let line = roster.defs.expect("line droid");
        let mut rng = rand::rngs::StdRng::seed_from_u64(9);
        let sizes: std::collections::BTreeSet<u32> =
            (0..2000).filter_map(|_| roster.table.pick_group(1, &mut rng)).filter(|(id, _)| **id == line).map(|(_, n)| n).collect();
        assert_eq!(sizes, [1, 2].into_iter().collect());
    }

    #[test]
    fn a_droid_with_a_blaster_shoots_rather_than_walking_up_to_punch() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (droid, player) = crate::testing::droid_facing_player(&mut app, "line droid", 4);
        let struck = crate::testing::run_until_struck(&mut app, droid, 10);
        assert!(struck.ranged, "four tiles off with a clear line: it shoots");
        assert_eq!(struck.target, player);
    }

    #[test]
    fn a_probe_that_notices_the_player_wakes_every_droid_on_its_deck() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (probe, player, others) = crate::testing::probe_and_sleepers(&mut app);
        app.world_mut().write_message(Noticed { observer: probe, subject: player, at: Point::ZERO });
        app.update();
        for d in others {
            assert!(app.world().get::<Aware>(d).is_some_and(|a| a.of(player).is_alert()), "a sleeper slept through the alarm");
        }
    }
}
```

`Noticed` may carry fields beyond these three; read `crates/rl-bevy/src/stealth.rs:73` and fill them. The three `testing.rs` helpers are small: spawn named monsters beside the player at fixed points and run the app.

- [ ] **Step 3: Run them and watch them fail**

Run: `cargo test -p foundry every_deck_in_the_slice`

- [ ] **Step 4: The loader, the brains and the spawner**

`Roster::load` mirrors `examples/delve/src/main.rs:295-307`: read the file through `Names`, then one `BandedEntry` per spawn tuple, `BandedEntry::new(id).bands(lo, hi).weight(w).group(gmin, gmax)`. The brain for a monster with `ranged` is `MeleeAdjacent`, then `ShootAtRange::default()`, then `FleeWhenHurt` if `flee_at > 0`, then `Hunt`, `SearchLastKnown`, `Wander { chance_pct: 30 }`; without `ranged`, drop `ShootAtRange`.

`spawn_monster` spawns the actor with `Actor, Blocks, Position, OnMap, Health::full(hp), Armor, Faction, Resists(resistances(profile, ..)), Perception, Mind, Notice(NoticeStats::default()), MeleeAttack { cost: None, .. }`, `RangedAttack { cost: None, .. }` when it has one, `DarkSight(n)` when it has one, `Alarm` when it has one, `Name` and `Glyph`, and a `Kind(id)` component naming its definition, for drops.

- [ ] **Step 5: The alarm**

`sound_alarm`, in `TurnSet::React`: for each `Noticed` whose observer has `Alarm`, every actor of the `droids` faction on the observer's map is made alert to the subject at `at`, through `Aware`'s own methods as `crates/rl-bevy/src/stealth.rs::update_awareness` uses them. Log one line the first time a deck's alarm sounds.

- [ ] **Step 6: Ion blinds radar**

Spec section 8.2: an ion hit applies **sensors down**, which strips a droid's dark sight for a few turns. This is what makes ion the answer to radar as well as to plating.

Write the test first, in `droids.rs`:

```rust
#[test]
fn an_ion_hit_blinds_a_probes_radar_for_three_turns_and_a_slug_does_not() {
    let mut app = crate::testing::headless(RunSeed(1));
    let probe = crate::testing::lone_monster(&mut app, "probe droid");
    crate::testing::hit(&mut app, probe, "kinetic", 1);
    assert_eq!(app.world().get::<DarkSight>(probe).map(|d| d.0), Some(4), "a slug does nothing to its sensors");
    crate::testing::hit(&mut app, probe, "ion", 1);
    assert_eq!(app.world().get::<DarkSight>(probe), None, "blinded");
    crate::testing::pass_turns(&mut app, 3);
    assert_eq!(app.world().get::<DarkSight>(probe).map(|d| d.0), Some(4), "and back after three turns");
}
```

Then a `Jammed { sight: i32, turns: u32 }` component and two systems in `TurnSet::React`. `jam_sensors` reads `DamageDealt` for hits of the `ion` kind that dealt more than nothing to a target with `DarkSight`: it takes the `DarkSight` off, keeps it in `Jammed` with three turns, and inflicts the `sensors down` status so the target's badge shows it. A second ion hit while jammed resets the turns to three rather than stacking. `unjam_sensors` counts `Jammed` down once per whole turn in `TurnEnd` and puts the `DarkSight` back at zero. A player wearing a rangefinder helmet is jammed the same way, since the rule is about the sensor rather than about droids; Task 5's `grant_dark_sight` must not re-grant dark sight to a jammed wearer, so check for `Jammed` there.

- [ ] **Step 7: Population**

A deck is populated the first time it is entered, as Delve's floors are: read how Delve fills a floor on first arrival and follow it. Draw from `Seed::stream(b"foundry.spawns", deck as u64)`, never from a combat stream. Aim for `4 + deck * 2` groups a deck, placed in rooms away from the entry.

- [ ] **Step 8: Run and commit**

Run: `cargo test -p foundry`
Expected: PASS.

```bash
git add examples/foundry
git commit -m "feat(foundry): line droids, probes, heavies and rats, that shoot and raise the alarm"
```

---

### Task 9: Loot on the decks and from the dead

**Files:**
- Create: `examples/foundry/src/loot.rs`
- Modify: `main.rs`, `testing.rs`
- Test: `examples/foundry/src/loot.rs`

**Interfaces:**
- Consumes: `Armory` and `spawn_item` from Task 5, `Roster` and `Kind` from Task 8, the deck's `A` and `L` marks from Task 4, `Seed::stream`, `DeathEvent { entity, at, .. }`.
- Produces: `loot::Drops(StdRng)`, a resource seeded from `Seed::stream(b"foundry.drops", 0)`; `loot::roll_drops(drops: &[(Id<ItemDef>, u32)], rng: &mut impl Rng) -> Vec<Id<ItemDef>>`; systems `scatter_on_arrival` and `drop_on_death`.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn a_ten_percent_drop_lands_near_one_time_in_ten_over_many_deaths() {
        // A property over many rolls rather than one lucky seed: the
        // design's rate, within a margin a correct roll never leaves.
        let armory = crate::gear::Armory::load(&crate::content::registries());
        let slugs = armory.defs.expect("slugs");
        let mut rng = rand::rngs::StdRng::seed_from_u64(4);
        let hits = (0..10_000).filter(|_| !roll_drops(&[(slugs, 10)], &mut rng).is_empty()).count();
        assert!((850..=1150).contains(&hits), "{hits} drops in ten thousand");
    }

    #[test]
    fn drops_never_draw_from_the_combat_stream() {
        // The randomness rule: a game's draws come from its own stream. A
        // kill that rolled loot from the combat stream would shift every
        // later blow in the run.
        let (with_loot, without) = crate::testing::combat_rolls_across_a_kill(RunSeed(8));
        assert_eq!(with_loot, without);
    }

    #[test]
    fn every_armory_on_a_deck_holds_something_and_every_store_holds_two() {
        let mut app = crate::testing::headless(RunSeed(5));
        crate::testing::arrive_on(&mut app, 1);
        let (armories, stores) = crate::testing::items_at_marks(&mut app);
        assert!(armories.iter().all(|n| *n == 1));
        assert!(stores.iter().all(|n| *n == 2));
    }
}
```

`combat_rolls_across_a_kill` runs the same fight twice, once with the dying monster's drops set to 100% and once with none, and returns the sequence of damage amounts after the kill; identical sequences mean loot did not touch the combat stream.

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p foundry a_ten_percent_drop`

- [ ] **Step 3: Implement**

`roll_drops`: one roll of `0..100` per entry, in order, keeping those under their percentage. `drop_on_death`: on each `DeathEvent { entity, at, .. }`, look up the dead actor's `Kind` in the `Roster`, roll its drops with `Drops`, and spawn each item at `at` on the actor's map. The engine takes a dead non-player out of the world before the game's systems run but leaves its components readable until the frame ends, which is why `Kind` can still be read here. `scatter_on_arrival`, on a deck's first arrival: one item from the `Armory` table at each `A` mark, two at each `L` (one on the mark, one on a free adjacent floor tile), and `3 + deck` more on random floor tiles, all drawn from `Seed::stream(b"foundry.scatter", deck as u64)`.

- [ ] **Step 4: Run and commit**

Run: `cargo test -p foundry`
Expected: PASS.

```bash
git add examples/foundry
git commit -m "feat(foundry): loot on the decks and in the wreckage, from its own stream"
```

---

### Task 10: The first charge, and a choice

**Files:**
- Create: `examples/foundry/src/mission.rs`, `examples/foundry/src/upgrades.rs`, `examples/foundry/assets/quests.ron`, `examples/foundry/assets/abilities.ron`
- Modify: `main.rs`, `testing.rs`
- Test: `mission.rs`, `upgrades.rs`

**Interfaces:**
- Consumes: the `R` mark from Task 4, `FactsPlugin`, `Happened`, the quest loader as `examples/corsair/src/quests.rs` uses it, `Abilities`, `Grants`, `Speed`, `RangedAttack`, `ItemEvent`, `RunOver`.
- Produces:
  - `mission::Console`, a marker on the reactor console entity; `mission::SetCharge`, an intent the player sends when adjacent to it.
  - `upgrades::Upgrade` enum, `Stims`, `Uplink`, `Servos`; `upgrades::OFFERED: [Upgrade; 3]`; `upgrades::apply(upgrade, player, world)`.
  - `upgrades::Choosing`, a resource that is `Some` while the pick is on screen.

- [ ] **Step 1: Content**

A quest's objective vocabulary is the game's own: Corsair's `on:` values (`Killed`, `EnteredCave` and the rest) are an enum in `examples/corsair/src/quests.rs`, turned into engine facts there. Foundry's has one variant for the slice, `ChargeSet(deck)`, reported as `Fact::new(facts.charge_set).about(deck as u64)`.

`assets/quests.ron`:

```ron
// Foundry's mission: what the charges ask of you.
//
// Every field:
//   name:       unique, referred to by `after`
//   title:      what the mission screen shows
//   text:       the brief, shown under the title
//   after:      optional; names of objectives that must be done before this one opens
//   objectives: a list of (text, on, need)
//     on:       what it counts: ChargeSet(deck)
//     need:     Total(n) counts matching facts
//   victory:    optional; finishing this wins the run. The first slice ends on the
//               upgrade chosen after the charge, not on the charge, so it is false here.
#![enable(implicit_some)]
[
    (
        name: "first_charge",
        title: "The first reactor",
        text: "The reactor on the third deck feeds the assembly lines above it. Set a charge on its console.",
        objectives: [(text: "Set a charge on the reactor", on: ChargeSet(3), need: Total(1))],
    ),
]
```

`assets/abilities.ron`, the one ability the first pick can grant, with the header comment `examples/corsair/assets/abilities.ron` carries, trimmed to the fields used:

```ron
// Foundry's abilities: what a commando calls on besides a weapon.
//
// Every field (the ones marked "optional" may be left out):
//   name:        unique; `stims` is granted by the first upgrade
//   description: optional; what it is, in a sentence, for the list on `a`
//   look:        optional; (glyph:, color: (r:, g:, b:))
//   aim:         Foe (default) | Ally | SelfOnly | Ground | Anyone
//   mode:        Own | Adjacent | Bolt(range:) | Ball(range:, radius:) | Beam(range:) | Cone(length:)
//   costs:       optional; Pool(stat, n) | Charge(n) | Health(n) | Item(tag, n)
//   time:        optional; hundredths of a step the turn costs (default 100)
//   cooldown:    optional; hundredths before it may be used again (default 0)
//   effects:     each (kind:, chance:, args:); kind is one the engine ships:
//                Harm, Mend, Inflict, Cleanse, Shove, Pull, Teleport
#![enable(implicit_some)]
[
    (
        name: "stims",
        description: "A shot of combat stimulants. It closes a wound, and needs twenty turns before the next.",
        look: (glyph: '+', color: (r: 140, g: 230, b: 160)),
        aim: SelfOnly,
        mode: Own,
        cooldown: 2000,
        effects: [(kind: "Mend", args: (kind: "care", roll: "2d4+4"))],
    ),
]
```

- [ ] **Step 2: Write the failing tests**

In `mission.rs`:

```rust
#[test]
fn setting_the_charge_takes_three_turns_and_completes_the_quest() {
    let mut app = crate::testing::headless(RunSeed(2));
    let player = crate::testing::beside_the_console(&mut app);
    app.world_mut().write_message(Intent::new(player, SetCharge));
    crate::testing::settle(&mut app);
    assert_eq!(crate::testing::clock(&app), 300, "a charge takes three turns to set");
    assert!(crate::testing::quest_done(&app, "first_charge"));
    assert!(app.world().resource::<crate::upgrades::Choosing>().0.is_some(), "and the choice is offered");
}
```

In `upgrades.rs`:

```rust
#[test]
fn each_upgrade_does_what_it_says() {
    // Stims: an ability to use. Uplink: a shot reaches one further, on the
    // weapon worn now and on one put on later. Servos: ten percent faster.
    let mut app = crate::testing::headless(RunSeed(2));
    let (player, blaster) = crate::testing::player_with_hand_blaster(&mut app);
    for u in OFFERED {
        crate::testing::pick(&mut app, player, u);
    }
    assert!(crate::testing::knows(&app, player, "stims"));
    assert_eq!(app.world().get::<RangedAttack>(blaster).unwrap().range, 6, "five, and one more");
    assert_eq!(app.world().get::<Speed>(player).map(|s| s.0), Some(110));
    let carbine = crate::testing::equip_new(&mut app, player, "blaster carbine");
    assert_eq!(app.world().get::<RangedAttack>(carbine).unwrap().range, 10, "nine, and one more, put on after the pick");
}

#[test]
fn taking_off_an_uplinked_weapon_takes_the_extra_reach_with_it() {
    let mut app = crate::testing::headless(RunSeed(2));
    let (player, blaster) = crate::testing::player_with_hand_blaster(&mut app);
    crate::testing::pick(&mut app, player, Upgrade::Uplink);
    crate::testing::unequip(&mut app, player, blaster);
    assert_eq!(app.world().get::<RangedAttack>(blaster).unwrap().range, 5, "back to what the file says");
}
```

`Speed`'s real shape is in `crates/rl-bevy/src/turn.rs`; use it. The `testing.rs` helpers are thin wrappers over spawning and the engine's intents.

- [ ] **Step 3: Run them and watch them fail**

Run: `cargo test -p foundry setting_the_charge`

- [ ] **Step 4: The mission**

`SetCharge` is an action registered with the engine's `add_action`, resolved in `TurnSet::Resolve`: if the actor is adjacent to a `Console` on its map, it charges 300, writes a `charge_set` fact through `Happened`, marks the console spent, and logs it; otherwise it fails at `BASE_ACTION_COST` with a log line saying there is nothing to set a charge on. Read how Corsair's quests report facts and how Delve registers an action. The console is spawned at the `R` mark on first arrival on deck 3, beside the Task 9 scatter.

When the quest completes, set `Choosing(Some(OFFERED))` and open a modal listing the three with one line each, from spec section 11. Picking one calls `apply` and then ends the run with `RunOver::won().saying(..)`, the words saying the first charge is set and the rest of the foundry is still below.

- [ ] **Step 5: The upgrades**

```rust
/// What finishing an objective may give, one of three, kept for the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Upgrade {
    /// An ability that patches you up.
    Stims,
    /// One more tile of reach on every ranged weapon worn.
    Uplink,
    /// A tenth faster at everything.
    Servos,
}

/// The three the first reactor offers, from spec section 11.
pub const OFFERED: [Upgrade; 3] = [Upgrade::Stims, Upgrade::Uplink, Upgrade::Servos];
```

`Stims` adds `stims` to the player's `Grants`. `Servos` sets `Speed` to 110, or adds ten to what it is. `Uplink` inserts an `Uplinked` marker on the player; a system in `TurnSet::React` on `ItemEvent::Equipped` adds one to the range of a worn item's `RangedAttack` and tags it `Reached`, and on `ItemEvent::Unequipped` takes it off again. Applying `Uplink` also raises every ranged weapon already worn. The same must hold for a weapon's `Stowed::Ranged` from Task 6, so a locked blaster comes back with its extra reach; handle that and add it to the tests.

- [ ] **Step 6: Run and commit**

Run: `cargo test -p foundry`
Expected: PASS.

```bash
git add examples/foundry
git commit -m "feat(foundry): the first charge, and one upgrade of three"
```

---

### Task 11: The play surface, and the record

**Files:**
- Create: `examples/foundry/src/input.rs`
- Modify: `examples/foundry/src/main.rs`
- Modify: `README.md`, `docs/OVERVIEW.md` if it lists the examples, `CHANGELOG.md`
- Test: `examples/foundry/src/main.rs`, and `examples/foundry/tests/fingerprint.rs`

**Interfaces:**
- Consumes: every earlier task.
- Produces: a game that runs with `cargo run -p foundry`, and a fingerprint tripwire.

- [ ] **Step 1: Input**

`input.rs`, modelled on `examples/corsair/src/input.rs`: movement from the engine; `f` fires at the nearest enemy in sight, as Corsair's `fire` does; `t` throws; `g` picks up; `e` sets a charge when beside a console; `i` opens the inventory; `x` inspects; `a` opens abilities. Declare each with the engine's controls declaration so the controls screen lists them.

- [ ] **Step 2: The screen**

`main.rs`, modelled on `examples/corsair/src/main.rs:52-230` but smaller: the same `Screen` cut, and `VitalsPanel`, `GearPanel`, `NearbyPanel`, `LogPanel`, `InspectPanel`, `TargetPanel`, `AbilityPanel`, `InventoryPanel`, `ControlsPanel`, `GameMenuPanel::new(..).title("Foundry").died("The foundry keeps you.").won("The first charge is set.")`, and `NarratorPlugin::default()`. Arguments: `--seed N` only. Each deck's ambient light is set on arrival as Delve's `light_the_floor` does: deck 1 dim, decks 2 and 3 dark, so radar matters.

`main.rs` must still be only wiring and the run's start, under 400 lines. Anything else found there goes into its own module.

- [ ] **Step 3: A fingerprint tripwire**

`examples/foundry/tests/fingerprint.rs`, following `crates/rl-bevy/tests/fingerprint.rs`: a headless run on a fixed seed, the player driven by a fixed script of moves and shots for 200 turns, and a hash of every actor's position and health and every weapon's heat at the end. The test is labelled as a tripwire in its name and its doc comment: it pins behaviour that has no property to state, and a change that moves it is re-baselined on purpose with a `CHANGELOG.md` line.

- [ ] **Step 4: Play it**

Run: `cargo run -p foundry -- --seed 7`, and play through: find a weapon, fight a line droid and watch it shoot, fire a blaster until it locks and watch it cool, run a slugthrower dry, walk into a probe's radar in the dark, reach deck 3, set the charge, and pick an upgrade. Look at every screen as you go. Anything that looks off gets fixed now, whether or not this plan named it. Record what you saw in the task report.

- [ ] **Step 5: The record**

`README.md`: Foundry beside the other examples, one line saying what it demonstrates. `docs/OVERVIEW.md`, if it lists examples: the same. `CHANGELOG.md`, Unreleased: Foundry's first slice.

- [ ] **Step 6: Verify and commit**

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && scripts/check-tiers.sh && scripts/check-tiers.sh --wasm && scripts/check-guide.sh
wc -l examples/foundry/src/*.rs   # none over 400
git add examples/foundry README.md docs CHANGELOG.md
git commit -m "feat(foundry): the first slice, playable"
```

---

## What this plan does not do

- Decks 4 to 10, the other twelve weapons, the other fourteen armor pieces, shield droids, guard droids, infiltrators, hunters, mini-bosses and the overseer. Spec section 13 lists them as absent from the slice.
- Composed encounters and the private asset pack; see "Decisions" above.
- Keeping at range: `ShootAtRange` shoots when there is a shot but does not back off to keep one.
- The second and third reactors and their upgrade picks.
