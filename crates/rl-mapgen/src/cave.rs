//! A piece that is not built of rectangles.
//!
//! [`Cave`] is a [`Shape`] like any other: [`Accrete`](crate::Accrete)
//! attaches it to a wall of what stands, publishes it as one area and
//! joins it as its shape says. What differs is only how its body is
//! drawn, by the cellular automaton applied at randomly chosen cells
//! inside a box rather than in rounds over all of it, which gives one
//! cave a mix of sharp and round edges, wide and narrow parts, that a
//! whole-box automaton smooths away.
//!
//! A cave grown this way is one area among the others, not a cavern laid
//! over the map and cut into regions afterwards, so the survey rates each
//! cave's seclusion and anything that fills a room can fill one.

use rand::{Rng, RngCore};
use rl_core::{Grid, Point, Steps};
use rl_grid::BitGrid;
use rl_grid::region::{flood, label_regions};

use crate::accrete::{Join, Piece, Shape};
use crate::passes::{WALL_THRESHOLD, walled};

/// A cave drawn inside a box of `w` by `h`: rock scattered at `fill_pct`,
/// the automaton's rule applied at random cells `visits_pct` of the box
/// over, one smoothing round over the whole box, and the largest region
/// of open ground kept as the body.
///
/// The rule is [`CellularCave`](crate::passes::CellularCave)'s, a cell
/// walled when five of its eight neighbours are, with the rim of the box
/// counting as wall, so a cave rounds off against its box rather than
/// running square along it, and the same numbers read the same way in
/// both. Applied at random
/// cells, each visit sees what earlier visits changed, which is where the
/// variety comes from; fewer visits leave it rougher and more round it
/// off, and the last whole round takes off the single cells of rock or
/// floor that random visits leave standing.
///
/// The body is the largest region joined orthogonally, never one joined
/// only corner to corner, because accretion promises every open cell can
/// be walked to from every other by orthogonal steps.
///
/// Its sites are only those in the rock round it, outside its rim: rock
/// it encloses, a pillar or a hollow, offers none, because a piece joined
/// there would hang inside the cave rather than beside it.
#[derive(Debug, Clone)]
pub struct Cave {
    /// The shape's name.
    pub name: String,
    /// The box's width, smallest and largest.
    pub w: (i32, i32),
    /// The box's height, smallest and largest.
    pub h: (i32, i32),
    /// The share of the box that starts as rock, in a hundred: the more,
    /// the smaller and more broken the cave.
    pub fill_pct: u32,
    /// How many cells the rule is applied at, as a share of the box's
    /// cells in a hundred; more than a hundred visits some cells twice.
    pub visits_pct: u32,
    /// How it is joined.
    pub join: Join,
}

/// How many times a box the automaton shut is grown again before the cave
/// is given up for a single cell.
const REDRAWS: u32 = 8;

impl Cave {
    /// The body grown in one box of `w` by `h`, in box coordinates; empty
    /// when the automaton left no open ground.
    fn grow(&self, w: i32, h: i32, rng: &mut dyn RngCore) -> Vec<Point> {
        let mut solid = Grid::from_fn(w, h, |_| rng.random_range(0..100) < self.fill_pct);
        // In u64: the product for a large box at a large share is past
        // what a u32 holds before it is divided.
        let visits = (u64::from(self.visits_pct) * w as u64 * h as u64 / 100).max(1);
        for _ in 0..visits {
            let p = Point::new(rng.random_range(0..w), rng.random_range(0..h));
            let wall = walled(&solid, p, WALL_THRESHOLD);
            solid.set(p, wall);
        }
        let smooth = Grid::from_fn(w, h, |p| walled(&solid, p, WALL_THRESHOLD));
        let regions = label_regions(&smooth, |i| !smooth.cells()[i], Steps::Four);
        let Some(keep) = regions.largest() else {
            return Vec::new();
        };
        smooth.iter().filter(|(p, _)| regions.label(*p) == Some(keep)).map(|(p, _)| p).collect()
    }
}

