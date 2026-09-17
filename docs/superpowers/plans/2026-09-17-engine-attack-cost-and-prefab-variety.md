# Engine: attack cost and prefab variety - implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the engine per-weapon attack cost and prefab orientation and weighted choice, the two engine gaps the Foundry design found.

**Architecture:** Two independent changes in two crates. `MeleeAttack` and `RangedAttack` gain an optional cost that `resolve_attacks` charges instead of `BASE_ACTION_COST`, which is what lets a weapon be fast or slow. `Prefab` gains rotation and flip, both carrying marks with the tiles, and a new `StampOneOf` pass picks among weighted candidates, which is what lets one authored vault appear eight ways.

**Tech Stack:** Rust, Bevy 0.17 at tier 2, `rand` through the engine's seeded streams, criterion for benches, `cargo test --workspace`.

**Spec:** `docs/superpowers/specs/2026-09-17-foundry-design.md`, sections 10.2 and 10.3.

## Global Constraints

These come from `CLAUDE.md` and apply to every task below.

- `cargo fmt --all --check` passes; `rustfmt.toml` pins the width, so never hand-wrap.
- `cargo clippy --workspace --all-targets -- -D warnings` passes, with no crate-wide allow added.
- `cargo test --workspace` passes, doc-tests included; never fence an example as `ignore`.
- `#![deny(missing_docs)]` holds: every public item gets a doc comment saying why and why-not, at the density of `crates/rl-core/src/turn.rs`.
- `scripts/check-tiers.sh` and `scripts/check-tiers.sh --wasm` pass. `rl-mapgen` is tier 1 and must stay Bevy-free and wasm-clean; no `std::time::Instant`.
- No theme words in engine crates: no fantasy, sci-fi or pirate vocabulary in types, docs or constants.
- No `HashMap` or `HashSet` in gameplay or generation paths; `BTreeMap`, `Vec` or a `BitGrid`, with the reason stated.
- Costs and clocks are integers in hundredths of a step. `BASE_ACTION_COST` is 100.
- Randomness comes through the caller's `&mut impl Rng` or a pass's `ctx.rng()`. Never construct a generator from a constant or from entropy.
- No `TODO` comments in source.
- Plain dash, never an em dash. American spelling in identifiers.
- Tests are properties over a seed range where a property exists, fingerprint tripwires labelled as such where none does, and named as a sentence describing the property.
- `docs/OVERVIEW.md` changes in the same commit as the system it gains or loses.

---

## File Structure

| File | Responsibility | Change |
|---|---|---|
| `crates/rl-bevy/src/combat.rs` | Attack messages and `resolve_attacks` | Modify: `cost` field on two structs, honoured by the resolver |
| `crates/rl-mapgen/src/prefab.rs` | Prefab parsing, transformation and stamping | Modify: `rotated`, `flipped`, `Orient`, `StampOneOf` |
| `crates/rl-mapgen/src/lib.rs` | Crate exports and module docs | Modify: export the new items |
| `docs/OVERVIEW.md` | The inventory of what the engine has | Modify: both entries |
| `docs/TODO.md` | Outstanding work | Modify: strike the attack-cost item |
| `CHANGELOG.md` | Release notes | Modify: record the breaking change |

Roughly 57 sites across `crates/` construct `MeleeAttack { .. }` or `RangedAttack { .. }` as literals, nearly all in tests. Task 1 updates every one of them with `cost: None`. This is deliberate: the alternative, a second component holding the cost, splits one weapon's data across two places and is a thing every game can forget to add.

---

### Task 1: Melee attacks carry their cost

