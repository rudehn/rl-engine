//! Openings a place hides: a loop's shortcut or a dead end's one door,
//! walled over and recorded, so a game can let them be found.
//!
//! [`HideOpenings`] chooses the openings that are safe to hide, walls
//! them over and publishes a [`Secret`] for every cell it walled. Finding
//! one is the other half and is not this crate's: the generator only says
//! what it hid, the way it says where the way in is.
//!
//! Safe means two things only. A loop's opening, whose closing leaves
//! every area joined to every other it was joined to, so a secret door is
//! a shortcut and never the only way on; or the one opening of a dead-end
//! room that holds neither the way in nor the way down, so a hidden room
//! is a pocket. Neither is ever an opening joined locked or sealed, whose lock
//! is already what keeps it, nor an opening of a piece painted from a
//! prefab, whose entries were drawn by hand. And nothing is hidden that
//! makes the walk from the way in to the way down any longer, so a player
//! who never finds a secret still walks the place as it was grown.

use rand::Rng;
use rand::seq::SliceRandom;
use rl_core::{Grid2D, Point};
use rl_grid::{BitGrid, Terrain, TileId, TileTables};

use crate::accrete::{Area, Join, Opening};
use crate::chain::{BuildError, Pass, Phase};
use crate::context::BuildContext;
use crate::dungeon::{ExitPoint, Room, walk_through_doors};
use crate::passes::StartPoint;
use crate::survey::Surveyed;

/// One cell a place hid: wall now, and what it is once found.
///
/// Every cell of one opening shares a [`group`](Self::group), so a mouth
/// wider than a cell is found whole rather than a cell at a time. A
/// hidden room is the one of [`joins`](Self::joins) whose
/// [`AreaFacts::dead_end`](crate::AreaFacts::dead_end) is set; a loop's
/// shortcut joins two areas neither of which is, since a dead end's one
/// door is never part of a loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Secret {
    /// Where.
    pub at: Point,
    /// The tile it really is, laid back when it is found.
    pub is: TileId,
    /// Percent chance a turn of being spotted while in sight, the unit a
    /// hidden prop's chance is given in: [`HideOpenings::loop_spot`] or
    /// [`HideOpenings::room_spot`], by what it hides.
    pub spot: u8,
    /// The opening it hides, numbered from nought in the order they were
    /// hidden.
    pub group: u32,
    /// The two areas its opening joins, by [`Area::index`].
    pub joins: (usize, usize),
}

/// Walls over openings that are safe to hide and publishes a [`Secret`]
/// for each cell walled.
///
/// The hidden rooms come first, since a place may need them: a count
/// drawn from [`rooms`](Self::rooms), raised to
/// [`at_least_rooms`](Self::at_least_rooms), of dead ends the
/// [`Surveyed`] names, each holding neither way and each a room: an area
/// with a [`Room`] its growth emitted on its floor, as
/// [`Accrete`](crate::Accrete) emits one for every shape that
/// [`is_room`](crate::Shape::is_room). A dead-end passage is never hidden,
/// since what a hidden room is for, something stood inside it, needs a
/// room to stand in. Then a count drawn from
/// [`loops`](Self::loops) of loops' openings. The candidates of each are
/// tried in an order drawn from the pass's stream, and each is kept only
/// when it is safe with every opening hidden before it already shut. A
/// place with fewer safe openings than were drawn hides what it has, and
/// fails only when it hid fewer rooms than `at_least_rooms`, for
/// [`Chain::run_retrying`](crate::Chain::run_retrying) to grow again.
/// Counts are of openings, not cells.
///
/// Safe is checked on the floor and not on the area graph alone, since a
/// bridge joins what no [`Opening`] names. Two walks are taken from the
/// way in, both the walk [`FarthestExit`](crate::dungeon::FarthestExit)
/// draws the way down by: one with every lock and seal shut and one with
/// all of them opened. A loop's opening is kept when neither walk loses a
/// cell but its own, the areas stay as joined as they were, and the walk
/// to the way down costs what it did. A dead end's door is kept when both
/// walks lose exactly its floor, which also keeps any room behind a lock
/// from being hidden twice over.
///
/// A walled cell takes the tile of the wall its opening stands in: the
/// first cell beside it, square to the way through, that blocks both
/// walking and sight. An opening with no such cell beside it is not
/// hidden, since a secret that does not look like the wall round it is
/// not one.
///
/// Runs in [`Phase::Finish`], after [`Survey`](crate::Survey), whose dead
/// ends it reads and whose [`Surveyed`] it leaves as it was: what that
/// says describes the place with every secret found. A pass that stands
/// something behind a secret runs after this one and reads both.
#[derive(Debug, Clone, Copy)]
pub struct HideOpenings {
    /// How many loops' openings to try to hide: a count drawn evenly from
    /// this range, fewer when the place has fewer safe ones.
    pub loops: (u32, u32),
    /// How many dead-end rooms' doors to try to hide: a count drawn evenly
    /// from this range, fewer when the place has fewer safe ones, but
    /// never fewer than [`at_least_rooms`](Self::at_least_rooms) without
    /// failing.
    pub rooms: (u32, u32),
    /// The [`Secret::spot`] of every cell of a loop's opening.
    pub loop_spot: u8,
    /// The [`Secret::spot`] of every cell of a hidden room's door.
    pub room_spot: u8,
    /// The fewest hidden rooms the place may have; a place with fewer safe
    /// dead ends fails. Nought for a place that may have none.
    pub at_least_rooms: u32,
}

