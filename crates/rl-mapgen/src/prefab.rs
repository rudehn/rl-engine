//! Hand-drawn pieces stamped into a generated map.
//!
//! A [`Prefab`] is rows of characters and a legend from character to tile.
//! A character the legend does not know is transparent: the generated
//! map shows through, so a prefab can be an irregular shape. The stamp
//! pass emits where it landed as a [`Stamped`] so later passes can keep
//! out of it or spawn into it.

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
}

impl<C: BuildContext> Pass<C> for StampPrefab {
    fn name(&self) -> &'static str {
        self.name
    }
    fn phase(&self) -> Phase {
        Phase::Structures
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        let (w, h) = (self.prefab.width(), self.prefab.height());
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
        self.prefab.stamp(ctx.terrain_mut(), origin);
        let marks = self.prefab.marks().iter().map(|(c, p)| (*c, origin + *p)).collect();
        ctx.emit(Stamped { bounds: placed, marks });
        Ok(())
    }
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

    #[test]
    fn parse_keeps_marks_and_transparency() {
        let p = vault(TileId(0), TileId(1));
        assert_eq!((p.width(), p.height()), (5, 4));
        assert_eq!(p.tile(Point::new(2, 1)), None, "the mark is transparent");
        assert_eq!(p.marks(), &[('$', Point::new(2, 1))]);
        assert!(Prefab::parse(&["##", "#"], |_| None).is_err());
    }

    #[test]
    fn stamps_centred_in_a_room_and_reports_marks_in_map_coordinates() {
        let tiles = TileRegistry::standard();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        let mut c = BaseContext::blank(60, 40, tiles, wall);
        Chain::new()
            .then(Rooms { floor, min_size: 8, max_size: 10, ..Default::default() })
            .then(StampPrefab { name: "vault", prefab: vault(wall, floor), at: Placement::InRoom(0) })
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
        let off = StampPrefab { name: "off", prefab: vault(wall, floor), at: Placement::At(Point::new(18, 18)) };
        assert!(Chain::new().then(off).run(&mut c, RunSeed(1)).is_err());
        c.emit(Room(Rect::new(2, 2, 3, 3)));
        let small = StampPrefab { name: "small", prefab: vault(wall, floor), at: Placement::AnyRoom };
        assert!(Chain::new().then(small).run(&mut c, RunSeed(1)).is_err());
    }
}
