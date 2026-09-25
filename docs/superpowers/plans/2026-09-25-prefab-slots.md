# Prefab Slots Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A prefab is a RON file whose legend maps each glyph to a tile or to one slot (a prop, an item by name or tag, a monster by name or role, or a mark left to the game), and the engine fills every slot on the first entry to a place, with guards that hold their post.

**Architecture:** `rl-mapgen` learns to paint ground under a mark and to carry an opaque prefab key through a stamp. `rl-rules` owns the file formats (`prefab`, `role`), the filtered banded draw, the coverage report and the `KeepPost` tactic, all testable without an `App`. `rl-bevy` owns the loop: `ActorMaker` beside `ItemMaker`, a `Prefabs` resource, `PrefabPlugin<A, I>` filling slots in `PrefabSet::Fill`, and a saved `Post` component. Foundry adopts all of it.

**Tech Stack:** Rust, Bevy (tier 2 only), serde + RON, `rand`.

**Spec:** `docs/superpowers/specs/2026-09-24-prefab-slots-design.md`

## Global Constraints

- Tier 0 and 1 crates (`rl-core`, `rl-grid`, `rl-mapgen`, `rl-rules`) never depend on Bevy; `rl-rules` gains no dependency on `rl-mapgen`.
- `#![deny(missing_docs)]`: every public item gets a doc comment that says why, at the density of `crates/rl-core/src/turn.rs`.
- No `HashMap`/`HashSet` in gameplay or generation paths: `BTreeMap`, `Vec`.
- Randomness only through `RunSeed`/`Seed` streams; functions take `&mut impl Rng`. Never a generator from a constant or entropy in engine code.
- No theme words in engine crates (no "vault", "treasure", "guard" in type names; "post" and "role" are fine).
- No `TODO` comments in source.
- Prose: plain dash, never an em dash; American spelling in identifiers; one sentence per line in Markdown docs.
- Every RON schema carries a top-of-file comment listing the full option space.
- Test names read as sentences describing the property; property-over-seed-range where a property exists.
- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass at every commit.
- `scripts/check-tiers.sh` and `scripts/check-tiers.sh --wasm` pass.
- A task that edits a file named in any `docs/guide/src/systems/*.md` manifest re-reads that page, corrects any sentence the change made untrue, and runs `python3 scripts/check-systems.py --bless <system>` for it; `python3 scripts/check-systems.py` passes at the end of every task except 7 and 8 (see Task 9).
- A task that changes what a game writes adds its `CHANGELOG.md` line under `Unreleased` in the same commit.
- Commit messages carry no agent co-author or attribution line (the user's standing rule), and are in the repo's style: `area: what changed, in words`.

## Review Focus

1. A later mapgen pass overwrites a slot's cell with a wall: the slot spawns nothing there, never a monster inside a wall. Pinned in Task 7 (`a_slot_whose_cell_is_no_longer_walkable_spawns_nothing`).
2. A slot asks past the deepest row of a table (deck 10 with `+2`): it draws from the deepest band that has a member, never nothing. Pinned in Task 3 (`a_role_asked_past_the_deepest_row_draws_from_the_deepest_band`).
3. Two prefabs use the same glyph for different slots: each stamp's marks resolve through its own prefab, never the other's. Pinned in Task 7 (`two_prefabs_sharing_a_glyph_fill_it_each_their_own_way`).
4. A guard's post is taken by another actor: the guard waits beside it rather than oscillating or panicking. Pinned in Task 6 (`a_guard_whose_post_is_taken_waits_rather_than_wandering`).
5. A continued run: a deck saved and loaded does not fill its prefabs a second time, and its guards keep their posts. Pinned in Task 8 (`a_continued_run_neither_refills_a_deck_nor_forgets_a_guards_post`).

---

### Task 1: Marks with ground, and a prefab key carried through the stamp

**Files:**
- Modify: `crates/rl-mapgen/src/prefab.rs`
- Modify: `crates/rl-bevy/src/places.rs:81-116, 354, 413`
- Modify: `crates/rl-bevy/src/loot.rs:533`
- Modify: `examples/corsair/src/places.rs:140`
- Modify: `docs/guide/src/systems/mapgen.md`, `docs/guide/src/systems/places.md` (if their sentences about marks or spots change), `CHANGELOG.md`

**Interfaces:**
- Produces:
  - `rl_mapgen::prefab::Cell { Tile(TileId), Mark(Option<TileId>), Clear }`
  - `Prefab::parse_cells(rows: &[&str], legend: impl Fn(char) -> Cell) -> Result<Prefab, String>`
  - `Prefab::keyed(self, key: u32) -> Prefab`, `Prefab::key(&self) -> Option<u32>`
  - `Stamped { bounds: Rect, marks: Vec<(char, Point)>, prefab: Option<u32> }`
  - `rl_bevy::places::Spot { tag: u32, at: Point, prefab: Option<u32> }` with `#[serde(default)]` on `prefab`

- [ ] **Step 1: Write the failing tests** in `crates/rl-mapgen/src/prefab.rs`'s `mod tests`:

```rust
#[test]
fn a_mark_with_ground_paints_the_ground_and_is_still_a_mark() {
    let r = TileRegistry::standard();
    let (wall, floor) = (r.expect("wall"), r.expect("floor"));
    let p = Prefab::parse_cells(&["#s#"], |c| match c {
        '#' => Cell::Tile(wall),
        's' => Cell::Mark(Some(floor)),
        _ => Cell::Clear,
    })
    .unwrap();
    assert_eq!(p.tile(Point::new(1, 0)), Some(floor), "the mark's cell is painted");
    assert_eq!(p.marks(), &[('s', Point::new(1, 0))], "and still reported as a mark");
}

#[test]
fn a_key_survives_every_facing_and_reaches_the_stamp() {
    let r = TileRegistry::standard();
    let wall = r.expect("wall");
    let piece = Prefab::parse(&["#.", "A#"], |c| (c == '#').then_some(wall)).unwrap().keyed(7);
    for quarters in 0..4 {
        assert_eq!(piece.rotated(quarters).key(), Some(7));
        assert_eq!(piece.rotated(quarters).flipped().key(), Some(7));
    }
    let mut ctx = BaseContext::blank(10, 10, r.clone(), r.expect("floor"));
    Chain::new().then(StampPrefab { name: "keyed", prefab: piece, at: Placement::At(Point::new(2, 2)), orient: Orient::TurnedOrMirrored }).run(&mut ctx, RunSeed(1)).unwrap();
    let stamped = ctx.outputs().first::<Stamped>().unwrap();
    assert_eq!(stamped.prefab, Some(7));
}

#[test]
fn a_piece_parsed_from_a_tile_legend_has_no_key() {
    let r = TileRegistry::standard();
    let wall = r.expect("wall");
    assert_eq!(Prefab::parse(&["#A#"], |c| (c == '#').then_some(wall)).unwrap().key(), None);
}
```

Check the imports the existing tests in this module use (`BaseContext`, `Chain`, `RunSeed`, `TileRegistry`) and add any missing ones to the test module.

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p rl-mapgen prefab::tests -- --nocapture`
Expected: compile errors, `Cell`, `parse_cells`, `keyed`, `key` and `Stamped::prefab` do not exist.

- [ ] **Step 3: Implement.** In `crates/rl-mapgen/src/prefab.rs`:

```rust
/// What one character of a drawn piece stands for.
///
/// A mark may carry the tile under it, so a piece whose marks are where
/// things will stand paints a floor for them to stand on rather than
/// leaving whatever the map had there, which could be a wall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    /// Paints this tile.
    Tile(TileId),
    /// A mark: a position the piece's owner gives a meaning to, painted
    /// with the tile when there is one and transparent when not.
    Mark(Option<TileId>),
    /// Leaves the map as it was.
    Clear,
}
```

Add `key: Option<u32>` to `Prefab` with a doc comment ("An opaque number its owner set, carried to [`Stamped::prefab`], so the stamp's marks can be traced back to the definition that gave them meaning. `None` for a piece nobody keyed.").
Rewrite `parse` as a call to `parse_cells` mapping `Some(t)` to `Cell::Tile(t)`, a space to `Cell::Clear`, and anything else to `Cell::Mark(None)`, so its behavior is unchanged.
`parse_cells` is the current `parse` body with the `match legend(ch)` replaced by:

```rust
match legend(ch) {
    Cell::Tile(t) => {
        cells.set(p, Some(t));
    }
    Cell::Mark(under) => {
        cells.set(p, under);
        marks.push((ch, p));
    }
    Cell::Clear => {}
}
```

and `Ok(Self { cells, marks, key: None })`.
Add:

```rust
/// This piece, keyed: its stamp reports `key` as [`Stamped::prefab`].
pub fn keyed(mut self, key: u32) -> Self {
    self.key = Some(key);
    self
}

/// The key its owner set, if any.
pub fn key(&self) -> Option<u32> {
    self.key
}
```

`flipped` and `turned` build `Self { cells, marks, key: self.key }`.
`Stamped` gains:

```rust
/// The stamped piece's [`Prefab::key`], so whoever fills its marks finds
/// the definition that gave them meaning; `None` for an unkeyed piece.
pub prefab: Option<u32>,
```

and `StampPrefab::apply` emits `Stamped { bounds: placed, marks, prefab: prefab.key() }`.
Update the module doc's second paragraph to mention `Cell` and the key in one sentence each.

- [ ] **Step 4: Carry the key into `Spot`.** In `crates/rl-bevy/src/places.rs`, `Spot` gains:

```rust
/// The key of the prefab whose mark this is, when it was keyed, so the
/// engine can find what the mark stands for. `None` for a spot a game
/// made itself or a mark of an unkeyed piece.
#[serde(default)]
pub prefab: Option<u32>,
```

`PlaceBuild::from_context` builds `Spot { tag: *c as u32, at: *p, prefab: s.prefab }`.
Add `prefab: None` to the literals at `places.rs:354`, `places.rs:413`, `loot.rs:533` and `examples/corsair/src/places.rs:140`.
Add a test to `places.rs`'s tests:

```rust
#[test]
fn a_keyed_stamps_marks_become_spots_carrying_its_key() {
    use rl_mapgen::prefab::{Orient, Placement, Prefab, StampPrefab};
    use rl_mapgen::{BaseContext, BuildContext, Chain};
    let tiles = rl_grid::TileRegistry::standard();
    let wall = tiles.expect("wall");
    let piece = Prefab::parse(&["#A#"], |c| (c == '#').then_some(wall)).unwrap().keyed(3);
    let mut ctx = BaseContext::blank(10, 10, tiles.clone(), tiles.expect("floor"));
    Chain::new().then(StampPrefab { name: "keyed", prefab: piece, at: Placement::At(Point::new(1, 1)), orient: Orient::Fixed }).run(&mut ctx, rl_core::RunSeed(0)).unwrap();
    ctx.emit(rl_mapgen::passes::StartPoint(Point::new(5, 5)));
    let build = PlaceBuild::from_context(ctx).unwrap();
    assert_eq!(build.spots, vec![Spot { tag: 'A' as u32, at: Point::new(2, 1), prefab: Some(3) }]);
}
```

If `StartPoint`'s constructor or `ctx.emit` differ from this, read `crates/rl-mapgen/src/passes.rs` and match them.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p rl-mapgen && cargo test -p rl-bevy places && cargo test -p corsair`
Expected: PASS, including Corsair's fingerprint tripwire, unchanged.

