//! Placing a place's picks in the areas they prefer.
//!
//! A run's pool draws each place's picks before any place is built; the
//! picks laid as areas of their own are [`Accrete`](crate::Accrete)'s
//! pieces, and [`StampPicks`] is the pass for the rest: each a prefab
//! stamped inside an area its [`Prefer`] names and its shapes allow, or a
//! dressing given such an area, published as a [`DressAt`] for the pass
//! that dresses. The file, the draw and what a kind means are
//! `atglyph-rules`'s; this pass reads only what growth, the
//! [`Survey`](crate::Survey) and the secrets published.

use atglyph_core::{GridRect, Point};
use rand::Rng;

use crate::accrete::Area;
use crate::chain::{BuildError, Pass, Phase};
use crate::context::BaseContext;
use crate::dungeon::{ExitPoint, Room, walk_through_doors};
use crate::passes::StartPoint;
use crate::prefab::{Orient, Prefab, Stamped, laid_in};
use crate::secrets::SecretCell;
use crate::survey::Surveyed;

/// The area a pick prefers.
///
/// Every preference but [`Hidden`](Prefer::Hidden) reads only the areas
/// walked to from the way in with every lock and seal shut and every
/// secret hidden, so nothing meant to be come upon is put behind a lock
/// or a secret it was not drawn for: a locked room's one way in makes it
/// a dead end by count, and it is still never one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize)]
pub enum Prefer {
    /// The area farthest from the route it fits, by
    /// [`AreaFacts::seclusion`](crate::AreaFacts::seclusion): what a
    /// player went out of their way to find.
    Secluded,
    /// An area with one way in.
    DeadEnd,
    /// An area reached only through a secret: the dead-end side of a
    /// [`SecretCell`]'s [`joins`](SecretCell::joins).
    Hidden,
    /// An area three ways or more lead out of.
    Junction,
    /// Any area it fits.
    #[default]
    Anywhere,
}

/// What a pick lays in the area it is given.
#[derive(Debug, Clone)]
pub enum Lay {
    /// A piece stamped inside the area, which must hold it with a cell of
    /// the area all round.
    Stamp(Prefab),
    /// A dressing, by name, which fits an area of any size and is laid by
    /// the pass that dresses, from the [`DressAt`] this one publishes.
    Dress(String),
}

/// One pick to place in an area.
#[derive(Debug, Clone)]
pub struct PoolPick {
    /// What a failure names it by.
    pub name: String,
    /// What it lays.
    pub lay: Lay,
    /// The shapes whose areas it may go in, by [`Area::shape`]; any if
    /// empty.
    pub shapes: Vec<String>,
    /// The area it prefers.
    pub prefer: Prefer,
    /// Whether a place it fits nowhere fails, rather than dropping it.
    pub required: bool,
}

/// A dressing given an area: the pass that dresses lays it there first,
/// before it dresses the areas no stamp or pick took.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DressAt {
    /// The dressing's name.
    pub dressing: String,
    /// The area, by [`Area::index`].
    pub area: usize,
}

/// Places each of a place's picks in an area its preference names.
///
/// The picks are placed in the order given, each in an area that is a
/// room, as [`StampEachRoom`](crate::prefab::StampEachRoom) furnishes, of
/// one of its shapes, painted from no prefab, touched by no [`Stamped`]
/// and given to no dressing before it, and preferred: for
/// [`Prefer::Secluded`] the most secluded such area it fits, ties to the
/// first laid, and for any other an area drawn evenly among those it fits.
/// A piece is laid as `StampEachRoom` lays one, in a facing [`Orient`]
/// allows with a cell of the room all round it, off the way in and the
/// way down, and emits its [`Stamped`], so the next pick and every later
/// pass keep out of it. A dressing fits any such area and is published as
/// a [`DressAt`], laid by the pass that dresses.
///
/// A required pick that fits nowhere fails the place, naming it, for
/// [`Chain::run_retrying`](crate::Chain::run_retrying) to grow another
/// layout for the same picks; one that is not is dropped, since a place
/// short of a flourish is a smaller place and not a broken one.
///
/// Runs in [`Phase::Finish`], after [`Survey`](crate::Survey), whose
/// facts it reads, and after [`HideOpenings`](crate::HideOpenings), whose
/// secrets it reads; fails without a survey or a way in.
#[derive(Debug, Clone)]
pub struct StampPicks {
    /// A stable name, so two passes in one chain draw different streams.
    pub name: &'static str,
    /// The picks, in the order they choose their areas.
    pub picks: Vec<PoolPick>,
    /// How a piece may be turned before it lands.
    pub orient: Orient,
}