/// The cells each walk from the way in reaches as the place now stands,
/// and what the walk to the way down costs.
struct Walks {
    shut: BitGrid,
    open: BitGrid,
    to_exit: Option<i32>,
}

impl Walks {
    fn of<C: BuildContext>(ctx: &C, start: Point, exit: Option<Point>) -> Self {
        let (w, h) = (ctx.terrain().width(), ctx.terrain().height());
        let reached = |locks_shut: bool| {
            let walk = walk_through_doors(ctx, start, locks_shut);
            let mut cells = BitGrid::new(w, h);
            for (p, _) in walk.iter() {
                cells.insert(p);
            }
            (cells, exit.and_then(|e| walk.value(e)))
        };
        let (shut, to_exit) = reached(true);
        // With nothing locked or sealed the two walks are one walk. A
        // place is walked for every opening it tries to hide, and walking
        // it twice each time was a fifth of what growing one cost.
        let open = if crate::dungeon::lock_cells(ctx).is_clear() { shut.clone() } else { reached(false).0 };
        Self { shut, open, to_exit }
    }
}

/// What `before` reached and `after` does not, but for `own`, in the
/// grid's row order, so two such lists compare as sets.
fn lost(before: &BitGrid, after: &BitGrid, own: &[Point]) -> Vec<Point> {
    before.iter().filter(|p| !after.contains(*p) && !own.contains(p)).collect()
}

/// The tile of the wall `o` stands in, if it stands in one.
fn wall_beside(terrain: &Terrain, tables: &TileTables, o: &Opening) -> Option<TileId> {
    let blocks = |t: &TileId| !tables.passable[t.index()] && tables.opaque[t.index()];
    o.cells.iter().flat_map(|c| o.facing.perpendicular().map(|d| *c + d.offset())).filter(|n| !o.cells.contains(n)).filter_map(|n| terrain.get(n)).find(blocks)
}

/// How many parts `count` areas fall into, joined by every opening not
/// `gone`. Union by walking, since the graph is small and built once a
/// try.
fn parts(count: usize, openings: &[Opening], gone: &[bool]) -> usize {
    let mut part = vec![usize::MAX; count];
    let mut found = 0;
    for root in 0..count {
        if part[root] != usize::MAX {
            continue;
        }
        part[root] = found;
        let mut stack = vec![root];
        while let Some(a) = stack.pop() {
            for (o, _) in openings.iter().zip(gone).filter(|(_, g)| !**g) {
                let next = if o.areas.0 == a {
                    o.areas.1
                } else if o.areas.1 == a {
                    o.areas.0
                } else {
                    continue;
                };
                if next < count && part[next] == usize::MAX {
                    part[next] = found;
                    stack.push(next);
                }
            }
        }
        found += 1;
    }
    found
}

