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
//! [`StampEachRoom`] is the pass that keeps rooms from being left empty: a
//! weighted piece in every room nothing was stamped in, each in a facing
//! [`Orient::facings`] allows and that fits, off the way in and out.
//! A piece's legend maps a character to a [`Cell`], so a mark can paint the
//! tile under it instead of leaving whatever the map had there. A piece
//! [`Prefab::keyed`] carries that key through every stamp as
//! [`Stamped::prefab`], so a game can trace a stamp's marks back to it.
//!
//! Either pass emits where it landed as a [`Stamped`], in map coordinates,
//! so later passes can keep out of it or spawn into it.

use rand::Rng;
use rl_core::{Grid, Grid2D, Point, Rect};
use rl_grid::TileId;

use crate::chain::{BuildError, Pass, Phase};
use crate::context::BuildContext;
use crate::dungeon::Room;

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

/// A piece of map drawn by hand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prefab {
    cells: Grid<Option<TileId>>,
    /// Marks the legend gave a meaning to besides a tile, by character,
    /// in prefab-local coordinates: an entrance, a spawn, a chest.
    marks: Vec<(char, Point)>,
    /// An opaque number its owner set, carried to [`Stamped::prefab`], so
    /// the stamp's marks can be traced back to the definition that gave
    /// them meaning. `None` for a piece nobody keyed.
    key: Option<u32>,
}

impl Prefab {
    /// Parses `rows`, all the same width, with `legend` naming the tile
    /// for a character. A character with no tile is transparent; if it is
    /// not a space it is kept as a mark.
    pub fn parse(rows: &[&str], legend: impl Fn(char) -> Option<TileId>) -> Result<Self, String> {
        Self::parse_cells(rows, |ch| match legend(ch) {
            Some(t) => Cell::Tile(t),
            None if ch == ' ' => Cell::Clear,
            None => Cell::Mark(None),
        })
    }

