//! The tactics every roguelike needs.

use std::collections::VecDeque;

use rand::Rng;
use rl_core::{Direction, Point, geometry};
use rl_grid::{clear_shot, footprint};

use crate::ability::{Aim, Usable};
use crate::ai::brain::{Decision, Tactic, TacticCtx};
use crate::ai::snapshot::{ActorView, ItemView, Snapshot};
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

/// Run when health falls below a share, down a field away from every
/// enemy in sight, or straight away from the nearest if there is none.
///
/// Only for a mind with [`Wits::FLEES`]: a mindless thing fights on, and
/// one with no health has nothing to run from.
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
        let hurt = ctx.snapshot.me.hp_pct().is_some_and(|p| p <= self.at_pct);
        if !ctx.snapshot.wits.has(Wits::FLEES) || !hurt || ctx.snapshot.enemies.is_empty() {
            return None;
        }
        let me = ctx.snapshot.me.pos;
        let foes: Vec<Point> = ctx.snapshot.enemies.iter().map(|e| e.pos).collect();
        if let Some(step) = ctx.step_away_from(&foes) {
            return Some(Decision::Step(step));
        }
        let enemy = ctx.snapshot.nearest_enemy()?.pos;
        let away = Direction::between(enemy, me)?;
        for d in [away, away.rotate_cw(), away.rotate_ccw()] {
            let step = me + d.offset();
            if steps_to(ctx.can_step, me, step) {
                return Some(Decision::Step(step));
            }
        }
        None
    }
}

/// Close on the enemies in sight, down a field toward all of them, which
/// leads to the nearest by the way round whatever is between; or straight
/// toward the nearest when there is no field.
#[derive(Debug, Clone, Copy, Default)]
pub struct Hunt;

impl<A: Copy> Tactic<A> for Hunt {
    fn name(&self) -> &'static str {
        "hunt"
    }
    fn evaluate(&self, ctx: &mut TacticCtx<'_, A>) -> Option<Decision<A>> {
        let target = ctx.snapshot.nearest_enemy()?.pos;
        let me = ctx.snapshot.me.pos;
        let foes: Vec<Point> = ctx.snapshot.enemies.iter().map(|e| e.pos).collect();
        if let Some(step) = ctx.step_toward(&foes) {
            return Some(Decision::Step(step));
        }
        let toward = Direction::between(me, target)?;
        for d in [toward, toward.rotate_cw(), toward.rotate_ccw()] {
            let step = me + d.offset();
            if steps_to(ctx.can_step, me, step) && geometry::chebyshev(step, target) < geometry::chebyshev(me, target) {
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
/// have nothing. Walks down a field toward the remembered cell, which is
/// where the enemy was and not where it is, so the search does not cheat.
/// On the remembered tile it returns `None`, so the next tactic mills about
/// there until the memory runs out.
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
        if me == target {
            return None;
        }
        if let Some(step) = ctx.step_toward(&[target]) {
            return Some(Decision::Step(step));
        }
        let toward = Direction::between(me, target)?;
        for d in [toward, toward.rotate_cw(), toward.rotate_ccw()] {
            let step = me + d.offset();
            if steps_to(ctx.can_step, me, step) && geometry::chebyshev(step, target) < geometry::chebyshev(me, target) {
                return Some(Decision::Step(step));
            }
        }
        None
    }
}

/// Whose company a mind keeps station on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Station {
    /// Its allies: a companion, an escort, a pack that stays together.
    Allies,
    /// Its enemies: a spotter that calls out where you are, a skirmisher
    /// that stays out of reach, a hound that bays and never bites.
    Enemies,
}

/// Keep station on somebody: close the gap when it is wider than
/// `keep_within`, back off when it is narrower than `no_closer_than`, and
/// leave the band between to the next tactic.
///
/// One tactic for both sides of the same behaviour. It was two, `Follow`
/// toward the allies and `Shadow` toward the enemies, with the same
/// close-the-gap block written twice; the only thing that ever differed was
/// the roster it read, so that is the only thing it takes.
///
/// Where it goes in a brain depends on which station it keeps. Toward
/// allies: above [`Wander`] and usually below [`MeleeAdjacent`], so a
/// companion fights what reaches it and otherwise stays with you. Toward
/// enemies: above anything that closes in, such as [`Hunt`], which would
/// spend the turn undoing the distance this keeps, with a shot or a
/// [`Hover`] under it for the band it leaves alone.
///
/// With nobody in sight it keeps station on where an enemy was last seen,
/// for a mind with [`Wits::SEARCHES`]: backing round a corner loses sight of
/// the enemy, and without this a [`SearchLastKnown`] below it would walk
/// straight back to the spot it just backed away from. It closes only to
/// `keep_within` of the spot, so it looks round the corner rather than
/// stepping up to it. An ally is never lost this way, since a roster of
/// allies is what is in sight and nothing remembers an ally that left.
///
/// A step never lengthens the gap while closing it, nor shortens it while
/// backing out: a field that leads round a pillar by way of the one it is
/// keeping station on is the way *to* it, not the way to keep a distance, so
/// a step the field offers that does either is passed over. Backing out of
/// sight is a last resort: a step away that keeps a clear line comes first,
/// which is what a spotter wants and what keeps a companion where you can
/// see it.
///
/// Cornered, with every step away taken or walled, it has nowhere to back
/// off to and returns `None`; the next tactic decides, and a [`Hover`]
/// beneath it stands its ground at whatever gap it has until the other
/// moves. That is the tactic's limit: getting past somebody who stands in
/// the only way out is a fight, and keeping station does not start one.
#[derive(Debug, Clone, Copy)]
pub struct Keep {
    /// Whose company it keeps.
    pub on: Station,
    /// How far it lets the nearest of them get before it closes.
    pub keep_within: i32,
    /// How near it lets the nearest of them come before it backs off.
    pub no_closer_than: i32,
}

impl Keep {
    /// Station on the allies: what a companion is.
    pub fn allies(keep_within: i32, no_closer_than: i32) -> Self {
        Self { on: Station::Allies, keep_within, no_closer_than }
    }

    /// Station on the enemies: what a spotter or a skirmisher is.
    pub fn enemies(keep_within: i32, no_closer_than: i32) -> Self {
        Self { on: Station::Enemies, keep_within, no_closer_than }
    }

    /// Who it is keeping station on: the nearest of them, every one of them
    /// for the field to flood from, and whether the nearest is somebody it
    /// can see rather than somewhere it remembers.
    fn watching<A: Copy>(&self, ctx: &TacticCtx<'_, A>) -> Option<(Point, Vec<Point>, bool)> {
        match self.on {
            Station::Allies => {
                let nearest = ctx.snapshot.allies.first()?;
                Some((nearest.pos, ctx.snapshot.allies.iter().map(|a| a.pos).collect(), true))
            }
            Station::Enemies => match ctx.snapshot.nearest_enemy() {
                Some(enemy) => Some((enemy.pos, ctx.snapshot.enemies.iter().map(|e| e.pos).collect(), true)),
                None if ctx.snapshot.wits.has(Wits::SEARCHES) => {
                    let spot = ctx.snapshot.last_known?;
                    Some((spot, vec![spot], false))
                }
                None => None,
            },
        }
    }
}

