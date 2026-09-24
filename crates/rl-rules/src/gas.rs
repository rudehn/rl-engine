//! Gas: which gases there are is a game's content; how any gas spreads,
//! fades and hides what is behind it is the engine's.
//!
//! A [`GasDef`] holds only what the engine acts on: how fast it spreads and
//! fades, the concentration that hides what is behind it, whether fire
//! catches in it, and a status it inflicts on whoever breathes enough of it.
//! Anything else a gas does is the game's.
//!
//! A gas is a [`TileField<u8>`] of concentrations. [`release`] puts some down
//! at a point, and what the cell has no room for spills outward at once, so a
//! grenade's worth fills a room and pours down a corridor rather than piling
//! up in one cell. [`diffuse`] steps it a turn: an exchange with each
//! neighbour smooths it, and swirls push a share of each cell one way, the
//! same way over a small patch, so no two clouds spread alike. No cell ever
//! ends a turn denser than the densest cell around it began, and every cell
//! holding gas then loses a share and never less than one unit, so every
//! cloud clears, however it was shaped and whatever it was put down on.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use rl_core::{Direction, Grid2D, Id, Point};
use rl_grid::TileField;
use serde::Deserialize;

use crate::content::{ContentError, Named, Registry};
use crate::names::Names;
use crate::status::StatusId;

/// A kind of gas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GasDef {
    /// The name content refers to it by.
    pub name: String,
    /// How freely it moves a turn, 0 to 100: most of it evens a cell out with
    /// its neighbours and the rest swirls, so a gas that spreads fast also
    /// churns, and one that does not spread stays where it was let go.
    pub spread: u8,
    /// Percent of what is in a cell lost each turn, and never less than one
    /// unit.
    pub fade: u8,
    /// The concentration at or above which it hides what is behind it.
    pub veils_at: Option<u8>,
    /// Whether fire catches in it, burning it away.
    pub burns: bool,
    /// What breathing enough of it does.
    pub inflicts: Option<Breath>,
}

/// A status for whoever breathes a gas at or above a concentration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Breath {
    /// The concentration it takes.
    pub at: u8,
    /// The status.
    pub status: StatusId,
    /// For how many whole turns.
    pub turns: u32,
}

/// A registered gas id.
pub type GasId = Id<GasDef>;

impl Named for GasDef {
    fn name(&self) -> &str {
        &self.name
    }
}

impl GasDef {
    /// A gas called `name` that spreads at half speed, fades a tenth a turn,
    /// hides nothing, does not burn and does nothing to whoever breathes it.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), spread: 50, fade: 10, veils_at: None, burns: false, inflicts: None }
    }

    /// Builder: sets `spread`.
    pub fn spread(mut self, pct: u8) -> Self {
        self.spread = pct;
        self
    }

    /// Builder: sets `fade`.
    pub fn fade(mut self, pct: u8) -> Self {
        self.fade = pct;
        self
    }

    /// Builder: it hides what is behind it at or above `concentration`.
    pub fn veils_at(mut self, concentration: u8) -> Self {
        self.veils_at = Some(concentration);
        self
    }

    /// Builder: fire catches in it.
    pub fn burns(mut self) -> Self {
        self.burns = true;
        self
    }

    /// Builder: breathing it at or above `at` inflicts `status` for `turns`.
    pub fn inflicts(mut self, at: u8, status: StatusId, turns: u32) -> Self {
        self.inflicts = Some(Breath { at, status, turns });
        self
    }

    /// Whether `concentration` of it hides what is behind it.
    pub fn veils(&self, concentration: u8) -> bool {
        self.veils_at.is_some_and(|at| concentration >= at)
    }

    /// What breathing `concentration` of it inflicts, if that is enough to.
    pub fn breathed(&self, concentration: u8) -> Option<Breath> {
        self.inflicts.filter(|b| concentration > 0 && concentration >= b.at)
    }
}

/// A gas as a content file writes it, before its status is resolved.
#[derive(Deserialize)]
struct Authored {
    name: String,
    #[serde(default = "half")]
    spread: u8,
    #[serde(default = "tenth")]
    fade: u8,
    #[serde(default)]
    veils_at: Option<u8>,
    #[serde(default)]
    burns: bool,
    #[serde(default)]
    inflicts: Option<(u8, String, u32)>,
}

fn half() -> u8 {
    50
}

