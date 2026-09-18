//! Hand-drawn pieces stamped into a generated map.
//!
//! A [`Prefab`] is rows of characters and a legend from character to tile.
//! A character the legend does not know is transparent: the generated map
//! shows through, so a prefab can be an irregular shape. [`Prefab::rotated`]
//! and [`Prefab::flipped`] carry a piece's marks with its tiles, since a
//! mark is a position in the piece, not on the map, and a vault's chest
//! stays in its alcove however the vault is laid down.
//!
//! [`StampPrefab`] stamps one piece at a [`Placement`], and [`Orient`]
//! says how it may be turned first: fixed, one of the four quarter-turns,
//! or one of all eight facings with mirroring. Authored per stamp rather
//! than per prefab, since the same vault may be free to turn in a cave and
//! fixed against a corridor that has to meet its door. [`StampOneOf`] is the
//! same pass with a weighted choice of pieces in front of it, for a chain
//! that wants variety without one entry per piece; a piece with no weight
//! stays in the list without ever being drawn, and nothing carrying weight
//! fails the chain rather than generating a map its game does not expect.
//!
//! Either pass emits where it landed as a [`Stamped`], in map coordinates,
//! so later passes can keep out of it or spawn into it.

use rand::Rng;
use rl_core::{Grid, Grid2D, Point, Rect};
use rl_grid::TileId;

use crate::chain::{BuildError, Pass, Phase};
use crate::context::BuildContext;
use crate::dungeon::Room;

/// A piece of map drawn by hand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prefab {
    cells: Grid<Option<TileId>>,
    /// Marks the legend gave a meaning to besides a tile, by character,
    /// in prefab-local coordinates: an entrance, a spawn, a chest.
    marks: Vec<(char, Point)>,
}

impl Prefab {
    /// Parses `rows`, all the same width, with `legend` naming the tile
    /// for a character. A character with no tile is transparent; if it is
    /// not a space it is kept as a mark.
    pub fn parse(rows: &[&str], legend: impl Fn(char) -> Option<TileId>) -> Result<Self, String> {
        let height = rows.len() as i32;
        let width = rows.first().map(|r| r.chars().count()).unwrap_or(0) as i32;
        if width == 0 || height == 0 {
            return Err("a prefab needs at least one row and one column".into());
        }
        let mut cells: Grid<Option<TileId>> = Grid::new(width, height);
        let mut marks = Vec::new();
        for (y, row) in rows.iter().enumerate() {
            if row.chars().count() as i32 != width {
                return Err(format!("row {y} is {} wide, the first row is {width}", row.chars().count()));
            }
            for (x, ch) in row.chars().enumerate() {
                let p = Point::new(x as i32, y as i32);
                match legend(ch) {
                    Some(t) => {
                        cells.set(p, Some(t));
                    }
                    None if ch != ' ' => marks.push((ch, p)),
                    None => {}
                }
            }
        }
        Ok(Self { cells, marks })
    }

    /// Width in cells.
    pub fn width(&self) -> i32 {
        self.cells.width()
    }

    /// Height in cells.
    pub fn height(&self) -> i32 {
        self.cells.height()
    }

    /// The tile at prefab-local `p`, if the prefab paints there.
    pub fn tile(&self, p: Point) -> Option<TileId> {
        self.cells.get(p).copied().flatten()
    }

    /// Every mark, with its prefab-local position.
    pub fn marks(&self) -> &[(char, Point)] {
        &self.marks
    }

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
    ///
    /// A hand-drawn piece with an opening on one side reads as a
    /// different piece once mirrored, without a second drawing to keep
    /// in sync with the first.
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

    /// Writes the prefab with its top-left at `origin`.
    pub fn stamp(&self, terrain: &mut rl_grid::Terrain, origin: Point) {
        for y in 0..self.height() {
            for x in 0..self.width() {
                if let Some(t) = self.tile(Point::new(x, y)) {
                    terrain.set(origin.offset(x, y), t);
                }
            }
        }
    }
}

/// Where a prefab goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// Centred on the map.
    Center,
    /// Top-left at this point.
    At(Point),
    /// Centred in the emitted [`Room`] with this index, which must be big
    /// enough to hold it.
    InRoom(usize),
    /// Centred in a random emitted [`Room`] big enough to hold it.
    AnyRoom,
}

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

/// Where a prefab landed: its bounds and its marks in map coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamped {
    /// The cells it covers.
    pub bounds: Rect,
    /// Its marks, in map coordinates.
    pub marks: Vec<(char, Point)>,
}