**Files:**
- Modify: `crates/rl-bevy/src/combat.rs:73-78` (the `MeleeAttack` struct), `crates/rl-bevy/src/combat.rs:374-408` (`resolve_attacks`)
- Modify: every literal construction of `MeleeAttack` across `crates/`, adding `cost: None`
- Test: `crates/rl-bevy/src/combat.rs`, in its existing `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `rl_core::turn::BASE_ACTION_COST: u32`, `Resolution::done(Entity, u32)`
- Produces: `MeleeAttack { kind: DamageKindId, dice: DiceRoll, cost: Option<u32> }`, where `None` means `BASE_ACTION_COST`

- [ ] **Step 1: Write the failing test**

Add to the tests module in `crates/rl-bevy/src/combat.rs`:

```rust
#[test]
fn a_blow_costs_what_its_weapon_says_and_an_ordinary_turn_when_it_says_nothing() {
    // Identical blows but for what the weapon charges: 70 hundredths of a
    // step against 140, and nothing stated. A weapon that charges half as
    // much comes round twice as often, which is the whole of weapon speed.
    let quick = attack_cost_over_time(Some(70));
    let heavy = attack_cost_over_time(Some(140));
    let plain = attack_cost_over_time(None);
    assert_eq!(quick, 70, "the weapon's cost is what the turn charges");
    assert_eq!(heavy, 140);
    assert_eq!(plain, BASE_ACTION_COST, "no cost stated is the ordinary cost");
}
```

And the helper it needs, beside it:

```rust
/// Charges one melee blow with `cost` and answers what the turn cost.
fn attack_cost_over_time(cost: Option<u32>) -> u32 {
    use crate::testing::TestApp;
    let mut app = TestApp::new();
    let kind = app.damage_kind("impact");
    let attacker = app.spawn_actor(Point::new(1, 1));
    let target = app.spawn_actor(Point::new(2, 1));
    app.world_mut().entity_mut(attacker).insert(MeleeAttack { kind, dice: DiceRoll::flat(1), cost });
    app.attack(attacker, target);
    app.spent(attacker)
}
```

If `TestApp` does not expose `damage_kind`, `spawn_actor`, `attack` or `spent`, read `crates/rl-bevy/src/testing.rs` and use whatever it does expose; the test's shape matters, not these names.

- [ ] **Step 2: Run the test and watch it fail**

Run: `cargo test -p rl-bevy a_blow_costs_what_its_weapon_says`
Expected: a compile error, `struct MeleeAttack has no field named cost`.

- [ ] **Step 3: Add the field**

In `crates/rl-bevy/src/combat.rs`, on `MeleeAttack`:

```rust
    /// What one blow with it costs, in hundredths of a step. `None` is
    /// [`BASE_ACTION_COST`], so a game that does not care about weapon
    /// speed writes nothing and every blow costs a turn.
    pub cost: Option<u32>,
```

- [ ] **Step 4: Charge it in the resolver**

In `resolve_attacks`, the melee branch currently discards everything but kind and dice. Keep the cost:

```rust
        let weapon = if geometry::is_adjacent(pos.0, target_pos.0) {
            loadout.melee(intent.actor).map(|m| (m.kind, m.dice, m.cost))
        } else {
            loadout.ranged(intent.actor).filter(|r| line_of_fire(&map, &occupancy, pos.0, target_pos.0, r.range)).map(|r| (r.kind, r.dice, None))
        };
        let mut spent = rl_core::turn::BASE_ACTION_COST;
        if let Some((kind, dice, cost)) = weapon {
            spent = cost.unwrap_or(rl_core::turn::BASE_ACTION_COST);
            // Floored where it is rolled: a blow that rolls below zero has
            // missed, and the pipeline would read a negative one as a heal.
            let amount = dice.roll_at_least(&mut **rng, 0);
            damage.write(DamageEvent { target, hit: Hit::by(intent.actor, kind, amount) });
            for (kind, dice) in loadout.strikes(intent.actor) {
                let amount = dice.roll_at_least(&mut **rng, 0);
                damage.write(DamageEvent { target, hit: Hit::by(intent.actor, kind, amount) });
            }
        }
        resolution.done(intent.actor, spent);
```

Note the ranged arm passes `None` for now; Task 2 gives it a real cost. A swing at nothing in reach still costs `BASE_ACTION_COST`, which is the behaviour the function's doc comment already promises.

Also extend `Loadout::melee` so the cost survives the stat fold, which rebuilds the struct:

```rust
        Some(MeleeAttack { dice: DiceRoll { bonus: base.dice.bonus + bonus, ..base.dice }, ..base })