impl<A: Copy> Tactic<A> for Keep {
    /// The two names this behaviour has always had, because a log that says
    /// `shadow` says more about what happened than one that says `keep`.
    fn name(&self) -> &'static str {
        match self.on {
            Station::Allies => "follow",
            Station::Enemies => "shadow",
        }
    }

    fn evaluate(&self, ctx: &mut TacticCtx<'_, A>) -> Option<Decision<A>> {
        let me = ctx.snapshot.me.pos;
        let (nearest, marks, in_sight) = self.watching(ctx)?;
        let gap = geometry::chebyshev(me, nearest);
        if gap > self.keep_within {
            // Closing: the field's way first, and never a step that widens
            // the gap to the one being closed on.
            let closing = |p: Point| geometry::chebyshev(p, nearest) <= gap;
            if let Some(step) = ctx.fields.descents_toward(&marks, me).into_iter().find(|p| (ctx.can_step)(*p) && closing(*p)) {
                return Some(Decision::Step(step));
            }
            let toward = Direction::between(me, nearest)?;
            for d in [toward, toward.rotate_cw(), toward.rotate_ccw()] {
                let step = me + d.offset();
                if steps_to(ctx.can_step, me, step) && geometry::chebyshev(step, nearest) < gap {
                    return Some(Decision::Step(step));
                }
            }
            return None;
        }
        if gap >= self.no_closer_than {
            return None;
        }
        // Backing off. Steps that widen the gap before steps that only hold
        // it, the field's way first within each, then every direction
        // straightest first; and within each, one that keeps a clear line to
        // the other before one that does not.
        let field = ctx.fields.descents_away(&marks, me);
        let straight = Direction::between(nearest, me)?;
        let (cw, ccw) = (straight.rotate_cw(), straight.rotate_ccw());
        let turns = [straight, cw, ccw, cw.rotate_cw(), ccw.rotate_ccw(), cw.rotate_cw().rotate_cw(), ccw.rotate_ccw().rotate_ccw()];
        let direct = turns.iter().map(|d| me + d.offset()).filter(|p| steps_to(ctx.can_step, me, *p));
        let open: Vec<Point> = field.into_iter().chain(direct).filter(|p| (ctx.can_step)(*p)).collect();
        let widening = open.iter().copied().filter(|p| geometry::chebyshev(*p, nearest) > gap);
        let holding = open.iter().copied().filter(|p| geometry::chebyshev(*p, nearest) == gap);
        let clear = |p: Point| in_sight && clear_shot(p, nearest, geometry::chebyshev(p, nearest), ctx.bounds, |c| c != p && c != me && (ctx.blocks_shot)(c));
        for tier in [widening.collect::<Vec<_>>(), holding.collect::<Vec<_>>()] {
            if let Some(step) = tier.iter().copied().find(|p| clear(*p)).or(tier.first().copied()) {
                return Some(Decision::Step(step));
            }
        }
        None
    }
}

/// Whether stepping from `from` to the cell `to` beside it cuts a corner
/// between two cells the actor cannot step on, which the move resolver
/// refuses.
fn squeezes(can_step: &dyn Fn(Point) -> bool, from: Point, to: Point) -> bool {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    dx != 0 && dy != 0 && !(can_step(from.offset(dx, 0)) && can_step(from.offset(0, dy)))
}

/// Whether a step from `from` to the cell `to` beside it is one the move
/// resolver will actually take: somewhere the actor may stand, reached by a
/// way it may go.
///
/// Every tactic that picks a neighbour itself asks this rather than
/// `can_step` alone. Asking `can_step` alone is how a mind came to spend
/// turn after turn deciding on a diagonal the resolver refused: nothing
/// told it the step had not happened, so it decided the same way again on
/// its next turn, and a droid could shuffle at a doorway for as long as its
/// target stayed where it was.
///
/// It is stricter than the resolver by a hair, and deliberately so: the
/// resolver's corner rule reads terrain alone, while `can_step` also
/// refuses a cell that is occupied or on fire, so a mind declines a
/// diagonal whose crook holds an ally where the resolver would have let it
/// through. Declining a legal step costs one turn; deciding on an illegal
/// one costs every turn until the world moves.
fn steps_to(can_step: &dyn Fn(Point) -> bool, from: Point, to: Point) -> bool {
    can_step(to) && !squeezes(can_step, from, to)
}

/// Hold still while there is something to keep an eye on: an enemy in
/// sight, or where one was last seen for a mind with [`Wits::SEARCHES`].
///
/// What a spotter does in the band [`Keep`] leaves to the next tactic:
/// below it and above anything that closes in or drifts, so the turns at
/// the distance it keeps are spent watching, not hunting the gap shut or
/// wandering out of it. With nothing to watch it passes, and whatever is
/// below it, a search or a wander, has the turn.
#[derive(Debug, Clone, Copy, Default)]
pub struct Hover;

impl<A: Copy> Tactic<A> for Hover {
    fn name(&self) -> &'static str {
        "hover"
    }
    fn evaluate(&self, ctx: &mut TacticCtx<'_, A>) -> Option<Decision<A>> {
        let remembers = ctx.snapshot.wits.has(Wits::SEARCHES) && ctx.snapshot.last_known.is_some();
        (!ctx.snapshot.enemies.is_empty() || remembers).then_some(Decision::Wait)
    }
}

/// Drift: some chance of a random step, otherwise wait.
///
/// Never straight back where it came from while any other cell is open,
/// so a wanderer wanders rather than dithers between two cells.
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
        let back = ctx.snapshot.came_from;
        let start = ctx.rng.random_range(0..8);
        let mut only_back = None;
        for i in 0..8 {
            let step: Point = me + Direction::from_index((start + i) % 8).offset();
            if !steps_to(ctx.can_step, me, step) {
                continue;
            }
            if Some(step) == back {
                only_back = Some(step);
                continue;
            }
            return Some(Decision::Step(step));
        }
        Some(only_back.map_or(Decision::Wait, Decision::Step))
    }
}

/// Step out of the way of anyone who is neither friend nor foe standing
/// too close: a civilian in a crowd, a guard treading round a shopkeeper.
///
/// The first tactic over [`Snapshot::others`], and what a game with no
/// combat wants of a bystander: it fires when one of the others is within
/// `space`, steps down a field away from all of them, and otherwise lets
/// the next tactic have the turn.
#[derive(Debug, Clone, Copy)]
pub struct GiveWay {
    /// How close another may come before this one moves off.
    pub space: i32,
}

impl Default for GiveWay {
    /// Moves off anyone adjacent.
    fn default() -> Self {
        Self { space: 1 }
    }
}