impl Pass for StampPicks {
    fn name(&self) -> &'static str {
        self.name
    }
    fn phase(&self) -> Phase {
        Phase::Finish
    }
    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let err = |reason: String| BuildError::new(self.name, reason);
        let surveyed = ctx.outputs().first::<Surveyed>().ok_or_else(|| err("no survey emitted before it".into()))?.clone();
        let start = ctx.outputs().first::<StartPoint>().ok_or_else(|| err("no start point emitted before it".into()))?.0;
        let keep: Vec<Point> = std::iter::once(start).chain(ctx.outputs().first::<ExitPoint>().map(|e| e.0)).collect();
        let areas: Vec<Area> = ctx.outputs().iter::<Area>().cloned().collect();
        let rooms: Vec<GridRect> = ctx.outputs().iter::<Room>().map(|r| r.0).collect();
        // A room on an area's floor, and not its inner rectangle, since a
        // lake measures that again and leaves the room as growth emitted
        // it; pieces' floors never share a cell, so the room is the area's.
        let is_room = |a: &Area| a.cells.iter().any(|c| rooms.iter().any(|r| r.contains(*c)));
        let secrets: Vec<SecretCell> = ctx.outputs().iter::<SecretCell>().copied().collect();
        let walked = walk_through_doors(ctx, start, true);
        let facts = |i: usize| surveyed.areas.get(i).copied();
        let hidden = |a: &Area| secrets.iter().any(|s| (s.joins.0 == a.index || s.joins.1 == a.index) && facts(a.index).is_some_and(|f| f.dead_end));
        let reached = |a: &Area| a.cells.iter().any(|c| walked.value(*c).is_some());
        let prefers = |a: &Area, prefer: Prefer| match prefer {
            Prefer::Hidden => hidden(a),
            _ if hidden(a) || !reached(a) => false,
            Prefer::DeadEnd => facts(a.index).is_some_and(|f| f.dead_end),
            Prefer::Junction => facts(a.index).is_some_and(|f| f.junction),
            Prefer::Secluded | Prefer::Anywhere => facts(a.index).is_some_and(|f| f.seclusion != u32::MAX),
        };
        // The areas given to a dressing so far, which no later pick takes.
        let mut dressed: Vec<usize> = Vec::new();

        for pick in &self.picks {
            let facings = match &pick.lay {
                Lay::Stamp(prefab) => self.orient.facings(prefab),
                Lay::Dress(_) => Vec::new(),
            };
            let taken: Vec<GridRect> = ctx.outputs().iter::<Stamped>().map(|s| s.bounds).collect();
            // Each area it may go in, with every facing that fits there and
            // where that facing would be laid; none for a dressing.
            let fits: Vec<(&Area, Vec<(&Prefab, Point)>)> = areas
                .iter()
                .filter(|a| {
                    a.entries.is_none()
                        && is_room(a)
                        && (pick.shapes.is_empty() || pick.shapes.contains(&a.shape))
                        && taken.iter().all(|t| a.inner.intersection(t).is_none())
                        && !dressed.contains(&a.index)
                        && prefers(a, pick.prefer)
                })
                .map(|a| (a, facings.iter().filter_map(|f| laid_in(a.inner, f, &keep).map(|at| (f, at))).collect::<Vec<_>>()))
                .filter(|(_, faces)| matches!(pick.lay, Lay::Dress(_)) || !faces.is_empty())
                .collect();
            if fits.is_empty() {
                if pick.required {
                    return Err(err(format!("{}: no {:?} area it is allowed in fits it", pick.name, pick.prefer)));
                }
                continue;
            }
            let (area, faces) = match pick.prefer {
                // The first laid of the most secluded, since `max_by_key`
                // keeps the last of equals and the areas run in laying order.
                Prefer::Secluded => fits.iter().rev().max_by_key(|(a, _)| facts(a.index).map_or(0, |f| f.seclusion)).expect("not empty"),
                _ => &fits[ctx.rng().random_range(0..fits.len())],
            };
            match &pick.lay {
                Lay::Dress(dressing) => {
                    dressed.push(area.index);
                    ctx.emit(DressAt { dressing: dressing.clone(), area: area.index });
                }
                Lay::Stamp(_) => {
                    let (piece, origin) = faces[ctx.rng().random_range(0..faces.len())];
                    piece.stamp(ctx.terrain_mut(), origin);
                    let marks = piece.marks().iter().map(|(c, p)| (*c, origin + *p)).collect();
                    ctx.emit(Stamped { bounds: GridRect::new(origin.x, origin.y, piece.width(), piece.height()), marks, prefab: piece.key() });
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accrete::{Join, Opening};
    use crate::chain::Chain;
    use crate::context::BaseContext;
    use crate::survey::Survey;
    use atglyph_core::{Direction, RunSeed};
    use atglyph_grid::TileRegistry;

    /// Six rooms of five by five. The way in is in the first and the way
    /// down in the second; the third is a junction, joined to the second,
    /// to the fourth, a dead end far along the row, and to the sixth, a
    /// dead end below it behind a secret; the fifth is a dead end below
    /// the first. `change` alters a room before it is published, and
    /// `join` is how the fourth is joined; with `hide` false the sixth's
    /// door is left open and no secret published.
    fn place(change: impl Fn(usize, &mut Area), join: Join, hide: bool) -> BaseContext {
        let tiles = TileRegistry::standard();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        let mut ctx = BaseContext::blank(25, 13, tiles, wall);
        let corners = [(1, 1), (7, 1), (13, 1), (19, 1), (1, 7), (13, 7)];
        for (index, (x, y)) in corners.into_iter().enumerate() {
            let bounds = GridRect::new(x, y, 5, 5);
            let mut area = Area { index, shape: "room".into(), district: "d".into(), cells: bounds.cells().collect(), bounds, inner: bounds, entries: None };
            change(index, &mut area);
            for c in &area.cells {
                ctx.terrain_mut().set(*c, floor);
            }
            ctx.emit(Room(area.inner));
            ctx.emit(area);
        }
        let openings = [
            (Point::new(6, 3), Direction::East, (0, 1), Join::Door),
            (Point::new(12, 3), Direction::East, (1, 2), Join::Door),
            (Point::new(18, 3), Direction::East, (2, 3), join),
            (Point::new(3, 6), Direction::South, (0, 4), Join::Door),
            (Point::new(15, 6), Direction::South, (2, 5), Join::Door),
        ];
        for (at, facing, areas, join) in openings {
            ctx.terrain_mut().set(at, floor);
            ctx.emit(Opening { cells: vec![at], facing, areas, join });
        }
        ctx.emit(StartPoint(Point::new(1, 1)));
        ctx.emit(ExitPoint(Point::new(11, 5)));
        Chain::new().then(Survey).run(&mut ctx, RunSeed(0)).unwrap();
        if !hide {
            return ctx;
        }
        // Hidden as `HideOpenings` hides a dead end's door: after the
        // survey, which describes the place with it found.
        let door = Point::new(15, 6);
        ctx.terrain_mut().set(door, wall);
        ctx.emit(SecretCell { at: door, is: floor, spot: 5, group: 0, joins: (2, 5) });
        ctx
    }

    fn plain() -> BaseContext {
        place(|_, _| {}, Join::Door, true)
    }

    fn pick(name: &str, size: usize, prefer: Prefer, required: bool) -> PoolPick {
        let tiles = TileRegistry::standard();
        let row = ".".repeat(size);
        let rows: Vec<&str> = (0..size).map(|_| row.as_str()).collect();
        let prefab = Prefab::parse(&rows, |_| Some(tiles.expect("floor"))).unwrap();
        PoolPick { name: name.into(), lay: Lay::Stamp(prefab), shapes: Vec::new(), prefer, required }
    }

    /// The area each stamp landed in, in the order the picks were placed.
    fn landed(mut ctx: BaseContext, picks: Vec<PoolPick>, seed: u64) -> Result<Vec<usize>, BuildError> {
        Chain::new().then(StampPicks { name: "picks", picks, orient: Orient::Turned }).run(&mut ctx, RunSeed(seed))?;
        let areas: Vec<&Area> = ctx.outputs().iter::<Area>().collect();
        Ok(ctx
            .outputs()
            .iter::<Stamped>()
            .map(|s| areas.iter().find(|a| a.inner.intersection(&s.bounds) == Some(s.bounds)).expect("a stamp lies inside one area").index)
            .collect())
    }

    /// The area of `candidates` the survey rates most secluded.
    fn most_secluded(ctx: &BaseContext, candidates: &[usize]) -> usize {
        let s = ctx.outputs().first::<Surveyed>().unwrap();
        *candidates.iter().rev().max_by_key(|i| s.areas[**i].seclusion).unwrap()
    }

    /// A lake takes floor from an area and measures its inner rectangle
    /// again, and leaves the room growth emitted as it was; the area is
    /// still a room a pick may land in, inside its new rectangle.
    #[test]
    fn an_area_a_lake_measured_again_still_takes_a_pick() {
        let mut ctx = plain();
        for a in ctx.outputs_mut().iter_mut::<Area>().filter(|a| a.index == 3) {
            a.inner = GridRect::new(19, 1, 5, 4);
        }
        assert_eq!(landed(ctx, vec![pick("p", 2, Prefer::Secluded, true)], 0).unwrap(), vec![3]);
    }

    #[test]
    fn a_secluded_pick_lands_in_the_most_secluded_area_it_fits() {
        for seed in 0..20 {
            let ctx = plain();
            let expect = most_secluded(&ctx, &[0, 1, 2, 3, 4]);
            assert_eq!(expect, 3, "the far dead end is the most secluded, or this proves little");
            assert_eq!(landed(ctx, vec![pick("p", 3, Prefer::Secluded, true)], seed).unwrap(), vec![3], "seed {seed}");

            // Too small to hold it with a cell all round, the far room
            // gives way to the next most secluded.
            let narrow = place(
                |i, a| {
                    if i == 3 {
                        *a = Area { inner: GridRect::new(19, 1, 4, 5), ..a.clone() }
                    }
                },
                Join::Door,
                true,
            );
            let expect = most_secluded(&narrow, &[0, 1, 2, 4]);
            assert_eq!(landed(narrow, vec![pick("p", 3, Prefer::Secluded, true)], seed).unwrap(), vec![expect], "seed {seed}");

            // A second pick keeps out of the first one's room.
            let two = landed(plain(), vec![pick("a", 3, Prefer::Secluded, true), pick("b", 3, Prefer::Secluded, true)], seed).unwrap();
            assert_eq!(two, vec![3, most_secluded(&plain(), &[0, 1, 2, 4])], "seed {seed}");
        }
    }

    #[test]
    fn a_hidden_pick_lands_only_behind_a_secret_and_no_other_does() {
        for seed in 0..20 {
            assert_eq!(landed(plain(), vec![pick("p", 3, Prefer::Hidden, true)], seed).unwrap(), vec![5], "seed {seed}");
            for prefer in [Prefer::Anywhere, Prefer::DeadEnd, Prefer::Secluded, Prefer::Junction] {
                let here = landed(plain(), vec![pick("p", 3, prefer, true)], seed).unwrap();
                assert!(!here.contains(&5), "seed {seed}: {prefer:?} found the hidden room");
            }
        }
        let none = place(|_, _| {}, Join::Door, false);
        assert!(landed(none, vec![pick("p", 3, Prefer::Hidden, false)], 0).unwrap().is_empty(), "with no secret there is nowhere hidden");
    }

    #[test]
    fn a_dead_end_pick_lands_only_in_a_dead_end_and_a_junction_pick_only_in_a_junction() {
        let mut seen = Vec::new();
        for seed in 0..40 {
            let here = landed(plain(), vec![pick("p", 3, Prefer::DeadEnd, true)], seed).unwrap();
            assert!(here == vec![3] || here == vec![4], "seed {seed}: {here:?}");
            seen.extend(here);
            assert_eq!(landed(plain(), vec![pick("p", 3, Prefer::Junction, true)], seed).unwrap(), vec![2], "seed {seed}");
        }
        assert!(seen.contains(&3) && seen.contains(&4), "either dead end may be drawn: {seen:?}");
    }

    #[test]
    fn no_pick_lands_behind_a_lock_in_a_painted_piece_or_outside_its_shapes() {
        for seed in 0..20 {
            for prefer in [Prefer::Secluded, Prefer::DeadEnd, Prefer::Anywhere] {
                let locked = landed(place(|_, _| {}, Join::Locked(1), true), vec![pick("p", 3, prefer, true)], seed).unwrap();
                assert!(!locked.contains(&3), "seed {seed}: {prefer:?} went behind the lock");
                let painted = landed(place(|i, a| a.entries = (i == 3).then(Vec::new), Join::Door, true), vec![pick("p", 3, prefer, true)], seed).unwrap();
                assert!(!painted.contains(&3), "seed {seed}: {prefer:?} stamped into a painted piece");
            }
            let hall = place(|i, a| a.shape = if i == 4 { "hall" } else { "room" }.into(), Join::Door, true);
            let only = PoolPick { shapes: vec!["hall".into()], ..pick("p", 3, Prefer::Anywhere, true) };
            assert_eq!(landed(hall, vec![only], seed).unwrap(), vec![4], "seed {seed}");
        }
    }

    #[test]
    fn a_required_pick_that_fits_nowhere_fails_the_place_and_a_flavor_one_is_dropped() {
        let err = landed(plain(), vec![pick("great hall", 9, Prefer::Anywhere, true)], 0).unwrap_err();
        assert_eq!(err.pass, "picks");
        assert!(err.reason.contains("great hall"), "{}", err.reason);
        let kept = landed(plain(), vec![pick("rug", 9, Prefer::Anywhere, false), pick("p", 3, Prefer::Junction, true)], 0).unwrap();
        assert_eq!(kept, vec![2], "the flavor pick is dropped and the next still placed");
    }

    fn dress(name: &str, prefer: Prefer, required: bool) -> PoolPick {
        PoolPick { name: name.into(), lay: Lay::Dress(name.into()), shapes: Vec::new(), prefer, required }
    }

    /// Where each dressing was given an area, by name and area.
    fn dressed(mut ctx: BaseContext, picks: Vec<PoolPick>, seed: u64) -> Result<Vec<(String, usize)>, BuildError> {
        Chain::new().then(StampPicks { name: "picks", picks, orient: Orient::Turned }).run(&mut ctx, RunSeed(seed))?;
        Ok(ctx.outputs().iter::<DressAt>().map(|d| (d.dressing.clone(), d.area)).collect())
    }

    #[test]
    fn a_dressed_pick_is_given_an_area_its_preference_names_and_never_one_taken() {
        for seed in 0..20 {
            assert_eq!(dressed(plain(), vec![dress("a", Prefer::Secluded, true)], seed).unwrap(), vec![("a".into(), 3)], "seed {seed}");
            assert_eq!(dressed(plain(), vec![dress("a", Prefer::Hidden, true)], seed).unwrap(), vec![("a".into(), 5)], "seed {seed}");
            assert_eq!(dressed(plain(), vec![dress("a", Prefer::Junction, true)], seed).unwrap(), vec![("a".into(), 2)], "seed {seed}");

            // Two dead ends are reached, and each is given once.
            let mut two: Vec<usize> = dressed(plain(), vec![dress("a", Prefer::DeadEnd, true), dress("b", Prefer::DeadEnd, true)], seed)
                .unwrap()
                .into_iter()
                .map(|(_, area)| area)
                .collect();
            two.sort_unstable();
            assert_eq!(two, vec![3, 4], "seed {seed}");

            // A stamp's room is not given to a dressing, nor a dressing's to a stamp.
            let after_stamp = dressed(plain(), vec![pick("p", 3, Prefer::Secluded, true), dress("a", Prefer::Secluded, true)], seed).unwrap();
            assert_eq!(after_stamp, vec![("a".into(), most_secluded(&plain(), &[0, 1, 2, 4]))], "seed {seed}");
            let after_dress = landed(plain(), vec![dress("a", Prefer::Secluded, true), pick("p", 3, Prefer::Secluded, true)], seed).unwrap();
            assert_eq!(after_dress, vec![most_secluded(&plain(), &[0, 1, 2, 4])], "seed {seed}");

            // Never behind a lock, and never in a painted piece.
            let locked = dressed(place(|_, _| {}, Join::Locked(1), true), vec![dress("a", Prefer::DeadEnd, true)], seed).unwrap();
            assert_eq!(locked, vec![("a".into(), 4)], "seed {seed}");
            let painted = dressed(place(|i, a| a.entries = (i == 3).then(Vec::new), Join::Door, true), vec![dress("a", Prefer::DeadEnd, true)], seed).unwrap();
            assert_eq!(painted, vec![("a".into(), 4)], "seed {seed}");
        }
    }

    #[test]
    fn a_required_dressing_with_no_area_fails_the_place_and_a_flavor_one_is_dropped() {
        let three = |third_required| vec![dress("a", Prefer::DeadEnd, true), dress("b", Prefer::DeadEnd, true), dress("c", Prefer::DeadEnd, third_required)];
        let err = dressed(plain(), three(true), 0).unwrap_err();
        assert!(err.pass == "picks" && err.reason.contains("c:"), "{}", err.reason);
        let kept = dressed(plain(), three(false), 0).unwrap();
        assert_eq!(kept.len(), 2, "the third has no dead end left and is dropped: {kept:?}");
        let none = dressed(place(|_, _| {}, Join::Door, false), vec![dress("h", Prefer::Hidden, true)], 0).unwrap_err();
        assert!(none.reason.contains("h:"), "{}", none.reason);
    }
}
