//! Fire and gas as the map shows them: blended in over a moment rather than
//! snapping from turn to turn, held back where a blast has yet to reach,
//! and churning while they hang.
//!
//! The fields are `rl-bevy`'s and change a whole turn at a time, so drawn
//! as they stand a cloud jumps a cell every key press and a grenade's smoke
//! is on the floor before its blast has gone off. [`ShownFields`] is the
//! view's memory of what it last showed on each cell. When a cell's fields
//! change it blends from what was showing to what is there now over
//! [`BLEND_SECS`], starting when the last [`Wavefront`] still playing
//! reaches the cell, so smoke rolls out behind the blast that left it and
//! fire catches where the blast has been. [`churn`] stirs how thick a cloud
//! looks, cell by cell and moment by moment, so smoke that hangs between
//! turns still moves.
//!
//! Only the drawing waits and stirs. What the fields hold, what hides whom
//! and what a turn does are the fields' own, and a player who acts before a
//! blend has finished sees it pick up from wherever it had got to.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_core::Point;
use rl_core::geometry::chebyshev;
use rl_core::seed::position_hash;
use rl_rules::GasId;

use crate::map_view::MapView;

/// Seconds a cell takes to go from what it showed to what its fields hold.
pub const BLEND_SECS: f32 = 0.35;

/// A blast's front going out from where it went off.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wavefront {
    /// Where it went off.
    pub from: Point,
    /// When, in seconds of the app's clock.
    pub begins: f32,
    /// Seconds it takes to go one cell further.
    pub ring_secs: f32,
    /// When the blast is over. What changed further out than it reached
    /// shows then, so a blast never holds back a cloud across the map.
    pub ends: f32,
}

impl Wavefront {
    /// When it reaches `cell`: at once where it went off, then a ring a
    /// cell, and up to half a ring more by the cell's position so the front
    /// is ragged, and never after it ends.
    pub fn reaches(&self, cell: Point) -> f32 {
        let out = chebyshev(self.from, cell);
        let rings = if out == 0 { 0.0 } else { out as f32 + (position_hash(0, cell.x, cell.y) % 100) as f32 / 200.0 };
        (self.begins + rings * self.ring_secs).min(self.ends)
    }
}

/// Every blast's front on the current map, written by whatever plays them
/// before the map is drawn: [`ParticlesPlugin`](crate::particles::ParticlesPlugin)
/// does. Empty without it, and then fields show as soon as they change.
#[derive(Resource, Debug, Clone, Default)]
pub struct Wavefronts(pub Vec<Wavefront>);

/// What a cell shows of the fields: the densest gas on it and how dense,
/// and whether it burns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FieldLook {
    /// The densest gas, and how much of it.
    pub gas: Option<(GasId, u8)>,
    /// Whether it burns.
    pub burning: bool,
}

impl FieldLook {
    fn is_empty(&self) -> bool {
        self.gas.is_none() && !self.burning
    }
}

/// A cell on its way from one look to another.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Change {
    from: FieldLook,
    to: FieldLook,
    since: f32,
}

impl Change {
    /// What it shows at `t`: what it was until `since`, then gas thickening
    /// or thinning toward what it is over [`BLEND_SECS`], one gas giving
    /// way to another halfway through, and fire lit or out at once, since a
    /// flame has no half.
    fn at(&self, t: f32) -> FieldLook {
        if t < self.since {
            return self.from;
        }
        let k = ((t - self.since) / BLEND_SECS).min(1.0);
        let toward = |a: u8, b: u8, k: f32| (f32::from(a) + (f32::from(b) - f32::from(a)) * k).round() as u8;
        let gas = match (self.from.gas, self.to.gas) {
            (Some((a, x)), Some((b, y))) if a == b => Some((a, toward(x, y, k))),
            (Some((a, x)), Some(_)) if k < 0.5 => Some((a, toward(x, 0, k * 2.0))),
            (Some(_), Some((b, y))) => Some((b, toward(0, y, k * 2.0 - 1.0))),
            (None, Some((b, y))) => Some((b, toward(0, y, k))),
            (Some((a, x)), None) => Some((a, toward(x, 0, k))),
            (None, None) => None,
        };
        FieldLook { gas: gas.filter(|(_, c)| *c > 0), burning: self.to.burning }
    }
}