```

This already carries `cost` through `..base`, so it needs no edit. Confirm by reading it rather than assuming.

- [ ] **Step 5: Fix every construction site**

Run: `cargo build --workspace --all-targets 2>&1 | grep -c "missing field"`
Add `cost: None` to each `MeleeAttack { .. }` literal the compiler names. They are nearly all in tests, in `crates/rl-bevy/`, `crates/rl-ui/` and `crates/rl-bevy/tests/fingerprint.rs`.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p rl-bevy`
Expected: PASS, the new test included, with the fingerprint test unchanged, because `cost: None` charges exactly what the code charged before.

- [ ] **Step 7: Commit**

```bash
git add crates/rl-bevy crates/rl-ui
git commit -m "feat(combat): a melee weapon charges its own cost"
```

---

### Task 2: Ranged attacks carry their cost

**Files:**
- Modify: `crates/rl-bevy/src/combat.rs:85-92` (the `RangedAttack` struct), `crates/rl-bevy/src/combat.rs` (`resolve_attacks`, the ranged arm)
- Modify: every literal construction of `RangedAttack` across `crates/`, adding `cost: None`
- Test: `crates/rl-bevy/src/combat.rs`

**Interfaces:**
- Consumes: `MeleeAttack.cost` from Task 1, and the same `Resolution::done`
- Produces: `RangedAttack { kind: DamageKindId, dice: DiceRoll, range: i32, cost: Option<u32> }`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn a_shot_charges_the_weapons_cost_and_a_shot_at_nothing_still_costs_a_turn() {
    // A marksman rifle is slow, a hand blaster fast, and a shot with no
    // line of fire costs the ordinary turn: the shooter spent it aiming.
    let slow = shot_cost(Some(140), 14, Point::new(6, 1));
    let fast = shot_cost(Some(80), 5, Point::new(3, 1));
    let missed = shot_cost(Some(140), 1, Point::new(9, 1));
    assert_eq!(slow, 140);
    assert_eq!(fast, 80);
    assert_eq!(missed, BASE_ACTION_COST, "out of range is a spent turn, not a free one");
}

/// Fires one shot with `cost` and `range` at a target at `at`, and
/// answers what the turn cost.
fn shot_cost(cost: Option<u32>, range: i32, at: Point) -> u32 {
    use crate::testing::TestApp;
    let mut app = TestApp::new();
    let kind = app.damage_kind("energy");
    let attacker = app.spawn_actor(Point::new(1, 1));
    let target = app.spawn_actor(at);
    app.world_mut().entity_mut(attacker).insert(RangedAttack { kind, dice: DiceRoll::flat(1), range, cost });
    app.attack(attacker, target);
    app.spent(attacker)
}
```

- [ ] **Step 2: Run the test and watch it fail**

Run: `cargo test -p rl-bevy a_shot_charges_the_weapons_cost`
Expected: a compile error, `struct RangedAttack has no field named cost`.

- [ ] **Step 3: Add the field**

```rust
    /// What one shot costs, in hundredths of a step. `None` is
    /// [`BASE_ACTION_COST`]. A shot that finds nothing in reach costs the
    /// ordinary turn rather than this, since what was spent was the aim.
    pub cost: Option<u32>,
```

- [ ] **Step 4: Charge it in the resolver**

Replace the `None` Task 1 left in the ranged arm:

```rust
            loadout.ranged(intent.actor).filter(|r| line_of_fire(&map, &occupancy, pos.0, target_pos.0, r.range)).map(|r| (r.kind, r.dice, r.cost))