/// Stamps one prefab. Fails if the placement does not fit on the map.
#[derive(Debug, Clone)]
pub struct StampPrefab {
    /// A stable name, so two stamps in one chain draw different streams.
    pub name: &'static str,
    /// The piece.
    pub prefab: Prefab,
    /// Where it goes.
    pub at: Placement,
    /// How it may be turned before it lands.
    pub orient: Orient,
}

impl<C: BuildContext> Pass<C> for StampPrefab {
    fn name(&self) -> &'static str {
        self.name
    }
    fn phase(&self) -> Phase {
        Phase::Structures
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        let prefab = self.orient.apply(&self.prefab, ctx.rng());
        let (w, h) = (prefab.width(), prefab.height());
        let bounds = ctx.terrain().bounds();
        let centred = |r: Rect| Point::new(r.x + (r.width - w) / 2, r.y + (r.height - h) / 2);
        let fits = |r: &Rect| r.width >= w && r.height >= h;
        let origin = match self.at {
            Placement::Center => centred(bounds),
            Placement::At(p) => p,
            Placement::InRoom(i) => {
                let room = ctx.outputs().iter::<Room>().nth(i).ok_or_else(|| BuildError::new(self.name, format!("no room {i}")))?.0;
                if !fits(&room) {
                    return Err(BuildError::new(self.name, format!("room {i} is {}x{}, the prefab {w}x{h}", room.width, room.height)));
                }
                centred(room)
            }
            Placement::AnyRoom => {
                let rooms: Vec<Rect> = ctx.outputs().iter::<Room>().map(|r| r.0).filter(fits).collect();
                if rooms.is_empty() {
                    return Err(BuildError::new(self.name, format!("no room holds {w}x{h}")));
                }
                centred(rooms[ctx.rng().random_range(0..rooms.len())])
            }
        };
        let placed = Rect::new(origin.x, origin.y, w, h);
        if placed.intersection(&bounds) != Some(placed) {
            return Err(BuildError::new(self.name, format!("{placed:?} is off the map")));
        }
        prefab.stamp(ctx.terrain_mut(), origin);
        let marks = prefab.marks().iter().map(|(c, p)| (*c, origin + *p)).collect();
        ctx.emit(Stamped { bounds: placed, marks });
        Ok(())
    }
}

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
        let mut total: u32 = 0;
        for (_, w) in &self.choices {
            total = total.checked_add(*w).ok_or_else(|| BuildError::new(self.name, "the choices' weights overflow a u32".to_string()))?;
        }
        if total == 0 {
            return Err(BuildError::new(self.name, "no candidate carries weight".to_string()));
        }
        let roll = ctx.rng().random_range(0..total);
        let weights: Vec<u32> = self.choices.iter().map(|(_, w)| *w).collect();
        let chosen = self.choices[pick_weighted(&weights, roll)].0.clone();
        StampPrefab { name: self.name, prefab: chosen, at: self.at, orient: self.orient }.apply(ctx)
    }
}