/// What the view last showed of the fields, cell by cell, on its way to
/// what they hold.
#[derive(Resource, Debug, Clone, Default)]
pub struct ShownFields {
    map: Option<MapId>,
    cells: BTreeMap<Point, Change>,
}

impl ShownFields {
    /// What `p` shows at `t`.
    pub fn at(&self, p: Point, t: f32) -> FieldLook {
        self.cells.get(&p).map_or(FieldLook::default(), |c| c.at(t))
    }

    /// Sets every cell of `now` blending toward what its fields hold, from
    /// what it showed at `t`, starting once the last of `fronts` still
    /// playing reaches it. A cell that has not changed carries on as it
    /// was, and one on another map than last time starts from nothing.
    pub fn track(&mut self, map: MapId, now: impl IntoIterator<Item = (Point, FieldLook)>, t: f32, fronts: &[Wavefront]) {
        if self.map != Some(map) {
            self.cells.clear();
            self.map = Some(map);
        }
        let reached = |p: Point| fronts.iter().filter(|w| w.ends > t).map(|w| w.reaches(p)).fold(t, f32::max);
        for (p, truth) in now {
            match self.cells.get_mut(&p) {
                Some(change) if change.to == truth => {}
                Some(change) => *change = Change { from: change.at(t), to: truth, since: reached(p) },
                None if truth.is_empty() => {}
                None => {
                    self.cells.insert(p, Change { from: FieldLook::default(), to: truth, since: reached(p) });
                }
            }
        }
        self.cells.retain(|_, c| !(c.to.is_empty() && t >= c.since + BLEND_SECS));
    }
}

impl ShownFields {
    /// Every cell of the clouds the player sees some of: of `cells`, those
    /// showing gas that gas connects, cell to neighbouring cell, to one in
    /// sight, where `known` says the ground under it is known.
    ///
    /// Smoke in sight is seen whole, the way a cloud is from outside it,
    /// though nothing inside it is: without this, a cloud thick enough to
    /// hide behind hides its own far side, and reads as a wall with bare
    /// floor behind it. Only over known ground, so a cloud never shows the
    /// shape of a room nobody has seen.
    pub fn clouds_in_sight(
        &self,
        cells: impl IntoIterator<Item = Point>,
        t: f32,
        sees: impl Fn(Point) -> bool,
        known: impl Fn(Point) -> bool,
    ) -> BTreeSet<Point> {
        let hazy: BTreeSet<Point> = cells.into_iter().filter(|p| self.at(*p, t).gas.is_some() && (sees(*p) || known(*p))).collect();
        let mut frontier: Vec<Point> = hazy.iter().copied().filter(|p| sees(*p)).collect();
        let mut cloud: BTreeSet<Point> = frontier.iter().copied().collect();
        while let Some(p) = frontier.pop() {
            for d in rl_core::Direction::ALL {
                let next = p + d.offset();
                if hazy.contains(&next) && cloud.insert(next) {
                    frontier.push(next);
                }
            }
        }
        cloud
    }
}