/// Walls `cells` over with `wall`, handing back what each was.
fn wall_over<C: BuildContext>(ctx: &mut C, cells: &[Point], wall: TileId) -> Vec<TileId> {
    cells
        .iter()
        .map(|c| {
            let was = ctx.terrain().get(*c).expect("an opening's cells are on the map");
            ctx.terrain_mut().set(*c, wall);
            was
        })
        .collect()
}

/// Lays back on `cells` the tiles [`wall_over`] handed back.
fn lay_back<C: BuildContext>(ctx: &mut C, cells: &[Point], was: Vec<TileId>) {
    for (c, t) in cells.iter().zip(was) {
        ctx.terrain_mut().set(*c, t);
    }
}

impl<C: BuildContext> Pass<C> for HideOpenings {
    fn name(&self) -> &'static str {
        "hide_openings"
    }
    fn phase(&self) -> Phase {
        Phase::Finish
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        let err = |reason: String| BuildError::new("hide_openings", reason);
        for (what, (lo, hi)) in [("loops", self.loops), ("rooms", self.rooms)] {
            if lo > hi {
                return Err(err(format!("{what} run from {lo} down to {hi}")));
            }
        }
        let start = ctx.outputs().first::<StartPoint>().ok_or_else(|| err("no start point emitted before it".into()))?.0;
        let exit = ctx.outputs().first::<ExitPoint>().map(|e| e.0);
        let surveyed = ctx.outputs().first::<Surveyed>().ok_or_else(|| err("no survey emitted before it".into()))?;
        let dead_ends: Vec<bool> = surveyed.areas.iter().map(|a| a.dead_end).collect();
        let areas: Vec<Area> = ctx.outputs().iter::<Area>().cloned().collect();
        let openings: Vec<Opening> = ctx.outputs().iter::<Opening>().cloned().collect();
        let count = areas.iter().map(|a| a.index + 1).max().unwrap_or(0);
        let tables = ctx.tiles().tables();
        let ways: Vec<Point> = std::iter::once(start).chain(exit).collect();
        let painted = |i: usize| areas.iter().any(|a| a.index == i && a.entries.is_some());
        // Pieces' floors never share a cell, so a room's rectangle on an
        // area's floor is that area's own.
        let room_rects: Vec<rl_core::Rect> = ctx.outputs().iter::<Room>().map(|r| r.0).collect();
        let is_room = |a: &Area| a.cells.iter().any(|c| room_rects.iter().any(|r| r.contains(*c)));

        // Each opening's wall tile, or `None` where it may never be hidden.
        let walls: Vec<Option<TileId>> = openings
            .iter()
            .map(|o| {
                let passable = |c: &Point| ctx.terrain().get(*c).is_some_and(|t| tables.passable[t.index()]);
                let kind = !matches!(o.join, Join::Locked(_) | Join::Sealed) && !painted(o.areas.0) && !painted(o.areas.1);
                let clear = o.cells.iter().all(|c| passable(c) && !ways.contains(c));
                if kind && clear { wall_beside(ctx.terrain(), &tables, o) } else { None }
            })
            .collect();

        // A dead end holding neither way, with its one opening hideable.
        let mut rooms: Vec<(usize, &Area)> = areas
            .iter()
            .filter(|a| dead_ends.get(a.index) == Some(&true) && a.entries.is_none() && is_room(a) && !a.cells.iter().any(|c| ways.contains(c)))
            .filter_map(|a| {
                let mut its = openings.iter().enumerate().filter(|(_, o)| o.areas.0 == a.index || o.areas.1 == a.index);
                match (its.next(), its.next()) {
                    (Some((i, _)), None) if walls[i].is_some() => Some((i, a)),
                    _ => None,
                }
            })
            .collect();
        let want_rooms = ctx.rng().random_range(self.rooms.0..=self.rooms.1).max(self.at_least_rooms);
        rooms.shuffle(ctx.rng());

        let mut gone = vec![false; openings.len()];
        let mut hidden: Vec<(usize, Vec<TileId>)> = Vec::new();
        let mut now = Walks::of(ctx, start, exit);
        for (i, room) in rooms {
            if hidden.len() as u32 == want_rooms {
                break;
            }
            let o = &openings[i];
            let was = wall_over(ctx, &o.cells, walls[i].expect("only hideable openings are candidates"));
            let after = Walks::of(ctx, start, exit);
            let mut mine = BitGrid::new(ctx.terrain().width(), ctx.terrain().height());
            for c in &room.cells {
                mine.insert(*c);
            }
            let floor = |reached: &BitGrid| reached.iter().filter(|c| mine.contains(*c)).collect::<Vec<Point>>();
            let (shut, open) = (floor(&now.shut), floor(&now.open));
            let pocket = !shut.is_empty() && lost(&now.shut, &after.shut, &o.cells) == shut && lost(&now.open, &after.open, &o.cells) == open;
            if pocket && after.to_exit == now.to_exit {
                gone[i] = true;
                hidden.push((i, was));
                now = after;
            } else {
                lay_back(ctx, &o.cells, was);
            }
        }
        let rooms_hidden = hidden.len() as u32;
        if rooms_hidden < self.at_least_rooms {
            return Err(err(format!("{rooms_hidden} hidden rooms, fewer than {}", self.at_least_rooms)));
        }

        let mut loops: Vec<usize> = (0..openings.len()).filter(|i| walls[*i].is_some() && !gone[*i]).collect();
        let want_loops = ctx.rng().random_range(self.loops.0..=self.loops.1);
        loops.shuffle(ctx.rng());
        for i in loops {
            if hidden.len() as u32 - rooms_hidden == want_loops {
                break;
            }
            let o = &openings[i];
            let before = parts(count, &openings, &gone);
            gone[i] = true;
            if parts(count, &openings, &gone) != before {
                gone[i] = false;
                continue;
            }
            let was = wall_over(ctx, &o.cells, walls[i].expect("only hideable openings are candidates"));
            let after = Walks::of(ctx, start, exit);
            let kept = lost(&now.shut, &after.shut, &o.cells).is_empty() && lost(&now.open, &after.open, &o.cells).is_empty();
            if kept && after.to_exit == now.to_exit {
                hidden.push((i, was));
                now = after;
            } else {
                gone[i] = false;
                lay_back(ctx, &o.cells, was);
            }
        }

        for (group, (i, was)) in hidden.into_iter().enumerate() {
            let o = &openings[i];
            let spot = if (group as u32) < rooms_hidden { self.room_spot } else { self.loop_spot };
            for (at, is) in o.cells.iter().zip(was) {
                ctx.emit(Secret { at: *at, is, spot, group: group as u32, joins: o.areas });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accrete::Rectangle;
    use crate::accrete::tests::{accrete, owing, rig};
    use crate::chain::Chain;
    use crate::context::BaseContext;
    use crate::dungeon::{FarthestExit, RandomStart};
    use crate::loops::OpenLoops;
    use crate::survey::Survey;
    use rl_core::{Direction, RunSeed};
    use rl_grid::TileRegistry;

    const ASKED: HideOpenings = HideOpenings { loops: (1, 3), rooms: (0, 2), loop_spot: 6, room_spot: 5, at_least_rooms: 0 };

    /// A place grown with loops, a way in and a way down, and surveyed; at
    /// an even seed it owes a piece painted from a prefab and joined
    /// locked, at an odd one it may grow rooms joined by a lock drawn cell
    /// by cell, so both kinds of lock and the painted piece are among the
    /// openings the pass must leave alone. `hide` is added last.
    fn grown(seed: u64, hide: Option<HideOpenings>) -> Result<BaseContext, BuildError> {
        let r = rig();
        let growth = if seed.is_multiple_of(2) {
            owing(&r, Join::Locked(7))
        } else {
            let mut pass = accrete(&r, 3);
            pass.shapes.push(Box::new(Rectangle { name: "vault".into(), w: (5, 8), h: (5, 7), join: Join::Locked(5) }));
            let vault = pass.shapes.len() - 1;
            for d in &mut pass.districts {
                d.shapes.push((vault, 2));
            }
            pass.locked = r.tiles.id("locked").expect("registered by the rig");
            pass
        };
        let chain = Chain::new().then(growth).then(OpenLoops { door: r.door, min_detour: 20, max: 6 }).then(RandomStart).then(FarthestExit).then(Survey);
        let chain = match hide {
            Some(h) => chain.then(h),
            None => chain,
        };
        chain.run_retrying(|| BaseContext::blank(84, 48, r.tiles.clone(), r.wall), RunSeed(seed), 20)
    }

    fn secrets(ctx: &BaseContext) -> Vec<Secret> {
        ctx.outputs().iter::<Secret>().copied().collect()
    }

    fn ways(ctx: &BaseContext) -> (Point, Point) {
        (ctx.outputs().first::<StartPoint>().unwrap().0, ctx.outputs().first::<ExitPoint>().unwrap().0)
    }

    /// The groups, each with its cells.
    fn groups(found: &[Secret]) -> Vec<Vec<Secret>> {
        let count = found.iter().map(|s| s.group as usize + 1).max().unwrap_or(0);
        (0..count).map(|g| found.iter().copied().filter(|s| s.group as usize == g).collect()).collect()
    }

    fn hidden_room(ctx: &BaseContext, s: &Secret) -> Option<usize> {
        let surveyed = ctx.outputs().first::<Surveyed>().unwrap();
        [s.joins.0, s.joins.1].into_iter().find(|a| surveyed.areas[*a].dead_end)
    }

    fn seeds() -> std::ops::Range<u64> {
        0..30
    }

    #[test]
    fn the_way_down_is_walked_to_from_the_way_in_as_far_as_before_with_every_secret_shut() {
        let mut hid = 0;
        for s in seeds() {
            let without = grown(s, None).unwrap();
            let with = grown(s, Some(ASKED)).unwrap();
            let (start, exit) = ways(&with);
            assert_eq!(ways(&without), (start, exit), "seed {s}: the same attempt was kept");
            let before = walk_through_doors(&without, start, true).value(exit).expect("the exit was drawn by this walk");
            let after = walk_through_doors(&with, start, true).value(exit);
            assert_eq!(after, Some(before), "seed {s}: no secret is on the route, so the walk down costs what it did");
            hid += secrets(&with).len();
        }
        assert!(hid > 0, "some seed hid something, or this proves nothing");
    }

    #[test]
    fn with_every_secret_found_the_place_is_as_grown_and_every_open_cell_is_walked_to() {
        for s in seeds() {
            let without = grown(s, None).unwrap();
            let mut with = grown(s, Some(ASKED)).unwrap();
            let start = ways(&with).0;
            for secret in secrets(&with) {
                with.terrain_mut().set(secret.at, secret.is);
            }
            assert_eq!(with.terrain(), without.terrain(), "seed {s}: a secret is only ever its own opening walled over");
            let passable = with.tiles().tables().passable;
            let reached = walk_through_doors(&with, start, false);
            for (p, t) in with.terrain().iter() {
                assert!(!passable[t.index()] || reached.value(p).is_some(), "seed {s}: {p:?} is open and walked to by nobody");
            }
        }
    }

    #[test]
    fn a_hidden_rooms_only_way_in_is_its_secret_and_it_holds_neither_way() {
        let mut rooms = 0;
        for s in seeds() {
            let ctx = grown(s, Some(HideOpenings { rooms: (2, 2), ..ASKED })).unwrap();
            let (start, exit) = ways(&ctx);
            let found = secrets(&ctx);
            let areas: Vec<&Area> = ctx.outputs().iter::<Area>().collect();
            for group in groups(&found) {
                let Some(room) = hidden_room(&ctx, &group[0]) else { continue };
                rooms += 1;
                let cells = &areas[room].cells;
                let rooms: Vec<rl_core::Rect> = ctx.outputs().iter::<Room>().map(|r| r.0).collect();
                assert!(cells.iter().any(|c| rooms.iter().any(|r| r.contains(*c))), "seed {s}: a hidden room is a room");
                assert!(!cells.contains(&start) && !cells.contains(&exit), "seed {s}: a hidden room holds neither way");
                let shut = walk_through_doors(&ctx, start, false);
                assert!(cells.iter().all(|c| shut.value(*c).is_none()), "seed {s}: with its secret shut nothing walks into it");
                let mut opened = ctx.terrain().clone();
                for secret in &group {
                    opened.set(secret.at, secret.is);
                }
                let mut alone = BaseContext::blank(84, 48, ctx.tiles().clone(), ctx.tiles().expect("wall"));
                *alone.terrain_mut() = opened;
                let open = walk_through_doors(&alone, start, false);
                assert!(cells.iter().all(|c| open.value(*c).is_some()), "seed {s}: its secret alone opens all of it");
            }
        }
        assert!(rooms > 0, "some seed hid a room, or this proves nothing");
    }

    #[test]
    fn counts_fall_within_the_bounds_asked_and_no_secret_is_a_lock_a_painted_piece_or_a_way() {
        let asked = HideOpenings { loops: (1, 2), rooms: (1, 1), loop_spot: 6, room_spot: 5, at_least_rooms: 1 };
        let (mut short, mut tried) = (0, 0);
        for s in seeds() {
            let ctx = grown(s, Some(asked)).unwrap();
            let found = secrets(&ctx);
            let (start, exit) = ways(&ctx);
            let openings: Vec<&Opening> = ctx.outputs().iter::<Opening>().collect();
            let painted: Vec<usize> = ctx.outputs().iter::<Area>().filter(|a| a.entries.is_some()).map(|a| a.index).collect();
            let all = groups(&found);
            let rooms = all.iter().filter(|g| hidden_room(&ctx, &g[0]).is_some()).count() as u32;
            assert_eq!(rooms, 1, "seed {s}: one room was asked and one required");
            let loops = all.len() as u32 - rooms;
            assert!(loops <= asked.loops.1, "seed {s}: {loops} loops hidden");
            // A place with a loop at all has something to try; it falls
            // short only where every loop it has is on the route.
            if ctx.outputs().first::<Surveyed>().unwrap().loops > 0 {
                tried += 1;
                short += u32::from(loops < asked.loops.0);
            }
            let tables = ctx.tiles().tables();
            for group in &all {
                let cells: Vec<Point> = group.iter().map(|g| g.at).collect();
                let o = openings.iter().find(|o| o.cells == cells).unwrap_or_else(|| panic!("seed {s}: a group is one opening's cells, {cells:?}"));
                assert!(!matches!(o.join, Join::Locked(_) | Join::Sealed), "seed {s}: a lock is never hidden");
                assert!(!painted.contains(&o.areas.0) && !painted.contains(&o.areas.1), "seed {s}: a painted piece's entries are never hidden");
                let spot = if hidden_room(&ctx, &group[0]).is_some() { asked.room_spot } else { asked.loop_spot };
                for g in group {
                    assert_eq!((g.joins, g.spot), (o.areas, spot), "seed {s}: each kind carries its own chance");
                    assert!(g.at != start && g.at != exit);
                    let t = ctx.terrain().get(g.at).unwrap();
                    assert!(!tables.passable[t.index()] && tables.opaque[t.index()], "seed {s}: a secret is walled");
                    let beside = o.facing.perpendicular().map(|d| ctx.terrain().get(g.at + d.offset()));
                    assert!(beside.contains(&Some(t)) || cells.len() > 1, "seed {s}: walled with the wall it stands in");
                }
            }
        }
        assert!(tried > 0 && short * 5 <= tried, "{short} of {tried} places with a loop hid fewer loops than the least asked");
    }

    /// A place drawn from pieces, each a rectangle of floor and whether
    /// its growth called it a room, and openings of floor between them,
    /// the way in at the first piece's corner and the way down at the
    /// second's far corner.
    fn drawn(w: i32, h: i32, pieces: &[(rl_core::Rect, bool)], openings: &[(Point, Direction, (usize, usize))]) -> BaseContext {
        let tiles = TileRegistry::standard();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        let mut ctx = BaseContext::blank(w, h, tiles, wall);
        for (index, (bounds, room)) in pieces.iter().enumerate() {
            let cells: Vec<Point> = bounds.cells().collect();
            for c in &cells {
                ctx.terrain_mut().set(*c, floor);
            }
            if *room {
                ctx.emit(Room(*bounds));
            }
            ctx.emit(Area { index, shape: "s".into(), district: "d".into(), inner: *bounds, bounds: *bounds, cells, entries: None });
        }
        for (at, facing, areas) in openings {
            ctx.terrain_mut().set(*at, floor);
            ctx.emit(Opening { cells: vec![*at], facing: *facing, areas: *areas, join: Join::Door });
        }
        let (first, second) = (pieces[0].0, pieces[1].0);
        ctx.emit(StartPoint(Point::new(first.x, first.y)));
        ctx.emit(ExitPoint(Point::new(second.x + second.width - 1, second.y + second.height - 1)));
        ctx
    }

    /// Two rooms and one door between them, the way in in one and the way
    /// down in the other: no loop, and no dead end without a way in it.
    fn two_rooms() -> BaseContext {
        let pieces = [(rl_core::Rect::new(1, 1, 4, 3), true), (rl_core::Rect::new(6, 1, 4, 3), true)];
        drawn(11, 5, &pieces, &[(Point::new(5, 2), Direction::East, (0, 1))])
    }

    /// Two rooms holding the ways, and two dead ends: a passage below the
    /// first and a room beside the second.
    #[test]
    fn a_dead_end_passage_is_never_hidden_and_a_dead_end_room_is() {
        let pieces = [
            (rl_core::Rect::new(1, 1, 4, 3), true),
            (rl_core::Rect::new(6, 1, 4, 3), true),
            (rl_core::Rect::new(2, 5, 1, 3), false),
            (rl_core::Rect::new(11, 1, 4, 3), true),
        ];
        let openings =
            [(Point::new(5, 2), Direction::East, (0, 1)), (Point::new(2, 4), Direction::South, (0, 2)), (Point::new(10, 2), Direction::East, (1, 3))];
        for seed in 0..8 {
            let mut ctx = drawn(16, 9, &pieces, &openings);
            Chain::new().then(Survey).then(HideOpenings { loops: (0, 0), rooms: (2, 2), ..ASKED }).run(&mut ctx, RunSeed(seed)).unwrap();
            let found = secrets(&ctx);
            assert_eq!(found.iter().map(|s| (s.at, s.joins, s.spot)).collect::<Vec<_>>(), vec![(Point::new(10, 2), (1, 3), 5)], "seed {seed}");
        }
    }

    #[test]
    fn a_place_with_nothing_safe_to_hide_hides_nothing_and_fails_only_when_rooms_were_required() {
        let mut ctx = two_rooms();
        let before = ctx.terrain().clone();
        Chain::new()
            .then(Survey)
            .then(HideOpenings { loops: (3, 3), rooms: (2, 2), loop_spot: 6, room_spot: 5, at_least_rooms: 0 })
            .run(&mut ctx, RunSeed(0))
            .unwrap();
        assert!(secrets(&ctx).is_empty());
        assert_eq!(ctx.terrain(), &before);

        let mut ctx = two_rooms();
        let required = HideOpenings { loops: (0, 0), rooms: (0, 0), loop_spot: 6, room_spot: 5, at_least_rooms: 1 };
        let err = Chain::new().then(Survey).then(required).run(&mut ctx, RunSeed(0)).unwrap_err();
        assert_eq!(err.pass, "hide_openings");
        assert!(err.reason.contains("fewer than 1"), "{}", err.reason);
    }

    #[test]
    fn a_range_the_wrong_way_round_is_refused_by_name() {
        let mut ctx = two_rooms();
        let err = Chain::new().then(Survey).then(HideOpenings { loops: (2, 1), ..ASKED }).run(&mut ctx, RunSeed(0)).unwrap_err();
        assert!(err.reason.contains("loops"), "{}", err.reason);
    }
}
