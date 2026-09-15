//! Fire: how it catches, spreads and burns out, whatever is burning.
//!
//! A burning cell holds the turns it has left in a [`TileField<u8>`]. Each
//! turn every burning cell burns down one, and a cell with something to burn
//! may catch, with one chance for each burning neighbour. What a cell would
//! burn as is the caller's to say, as [`Tinder`], so a tile, a crate on it and
//! a vapour in it all feed one fire.
//!
//! The rolls are passed in, one per cell, rather than drawn from a stream: a
//! caller that hashes them from the turn and the cell gets the same spread
//! whichever order the cells are visited in, and has no generator to save.
//!
//! This decides nothing about what burning a cell uses up. A caller that
//! keeps offering the same tinder once a cell has burnt out keeps a fire
//! going forever; the Bevy layer replaces a burnt tile with the one it leaves
//! and takes the fuel off an entity, which is what ends every fire.

use rl_core::Point;
use rl_grid::TileField;

/// What a cell would burn as, if it caught.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tinder {
    /// Percent chance a turn that it catches from each burning neighbour.
    pub catch_pct: u8,
    /// Turns it burns once alight.
    pub turns: u8,
}

impl Tinder {
    /// The more flammable of this and `other`: the likelier to catch, and on a
    /// tie the longer burning. A cell with dry grass and an oil-soaked crate on
    /// it burns as the crate.
    pub fn or(self, other: Option<Tinder>) -> Tinder {
        match other {
            Some(o) if (o.catch_pct, o.turns) > (self.catch_pct, self.turns) => o,
            _ => self,
        }
    }
}

/// The percent chance a cell catches with `burning` neighbours alight, each
/// passing it on with `catch_pct` percent: one minus the chance that every one
/// of them fails.
pub fn catch_chance(catch_pct: u8, burning: u32) -> u32 {
    let pct = u32::from(catch_pct.min(100));
    100 - (0..burning).fold(100, |missed, _| missed * (100 - pct) / 100)
}

/// One turn of fire over `flames`, returning the cells whose fire went out.
///
/// A burning cell burns down one turn. A cell that is not burning catches when
/// `tinder` says it has something to burn and `roll` for it, taken below a
/// hundred, falls under [`catch_chance`] for its burning neighbours, all eight
/// of them counted; it then burns for its tinder's turns.
pub fn spread(flames: &mut TileField<u8>, tinder: impl Fn(Point) -> Option<Tinder>, roll: impl Fn(Point) -> u32) -> Vec<Point> {
    let mut out = Vec::new();
    if flames.is_clear() {
        return out;
    }
    flames.step(|p, around| {
        let now = around.here();
        if now > 0 {
            if now == 1 {
                out.push(p);
            }
            return now - 1;
        }
        // Neighbours first: most of a map is nowhere near a fire, and asking
        // what a cell would burn as is the dearer question.
        let burning = around.neighbours().filter(|(_, turns)| *turns > 0).count() as u32;
        if burning == 0 {
            return 0;
        }
        let Some(fuel) = tinder(p) else { return 0 };
        if roll(p) % 100 < catch_chance(fuel.catch_pct, burning) { fuel.turns.max(1) } else { 0 }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rl_core::seed::position_hash;
    use std::cell::RefCell;

    /// A field of grass that is used up as it burns: what has burnt offers no
    /// tinder again, the way a tile that leaves ash does.
    fn burn_until_out(width: i32, height: i32, grass: impl Fn(Point) -> bool, start: Point, seed: u64) -> (Vec<Point>, u32) {
        let mut flames = TileField::new(width, height);
        flames.set(start, 3);
        let ash = RefCell::new(vec![start]);
        let mut turn = 0;
        while !flames.is_clear() {
            turn += 1;
            assert!(turn < 500, "seed {seed}: still burning after {turn} turns");
            let catches = |p: Point| (grass(p) && !ash.borrow().contains(&p)).then_some(Tinder { catch_pct: 45, turns: 3 });
            let roll = |p: Point| position_hash(seed ^ u64::from(turn), p.x, p.y) as u32;
            spread(&mut flames, catches, roll);
            ash.borrow_mut().extend(flames.set_cells().map(|(p, _)| p));
            let mut burnt = ash.borrow_mut();
            burnt.sort_by_key(|p| (p.y, p.x));
            burnt.dedup();
        }
        (ash.into_inner(), turn)
    }

    #[test]
    fn a_cell_catches_likelier_the_more_of_its_neighbours_burn() {
        assert_eq!(catch_chance(50, 0), 0);
        assert_eq!(catch_chance(50, 1), 50);
        assert_eq!(catch_chance(50, 2), 75);
        assert_eq!(catch_chance(100, 1), 100);
        assert_eq!(catch_chance(0, 8), 0, "what does not catch never catches");
        assert!(catch_chance(10, 8) > catch_chance(10, 1));
    }

    #[test]
    fn a_fire_burns_down_and_goes_out_on_bare_ground() {
        let mut flames = TileField::new(5, 5);
        flames.set(Point::new(2, 2), 2);
        assert!(spread(&mut flames, |_| None, |_| 0).is_empty());
        assert_eq!(flames.get(Point::new(2, 2)), 1);
        assert_eq!(spread(&mut flames, |_| None, |_| 0), vec![Point::new(2, 2)], "reported the turn it went out");
        assert!(flames.is_clear(), "and nothing caught around it");
    }

    #[test]
    fn a_fire_spreads_through_what_burns_and_every_fire_goes_out() {
        for seed in 0..16u64 {
            let (burnt, _) = burn_until_out(24, 16, |_| true, Point::new(12, 8), seed);
            assert!(burnt.len() > 20, "seed {seed}: a grass fire reached only {} cells", burnt.len());
        }
    }

    /// A ring of bare ground around the start: over every seed, nothing
    /// outside it ever catches, because fire spreads only through what burns.
    #[test]
    fn a_firebreak_holds_whatever_the_rolls() {
        let start = Point::new(10, 10);
        let ring = |p: Point| rl_core::geometry::chebyshev(p, start) == 4;
        for seed in 0..32u64 {
            let (burnt, _) = burn_until_out(21, 21, |p| !ring(p), start, seed);
            assert!(burnt.iter().all(|p| rl_core::geometry::chebyshev(*p, start) < 4), "seed {seed}: the fire jumped the break");
        }
    }

    #[test]
    fn the_likelier_tinder_is_what_a_cell_burns_as() {
        let grass = Tinder { catch_pct: 40, turns: 2 };
        let oil = Tinder { catch_pct: 90, turns: 1 };
        assert_eq!(grass.or(Some(oil)), oil);
        assert_eq!(oil.or(Some(grass)), oil);
        assert_eq!(grass.or(None), grass);
        assert_eq!(grass.or(Some(Tinder { catch_pct: 40, turns: 5 })).turns, 5, "on a tie, the longer burning");
    }
}