/// How much thicker or thinner than it is a cloud looks at `p` at `t`,
/// from -1 to 1: noise over the map two cells to a lattice step and over
/// time a step every second and a bit, smoothly between, so a cloud swells
/// and thins in patches rather than flickering cell by cell.
pub fn churn(p: Point, t: f32) -> f32 {
    let (x, y, z) = (p.x as f32 / 2.0, p.y as f32 / 2.0, t / 1.3);
    let (x0, y0, z0) = (x.floor(), y.floor(), z.floor());
    let smooth = |f: f32| f * f * (3.0 - 2.0 * f);
    let (fx, fy, fz) = (smooth(x - x0), smooth(y - y0), smooth(z - z0));
    let corner = |dx: f32, dy: f32, dz: f32| (position_hash((z0 + dz) as i64 as u64, (x0 + dx) as i32, (y0 + dy) as i32) % 1024) as f32 / 1023.0;
    let lerp = |a: f32, b: f32, k: f32| a + (b - a) * k;
    let plane = |dz: f32| lerp(lerp(corner(0.0, 0.0, dz), corner(1.0, 0.0, dz), fx), lerp(corner(0.0, 1.0, dz), corner(1.0, 1.0, dz), fx), fy);
    lerp(plane(0.0), plane(1.0), fz) * 2.0 - 1.0
}

