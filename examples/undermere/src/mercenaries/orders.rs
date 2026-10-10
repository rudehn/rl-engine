//! The three orders a hired party is under, the brain each one thinks
//! with, and the one key that cycles them.

use std::sync::Arc;

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_rules::ai::brain::Tactic;
use rl_engine::rl_rules::ai::tactics::{FleeWhenHurt, Hunt, Keep, KeepPost, Led, MeleeAdjacent, Scavenge, Station, ThrowAtRange, UseCarried};
use serde::{Deserialize, Serialize};

use super::{KitSeen, MERCENARY, Mercenary, PutOnBetter};
use crate::bestiary::{Bestiary, MonsterDef, SCAVENGE_REACH};
use crate::input::Binds;

/// How far from the adventurer a mercenary lets itself be before it
/// closes, and the furthest a follower's hunt or anyone's fetch takes it:
/// three cells, near enough to be a step from a fight at the adventurer's
/// side and far enough not to stand in their way.
pub const LEASH: i32 = 3;

/// What the look panel says of a hired mercenary in place of its kind's
/// `a sword for hire`.
pub const IN_PAY: &str = "in your pay";

/// What the party has been told to do, cycled by the order key.
///
/// A closed set, since it is the game's own three and not content: a kind
/// of mercenary with an order of its own would be a brain of its own.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Order {
    /// Keep within [`LEASH`] of the adventurer, fight what comes within
    /// it, and fetch gear; what it is hired on.
    #[default]
    Follow,
    /// Stand where it was ordered and fight only what reaches it.
    Hold,
    /// Chase whatever it sees, and come back once nothing is left.
    FightFreely,
}

impl Order {
    /// The order after this one: follow, hold, fight freely, and round.
    pub fn next(self) -> Self {
        match self {
            Order::Follow => Order::Hold,
            Order::Hold => Order::FightFreely,
            Order::FightFreely => Order::Follow,
        }
    }

    /// The order as it is given, `You order the party to hold.`
    pub fn word(self) -> &'static str {
        match self {
            Order::Follow => "follow",
            Order::Hold => "hold",
            Order::FightFreely => "fight freely",
        }
    }

    /// The order as a mercenary is seen keeping it, on the party panel
    /// and the look panel.
    pub fn doing(self) -> &'static str {
        match self {
            Order::Follow => "following",
            Order::Hold => "holding",
            Order::FightFreely => "fighting freely",
        }
    }
}

/// The tactic it wraps, except a step that would leave the mercenary more
/// than [`LEASH`] from the one it follows: then it leaves the turn to the
/// tactics after it.
///
/// What keeps a follower's hunt and anyone's fetch at the adventurer's
/// side. `Keep::leader` above them only closes a gap already open, so
/// without this a hunt would step out to four, be called back to three,
/// and step out again for as long as the foe stood there; with it, a foe
/// beyond the leash is left alone. With the leader gone from the map it
/// lets every step through, since there is nobody to keep within reach
/// of.
#[derive(Debug, Clone, Copy)]
pub struct Leashed<T>(pub T);

impl<T: Tactic<Entity>> Tactic<Entity> for Leashed<T> {
    fn name(&self) -> &'static str {
        self.0.name()
    }

    fn evaluate(&self, ctx: &mut TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
        let decision = self.0.evaluate(ctx)?;
        if let (Decision::Step(to), Some(led)) = (&decision, ctx.snapshot.sense::<Led>())
            && geometry::chebyshev(*to, led.at) > LEASH
        {
            return None;
        }
        Some(decision)
    }

    fn keeps_with(&self) -> Option<Station> {
        self.0.keeps_with()
    }
}

/// Closing on the one it follows while it cannot see them, wherever the
/// gap stands.
///
/// What keeps a follower from losing the adventurer round a corner.
/// `Keep::leader` measures the gap as the crow flies, so a follower three
/// cells from a corner the adventurer has just turned stands still while
/// they walk on, the gap holding at three across the wall and the walk
/// between them growing a step a turn; out of sight is where those two
/// part, so out of sight it closes.
#[derive(Debug, Clone, Copy)]
pub struct Unseen(pub Keep);