impl<A: Copy> Tactic<A> for GiveWay {
    fn name(&self) -> &'static str {
        "give_way"
    }
    fn evaluate(&self, ctx: &mut TacticCtx<'_, A>) -> Option<Decision<A>> {
        let me = ctx.snapshot.me.pos;
        let crowd: Vec<Point> = ctx.snapshot.others.iter().map(|o| o.pos).collect();
        if !crowd.iter().any(|p| geometry::chebyshev(me, *p) <= self.space) {
            return None;
        }
        if let Some(step) = ctx.step_away_from(&crowd) {
            return Some(Decision::Step(step));
        }
        let nearest = ctx.snapshot.others.first()?.pos;
        let away = Direction::between(nearest, me)?;
        for d in [away, away.rotate_cw(), away.rotate_ccw()] {
            let step = me + d.offset();
            if steps_to(ctx.can_step, me, step) {
                return Some(Decision::Step(step));
            }
        }
        None
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
            if usable.aim.worth_aiming_at(Some(relation), is_user, actor.is_hurt()) {
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
            .filter(|(actor, relation, is_user)| usable.aim.worth_aiming_at(Some(*relation), *is_user, actor.is_hurt()))
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

/// Throw something carried at the nearest enemy it reaches down a clear
/// line, when that enemy is not already at its elbow.
///
/// Only for a mind with [`Wits::THROWS`] carrying something to throw. It
/// leaves an adjacent enemy to [`MeleeAdjacent`], which belongs above it: a
/// blow in reach beats a knife that has to be fetched back. The line is
/// judged by `blocks_shot`, the predicate a throw flies by, so a mind never
/// throws into a wall it thought was clear.
#[derive(Debug, Clone, Copy)]
pub struct ThrowAtRange {
    /// Percentage chance of throwing on a turn a throw is there to be made.
    /// Below a hundred so a thrower sometimes closes in instead.
    pub chance_pct: u32,
}

impl Default for ThrowAtRange {
    fn default() -> Self {
        Self { chance_pct: 100 }
    }
}

impl<A: Copy> Tactic<A> for ThrowAtRange {
    fn name(&self) -> &'static str {
        "throw_at_range"
    }

    fn evaluate(&self, ctx: &mut TacticCtx<'_, A>) -> Option<Decision<A>> {
        let s = ctx.snapshot;
        if !s.wits.has(Wits::THROWS) || s.missiles.is_empty() || s.enemies.is_empty() {
            return None;
        }
        if self.chance_pct < 100 && !ctx.rng.random_ratio(self.chance_pct.min(100), 100) {
            return None;
        }
        let me = s.me.pos;
        for enemy in &s.enemies {
            let distance = geometry::chebyshev(me, enemy.pos);
            if distance < 2 {
                continue;
            }
            let clear = |range| clear_shot(me, enemy.pos, range, ctx.bounds, |p| p != me && (ctx.blocks_shot)(p));
            if let Some(missile) = s.missiles.iter().find(|m| m.range >= distance && clear(m.range)) {
                return Some(Decision::Throw { item: missile.item, at: enemy.pos });
            }
        }
        None
    }
}

/// Shoot the nearest enemy in reach down a clear line, when that enemy is
/// not already at its elbow.
///
/// For any mind whose snapshot has a [`Snapshot::reach`]: firing what it
/// holds takes no wits, so a mindless sentry shoots as readily as a
/// sapient one. It leaves an adjacent enemy to [`MeleeAdjacent`], which
/// belongs above it. It shoots when there is a shot and does not back off
/// to keep one; holding a distance is a separate tactic. The line is judged
/// by `blocks_shot`, the predicate a shot flies by, so a mind never fires
/// into a wall it thought was clear.
///
/// `enemies` is nearest-first the way [`ThrowAtRange`] relies on
/// ([`Snapshot::sort`]), so the first in reach down a clear line is the
/// nearest one, not merely the first found.
#[derive(Debug, Clone, Copy)]
pub struct ShootAtRange {
    /// Percentage chance of shooting on a turn there is a shot to take.
    /// Below a hundred so a shooter sometimes closes in instead.
    pub chance_pct: u32,
}

impl Default for ShootAtRange {
    fn default() -> Self {
        Self { chance_pct: 100 }
    }
}

impl<A: Copy> Tactic<A> for ShootAtRange {
    fn name(&self) -> &'static str {
        "shoot_at_range"
    }

    fn evaluate(&self, ctx: &mut TacticCtx<'_, A>) -> Option<Decision<A>> {
        let s = ctx.snapshot;
        let reach = s.reach?;
        if s.enemies.is_empty() {
            return None;
        }
        if self.chance_pct < 100 && !ctx.rng.random_ratio(self.chance_pct.min(100), 100) {
            return None;
        }
        let me = s.me.pos;
        s.enemies
            .iter()
            .filter(|e| (2..=reach).contains(&geometry::chebyshev(me, e.pos)))
            .find(|e| clear_shot(me, e.pos, reach, ctx.bounds, |p| p != me && (ctx.blocks_shot)(p)))
            .map(|e| Decision::Attack(e.id))
    }
}

/// Fetch what is worth having from where it lies: gear better than what it
/// wears, or something to throw while it carries nothing to throw.
///
/// Standing on it, it takes it: [`Decision::EquipFromGround`] for gear,
/// which is quicker than picking up and putting on, and
/// [`Decision::PickUp`] for something to throw. Otherwise it walks toward
/// the nearest such thing no more than `reach` away, by the shortest walk
/// round whatever is in the way. Gear takes [`Wits::EQUIPS`]; something to
/// throw takes [`Wits::PICKS_UP`] and [`Wits::THROWS`], since a knife it
/// will never throw is not worth the walk.
///
/// Where it sits in a brain says when it is worth the detour: above
/// [`Hunt`], a thrower fetches a knife on its way into a fight; below it,
/// only once nothing is in sight.
#[derive(Debug, Clone, Copy)]
pub struct Scavenge {
    /// The furthest it will go for something, in steps.
    pub reach: i32,
}

impl Scavenge {
    /// Whether `item` is worth having to a mind that knows `snapshot`, and
    /// whether that is as gear.
    fn wanted<A: Copy>(snapshot: &Snapshot<A>, item: &ItemView<A>) -> Option<Worth> {
        if snapshot.wits.has(Wits::EQUIPS) && item.gain.is_some_and(|g| g > 0) {
            return Some(Worth::Gear);
        }
        let throws = snapshot.wits.has(Wits::PICKS_UP.with(Wits::THROWS));
        (throws && item.throw_range.is_some() && snapshot.missiles.is_empty()).then_some(Worth::Missile)
    }
}

/// Why something is worth fetching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Worth {
    Gear,
    Missile,
}

impl<A: Copy> Tactic<A> for Scavenge {
    fn name(&self) -> &'static str {
        "scavenge"
    }

    fn evaluate(&self, ctx: &mut TacticCtx<'_, A>) -> Option<Decision<A>> {
        let s = ctx.snapshot;
        let me = s.me.pos;
        if let Some((item, worth)) = s.items.iter().filter(|i| i.pos == me).find_map(|i| Self::wanted(s, i).map(|w| (i, w))) {
            return Some(match worth {
                Worth::Gear => Decision::EquipFromGround(item.id),
                Worth::Missile => Decision::PickUp,
            });
        }
        let target = s.items.iter().filter(|i| geometry::chebyshev(me, i.pos) <= self.reach).find(|i| Self::wanted(s, i).is_some())?;
        first_step(me, target.pos, self.reach + 2, ctx.can_step).map(Decision::Step)
    }
}

