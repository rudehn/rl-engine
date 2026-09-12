//! The tactics every roguelike needs.

use rand::Rng;
use rl_core::{Direction, Point, geometry};

use crate::ai::brain::{Decision, Tactic, TacticCtx};

/// Attack an adjacent enemy, the nearest by position on a tie.
#[derive(Debug, Clone, Copy, Default)]
pub struct MeleeAdjacent;

impl<A: Copy> Tactic<A> for MeleeAdjacent {
    fn name(&self) -> &'static str {
        "melee_adjacent"
    }
    fn evaluate(&self, ctx: &mut TacticCtx<'_, A>) -> Option<Decision<A>> {
        ctx.snapshot.adjacent_enemies().next().map(|e| Decision::Attack(e.id))
    }
}

/// Run when health falls below a share, descending the escape map, or
/// away from the nearest enemy if there is none.
#[derive(Debug, Clone, Copy)]
pub struct FleeWhenHurt {
    /// Flee at or below this percentage of health.
    pub at_pct: i32,
}

impl<A: Copy> Tactic<A> for FleeWhenHurt {
    fn name(&self) -> &'static str {
        "flee_when_hurt"
    }
    fn evaluate(&self, ctx: &mut TacticCtx<'_, A>) -> Option<Decision<A>> {
        if ctx.snapshot.me.hp_pct() > self.at_pct || ctx.snapshot.enemies.is_empty() {
            return None;
        }
        let me = ctx.snapshot.me.pos;
        if let Some(map) = ctx.escape {
            for step in map.descents(me) {
                if (ctx.can_step)(step) {
                    return Some(Decision::Step(step));
                }
            }
        }
        let enemy = ctx.snapshot.nearest_enemy()?.pos;
        let away = Direction::between(enemy, me)?;
        for d in [away, away.rotate_cw(), away.rotate_ccw()] {
            let step = me + d.offset();
            if (ctx.can_step)(step) {
                return Some(Decision::Step(step));
            }
        }
        None
    }
}

/// Close on the nearest enemy, descending the approach map, or stepping
/// straight toward it when there is none.
#[derive(Debug, Clone, Copy, Default)]
pub struct Hunt;

impl<A: Copy> Tactic<A> for Hunt {
    fn name(&self) -> &'static str {
        "hunt"
    }
    fn evaluate(&self, ctx: &mut TacticCtx<'_, A>) -> Option<Decision<A>> {
        let target = ctx.snapshot.nearest_enemy()?.pos;
        let me = ctx.snapshot.me.pos;
        if let Some(map) = ctx.approach {
            for step in map.descents(me) {
                if (ctx.can_step)(step) {
                    return Some(Decision::Step(step));
                }
            }
        }
        let toward = Direction::between(me, target)?;
        for d in [toward, toward.rotate_cw(), toward.rotate_ccw()] {
            let step = me + d.offset();
            if (ctx.can_step)(step) && geometry::chebyshev(step, target) < geometry::chebyshev(me, target) {
                return Some(Decision::Step(step));
            }
        }
        None
    }
}

/// Drift: some chance of a random step, otherwise wait.
#[derive(Debug, Clone, Copy)]
pub struct Wander {
    /// Chance per turn of moving, in percent.
    pub chance_pct: u32,
}