- [ ] **Step 6: Docs and checks.** Re-read `docs/guide/src/systems/mapgen.md` and `places.md`; add one sentence to `mapgen.md`'s `The model` for `Cell` and the key, and to `places.md` for `Spot::prefab`, then bless both.
`CHANGELOG.md` under `Unreleased`:

```markdown
- A prefab mark can paint the ground under it: `Prefab::parse_cells` takes a legend to `Cell`, `Tile`, `Mark(Option<TileId>)` or `Clear`, and `parse` is unchanged. A piece can be `keyed`, and its stamp reports the key as `Stamped::prefab`, carried to `Spot::prefab`. Breaking only for a struct literal: `Stamped { .., prefab: None }` and `Spot { .., prefab: None }`.
```

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && python3 scripts/check-systems.py`

- [ ] **Step 7: Commit**

```bash
git add -A crates/rl-mapgen crates/rl-bevy examples/corsair docs/guide CHANGELOG.md
git commit -m "mapgen: a mark can paint its ground, and a keyed piece's stamp says which piece it was"
```

---

### Task 2: A banded draw restricted to some rows

**Files:**
- Modify: `crates/rl-rules/src/content/table.rs`

**Interfaces:**
- Produces:
  - `BandedTable::band_where(&self, band: i32, keep: impl Fn(&T) -> bool) -> Option<i32>`
  - `BandedTable::pick_where(&self, band: i32, keep: impl Fn(&T) -> bool, rng: &mut impl Rng) -> Option<&BandedEntry<T>>`
  - `pick` is `pick_where(band, |_| true, rng)` and draws exactly as before.

- [ ] **Step 1: Write the failing tests** in `table.rs`'s tests:

```rust
fn rows() -> BandedTable<&'static str> {
    BandedTable::new(vec![
        BandedEntry::new("rat").bands(1, 10).weight(5),
        BandedEntry::new("crab").bands(2, 7).weight(3),
        BandedEntry::new("heavy").bands(3, 8).weight(2),
        BandedEntry::new("ghost").bands(1, 10).weight(0),
    ])
}

#[test]
fn a_restricted_draw_never_returns_a_row_it_was_told_to_leave_out() {
    let table = rows();
    for s in 0..500 {
        let mut rng = rand::rngs::StdRng::seed_from_u64(s);
        let e = table.pick_where(5, |n| *n != "rat", &mut rng).unwrap();
        assert_ne!(e.item, "rat", "seed {s}");
        assert_ne!(e.item, "ghost", "seed {s}: a row with no weight is never drawn");
    }
}

#[test]
fn an_unrestricted_draw_is_the_plain_draw_over_a_span_of_seeds() {
    let table = rows();
    for s in 0..500 {
        let (mut a, mut b) = (rand::rngs::StdRng::seed_from_u64(s), rand::rngs::StdRng::seed_from_u64(s));
        assert_eq!(table.pick(4, &mut a).map(|e| e.item), table.pick_where(4, |_| true, &mut b).map(|e| e.item), "seed {s}");
    }
}

#[test]
fn a_restricted_band_is_exact_where_a_kept_row_applies_and_falls_back_shallower_first() {
    let table = rows();
    let heavy_or_crab = |n: &&str| *n == "heavy" || *n == "crab";
    assert_eq!(table.band_where(5, heavy_or_crab), Some(5), "both apply at five");
    assert_eq!(table.band_where(12, heavy_or_crab), Some(8), "past the deepest, the deepest");
    assert_eq!(table.band_where(1, heavy_or_crab), Some(2), "shallower than all, the shallowest");
    assert_eq!(table.band_where(5, |n| *n == "ghost"), None, "a row with no weight covers nothing");
    assert_eq!(table.band_where(5, |n| *n == "nobody"), None);
}
```

Add `use rand::SeedableRng;` to the tests module if it is missing.

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p rl-rules content::table`
Expected: compile errors, `pick_where`/`band_where` not found.

- [ ] **Step 3: Implement** in `impl<T> BandedTable<T>`:

```rust
/// Draws one row at `band` by weight among those `keep` accepts, or
/// `None` if none applies there. No fallback: ask
/// [`band_where`](Self::band_where) first for the band to draw at.
pub fn pick_where(&self, band: i32, keep: impl Fn(&T) -> bool, rng: &mut impl Rng) -> Option<&BandedEntry<T>> {
    let kept = || self.at(band).filter(|e| keep(&e.item));
    let total: u64 = kept().map(|e| e.weight as u64).sum();
    if total == 0 {
        return None;
    }
    let mut roll = rng.random_range(0..total);
    for e in kept() {
        if (e.weight as u64) > roll {
            return Some(e);
        }
        roll -= e.weight as u64;
    }
    None
}

/// The band a draw among the rows `keep` accepts is made at, the way
/// `LootTable::band_for` answers for a tag: `band` itself when a kept row
/// with weight applies there, else the deepest band shallower than it
/// that one covers, so a request past the table's end gets its deepest,
/// else the shallowest band deeper. `None` when no kept row has weight.
pub fn band_where(&self, band: i32, keep: impl Fn(&T) -> bool) -> Option<i32> {
    let kept: Vec<&BandedEntry<T>> = self.entries.iter().filter(|e| e.weight > 0 && keep(&e.item)).collect();
    if kept.is_empty() {
        return None;
    }
    if kept.iter().any(|e| e.applies(band)) {
        return Some(band);
    }
    let shallower = kept.iter().map(|e| e.max_band).filter(|deepest| *deepest < band).max();
    shallower.or_else(|| kept.iter().map(|e| e.min_band).filter(|shallowest| *shallowest > band).min())
}
```

Replace `pick`'s body with `self.pick_where(band, |_| true, rng)`.

- [ ] **Step 4: Run the tests, then every fingerprint**

Run: `cargo test -p rl-rules && cargo test --workspace --test fingerprint`
Expected: PASS, and every game's fingerprint unchanged, since `pick` draws the same numbers.

- [ ] **Step 5: Checks and commit.** Bless any systems page that lists `crates/rl-rules/src/content/table.rs` after re-reading it (`grep -l table.rs docs/guide/src/systems/*.md`). No `CHANGELOG.md` line: nothing a game writes changes.

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && python3 scripts/check-systems.py
git add -A crates/rl-rules docs/guide
git commit -m "rules: a banded draw among some rows, falling back the way a tagged one does"
```

---

### Task 3: Roles

**Files:**
- Create: `crates/rl-rules/src/role.rs`
- Modify: `crates/rl-rules/src/lib.rs` (module, re-exports, module docs)

**Interfaces:**
- Consumes: `BandedTable::band_where`, `pick_where` (Task 2); `Names::id::<M>` (existing).
- Produces:
  - `pub struct RoleDef<M> { pub name: String, pub members: Vec<Id<M>> }`, `impl<M> Named`, `RoleDef::fits(&self, id: Id<M>) -> bool`
  - `pub type RoleId<M> = Id<RoleDef<M>>`
  - `role::load<M: 'static>(text: &str, names: &Names<'_>) -> Result<Registry<RoleDef<M>>, ContentError>`
  - `role::band_for<M>(table: &BandedTable<Id<M>>, role: &RoleDef<M>, band: i32) -> Option<i32>`
  - `role::draw<M>(table: &BandedTable<Id<M>>, role: &RoleDef<M>, band: i32, rng: &mut impl Rng) -> Option<Id<M>>`
  - Re-exported at the crate root: `RoleDef`, `RoleId`.

- [ ] **Step 1: Write the failing tests** at the bottom of the new `role.rs`:

```rust
#[cfg(test)]
mod tests {
    use rand::SeedableRng;

    use super::*;
    use crate::content::BandedEntry;