/// What the view shows is what the fields hold: every frame, each cell of
/// the view set blending toward what fire and gas put on it now.
pub fn track_fields(
    mut shown: ResMut<ShownFields>,
    view: Res<MapView>,
    map: Res<WorldMap>,
    time: Res<Time>,
    fronts: Res<Wavefronts>,
    fire: Option<Res<Fire>>,
    gases: Option<Res<Gases>>,
) {
    if fire.is_none() && gases.is_none() {
        return;
    }
    let now = view.viewport.cells().filter_map(|s| view.to_world(s)).map(|p| {
        let gas = gases.as_deref().and_then(|g| g.densest(p));
        (p, FieldLook { gas, burning: fire.as_deref().is_some_and(|f| f.is_burning(p)) })
    });
    shown.track(map.current(), now, time.elapsed_secs(), &fronts.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMOKE: GasId = GasId::from_raw(0);

    fn smoke(amount: u8) -> FieldLook {
        FieldLook { gas: Some((SMOKE, amount)), burning: false }
    }

    #[test]
    fn a_cell_blends_from_what_it_showed_to_what_its_fields_hold() {
        let mut shown = ShownFields::default();
        let p = Point::new(3, 3);
        shown.track(MapId::SURFACE, [(p, smoke(200))], 10.0, &[]);
        assert_eq!(shown.at(p, 10.0), FieldLook::default(), "nothing yet, the moment it arrived");
        assert_eq!(shown.at(p, 10.0 + BLEND_SECS / 2.0), smoke(100), "half of it halfway");
        assert_eq!(shown.at(p, 10.0 + BLEND_SECS), smoke(200), "all of it once blended");

        shown.track(MapId::SURFACE, [(p, smoke(100))], 11.0, &[]);
        assert_eq!(shown.at(p, 11.0 + BLEND_SECS / 2.0), smoke(150), "thinning toward the next turn's");
        shown.track(MapId::SURFACE, [(p, FieldLook::default())], 11.0 + BLEND_SECS / 2.0, &[]);
        assert_eq!(shown.at(p, 11.0 + BLEND_SECS / 2.0), smoke(150), "and a change halfway through picks up from where it had got to");
        shown.track(MapId::SURFACE, [(p, FieldLook::default())], 13.0, &[]);
        assert!(shown.cells.is_empty(), "a cell that has cleared is forgotten");
    }

    /// The smoke a grenade leaves shows cell by cell as its blast reaches
    /// each, nearest first, and nothing waits for a blast that is over.
    #[test]
    fn what_a_blast_left_shows_only_once_its_front_has_reached_it() {
        let mut shown = ShownFields::default();
        let front = Wavefront { from: Point::new(0, 0), begins: 5.0, ring_secs: 0.1, ends: 5.5 };
        let (near, far, beyond) = (Point::new(1, 0), Point::new(3, 0), Point::new(20, 0));
        shown.track(MapId::SURFACE, [(near, smoke(255)), (far, smoke(255)), (beyond, smoke(255))], 5.0, &[front]);
        let reached = |p: Point| front.reaches(p);
        assert!(reached(near) < reached(far), "the nearer first");
        let before = reached(far) - 0.01;
        assert!(shown.at(near, before).gas.is_some(), "the near cell is thickening");
        assert_eq!(shown.at(far, before), FieldLook::default(), "while the far one is still clear");
        assert_eq!(shown.at(far, reached(far) + BLEND_SECS), smoke(255), "until the front gets there");
        assert_eq!(reached(beyond), front.ends, "past its reach it waits for the blast to end, and no longer");

        let over = Wavefront { begins: 1.0, ends: 1.5, ..front };
        shown.track(MapId::SURFACE, [(Point::new(9, 9), smoke(255))], 5.0, &[over]);
        assert_eq!(shown.at(Point::new(9, 9), 5.0 + BLEND_SECS), smoke(255), "a blast already over holds nothing back");
    }

    #[test]
    fn one_gas_gives_way_to_another_and_fire_lights_at_once() {
        let other = GasId::from_raw(1);
        let change = Change { from: smoke(200), to: FieldLook { gas: Some((other, 100)), burning: true }, since: 0.0 };
        assert_eq!(change.at(-0.1), smoke(200), "as it was, before");
        assert_eq!(change.at(BLEND_SECS * 0.25).gas, Some((SMOKE, 100)), "the first thinning");
        assert_eq!(change.at(BLEND_SECS * 0.75).gas, Some((other, 50)), "then the second thickening");
        assert!(change.at(0.0).burning, "and the flame lit the moment the change began");
    }

    #[test]
    fn a_new_map_starts_from_nothing() {
        let mut shown = ShownFields::default();
        let p = Point::new(1, 1);
        shown.track(MapId::SURFACE, [(p, smoke(255))], 0.0, &[]);
        shown.track(MapId(2), [(p, smoke(255))], 5.0, &[]);
        assert_eq!(shown.at(p, 5.0), FieldLook::default(), "blending in afresh, not carried over from the last map");
    }

    /// A cloud in sight is seen whole: its far side, which it hides, is
    /// drawn as cloud over the ground the player knows, and a second cloud
    /// that touches nothing in sight is not.
    #[test]
    fn a_cloud_in_sight_is_seen_whole_over_known_ground() {
        let mut shown = ShownFields::default();
        let near = (0..4).map(|x| (Point::new(x, 0), smoke(255)));
        let apart = (7..9).map(|x| (Point::new(x, 0), smoke(255)));
        shown.track(MapId::SURFACE, near.chain(apart), 0.0, &[]);
        let t = BLEND_SECS;
        let cells = (0..10).map(|x| Point::new(x, 0));
        let sees = |p: Point| p.x == 0;
        let cloud = shown.clouds_in_sight(cells.clone(), t, sees, |p| p.x != 3);
        assert_eq!(
            cloud.into_iter().collect::<Vec<_>>(),
            vec![Point::new(0, 0), Point::new(1, 0), Point::new(2, 0)],
            "the cloud in sight, as far as the ground is known, and not the one apart"
        );
    }

    /// Stirring is smooth: a cloud swells and thins, it does not flicker.
    #[test]
    fn churn_stays_in_range_and_moves_smoothly() {
        for x in 0..20 {
            for step in 0..200 {
                let t = step as f32 * 0.02;
                let (a, b) = (churn(Point::new(x, 3), t), churn(Point::new(x, 3), t + 0.02));
                assert!((-1.0..=1.0).contains(&a), "{a}");
                assert!((a - b).abs() < 0.1, "a jump from {a} to {b} in a fiftieth of a second at x {x}, t {t}");
            }
        }
        let spread: Vec<f32> = (0..40).map(|x| churn(Point::new(x, 0), 0.5)).collect();
        assert!(spread.iter().any(|c| *c > 0.2) && spread.iter().any(|c| *c < -0.2), "and it does vary across a cloud: {spread:?}");
    }
}