impl<A: Copy> Tactic<A> for Wander {
    fn name(&self) -> &'static str {
        "wander"
    }
    fn evaluate(&self, ctx: &mut TacticCtx<'_, A>) -> Option<Decision<A>> {
        if ctx.rng.random_range(0..100) >= self.chance_pct {
            return Some(Decision::Wait);
        }
        let me = ctx.snapshot.me.pos;
        let start = ctx.rng.random_range(0..8);
        for i in 0..8 {
            let step: Point = me + Direction::from_index((start + i) % 8).offset();
            if (ctx.can_step)(step) {
                return Some(Decision::Step(step));
            }
        }
        Some(Decision::Wait)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::brain::Brain;
    use crate::ai::snapshot::{ActorView, Snapshot};
    use rand::{SeedableRng, rngs::StdRng};
    use rl_core::Id;
    use rl_grid::{DijkstraMap, PathRules, Terrain, TileRegistry};

    fn view(id: u32, x: i32, y: i32, hp: i32) -> ActorView<u32> {
        ActorView { id, pos: Point::new(x, y), hp, max_hp: 10, faction: Id::from_raw(0) }
    }

    fn brain() -> Brain<u32> {
        Brain::new().then(MeleeAdjacent).then(FleeWhenHurt { at_pct: 30 }).then(Hunt).then(Wander { chance_pct: 100 })
    }

    fn open() -> (Terrain, TileRegistry) {
        let r = TileRegistry::standard();
        (Terrain::filled(12, 12, r.expect("floor")), r)
    }

    #[test]
    fn tactics_fire_in_priority_order() {
        let (t, r) = open();
        let view_t = t.view(&r);
        let can_step = |p: Point| view_t.is_walkable(p);
        let mut rng = StdRng::seed_from_u64(1);
        let b = brain();

        let mut adjacent = Snapshot::alone(view(1, 5, 5, 10));
        adjacent.enemies.push(view(2, 6, 5, 10));
        let (d, who) = b.decide(&mut TacticCtx { snapshot: &adjacent, approach: None, escape: None, can_step: &can_step, rng: &mut rng });
        assert_eq!((d, who), (Decision::Attack(2), Some("melee_adjacent")));

        let mut hurt = Snapshot::alone(view(1, 5, 5, 2));
        hurt.enemies.push(view(2, 8, 5, 10));
        let (d, who) = b.decide(&mut TacticCtx { snapshot: &hurt, approach: None, escape: None, can_step: &can_step, rng: &mut rng });
        assert_eq!((d, who), (Decision::Step(Point::new(4, 5)), Some("flee_when_hurt")));

        let mut far = Snapshot::alone(view(1, 5, 5, 10));
        far.enemies.push(view(2, 8, 5, 10));
        let mut map = DijkstraMap::covering(&view_t);
        map.build(&view_t, [Point::new(8, 5)], PathRules::default());
        let (d, who) = b.decide(&mut TacticCtx { snapshot: &far, approach: Some(&map), escape: None, can_step: &can_step, rng: &mut rng });
        assert_eq!((d, who), (Decision::Step(Point::new(6, 5)), Some("hunt")));

        let alone = Snapshot::alone(view(1, 5, 5, 10));
        let (d, who) = b.decide(&mut TacticCtx { snapshot: &alone, approach: None, escape: None, can_step: &can_step, rng: &mut rng });
        assert!(matches!(d, Decision::Step(_)));
        assert_eq!(who, Some("wander"));
        assert_eq!(
            Brain::<u32>::new().decide(&mut TacticCtx { snapshot: &alone, approach: None, escape: None, can_step: &can_step, rng: &mut rng }),
            (Decision::Wait, None)
        );
    }

    #[test]
    fn an_escape_map_beats_the_straight_line_away_and_blocked_steps_are_skipped() {
        let (t, r) = open();
        let view_t = t.view(&r);
        let mut esc = DijkstraMap::covering(&view_t);
        esc.build(&view_t, [Point::new(8, 5)], PathRules::default());
        esc.scale(-12, 10);
        esc.rescan(&view_t, PathRules::default());
        let mut hurt = Snapshot::alone(view(1, 5, 5, 1));
        hurt.enemies.push(view(2, 8, 5, 10));
        let blocked = |p: Point| p != Point::new(4, 5) && view_t.is_walkable(p);
        let mut rng = StdRng::seed_from_u64(1);
        let (d, _) = brain().decide(&mut TacticCtx { snapshot: &hurt, approach: None, escape: Some(&esc), can_step: &blocked, rng: &mut rng });
        match d {
            Decision::Step(p) => {
                assert_ne!(p, Point::new(4, 5), "the blocked cell was skipped");
                assert!(geometry::chebyshev(p, Point::new(8, 5)) >= 3, "moved away: {p:?}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn snapshots_sort_nearest_first_deterministically() {
        let mut s = Snapshot::alone(view(1, 5, 5, 10));
        s.enemies = vec![view(3, 9, 9, 10), view(2, 6, 4, 10), view(4, 4, 6, 10)];
        s.sort();
        assert_eq!(s.enemies.iter().map(|e| e.id).collect::<Vec<_>>(), vec![2, 4, 3]);
        assert_eq!(s.adjacent_enemies().count(), 2);
        assert_eq!(view(1, 0, 0, 3).hp_pct(), 30);
    }
}