impl Tactic<Entity> for Unseen {
    fn name(&self) -> &'static str {
        Tactic::<Entity>::name(&self.0)
    }

    fn evaluate(&self, ctx: &mut TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
        let led = ctx.snapshot.sense::<Led>()?;
        if led.in_sight {
            return None;
        }
        self.0.evaluate(ctx)
    }

    fn keeps_with(&self) -> Option<Station> {
        Tactic::<Entity>::keeps_with(&self.0)
    }
}

/// The three brains a mercenary thinks with, one for each [`Order`],
/// built once at load from the one kind, beside its ally mind.
///
/// Each begins the same way: run when hurt, use what it carries, strike
/// what is beside it, throw at what is not. Where they differ is what a
/// turn with none of that to do is spent on:
///
/// - follow keeps within [`LEASH`] of the adventurer, then hunts and
///   fetches only within it, so it fights what comes near and never more
///   than three cells from them;
/// - hold keeps its [`Post`], and neither hunts nor fetches;
/// - fight freely hunts whatever it sees first, then comes back within
///   the leash and fetches there.
#[derive(Resource, Clone)]
pub struct MercenaryBrains {
    follow: Arc<Brain<Entity>>,
    hold: Arc<Brain<Entity>>,
    free: Arc<Brain<Entity>>,
}

impl MercenaryBrains {
    /// The three brains for `kind`, the mercenary's row of the bestiary:
    /// it runs at the kind's own threshold.
    pub fn new(kind: &MonsterDef) -> Self {
        let first = || {
            let mut brain = Brain::new();
            if kind.flee_at() > 0 {
                brain = brain.then(FleeWhenHurt { at_pct: kind.flee_at() });
            }
            brain.then(UseCarried::default()).then(PutOnBetter).then(MeleeAdjacent).then(ThrowAtRange::default())
        };
        let keep = Keep::leader(LEASH, 1);
        let unseen = Unseen(Keep::leader(1, 1));
        let fetch = Leashed(Scavenge { reach: SCAVENGE_REACH });
        Self {
            follow: Arc::new(first().then(keep).then(unseen).then(Leashed(Hunt)).then(fetch)),
            hold: Arc::new(first().then(KeepPost)),
            free: Arc::new(first().then(Hunt).then(keep).then(unseen).then(fetch)),
        }
    }

    /// The mind a mercenary under `order` thinks with.
    pub fn mind(&self, order: Order) -> Mind {
        Mind(match order {
            Order::Follow => self.follow.clone(),
            Order::Hold => self.hold.clone(),
            Order::FightFreely => self.free.clone(),
        })
    }
}

/// Puts `mercenary` under `order`: its brain for it, and on hold a
/// [`Post`] at the cell it stands on, which any other order takes off.
/// The order itself is the caller's to have written on its [`Mercenary`].
///
/// Also takes [`Wits::TRAVELS`] off it, under every order: the party goes
/// through a way only after the adventurer, by
/// [`follow_through`](super::follow_through), and never by the engine's
/// own travel, which would take one fleeing alone down a stair. Here
/// rather than in the kind's row alone, since every way a mercenary is
/// given a brain comes through this, and a trait added to the kind later
/// would otherwise hand it the wit unnoticed.
pub fn obey(world: &mut World, mercenary: Entity, order: Order) {
    let mind = world.resource::<MercenaryBrains>().mind(order);
    let at = world.get::<Position>(mercenary).map(|p| p.0);
    let mut e = world.entity_mut(mercenary);
    e.insert(mind);
    if let Some(mut wits) = e.get_mut::<Intelligence>() {
        wits.0 = wits.0.without(Wits::TRAVELS);
    }
    match (order, at) {
        (Order::Hold, Some(at)) => {
            e.insert(Post(at));
        }
        _ => {
            e.remove::<Post>();
        }
    }
}

/// What the order key reads before it orders anyone.
#[derive(bevy::ecs::system::SystemParam)]
pub struct OrderKey<'w, 's> {
    keys: ControlInput<'w>,
    binds: Res<'w, Binds>,
    modals: Res<'w, Modals>,
    turns: Res<'w, Turns>,
    log: ResMut<'w, MessageLog>,
    player: Query<'w, 's, Entity, (With<Player>, With<MyTurn>)>,
}

