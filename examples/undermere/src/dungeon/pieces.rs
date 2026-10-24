//! What Undermere asks of the pool's pieces once they are laid, beyond
//! what the engine's own passes promise: a stamped piece keeps off the way
//! in, and a grown one is walled and floored as the floor round it is.

use super::*;
use atglyph::mapgen::prefab::Stamped;
use atglyph::primitives::geometry;

use super::lairs::{FINAL_BOSSES, MINI_BOSSES};

/// Fails the floor when a piece stamped inside an area stands nearer the
/// way in than [`MIN_DISTANCE_FROM_ENTRY`]: the guards, sleepers, traps
/// and prizes a risk-and-reward piece is made of are the floor's own
/// threat, and the population keeps every monster that far from where a
/// floor is entered, so a piece does as well.
///
/// `StampPicks` lays its pieces after the way in is drawn, by an area's
/// seclusion and never by how near the way in it is, so this is where the
/// distance is held: a grown piece is already kept clear by
/// [`RandomStart::clear_of_stamps`], and a one-cell stamp is a dressing's
/// slot, which keeps off the way in by its own rule. A floor that fails
/// is grown again from the next attempt, with the same picks.
pub(super) struct QuietWayIn;

impl Pass for QuietWayIn {
    fn name(&self) -> &'static str {
        "quiet_way_in"
    }

    fn phase(&self) -> Phase {
        Phase::Finish
    }

    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let err = |reason: String| BuildError::new("quiet_way_in", reason);
        let start = ctx.outputs().first::<StartPoint>().ok_or_else(|| err("no start point emitted before it".into()))?.0;
        for stamp in ctx.outputs().iter::<Stamped>().filter(|s| s.bounds.width * s.bounds.height > 1) {
            let d = geometry::chebyshev(start, geometry::clamp_to(start, stamp.bounds));
            if d < MIN_DISTANCE_FROM_ENTRY {
                return Err(err(format!("a piece at {:?} stands {d} from the way in", stamp.bounds)));
            }
        }
        Ok(())
    }
}

/// Paints every risk-and-reward and reward piece grown as an area of its
/// own in the floor's own wall and floor: a piece is drawn in `hall wall`
/// and `hall floor`, which this lays the floor's own over, so a vault in
/// the caves is walled in cave wall, and an entry growth never joined,
/// left as the rock it was drawn over, reads as the rest of its wall
/// rather than a patch of some other stone. A piece joined by a door is
/// joined by a carved one, unlike the floor's others, so it is seen before
/// it is understood. What a piece draws in a tile of its own, water,
/// straw, planks, is its own and kept. A boss's lair and the throne are
/// drawn in their own stone and passed over.
pub(super) struct PieceLook {
    /// The floor's own wall.
    pub(super) wall: TileId,
    /// The floor's own floor.
    pub(super) floor: TileId,
    /// What a piece's wall is drawn in.
    pub(super) piece_wall: TileId,
    /// What a piece's floor is drawn in.
    pub(super) piece_floor: TileId,
    /// The floor's own door, which a piece joined by a door is joined by.
    pub(super) door: TileId,
    /// What a piece's own door is carved as.
    pub(super) carved: TileId,
}

impl Pass for PieceLook {
    fn name(&self) -> &'static str {
        "piece_look"
    }

    fn phase(&self) -> Phase {
        Phase::Connect
    }

    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let bosses: Vec<&str> = MINI_BOSSES.iter().map(|b| b.name).chain(FINAL_BOSSES.iter().map(|b| b.piece)).collect();
        let grown: Vec<&Area> = ctx.outputs().iter::<Area>().filter(|a| a.entries.is_some() && !bosses.contains(&a.shape.as_str())).collect();
        let (pieces, entries): (Vec<GridRect>, Vec<Point>) =
            (grown.iter().map(|a| a.bounds).collect(), grown.iter().flat_map(|a| a.entries.iter().flatten().copied()).collect());
        let tables = ctx.tiles().tables();
        for at in entries {
            match ctx.terrain().get(at) {
                Some(t) if t == self.door => {
                    ctx.terrain_mut().set(at, self.carved);
                }
                // An entry growth never joined is the rock it was drawn
                // over, which a piece walled in a stone of its own, as the
                // gatehouse's curtain is, shows as a patch of the floor's
                // own wall in its face: it takes the wall beside it.
                Some(t) if t == self.wall => {
                    let own = Direction::CARDINALS
                        .iter()
                        .filter_map(|d| ctx.terrain().get(at + d.offset()))
                        .find(|n| *n != self.wall && *n != self.piece_wall && !tables.passable[n.index()] && tables.opaque[n.index()]);
                    if let Some(own) = own {
                        ctx.terrain_mut().set(at, own);
                    }
                }
                _ => {}
            }
        }
        for b in pieces {
            let around = GridRect::new(b.x - 1, b.y - 1, b.width + 2, b.height + 2);
            for p in around.cells() {
                match ctx.terrain().get(p) {
                    Some(t) if t == self.piece_wall => {
                        ctx.terrain_mut().set(p, self.wall);
                    }
                    Some(t) if t == self.piece_floor => {
                        ctx.terrain_mut().set(p, self.floor);
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}
