//! The tactics every roguelike needs.

use rand::Rng;
use rl_core::{Direction, Point, geometry};
use rl_grid::footprint;

use crate::ability::{Aim, Usable};
use crate::ai::brain::{Decision, Tactic, TacticCtx};
use crate::ai::snapshot::{ActorView, Snapshot};
use crate::ai::wits::Wits;
use crate::faction::Relation;

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
///
/// Only for a mind with [`Wits::FLEES`]: a mindless thing fights on.
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
        if !ctx.snapshot.wits.has(Wits::FLEES) || ctx.snapshot.me.hp_pct() > self.at_pct || ctx.snapshot.enemies.is_empty() {
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

/// Walk to where an enemy was last seen, and stop mattering on arrival.
///
/// Fires only when nothing is in sight, so it belongs below [`Hunt`] and
/// above [`Wander`]: hunt what you see, search what you lost, drift when you
/// have nothing. Steps greedily rather than down a flow field, because the
/// shared fields point at where the enemy is, and a search that followed
/// them would be a search that cheats. On the remembered tile it returns
/// `None`, so the next tactic mills about there until the memory runs out.
///
/// Only for a mind with [`Wits::SEARCHES`]: a mindless thing forgets what it
/// cannot perceive.
#[derive(Debug, Clone, Copy, Default)]
pub struct SearchLastKnown;

impl<A: Copy> Tactic<A> for SearchLastKnown {
    fn name(&self) -> &'static str {
        "search_last_known"
    }
    fn evaluate(&self, ctx: &mut TacticCtx<'_, A>) -> Option<Decision<A>> {
        if !ctx.snapshot.wits.has(Wits::SEARCHES) || !ctx.snapshot.enemies.is_empty() {
            return None;
        }
        let target = ctx.snapshot.last_known?;
        let me = ctx.snapshot.me.pos;
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

/// Use the best ability in reach, when one covers something worth
/// covering.
///
/// The engine cannot know what an ability does, so this scores what its
/// footprint would land on: the things its [`Aim`]
/// wants, less the things it does not. A monster will not drop a burst on
/// its own allies to catch one enemy, and never learns what the burst is.
///
/// It sits wherever a game puts it in the priority list. Above
/// [`MeleeAdjacent`] a caster leads with its ability and falls back to
/// teeth; below it, teeth first and the ability when nothing is in reach.
#[derive(Debug, Clone, Copy)]
pub struct UseAbility {
    /// Percentage chance of reaching for an ability on a turn one is
    /// available. Below a hundred so a caster does not fire every single
    /// turn, which reads as a machine rather than a monster.
    pub chance_pct: u32,
}

impl Default for UseAbility {
    fn default() -> Self {
        Self { chance_pct: 100 }
    }
}

impl UseAbility {
    /// What `usable` aimed at `at` would be worth, or `None` when it is
    /// not worth using.
    ///
    /// Two points for something worth hitting, three off for something hit
    /// by mistake, so one ally caught in a ground burst outweighs one enemy
    /// in it. Who is hit is [`Aim::hits`], the rule the resolver lands the
    /// use with, and what is worth it is [`Aim::worth_aiming_at`].
    fn score<A: Copy>(usable: &Usable, cells: &[Point], snapshot: &Snapshot<A>) -> i32 {
        if usable.aim == Aim::SelfOnly {
            // Nothing to weigh: a self ability is worth using when there
            // is anything to use it against at all.
            return if snapshot.enemies.is_empty() { 0 } else { 1 };
        }
        // Who the footprint hits and whether each was worth hitting, by the
        // rule the resolver lands it with: the user stands in its own blast,
        // and a foe-aimed shape passes an ally by rather than harming it.
        // Healing an ally that was not hurt wastes nothing, so only the
        // other aims count what they catch by mistake.
        let (mut worth, mut harm) = (0, 0);
        for (actor, relation, is_user) in Self::everyone(snapshot) {
            if !cells.contains(&actor.pos) || !usable.aim.hits(Some(relation), is_user) {
                continue;
            }
            if usable.aim.worth_aiming_at(Some(relation), is_user, actor.hp_pct() < 100) {
                worth += 1;
            } else if usable.aim != Aim::Ally {
                harm += 1;
            }
        }
        worth * 2 - harm * 3
    }

    /// The cells worth pointing an ability at, the user's own first.
    fn aims<A: Copy>(usable: &Usable, snapshot: &Snapshot<A>) -> Vec<Point> {
        Self::everyone(snapshot)
            .filter(|(actor, relation, is_user)| usable.aim.worth_aiming_at(Some(*relation), *is_user, actor.hp_pct() < 100))
            .map(|(actor, _, _)| actor.pos)
            .collect()
    }

    /// The user, then what it sees, each with how it stands to the user.
    fn everyone<A: Copy>(snapshot: &Snapshot<A>) -> impl Iterator<Item = (&ActorView<A>, Relation, bool)> {
        std::iter::once((&snapshot.me, Relation::Allied, true))
            .chain(snapshot.enemies.iter().map(|e| (e, Relation::Hostile, false)))
            .chain(snapshot.allies.iter().map(|a| (a, Relation::Allied, false)))
    }
}

impl<A: Copy> Tactic<A> for UseAbility {
    fn name(&self) -> &'static str {
        "use_ability"
    }

    fn evaluate(&self, ctx: &mut TacticCtx<'_, A>) -> Option<Decision<A>> {
        if ctx.snapshot.usable.is_empty() {
            return None;
        }
        if self.chance_pct < 100 && !ctx.rng.random_ratio(self.chance_pct.min(100), 100) {
            return None;
        }
        let me = ctx.snapshot.me.pos;
        let mut best: Option<(i32, Usable, Point)> = None;
        for usable in &ctx.snapshot.usable {
            for aim in Self::aims(usable, ctx.snapshot) {
                let shape = footprint(usable.mode, me, aim, ctx.bounds, |p| p != me && (ctx.blocks_shot)(p));
                let score = Self::score(usable, &shape.cells, ctx.snapshot);
                if score <= 0 {
                    continue;
                }
                // Ties go to the ability declared first, and to the
                // nearest aim, so two runs of one seed agree.
                if best.is_none_or(|(b, _, _)| score > b) {
                    best = Some((score, *usable, aim));
                }
            }
        }
        best.map(|(_, usable, aim)| Decision::Ability { ability: usable.ability, aim })
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

    /// Room enough that no shape is clipped by the edge.
    fn arena() -> rl_core::Rect {
        rl_core::Rect::new(0, 0, 40, 40)
    }

    fn nothing_blocks(_: Point) -> bool {
        false
    }

    fn brain() -> Brain<u32> {
        Brain::new().then(MeleeAdjacent).then(FleeWhenHurt { at_pct: 30 }).then(Hunt).then(Wander { chance_pct: 100 })
    }

    fn open() -> (Terrain, TileRegistry) {
        let r = TileRegistry::standard();
        (Terrain::filled(12, 12, r.expect("floor")), r)
    }

    #[test]
    fn a_lost_enemy_is_searched_for_where_it_was_seen_and_hunting_outranks_it() {
        let (t, r) = open();
        let view_t = t.view(&r);
        let can_step = |p: Point| view_t.is_walkable(p);
        let mut rng = StdRng::seed_from_u64(1);
        let b: Brain<u32> = Brain::new().then(MeleeAdjacent).then(Hunt).then(SearchLastKnown).then(Wander { chance_pct: 0 });
        let mut decide = |snapshot: &Snapshot<u32>| {
            b.decide(&mut TacticCtx {
                snapshot,
                approach: None,
                escape: None,
                can_step: &can_step,
                blocks_shot: &nothing_blocks,
                bounds: arena(),
                rng: &mut rng,
            })
        };

        let mut lost = Snapshot::alone(view(1, 5, 5, 10));
        lost.last_known = Some(Point::new(9, 5));
        assert_eq!(decide(&lost), (Decision::Step(Point::new(6, 5)), Some("search_last_known")));

        let mut seen = lost.clone();
        seen.enemies.push(view(2, 5, 8, 10));
        assert_eq!(decide(&seen).1, Some("hunt"), "something in sight is hunted, not searched for");

        let mut arrived = Snapshot::alone(view(1, 9, 5, 10));
        arrived.last_known = Some(Point::new(9, 5));
        assert_eq!(decide(&arrived), (Decision::Wait, Some("wander")), "on the tile the search has nothing left to do");

        assert_eq!(decide(&Snapshot::alone(view(1, 5, 5, 10))).1, Some("wander"), "tracking nothing, it drifts");
    }

    /// The brain is what a mind would like; its wits are what it manages. A
    /// mindless one hurt beside its enemy fights on through a flee it was
    /// given, and forgets a trail it was told to search.
    #[test]
    fn a_mind_without_the_wits_to_run_or_search_does_neither() {
        let (t, r) = open();
        let view_t = t.view(&r);
        let can_step = |p: Point| view_t.is_walkable(p);
        let mut rng = StdRng::seed_from_u64(1);
        let b: Brain<u32> = Brain::new().then(FleeWhenHurt { at_pct: 50 }).then(SearchLastKnown).then(Hunt);
        let mut decide = |snapshot: &Snapshot<u32>| {
            b.decide(&mut TacticCtx {
                snapshot,
                approach: None,
                escape: None,
                can_step: &can_step,
                blocks_shot: &nothing_blocks,
                bounds: arena(),
                rng: &mut rng,
            })
            .1
        };

        let mut hurt = Snapshot::alone(view(1, 5, 5, 2));
        hurt.enemies.push(view(2, 8, 5, 10));
        assert_eq!(decide(&hurt), Some("flee_when_hurt"));
        hurt.wits = Wits::MINDLESS;
        assert_eq!(decide(&hurt), Some("hunt"), "a mindless thing hunts on at two health in ten");

        let mut lost = Snapshot::alone(view(1, 5, 5, 10));
        lost.last_known = Some(Point::new(9, 5));
        lost.wits = Wits::ANIMAL;
        assert_eq!(decide(&lost), Some("search_last_known"), "an animal follows the trail");
        lost.wits = Wits::MINDLESS;
        assert_eq!(decide(&lost), None, "a mindless thing has nothing to follow");
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
        let (d, who) = b.decide(&mut TacticCtx {
            snapshot: &adjacent,
            approach: None,
            escape: None,
            can_step: &can_step,
            blocks_shot: &nothing_blocks,
            bounds: arena(),
            rng: &mut rng,
        });
        assert_eq!((d, who), (Decision::Attack(2), Some("melee_adjacent")));

        let mut hurt = Snapshot::alone(view(1, 5, 5, 2));
        hurt.enemies.push(view(2, 8, 5, 10));
        let (d, who) = b.decide(&mut TacticCtx {
            snapshot: &hurt,
            approach: None,
            escape: None,
            can_step: &can_step,
            blocks_shot: &nothing_blocks,
            bounds: arena(),
            rng: &mut rng,
        });
        assert_eq!((d, who), (Decision::Step(Point::new(4, 5)), Some("flee_when_hurt")));

        let mut far = Snapshot::alone(view(1, 5, 5, 10));
        far.enemies.push(view(2, 8, 5, 10));
        let mut map = DijkstraMap::covering(&view_t);
        map.build(&view_t, [Point::new(8, 5)], PathRules::default());
        let (d, who) = b.decide(&mut TacticCtx {
            snapshot: &far,
            approach: Some(&map),
            escape: None,
            can_step: &can_step,
            blocks_shot: &nothing_blocks,
            bounds: arena(),
            rng: &mut rng,
        });
        assert_eq!((d, who), (Decision::Step(Point::new(6, 5)), Some("hunt")));

        let alone = Snapshot::alone(view(1, 5, 5, 10));
        let (d, who) = b.decide(&mut TacticCtx {
            snapshot: &alone,
            approach: None,
            escape: None,
            can_step: &can_step,
            blocks_shot: &nothing_blocks,
            bounds: arena(),
            rng: &mut rng,
        });
        assert!(matches!(d, Decision::Step(_)));
        assert_eq!(who, Some("wander"));
        assert_eq!(
            Brain::<u32>::new().decide(&mut TacticCtx {
                snapshot: &alone,
                approach: None,
                escape: None,
                can_step: &can_step,
                blocks_shot: &nothing_blocks,
                bounds: arena(),
                rng: &mut rng
            }),
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
        let (d, _) = brain().decide(&mut TacticCtx {
            snapshot: &hurt,
            approach: None,
            escape: Some(&esc),
            can_step: &blocked,
            blocks_shot: &nothing_blocks,
            bounds: arena(),
            rng: &mut rng,
        });
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

    fn usable(raw: u32, aim: Aim, mode: rl_grid::TargetMode) -> Usable {
        Usable { ability: Id::from_raw(raw), aim, mode }
    }

    /// A mind picks the aim that covers the most of what the ability
    /// wants, and refuses one that would catch its own side.
    #[test]
    fn a_mind_aims_an_ability_it_cannot_understand_by_what_it_would_cover() {
        let mut rng = StdRng::seed_from_u64(4);
        let can_step = |_: Point| true;
        let burst = usable(0, Aim::Foe, rl_grid::TargetMode::Ball { range: 8, radius: 1 });

        // Two enemies in a huddle at (6,5) and one loner at (3,8): the
        // huddle is worth more, and the tactic has no idea what a ball is.
        let mut many = Snapshot::alone(view(1, 0, 5, 10));
        many.enemies = vec![view(2, 3, 8, 10), view(3, 6, 5, 10), view(4, 6, 6, 10)];
        many.usable = vec![burst];
        many.sort();
        let mut ctx =
            TacticCtx { snapshot: &many, approach: None, escape: None, can_step: &can_step, blocks_shot: &nothing_blocks, bounds: arena(), rng: &mut rng };
        let decision = UseAbility::default().evaluate(&mut ctx);
        let Some(Decision::Ability { ability, aim }) = decision else { panic!("expected an ability, got {decision:?}") };
        assert_eq!(ability, Id::from_raw(0));
        assert!(aim == Point::new(6, 5) || aim == Point::new(6, 6), "it aimed at the huddle: {aim:?}");

        // A burst on the ground takes no sides: put an ally in the huddle
        // and the loner becomes the better shot. A foe-aimed burst passes
        // the ally by, which the next test pins.
        let mut mixed = many.clone();
        mixed.usable = vec![usable(0, Aim::Ground, rl_grid::TargetMode::Ball { range: 8, radius: 1 })];
        mixed.allies = vec![view(9, 6, 6, 10)];
        mixed.enemies = vec![view(2, 3, 8, 10), view(3, 6, 5, 10)];
        let mut ctx =
            TacticCtx { snapshot: &mixed, approach: None, escape: None, can_step: &can_step, blocks_shot: &nothing_blocks, bounds: arena(), rng: &mut rng };
        let Some(Decision::Ability { aim, .. }) = UseAbility::default().evaluate(&mut ctx) else { panic!("expected an ability") };
        assert_eq!(aim, Point::new(3, 8), "one ally caught outweighs one enemy hit");
    }

    /// A mind weighs a footprint by who the resolver would hit: a foe-aimed
    /// burst passes an ally by and costs nothing for it, and a spray aimed
    /// at allies is pointed at the hurt user itself.
    #[test]
    fn a_mind_scores_a_footprint_by_who_the_resolver_would_hit() {
        let mut rng = StdRng::seed_from_u64(4);
        let can_step = |_: Point| true;

        // Two enemies side by side with an ally tucked under each, and a
        // loner. A foe-aimed burst on the pair hits both and passes the
        // allies by, so the pair is worth twice the loner.
        let mut s = Snapshot::alone(view(1, 0, 5, 10));
        s.enemies = vec![view(2, 3, 8, 10), view(3, 6, 5, 10), view(4, 7, 5, 10)];
        s.allies = vec![view(8, 6, 6, 10), view(9, 7, 6, 10)];
        s.usable = vec![usable(0, Aim::Foe, rl_grid::TargetMode::Ball { range: 8, radius: 1 })];
        s.sort();
        let mut ctx =
            TacticCtx { snapshot: &s, approach: None, escape: None, can_step: &can_step, blocks_shot: &nothing_blocks, bounds: arena(), rng: &mut rng };
        let Some(Decision::Ability { aim, .. }) = UseAbility::default().evaluate(&mut ctx) else { panic!("expected an ability") };
        assert_ne!(aim, Point::new(3, 8), "the ally under the pair costs a foe-aimed burst nothing");

        // The same room with the burst on the ground: now the ally burns,
        // and the loner is the better shot.
        s.usable = vec![usable(0, Aim::Ground, rl_grid::TargetMode::Ball { range: 8, radius: 1 })];
        let mut ctx =
            TacticCtx { snapshot: &s, approach: None, escape: None, can_step: &can_step, blocks_shot: &nothing_blocks, bounds: arena(), rng: &mut rng };
        let Some(Decision::Ability { aim, .. }) = UseAbility::default().evaluate(&mut ctx) else { panic!("expected an ability") };
        assert_eq!(aim, Point::new(3, 8));

        // Hurt and alone, with a spray aimed at allies: the one ally in
        // reach is itself.
        let mut hurt = Snapshot::alone(view(1, 0, 5, 4));
        hurt.usable = vec![usable(0, Aim::Ally, rl_grid::TargetMode::Adjacent)];
        let mut ctx =
            TacticCtx { snapshot: &hurt, approach: None, escape: None, can_step: &can_step, blocks_shot: &nothing_blocks, bounds: arena(), rng: &mut rng };
        assert_eq!(UseAbility::default().evaluate(&mut ctx), Some(Decision::Ability { ability: Id::from_raw(0), aim: Point::new(0, 5) }));

        // Whole again, there is nothing to mend.
        let mut whole = hurt.clone();
        whole.me = view(1, 0, 5, 10);
        let mut ctx =
            TacticCtx { snapshot: &whole, approach: None, escape: None, can_step: &can_step, blocks_shot: &nothing_blocks, bounds: arena(), rng: &mut rng };
        assert_eq!(UseAbility::default().evaluate(&mut ctx), None);
    }

    /// Nothing worth hitting, nothing to use: the next tactic gets its
    /// turn rather than the actor burning one on an empty patch of floor.
    #[test]
    fn an_ability_with_nothing_under_it_lets_the_next_tactic_decide() {
        let mut rng = StdRng::seed_from_u64(4);
        let can_step = |_: Point| true;
        let mut alone = Snapshot::alone(view(1, 0, 5, 10));
        alone.usable = vec![usable(0, Aim::Foe, rl_grid::TargetMode::Bolt { range: 6 })];
        let mut ctx =
            TacticCtx { snapshot: &alone, approach: None, escape: None, can_step: &can_step, blocks_shot: &nothing_blocks, bounds: arena(), rng: &mut rng };
        assert_eq!(UseAbility::default().evaluate(&mut ctx), None, "no enemies, no aim");

        // A wall between: the bolt stops short, so it covers nothing.
        let mut walled = Snapshot::alone(view(1, 0, 5, 10));
        walled.enemies = vec![view(2, 5, 5, 10)];
        walled.usable = vec![usable(0, Aim::Foe, rl_grid::TargetMode::Bolt { range: 6 })];
        let wall = |p: Point| p == Point::new(2, 5);
        let mut ctx = TacticCtx { snapshot: &walled, approach: None, escape: None, can_step: &can_step, blocks_shot: &wall, bounds: arena(), rng: &mut rng };
        assert_eq!(UseAbility::default().evaluate(&mut ctx), None, "the wall is in the way");
    }

    /// An actor with nothing to use never reaches for one.
    #[test]
    fn a_monster_with_no_abilities_never_offers_one() {
        let mut rng = StdRng::seed_from_u64(1);
        let can_step = |_: Point| true;
        let mut sees = Snapshot::alone(view(1, 0, 0, 10));
        sees.enemies = vec![view(2, 1, 0, 10)];
        let mut ctx =
            TacticCtx { snapshot: &sees, approach: None, escape: None, can_step: &can_step, blocks_shot: &nothing_blocks, bounds: arena(), rng: &mut rng };
        assert_eq!(UseAbility::default().evaluate(&mut ctx), None);
    }
}