    /// Parses `rows`, all the same width, with `legend` naming the
    /// [`Cell`] for a character: a tile, a mark that may carry a tile
    /// under it, or transparency.
    pub fn parse_cells(rows: &[&str], legend: impl Fn(char) -> Cell) -> Result<Self, String> {
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
                    Cell::Tile(t) => {
                        cells.set(p, Some(t));
                    }
                    Cell::Mark(under) => {
                        cells.set(p, under);
                        marks.push((ch, p));
                    }
                    Cell::Clear => {}
                }
            }
        }
        Ok(Self { cells, marks, key: None })
    }

    /// This piece, keyed: its stamp reports `key` as [`Stamped::prefab`].
    pub fn keyed(mut self, key: u32) -> Self {
        self.key = Some(key);
        self
    }

    /// The key its owner set, if any.
    pub fn key(&self) -> Option<u32> {
        self.key
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
        Self { cells, marks, key: self.key }
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
        Self { cells, marks, key: self.key }
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
    /// Centred in the emitted [`Room`] with this index, which must exceed
    /// the piece by a cell on every side, so the room's floor runs all
    /// round it and whichever way its opening faces, it opens onto floor.
    InRoom(usize),
    /// Centred in a random emitted [`Room`] that exceeds it by a cell on
    /// every side, as [`InRoom`](Placement::InRoom) requires, and
    /// clear of every [`Stamped`] this chain already emitted: a room an
    /// earlier `AnyRoom` stamp landed in is never chosen again, so two
    /// stamps in one chain never draw over each other and bury a mark
    /// under the next piece's wall.
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
    /// Every facing this policy allows `prefab` to land in: one when
    /// fixed, the four quarter-turns, or those and their mirror images.
    /// A symmetric piece repeats itself in the list, which leaves each
    /// distinct facing equally likely to a uniform pick among them.
    pub fn facings(self, prefab: &Prefab) -> Vec<Prefab> {
        let turns = |p: &Prefab| (0..4).map(|q| p.rotated(q)).collect::<Vec<_>>();
        match self {
            Orient::Fixed => vec![prefab.clone()],
            Orient::Turned => turns(prefab),
            Orient::TurnedOrMirrored => {
                let mut all = turns(prefab);
                all.extend(turns(&prefab.flipped()));
                all
            }
        }
    }

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
    /// The stamped piece's [`Prefab::key`], so whoever fills its marks finds
    /// the definition that gave them meaning; `None` for an unkeyed piece.
    pub prefab: Option<u32>,
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
        // A room must exceed the piece by a cell on every side, so the
        // piece is laid with the room's floor all round it: one exactly
        // its size would put its walls on the room's edge, over the room's
        // doorways, with an opening that may face the room's own wall.
        let fits = |r: &Rect| r.width >= w + 2 && r.height >= h + 2;
        let origin = match self.at {
            Placement::Center => centred(bounds),
            Placement::At(p) => p,
            Placement::InRoom(i) => {
                let room = ctx.outputs().iter::<Room>().nth(i).ok_or_else(|| BuildError::new(self.name, format!("no room {i}")))?.0;
                if !fits(&room) {
                    return Err(BuildError::new(
                        self.name,
                        format!("room {i} is {}x{}, the prefab {w}x{h} and a cell of floor all round", room.width, room.height),
                    ));
                }
                centred(room)
            }
            Placement::AnyRoom => {
                let taken: Vec<Rect> = ctx.outputs().iter::<Stamped>().map(|s| s.bounds).collect();
                let free = |r: &Rect| taken.iter().all(|t| r.intersection(t).is_none());
                let rooms: Vec<Rect> = ctx.outputs().iter::<Room>().map(|r| r.0).filter(fits).filter(free).collect();
                if rooms.is_empty() {
                    return Err(BuildError::new(self.name, format!("no free room holds {w}x{h} with a cell of floor all round")));
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
        ctx.emit(Stamped { bounds: placed, marks, prefab: prefab.key() });
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

/// Stamps one of several pieces into every room nothing was stamped in.
///
/// This is the pass that keeps a map from having empty rooms: after the
/// pieces a game needs are down and the ways in and out are chosen, every
/// emitted [`Room`] that no [`Stamped`] touches gets a piece drawn by
/// weight from those that fit it. A room nothing fits is left as it was,
/// since a room too small to furnish is a smaller map, not a failed one;
/// only a list with no weight at all fails the chain, as it does for
/// [`StampOneOf`].
///
/// A piece fits a room in a facing [`Orient`] allows when the room exceeds
/// it by a cell on every side, the rule [`Placement::AnyRoom`] keeps, so
/// a corridor meeting the room anywhere on its edge still reaches every
/// side of it. It is laid centred, and slid to the nearest position where
/// it paints and marks neither the [`StartPoint`](crate::passes::StartPoint)
/// nor the [`ExitPoint`](crate::dungeon::ExitPoint), which is why the pass
/// runs in [`Phase::Finish`]: what stands at the way in must not land on
/// a wall or share a cell with a slot. A cell the piece leaves showing may
/// lie over either, so an open piece stays centred more often than not.
/// Each facing that fits is equally likely, so a piece that only fits
/// turned is laid turned rather than refused.
///
/// It reads rooms and nothing else, so it furnishes what [`Rooms`](crate::dungeon::Rooms)
/// and [`Bsp`](crate::dungeon::Bsp) carve and nothing on a cave or any map
/// whose builder emits no [`Room`].
#[derive(Debug, Clone)]
pub struct StampEachRoom {
    /// A stable name, so two passes in one chain draw different streams.
    pub name: &'static str,
    /// The pieces and their weights.
    pub choices: Vec<(Prefab, u32)>,
    /// How a piece may be turned before it lands.
    pub orient: Orient,
}

impl<C: BuildContext> Pass<C> for StampEachRoom {
    fn name(&self) -> &'static str {
        self.name
    }
    fn phase(&self) -> Phase {
        Phase::Finish
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        if self.choices.iter().try_fold(0u32, |sum, (_, w)| sum.checked_add(*w)).is_none_or(|total| total == 0) {
            return Err(BuildError::new(self.name, "no candidate carries weight, or the weights overflow a u32".to_string()));
        }
        // Every candidate's facings once, rather than once per room.
        let facings: Vec<Vec<Prefab>> = self.choices.iter().map(|(p, _)| self.orient.facings(p)).collect();
        let keep: Vec<Point> =
            ctx.outputs().iter::<crate::passes::StartPoint>().map(|s| s.0).chain(ctx.outputs().iter::<crate::dungeon::ExitPoint>().map(|e| e.0)).collect();
        let taken: Vec<Rect> = ctx.outputs().iter::<Stamped>().map(|s| s.bounds).collect();
        let rooms: Vec<Rect> = ctx.outputs().iter::<Room>().map(|r| r.0).filter(|r| taken.iter().all(|t| r.intersection(t).is_none())).collect();
        for room in rooms {
            // What could go here: each candidate with weight, and the
            // facings of it that fit, each with where it would be laid.
            let fitting: Vec<(u32, Vec<(&Prefab, Point)>)> = self
                .choices
                .iter()
                .zip(&facings)
                .filter(|((_, w), _)| *w > 0)
                .map(|((_, w), faces)| (*w, faces.iter().filter_map(|f| laid_in(room, f, &keep).map(|at| (f, at))).collect::<Vec<_>>()))
                .filter(|(_, faces)| !faces.is_empty())
                .collect();
            if fitting.is_empty() {
                continue;
            }
            let weights: Vec<u32> = fitting.iter().map(|(w, _)| *w).collect();
            let total: u32 = weights.iter().sum();
            let roll = ctx.rng().random_range(0..total);
            let faces = &fitting[pick_weighted(&weights, roll)].1;
            let (piece, origin) = faces[ctx.rng().random_range(0..faces.len())];
            piece.stamp(ctx.terrain_mut(), origin);
            let marks = piece.marks().iter().map(|(c, p)| (*c, origin + *p)).collect();
            ctx.emit(Stamped { bounds: Rect::new(origin.x, origin.y, piece.width(), piece.height()), marks, prefab: piece.key() });
        }
        Ok(())
    }
}

/// Where `piece` is laid in `room`: centred, or else the position nearest
/// the centre that paints and marks none of `keep`, always with a cell of
/// the room all round it. `None` when the room is too small or every
/// position paints or marks something kept.
fn laid_in(room: Rect, piece: &Prefab, keep: &[Point]) -> Option<Point> {
    let (w, h) = (piece.width(), piece.height());
    if room.width < w + 2 || room.height < h + 2 {
        return None;
    }
    let centre = Point::new(room.x + (room.width - w) / 2, room.y + (room.height - h) / 2);
    // A cell the piece leaves showing is no cover: only what it paints or
    // marks may not land on something kept.
    let clear = |o: &Point| {
        keep.iter().all(|k| {
            let local = Point::new(k.x - o.x, k.y - o.y);
            !Rect::new(0, 0, w, h).contains(local) || (piece.tile(local).is_none() && piece.marks().iter().all(|(_, m)| *m != local))
        })
    };
    let xs = room.x + 1..=room.x + room.width - w - 1;
    let ys = room.y + 1..=room.y + room.height - h - 1;
    // Nearest the centre first, then top to bottom and left to right, so
    // a tie always slides the same way.
    ys.flat_map(|y| xs.clone().map(move |x| Point::new(x, y))).filter(clear).min_by_key(|o| ((o.x - centre.x).abs() + (o.y - centre.y).abs(), o.y, o.x))
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
        Chain::new()
            .then(StampPrefab { name: "keyed", prefab: piece, at: Placement::At(Point::new(2, 2)), orient: Orient::TurnedOrMirrored })
            .run(&mut ctx, RunSeed(1))
            .unwrap();
        let stamped = ctx.outputs().first::<Stamped>().unwrap();
        assert_eq!(stamped.prefab, Some(7));
    }

    #[test]
    fn a_piece_parsed_from_a_tile_legend_has_no_key() {
        let r = TileRegistry::standard();
        let wall = r.expect("wall");
        assert_eq!(Prefab::parse(&["#A#"], |c| (c == '#').then_some(wall)).unwrap().key(), None);
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

    /// A piece laid in a room always has the room's floor all round it: a
    /// room no bigger than the piece would put the piece's walls on the
    /// room's own edge, where an opening facing the room's wall leads
    /// nowhere and the room's doorways are walled over. So a room must
    /// exceed the piece by a cell on every side, for `InRoom` and for
    /// `AnyRoom` alike.
    #[test]
    fn a_piece_laid_in_a_room_has_floor_all_round_it_and_a_room_its_own_size_is_refused() {
        let tiles = TileRegistry::standard();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        let room = |c: &mut BaseContext, r: Rect| {
            for p in r.cells() {
                c.terrain_mut().set(p, floor);
            }
            c.emit(Room(r));
        };
        // The vault is 5x4: a room of exactly 5x4, and one of 6x5, still
        // short of a cell on one side each way, are both refused.
        for (w, h) in [(5, 4), (6, 5), (7, 5), (6, 6)] {
            let mut c = BaseContext::blank(30, 20, tiles.clone(), wall);
            room(&mut c, Rect::new(2, 2, w, h));
            let any = StampPrefab { name: "any", prefab: vault(wall, floor), at: Placement::AnyRoom, orient: Orient::Fixed };
            assert!(Chain::new().then(any).run(&mut c, RunSeed(1)).is_err(), "AnyRoom took a {w}x{h} room for a 5x4 piece");
            let one = StampPrefab { name: "one", prefab: vault(wall, floor), at: Placement::InRoom(0), orient: Orient::Fixed };
            assert!(Chain::new().then(one).run(&mut c, RunSeed(1)).is_err(), "InRoom took a {w}x{h} room for a 5x4 piece");
        }
        let mut c = BaseContext::blank(30, 20, tiles.clone(), wall);
        room(&mut c, Rect::new(2, 2, 7, 6));
        let any = StampPrefab { name: "any", prefab: vault(wall, floor), at: Placement::AnyRoom, orient: Orient::Fixed };
        Chain::new().then(any).run(&mut c, RunSeed(1)).expect("a 7x6 room holds a 5x4 piece with a cell to spare all round");
        let placed = c.outputs().first::<Stamped>().unwrap().bounds;
        let ring = Rect::new(placed.x - 1, placed.y - 1, placed.width + 2, placed.height + 2);
        for p in ring.cells().filter(|p| !placed.contains(*p)) {
            assert_eq!(c.terrain().get(p), Some(floor), "the ring cell {p:?} around the piece is floor");
        }
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
        let text = format!("{err:?}");
        assert!(text.contains("vault"));
        assert!(text.contains("overflow"), "the error says what went wrong, not just which pass: {text}");
    }

    /// Three `AnyRoom` stamps in one chain, over a span of seeds, never
    /// choose the same room twice: if they did, the second stamp would
    /// draw over the first's tiles while the first's mark was still
    /// reported, and an item's spot could land on a wall.
    #[test]
    fn any_room_stamps_in_one_chain_never_land_on_each_other() {
        let wall = TileRegistry::standard().expect("wall");
        let piece = |mark: char| {
            let middle = format!("#{mark}#");
            Prefab::parse(&["###", &middle, "###"], |c| match c {
                '#' => Some(wall),
                _ => None,
            })
            .unwrap()
        };
        for seed in 0..200 {
            let tiles = TileRegistry::standard();
            let floor = tiles.expect("floor");
            // 70x40 with rooms 5 to 11, every one big enough for the 3x3
            // piece and its cell of floor all round: at smaller maps or a
            // higher minimum, `Rooms` itself sometimes misses its room
            // count by chance, which is that pass's own property, not
            // this one's.
            let mut c = BaseContext::blank(70, 40, tiles, wall);
            Chain::new()
                .then(Rooms { floor, min_size: 5, max_size: 11, ..Default::default() })
                .then(StampPrefab { name: "a", prefab: piece('a'), at: Placement::AnyRoom, orient: Orient::Fixed })
                .then(StampPrefab { name: "b", prefab: piece('b'), at: Placement::AnyRoom, orient: Orient::Fixed })
                .then(StampPrefab { name: "c", prefab: piece('c'), at: Placement::AnyRoom, orient: Orient::Fixed })
                .run(&mut c, RunSeed(seed))
                .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
            let bounds: Vec<Rect> = c.outputs().iter::<Stamped>().map(|s| s.bounds).collect();
            for i in 0..bounds.len() {
                for j in (i + 1)..bounds.len() {
                    assert!(bounds[i].intersection(&bounds[j]).is_none(), "seed {seed}: stamps {i} and {j} overlap: {:?} and {:?}", bounds[i], bounds[j]);
                }
            }
        }
    }

    /// A blank map with floor carved for each of `rooms`, each emitted as
    /// a [`Room`], for a test that needs rooms of an exact size.
    fn with_rooms(rooms: &[Rect]) -> (BaseContext, TileId, TileId) {
        let tiles = TileRegistry::standard();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        let mut c = BaseContext::blank(40, 30, tiles, wall);
        for r in rooms {
            for p in r.cells() {
                c.terrain_mut().set(p, floor);
            }
            c.emit(Room(*r));
        }
        (c, wall, floor)
    }

    /// A 3x3 block of wall with a mark in the middle, keyed by `mark`, so
    /// a test can tell which piece landed where.
    fn block(wall: TileId, mark: char) -> Prefab {
        let middle = format!("#{mark}#");
        Prefab::parse(&["###", &middle, "###"], |c| match c {
            '#' => Some(wall),
            _ => None,
        })
        .unwrap()
    }

    /// Over a span of seeds on a full dungeon, every room gets exactly one
    /// piece, and the room an earlier stamp took gets no second one: the
    /// property the pass exists for, that no room is left empty.
    #[test]
    fn every_room_is_furnished_once_and_a_room_already_stamped_is_left_alone() {
        for seed in 0..100 {
            let tiles = TileRegistry::standard();
            let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
            let mut c = BaseContext::blank(70, 40, tiles, wall);
            Chain::new()
                .then(Rooms { floor, min_size: 5, max_size: 11, ..Default::default() })
                .then(StampPrefab { name: "first", prefab: block(wall, 'f'), at: Placement::AnyRoom, orient: Orient::Fixed })
                .then(StampEachRoom { name: "each", choices: vec![(block(wall, 'a'), 1), (block(wall, 'b'), 1)], orient: Orient::Fixed })
                .run(&mut c, RunSeed(seed))
                .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
            let stamps: Vec<Rect> = c.outputs().iter::<Stamped>().map(|s| s.bounds).collect();
            for room in c.outputs().iter::<Room>() {
                let inside = stamps.iter().filter(|b| room.0.intersection(b).is_some()).count();
                assert_eq!(inside, 1, "seed {seed}: room {:?} holds {inside} pieces", room.0);
            }
        }
    }

    /// A room too small for every candidate is left as it was, and the
    /// chain does not fail: an unfurnished room is a plainer map, where a
    /// missing required piece is a broken one.
    #[test]
    fn a_room_nothing_fits_is_left_alone_without_failing_the_chain() {
        let (mut c, wall, floor) = with_rooms(&[Rect::new(2, 2, 4, 4), Rect::new(10, 2, 5, 5)]);
        Chain::new().then(StampEachRoom { name: "each", choices: vec![(block(wall, 'a'), 1)], orient: Orient::Fixed }).run(&mut c, RunSeed(1)).unwrap();
        let stamps: Vec<Rect> = c.outputs().iter::<Stamped>().map(|s| s.bounds).collect();
        assert_eq!(stamps, vec![Rect::new(11, 3, 3, 3)], "only the 5x5 room takes the 3x3 piece");
        assert!(Rect::new(2, 2, 4, 4).cells().all(|p| c.terrain().get(p) == Some(floor)), "the 4x4 room is untouched");
    }

    /// A piece that fits a room only when turned is laid turned when the
    /// stamp may turn it, and not at all when it is fixed.
    #[test]
    fn a_piece_that_fits_only_turned_is_laid_turned_and_a_fixed_one_is_not_laid() {
        let bar = |wall: TileId| Prefab::parse(&["#####"], |c| (c == '#').then_some(wall)).unwrap();
        let tall = Rect::new(2, 2, 3, 7);
        let (mut c, wall, _) = with_rooms(&[tall]);
        Chain::new().then(StampEachRoom { name: "each", choices: vec![(bar(wall), 1)], orient: Orient::Fixed }).run(&mut c, RunSeed(1)).unwrap();
        assert_eq!(c.outputs().iter::<Stamped>().count(), 0, "a fixed 5x1 bar does not fit a room 3 wide");
        let (mut c, wall, _) = with_rooms(&[tall]);
        Chain::new().then(StampEachRoom { name: "each", choices: vec![(bar(wall), 1)], orient: Orient::Turned }).run(&mut c, RunSeed(1)).unwrap();
        let laid = c.outputs().first::<Stamped>().expect("the bar is laid turned").bounds;
        assert_eq!((laid.width, laid.height), (1, 5));
    }

    /// The way in and the way out are never under a piece: a piece whose
    /// centred place covers either slides to the nearest place that does
    /// not, so the room is still furnished and nothing lands on a wall.
    #[test]
    fn a_piece_never_covers_the_start_or_the_exit_and_slides_off_them() {
        let room = Rect::new(2, 2, 9, 9);
        let (mut c, wall, _) = with_rooms(&[room]);
        // The centre of the room, which a centred 3x3 piece would cover.
        let start = Point::new(6, 6);
        c.emit(crate::passes::StartPoint(start));
        c.emit(crate::dungeon::ExitPoint(Point::new(7, 6)));
        Chain::new().then(StampEachRoom { name: "each", choices: vec![(block(wall, 'a'), 1)], orient: Orient::Fixed }).run(&mut c, RunSeed(1)).unwrap();
        let laid = c.outputs().first::<Stamped>().expect("the room is still furnished").bounds;
        assert!(!laid.contains(start) && !laid.contains(Point::new(7, 6)), "{laid:?} covers the way in or out");
        assert!(laid.x > room.x && laid.y > room.y && laid.right() < room.right() && laid.bottom() < room.bottom(), "{laid:?} keeps a cell of floor all round");
    }

    /// A piece that leaves a cell showing may lie over the way in, so an
    /// open piece stays centred rather than sliding off something it never
    /// touches.
    #[test]
    fn a_piece_stays_centred_over_the_start_when_it_leaves_that_cell_showing() {
        let room = Rect::new(2, 2, 7, 7);
        let (mut c, wall, _) = with_rooms(&[room]);
        c.emit(crate::passes::StartPoint(Point::new(5, 5)));
        let ring = Prefab::parse(&["###", "# #", "###"], |c| (c == '#').then_some(wall)).unwrap();
        Chain::new().then(StampEachRoom { name: "each", choices: vec![(ring, 1)], orient: Orient::Fixed }).run(&mut c, RunSeed(1)).unwrap();
        assert_eq!(c.outputs().first::<Stamped>().expect("the room is furnished").bounds, Rect::new(4, 4, 3, 3));
    }

    /// A candidate with no weight is never laid, and a list where nothing
    /// carries weight fails the chain rather than furnishing nothing.
    #[test]
    fn an_unweighted_piece_is_never_laid_and_a_list_with_no_weight_fails() {
        for seed in 0..40 {
            let (mut c, wall, _) = with_rooms(&[Rect::new(2, 2, 5, 5), Rect::new(10, 2, 5, 5), Rect::new(2, 10, 5, 5)]);
            let choices = vec![(block(wall, 'a'), 1), (block(wall, 'z'), 0)];
            Chain::new().then(StampEachRoom { name: "each", choices, orient: Orient::Fixed }).run(&mut c, RunSeed(seed)).unwrap();
            assert!(c.outputs().iter::<Stamped>().all(|s| s.marks[0].0 == 'a'), "seed {seed}: the unweighted piece was laid");
        }
        let (mut c, wall, _) = with_rooms(&[Rect::new(2, 2, 5, 5)]);
        let none = StampEachRoom { name: "each", choices: vec![(block(wall, 'a'), 0)], orient: Orient::Fixed };
        assert!(Chain::new().then(none).run(&mut c, RunSeed(1)).is_err());
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