/// The index into `weights` that `roll` falls under, when the weights are
/// laid end to end starting at zero: `roll` below the first is index zero,
/// past it and below the sum of the first two is index one, and so on. A
/// weight of zero is a span nothing falls in, which is how a zero-weighted
/// candidate is never chosen.
///
/// # Panics
/// Panics if `roll` is not below the sum of `weights`; every caller draws
/// it from `0..total` first, so that sum is always the bound `roll` was
/// drawn under.
fn pick_weighted(weights: &[u32], roll: u32) -> usize {
    let mut roll = roll;
    weights
        .iter()
        .position(|&w| {
            if roll < w {
                true
            } else {
                roll -= w;
                false
            }
        })
        .expect("the roll is below the total, so some candidate holds it")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::Chain;
    use crate::context::BaseContext;
    use crate::dungeon::Rooms;
    use rl_core::RunSeed;
    use rl_grid::TileRegistry;

    fn vault(wall: TileId, floor: TileId) -> Prefab {
        Prefab::parse(
            &[
                "#####", //
                "#.$.#", //
                "#...#", //
                "##+##", //
            ],
            |c| match c {
                '#' => Some(wall),
                '.' | '+' => Some(floor),
                _ => None,
            },
        )
        .unwrap()
    }

    /// A piece with no left-right symmetry: the opening is on the left
    /// only, and the mark sits off the centre column. `vault` cannot
    /// stand in for a mirror test, since every row of it reads the same
    /// backwards and its mark sits on the one column a mirror fixes.
    fn lopsided(wall: TileId, floor: TileId) -> Prefab {
        Prefab::parse(
            &[
                "#####", //
                ".$..#", //
                "#####", //
            ],
            |c| match c {
                '#' => Some(wall),
                '.' => Some(floor),
                _ => None,
            },
        )
        .unwrap()
    }

    /// A square with a different mark in each corner, so no rotation or
    /// mirror maps it onto itself. `lopsided` is not enough for a test
    /// that must tell every one of the eight facings apart: its top and
    /// bottom border rows are identical, so its own mirror image already
    /// equals a plain quarter-turn of it, which would hide a `.flipped()`
    /// that had stopped running.
    fn four_marked_corners(floor: TileId) -> Prefab {
        Prefab::parse(
            &[
                "1.2", //
                "...", //
                "4.3", //
            ],
            |c| match c {
                '.' => Some(floor),
                _ => None,
            },
        )
        .unwrap()
    }

    #[test]
    fn parse_keeps_marks_and_transparency() {
        let p = vault(TileId(0), TileId(1));
        assert_eq!((p.width(), p.height()), (5, 4));
        assert_eq!(p.tile(Point::new(2, 1)), None, "the mark is transparent");
        assert_eq!(p.marks(), &[('$', Point::new(2, 1))]);
        assert!(Prefab::parse(&["##", "#"], |_| None).is_err());
    }

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
        let (wall, floor) = (TileId(0), TileId(1));
        let p = lopsided(wall, floor);
        let w = p.width();
        let flipped = p.flipped();
        assert_eq!((flipped.width(), flipped.height()), (w, p.height()), "a mirror keeps the shape");
        let (_, before) = p.marks()[0];
        let (_, after) = flipped.marks()[0];
        assert_eq!(after, Point::new(w - 1 - before.x, before.y));
        assert_ne!(after.x, before.x, "the fixture's mark is off-centre, so a mirror must move it");

        // The opening is on the left in the source piece and the wall is
        // on the right; a mirror swaps which side is which.
        assert_eq!(p.tile(Point::new(0, 1)), Some(floor));
        assert_eq!(p.tile(Point::new(w - 1, 1)), Some(wall));
        assert_eq!(flipped.tile(Point::new(0, 1)), Some(wall));
        assert_eq!(flipped.tile(Point::new(w - 1, 1)), Some(floor));

        assert_eq!(p.flipped().flipped(), p, "twice mirrored is where it started");
    }

    #[test]
    fn stamps_centred_in_a_room_and_reports_marks_in_map_coordinates() {
        let tiles = TileRegistry::standard();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        let mut c = BaseContext::blank(60, 40, tiles, wall);
        Chain::new()
            .then(Rooms { floor, min_size: 8, max_size: 10, ..Default::default() })
            .then(StampPrefab { name: "vault", prefab: vault(wall, floor), at: Placement::InRoom(0), orient: Orient::Fixed })
            .run(&mut c, RunSeed(2))
            .unwrap();
        let room = c.outputs().first::<Room>().unwrap().0;
        let stamped = c.outputs().first::<Stamped>().unwrap();
        assert!(room.contains(stamped.bounds.origin()) && room.contains(Point::new(stamped.bounds.right() - 1, stamped.bounds.bottom() - 1)));
        let (_, coin) = stamped.marks[0];
        assert_eq!(c.terrain().get(coin), Some(floor), "the mark left the room floor showing");
        assert_eq!(c.terrain().get(stamped.bounds.origin()), Some(wall));
    }

    #[test]
    fn off_map_and_undersized_placements_fail() {
        let tiles = TileRegistry::standard();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        let mut c = BaseContext::blank(20, 20, tiles, wall);
        let off = StampPrefab { name: "off", prefab: vault(wall, floor), at: Placement::At(Point::new(18, 18)), orient: Orient::Fixed };
        assert!(Chain::new().then(off).run(&mut c, RunSeed(1)).is_err());
        c.emit(Room(Rect::new(2, 2, 3, 3)));
        let small = StampPrefab { name: "small", prefab: vault(wall, floor), at: Placement::AnyRoom, orient: Orient::Fixed };
        assert!(Chain::new().then(small).run(&mut c, RunSeed(1)).is_err());
    }

    #[test]
    fn an_oriented_stamp_is_the_same_piece_under_one_seed_and_varies_across_seeds() {
        // Determinism first: the same seed lays the same piece down, or a
        // saved run would reload a different map than it saved.
        let first = stamped_marks(RunSeed(7), Orient::TurnedOrMirrored);
        assert_eq!(first, stamped_marks(RunSeed(7), Orient::TurnedOrMirrored), "one seed, one map");

        // Then variety: over a span of seeds an oriented stamp must land its
        // marks in more than one arrangement, or the orientation did nothing.
        let seen: std::collections::BTreeSet<_> = (0..40).map(|s| stamped_marks(RunSeed(s), Orient::TurnedOrMirrored)).collect();
        assert!(seen.len() > 1, "forty seeds laid the piece exactly one way");

        // `four_marked_corners` has a different mark in each corner, so
        // none of the eight facings coincide: mirroring must reach four
        // that turning alone cannot, the four whose corner order is the
        // mirror image of a turned one's. If `.flipped()` stopped running,
        // `TurnedOrMirrored` would draw the same rng calls but only ever
        // land a turned facing, and `seen` would shrink to `turned_only`.
        let turned_only: std::collections::BTreeSet<_> = (0..40).map(|s| stamped_marks(RunSeed(s), Orient::Turned)).collect();
        assert!(seen.len() > turned_only.len(), "mirroring must reach facings a quarter-turn alone cannot");

        // And a fixed stamp faces one way whatever the seed, so every chain
        // that has one today keeps the map it has today.
        let fixed: std::collections::BTreeSet<_> = (0..40).map(|s| stamped_marks(RunSeed(s), Orient::Fixed)).collect();
        assert_eq!(fixed.len(), 1, "a fixed stamp faces the same way under every seed");
    }

    /// Stamps `four_marked_corners` into a fixed room under `seed` and
    /// answers its marks relative to the stamp's own bounds, which is its
    /// facing. `four_marked_corners`, not `lopsided`, is the fixture: a
    /// mirror of `lopsided` lands on one of its own quarter-turns because
    /// its top and bottom border rows are identical, so it cannot tell a
    /// facing mirroring alone reaches apart from one turning alone reaches.
    /// A square with four distinct corner marks has no such accident: no
    /// rotation or reflection maps it onto itself, so all eight facings,
    /// and their four-mark arrangements, are pairwise distinct.
    fn stamped_marks(seed: RunSeed, orient: Orient) -> Vec<(char, Point)> {
        let tiles = TileRegistry::standard();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        let mut c = BaseContext::blank(60, 40, tiles, wall);
        Chain::new()
            .then(Rooms { floor, min_size: 8, max_size: 10, ..Default::default() })
            .then(StampPrefab { name: "vault", prefab: four_marked_corners(floor), at: Placement::InRoom(0), orient })
            .run(&mut c, seed)
            .unwrap();
        let stamped = c.outputs().first::<Stamped>().unwrap();
        let origin = stamped.bounds.origin();
        stamped.marks.iter().map(|(ch, p)| (*ch, Point::new(p.x - origin.x, p.y - origin.y))).collect()
    }

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
            // 60x40, not the crate's usual smaller fixture size: at 40x30 the
            // `Rooms` pass's own room-count floor (its `min_rooms`, unrelated
            // to this pass) sometimes misses by chance in 30 attempts, which
            // is a property of that pass, not of the one under test here.
            let mut c = BaseContext::blank(60, 40, tiles, wall);
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

    #[test]
    fn weights_that_would_overflow_a_u32_sum_fail_the_chain_instead_of_wrapping() {
        let tiles = TileRegistry::standard();
        let wall = tiles.expect("wall");
        let mut c = BaseContext::blank(20, 20, tiles, wall);
        let piece = Prefab::parse(&["#"], |ch| match ch {
            '#' => Some(wall),
            _ => None,
        })
        .unwrap();
        let err = Chain::new()
            .then(StampOneOf { name: "vault", choices: vec![(piece.clone(), u32::MAX), (piece, u32::MAX)], at: Placement::Center, orient: Orient::Fixed })
            .run(&mut c, RunSeed(1))
            .unwrap_err();
        // Named and caught rather than wrapped: a wrapped total could land a
        // roll on the wrong candidate, or on none, silently.
        assert!(format!("{err:?}").contains("vault"));
    }

    #[test]
    fn a_roll_picks_the_candidate_whose_running_weight_it_falls_under_and_never_a_zero_weighted_one() {
        // Weights [3, 1, 0] laid end to end: rolls 0..3 are the first
        // candidate's span, roll 3 is the second's only slot, and the third
        // carries no weight, so no roll in the total's range of 0..4 can
        // ever land on it.
        let weights = [3u32, 1, 0];
        for roll in 0..3 {
            assert_eq!(pick_weighted(&weights, roll), 0, "roll {roll} is under the first weight of 3");
        }
        assert_eq!(pick_weighted(&weights, 3), 1, "roll 3 is the one slot the second weight of 1 covers");
        for roll in 0..4 {
            assert_ne!(pick_weighted(&weights, roll), 2, "a zero weight is never the candidate a roll lands on");
        }
    }
}
