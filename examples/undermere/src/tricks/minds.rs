//! What a mind decides by that no trait's fold says: a thief running
//! with what it took, a bat's erratic step, and a floor's own patrols
//! and anchors.

use super::*;

/// Marks a thief that has just taken something and should run rather than
/// fight for it: set the moment [`steal`] lands, and read as the
/// [`Stole`] sense by [`sense_stole`] for [`FleeAfterStealing`] to find.
/// Never taken off again: a monkey with the way out in sight keeps
/// fleeing it until it is through or killed, there being no "it got away
/// clean" to undo this by.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Stole;

/// Tells the mind holding the turn that it has [`Stole`]n, if it has.
///
/// In `PerceiveSet::Annotate`, beside every other sense a game pushes.
pub fn sense_stole(mut thinking: ResMut<Thinking>, stolen: Query<(), With<Stole>>) {
    let Some(thinker) = thinking.actor() else { return };
    if stolen.contains(thinker)
        && let Some(snapshot) = thinking.snapshot_mut()
    {
        snapshot.add_sense(Stole);
    }
}

/// Runs for the floor's own way out, or away from the nearest enemy with
/// none in sight, for a mind that has [`Stole`]n: a monkey's `stealing`.
///
/// Outranks every other tactic in the brain that carries it, `stealing`'s
/// own, above even `FleeWhenHurt`: the job is done, and nothing is worth
/// trading a blow for now. `stealing` grants `travels` itself, whose
/// `Wits::FLEES_THROUGH` half is what brings the `Ways` sense a mind
/// without it never gets, always there to flee by once something has
/// actually been taken.
#[derive(Debug, Clone, Copy, Default)]
pub struct FleeAfterStealing;

impl Tactic<Entity> for FleeAfterStealing {
    fn name(&self) -> &'static str {
        "flee_after_stealing"
    }

    fn evaluate(&self, ctx: &mut TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
        ctx.snapshot.sense::<Stole>()?;
        let me = ctx.snapshot.me.pos;
        if let Some(ways) = ctx.snapshot.sense::<Ways>() {
            let nearest = ways.0.iter().copied().min_by_key(|p| geometry::chebyshev(me, *p));
            if let Some(way) = nearest {
                if way == me {
                    return Some(Decision::GoThrough);
                }
                if let Some(step) = ctx.step_toward(&[way]) {
                    return Some(Decision::Step(step));
                }
            }
        }
        let foes: Vec<Point> = ctx.snapshot.enemies.iter().map(|e| e.pos).collect();
        ctx.step_away_from(&foes).map(Decision::Step)
    }
}

// ANCHOR: steal
/// Takes a random thing its target carries but does not wear, then marks
/// the taker [`Stole`] so its brain flees with it: a monkey's `stealing`.
/// Takes no arguments.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Steal;

impl Effect for Steal {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let (thief, targets, landing) = (landing.user, landing.targets.clone(), landing.clone());
        world.commands.queue(move |world: &mut World| {
            let took = targets.iter().any(|victim| steal(world, thief, *victim));
            if !took {
                crate::effects::showed_nothing(world, &landing);
            }
        });
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        "takes something it is not wearing, then runs".to_string()
    }
}

impl FromArgs for Steal {
    const KIND: &'static str = "Steal";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        crate::effects::moving::no_args(args, Self::KIND).map(|_| Self)
    }
}
// ANCHOR_END: steal

/// `thief` takes a random thing `victim` carries but does not wear, as
/// [`Steal`] says, marks `thief` [`Stole`] and says whether it took
/// anything: nothing from an empty pack, nor from what `victim` wears.
pub fn steal(world: &mut World, thief: Entity, victim: Entity) -> bool {
    if thief == victim {
        return false;
    }
    let Some(carried) = world.get::<Inventory>(victim).map(|bag| bag.items.clone()) else { return false };
    let worn = world.get::<Equipped>(victim);
    let free: Vec<Entity> = carried.into_iter().filter(|item| !worn.is_some_and(|w| w.0.contains(*item))).collect();
    if free.is_empty() {
        return false;
    }
    let pick = world.resource_mut::<EffectRng>().random_range(0..free.len());
    let item = free[pick];
    if let Some(mut bag) = world.get_mut::<Inventory>(victim) {
        bag.items.retain(|i| *i != item);
    }
    match world.get_mut::<Inventory>(thief) {
        Some(mut bag) => bag.items.push(item),
        None => {
            world.entity_mut(thief).insert(Inventory { items: vec![item] });
        }
    }
    world.entity_mut(thief).insert(Stole);
    if let Some(at) = world.get::<Position>(victim).map(|p| p.0)
        && seen_by_player(world, at)
    {
        let name = |e: Entity, world: &World| world.get::<Name>(e).map_or_else(String::new, |n| n.as_str().to_string());
        let (thief_name, victim_name, item_name) = (name(thief, world), name(victim, world), name(item, world));
        let line = format!("{} snatches {}'s {} and bolts!", crate::effects::capital(&thief_name), victim_name, item_name);
        world.write_message(Tell::new(line, Tones::BAD));
        // Learned here, where a thing is taken, rather than off the blow
        // it rode: a blow at an empty pack takes nothing and shows nothing.
        if let Some(kind) = world.get::<Kind>(thief).map(|k| k.0) {
            let name = world.resource::<Bestiary>().defs.get(kind).name.clone();
            world.resource_mut::<MonsterLore>().show(&name, "stealing");
        }
    }
    true
}