/// Cycles the order of every mercenary following the adventurer at once,
/// on the order key, and says the new one in a line of the log. Costs no
/// turn: an order is a word, and a word is not worth a blow taken.
///
/// The new order is the one after the order of whoever joined first, so
/// a party whose second was hired on follow while the first held comes
/// together on the next press. With nobody hired the key says so and
/// does nothing, as a key with nothing to do does.
///
/// In `EngineSet::Input` beside the adventurer's other keys, while they
/// hold the turn and no screen is up, so nobody is deciding while an
/// order changes.
pub fn order_party(mut key: OrderKey, mut commands: Commands, party: Query<(Entity, &Mercenary, &Leader, Option<&Joined>)>) {
    if key.modals.any_open() || !key.keys.just_pressed(key.binds.order) {
        return;
    }
    let Ok(me) = key.player.single() else { return };
    let now = key.turns.turn_number();
    let mut ours: Vec<(Option<Joined>, Entity, Order)> = party.iter().filter(|(.., l, _)| l.0 == me).map(|(e, m, _, j)| (j.copied(), e, m.order)).collect();
    ours.sort_by_key(|(joined, e, _)| (joined.is_none(), *joined, *e));
    let Some(&(_, _, first)) = ours.first() else {
        key.log.muted("You have nobody to order.", now);
        return;
    };
    let order = first.next();
    for (_, mercenary, _) in ours {
        commands.queue(move |world: &mut World| {
            if let Some(mut hired) = world.get_mut::<Mercenary>(mercenary) {
                hired.order = order;
                obey(world, mercenary, order);
            }
        });
    }
    key.log.info(format!("You order the party to {}.", order.word()), now);
}