fn tenth() -> u8 {
    10
}

impl Named for Authored {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Loads gases from RON, every status named resolved through `names`.
///
/// Every field but `name` may be left out:
///
/// - `spread`: how freely it moves a turn, 0 to 100, evening out with its
///   neighbours and swirling; 50 when left out.
/// - `fade`: percent lost from each cell a turn, and at least one unit; 10.
/// - `veils_at`: the concentration, 1 to 255, at or above which it hides what
///   is behind it; hides nothing when left out.
/// - `burns`: whether fire catches in it; false.
/// - `inflicts`: `(at, status, turns)`, a status for whoever breathes `at` or
///   more; nothing.
///
/// Reports every unknown status in the file at once.
///
/// ```
/// use rl_rules::{Names, Registry, StatusDef, gas};
///
/// let statuses = Registry::from_defs(vec![StatusDef::new("choking")]).unwrap();
/// let gases = gas::load(r#"[(name: "smoke", veils_at: Some(90)), (name: "fumes", burns: true, inflicts: Some((60, "choking", 2)))]"#, &Names::new().statuses(&statuses)).unwrap();
/// assert!(gases.get(gases.expect("smoke")).veils(120));
/// assert_eq!(gases.get(gases.expect("fumes")).breathed(80).map(|b| b.turns), Some(2));
/// ```
pub fn load(text: &str, names: &Names<'_>) -> Result<Registry<GasDef>, ContentError> {
    let authored: Registry<Authored> = Registry::from_ron_str(text)?;
    let mut errors = Vec::new();
    let mut defs = Vec::new();
    for (_, a) in authored.iter() {
        let inflicts = a.inflicts.as_ref().and_then(|(at, status, turns)| match names.status(status) {
            Ok(status) => Some(Breath { at: *at, status, turns: *turns }),
            Err(e) => {
                errors.push(format!("{}: {e}", a.name));
                None
            }
        });
        defs.push(GasDef { name: a.name.clone(), spread: a.spread.min(100), fade: a.fade.min(100), veils_at: a.veils_at, burns: a.burns, inflicts });
    }
    if !errors.is_empty() {
        return Err(ContentError::Invalid(errors));
    }
    Registry::from_defs(defs)
}

/// The most gas a cell holds.
pub const FULL: u8 = u8::MAX;

/// Puts `amount` of gas at `p`, in a cell `holds` says gas can be in.
///
/// What the cell has room for stays there. The rest spills outward at once,
/// nearest cell first, filling each to [`FULL`] and passing through the ones
/// already full, so a second release over a cloud makes it wider rather than
/// denser, and walls shape it: the same amount is a round cloud in the open
/// and a long one down a corridor. `roll`, a number per cell, nudges which of
/// two nearly as near cells fills first, so the edge is ragged rather than a
/// ring; a caller that hashes it from the cell spills alike every time. What
/// no reachable cell has room for is lost, as it would be in a sealed room
/// already full.
pub fn release(field: &mut TileField<u8>, p: Point, amount: u16, holds: impl Fn(Point) -> bool, roll: impl Fn(Point) -> u32) {
    let (width, height) = (field.width(), field.height());
    let inside = |q: Point| q.x >= 0 && q.y >= 0 && q.x < width && q.y < height;
    if !inside(p) || !holds(p) {
        return;
    }
    let mut left = u32::from(amount);
    // Which cells the spill has reached: a flag per cell of the field, since
    // a spill can reach anywhere gas can go.
    let mut reached = vec![false; (width * height) as usize];
    let at = |q: Point| (q.y * width + q.x) as usize;
    reached[at(p)] = true;
    // Nearest first, a step straight costing ten and one on the diagonal
    // fourteen, with up to eight more from the roll; a tie goes to the upper
    // left, so the order is the same however the heap was filled.
    let mut frontier = BinaryHeap::from([Reverse((0u32, p.y, p.x))]);
    while let Some(Reverse((cost, y, x))) = frontier.pop() {
        let q = Point::new(x, y);
        let room = u32::from(FULL - field.get(q));
        let take = room.min(left);
        field.update(q, |c| c + take as u8);
        left -= take;
        if left == 0 {
            return;
        }
        for d in Direction::ALL {
            let next = q + d.offset();
            if !inside(next) || reached[at(next)] || !holds(next) {
                continue;
            }
            reached[at(next)] = true;
            let step = if d.offset().x != 0 && d.offset().y != 0 { 14 } else { 10 };
            frontier.push(Reverse((cost + step + roll(next) % 9, next.y, next.x)));
        }
    }
}

/// How a cell's share of a turn's movement divides: most of `spread` goes to
/// the even exchange with every neighbour, the rest to a swirl in one
/// direction. The two never move more than a cell holds, so no cell goes
/// below nothing whatever the spread.
const EXCHANGE_PCT: i32 = 60;

/// How wide a patch swirls the same way: wide enough that a swirl moves a
/// lobe of the cloud rather than speckling it, narrow enough that a cloud a
/// grenade makes holds several.
const SWIRL_PATCH: i32 = 3;

/// One turn of `def` spreading and fading over `field`, never into a cell
/// that `holds` says gas cannot be in.
///
/// Three things happen to every cell at once:
///
/// - Each neighbour that can hold gas exchanges a share of the difference
///   with it, so gas flows downhill.
/// - It pushes a share of what it holds to one neighbour: the direction is
///   `roll` of the corner of the patch it is in, so a patch pushes one way
///   together, and how much is its own `roll`. A caller that hashes `roll`
///   from the turn and the cell swirls alike whichever order the cells are
///   visited in, and differently every turn.
/// - What that leaves is held to no more than the densest of it and its
///   neighbours before the turn, so two swirls meeting cannot pile gas up
///   denser than it was, and then fades by `fade` percent and never by less
///   than one, which is what ends every cloud rather than leaving a thin one
///   forever.
///
/// The densest cell therefore loses at least one unit every turn, and a
/// cloud is gone within [`FULL`] turns at the very longest.
pub fn diffuse(field: &mut TileField<u8>, def: &GasDef, holds: impl Fn(Point) -> bool, roll: impl Fn(Point) -> u32) {
    if field.is_clear() {
        return;
    }
    let spread = i32::from(def.spread.min(100));
    let (exchange, swirl) = (spread * EXCHANGE_PCT / 100, spread - spread * EXCHANGE_PCT / 100);
    let fade = i32::from(def.fade.min(100));
    // Every cell asks each of its eight neighbours which way it pushes and
    // how much, so both are worked out once before the step, a direction a
    // patch and a share a cell that holds anything, and laid out a cell each
    // so the step reads them by index. These and a copy of the field are a
    // few bytes a cell, allocated each step: the field's own step allocates
    // nothing, and this rule spends that on reading a neighbour cheaply.
    let (width, height) = (field.width(), field.height());
    let mut pushes: Vec<(u8, u8)> = Vec::with_capacity(field.cells().len());
    // Whether each cell holds gas, asked once rather than by each of its
    // neighbours in turn: for a caller that asks the map, the dearest part
    // of a step.
    let mut open: Vec<bool> = Vec::with_capacity(field.cells().len());
    for y in 0..height {
        let mut toward = 0;
        for x in 0..width {
            open.push(holds(Point::new(x, y)));
            if x % SWIRL_PATCH == 0 {
                toward = (roll(Point::new(x, y / SWIRL_PATCH * SWIRL_PATCH)) % 8) as u8;
            }
            let c = field.cells()[(y * width + x) as usize];
            // Scaled rather than taken modulo, since a divisor known only at
            // run time costs a real division for every cell of the map.
            let share = if c == 0 || swirl == 0 { 0 } else { ((u64::from(roll(Point::new(x, y)) >> 8) * (swirl as u64 + 1)) >> 24) as u8 };
            pushes.push((toward, share));
        }
    }
    let at = |p: Point| (p.y * width + p.x) as usize;
    let offsets = Direction::ALL.map(Direction::offset);
    let before: Vec<u8> = field.cells().to_vec();
    let inside = |n: Point| n.x >= 0 && n.y >= 0 && n.x < width && n.y < height;
    field.step(|p, around| {
        let here = i32::from(around.here());
        // Most of a map is clear air nowhere near a cloud, with nothing to
        // work out.
        if here == 0 && offsets.iter().all(|d| !inside(p + *d) || before[at(p + *d)] == 0) {
            return 0;
        }
        if !open[at(p)] {
            return 0;
        }
        let (mine, my_share) = pushes[at(p)];
        let (mut flow, mut densest, mut pushed) = (0, here, false);
        for (k, d) in offsets.iter().enumerate() {
            let n = p + *d;
            if !inside(n) || !open[at(n)] {
                continue;
            }
            pushed |= mine as usize == k;
            // Truncated toward zero, so what this cell takes across an edge
            // is exactly what the neighbour gives across it.
            let c = i32::from(before[at(n)]);
            flow += (c - here) * exchange / 800;
            if c == 0 {
                continue;
            }
            densest = densest.max(c);
            // The neighbour pushes this way if its direction is the opposite
            // of the one it lies in, which `Direction::ALL` puts four along.
            let (theirs, their_share) = pushes[at(n)];
            if theirs as usize == (k + 4) % 8 {
                flow += c * i32::from(their_share) / 100;
            }
        }
        if pushed {
            flow -= here * i32::from(my_share) / 100;
        }
        let kept = (here + flow).min(densest);
        let left = if kept <= 0 { 0 } else { kept - (kept * fade / 100).max(1) };
        left.clamp(0, i32::from(FULL)) as u8
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use rl_core::seed::position_hash;

    fn open(_: Point) -> bool {
        true
    }

    /// A turn's rolls, hashed from `salt` and the cell as a caller hashes
    /// them from the run's seed and the turn.
    fn rolls(salt: u64) -> impl Fn(Point) -> u32 {
        move |p| position_hash(salt, p.x, p.y) as u32
    }

    fn total(field: &TileField<u8>) -> u32 {
        field.cells().iter().map(|c| u32::from(*c)).sum()
    }

    #[test]
    fn a_puff_spreads_to_its_neighbours_its_densest_cell_never_gains_and_it_clears() {
        let def = GasDef::new("reek").spread(80).fade(10);
        let mut field = TileField::new(15, 15);
        let middle = Point::new(7, 7);
        field.set(middle, 200);
        diffuse(&mut field, &def, open, rolls(0));
        assert!(field.get(middle) < 200, "the middle gave some away");
        assert!(field.get(Point::new(8, 7)) > 0 && field.get(Point::new(8, 8)) > 0, "to every neighbour, the corners too");
        let mut densest = *field.cells().iter().max().unwrap();
        for turn in 1..200 {
            if field.is_clear() {
                return;
            }
            diffuse(&mut field, &def, open, rolls(turn));
            let now = *field.cells().iter().max().unwrap();
            assert!(now < densest, "turn {turn}: the densest cell went from {densest} to {now}");
            densest = now;
        }
        panic!("the puff never cleared");
    }

    #[test]
    fn a_wall_holds_gas_back() {
        let def = GasDef::new("reek").spread(100).fade(1);
        let wall = |p: Point| p.x == 5;
        let mut field = TileField::new(10, 5);
        release(&mut field, Point::new(2, 2), 2000, |p| !wall(p), rolls(0));
        for turn in 0..30 {
            diffuse(&mut field, &def, |p| !wall(p), rolls(turn));
            assert!(field.set_cells().all(|(p, _)| p.x < 5), "nothing crossed the wall: {:?}", field.set_cells().collect::<Vec<_>>());
        }
    }

    /// The property the fade exists for, over clouds of every shape and
    /// every spread: every one clears, the densest cell loses something
    /// every turn however the swirls meet, and moving gas about never makes
    /// more of it.
    #[test]
    fn every_cloud_clears_whatever_its_shape() {
        for seed in 0..32u64 {
            let def = GasDef::new("reek").spread((position_hash(seed, 0, 0) % 101) as u8).fade((position_hash(seed, 1, 0) % 30) as u8);
            let mut field = TileField::new(20, 12);
            let holds = |p: Point| !position_hash(seed, p.x, p.y).is_multiple_of(7);
            for i in 0..6 {
                let h = position_hash(seed, i, 7);
                release(&mut field, Point::new((h % 20) as i32, ((h >> 8) % 12) as i32), (h >> 16) as u16 % 1500, holds, rolls(seed));
            }
            let mut turns = 0;
            while !field.is_clear() {
                let (densest, before) = (*field.cells().iter().max().unwrap(), total(&field));
                diffuse(&mut field, &def, holds, rolls(seed ^ turns));
                assert!(*field.cells().iter().max().unwrap() < densest, "seed {seed}, turn {turns}: the densest cell gained");
                assert!(total(&field) < before, "seed {seed}, turn {turns}: the cloud grew from {before} to {}", total(&field));
                turns += 1;
                assert!(turns <= u64::from(FULL), "seed {seed}: still hanging after {turns} turns");
            }
        }
    }

    /// Two turns' rolls are two different clouds from the same puff, where
    /// an even exchange alone would spread every puff the same.
    #[test]
    fn swirls_spread_the_same_puff_differently_under_different_rolls() {
        let def = GasDef::new("reek").spread(60).fade(5);
        let spread = |salt: u64| {
            let mut field = TileField::new(21, 21);
            release(&mut field, Point::new(10, 10), 6000, open, rolls(0));
            for turn in 0..6 {
                diffuse(&mut field, &def, open, rolls(salt * 100 + turn));
            }
            field
        };
        assert_eq!(spread(1), spread(1), "the same rolls, the same cloud");
        assert_ne!(spread(1), spread(2), "other rolls, another cloud");
    }

    #[test]
    fn a_release_fills_its_cell_and_spills_the_rest_to_the_nearest_cells() {
        let mut field = TileField::new(15, 15);
        let at = Point::new(7, 7);
        release(&mut field, at, 100, open, rolls(0));
        assert_eq!((field.get(at), total(&field)), (100, 100), "what fits stays put");
        release(&mut field, at, 9 * u16::from(FULL) - 100, open, rolls(0));
        let full: Vec<Point> = field.set_cells().filter(|(_, c)| *c == FULL).map(|(p, _)| p).collect();
        assert_eq!(full.len(), 9, "nine cells' worth fills nine cells: {full:?}");
        assert!(full.iter().all(|p| rl_core::geometry::chebyshev(*p, at) <= 2), "all of them near where it was let go: {full:?}");
        assert_eq!(total(&field), 9 * u32::from(FULL), "and none of it lost");
    }

    #[test]
    fn a_release_runs_down_a_corridor_and_what_a_sealed_room_cannot_hold_is_lost() {
        // A room three wide with a corridor running east from it.
        let room = |p: Point| (1..=3).contains(&p.x) && (1..=3).contains(&p.y);
        let corridor = |p: Point| p.y == 2 && p.x >= 4;
        let mut field = TileField::new(30, 5);
        release(&mut field, Point::new(2, 2), 20 * u16::from(FULL), |p| room(p) || corridor(p), rolls(0));
        assert!(field.get(Point::new(14, 2)) == FULL, "eleven cells' worth ran down the corridor");
        assert_eq!(total(&field), 20 * u32::from(FULL));

        let mut sealed = TileField::new(5, 5);
        release(&mut sealed, Point::new(2, 2), 20 * u16::from(FULL), room, rolls(0));
        assert!(sealed.set_cells().all(|(p, c)| room(p) && c == FULL), "the room is full and nothing is outside it");
        assert_eq!(total(&sealed), 9 * u32::from(FULL), "and what it had no room for is gone");
    }

    #[test]
    fn a_release_into_a_wall_or_off_the_field_puts_nothing_down() {
        let mut field = TileField::new(5, 5);
        release(&mut field, Point::new(2, 2), 500, |p| p != Point::new(2, 2), rolls(0));
        release(&mut field, Point::new(9, 2), 500, open, rolls(0));
        assert!(field.is_clear());
    }

    #[test]
    fn gases_load_by_name_and_an_unknown_status_is_named() {
        use crate::status::StatusDef;
        let statuses = Registry::from_defs(vec![StatusDef::new("choking")]).unwrap();
        let names = Names::new().statuses(&statuses);
        let gases =
            load(r#"[(name: "smoke", spread: 70, veils_at: Some(90)), (name: "fumes", burns: true, inflicts: Some((60, "choking", 2)))]"#, &names).unwrap();
        let smoke = gases.get(gases.expect("smoke"));
        assert_eq!((smoke.spread, smoke.fade, smoke.veils(89), smoke.veils(90)), (70, 10, false, true));
        let fumes = gases.get(gases.expect("fumes"));
        assert!(fumes.burns);
        assert_eq!(fumes.breathed(59), None, "too thin to bite");
        assert_eq!(fumes.breathed(60), Some(Breath { at: 60, status: statuses.expect("choking"), turns: 2 }));

        let err = load(r#"[(name: "fumes", inflicts: Some((60, "chokign", 2)))]"#, &names).unwrap_err().to_string();
        assert!(err.contains("fumes") && err.contains("chokign"), "{err}");
    }
}
