//! What Undermere asks of the pool's pieces once they are laid, beyond
//! what the engine's own passes promise: a stamped piece keeps off the way
//! in.

use super::*;
use rl_engine::rl_core::geometry;
use rl_engine::rl_mapgen::prefab::Stamped;

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

impl Pass<BaseContext> for QuietWayIn {
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