/// Says each mercenary's order, `holding`, on the look panel, on the
/// rail and on its row of the party panel; and on the look panel that it
/// is in the adventurer's pay, and what it holds as they know it.
///
/// On the look panel the pay takes the place of its kind's own words, `a
/// sword for hire`, which it no longer is, and the order the place of the
/// fight's forecast, since the odds of a fight with somebody the
/// adventurer is paying are nothing a player is asking. What it holds is
/// said as a candidate's kit is, by [`KitSeen`]: its own kit at the level
/// it told when it was hired, anything handed to it by what is known of
/// it, and a potion of a kind not yet known by its face. On the rail the
/// order takes the place of what the mercenary knows of the adventurer
/// and what it is doing this turn, `unaware` and `follow`, which say less
/// of a hired hand than its order.
///
/// In `ViewSet::Annotate`, after the kind's own words and the game's own
/// forecast, so both are there to take off and nothing puts a forecast
/// back after it.
pub fn show_orders(
    look: Option<ResMut<InspectView>>,
    rail: Option<ResMut<NearbyView>>,
    party: Option<ResMut<PartyView>>,
    hired: Query<&Mercenary>,
    kits: KitSeen,
    bestiary: Res<Bestiary>,
    mut facets: ResMut<Facets>,
) {
    if let Some(mut view) = look
        && let Some((who, order)) = view.subject.as_ref().and_then(|s| hired.get(s.entity).ok().map(|m| (s.entity, m.order)))
    {
        let (forecast, kind) = (facets.declare("forecast"), facets.declare("trait"));
        let def = bestiary.defs.get(bestiary.defs.expect(MERCENARY));
        let for_hire: Vec<&str> = def.folded.says.iter().filter(|(name, ..)| name == MERCENARY).map(|(_, says, _)| says.as_str()).collect();
        view.facets.retain(|f| f.key != forecast && !(f.key == kind && for_hire.contains(&f.text.as_str())));
        (view.duel, view.odds) = (None, None);
        view.facets.push(facets.facet("pay", IN_PAY));
        view.facets.push(facets.facet("order", order.doing()));
        for (how, said) in kits.of(who) {
            view.facets.push(facets.facet("kit", format!("{} {said}", how.verb())));
        }
    }
    if let Some(mut view) = rail {
        for row in view.actors.iter_mut() {
            if let Ok(hired) = hired.get(row.entity) {
                (row.doing, row.alert) = (None, None);
                row.facets.push(facets.facet("order", hired.order.doing()));
            }
        }
    }
    if let Some(mut view) = party {
        for row in view.rows_mut() {
            if let Ok(hired) = hired.get(row.entity) {
                row.facets.push(facets.facet("order", hired.order.doing()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, VecDeque};
    use std::sync::Arc;

    use rl_engine::rl_bevy::testing::{KeyScriptPlugin, press};

    use super::*;
    use crate::mercenaries::fixtures::hired;
    use crate::testing::{arena, arena_with, at, clear_floor, here, pass_turns, said, still};

    /// Whether `who` thinks with the brain for `order`.
    fn thinks(app: &App, who: Entity, order: Order) -> bool {
        let mind = app.world().get::<Mind>(who).expect("a mind");
        Arc::ptr_eq(&mind.0, &app.world().resource::<MercenaryBrains>().mind(order).0)
    }

    /// Puts `mercenary` under `order`, as the order key does.
    fn order(app: &mut App, mercenary: Entity, order: Order) {
        app.world_mut().get_mut::<Mercenary>(mercenary).expect("a mercenary").order = order;
        obey(app.world_mut(), mercenary, order);
    }

    /// How far apart `a` and `b` stand.
    fn gap(app: &App, a: Entity, b: Entity) -> i32 {
        geometry::chebyshev(at(app, a), at(app, b))
    }

    /// The cells from `from` to the cell furthest from it, at most
    /// `steps` of them, a step at a time with no corner cut and through
    /// any door, with whatever stood on it taken away: the long way
    /// across the floor.
    fn the_long_way(app: &mut App, from: Point, steps: usize) -> Vec<Point> {
        let (map, floor) = (app.world().resource::<WorldMap>(), here(app));
        let mut came: BTreeMap<Point, Point> = BTreeMap::from([(from, from)]);
        let (mut queue, mut last) = (VecDeque::from([from]), from);
        while let Some(p) = queue.pop_front() {
            last = p;
            for d in [Direction::North, Direction::East, Direction::South, Direction::West] {
                let next = p + d.offset();
                let passable = map.is_walkable_on(floor, next) || map.opens(next).is_some();
                if passable && !came.contains_key(&next) {
                    came.insert(next, p);
                    queue.push_back(next);
                }
            }
        }
        let mut way = vec![last];
        while let Some(&back) = came.get(way.last().expect("a cell")).filter(|b| **b != from) {
            way.push(back);
        }
        way.reverse();
        way.truncate(steps);
        let world = app.world_mut();
        let standing: Vec<Entity> = world
            .query_filtered::<(Entity, &Position), (With<Blocks>, Without<Actor>)>()
            .iter(world)
            .filter(|(_, p)| way.contains(&p.0))
            .map(|(e, _)| e)
            .collect();
        for thing in standing {
            world.despawn(thing);
        }
        way
    }

    /// Walks `me` onto each cell of `way` in turn, a step a turn, a door
    /// in the way opened first; after every turn, `each` is asked of the
    /// app.
    fn walk(app: &mut App, me: Entity, way: &[Point], mut each: impl FnMut(&App)) {
        for cell in way {
            for _ in 0..3 {
                let Some(d) = Direction::between(at(app, me), *cell) else { break };
                app.world_mut().write_message(Intent::new(me, Bump(d)));
                app.update();
                each(app);
                if at(app, me) == *cell {
                    break;
                }
            }
            assert_eq!(at(app, me), *cell, "the adventurer walks the way, {way:?}");
        }
    }

    /// A hired follower keeps within three of the adventurer across a
    /// floor with nothing on it to fight, over a span of seeds: every
    /// time they stop on the long way across, through whatever rooms,
    /// corridors, corners and doors it takes, it is within three of
    /// them; and while they walk it is never more than a step behind
    /// that, since a diagonal stride takes longer than a straight one and
    /// it cannot gain on somebody walking at its own pace.
    #[test]
    fn a_follower_keeps_within_three_of_the_adventurer_across_a_floor_with_nothing_to_fight_over_a_span_of_seeds() {
        for seed in 0..6 {
            let (mut app, me, here) = arena(seed);
            clear_floor(&mut app);
            let brenna = hired(&mut app, me, here.offset(-1, 0), "Brenna");
            let way = the_long_way(&mut app, here, 60);
            assert!(way.len() >= 30, "seed {seed}: a long way to walk, {} cells", way.len());
            for stretch in way.chunks(6) {
                let mut widest = 0;
                walk(&mut app, me, stretch, |app| widest = widest.max(gap(app, me, brenna)));
                assert!(widest <= LEASH + 1, "seed {seed}: it fell {widest} behind on the way to {:?}", stretch.last());
                pass_turns(&mut app, 2);
                assert!(gap(&app, me, brenna) <= LEASH, "seed {seed}: {} away where they stopped, {:?}", gap(&app, me, brenna), at(&app, me));
            }
        }
    }

    /// On hold a mercenary keeps the cell it was ordered on while a foe
    /// stands in sight out of its reach, over a span of seeds; a follower
    /// in its place goes for the same foe, which is what holding stops.
    #[test]
    fn on_hold_it_keeps_its_post_while_a_foe_is_in_sight_beyond_reach_over_a_span_of_seeds() {
        for seed in 0..4 {
            for held in [true, false] {
                let (mut app, me, here) = arena(seed);
                let brenna = hired(&mut app, me, here.offset(-2, 0), "Brenna");
                let post = at(&app, brenna);
                if held {
                    order(&mut app, brenna, Order::Hold);
                }
                let goblin = still(&mut app, "goblin", here.offset(2, 0));
                pass_turns(&mut app, 1);
                let sees = app.world().get::<Viewshed>(brenna).is_some_and(|v| v.can_see(at(&app, goblin)));
                assert!(sees, "seed {seed}: the goblin is in its sight");
                let mut moved = false;
                for _ in 0..6 {
                    pass_turns(&mut app, 1);
                    moved |= at(&app, brenna) != post;
                }
                if held {
                    assert!(!moved, "seed {seed}: held at its post");
                    assert_eq!(app.world().get::<Post>(brenna), Some(&Post(post)), "seed {seed}: posted where it was ordered");
                } else {
                    assert!(moved, "seed {seed}: a follower goes for it");
                }
            }
        }
    }

    /// A follower leaves a foe beyond its leash alone, never more than
    /// three from the adventurer; fighting freely it chases the same foe
    /// past the leash, strikes it down, and comes back within three once
    /// nothing is left, over a span of seeds.
    #[test]
    fn fighting_freely_it_chases_a_foe_in_sight_and_comes_back_when_none_is_left_where_a_follower_lets_it_be_over_a_span_of_seeds() {
        for seed in 0..4 {
            for free in [true, false] {
                let (mut app, me, here) = arena(seed);
                let brenna = hired(&mut app, me, here.offset(-1, 0), "Brenna");
                if free {
                    order(&mut app, brenna, Order::FightFreely);
                }
                let goblin = still(&mut app, "goblin", here.offset(5, 0));
                app.world_mut().entity_mut(goblin).insert(Health::full(1));
                let (mut widest, mut down_at) = (0, None);
                for turn in 0..30 {
                    pass_turns(&mut app, 1);
                    widest = widest.max(gap(&app, me, brenna));
                    if app.world().get::<Health>(goblin).is_none_or(|h| h.current <= 0) {
                        down_at.get_or_insert(turn);
                    }
                }
                if free {
                    assert!(widest > LEASH, "seed {seed}: it chased past the leash, {widest}");
                    assert!(down_at.is_some_and(|t| t < 20), "seed {seed}: and struck the goblin down, at {down_at:?}");
                    assert!(gap(&app, me, brenna) <= LEASH, "seed {seed}: and came back, {} away", gap(&app, me, brenna));
                } else {
                    assert!(widest <= LEASH && down_at.is_none(), "seed {seed}: a follower stays within {LEASH}, at {widest}, and lets it be");
                }
            }
        }
    }

    /// `o` cycles every mercenary's order at once, follow to hold to fight
    /// freely and round, each with one line of the log, a post on hold
    /// and none otherwise, and the brain for the order; and it never
    /// spends a turn.
    #[test]
    fn o_cycles_every_mercenarys_order_at_once_and_costs_no_turn() {
        let (mut app, me, here) = arena_with(3, KeyScriptPlugin);
        let party = [hired(&mut app, me, here.offset(-1, 0), "Brenna"), hired(&mut app, me, here.offset(1, 0), "Tolvar")];
        for want in [Order::Hold, Order::FightFreely, Order::Follow] {
            let before = app.world().resource::<Turns>().now();
            press(&mut app, KeyCode::KeyO);
            assert_eq!(app.world().resource::<Turns>().now(), before, "{want:?}: no turn spent");
            assert_eq!(said(&app).last(), Some(&format!("You order the party to {}.", want.word())));
            for m in party {
                assert_eq!(app.world().get::<Mercenary>(m).map(|h| h.order), Some(want));
                assert!(thinks(&app, m, want), "{want:?}: the brain for it");
                let post = app.world().get::<Post>(m).copied();
                assert_eq!(post, (want == Order::Hold).then(|| Post(at(&app, m))), "{want:?}: posted only on hold");
            }
        }
    }

    /// With nobody hired, `o` says so and costs nothing; and the controls
    /// screen lists it under "Act".
    #[test]
    fn o_with_nobody_hired_says_so_spends_no_turn_and_is_listed_under_act() {
        let (mut app, _, _) = arena_with(3, KeyScriptPlugin);
        let listed = app.world().resource::<Controls>().find("Act", "order the party: follow, hold, fight freely");
        assert_eq!(listed, Some(app.world().resource::<Binds>().order), "on the controls screen");
        let before = app.world().resource::<Turns>().now();
        press(&mut app, KeyCode::KeyO);
        assert_eq!(said(&app).last().map(String::as_str), Some("You have nobody to order."));
        assert_eq!(app.world().resource::<Turns>().now(), before);
    }

    /// The log names a mercenary as a proper noun, with no article before
    /// it: `Brenna hits the goblin for 2.`
    #[test]
    fn the_log_names_a_mercenary_by_its_name_with_no_article() {
        let (mut app, me, here) = arena(4);
        hired(&mut app, me, here.offset(-1, 0), "Brenna");
        still(&mut app, "goblin", here.offset(-2, 0));
        pass_turns(&mut app, 10);
        let lines = said(&app);
        assert!(lines.iter().any(|l| l.starts_with("Brenna hits the goblin")), "{lines:?}");
        assert!(!lines.iter().any(|l| l.to_lowercase().contains("the brenna")), "{lines:?}");
    }

    /// The order is said on the look panel beside the mercenary's pay and
    /// on the mercenary's row of the party panel, and changes with it.
    #[test]
    fn the_look_panel_and_the_party_panel_say_each_mercenarys_order() {
        let views = (rl_engine::rl_ui::InspectViewPlugin, rl_engine::rl_ui::PartyViewPlugin);
        let (mut app, me, here) = arena_with(2, views);
        app.add_systems(Update, (crate::bestiary_lore::annotate_inspect, show_orders).chain().in_set(ViewSet::Annotate));
        let brenna = hired(&mut app, me, here.offset(-1, 0), "Brenna");
        let modal = rl_engine::rl_ui::view::inspect::inspect_modal(app.world().resource::<Modals>());
        app.world_mut().resource_mut::<Modals>().open(modal);
        app.world_mut().resource_mut::<InspectView>().cursor = at(&app, brenna);
        for (want, word) in [(Order::Follow, "following"), (Order::Hold, "holding")] {
            order(&mut app, brenna, want);
            app.update();
            let look = app.world().resource::<InspectView>();
            let facets: Vec<&str> = look.facets.iter().map(|f| f.text.as_str()).collect();
            assert_eq!(facets.last(), Some(&word), "the look: {facets:?}");
            let party = app.world().resource::<PartyView>();
            let row: Vec<&str> = party.rows.iter().filter(|r| r.entity == brenna).flat_map(|r| r.facets.iter().map(|f| f.text.as_str())).collect();
            assert_eq!(row, [word], "the party row");
        }
    }
}