/// The first step of a shortest walk from `from` to `to` through cells
/// `can_step` allows, never straying more than `limit` from `from`.
///
/// A diagonal step may not cut between two cells it cannot step on, the
/// rule a move is resolved by, so the walk is one the mover can take. Ties
/// go to the first direction in [`Direction::ALL`], so two runs agree.
fn first_step(from: Point, to: Point, limit: i32, can_step: &dyn Fn(Point) -> bool) -> Option<Point> {
    if from == to {
        return None;
    }
    let side = 2 * limit + 1;
    let corner = Point::new(from.x - limit, from.y - limit);
    let index = |p: Point| {
        let (x, y) = (p.x - corner.x, p.y - corner.y);
        (x >= 0 && y >= 0 && x < side && y < side).then_some((y * side + x) as usize)
    };
    // Where each reached cell was reached from, in a square around `from`
    // rather than a map, so the search allocates once and in order.
    let mut came_from: Vec<Option<Point>> = vec![None; (side * side) as usize];
    came_from[index(from)?] = Some(from);
    let mut frontier = VecDeque::from([from]);
    while let Some(at) = frontier.pop_front() {
        for d in Direction::ALL {
            let next = at + d.offset();
            let Some(i) = index(next) else { continue };
            let (dx, dy) = d.delta();
            let squeezes = d.is_diagonal() && !(can_step(at.offset(dx, 0)) && can_step(at.offset(0, dy)));
            if came_from[i].is_some() || squeezes || !can_step(next) {
                continue;
            }
            came_from[i] = Some(at);
            if next == to {
                let mut step = next;
                loop {
                    let prev = came_from[index(step)?]?;
                    if prev == from {
                        return Some(step);
                    }
                    step = prev;
                }
            }
            frontier.push_back(next);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::brain::{Brain, Fields, NoFields};
    use crate::ai::snapshot::Missile;
    use crate::ai::snapshot::{ActorView, Snapshot, Vitals};
    use rand::{SeedableRng, rngs::StdRng};
    use rl_core::Id;
    use rl_grid::{DijkstraMap, PathRules, Terrain, TileRegistry};

    fn view(id: u32, x: i32, y: i32, hp: i32) -> ActorView<u32> {
        ActorView { id, pos: Point::new(x, y), health: Some(Vitals { current: hp, max: 10 }), faction: Some(Id::from_raw(0)) }
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

    /// Fields built on demand over one terrain, the way the engine builds
    /// them, so a test of a tactic is a test of the tactic and not of a
    /// hand-made map.
    struct Given<'a> {
        view: rl_grid::TerrainView<'a>,
    }

    impl<'a> Given<'a> {
        fn over(view: &rl_grid::TerrainView<'a>) -> Self {
            Self { view: view.clone() }
        }

        fn field(&self, goals: &[Point], away: bool) -> DijkstraMap {
            let mut map = DijkstraMap::covering(&self.view);
            map.build(&self.view, goals.iter().copied(), PathRules::default());
            if away {
                map.scale(-12, 10);
                map.rescan(&self.view, PathRules::default());
            }
            map
        }
    }

    impl Fields for Given<'_> {
        fn descents_toward(&mut self, goals: &[Point], from: Point) -> Vec<Point> {
            self.field(goals, false).descents(from)
        }

        fn descents_away(&mut self, goals: &[Point], from: Point) -> Vec<Point> {
            self.field(goals, true).descents(from)
        }
    }

    /// A floor with the crook of a corner walled off: the cells east and
    /// south of (5, 5) are wall, so the step southeast from it is a
    /// diagonal squeezing between two walls, which the move resolver
    /// refuses. Everything else is open.
    fn cornered() -> (Terrain, TileRegistry) {
        let r = TileRegistry::standard();
        let mut t = Terrain::filled(12, 12, r.expect("floor"));
        let wall = r.expect("wall");
        t.set(Point::new(6, 5), wall);
        t.set(Point::new(5, 6), wall);
        (t, r)
    }

    /// No tactic decides on a diagonal that squeezes between two walls.
    ///
    /// The move resolver refuses that step, and refuses it silently: a mind
    /// that decided on it spent the turn, moved nowhere, and decided the
    /// same way on its next turn, so a droid could shuffle at a doorway for
    /// as long as its target stood still. Every tactic that picks a
    /// neighbour itself is here, because every one of them had the bug and
    /// they all now ask `steps_to`.
    #[test]
    fn no_tactic_decides_on_a_diagonal_that_squeezes_between_two_walls() {
        let (t, r) = cornered();
        let view_t = t.view(&r);
        let can_step = |p: Point| view_t.is_walkable(p);
        let mut rng = StdRng::seed_from_u64(1);
        let me = Point::new(5, 5);
        let corner = Point::new(6, 6);
        assert!(can_step(corner), "the cell itself is open; it is the way in that is not");

        // Each tactic, and a mind it fires for whose straight-line
        // fallback points at that corner: something to chase southeast,
        // something to run from northwest, or a friend to catch up with.
        let mut cases: Vec<(&str, Brain<u32>, Snapshot<u32>)> = Vec::new();

        let mut hunting = Snapshot::alone(view(1, me.x, me.y, 10));
        hunting.enemies.push(view(2, 9, 9, 10));
        cases.push(("hunt", Brain::new().then(Hunt), hunting));

        let mut searching = Snapshot::alone(view(1, me.x, me.y, 10));
        searching.last_known = Some(Point::new(9, 9));
        cases.push(("search_last_known", Brain::new().then(SearchLastKnown), searching));

        let mut fleeing = Snapshot::alone(view(1, me.x, me.y, 1));
        fleeing.enemies.push(view(2, 4, 4, 10));
        cases.push(("flee_when_hurt", Brain::new().then(FleeWhenHurt { at_pct: 50 }), fleeing));

        let mut following = Snapshot::alone(view(1, me.x, me.y, 10));
        following.allies.push(view(3, 9, 9, 10));
        cases.push(("follow", Brain::new().then(Keep::allies(3, 1)), following));

        let mut spotting = Snapshot::alone(view(1, me.x, me.y, 10));
        spotting.enemies.push(view(2, 9, 9, 10));
        cases.push(("shadow", Brain::new().then(Keep::enemies(5, 3)), spotting));

        let mut crowded = Snapshot::alone(view(1, me.x, me.y, 10));
        crowded.others.push(view(4, 4, 4, 10));
        cases.push(("give_way", Brain::new().then(GiveWay::default()), crowded));

        for (name, brain, snapshot) in &cases {
            let (decision, which) = brain.decide(&mut TacticCtx {
                snapshot,
                fields: &mut NoFields,
                can_step: &can_step,
                blocks_shot: &nothing_blocks,
                bounds: arena(),
                rng: &mut rng,
            });
            assert_ne!(decision, Decision::Step(corner), "{name} decided on the squeeze: {which:?}");
        }
    }

    /// A drift with nowhere to drift but through a corner waits.
    ///
    /// Its own terrain, because it is the one tactic that tries all eight
    /// ways: everything round it is walled but the one diagonal it may not
    /// take.
    #[test]
    fn a_wander_with_only_a_squeeze_open_waits_instead() {
        let r = TileRegistry::standard();
        let mut t = Terrain::filled(12, 12, r.expect("wall"));
        let (me, corner) = (Point::new(5, 5), Point::new(6, 6));
        t.set(me, r.expect("floor"));
        t.set(corner, r.expect("floor"));
        let view_t = t.view(&r);
        let can_step = |p: Point| view_t.is_walkable(p);
        let mut rng = StdRng::seed_from_u64(7);

        let b: Brain<u32> = Brain::new().then(Wander { chance_pct: 100 });
        let snapshot = Snapshot::alone(view(1, me.x, me.y, 10));
        let (decision, _) = b.decide(&mut TacticCtx {
            snapshot: &snapshot,
            fields: &mut NoFields,
            can_step: &can_step,
            blocks_shot: &nothing_blocks,
            bounds: arena(),
            rng: &mut rng,
        });
        assert_eq!(decision, Decision::Wait, "the only open cell is through a corner, so there is nowhere to drift");
    }

    #[test]
    fn a_lost_enemy_is_searched_for_where_it_was_seen_and_hunting_outranks_it() {
        let (t, r) = open();
        let view_t = t.view(&r);
        let can_step = |p: Point| view_t.is_walkable(p);
        let mut rng = StdRng::seed_from_u64(1);
        let b: Brain<u32> = Brain::new().then(MeleeAdjacent).then(Hunt).then(SearchLastKnown).then(Wander { chance_pct: 0 });
        let mut decide = |snapshot: &Snapshot<u32>| {
            b.decide(&mut TacticCtx { snapshot, fields: &mut NoFields, can_step: &can_step, blocks_shot: &nothing_blocks, bounds: arena(), rng: &mut rng })
        };

        let mut lost = Snapshot::alone(view(1, 5, 5, 10));
        lost.last_known = Some(Point::new(9, 5));
        assert_eq!(decide(&lost), (Decision::Step(Point::new(6, 5)), Some("search_last_known")));

        let mut seen = Snapshot::alone(view(1, 5, 5, 10));
        seen.last_known = Some(Point::new(9, 5));
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
            b.decide(&mut TacticCtx { snapshot, fields: &mut NoFields, can_step: &can_step, blocks_shot: &nothing_blocks, bounds: arena(), rng: &mut rng }).1
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
            fields: &mut NoFields,
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
            fields: &mut NoFields,
            can_step: &can_step,
            blocks_shot: &nothing_blocks,
            bounds: arena(),
            rng: &mut rng,
        });
        assert_eq!((d, who), (Decision::Step(Point::new(4, 5)), Some("flee_when_hurt")));

        let mut far = Snapshot::alone(view(1, 5, 5, 10));
        far.enemies.push(view(2, 8, 5, 10));
        let mut fields = Given::over(&view_t);
        let (d, who) =
            b.decide(&mut TacticCtx { snapshot: &far, fields: &mut fields, can_step: &can_step, blocks_shot: &nothing_blocks, bounds: arena(), rng: &mut rng });
        assert_eq!((d, who), (Decision::Step(Point::new(6, 5)), Some("hunt")));

        let alone = Snapshot::alone(view(1, 5, 5, 10));
        let (d, who) = b.decide(&mut TacticCtx {
            snapshot: &alone,
            fields: &mut NoFields,
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
                fields: &mut NoFields,
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
        let mut hurt = Snapshot::alone(view(1, 5, 5, 1));
        hurt.enemies.push(view(2, 8, 5, 10));
        let blocked = |p: Point| p != Point::new(4, 5) && view_t.is_walkable(p);
        let mut rng = StdRng::seed_from_u64(1);
        let mut fields = Given::over(&view_t);
        let (d, _) = brain().decide(&mut TacticCtx {
            snapshot: &hurt,
            fields: &mut fields,
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
        assert_eq!(view(1, 0, 0, 3).hp_pct(), Some(30));
        assert_eq!(ActorView::at(9u32, Point::ZERO).hp_pct(), None, "no health, no percentage");
        assert!(!ActorView::at(9u32, Point::ZERO).is_hurt(), "and never hurt, so nothing mends it");
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
        let mut ctx = TacticCtx { snapshot: &many, fields: &mut NoFields, can_step: &can_step, blocks_shot: &nothing_blocks, bounds: arena(), rng: &mut rng };
        let decision = UseAbility::default().evaluate(&mut ctx);
        let Some(Decision::Ability { ability, aim }) = decision else { panic!("expected an ability, got {decision:?}") };
        assert_eq!(ability, Id::from_raw(0));
        assert!(aim == Point::new(6, 5) || aim == Point::new(6, 6), "it aimed at the huddle: {aim:?}");

        // A burst on the ground takes no sides: put an ally in the huddle
        // and the loner becomes the better shot. A foe-aimed burst passes
        // the ally by, which the next test pins.
        let mut mixed = Snapshot::alone(view(1, 0, 5, 10));
        mixed.usable = vec![usable(0, Aim::Ground, rl_grid::TargetMode::Ball { range: 8, radius: 1 })];
        mixed.allies = vec![view(9, 6, 6, 10)];
        mixed.enemies = vec![view(2, 3, 8, 10), view(3, 6, 5, 10)];
        let mut ctx = TacticCtx { snapshot: &mixed, fields: &mut NoFields, can_step: &can_step, blocks_shot: &nothing_blocks, bounds: arena(), rng: &mut rng };
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
        let mut ctx = TacticCtx { snapshot: &s, fields: &mut NoFields, can_step: &can_step, blocks_shot: &nothing_blocks, bounds: arena(), rng: &mut rng };
        let Some(Decision::Ability { aim, .. }) = UseAbility::default().evaluate(&mut ctx) else { panic!("expected an ability") };
        assert_ne!(aim, Point::new(3, 8), "the ally under the pair costs a foe-aimed burst nothing");

        // The same room with the burst on the ground: now the ally burns,
        // and the loner is the better shot.
        s.usable = vec![usable(0, Aim::Ground, rl_grid::TargetMode::Ball { range: 8, radius: 1 })];
        let mut ctx = TacticCtx { snapshot: &s, fields: &mut NoFields, can_step: &can_step, blocks_shot: &nothing_blocks, bounds: arena(), rng: &mut rng };
        let Some(Decision::Ability { aim, .. }) = UseAbility::default().evaluate(&mut ctx) else { panic!("expected an ability") };
        assert_eq!(aim, Point::new(3, 8));

        // Hurt and alone, with a spray aimed at allies: the one ally in
        // reach is itself.
        let mut hurt = Snapshot::alone(view(1, 0, 5, 4));
        hurt.usable = vec![usable(0, Aim::Ally, rl_grid::TargetMode::Adjacent)];
        let mut ctx = TacticCtx { snapshot: &hurt, fields: &mut NoFields, can_step: &can_step, blocks_shot: &nothing_blocks, bounds: arena(), rng: &mut rng };
        assert_eq!(UseAbility::default().evaluate(&mut ctx), Some(Decision::Ability { ability: Id::from_raw(0), aim: Point::new(0, 5) }));

        // Whole again, there is nothing to mend.
        let mut whole = Snapshot::alone(view(1, 0, 5, 10));
        whole.usable = vec![usable(0, Aim::Ally, rl_grid::TargetMode::Adjacent)];
        let mut ctx = TacticCtx { snapshot: &whole, fields: &mut NoFields, can_step: &can_step, blocks_shot: &nothing_blocks, bounds: arena(), rng: &mut rng };
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
        let mut ctx = TacticCtx { snapshot: &alone, fields: &mut NoFields, can_step: &can_step, blocks_shot: &nothing_blocks, bounds: arena(), rng: &mut rng };
        assert_eq!(UseAbility::default().evaluate(&mut ctx), None, "no enemies, no aim");

        // A wall between: the bolt stops short, so it covers nothing.
        let mut walled = Snapshot::alone(view(1, 0, 5, 10));
        walled.enemies = vec![view(2, 5, 5, 10)];
        walled.usable = vec![usable(0, Aim::Foe, rl_grid::TargetMode::Bolt { range: 6 })];
        let wall = |p: Point| p == Point::new(2, 5);
        let mut ctx = TacticCtx { snapshot: &walled, fields: &mut NoFields, can_step: &can_step, blocks_shot: &wall, bounds: arena(), rng: &mut rng };
        assert_eq!(UseAbility::default().evaluate(&mut ctx), None, "the wall is in the way");
    }

    fn missile(item: u32, range: i32) -> Missile<u32> {
        Missile { item, range }
    }

    /// A thrower throws at what it reaches down a clear line, and leaves
    /// what is at its elbow to the blow above it in the brain.
    #[test]
    fn a_thrower_throws_down_a_clear_line_at_what_is_out_of_arms_reach() {
        let mut rng = StdRng::seed_from_u64(2);
        let can_step = |_: Point| true;
        let decide = |s: &Snapshot<u32>, blocks: &dyn Fn(Point) -> bool, rng: &mut StdRng| {
            ThrowAtRange::default().evaluate(&mut TacticCtx {
                snapshot: s,
                fields: &mut NoFields,
                can_step: &can_step,
                blocks_shot: blocks,
                bounds: arena(),
                rng,
            })
        };
        let mut s = Snapshot::alone(view(1, 0, 5, 10));
        s.missiles = vec![missile(40, 5)];
        s.enemies = vec![view(2, 4, 5, 10)];
        assert_eq!(decide(&s, &nothing_blocks, &mut rng), Some(Decision::Throw { item: 40, at: Point::new(4, 5) }));
        let wall = |p: Point| p == Point::new(2, 5);
        assert_eq!(decide(&s, &wall, &mut rng), None, "not through a wall");
        s.enemies = vec![view(2, 8, 5, 10)];
        assert_eq!(decide(&s, &nothing_blocks, &mut rng), None, "not past its reach");
        s.enemies = vec![view(2, 1, 5, 10)];
        assert_eq!(decide(&s, &nothing_blocks, &mut rng), None, "not at its elbow, where a blow is better");
        s.enemies = vec![view(2, 4, 5, 10)];
        s.wits = Wits::ANIMAL;
        assert_eq!(decide(&s, &nothing_blocks, &mut rng), None, "and never without the wits to throw");
    }

    /// A snapshot of a shooter alone at the origin with `reach`, facing one
    /// enemy at `enemy_at`.
    fn shooter_snapshot(reach: Option<i32>, enemy_at: Point) -> Snapshot<u32> {
        let mut s = Snapshot::alone(view(1, 0, 0, 10));
        s.reach = reach;
        s.enemies = vec![view(2, enemy_at.x, enemy_at.y, 10)];
        s
    }

    /// Evaluates `tactic` against `s`, with `blocks_shot` as the line of
    /// fire's predicate and room enough that no shape clips the arena.
    fn decide(tactic: &ShootAtRange, s: &Snapshot<u32>, blocks_shot: impl Fn(Point) -> bool) -> Option<Decision<u32>> {
        let can_step = |_: Point| true;
        let mut rng = StdRng::seed_from_u64(1);
        tactic.evaluate(&mut TacticCtx { snapshot: s, fields: &mut NoFields, can_step: &can_step, blocks_shot: &blocks_shot, bounds: arena(), rng: &mut rng })
    }

    #[test]
    fn a_shooter_fires_down_a_clear_line_at_an_enemy_in_reach() {
        let s = shooter_snapshot(Some(5), Point::new(4, 0));
        let d = decide(&ShootAtRange::default(), &s, |_| false);
        assert!(matches!(d, Some(Decision::Attack(_))), "an enemy four off, reach five, clear: shoot");
    }

    #[test]
    fn a_shooter_holds_fire_when_the_line_is_blocked_the_enemy_is_out_of_reach_or_at_its_elbow() {
        // Each is its own reason to decline, and each must decline on its own.
        let blocked = decide(&ShootAtRange::default(), &shooter_snapshot(Some(5), Point::new(4, 0)), |p| p == Point::new(2, 0));
        let far = decide(&ShootAtRange::default(), &shooter_snapshot(Some(5), Point::new(7, 0)), |_| false);
        let close = decide(&ShootAtRange::default(), &shooter_snapshot(Some(5), Point::new(1, 0)), |_| false);
        let unarmed = decide(&ShootAtRange::default(), &shooter_snapshot(None, Point::new(4, 0)), |_| false);
        assert!(blocked.is_none(), "a wall in the way");
        assert!(far.is_none(), "seven off with a reach of five");
        assert!(close.is_none(), "adjacent is MeleeAdjacent's");
        assert!(unarmed.is_none(), "nothing to shoot with");
    }

    /// A scavenger walks round a wall to gear better than what it wears,
    /// puts it on where it lies, and leaves gear worse than its own alone.
    #[test]
    fn a_scavenger_fetches_better_gear_round_a_wall_and_puts_it_on_where_it_lies() {
        let (mut t, r) = open();
        for y in 3..=7 {
            t.set(Point::new(6, y), r.expect("wall"));
        }
        let view_t = t.view(&r);
        let can_step = |p: Point| view_t.is_walkable(p);
        let mut rng = StdRng::seed_from_u64(3);
        let blade = ItemView { id: 50, pos: Point::new(8, 5), throw_range: None, gain: Some(2) };
        let rag = ItemView { id: 51, pos: Point::new(4, 5), throw_range: None, gain: Some(-1) };
        let mut me = Point::new(5, 5);
        for turn in 0..16 {
            let mut s = Snapshot::alone(view(1, me.x, me.y, 10));
            s.items = vec![rag, blade];
            s.sort();
            let mut ctx = TacticCtx { snapshot: &s, fields: &mut NoFields, can_step: &can_step, blocks_shot: &nothing_blocks, bounds: arena(), rng: &mut rng };
            let decision = Scavenge { reach: 5 }.evaluate(&mut ctx);
            match decision {
                Some(Decision::Step(to)) => {
                    assert!(view_t.is_walkable(to) && geometry::is_adjacent(me, to), "turn {turn}: a step from {me:?} to {to:?}");
                    me = to;
                }
                Some(Decision::EquipFromGround(id)) => {
                    assert_eq!((id, me), (50, blade.pos), "turn {turn}: put on the blade where it lay");
                    return;
                }
                other => panic!("turn {turn}: {other:?}"),
            }
        }
        panic!("never reached the blade, and stopped at {me:?}");
    }

    /// Something to throw is worth picking up while there is nothing in
    /// hand to throw, and only to a mind that would throw it.
    #[test]
    fn a_scavenger_takes_something_to_throw_only_while_it_has_nothing_to_throw() {
        let mut rng = StdRng::seed_from_u64(3);
        let can_step = |_: Point| true;
        let mut decide = |s: &Snapshot<u32>| {
            Scavenge { reach: 4 }.evaluate(&mut TacticCtx {
                snapshot: s,
                fields: &mut NoFields,
                can_step: &can_step,
                blocks_shot: &nothing_blocks,
                bounds: arena(),
                rng: &mut rng,
            })
        };
        let mut s = Snapshot::alone(view(1, 5, 5, 10));
        s.items = vec![ItemView { id: 60, pos: Point::new(5, 5), throw_range: Some(5), gain: None }];
        assert_eq!(decide(&s), Some(Decision::PickUp), "underfoot, it is taken up");
        s.missiles = vec![missile(61, 5)];
        assert_eq!(decide(&s), None, "with one in hand, the next is left");
        s.missiles.clear();
        s.wits = Wits::ANIMAL;
        assert_eq!(decide(&s), None, "an animal has no use for it");
        s.wits = Wits::SAPIENT.without(Wits::THROWS);
        assert_eq!(decide(&s), None, "nor has a mind that will not throw it");
    }

    /// An actor with nothing to use never reaches for one.
    #[test]
    fn a_monster_with_no_abilities_never_offers_one() {
        let mut rng = StdRng::seed_from_u64(1);
        let can_step = |_: Point| true;
        let mut sees = Snapshot::alone(view(1, 0, 0, 10));
        sees.enemies = vec![view(2, 1, 0, 10)];
        let mut ctx = TacticCtx { snapshot: &sees, fields: &mut NoFields, can_step: &can_step, blocks_shot: &nothing_blocks, bounds: arena(), rng: &mut rng };
        assert_eq!(UseAbility::default().evaluate(&mut ctx), None);
    }

    /// A station-keeper's answer for one actor at `me` and one enemy at `foe` on open
    /// floor, in sight or only remembered there, with `blocked` cells
    /// nobody can step on or shoot through.
    fn shadow_of(tactic: Keep, me: Point, foe: Point, blocked: &[Point], in_sight: bool) -> Option<Decision<u32>> {
        let (t, r) = open();
        let view_t = t.view(&r);
        let can_step = |p: Point| !blocked.contains(&p) && p != foe && view_t.is_walkable(p);
        let blocks_shot = |p: Point| blocked.contains(&p) || p == foe;
        let mut rng = StdRng::seed_from_u64(1);
        let mut fields = Given::over(&view_t);
        let mut s = Snapshot::alone(view(1, me.x, me.y, 10));
        if in_sight {
            s.enemies.push(view(2, foe.x, foe.y, 10));
        } else {
            s.last_known = Some(foe);
        }
        let mut ctx = TacticCtx { snapshot: &s, fields: &mut fields, can_step: &can_step, blocks_shot: &blocks_shot, bounds: arena(), rng: &mut rng };
        tactic.evaluate(&mut ctx)
    }

    fn shadow(tactic: Keep, me: Point, foe: Point, blocked: &[Point]) -> Option<Decision<u32>> {
        shadow_of(tactic, me, foe, blocked, true)
    }

    /// Whether a shot from `from` reaches `to` past `blocked`.
    fn in_line(from: Point, to: Point, blocked: &[Point]) -> bool {
        clear_shot(from, to, geometry::chebyshev(from, to), arena(), |p| p != from && (blocked.contains(&p) || p == to))
    }

    /// Out of sight, a shadow keeps its distance from where the enemy was
    /// last seen as it would from the enemy, rather than leaving the next
    /// tactic to walk it back in; a mind without the wits to follow a
    /// trail has nothing to keep its distance from.
    #[test]
    fn a_shadow_that_has_lost_sight_keeps_its_distance_from_where_the_enemy_was_last_seen() {
        let keep = Keep::enemies(5, 3);
        let seen = Point::new(2, 5);
        let gap = |d: Option<Decision<u32>>| match d {
            Some(Decision::Step(p)) => geometry::chebyshev(p, seen),
            other => panic!("expected a step, got {other:?}"),
        };
        assert_eq!(gap(shadow_of(keep, Point::new(9, 5), seen, &[], false)), 6, "seven off the place, it closes a step");
        assert_eq!(gap(shadow_of(keep, Point::new(3, 5), seen, &[], false)), 2, "one off the place, it backs away");
        assert_eq!(shadow_of(keep, Point::new(6, 5), seen, &[], false), None, "four off is inside the band");
        let (t, r) = open();
        let view_t = t.view(&r);
        let can_step = |p: Point| view_t.is_walkable(p);
        let mut rng = StdRng::seed_from_u64(1);
        let mut s = Snapshot::alone(view(1, 3, 5, 10));
        s.last_known = Some(seen);
        s.wits = Wits::MINDLESS;
        let mut ctx = TacticCtx { snapshot: &s, fields: &mut NoFields, can_step: &can_step, blocks_shot: &nothing_blocks, bounds: arena(), rng: &mut rng };
        assert_eq!(keep.evaluate(&mut ctx), None, "a mindless thing does not remember where");
    }

    /// The same tactic, the other station: a companion closes on the one it
    /// follows, gives way when crowded, and leaves the band between alone.
    ///
    /// Giving way is what the consolidation changed for allies: the old
    /// `Follow` stepped down a field away and took whatever it offered, and
    /// this never takes a step that brings it nearer, which is the rule the
    /// enemy-facing half always had.
    #[test]
    fn a_companion_closes_gives_way_when_crowded_and_never_gives_way_nearer() {
        let keep = Keep::allies(3, 2);
        let (t, r) = open();
        let view_t = t.view(&r);
        let can_step = |p: Point| view_t.is_walkable(p);
        let mut rng = StdRng::seed_from_u64(4);
        let mate = Point::new(5, 5);
        let decide = |me: Point, rng: &mut StdRng| {
            let mut snapshot = Snapshot::alone(view(1, me.x, me.y, 10));
            snapshot.allies.push(view(3, mate.x, mate.y, 10));
            let mut fields = Given::over(&view_t);
            let mut ctx = TacticCtx { snapshot: &snapshot, fields: &mut fields, can_step: &can_step, blocks_shot: &nothing_blocks, bounds: arena(), rng };
            keep.evaluate(&mut ctx)
        };
        let gap = |d: Option<Decision<u32>>| match d {
            Some(Decision::Step(p)) => geometry::chebyshev(p, mate),
            other => panic!("expected a step, got {other:?}"),
        };

        assert_eq!(gap(decide(Point::new(10, 5), &mut rng)), 4, "five off, it closes a step");
        assert_eq!(gap(decide(Point::new(6, 5), &mut rng)), 2, "beside it, it gives way a step");
        for x in 7..=8 {
            assert_eq!(decide(Point::new(x, 5), &mut rng), None, "{} off is inside the band", x - 5);
        }

        let mut lonely = Snapshot::alone(view(1, 9, 5, 10));
        lonely.enemies.push(view(2, 5, 5, 10));
        let mut fields = Given::over(&view_t);
        let mut ctx = TacticCtx { snapshot: &lonely, fields: &mut fields, can_step: &can_step, blocks_shot: &nothing_blocks, bounds: arena(), rng: &mut rng };
        assert_eq!(keep.evaluate(&mut ctx), None, "an enemy is not company: with no ally in sight there is nobody to follow");
    }

    /// A spotter closes on an enemy too far off, backs away from one too
    /// near, and in between leaves the turn to whatever comes after it.
    #[test]
    fn a_shadow_closes_on_a_far_enemy_backs_off_a_near_one_and_leaves_the_band_between_to_the_next_tactic() {
        let keep = Keep::enemies(5, 3);
        let foe = Point::new(2, 5);
        let gap = |d: Option<Decision<u32>>| match d {
            Some(Decision::Step(p)) => geometry::chebyshev(p, foe),
            other => panic!("expected a step, got {other:?}"),
        };
        assert_eq!(gap(shadow(keep, Point::new(9, 5), foe, &[])), 6, "seven off, it closes a step");
        assert_eq!(gap(shadow(keep, Point::new(3, 5), foe, &[])), 2, "one off, it backs a step away");
        for x in 5..=7 {
            assert_eq!(shadow(keep, Point::new(x, 5), foe, &[]), None, "{} off is inside the band", x - 2);
        }
        let mut alone = Snapshot::alone(view(1, 5, 5, 10));
        alone.allies.push(view(3, 9, 5, 10));
        let mut rng = StdRng::seed_from_u64(1);
        let can_step = |_: Point| true;
        let mut ctx = TacticCtx { snapshot: &alone, fields: &mut NoFields, can_step: &can_step, blocks_shot: &nothing_blocks, bounds: arena(), rng: &mut rng };
        assert_eq!(keep.evaluate(&mut ctx), None, "with no enemy in sight there is nobody to shadow");
    }

    /// Over a spread of places and cells taken, and with the enemy in
    /// sight or only remembered, a shadow's step never brings it nearer an
    /// enemy already inside `no_closer_than`, never takes it further from
    /// one already past `keep_within`, is always taken when a step that
    /// helps is there to take, and backs off within sight of an enemy it
    /// can see whenever a step away allows it.
    #[test]
    fn a_shadow_step_never_shortens_a_gap_it_is_backing_out_of_nor_lengthens_one_it_is_closing_over_a_range_of_seeds() {
        let tactic = Keep::enemies(5, 3);
        let around = |p: Point| Direction::ALL.into_iter().map(move |d| p + d.offset());
        for seed in 0..400u64 {
            let mut rng = StdRng::seed_from_u64(seed);
            let mut cell = || Point::new(rng.random_range(0..12), rng.random_range(0..12));
            let (me, foe) = (cell(), cell());
            let blocked: Vec<Point> = (0..6).map(|_| cell()).filter(|p| *p != me && *p != foe).collect();
            if me == foe {
                continue;
            }
            let (t, r) = open();
            let view_t = t.view(&r);
            let free = |p: Point| !blocked.contains(&p) && p != foe && view_t.is_walkable(p);
            let gap = geometry::chebyshev(me, foe);
            let in_sight = seed % 2 == 0;
            let decided = shadow_of(tactic, me, foe, &blocked, in_sight);
            let after = match decided {
                Some(Decision::Step(p)) => {
                    assert!(free(p) && geometry::chebyshev(me, p) == 1, "seed {seed}: {me:?} stepped to {p:?}, which is no step");
                    Some(geometry::chebyshev(p, foe))
                }
                None => None,
                other => panic!("seed {seed}: a shadow only steps or passes, not {other:?}"),
            };
            if gap > tactic.keep_within {
                if let Some(after) = after {
                    assert!(after <= gap, "seed {seed}: closing from {gap}, it stepped out to {after}");
                }
                let helps = around(me).any(|p| free(p) && geometry::chebyshev(p, foe) < gap);
                assert!(!helps || after.is_some(), "seed {seed}: {gap} off with a step toward to take at {me:?}, it passed");
            } else if gap < tactic.no_closer_than {
                if let Some(after) = after {
                    assert!(after >= gap, "seed {seed}: backing out from {gap}, it stepped in to {after}");
                }
                let helps = around(me).any(|p| free(p) && geometry::chebyshev(p, foe) > gap);
                assert!(!helps || after.is_some(), "seed {seed}: {gap} off with a step away to take at {me:?}, it passed");
                let watching = around(me).any(|p| free(p) && geometry::chebyshev(p, foe) > gap && in_line(p, foe, &blocked));
                if let (true, true, Some(Decision::Step(p))) = (in_sight, watching, decided) {
                    assert!(in_line(p, foe, &blocked), "seed {seed}: it backed out of sight to {p:?} with a step in sight to take");
                }
            } else {
                assert_eq!(decided, None, "seed {seed}: {gap} off is inside the band");
            }
        }
    }

    /// A hover waits while there is something to keep an eye on, an enemy
    /// in sight or where one was last seen, and otherwise lets the next
    /// tactic have the turn.
    #[test]
    fn a_hover_waits_while_it_has_something_to_watch_and_passes_when_it_has_nothing() {
        let mut rng = StdRng::seed_from_u64(1);
        let can_step = |_: Point| true;
        let mut decide = |s: &Snapshot<u32>| {
            Hover.evaluate(&mut TacticCtx {
                snapshot: s,
                fields: &mut NoFields,
                can_step: &can_step,
                blocks_shot: &nothing_blocks,
                bounds: arena(),
                rng: &mut rng,
            })
        };
        let mut watching = Snapshot::alone(view(1, 5, 5, 10));
        watching.enemies.push(view(2, 9, 5, 10));
        assert_eq!(decide(&watching), Some(Decision::Wait), "an enemy in sight");
        let mut remembering = Snapshot::alone(view(1, 5, 5, 10));
        remembering.last_known = Some(Point::new(9, 5));
        assert_eq!(decide(&remembering), Some(Decision::Wait), "where one was last seen");
        remembering.wits = Wits::MINDLESS;
        assert_eq!(decide(&remembering), None, "a mindless thing does not remember where");
        assert_eq!(decide(&Snapshot::alone(view(1, 5, 5, 10))), None, "nothing to watch");
    }
}