/// Steps a random way one time in [`Flit::chance_pct`] out of a hundred,
/// letting every other tactic choose the rest of the time: a vampire
/// bat's `flitting`, erratic rather than a beeline to its prey.
///
/// The same roll-then-step shape as [`atglyph::rules::ai::tactics::Wander`],
/// without its "wait, there is nothing better to do": this lets the next
/// tactic, `Hunt` wherever a brain puts it, decide both when the roll
/// fails and when every step is blocked.
#[derive(Debug, Clone, Copy)]
pub struct Flit {
    /// Chance per turn of a random step rather than the next tactic's own.
    pub chance_pct: u32,
}

impl Tactic<Entity> for Flit {
    fn name(&self) -> &'static str {
        "flit"
    }

    fn evaluate(&self, ctx: &mut TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
        if !ctx.rng.random_ratio(self.chance_pct.min(100), 100) {
            return None;
        }
        let me = ctx.snapshot.me.pos;
        let back = ctx.snapshot.came_from;
        let start = ctx.rng.random_range(0..8);
        let mut only_back = None;
        for i in 0..8 {
            let step: Point = me + Direction::from_index((start + i) % 8).offset();
            if !(ctx.can_step)(step) {
                continue;
            }
            if Some(step) == back {
                only_back = Some(step);
                continue;
            }
            return Some(Decision::Step(step));
        }
        only_back.map(Decision::Step)
    }
}

/// A loop of cells a monster walks, the lived-in dungeon's: not a trait's trick,
/// since nothing in the bestiary names it, but a floor's population may
/// put it on anyone it stands, the same way it decides who sleeps.
///
/// The engine keeps no list of a built floor's rooms once it is built, so
/// `population::place_patrol` draws its own waypoints off the floor's open
/// ground rather than true room centres; the effect for a watcher is the
/// same, a mind that goes room to room rather than standing or wandering.
#[derive(Component, Debug, Clone)]
pub struct PatrolRoute {
    /// At least two cells, each visited in turn before the loop starts
    /// over.
    pub waypoints: Vec<Point>,
    /// Which of `waypoints` is next.
    pub at: usize,
}

/// Where a patrolling mind is walking to next, pushed by [`sense_patrols`]
/// for [`Patrol`] to read: the same shape as the engine's own `Posted`,
/// one point rather than a post that never moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Patrolling(pub Point);

/// Pushed by [`sense_patrols`] beside [`Patrolling`] for a mind whose route
/// is one cell: held there rather than walking a loop, a captive in its
/// shackles or a beast over its meal. What [`Unanchored`] reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anchored;

/// The tactic it wraps, except while the mind holding the turn is
/// [`Anchored`]: then it leaves the turn to the tactics after it, among
/// which [`Patrol`] keeps the anchor.
///
/// For the tactics that move a mind for reasons of its kind, a packing
/// jackal's closing on its own and a bat's random flit, which rank above
/// `Patrol` in every brain and so took a shackled bat off its chain and a
/// feeding jackal off its meal. Hunting is left alone, so a feeder that
/// sees the adventurer still goes for them, and walks back once it has
/// lost them.
#[derive(Debug, Clone, Copy)]
pub struct Unanchored<T>(pub T);

impl<T: Tactic<Entity>> Tactic<Entity> for Unanchored<T> {
    fn name(&self) -> &'static str {
        self.0.name()
    }

    fn evaluate(&self, ctx: &mut TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
        if ctx.snapshot.sense::<Anchored>().is_some() {
            return None;
        }
        self.0.evaluate(ctx)
    }

    fn keeps_with(&self) -> Option<Station> {
        self.0.keeps_with()
    }
}

/// Advances the turn holder's own [`PatrolRoute`] past whichever waypoint
/// it just reached, and senses the one it is walking to now, and whether
/// it is [`Anchored`] there.
///
/// In `PerceiveSet::Annotate`, beside the engine's own `sense_posts` and
/// `sense_leaders`: nothing for an actor with no route, so one brain
/// serves a floor's own patrol and everybody else alike, the same reason
/// `KeepPost` passes over a monster with no post.
pub fn sense_patrols(mut thinking: ResMut<Thinking>, mut routes: Query<(&Position, &mut PatrolRoute)>) {
    let Some(actor) = thinking.actor() else { return };
    let Ok((at, mut route)) = routes.get_mut(actor) else { return };
    if route.waypoints.is_empty() {
        return;
    }
    if at.0 == route.waypoints[route.at] {
        route.at = (route.at + 1) % route.waypoints.len();
    }
    let target = route.waypoints[route.at];
    let anchored = route.waypoints.len() == 1;
    if let Some(snapshot) = thinking.snapshot_mut() {
        snapshot.add_sense(Patrolling(target));
        if anchored {
            snapshot.add_sense(Anchored);
        }
    }
}