```

- [ ] **Step 5: Fix every construction site**

Run: `cargo build --workspace --all-targets 2>&1 | grep "missing field"`
Add `cost: None` to each `RangedAttack { .. }` literal named.

- [ ] **Step 6: Run the whole workspace**

Run: `cargo test --workspace`
Expected: PASS. The fingerprint tripwire must not move: every existing weapon states no cost and so charges what it charged before. If it does move, stop and find out why rather than re-baselining.

- [ ] **Step 7: Commit**

```bash
git add crates
git commit -m "feat(combat): a shot charges its own cost"
```

---

### Task 3: A prefab turns and mirrors, and its marks go with it

**Files:**
- Modify: `crates/rl-mapgen/src/prefab.rs` (the `Prefab` impl)
- Test: `crates/rl-mapgen/src/prefab.rs`, in its existing `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `Prefab::parse`, `Prefab::tile`, `Prefab::marks`, `rl_core::{Grid, Grid2D, Point}`
- Produces: `Prefab::rotated(&self, quarters: u8) -> Prefab` and `Prefab::flipped(&self) -> Prefab`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn a_quarter_turn_moves_every_tile_and_its_marks_the_same_way() {
    let p = vault(TileId(0), TileId(1));
    let (w, h) = (p.width(), p.height());
    let turned = p.rotated(1);
    assert_eq!((turned.width(), turned.height()), (h, w), "a quarter turn swaps the sides");

    // The mark is the anchor: wherever it was, it is now at the point a
    // clockwise turn sends it to, and the tile under it is still nothing.
    let (_, before) = p.marks()[0];
    let (_, after) = turned.marks()[0];
    assert_eq!(after, Point::new(h - 1 - before.y, before.x));
    assert_eq!(turned.tile(after), None);

    // Four turns is where it started, which is the property that catches
    // an off-by-one in the transform.
    assert_eq!(p.rotated(4), p);
    assert_eq!(p.rotated(1).rotated(3), p);
}

#[test]
fn a_mirror_moves_every_tile_and_its_marks_the_same_way() {
    let p = vault(TileId(0), TileId(1));
    let w = p.width();
    let flipped = p.flipped();
    assert_eq!((flipped.width(), flipped.height()), (w, p.height()), "a mirror keeps the shape");
    let (_, before) = p.marks()[0];
    let (_, after) = flipped.marks()[0];
    assert_eq!(after, Point::new(w - 1 - before.x, before.y));
    assert_eq!(p.flipped().flipped(), p, "twice mirrored is where it started");
}
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p rl-mapgen quarter_turn`
Expected: FAIL, `no method named rotated found for struct Prefab`.

- [ ] **Step 3: Implement the transforms**

In `impl Prefab`, after `marks()`:

```rust
    /// A copy turned a quarter-turn clockwise `quarters` times, marks and
    /// all. Four is the piece as it was, so a caller may pass any number.
    ///
    /// Marks turn with the tiles because a mark is a position in the
    /// piece, not on the map: a vault's chest stays in its alcove however
    /// the vault is laid down.
    pub fn rotated(&self, quarters: u8) -> Self {
        let mut out = self.clone();
        for _ in 0..(quarters % 4) {
            out = out.turned();
        }
        out
    }

    /// A copy mirrored left to right, marks and all.
    pub fn flipped(&self) -> Self {
        let (w, h) = (self.width(), self.height());
        let mut cells: Grid<Option<TileId>> = Grid::new(w, h);
        for y in 0..h {
            for x in 0..w {
                cells.set(Point::new(w - 1 - x, y), self.tile(Point::new(x, y)));
            }
        }
        let marks = self.marks.iter().map(|(c, p)| (*c, Point::new(w - 1 - p.x, p.y))).collect();
        Self { cells, marks }
    }

    /// One quarter-turn clockwise.
    fn turned(&self) -> Self {
        let (w, h) = (self.width(), self.height());
        let mut cells: Grid<Option<TileId>> = Grid::new(h, w);
        for y in 0..h {
            for x in 0..w {
                cells.set(Point::new(h - 1 - y, x), self.tile(Point::new(x, y)));
            }
        }
        let marks = self.marks.iter().map(|(c, p)| (*c, Point::new(h - 1 - p.y, p.x))).collect();
        Self { cells, marks }
    }
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rl-mapgen`
Expected: PASS, the existing prefab tests included.

- [ ] **Step 5: Commit**

```bash
git add crates/rl-mapgen/src/prefab.rs
git commit -m "feat(mapgen): a prefab turns and mirrors, and its marks go with it"
```

---

### Task 4: A stamp may take any orientation

**Files:**
- Modify: `crates/rl-mapgen/src/prefab.rs` (`StampPrefab`, and a new `Orient`)
- Modify: `crates/rl-mapgen/src/lib.rs:35` (the export line)
- Modify: `examples/corsair/src/places.rs:124`, `examples/delve/src/floors.rs:196`
- Test: `crates/rl-mapgen/src/prefab.rs`

**Interfaces:**
- Consumes: `Prefab::rotated`, `Prefab::flipped` from Task 3, `BuildContext::rng`
- Produces: `Orient::{Fixed, Turned, TurnedOrMirrored}` and `StampPrefab { name, prefab, at, orient }`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn an_oriented_stamp_is_the_same_piece_under_one_seed_and_varies_across_seeds() {
    // Determinism first: the same seed lays the same piece down, or a
    // saved run would reload a different map than it saved.
    let first = stamped_marks(RunSeed(7), Orient::TurnedOrMirrored);
    assert_eq!(first, stamped_marks(RunSeed(7), Orient::TurnedOrMirrored), "one seed, one map");

    // Then variety: over a span of seeds an oriented stamp must land its
    // mark in more than one place, or the orientation did nothing.
    let seen: std::collections::BTreeSet<_> = (0..40).map(|s| stamped_marks(RunSeed(s), Orient::TurnedOrMirrored)).collect();
    assert!(seen.len() > 1, "forty seeds laid the piece exactly one way");

    // And a fixed stamp faces one way whatever the seed, so every chain
    // that has one today keeps the map it has today.
    let fixed: std::collections::BTreeSet<_> = (0..40).map(|s| stamped_marks(RunSeed(s), Orient::Fixed)).collect();
    assert_eq!(fixed.len(), 1, "a fixed stamp faces the same way under every seed");
}

/// Stamps the vault into a fixed room under `seed` and answers its marks
/// relative to the stamp's own bounds, which is its facing.
fn stamped_marks(seed: RunSeed, orient: Orient) -> Vec<(char, Point)> {
    let tiles = TileRegistry::standard();
    let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
    let mut c = BaseContext::blank(60, 40, tiles, wall);
    Chain::new()
        .then(Rooms { floor, min_size: 8, max_size: 10, ..Default::default() })
        .then(StampPrefab { name: "vault", prefab: vault(wall, floor), at: Placement::InRoom(0), orient })
        .run(&mut c, seed)
        .unwrap();
    let stamped = c.outputs().first::<Stamped>().unwrap();
    let origin = stamped.bounds.origin();
    stamped.marks.iter().map(|(ch, p)| (*ch, Point::new(p.x - origin.x, p.y - origin.y))).collect()
}
```