/// The piece whose body is `cells`, inside a box of `w` by `h`, with only
/// the sites the rock round the box can reach without crossing the body.
fn facing_out(cells: Vec<Point>, w: i32, h: i32) -> Piece {
    let mut piece = Piece::from_cells(cells);
    // The box with a ring of rock round it, so every site is on it and the
    // ring joins everything outside the rim into one region.
    let at = |p: Point| Point::new(p.x + 1, p.y + 1);
    let mut body = Grid::filled(w + 2, h + 2, false);
    for c in &piece.cells {
        body.set(at(*c), true);
    }
    let mut outside = BitGrid::new(w + 2, h + 2);
    flood(&body, Point::new(0, 0), |i| !body.cells()[i], Steps::Four, &mut outside);
    piece.sites.retain(|(q, _)| outside.contains(at(*q)));
    piece
}

impl Shape for Cave {
    fn name(&self) -> &str {
        &self.name
    }
    fn join(&self) -> Join {
        self.join
    }
    /// A box the automaton shuts completely is drawn again, and after
    /// eight of them the body is the box's middle cell alone: a draw
    /// always hands back a body, and numbers that shut every box show as
    /// caves of one cell rather than as a stall.
    fn draw(&self, rng: &mut dyn RngCore) -> Piece {
        let w = rng.random_range(self.w.0..=self.w.1);
        let h = rng.random_range(self.h.0..=self.h.1);
        for _ in 0..REDRAWS {
            let cells = self.grow(w, h, rng);
            if !cells.is_empty() {
                return facing_out(cells, w, h);
            }
        }
        facing_out(vec![Point::new(w / 2, h / 2)], w, h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accrete::tests::{Rig, build, rig, walkable};
    use crate::accrete::{Accrete, Area, District, Rectangle, bounds_of};
    use crate::chain::{BuildError, Chain};
    use crate::context::{BaseContext, BuildContext};
    use crate::dungeon::{FarthestExit, RandomStart};
    use crate::survey::{Survey, Surveyed};
    use crate::testing::par_seeds;
    use rand::rngs::StdRng;
    use rl_core::{Grid2D, RunSeed, SeedDomain};
    use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

    fn stream(s: u64) -> StdRng {
        RunSeed(s).rng(SeedDomain::new(b"test"), 0)
    }

    /// A rough small cave and a rounder large one.
    fn caves() -> [Cave; 2] {
        [
            Cave { name: "hollow".into(), w: (8, 14), h: (6, 10), fill_pct: 55, visits_pct: 50, join: Join::Mouth(2) },
            Cave { name: "cavern".into(), w: (16, 26), h: (10, 16), fill_pct: 52, visits_pct: 300, join: Join::Mouth(3) },
        ]
    }

    /// `cells` marked on a grid one larger than the box all round, so the
    /// rock outside the rim is one region.
    fn marked(cells: &[Point], cave: &Cave) -> Grid<bool> {
        let mut body = Grid::filled(cave.w.1 + 2, cave.h.1 + 2, false);
        for c in cells {
            body.set(Point::new(c.x + 1, c.y + 1), true);
        }
        body
    }

    #[test]
    fn a_cave_body_is_one_orthogonal_region_inside_its_box() {
        par_seeds(0..200, |s| {
            for cave in caves() {
                let piece = cave.draw(&mut stream(s));
                assert!(!piece.cells.is_empty(), "seed {s}: {} drew no body", cave.name);
                let b = bounds_of(&piece.cells);
                assert!(b.x >= 0 && b.y >= 0 && b.right() <= cave.w.1 && b.bottom() <= cave.h.1, "seed {s}: {} at {b:?}", cave.name);
                let body = marked(&piece.cells, &cave);
                let start = Point::new(piece.cells[0].x + 1, piece.cells[0].y + 1);
                let mut seen = BitGrid::new(body.width(), body.height());
                let reached = flood(&body, start, |i| body.cells()[i], Steps::Four, &mut seen);
                assert_eq!(reached, piece.cells.len(), "seed {s}: {} is more than one region", cave.name);
            }
        });
    }

    /// Numbers that shut every box draw a cave of one cell, the box's
    /// middle, with a site on each side, rather than stalling or drawing
    /// nothing.
    #[test]
    fn a_cave_whose_every_box_shuts_is_its_middle_cell_alone() {
        let shut = Cave { name: "shut".into(), w: (5, 9), h: (5, 9), fill_pct: 100, visits_pct: 50, join: Join::Mouth(1) };
        for s in 0..20 {
            let piece = shut.draw(&mut stream(s));
            assert_eq!(piece.cells.len(), 1, "seed {s}: {:?}", piece.cells);
            let c = piece.cells[0];
            assert!((2..=4).contains(&c.x) && (2..=4).contains(&c.y), "seed {s}: {c:?} is no box's middle");
            assert_eq!(piece.sites.len(), 4, "seed {s}: one site a side");
        }
    }

    #[test]
    fn every_site_is_rock_just_outside_the_body_facing_out_of_it() {
        let enclosing = AtomicUsize::new(0);
        par_seeds(0..200, |s| {
            for cave in caves() {
                let piece = cave.draw(&mut stream(s));
                let body = marked(&piece.cells, &cave);
                let is_body = |p: Point| body.get(Point::new(p.x + 1, p.y + 1)).copied().unwrap_or(false);
                let mut outside = BitGrid::new(body.width(), body.height());
                flood(&body, Point::new(0, 0), |i| !body.cells()[i], Steps::Four, &mut outside);
                let is_outside = |p: Point| outside.contains(Point::new(p.x + 1, p.y + 1));
                assert!(!piece.sites.is_empty(), "seed {s}: {} can be joined nowhere", cave.name);
                for (q, d) in &piece.sites {
                    assert!(!is_body(*q) && is_outside(*q), "seed {s}: {} offers {q:?}, which is not rock outside its rim", cave.name);
                    assert!(is_body(*q - d.offset()) && !is_body(*q + d.offset()), "seed {s}: {} offers {q:?} facing {d:?}, not out", cave.name);
                }
                // Nothing facing out was dropped, and what was dropped faced rock the cave encloses.
                let every = Piece::from_cells(piece.cells.clone()).sites;
                assert!(every.iter().filter(|(q, _)| is_outside(*q)).all(|site| piece.sites.contains(site)), "seed {s}: {}", cave.name);
                enclosing.fetch_add(usize::from(every.len() > piece.sites.len()), Relaxed);
            }
        });
        assert!(enclosing.into_inner() > 0, "some cave enclosed rock and offered no site on it");
    }

    /// Caves alone in one district, or caves in one and rectangles in
    /// another, at a share both sizes reach.
    fn accrete(rig: &Rig, rectangles: bool) -> Accrete {
        let [hollow, cavern] = caves();
        let shapes: Vec<Box<dyn Shape>> = vec![
            Box::new(hollow),
            Box::new(cavern),
            Box::new(Rectangle { name: "cell".into(), w: (5, 9), h: (5, 8), join: Join::Door }),
            Box::new(Rectangle { name: "hall".into(), w: (11, 18), h: (8, 12), join: Join::Mouth(3) }),
        ];
        let mut districts = vec![District { name: "caves".into(), shapes: vec![(0, 3), (1, 2)] }];
        if rectangles {
            districts.push(District { name: "rooms".into(), shapes: vec![(2, 3), (3, 2), (0, 1)] });
        }
        let mix = (0..districts.len()).map(|i| (i, 1)).collect();
        Accrete {
            floor: rig.floor,
            door: rig.door,
            shapes,
            districts,
            mix,
            districts_per_10k: 7,
            open_pct: 35,
            patience: 300,
            barriers: Vec::new(),
            locked: rig.door,
            sealed: rig.door,
            pieces: Vec::new(),
            late_pct: 0,
        }
    }

    fn is_cave(a: &Area) -> bool {
        a.shape == "hollow" || a.shape == "cavern"
    }

    const SIZES: [(i32, i32); 2] = [(84, 48), (160, 90)];

    #[test]
    fn caves_alone_and_caves_among_rectangles_grow_connected_to_their_share_or_fail() {
        let rig = rig();
        for (w, h) in SIZES {
            for rectangles in [false, true] {
                let (failed, mixed) = (AtomicUsize::new(0), AtomicUsize::new(0));
                par_seeds(0..200, |s| {
                    let ctx = match build(accrete(&rig, rectangles), &rig, w, h, s) {
                        Ok(ctx) => ctx,
                        Err(BuildError { pass, .. }) => {
                            assert_eq!(pass, "accrete", "{w}x{h} seed {s}");
                            failed.fetch_add(1, Relaxed);
                            return;
                        }
                    };
                    let tables = ctx.tiles().tables();
                    let open = |i: usize| tables.walkable[ctx.terrain().get_idx(i).index()];
                    let total = (0..(w * h) as usize).filter(|i| open(*i)).count();
                    assert!(total * 100 >= 35 * (w * h) as usize, "{w}x{h} seed {s}: {total} open cells is short of the share");
                    let start = ctx.outputs().first::<Area>().unwrap().cells[0];
                    let mut seen = BitGrid::new(w, h);
                    assert_eq!(flood(ctx.terrain().grid(), start, open, Steps::Four, &mut seen), total, "{w}x{h} seed {s}: not every open cell is reached");
                    let caves = ctx.outputs().iter::<Area>().filter(|a| is_cave(a)).count();
                    let all = ctx.outputs().iter::<Area>().count();
                    if rectangles {
                        mixed.fetch_add(usize::from(caves > 0 && caves < all), Relaxed);
                    } else {
                        assert_eq!(caves, all, "{w}x{h} seed {s}: caves alone grew something else");
                    }
                    for a in ctx.outputs().iter::<Area>() {
                        assert!(a.cells.iter().all(|c| walkable(&ctx, *c)), "{w}x{h} seed {s}: area {} is not all floor", a.index);
                    }
                });
                let (failed, mixed) = (failed.into_inner(), mixed.into_inner());
                assert!(failed < 10, "{w}x{h}, rectangles {rectangles}: {failed} of 200 failed");
                assert!(!rectangles || mixed > 100, "{w}x{h}: caves and rectangles shared {mixed} of 200 places");
            }
        }
        // A share caves cannot be packed to fails by name, and does not hang.
        let mut greedy = accrete(&rig, false);
        greedy.open_pct = 90;
        let once = Chain::new().then(greedy).run_retrying(|| BaseContext::blank(84, 48, rig.tiles.clone(), rig.wall), RunSeed(1), 1);
        assert_eq!(once.expect_err("90% open is out of reach").pass, "accrete");
    }

    #[test]
    fn the_survey_counts_every_cave_and_rates_its_seclusion() {
        let rig = rig();
        let (rated, secluded) = (AtomicUsize::new(0), AtomicUsize::new(0));
        for (w, h) in SIZES {
            par_seeds(0..200, |s| {
                let chain = Chain::new().then(accrete(&rig, true)).then(RandomStart).then(FarthestExit).then(Survey);
                let ctx = chain
                    .run_retrying(|| BaseContext::blank(w, h, rig.tiles.clone(), rig.wall), RunSeed(s), 20)
                    .unwrap_or_else(|e| panic!("{w}x{h} seed {s}: {e}"));
                let surveyed = ctx.outputs().first::<Surveyed>().unwrap();
                let areas: Vec<&Area> = ctx.outputs().iter::<Area>().collect();
                assert_eq!(surveyed.areas.len(), areas.len(), "{w}x{h} seed {s}");
                for i in areas.iter().filter(|a| is_cave(a)).map(|a| a.index) {
                    let facts = surveyed.areas[i];
                    assert!(facts.openings >= 1 && facts.seclusion != u32::MAX, "{w}x{h} seed {s}: cave {i} is {facts:?}");
                    rated.fetch_add(1, Relaxed);
                    secluded.fetch_add(usize::from(facts.seclusion > 0), Relaxed);
                }
            });
        }
        let (rated, secluded) = (rated.into_inner(), secluded.into_inner());
        assert!(rated > 400 && secluded > 0, "{rated} caves rated, {secluded} of them off the route");
    }
}