/// Walks toward whatever [`sense_patrols`] says a patrolling mind is
/// walking to, waiting the one pass it stands there before the next
/// waypoint is sensed. Leaves the turn to the next tactic for anyone with
/// no [`Patrolling`] sense, the same shape as the engine's own `KeepPost`,
/// so one brain serves a kind whether or not a floor put a route on it.
#[derive(Debug, Clone, Copy, Default)]
pub struct Patrol;

impl Tactic<Entity> for Patrol {
    fn name(&self) -> &'static str {
        "patrol"
    }

    fn evaluate(&self, ctx: &mut TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
        let target = ctx.snapshot.sense::<Patrolling>()?.0;
        let me = ctx.snapshot.me.pos;
        if me == target {
            return Some(Decision::Wait);
        }
        Some(ctx.step_toward(&[target]).map_or(Decision::Wait, Decision::Step))
    }
}

#[cfg(test)]
mod tests {
    use atglyph::primitives::Rect;
    use rand::SeedableRng;

    use super::*;

    /// `Flit`'s roll decides a random step about one turn in three, and
    /// leaves every other turn to `None`, for the tactic after it to
    /// decide, over a span of seeds.
    #[test]
    fn flit_steps_randomly_about_one_turn_in_three_over_a_span_of_seeds() {
        let me = ActorView { id: Entity::PLACEHOLDER, pos: Point::ZERO, health: None, faction: None };
        let snapshot = Snapshot::alone(me);
        let mut rolled = 0;
        for seed in 0..200u64 {
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            let ctx = &mut TacticCtx {
                snapshot: &snapshot,
                fields: &mut NoFields,
                can_step: &|_| true,
                blocks_shot: &|_| false,
                blocks_burst: &|_| false,
                bounds: Rect::new(-20, -20, 40, 40),
                rng: &mut rng,
            };
            if (Flit { chance_pct: 33 }).evaluate(ctx).is_some() {
                rolled += 1;
            }
        }
        assert!((40..100).contains(&rolled), "about a third of two hundred rolls stepped randomly: {rolled}");
    }

    /// A floor's patrol: a `PatrolRoute` of three waypoints is walked
    /// in order and the loop starts over, every waypoint visited at least
    /// twice over enough passes, by the universal brain's own `Patrol`
    /// slot, with no trait and no marker beyond the route itself.
    #[test]
    fn a_monster_given_a_patrol_route_walks_every_waypoint_in_order_and_loops_back_to_the_first() {
        use crate::testing::{arena, at, monster, pass_turns, stand_on};
        let (mut app, me, here) = arena(80);
        // Far across the arena's own open ground, outside the rat's sight,
        // so nothing but its own route ever has a say in where it walks.
        stand_on(&mut app, me, here.offset(-5, -5));
        let waypoints = vec![here.offset(5, 5), here.offset(5, 2), here.offset(2, 5)];
        let walker = monster(&mut app, "rat", waypoints[0]);
        app.world_mut().entity_mut(walker).insert(PatrolRoute { waypoints: waypoints.clone(), at: 0 });
        let mut visits = [0u32; 3];
        let mut trace = Vec::new();
        for _ in 0..60 {
            pass_turns(&mut app, 1);
            let now = at(&app, walker);
            trace.push(now);
            if let Some(i) = waypoints.iter().position(|w| *w == now) {
                visits[i] += 1;
            }
        }
        assert!(visits.iter().all(|v| *v >= 2), "every waypoint visited at least twice, looping: {visits:?}, waypoints {waypoints:?}, trace {trace:?}");
    }

    /// An anchor holds what its kind would otherwise drift by: a shackled
    /// vampire bat never flits off its chain, and a jackal anchored over
    /// its meal never closes on a packmate standing four cells off, over
    /// forty turns of each.
    #[test]
    fn an_anchored_bat_never_flits_and_an_anchored_jackal_never_drifts_to_its_pack() {
        use crate::testing::{arena, at, monster, pass_turns, stand_on, still};
        let (mut app, me, here) = arena(81);
        stand_on(&mut app, me, here.offset(-5, -5));
        let registries = app.world().resource::<Registries>().clone();
        let chain = here.offset(5, -3);
        let bat = monster(&mut app, "vampire bat", chain);
        let shackles = crate::captives::shackled(&registries, app.world().resource::<Verbs>(), chain);
        app.world_mut().entity_mut(bat).insert(shackles);
        let meal = here.offset(5, 5);
        let jackal = monster(&mut app, "jackal", meal);
        app.world_mut().entity_mut(jackal).insert(PatrolRoute { waypoints: vec![meal], at: 0 });
        still(&mut app, "jackal", here.offset(1, 5));
        for turn in 0..40 {
            pass_turns(&mut app, 1);
            assert_eq!((at(&app, bat), at(&app, jackal)), (chain, meal), "turn {turn}: both held where they are anchored");
        }
    }
}