- [ ] **Step 2: Run the test and watch it fail**

Run: `cargo test -p rl-mapgen an_oriented_stamp`
Expected: a compile error, `cannot find type Orient in this scope`.

- [ ] **Step 3: Add the orientation policy**

In `crates/rl-mapgen/src/prefab.rs`, beside `Placement`:

```rust
/// How a piece may be turned before it is laid down.
///
/// Authored per stamp rather than per prefab, since the same vault may be
/// free to turn in a cave and fixed against a corridor that has to meet
/// its door.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Orient {
    /// Exactly as it was drawn.
    #[default]
    Fixed,
    /// One of the four quarter-turns, drawn from the pass's stream.
    Turned,
    /// One of the four quarter-turns, and mirrored or not: eight facings.
    TurnedOrMirrored,
}

impl Orient {
    /// The piece as this policy leaves it, drawing from `rng` only when
    /// there is a choice to make, so a fixed stamp advances no stream and
    /// a chain that adds one does not move every map after it.
    pub fn apply(self, prefab: &Prefab, rng: &mut impl Rng) -> Prefab {
        match self {
            Orient::Fixed => prefab.clone(),
            Orient::Turned => prefab.rotated(rng.random_range(0..4)),
            Orient::TurnedOrMirrored => {
                let turned = prefab.rotated(rng.random_range(0..4));
                if rng.random_bool(0.5) { turned.flipped() } else { turned }
            }
        }
    }
}
```

- [ ] **Step 4: Use it in the stamp pass**

Add the field to `StampPrefab`:

```rust
    /// How it may be turned before it lands.
    pub orient: Orient,
```

And in `apply`, before the placement is computed, since a turned piece has different sides and the fit check must see them:

```rust
        let prefab = self.orient.apply(&self.prefab, ctx.rng());
        let (w, h) = (prefab.width(), prefab.height());
```

Then replace the two later uses of `self.prefab` with `prefab`.

- [ ] **Step 5: Export it and fix the two games**

In `crates/rl-mapgen/src/lib.rs:35`:

```rust
    pub use crate::prefab::{Orient, Placement, Prefab, StampPrefab, Stamped};
```

In `examples/corsair/src/places.rs:124` and `examples/delve/src/floors.rs:196`, add `orient: Orient::Fixed` to the `StampPrefab { .. }` literal and add `Orient` to each file's `use`. Both games keep exactly the maps they have today.

- [ ] **Step 6: Run the tests**

Run: `cargo test --workspace`
Expected: PASS. Corsair's and Delve's map fingerprints must not move, because `Orient::Fixed` draws nothing from the stream.

- [ ] **Step 7: Commit**

```bash
git add crates/rl-mapgen examples/corsair examples/delve
git commit -m "feat(mapgen): a stamp may take any of eight facings"
```

---

### Task 5: One stamp, many candidates

**Files:**
- Modify: `crates/rl-mapgen/src/prefab.rs` (a new `StampOneOf` pass)
- Modify: `crates/rl-mapgen/src/lib.rs:35` (the export line)
- Test: `crates/rl-mapgen/src/prefab.rs`

**Interfaces:**
- Consumes: `Orient::apply` from Task 4, `Placement`, `Stamped`, `Pass`, `Phase::Structures`, `BuildError`
- Produces: `StampOneOf { name: &'static str, choices: Vec<(Prefab, u32)>, at: Placement, orient: Orient }`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn a_weighted_stamp_picks_every_candidate_that_carries_weight_and_never_one_that_does_not() {
    // Three candidates, each with its own mark so the stamped map says
    // which was chosen, and the third weightless.
    let wall = TileRegistry::standard().expect("wall");
    let piece = |mark: char| {
        let middle = format!("#{mark}#");
        Prefab::parse(&["###", &middle, "###"], |c| match c {
            '#' => Some(wall),
            _ => None,
        })
        .unwrap()
    };
    let mut picked = std::collections::BTreeSet::new();
    for seed in 0..60 {
        let tiles = TileRegistry::standard();
        let floor = tiles.expect("floor");
        let mut c = BaseContext::blank(40, 30, tiles, wall);
        Chain::new()
            .then(Rooms { floor, min_size: 8, max_size: 10, ..Default::default() })
            .then(StampOneOf {
                name: "vault",
                choices: vec![(piece('a'), 3), (piece('b'), 1), (piece('c'), 0)],
                at: Placement::InRoom(0),
                orient: Orient::Fixed,
            })
            .run(&mut c, RunSeed(seed))
            .unwrap();
        picked.insert(c.outputs().first::<Stamped>().unwrap().marks[0].0);
    }
    assert!(picked.contains(&'a') && picked.contains(&'b'), "both weighted candidates must come up over sixty seeds");
    assert!(!picked.contains(&'c'), "a weightless candidate is never chosen");
}

#[test]
fn a_weighted_stamp_with_nothing_to_choose_from_fails_the_chain() {
    let tiles = TileRegistry::standard();
    let wall = tiles.expect("wall");
    let mut c = BaseContext::blank(40, 30, tiles, wall);
    let err = Chain::new()
        .then(StampOneOf { name: "vault", choices: Vec::new(), at: Placement::Center, orient: Orient::Fixed })
        .run(&mut c, RunSeed(1))
        .unwrap_err();
    // Loudly, at generation time: a silent skip would leave a map missing
    // the thing the chain said it must have.
    assert!(format!("{err:?}").contains("vault"));
}
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo test -p rl-mapgen a_weighted_stamp`
Expected: a compile error, `cannot find struct StampOneOf in this scope`.

- [ ] **Step 3: Implement the pass**

```rust
/// Stamps one of several pieces, chosen by weight.
///
/// One entry per piece with the weight it is drawn at; a zero weight is
/// never drawn, which is how a game keeps a piece in the list while it is
/// being worked on. Fails if nothing carries weight, since a chain that
/// asked for a vault and got none has generated a map its game does not
/// expect.
#[derive(Debug, Clone)]
pub struct StampOneOf {
    /// A stable name, so two stamps in one chain draw different streams.
    pub name: &'static str,
    /// The pieces and their weights.
    pub choices: Vec<(Prefab, u32)>,
    /// Where the chosen piece goes.
    pub at: Placement,
    /// How it may be turned before it lands.
    pub orient: Orient,
}