    /// A test monster: only a name.
    struct Beast(&'static str);
    impl Named for Beast {
        fn name(&self) -> &str {
            self.0
        }
    }

    fn beasts() -> Registry<Beast> {
        Registry::from_defs(vec![Beast("rat"), Beast("crab"), Beast("heavy"), Beast("moth")]).unwrap()
    }

    fn table(b: &Registry<Beast>) -> BandedTable<Id<Beast>> {
        BandedTable::new(vec![
            BandedEntry::new(b.expect("rat")).bands(1, 10).weight(5),
            BandedEntry::new(b.expect("crab")).bands(2, 7).weight(3),
            BandedEntry::new(b.expect("heavy")).bands(3, 8).weight(2),
        ])
    }

    #[test]
    fn a_roles_file_names_each_role_and_the_monsters_that_fit_it() {
        let b = beasts();
        let roles: Registry<RoleDef<Beast>> = load(r#"{ "brute": ["heavy", "crab"], "swarm": ["rat"] }"#, &Names::new().with("monster", &b)).unwrap();
        let brute = roles.get(roles.expect("brute"));
        assert_eq!(brute.members, vec![b.expect("heavy"), b.expect("crab")]);
        assert!(brute.fits(b.expect("crab")) && !brute.fits(b.expect("rat")));
    }

    #[test]
    fn a_roles_file_refuses_every_empty_role_unknown_monster_and_repeated_member_at_once() {
        let b = beasts();
        let err = load::<Beast>(r#"{ "empty": [], "typo": ["hevy"], "twice": ["rat", "rat"] }"#, &Names::new().with("monster", &b)).unwrap_err().to_string();
        for said in ["empty", "hevy", "twice"] {
            assert!(err.contains(said), "{said:?} missing from: {err}");
        }
    }

    #[test]
    fn a_role_draw_only_ever_returns_a_member_of_the_role() {
        let b = beasts();
        let roles: Registry<RoleDef<Beast>> = load(r#"{ "brute": ["heavy", "crab"] }"#, &Names::new().with("monster", &b)).unwrap();
        let (brute, table) = (roles.get(roles.expect("brute")), table(&b));
        for band in 1..=12 {
            for s in 0..100 {
                let got = draw(&table, brute, band, &mut rand::rngs::StdRng::seed_from_u64(s)).unwrap();
                assert!(brute.fits(got), "band {band}, seed {s}: {}", b.name(got));
            }
        }
    }

    #[test]
    fn a_role_asked_past_the_deepest_row_draws_from_the_deepest_band() {
        let b = beasts();
        let roles: Registry<RoleDef<Beast>> = load(r#"{ "brute": ["heavy", "crab"] }"#, &Names::new().with("monster", &b)).unwrap();
        let brute = roles.get(roles.expect("brute"));
        assert_eq!(band_for(&table(&b), brute, 12), Some(8));
        assert_eq!(band_for(&table(&b), brute, 5), Some(5));
    }

    #[test]
    fn a_role_none_of_whose_members_has_a_spawn_row_draws_nothing() {
        let b = beasts();
        let roles: Registry<RoleDef<Beast>> = load(r#"{ "fliers": ["moth"] }"#, &Names::new().with("monster", &b)).unwrap();
        let fliers = roles.get(roles.expect("fliers"));
        assert_eq!(band_for(&table(&b), fliers, 5), None);
        assert_eq!(draw(&table(&b), fliers, 5, &mut rand::rngs::StdRng::seed_from_u64(0)), None);
    }
}
```

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p rl-rules role`
Expected: compile errors, the module is empty.

- [ ] **Step 3: Implement** `role.rs`:

```rust
//! Roles: which monsters fit which part in a prefab.
//!
//! A prefab's monster slot may ask for a role rather than a monster, so a
//! room keeps its shape on every floor while what stands in it changes: a
//! doorway held by something heavy is held by the heaviest thing the
//! floor has. A role is only a name and its members. Where and how often
//! each member turns up is the spawn table's, written once, so a role
//! draw is the game's own spawn table restricted to the role's members,
//! at the slot's band, with the rows' own weights, falling back the way a
//! tagged loot draw does when no member is found that deep.
//!
//! A roles file is a map from a role's name to its members' names, and
//! [`load`] resolves every one of them at once, reporting every problem
//! in the file together.

use std::collections::BTreeMap;

use rand::Rng;
use rl_core::Id;

use crate::content::{BandedTable, ContentError, Named, Registry};
use crate::names::Names;

/// A part in a prefab and the monsters that fit it.
pub struct RoleDef<M> {
    /// The name a prefab's slot asks for.
    pub name: String,
    /// Who fits, in the order the file names them.
    pub members: Vec<Id<M>>,
}

/// A role, by id.
pub type RoleId<M> = Id<RoleDef<M>>;

impl<M> Named for RoleDef<M> {
    fn name(&self) -> &str {
        &self.name
    }
}

impl<M> std::fmt::Debug for RoleDef<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RoleDef").field("name", &self.name).field("members", &self.members).finish()
    }
}

impl<M> RoleDef<M> {
    /// Whether `id` fits the role.
    pub fn fits(&self, id: Id<M>) -> bool {
        self.members.contains(&id)
    }
}

/// Loads a roles file, `{ "role": ["monster", ..], .. }`, resolving every
/// member through `names`, whose registry of `M` is the game's monsters.
///
/// Refuses a role with no members, a name that is no monster, and a
/// member named twice in one role, every one of them at once. Roles are
/// numbered in name order, the order the map is read in.
pub fn load<M: 'static>(text: &str, names: &Names<'_>) -> Result<Registry<RoleDef<M>>, ContentError> {
    let authored: BTreeMap<String, Vec<String>> = ron::from_str(text).map_err(|e| ContentError::Parse(e.to_string()))?;
    let mut errors = Vec::new();
    let mut roles = Vec::new();
    for (name, said) in authored {
        if said.is_empty() {
            errors.push(format!("{name}: a role with no members is a slot nothing can stand in"));
        }
        let mut members = Vec::new();
        for member in &said {
            match names.id::<M>(member) {
                Ok(id) if members.contains(&id) => errors.push(format!("{name}: {member:?} is named twice")),
                Ok(id) => members.push(id),
                Err(e) => errors.push(format!("{name}: {e}")),
            }
        }
        roles.push(RoleDef { name, members });
    }
    if !errors.is_empty() {
        return Err(ContentError::Invalid(errors));
    }
    Registry::from_defs(roles)
}

/// The band a draw for `role` at `band` is made at: `band` when a member
/// with a weighted row applies there, else the nearest band that has one,
/// shallower first. `None` when no member has a weighted row at all.
pub fn band_for<M>(table: &BandedTable<Id<M>>, role: &RoleDef<M>, band: i32) -> Option<i32> {
    table.band_where(band, |id| role.fits(*id))
}

/// Draws a member of `role` from `table` at `band`, or at the band
/// [`band_for`] falls back to, by the rows' own weights.
pub fn draw<M>(table: &BandedTable<Id<M>>, role: &RoleDef<M>, band: i32, rng: &mut impl Rng) -> Option<Id<M>> {
    let at = band_for(table, role, band)?;
    table.pick_where(at, |id| role.fits(*id), rng).map(|e| e.item)
}
```

If `Registry::from_defs` requires a bound `RoleDef<M>` does not meet, or `ron::from_str` needs the `IMPLICIT_SOME` options the rest of the crate uses, match what `crates/rl-rules/src/names.rs` does.
In `lib.rs`: `pub mod role;`, `pub use role::{RoleDef, RoleId};`, and one line in the crate's module docs naming the module.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rl-rules role && cargo test -p rl-rules --doc`
Expected: PASS.

- [ ] **Step 5: Checks and commit.** No `CHANGELOG.md` line yet (Task 9 describes roles as part of prefabs).

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && scripts/check-tiers.sh && scripts/check-tiers.sh --wasm
git add -A crates/rl-rules
git commit -m "rules: roles, a name and the monsters that fit it, drawn through the spawn table"
```

---

### Task 4: Prefab definitions from RON

**Files:**
- Create: `crates/rl-rules/src/prefab.rs`
- Modify: `crates/rl-rules/src/prop.rs` (share the row reader and the count), `crates/rl-rules/src/lib.rs`

**Interfaces:**
- Consumes: `RoleDef`, `RoleId` (Task 3); `ContentRoll`, `Stock` (existing, `prop.rs`); `rl_grid::TileRegistry`.
- Produces:
  - `pub enum Pick<M> { Kind(Id<M>), Role(RoleId<M>) }`
  - `pub enum Slot<M> { Prop(PropId), Item(ContentRoll), Monster { pick: Pick<M>, band: i32 }, Mark }`
  - `pub struct PrefabDef<M> { pub name: String, pub ground: Option<TileId>, .. }` with `rows(&self) -> Vec<&str>`, `tile(&self, glyph: char) -> Option<TileId>`, `slot(&self, glyph: char) -> Option<&Slot<M>>`, `slots(&self) -> impl Iterator<Item = (char, &Slot<M>)>`, `impl<M> Named`
  - `prefab::load<M: 'static>(text: &str, tiles: &TileRegistry, names: &Names<'_>) -> Result<PrefabDef<M>, ContentError>`, where `names` holds the game's props as `.with("prop", &props)`, its monsters as `.with("monster", &monsters)`, its roles as `.with("role", &roles)`, and its tags.
  - In `prop.rs`: `pub(crate) fn read_stock(item: Option<&str>, tag: Option<&str>, band: i32, names: &Names<'_>) -> Result<Stock, String>` and `pub(crate) enum CountRon` with `pub(crate) fn range(self) -> (u32, u32)`.
  - Re-exported at the crate root: `PrefabDef`, `Slot`, `Pick`.

- [ ] **Step 1: Share the row reader.** In `prop.rs`, make `CountRon` and its `range` `pub(crate)`, and add:

```rust
/// What a row naming an `item` or a `tag` puts in: a fixed item by name,
/// which takes no band offset, or anything carrying the tag. Shared by
/// a container's contents and a prefab's item slot, which are the same
/// row, so the two can never disagree about what one means.
pub(crate) fn read_stock(item: Option<&str>, tag: Option<&str>, band: i32, names: &Names<'_>) -> Result<Stock, String> {
    match (item, tag) {
        (Some(item), None) if band != 0 => Err(format!("{item:?} is a fixed item, drawn from no band, so a band offset means nothing on it")),
        (Some(item), None) => Ok(Stock::Item(item.to_string())),
        (None, Some(tag)) => names.tag(tag).map(Stock::Tag).map_err(|e| format!("it asks for {e}")),
        _ => Err("a row names an `item` or a `tag`, and exactly one of them".to_string()),
    }
}
```

Replace the `match (&row.item, &row.tag)` block in `load` with:

```rust
let what = match read_stock(row.item.as_deref(), row.tag.as_deref(), row.band, names) {
    Ok(what) => what,
    Err(e) => {
        errors.push(format!("{}: {e}", a.name));
        continue;
    }
};
```

Run: `cargo test -p rl-rules prop`
Expected: PASS; the test at `prop.rs:443` still finds "band offset means nothing", "exactly one of them" and "wepon".

- [ ] **Step 2: Write the failing tests** at the bottom of the new `prefab.rs`. The fixture:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::affix::TagDef;
    use crate::prop::PropDef;
    use rl_grid::{TileProps, TileRegistry};

    struct Beast(&'static str);
    impl Named for Beast {
        fn name(&self) -> &str {
            self.0
        }
    }

    struct World {
        tiles: TileRegistry,
        beasts: Registry<Beast>,
        roles: Registry<RoleDef<Beast>>,
        props: Registry<PropDef>,
        tags: Registry<TagDef>,
    }

    fn world() -> World {
        let mut tiles = TileRegistry::new();
        tiles.register(TileProps::wall("bulkhead")).unwrap();
        tiles.register(TileProps::floor("deck")).unwrap();
        let beasts = Registry::from_defs(vec![Beast("heavy"), Beast("warden")]).unwrap();
        let roles = crate::role::load(r#"{ "brute": ["heavy"] }"#, &Names::new().with("monster", &beasts)).unwrap();
        let tags = Registry::from_defs(vec![TagDef::new("weapon")]).unwrap();
        let props = crate::prop::load(r#"[(name: "locker", glyph: 'L', color: (1.0, 1.0, 1.0), blocks: true)]"#, &Names::new().tags(&tags)).unwrap();
        World { tiles, beasts, roles, props, tags }
    }

    fn read(w: &World, text: &str) -> Result<PrefabDef<Beast>, ContentError> {
        let names = Names::new().tags(&w.tags).with("prop", &w.props).with("monster", &w.beasts).with("role", &w.roles);
        load(text, &w.tiles, &names)
    }

    const GUARDED: &str = r#"(
        name: "guarded locker",
        ground: "deck",
        rows: ["#####", "#bLw#", "##W##", "##m##"],
        legend: {
            '#': Tile("bulkhead"),
            'L': Prop("locker"),
            'w': Item(tag: "weapon", band: 2),
            'b': Monster(role: "brute", band: 1),
            'W': Monster(monster: "warden"),
            'm': Mark,
        },
    )"#;
```

The tests:

```rust
    #[test]
    fn a_prefab_file_resolves_every_tile_and_slot_its_legend_names() {
        let w = world();
        let def = read(&w, GUARDED).unwrap();
        assert_eq!(def.name, "guarded locker");
        assert_eq!(def.ground, w.tiles.id("deck"));
        assert_eq!(def.tile('#'), w.tiles.id("bulkhead"));
        assert!(matches!(def.slot('L'), Some(Slot::Prop(p)) if *p == w.props.expect("locker")));
        assert!(matches!(def.slot('w'), Some(Slot::Item(ContentRoll { what: Stock::Tag(_), min: 1, max: 1, band: 2 }))));
        assert!(matches!(def.slot('b'), Some(Slot::Monster { pick: Pick::Role(r), band: 1 }) if *r == w.roles.expect("brute")));
        assert!(matches!(def.slot('W'), Some(Slot::Monster { pick: Pick::Kind(k), band: 0 }) if *k == w.beasts.expect("warden")));
        assert!(matches!(def.slot('m'), Some(Slot::Mark)));
        assert_eq!(def.rows(), vec!["#####", "#bLw#", "##W##", "##m##"]);
    }

    #[test]
    fn a_prefab_file_refuses_every_mistake_in_it_at_once() {
        let w = world();
        let text = r#"(
            name: "broken",
            ground: "bulkhead",
            rows: ["#?#", "#b"],
            legend: {
                '#': Tile("bulkhed"),
                'b': Monster(monster: "warden", band: 1),
                'r': Monster(role: "bruiser"),
                'x': Monster(monster: "heavy", role: "brute"),
                'i': Item(item: "knife", band: 1),
                't': Item(tag: "wepon"),
                'p': Prop("lockr"),
                ' ': Mark,
            },
        )"#;
        let err = read(&w, text).unwrap_err().to_string();
        for said in [
            "row 1 is 2 wide",
            "'?' in row 0 is not in the legend",
            "bulkhed",
            "a band offset means nothing",
            "bruiser",
            "exactly one of them",
            "wepon",
            "lockr",
            "a space",
            "nothing can stand on",
            "is in the legend and in no row",
        ] {
            assert!(err.contains(said), "{said:?} missing from: {err}");
        }
    }

    #[test]
    fn a_prefab_with_slots_and_no_ground_is_refused() {
        let w = world();
        let err = read(&w, r#"(name: "bare", rows: ["m"], legend: { 'm': Mark })"#).unwrap_err().to_string();
        assert!(err.contains("no `ground`"), "{err}");
    }

    #[test]
    fn a_prefab_of_tiles_alone_needs_no_ground() {
        let w = world();
        assert!(read(&w, r#"(name: "wall", rows: ["##"], legend: { '#': Tile("bulkhead") })"#).is_ok());
    }
}
```

- [ ] **Step 3: Run them and see them fail**

Run: `cargo test -p rl-rules prefab`
Expected: compile errors.

- [ ] **Step 4: Implement** `prefab.rs`. Module doc, in the crate's voice: what a prefab file is, one thing per cell and why (a guard can never be drawn on a wall), what each slot is, that the terrain half is built by whoever holds `rl-mapgen` from `rows`, `tile` and `slot`, and that this crate holds no mapgen type so it stays testable on its own. Then:

```rust
use std::collections::BTreeMap;

use rl_core::Id;
use rl_grid::{TileId, TileRegistry};
use serde::Deserialize;

use crate::content::{ContentError, Named};
use crate::names::Names;
use crate::prop::{ContentRoll, CountRon, PropDef, PropId, Stock, read_stock};
use crate::role::{RoleDef, RoleId};

/// Which monster a slot holds: that one, or one drawn for a role.
pub enum Pick<M> {
    /// Always this monster, on every floor.
    Kind(Id<M>),
    /// Drawn from the spawn table among the role's members.
    Role(RoleId<M>),
}

/// What a glyph that is not a tile stands for.
pub enum Slot<M> {
    /// A prop of this kind.
    Prop(PropId),
    /// Items, as a container row says them: a named item in a count, or
    /// that many draws of a tag at the place's band plus the offset.
    Item(ContentRoll),
    /// A monster, which holds the slot's cell as its post.
    Monster {
        /// Which.
        pick: Pick<M>,
        /// Bands deeper than the place a role is drawn at; nought for a
        /// named monster.
        band: i32,
    },
    /// A position left to the game, which finds it among the place's
    /// spots by its glyph and puts there what only it knows how to.
    Mark,
}

/// A prefab, as its file says it.
pub struct PrefabDef<M> {
    /// The name a mapgen chain asks for.
    pub name: String,
    /// The tile painted under every slot. Always present when the legend
    /// has a slot, and always a tile a monster can stand on.
    pub ground: Option<TileId>,
    rows: Vec<String>,
    tiles: BTreeMap<char, TileId>,
    slots: BTreeMap<char, Slot<M>>,
}
```

Write `Debug` by hand for `Pick`, `Slot` and `PrefabDef` (fields by name, `Id`s as themselves) so none requires `M: Debug`, and `impl<M> Named for PrefabDef<M>`.
Methods: `rows()` returns `self.rows.iter().map(String::as_str).collect()`; `tile(c)` is `self.tiles.get(&c).copied()`; `slot(c)` is `self.slots.get(&c)`; `slots()` iterates the map as `(char, &Slot<M>)`.

The authored shapes, all `deny_unknown_fields`:

```rust
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PrefabRon {
    name: String,
    #[serde(default)]
    ground: Option<String>,
    rows: Vec<String>,
    legend: BTreeMap<char, LegendRon>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
enum LegendRon {
    Tile(String),
    Prop(String),
    Item {
        #[serde(default)]
        item: Option<String>,
        #[serde(default)]
        tag: Option<String>,
        #[serde(default)]
        count: Option<CountRon>,
        #[serde(default)]
        band: i32,
    },
    Monster {
        #[serde(default)]
        monster: Option<String>,
        #[serde(default)]
        role: Option<String>,
        #[serde(default)]
        band: i32,
    },
    Mark,
}
```

`load` parses with `ron::options::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)`, collects every problem into one `Vec<String>` each prefixed `"{name}: "`, and returns `ContentError::Invalid` if any. The checks, with the wording the test looks for:

1. No rows, or a first row of width 0: `"has no rows"`.
2. A row of another width: `"row {y} is {w} wide, the first row is {first}"`.
3. A glyph in a row that is neither a space nor in the legend: `"'{c}' in row {y} is not in the legend"` (once per glyph, first row it appears in).
4. A legend key that is a space: `"a space is the map left as it was, and cannot be given a meaning"`.
5. A legend key used in no row: `"'{c}' is in the legend and in no row"`.
6. `Tile(name)`: `tiles.id(name)` or `"'{c}' paints unknown tile {name:?}"`.
7. `Prop(name)`: `names.id::<PropDef>(name)` or `"'{c}': {e}"`.
8. `Item { .. }`: `read_stock(item, tag, band, names)` or `"'{c}': {e}"`; the count is `count.map(CountRon::range).unwrap_or((1, 1))`, and `min > max` is `"'{c}' is written as {min} to {max}, which is no range at all"`. The slot is `Slot::Item(ContentRoll { what, min, max, band })`.
9. `Monster { .. }`: exactly one of `monster`/`role` or `"'{c}' names a `monster` or a `role`, and exactly one of them"`; a named monster with `band != 0` is `"'{c}': {name:?} is a fixed monster, drawn from no band, so a band offset means nothing on it"`; `names.id::<M>(name)` or `names.id::<RoleDef<M>>(role)`, else `"'{c}': {e}"`.
10. `ground`: when any slot exists and `ground` is `None`, `"has slots and no `ground` to stand them on"`; an unknown ground is `"its ground is unknown tile {name:?}"`; a ground whose `TileProps::walkable` is false is `"its ground {name:?} is a tile nothing can stand on"`.

In `lib.rs`: `pub mod prefab;`, `pub use prefab::{Pick, PrefabDef, Slot};`, and the module in the crate docs.
The file-format comment for the prefab schema lives in the module doc, as the spec's section 3 gives it, with `Item(item: ..)` and `Monster(monster: ..)`, and `Mark`.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p rl-rules`
Expected: PASS.

- [ ] **Step 6: Checks and commit.** `props.md` and `loot.md` list `prop.rs`: re-read, bless.

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && scripts/check-tiers.sh --wasm && python3 scripts/check-systems.py
git add -A crates/rl-rules docs/guide
git commit -m "rules: a prefab file, one tile or one slot to a glyph, every name resolved at load"
```

---

### Task 5: The coverage report

**Files:**
- Modify: `crates/rl-rules/src/prefab.rs`

**Interfaces:**
- Consumes: `PrefabDef`, `Slot`, `Pick` (Task 4), `role::band_for` (Task 3), `LootTable::band_for` (existing).
- Produces:
  - `pub enum Reach { Exact, Fallback(i32), Empty }`
  - `pub struct CoverageRow { pub prefab: String, pub glyph: char, pub what: String, pub reach: Vec<(i32, Reach)> }`
  - `pub struct Coverage { pub bands: RangeInclusive<i32>, pub rows: Vec<CoverageRow> }` with `empties(&self) -> Vec<(&str, char, i32)>` and `render(&self) -> String`
  - `pub struct Sources<'a, M, I> { pub roles: &'a Registry<RoleDef<M>>, pub monsters: &'a BandedTable<Id<M>>, pub items: &'a LootTable<I>, pub tags: &'a Registry<TagDef> }`
  - `pub fn coverage<'p, M: 'p, I>(prefabs: impl IntoIterator<Item = &'p PrefabDef<M>>, sources: &Sources<'_, M, I>, bands: RangeInclusive<i32>) -> Coverage`
  - Re-exported at the root: `Coverage`, `CoverageRow`, `Reach`.

- [ ] **Step 1: Write the failing tests** in `prefab.rs`'s tests, reusing Task 4's `world()` and `GUARDED`:

```rust
    fn heavy_on(w: &World, lo: i32, hi: i32) -> BandedTable<Id<Beast>> {
        BandedTable::new(vec![crate::content::BandedEntry::new(w.beasts.expect("heavy")).bands(lo, hi)])
    }

    fn weapons(w: &World, lo: i32, hi: i32) -> crate::loot::LootTable<u32> {
        let weapon = w.tags.expect("weapon");
        crate::loot::LootTable::new(vec![crate::loot::LootRow { item: 1u32, bands: (lo, hi), weight: 1, group: (1, 1), tags: vec![weapon] }])
    }

    #[test]
    fn coverage_reads_exact_where_a_slot_finds_its_band_and_fallback_where_it_does_not() {
        let w = world();
        let def = read(&w, GUARDED).unwrap();
        let (monsters, items) = (heavy_on(&w, 3, 8), weapons(&w, 1, 10));
        let sources = Sources { roles: &w.roles, monsters: &monsters, items: &items, tags: &w.tags };
        let report = coverage([&def], &sources, 1..=10);
        let brute = report.rows.iter().find(|r| r.glyph == 'b').unwrap();
        assert_eq!(brute.what, "brute +1");
        assert!(matches!(brute.reach[0], (1, Reach::Fallback(3))), "band 1 asks at 2, nothing until 3");
        assert!(matches!(brute.reach[4], (5, Reach::Exact)));
        assert!(matches!(brute.reach[9], (10, Reach::Fallback(8))), "band 10 asks at 11, the deepest is 8");
        let weapon = report.rows.iter().find(|r| r.glyph == 'w').unwrap();
        assert!(matches!(weapon.reach[9], (10, Reach::Fallback(10))));
        assert!(report.rows.iter().all(|r| r.glyph != 'W' && r.glyph != 'L' && r.glyph != 'm'), "named, prop and mark slots draw nothing");
        assert!(report.empties().is_empty());
    }

    #[test]
    fn coverage_names_every_slot_that_can_draw_nothing_at_all() {
        let w = world();
        let def = read(&w, GUARDED).unwrap();
        let (monsters, items) = (BandedTable::new(Vec::new()), weapons(&w, 1, 10));
        let sources = Sources { roles: &w.roles, monsters: &monsters, items: &items, tags: &w.tags };
        let report = coverage([&def], &sources, 1..=3);
        assert_eq!(report.empties(), vec![("guarded locker", 'b', 1), ("guarded locker", 'b', 2), ("guarded locker", 'b', 3)]);
    }

    #[test]
    fn a_rendered_report_has_a_column_per_band_and_a_row_per_drawn_slot() {
        let w = world();
        let def = read(&w, GUARDED).unwrap();
        let (monsters, items) = (heavy_on(&w, 3, 8), weapons(&w, 1, 10));
        let sources = Sources { roles: &w.roles, monsters: &monsters, items: &items, tags: &w.tags };
        let text = coverage([&def], &sources, 1..=10).render();
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("guarded locker") && lines[0].trim_end().ends_with("10"), "{text}");
        assert!(lines.iter().any(|l| l.trim_start().starts_with("b brute +1") && l.contains("~3") && l.contains("~8") && l.contains('✓')), "{text}");
        assert!(lines.iter().any(|l| l.trim_start().starts_with("w weapon +2")), "{text}");
    }
```

If `LootTable::new` or `LootRow`'s fields differ, read `crates/rl-rules/src/loot.rs` and build the table the way its own tests do.

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p rl-rules prefab`
Expected: compile errors.

- [ ] **Step 3: Implement.** Rows are for `Slot::Item` with `Stock::Tag` and `Slot::Monster` with `Pick::Role` only, in the order prefabs are given and then glyph order. `what` is the role's or tag's name followed by ` +N` or ` -N` when the offset is not nought. For each band `b` in `bands`, the asked band is `b + offset`; the reached band is `role::band_for(monsters, role, asked)` or `items.band_for(tag, asked)`; `None` is `Reach::Empty`, `Some(asked)` is `Reach::Exact`, `Some(x)` is `Reach::Fallback(x)`.
`render`: for each prefab with at least one row, a header line of the prefab's name padded to the label width, then each band right-aligned in a four-character column; then each row as two spaces, the glyph, a space, `what`, padded to the same width, and each cell as `✓`, `~N` or `✗` right-aligned in four characters. The label width is the longest of the prefab names and `"  g " + what`, plus two. Prefabs are separated by a blank line.
Doc comments say why: an author learns here what no startup check can tell them, whether the slot meets anything at the band it will be stamped at.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rl-rules`
Expected: PASS.

- [ ] **Step 5: Checks and commit.**

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings
git add -A crates/rl-rules
git commit -m "rules: a coverage report of every drawn prefab slot at every band"
```

---

### Task 6: Holding a post

**Files:**
- Modify: `crates/rl-rules/src/ai/tactics.rs` (add `Posted`, `KeepPost`), `crates/rl-rules/src/ai.rs`, `crates/rl-rules/src/lib.rs` (re-exports)
- Modify: `crates/rl-bevy/src/minds.rs` (add `Post`, `sense_posts`, register it)
- Modify: `crates/rl-save/src/run.rs` (`EntityState::post`)
- Modify: `docs/guide/src/systems/minds.md`, `docs/guide/src/systems/saving.md`, `CHANGELOG.md`

**Interfaces:**
- Produces:
  - `rl_rules::ai::tactics::Posted(pub Point)`, a sense
  - `rl_rules::ai::tactics::KeepPost`, a `Tactic<A>` named `"keep_post"`
  - `rl_bevy::minds::Post(pub Point)`, a component; `sense_posts` in `PerceiveSet::Annotate`, added by `MindsPlugin`
  - `rl_save::EntityState::post: Option<Point>`

- [ ] **Step 1: Write the failing rl-rules tests** in `tactics.rs`'s tests, with the module's own `Given`, `open()` and `view()` helpers; read how an existing test such as Wander's builds a `TacticCtx` and do the same:

```rust
#[test]
fn a_posted_actor_with_nothing_to_do_walks_back_to_its_post() {
    let (terrain, tiles) = open();
    let view = terrain.view(&tiles.tables());
    let mut snapshot = Snapshot::alone(view(0, 6, 6, 10));
    snapshot.add_sense(Posted(Point::new(2, 2)));
    let decision = /* evaluate `KeepPost` over `snapshot`, `Given::over(&view)`, a can_step of every open cell */;
    assert_eq!(decision, Some(Decision::Step(Point::new(5, 5))), "one step down the field toward the post");
}

#[test]
fn a_posted_actor_at_its_post_waits_there() {
    // as above, standing on (2, 2): Some(Decision::Wait)
}

#[test]
fn a_guard_whose_post_is_taken_waits_rather_than_wandering() {
    // standing at (3, 3), post (2, 2), `can_step` refusing (2, 2): Some(Decision::Wait)
}

#[test]
fn an_actor_with_no_post_is_left_to_the_next_tactic() {
    // no `Posted` sense: None
}
```

Write the elided bodies in full in the file, following the existing tests' construction exactly; `terrain.view` and the local `view` helper may be named differently, so read them first.

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p rl-rules tactics`
Expected: compile errors.

- [ ] **Step 3: Implement** in `tactics.rs`:

```rust
/// Where an actor was set to stand, pushed as a [`Sense`](crate::ai::Sense)
/// by whoever set it: a monster placed at a prefab's slot is posted there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Posted(pub Point);

/// Hold a post: with nothing better to do, walk back to the cell the
/// actor was set to stand on, and wait there.
///
/// Last before a brain's idle tactic, so a guard fights, flees and
/// searches as it otherwise would and only then goes home, and a guard
/// that loses the player searches where it last saw them before it does.
/// Waits beside a post something else is standing on rather than
/// wandering off, since the post will be free again. Leaves the turn to
/// the next tactic for an actor with no [`Posted`] sense, so one brain
/// serves a monster kind whether or not this one was posted.
#[derive(Debug, Clone, Copy, Default)]
pub struct KeepPost;

impl<A: Copy> Tactic<A> for KeepPost {
    fn name(&self) -> &'static str {
        "keep_post"
    }
    fn evaluate(&self, ctx: &mut TacticCtx<'_, A>) -> Option<Decision<A>> {
        let post = ctx.snapshot.sense::<Posted>()?.0;
        if ctx.snapshot.me.pos == post {
            return Some(Decision::Wait);
        }
        Some(ctx.step_toward(&[post]).map_or(Decision::Wait, Decision::Step))
    }
}
```

Re-export both where the other tactics are re-exported (`ai.rs`, `lib.rs`, and `rl-engine`'s prelude if it lists tactics).

- [ ] **Step 4: Run the rl-rules tests**

Run: `cargo test -p rl-rules tactics`
Expected: PASS.

- [ ] **Step 5: Write the failing rl-bevy test** in `minds.rs`'s tests, following the setup of an existing test there that runs a mind over several turns on an open map:

```rust
#[test]
fn a_posted_mind_walks_home_and_stays_there() {
    // A headless app with the plugins that test uses; an open map; one
    // actor with `Mind(Arc::new(Brain::new().then(KeepPost)))`,
    // `Post(Point::new(3, 3))` and `Position(Point::new(9, 3))`; run
    // twelve turns; the actor stands on (3, 3) and has done since the
    // sixth.
}
```

Write the body in full, against the real harness.

- [ ] **Step 6: Implement** in `minds.rs`:

```rust
/// Where an actor was set to stand and walks back to when it has nothing
/// better to do: a monster placed at a prefab's slot. Read by the
/// [`KeepPost`](rl_rules::ai::tactics::KeepPost) tactic through the
/// [`Posted`](rl_rules::ai::tactics::Posted) sense [`sense_posts`] pushes,
/// so a kind's shared brain posts only the ones given a post. Saved by
/// `rl-save` with the actor.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Post(pub Point);

/// Tells the mind holding the turn where its post is, if it has one.
pub fn sense_posts(mut thinking: ResMut<Thinking>, posts: Query<&Post>) {
    let Some(post) = thinking.actor().and_then(|actor| posts.get(actor).ok()).copied() else { return };
    if let Some(snapshot) = thinking.snapshot_mut() {
        snapshot.add_sense(rl_rules::ai::tactics::Posted(post.0));
    }
}
```

Register `sense_posts` in `MindsPlugin::build` in `PerceiveSet::Annotate`, the way its other annotate contributors are registered (read how `combat.rs:897` or `items.rs:544`'s system is added, including any run condition).

- [ ] **Step 7: Save the post.** In `crates/rl-save/src/run.rs`, `EntityState` gains:

```rust
/// Where it was posted, for an actor that holds a post, so a guard still
/// walks back to its cell after a load.
#[serde(default)]
pub post: Option<Point>,
```

`of` reads `e.get::<rl_bevy::minds::Post>().map(|p| p.0)` into it (match the path `rl_bevy` exports `Post` at), and `restore` inserts `Post(at)` when it is `Some`.
Add a test beside the existing `EntityState` round-trip tests in `rl-save` (read how the remains or transition one is written): an actor with a `Post` saved and restored has the same `Post`.

- [ ] **Step 8: Run the tests**

Run: `cargo test -p rl-bevy minds && cargo test -p rl-save && cargo test --workspace --test fingerprint`
Expected: PASS; fingerprints unchanged, since nothing is posted yet.

- [ ] **Step 9: Docs and checks.** `minds.md`: `The model` names `KeepPost`, `Posted` and `Post` in one or two sentences; `The line` says the engine decides how a post is kept and the game decides which actors have one and where `KeepPost` sits in a brain. `saving.md` (and `loot.md`, which lists `run.rs`): re-read, add that a post is saved if the page lists what `EntityState` keeps, bless each.
`CHANGELOG.md`:

```markdown
- A mind can hold a post: `Post(Point)` on an actor, and `KeepPost` in its brain after `SearchLastKnown` and before its idle tactic, walks it back to the cell and waits there when it has nothing better to do. `KeepPost` does nothing for an actor with no `Post`, so it can sit in every brain. `rl-save` saves the post with the actor.
```

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && scripts/check-tiers.sh --wasm && python3 scripts/check-systems.py
git add -A crates docs/guide CHANGELOG.md
git commit -m "minds: an actor can hold a post, walking home when it has nothing better to do"
```

---

### Task 7: `PrefabPlugin`, the engine filling slots

**Files:**
- Create: `crates/rl-bevy/src/prefabs.rs`
- Modify: `crates/rl-bevy/src/loot.rs` (`Found::Placed`, share the stock draw and the laying), `crates/rl-bevy/src/lib.rs` (module, re-exports, prelude), `crates/rl-engine` prelude if it lists `LootPlugin`

**Interfaces:**
- Consumes: `Cell`, `Prefab::parse_cells`, `keyed`, `Spot::prefab` (Task 1); `PrefabDef`, `Slot`, `Pick`, `RoleDef`, `role::draw`, `role::band_for`, `coverage`, `Sources` (Tasks 3-5); `Post` (Task 6); `ItemMaker`, `spawn_prop`, `PlaceEntered`, `WorldMap`, `Seed` (existing).
- Produces:
  - `pub trait ActorMaker: Resource { type Def: Send + Sync + 'static; fn make(&self, commands: &mut Commands, registries: &Registries, def: Id<Self::Def>, at: Point, map: MapId, rng: &mut StdRng) -> Entity; fn table(&self) -> &BandedTable<Id<Self::Def>>; fn band(&self, map: MapId) -> i32; }`
  - `#[derive(Resource)] pub struct Prefabs<M>(Arc<..>)`, `Clone`, with `new(defs: Registry<PrefabDef<M>>, roles: Registry<RoleDef<M>>) -> Self`, `defs()`, `roles()`, `piece(&self, name: &str) -> Result<Prefab, BuildError>`, `slot(&self, key: u32, glyph: char) -> Option<&Slot<M>>`
  - `Found::Placed`
  - `#[derive(SystemSet)] pub enum PrefabSet { Fill }`, inside `TurnSet::React`, before `LootSet::Scatter`
  - `pub struct PrefabPlugin<A, I>`, `Default`
  - `pub fn fill_prefabs<A: ActorMaker, I: ItemMaker<..>>` (see below for the bound)

- [ ] **Step 1: Share the loot helpers.** In `loot.rs`:
  - Add `Found::Placed` with the doc "Laid at a prefab's slot when the place was first built."
  - Extract from `fill_containers` the body that turns a `Stock` and a count into made-together draws:

```rust
/// What `count` of `what` comes to at `band`: a fixed item in that count,
/// or that many draws of a tag, each its own, with draws of one thing
/// made together. By definition id, so what is made comes out in one
/// order however the draws fell. Shared by containers and prefab slots,
/// which ask in the same words.
pub(crate) fn draw_stock<M: ItemMaker>(maker: &M, what: &Stock, count: u32, band: i32, rng: &mut StdRng) -> Vec<(Id<M::Def>, u32)>
```

    `fill_containers` computes its band as now and calls it; the draws happen in exactly the same order, so fingerprints do not move.
  - Give `lay` a `found: Found` parameter; `scatter_places` and `scatter_regions` pass `Found::Scatter`.
  - Make `stream_for` `pub(crate)`.

Run: `cargo test -p rl-bevy loot && cargo test --workspace --test fingerprint`
Expected: PASS, fingerprints unchanged.

- [ ] **Step 2: Write the failing tests** in the new `prefabs.rs`. Build the fixture from `loot.rs`'s own test fixture (its `Toy` item maker and its `place` helper at `loot.rs:523`), adding a toy `ActorMaker` whose `make` spawns `(Actor, Position(at), OnMap(map), ToyKind(def))` and records nothing else, a spawn table of two beasts on bands `1..=10` and `3..=8`, a role file, props with a `locker`, and tiles. Each test builds a keyed piece with `Prefabs::piece`, installs a place whose spots are the stamp's (stamp it with a one-pass chain and `PlaceBuild::from_context`, as Task 1's test does), writes `PlaceEntered { map, first: true, entry, exit: None }` and runs the `Turn` schedule once. The tests:

```rust
#[test]
fn a_prefab_fills_its_prop_item_and_monster_slots_on_the_first_entry() {}
#[test]
fn a_role_slot_draws_a_member_of_the_role_at_the_places_band_plus_its_offset() {}
#[test]
fn a_monster_placed_at_a_slot_holds_that_cell_as_its_post() {}
#[test]
fn a_slot_on_the_arrival_cell_spawns_nothing_and_every_other_slot_is_the_same() {}
#[test]
fn a_slot_whose_cell_is_no_longer_walkable_spawns_nothing() {}
#[test]
fn two_prefabs_sharing_a_glyph_fill_it_each_their_own_way() {}
#[test]
fn a_revisit_fills_nothing() {}
#[test]
fn a_mark_and_an_unkeyed_spot_are_left_to_the_game() {}
#[test]
#[should_panic(expected = "which is no item")]
fn play_refuses_a_prefab_holding_an_item_the_game_has_no_definition_for() {}
#[test]
#[should_panic(expected = "nothing in the loot table carries it")]
fn play_refuses_a_prefab_asking_for_a_tag_nothing_carries() {}
#[test]
#[should_panic(expected = "no monster in it has a row in the spawn table")]
fn play_refuses_a_role_none_of_whose_members_can_be_drawn() {}
```

Each body is written in full: assertions name the exact cell and kind expected. For `a_slot_on_the_arrival_cell_...`, fill the same place twice in two apps, once with the entry on a slot's cell and once elsewhere, and compare every other slot's result. For the `should_panic` tests, enter `EngineState::Playing` the way `loot.rs`'s `check_containers` tests do.

- [ ] **Step 3: Run them and see them fail**

Run: `cargo test -p rl-bevy prefabs`
Expected: compile errors.

- [ ] **Step 4: Implement** `prefabs.rs`. Module doc in the voice of `loot.rs`'s: what the engine owns (when a slot is filled, from which stream, what is skipped), what the game owns (what a monster and an item are, through `ActorMaker` and `ItemMaker`, and every `Mark`), and why (every game wrote this loop by hand at its marks; `docs/design/prefabs.md` has the reasoning).

```rust
/// A game's monster registry, as the engine asks it for things.
///
/// Implemented on the resource holding a game's monster definitions, the
/// one place that knows what a monster is. The engine decides when, where
/// and which, and asks this to make it. A game's own population spawns
/// through the same `make`, so a monster placed at a slot and one placed
/// by the game are built the same way and cannot drift apart.
pub trait ActorMaker: Resource {
    /// The game's monster definition.
    type Def: Send + Sync + 'static;

    /// Makes one `def` standing at `at` on `map`, and returns it. `rng`
    /// is the engine's stream for this slot, for whatever the game rolls
    /// on the monster itself.
    fn make(&self, commands: &mut Commands, registries: &Registries, def: Id<Self::Def>, at: Point, map: MapId, rng: &mut StdRng) -> Entity;

    /// Where each monster turns up, which a role draws from.
    fn table(&self) -> &BandedTable<Id<Self::Def>>;

    /// How deep, far or dangerous `map` is, in the numbering the table is
    /// written in.
    fn band(&self, map: MapId) -> i32;
}
```

`Prefabs<M>` holds `Arc<(Registry<PrefabDef<M>>, Registry<RoleDef<M>>)>` (or a small private struct), so a game shares one copy between its place builder and this resource. `piece(name)`:

```rust
/// The piece called `name`, keyed so its stamp's marks find their slots
/// again: each tile as the file paints it, each slot a mark painted with
/// the prefab's ground. An error naming the piece when there is none, so
/// a chain can `?` it.
pub fn piece(&self, name: &str) -> Result<Prefab, BuildError> {
    let id = self.defs().id(name).ok_or_else(|| BuildError::new("prefab", format!("no prefab is called {name:?}")))?;
    let def = self.defs().get(id);
    let cell = |c: char| match (def.tile(c), def.slot(c)) {
        (Some(t), _) => Cell::Tile(t),
        (None, Some(_)) => Cell::Mark(def.ground),
        (None, None) => Cell::Clear,
    };
    Prefab::parse_cells(&def.rows(), cell).map(|p| p.keyed(id.raw())).map_err(|e| BuildError::new("prefab", format!("{name}: {e}")))
}
```

`slot(key, glyph)` is `self.defs().try_get(Id::from_raw(key))?.slot(glyph)`.
The system:

```rust
/// Fills every keyed spot of a place on the arrival that built it: a prop,
/// items or a monster, whatever the prefab's slot says, each drawn from a
/// stream derived for its own cell, so nothing that happens at one slot
/// moves what is drawn at another. A slot draws first and then decides
/// whether to spawn: on the arrival cell, on a cell no longer walkable,
/// or on one this pass already filled, it spawns nothing.
pub fn fill_prefabs<A, I>(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, fill: Filling<A, I>)
where
    A: ActorMaker,
    I: ItemMaker,
```

with a `SystemParam` `Filling<A, I>` of `Res<Prefabs<A::Def>>`, `Res<A>`, `Res<I>`, `Res<Registries>`, `Res<WorldMap>`, `Res<Seed>`. For each `ev` with `first`, for each spot with `Some(key)` and a slot (`char::from_u32(spot.tag)`):
- the stream is `stream_for(&seed, b"prefab.content", position_hash(u64::from(ev.map.0), spot.at.x, spot.at.y))`;
- draw: `Slot::Mark` is skipped; `Slot::Prop(id)` draws nothing; `Slot::Item(roll)` rolls the count in `roll.min..=roll.max` from the stream (as `stock_containers` does) and calls `draw_stock(&*items, &roll.what, count, items.band(LootArea::Place(ev.map)) + roll.band, &mut rng)`; `Slot::Monster { pick, band }` is the kind itself or `role::draw(actors.table(), prefabs.roles().get(role), actors.band(ev.map) + band, &mut rng)`;
- skip when `spot.at == ev.entry`, when the place's terrain at `spot.at` is not walkable by `map.tables().walkable`, or when an earlier slot in this arrival filled it;
- spawn: `spawn_prop(&mut commands, &registries, id, spot.at, ev.map)`; each `(def, count)` through `items.make(.., Found::Placed, &mut rng)` with `Position(spot.at)` and `OnMap(ev.map)` (the `lay` helper); the monster through `actors.make(.., &mut rng)` and then `commands.entity(e).insert(Post(spot.at))`.

The check, `OnEnter(EngineState::Playing)`, in the manner of `check_containers`: for every prefab slot, a `Stock::Item` whose `items.id_of` is `None` is `"{prefab}: '{c}' holds {name:?}, which is no item"`; a `Stock::Tag` not carried by `items.table()` is `"{prefab}: '{c}' asks for anything tagged {tag:?}, and nothing in the loot table carries it"`; a `Pick::Role` whose `role::band_for(actors.table(), role, 0)` is `None` is `"{prefab}: '{c}' asks for role {role:?}, and no monster in it has a row in the spawn table"`; then `assert!(problems.is_empty(), "PrefabPlugin: {}", problems.join("; "))`.
The plugin:

```rust
impl<A: ActorMaker, I: ItemMaker> Plugin for PrefabPlugin<A, I> {
    fn build(&self, app: &mut App) {
        use crate::plugin::{Needs, Turn, TurnSet};
        app.needs::<Prefabs<A::Def>>("PrefabPlugin", "the game's prefabs and roles, as `Prefabs::new(prefabs, roles)`, inserted before play begins")
            .needs::<A>("PrefabPlugin", "the resource holding the game's monsters, implementing `ActorMaker`, inserted before play begins")
            .needs::<I>("PrefabPlugin", "the resource holding the game's items, implementing `ItemMaker`, inserted before play begins")
            .needs::<Registries>("PrefabPlugin", "`Registries`, whose props a prefab's slots name")
            .add_message::<PlaceEntered>()
            .configure_sets(Turn, PrefabSet::Fill.in_set(TurnSet::React).before(crate::loot::LootSet::Scatter))
            .add_systems(Turn, fill_prefabs::<A, I>.in_set(PrefabSet::Fill))
            .add_systems(OnEnter(crate::state::EngineState::Playing), check_prefabs::<A, I>);
    }

    fn finish(&self, app: &mut App) {
        crate::plugin::depends_on::<crate::plugin::CorePlugin>(app, "PrefabPlugin");
    }
}
```

`PrefabSet`'s doc says a game that builds a place in the same pass orders what it spawns after `PrefabSet::Fill`, so its own population can keep off the slots, and before `LootSet::Scatter`.
Export `ActorMaker`, `Prefabs`, `PrefabPlugin`, `PrefabSet` and `Post` from `rl_bevy` and its prelude beside `LootPlugin`, `ItemMaker` and `LootSet`.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p rl-bevy && cargo test --workspace --test fingerprint`
Expected: PASS; fingerprints unchanged.

- [ ] **Step 6: Overview and changelog, then a work-in-progress commit.** `docs/OVERVIEW.md`: `PrefabPlugin` in the `rl-bevy` plugin table with one line in the table's style, and the new public types in the `rl-rules`, `rl-mapgen` and `rl-bevy` sections. `CHANGELOG.md` gets its prefab line in Task 9.

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && scripts/check-overview.sh
git add -A crates docs/OVERVIEW.md
git commit -m "wip: prefab plugin"
```

`scripts/check-systems.py` fails here and in Task 8, since `PrefabPlugin` has no page yet; Task 9 writes it and squashes Tasks 7 to 9 into one commit.

---

### Task 8: Foundry's prefabs, roles and guards

**Files:**
- Create: `examples/foundry/assets/prefabs/armory_wide.ron`, `armory_tall.ron`, `store_wide.ron`, `store_tall.ron`, `reactor.ron`, `core.ron`, `guard_post.ron`
- Create: `examples/foundry/assets/roles.ron`
- Create: `examples/foundry/src/prefabs.rs`
- Modify: `examples/foundry/src/decks.rs`, `src/props.rs`, `src/droids.rs`, `src/droids/spawns.rs`, `src/run.rs`, `src/plugin.rs`, `src/main.rs`, `src/lib.rs`, `tests/fingerprint.rs`

**Interfaces:**
- Consumes: everything in Tasks 1-7.
- Produces: `foundry::prefabs::load(tiles: &TileRegistry) -> Prefabs<MonsterDef>`, `foundry::prefabs::coverage_report() -> String`, `Foundry::prefabs(&self) -> &Prefabs<MonsterDef>`, `impl ActorMaker for Roster`, and RON anchors `guard_post.ron:post` and `roles.ron:roles` and a Rust anchor `droids.rs:maker` for Task 9's page.

- [ ] **Step 1: The prefab files.** One per piece, each opening with the schema comment from the spec's section 3 (with `Item(item: ..)`, `Monster(monster: ..)` and `Mark`), then the piece. Tiles are Foundry's names from `decks.rs`: `hull`, `deck`, `grating`, `bulkhead`, `hatch`, `console`, `lamp`. The existing pieces keep their shapes exactly:
  - `armory_wide`: `["#####", "#g.A#", "#...#", "##.##"]`, `ground: "deck"`, `'#': Tile("bulkhead")`, `'.': Tile("deck")`, `'g': Tile("grating")`, `'A': Prop("armory locker")`.
  - `armory_tall`: `["####", "#A.#", "#g.#", "#..#", "##.#"]`, the same legend.
  - `store_wide`: `["#l###", "#L..#", "#..L#", "##.##"]`, `'l': Tile("lamp")`, `'L': Prop("supply crate")`.
  - `store_tall`: `["#l##", "#L.#", "#..#", "#.L#", "##.#"]`, the same legend.
  - `reactor`: `["#####", "#.c.#", "#.R.#", "#...#", "##h##"]`, `'c': Tile("console")`, `'h': Tile("hatch")`, `'R': Mark`.
  - `core`: `["#####", "#ccc#", "#.R.#", "#c.c#", "##h##"]`, the same legend.
  - `guard_post`, the new guarded piece, five by five so any room that takes a reactor takes it, wrapped in `// ANCHOR: post` / `// ANCHOR_END: post`:

```ron
(
    name: "guard post",
    ground: "deck",
    rows: [
        "#####",
        "#sAs#",
        "#.w.#",
        "#.b.#",
        "##.##",
    ],
    legend: {
        '#': Tile("bulkhead"),
        '.': Tile("deck"),
        'A': Prop("armory locker"),
        'w': Item(tag: "weapon", band: 2),
        's': Monster(role: "sentry", band: 1),
        'b': Monster(role: "brute", band: 2),
    },
)
```

  `roles.ron`, with its schema comment and `// ANCHOR: roles` around the map:

```ron
{
    "sentry": ["probe droid", "trooper droid"],
    "brute":  ["line droid", "scrap crab", "heavy droid"],
}
```

- [ ] **Step 2: Loading.** `src/prefabs.rs`: module doc (Foundry's pieces and roles are data; the engine fills them), `include_str!` of each file in a `const PIECES: [&str; 7]`, and:

```rust
/// Every piece and role, resolved against Foundry's tiles, props, tags
/// and roster. Panics with every problem in every file, since a broken
/// piece is a deck that cannot be built.
pub fn load(tiles: &TileRegistry) -> Prefabs<MonsterDef> {
    let registries = crate::content::registries();
    let roster = Roster::load(&registries);
    let base = registries.names();
    let monsters = base.clone().with("monster", &roster.defs);
    let roles = role::load(ROLES_RON, &monsters).unwrap_or_else(|e| panic!("assets/roles.ron: {e}"));
    let names = monsters.with("prop", &registries.props).with("role", &roles);
    let defs = PIECES.iter().map(|text| prefab::load(text, tiles, &names).unwrap_or_else(|e| panic!("assets/prefabs: {e}"))).collect();
    Prefabs::new(Registry::from_defs(defs).expect("every piece has its own name"), roles)
}

/// The coverage of every piece's drawn slots on every deck, for
/// `--prefabs` and for the test that holds it free of gaps.
pub fn coverage_report() -> Coverage { /* registries, roster, armory (`crate::testing::armory` or `Armory::load`), `Foundry::new(RunSeed(0)).prefabs()`, `Sources`, 1..=DECKS */ }
```

Write `coverage_report` in full against how `crate::testing::armory` and `Armory::load` are really called. Add `pub mod prefabs;` to `lib.rs`.
`Foundry` gains a `prefabs: Prefabs<MonsterDef>` field, loaded in `Foundry::new` after its tiles are registered, and `pub fn prefabs(&self) -> &Prefabs<MonsterDef>`. `armories`, `stores`, `reactor` and `core` become `self.prefabs.piece("armory wide")?` and so on; `generate`'s chain is unchanged except that it also stamps the guard post:

```rust
// Decks two, four, five, seven and eight hold a guard post: a locker and
// a weapon held by what the deck has. Not deck one, which teaches the
// lamp and the droids, and not the charge decks, which already fit a
// third piece in their rooms.
if matches!(deck, 2 | 4 | 5 | 7 | 8) {
    chain = chain.then(StampPrefab { name: "guard post", prefab: self.prefabs.piece("guard post")?, at: Placement::AnyRoom, orient: Orient::TurnedOrMirrored });
}
```

placed before the reactor match so the charge decks' order is unchanged.

- [ ] **Step 3: Roster is an `ActorMaker`**, in `droids.rs`, wrapped in `// ANCHOR: maker` / `// ANCHOR_END: maker`:

```rust
/// The engine's guards are made here: a monster at a prefab's slot is
/// made exactly as one a deck's population places, and a deck's band is
/// its number.
impl ActorMaker for Roster {
    type Def = MonsterDef;

    fn make(&self, commands: &mut Commands, registries: &Registries, def: Id<MonsterDef>, at: Point, map: MapId, _: &mut rand::rngs::StdRng) -> Entity {
        spawn_monster(commands, self, def, at, map, registries)
    }

    fn table(&self) -> &BandedTable<Id<MonsterDef>> {
        &self.table
    }

    fn band(&self, map: MapId) -> i32 {
        crate::decks::deck_of(map) as i32
    }
}
```

In `Roster::from_ron`, each brain becomes `brain.then(SearchLastKnown).then(KeepPost).then(Wander { chance_pct: 30 })`, and the doc comment above it says where `KeepPost` sits and why.

- [ ] **Step 4: The arrival.**
  - `run.rs` `prepare`: `commands.insert_resource(foundry.prefabs().clone())` before `PlaceRulesRes` takes `foundry`.
  - `plugin.rs`: add `PrefabPlugin::<Roster, Armory>::default()` beside `LootPlugin`, and order the arrival chain `.after(PrefabSet::Fill)` as well as `.before(LootSet::Scatter)`, with a sentence in the comment above it saying the engine's slots are filled first, so the deck's own population keeps off them.
  - `props.rs` `place_on_arrival`: remove the locker and crate at marks (the prefabs say so) and the `supply`/`locker` lookups that only served them; start `taken` with every spot's cell, so no loose crate or cable lands on a slot; keep `crates_for(deck, stores.len())` counting the `'L'` spots. Rewrite its doc comment to match.
  - `droids/spawns.rs` `populate_deck`: the `walkable` predicate also refuses every spot's cell, and its doc says so in one clause.
  - `main.rs`: `["--prefabs"] => { print!("{}", foundry::prefabs::coverage_report().render()); return AppExit::Success; }`, and the usage line becomes `usage: foundry [--seed N | --prefabs]`; the file's top doc names the flag.

- [ ] **Step 5: Foundry's tests.**
  - `prefabs.rs`: `every_drawn_slot_of_every_piece_finds_something_on_every_deck` asserts `coverage_report().empties()` is empty; `every_piece_and_role_file_loads` calls `load` on `Foundry::new(RunSeed(0)).tiles()`.
  - `decks.rs`: `only_the_guard_post_decks_hold_one_and_each_holds_exactly_one`, counting spots tagged `'b'` over 20 seeds, the way the reactor test counts `'R'`; the existing overlap test's mark list gains `'s' | 'w' | 'b'`.
  - A headless test, in `prefabs.rs` or where Foundry's other arrival tests live (`crate::testing::headless`): `a_guard_post_is_manned_and_its_guards_hold_their_cells`: enter deck 2 on a few seeds, find the stamped guard post's `'s'` and `'b'` spots, and assert each holds a monster of a kind in its role carrying `Post` equal to its cell, unless the cell was the entry.
  - `a_continued_run_neither_refills_a_deck_nor_forgets_a_guards_post`, beside Foundry's existing save round-trip test (read `src/save.rs`'s tests): save on deck 2, load, and assert the count of monsters carrying `Post` is unchanged and every one still has its `Post`.
  - `every_deck_builds_over_a_span_of_seeds_with_an_entry_and_an_exit` and every existing test pass unchanged; if the guard post makes a deck fail to build on some seed, report it rather than loosening the test.

Run: `cargo test -p foundry`
Expected: every test passes except `tests/fingerprint.rs`.

- [ ] **Step 6: Re-baseline the fingerprint on purpose.** Run `cargo test -p foundry --test fingerprint`, confirm the only failure is the number, and set the new number. Its `CHANGELOG.md` line comes in Task 9.

- [ ] **Step 7: See it in the running game.** Run `FOUNDRY_START=2 cargo run -p foundry -- --seed 7` (read `main.rs`'s top doc for how screenshots of a deeper deck are taken, and the `run` skill's guidance), walk to the guard post, and look: the locker and the item are inside, two sentries flank the locker, the brute stands in front of the opening, and each walks back to its cell after losing sight of the commando. Check the rendering against the rest of the deck; anything that looks off gets fixed here. Record what was seen for the final report.

- [ ] **Step 8: Checks and a work-in-progress commit.**

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && scripts/check-overview.sh
git add -A examples/foundry
git commit -m "wip: foundry prefabs"
```

---

### Task 9: The documentation, and one commit for the plugin

**Files:**
- Create: `docs/design/prefabs.md`, `docs/guide/src/systems/prefabs.md`
- Modify: `docs/guide/src/SUMMARY.md`, `docs/OVERVIEW.md`, `CHANGELOG.md`, `README.md`, `CLAUDE.md`'s target (`AGENTS.md` or whichever file `CLAUDE.md` links to), `docs/README.md`, `docs/superpowers/specs/2026-09-24-prefab-slots-design.md` (status line)

- [ ] **Step 1: `docs/design/prefabs.md`**, in the other design docs' shape (read `docs/design/loot.md` first): what a prefab slot is for; why the legend is one thing per cell; why a role is its own file and rolls the spawn table; why each slot draws from its own cell's stream and draws before it decides to spawn; why a monster at a slot holds a post and why only `Hold`; why prop slots take no band; fantasy-rogue's prefab system as the rejected alternative, with each lesson (coordinate lists beside a tile grid, a copied spawner that drifted, unsaved patrol state, silent empty slots, a rarity string doing two jobs); and what is left for later, from the spec's section 9.

- [ ] **Step 2: `docs/guide/src/systems/prefabs.md`**, the six parts in order, eighty lines of prose at most, every snippet an `include:` of a real anchor, fenced as the guide fences:

```markdown
<!-- documents:
     plugins: PrefabPlugin
     files: crates/rl-mapgen/src/prefab.rs
            crates/rl-rules/src/prefab.rs
            crates/rl-rules/src/role.rs
            crates/rl-rules/src/content/table.rs
            crates/rl-rules/src/ai/tactics.rs
            crates/rl-bevy/src/prefabs.rs
            crates/rl-bevy/src/places.rs
            crates/rl-bevy/src/minds.rs
            crates/rl-bevy/src/loot.rs
            crates/rl-save/src/run.rs -->
```

`Using it` opens with one sentence and quotes `examples/foundry/assets/prefabs/guard_post.ron:post`, then `examples/foundry/assets/roles.ron:roles` and `examples/foundry/src/droids.rs:maker` as the second anchor, since turning the system on takes the maker the piece does not show.
`The line`: the engine decides when a slot is filled, from which stream, what is skipped, how a role is drawn and falls back, and how a post is kept; the game decides what its pieces and roles are, what a monster and an item are, where the chain stamps each piece, and every `Mark`.
Add it to `SUMMARY.md` after `Map generation`.
Run `scripts/check-systems-style.sh docs/guide/src/systems/prefabs.md`, `python3 scripts/expand-guide.py` if that is how includes are filled (read `scripts/check-guide.sh`), `scripts/check-guide.sh`, then `python3 scripts/check-systems.py --bless prefabs`.

- [ ] **Step 3: The inventory and the log.**
  - `docs/OVERVIEW.md`: re-read what Task 7 added; add Foundry's prefabs, roles and guard post to its Foundry description.
  - `README.md` feature list: prefabs as data, with slots for props, items by tag and monsters by role, drawn at the place's depth, and guards that hold their post.
  - The agent guide's layout block: add `prefabs` to the `docs/design/` list, and remove prefabs from nothing (they were not in the "no design doc yet" list).
  - `docs/README.md`: the new design doc.
  - `CHANGELOG.md`:

```markdown
- Prefabs are data: `rl_rules::prefab::load` reads a piece from RON, rows of glyphs and a legend mapping each to a `Tile`, a `Prop`, an `Item` (a named item or a tag, with a count and a band offset, the row a container's contents use), a `Monster` (a named monster, or a role drawn from the spawn table at the place's band plus an offset), or a `Mark` left to the game. `rl_rules::role::load` reads a roles file, a role's name and the monsters that fit it. `PrefabPlugin::<A, I>`, opt-in, fills every slot on the arrival that built a place, each from a stream derived for its cell, through `ActorMaker`, a new trait a game implements on its monster registry, and `ItemMaker`; a monster placed at a slot holds that cell as its `Post`. `Prefabs::piece` hands a mapgen chain a keyed piece. `prefab::coverage` reports, for every drawn slot at every band, whether it finds something there, falls back, or finds nothing. `Found::Placed` is new, so a `match` on `Found` gains an arm. `docs/design/prefabs.md` is the reasoning and `docs/guide/src/systems/prefabs.md` the reference.
- Foundry's armory, stores, reactor and core are prefab files, with the lockers and crates in them placed by the engine; `roles.ron` names a sentry and a brute role; decks two, four, five, seven and eight hold a guard post, a locker and a weapon held by two sentries and a brute drawn for the deck; `--prefabs` prints the coverage report. Its fingerprint is re-baselined for the guards.
```

- [ ] **Step 4: Update the spec's status line** to "built on branch `prefab-slots`" and correct its body where the build moved from it: the item slot's field is `item:` and the monster slot's `monster:`; `Mark` is a legend entry; each slot draws from a stream derived for its cell; Foundry's roles are a sentry and a brute.

- [ ] **Step 5: Every check.**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace --doc
scripts/check-tiers.sh && scripts/check-tiers.sh --wasm
scripts/check-overview.sh
scripts/check-guide.sh
python3 scripts/check-systems.py
scripts/check-systems-style.sh docs/guide/src/systems/prefabs.md
```

Expected: every one passes. Any failure, including a flaky test unrelated to this work, is fixed, not skipped.

- [ ] **Step 6: One commit for the plugin, its game and its pages.** Squash the two work-in-progress commits and this task's work into one:

```bash
git add -A
git commit -m "wip: prefab docs"
git reset --soft HEAD~3
git commit -m "prefabs are the engine's: slots for props, items and guards, filled on arrival"
```

Check with `git log --oneline -12` that the three `wip:` commits are gone and the earlier task commits are untouched.