impl<C: BuildContext> Pass<C> for StampOneOf {
    fn name(&self) -> &'static str {
        self.name
    }
    fn phase(&self) -> Phase {
        Phase::Structures
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        let total: u32 = self.choices.iter().map(|(_, w)| w).sum();
        if total == 0 {
            return Err(BuildError::new(self.name, "no candidate carries weight".to_string()));
        }
        let mut roll = ctx.rng().random_range(0..total);
        let chosen = self
            .choices
            .iter()
            .find(|(_, w)| {
                if roll < *w {
                    true
                } else {
                    roll -= w;
                    false
                }
            })
            .map(|(p, _)| p.clone())
            .expect("the roll is below the total, so some candidate holds it");
        StampPrefab { name: self.name, prefab: chosen, at: self.at, orient: self.orient }.apply(ctx)
    }
}
```

- [ ] **Step 4: Export it**

In `crates/rl-mapgen/src/lib.rs:35`:

```rust
    pub use crate::prefab::{Orient, Placement, Prefab, StampOneOf, StampPrefab, Stamped};
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p rl-mapgen && cargo clippy -p rl-mapgen --all-targets -- -D warnings`
Expected: PASS and no warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/rl-mapgen
git commit -m "feat(mapgen): one stamp, many candidates, chosen by weight"
```

---

### Task 6: The documents say so

**Files:**
- Modify: `docs/OVERVIEW.md` (the `rl-mapgen` section and the `rl-bevy` combat entry)
- Modify: `docs/TODO.md` (section 3, the attack-cost item)
- Modify: `CHANGELOG.md` (the Unreleased section)
- Modify: `docs/PLAN.md` (the progress log)

**Interfaces:**
- Consumes: everything Tasks 1 to 5 built
- Produces: nothing code depends on

- [ ] **Step 1: Update the inventory**

In `docs/OVERVIEW.md`, in the `rl-mapgen` bullet list, replace the prefab line with:

```markdown
- Prefab stamping from ASCII with a legend and marks, placed at a point, centred, or in a room, in any of eight facings, and one stamp may choose among weighted candidates. A turned piece takes its marks with it.
```

And in the combat entry of the `rl-bevy` section, add:

```markdown
A weapon states what one blow with it costs, in hundredths of a step; stating nothing costs an ordinary turn.
```

- [ ] **Step 2: Strike the TODO item**

In `docs/TODO.md` section 3, remove the "Attack cost, and a place for a miss" bullet's first half, leaving the accuracy half, which is untouched by this work:

```markdown
- **A place for a miss.**
  Accuracy is deliberately absent (`docs/design/abilities.md`, "Accuracy does not exist"); the combat docs should say how a game adds a miss as a `DamageStage`, with an example.
```

- [ ] **Step 3: Record the breaking change**

In `CHANGELOG.md`, under Unreleased:

```markdown
### Changed

- `MeleeAttack` and `RangedAttack` carry a `cost`, so a weapon can be fast or slow. Breaking: both structs gained a field, and `cost: None` charges exactly what a blow charged before.

### Added

- Prefabs turn and mirror, marks included, and `StampOneOf` chooses among weighted candidates.
```

- [ ] **Step 4: Add the progress entry**

In `docs/PLAN.md`, at the top of the progress log, a dated entry saying what landed and why: weapon speed because a game asked for it, prefab facings because one authored vault laid eight ways is most of what map variety means.

- [ ] **Step 5: Run everything the build enforces**

Run:

```bash
cargo fmt --all --check && \
cargo clippy --workspace --all-targets -- -D warnings && \
cargo test --workspace && \
scripts/check-tiers.sh && scripts/check-tiers.sh --wasm
```

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add docs CHANGELOG.md
git commit -m "docs: weapon speed and prefab facings, on the record"
```

---

## What this plan does not do

- No game content. `examples/foundry` is the next plan, and it consumes both features.
- No senses model, no `NoticeStats` change: section 10.1 of the spec explains why radar needs no engine work.
- No per-deck prefab budget or mark-slot content resolution. Both are game-side, and both belong in the Foundry plan.
