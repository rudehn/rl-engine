//! The three orders a hired party is under, the brain each one thinks
//! with, and the one key that cycles them.

use std::sync::Arc;

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_rules::ai::Standard;
use rl_engine::rl_rules::ai::tactics::{Hunt, Keep, Leashed, OutOfSight, Scavenge, UseCarried};
use serde::{Deserialize, Serialize};

use super::{KIT_FACET, KitSeen, MERCENARY, Mercenary, PutOnBetter};
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

/// How often a mercenary throws on a turn a throw is there to be made:
/// always, since what a hire carries to throw is carried to be spent on
/// the adventurer's enemies, where a monster's half the time keeps it
/// closing in.
const THROW_CHANCE_PCT: u32 = 100;

/// The chain a mercenary decides with under every order: the engine's
/// standard chain at Undermere's numbers, running at the kind's own
/// threshold, striking and throwing, with nothing to close it. Each order
/// closes it its own way, by [`HOLD`], [`FIGHT_FREELY`], or on follow by
/// the engine's hunt held to the [`LEASH`].
const HIRED: Standard = Standard { flee: true, melee: true, throw_chance: Some(THROW_CHANCE_PCT), ..Standard::NONE };

/// The chain on hold: [`HIRED`]'s, closed by walking back to its post.
const HOLD: Standard = Standard { keep_post: true, ..HIRED };

/// The chain on fight freely: [`HIRED`]'s, closed by the engine's hunt
/// of whatever it sees, wherever that takes it.
const FIGHT_FREELY: Standard = Standard { hunt: true, ..HIRED };

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
    /// The three brains for `kind`, the mercenary's row of the bestiary,
    /// each the standard chain for what the kind can do.
    ///
    /// Every order puts the same two of the game's own in: what it carries
    /// used before any blow, mending first, and gear better than it wears
    /// put on once nothing is in reach. Follow keeps beside the adventurer
    /// before the hunt and hunts and fetches leashed in its place; fight
    /// freely keeps beside them and fetches only once the engine's hunt has
    /// nothing left to chase, after it, where a game's own route goes.
    pub fn new(kind: &MonsterDef) -> Self {
        let can = kind.parts.repertoire();
        let chain = |chain: Standard| chain.brain(can).before_attacks(UseCarried::default()).before_hunt(PutOnBetter);
        let keep = Keep::leader(LEASH, 1);
        let behind = OutOfSight(Keep::leader(1, 1));
        let fetch = Leashed { inner: Scavenge { reach: SCAVENGE_REACH }, reach: LEASH };
        Self {
            follow: Arc::new(
                chain(HIRED).before_hunt(keep).before_hunt(behind).instead_of_hunt(Leashed { inner: Hunt, reach: LEASH }).before_wander(fetch).build(),
            ),
            hold: Arc::new(chain(HOLD).build()),
            free: Arc::new(chain(FIGHT_FREELY).before_wander(keep).before_wander(behind).before_wander(fetch).build()),
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
/// Also takes [`Wits::FLEES_THROUGH`] off it, under every order: the party
/// goes through a way only after the adventurer, by
/// [`follow_through`](super::follow_through), and never of its own accord,
/// which would take one fleeing alone down a stair. Only the fleeing half:
/// following through, a kind's to have or not, takes one only after the
/// adventurer it stood beside. Here rather than in the kind's row alone,
/// since every way a mercenary is given a brain comes through this, and a
/// trait added to the kind later would otherwise hand it the wit
/// unnoticed.
pub fn obey(world: &mut World, mercenary: Entity, order: Order) {
    let mind = world.resource::<MercenaryBrains>().mind(order);
    let at = world.get::<Position>(mercenary).map(|p| p.0);
    let mut e = world.entity_mut(mercenary);
    e.insert(mind);
    if let Some(mut wits) = e.get_mut::<Intelligence>() {
        wits.0 = wits.0.without(Wits::FLEES_THROUGH);
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

/// Says each mercenary's order, `holding`, on the look panel and on its
/// row of the party panel; and on the look panel that it is in the
/// adventurer's pay, and what it holds as they know it.
///
/// On the look panel the pay takes the place of its kind's own words, `a
/// sword for hire`, which it no longer is; the engine forecasts no fight
/// with an ally, so the order stands where a forecast would. What it holds is
/// said as a candidate's kit is, by [`KitSeen`]: its own kit at the level
/// it told when it was hired, anything handed to it by what is known of
/// it, and a potion of a kind not yet known by its face. On the party
/// panel it stands on every mercenary's row but one left on another
/// floor, which the panel marks as left behind instead. The party panel
/// is where a follower is read, a freed captive as much as a hired one,
/// so the binary leaves followers off the nearby rail, by the engine's
/// `NearbyPanel::without_followers`.
///
/// In `ViewSet::Annotate`, after the kind's own words, so they are there
/// to take off.
pub fn show_orders(
    look: Option<ResMut<InspectView>>,
    party: Option<ResMut<PartyView>>,
    hired: Query<&Mercenary>,
    kits: KitSeen,
    bestiary: Res<Bestiary>,
    mut facets: ResMut<Facets>,
) {
    if let Some(mut view) = look
        && let Some((who, order)) = view.subject.as_ref().and_then(|s| hired.get(s.entity).ok().map(|m| (s.entity, m.order)))
    {
        let kind = facets.declare("trait");
        let def = bestiary.defs.get(bestiary.defs.expect(MERCENARY));
        let for_hire: Vec<&str> = def.folded.says.iter().filter(|(name, ..)| name == MERCENARY).map(|(_, says, _)| says.as_str()).collect();
        view.facets.retain(|f| !(f.key == kind && for_hire.contains(&f.text.as_str())));
        view.facets.push(facets.facet("pay", IN_PAY));
        view.facets.push(facets.facet("order", order.doing()));
        for (how, said) in kits.of(who) {
            view.facets.push(facets.facet(KIT_FACET, format!("{} {said}", how.verb())));
        }
    }
    if let Some(mut view) = party {
        // One left on another floor waits where it was left whatever its
        // order, so its row says that, in the panel's own words, and not
        // an order it cannot be following; the rail is too narrow for
        // both.
        let away = view.elsewhere.clone();
        for row in view.rows_mut().filter(|r| !away.contains(&r.entity)) {
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

    use bevy::ecs::system::RunSystemOnce;
    use rl_engine::rl_bevy::props::{Hidden, spawn_prop};
    use rl_engine::rl_bevy::testing::{KeyScriptPlugin, press};

    use super::*;
    use crate::mercenaries::fixtures::hired;
    use crate::testing::{arena, arena_with, at, clear_floor, here, pass_turns, said, stand_on, still};

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
    /// any door but a locked one, with whatever stood on it taken away: the long way
    /// across the floor.
    fn the_long_way(app: &mut App, from: Point, steps: usize) -> Vec<Point> {
        let (map, floor) = (app.world().resource::<WorldMap>(), here(app));
        let mut came: BTreeMap<Point, Point> = BTreeMap::from([(from, from)]);
        let (mut queue, mut last) = (VecDeque::from([from]), from);
        while let Some(p) = queue.pop_front() {
            last = p;
            for d in [Direction::North, Direction::East, Direction::South, Direction::West] {
                let next = p + d.offset();
                let passable = map.is_walkable_on(floor, next) || (map.opens(next).is_some() && map.lock(next).is_none());
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
    /// floor with nothing on it to fight: every turn of the long way
    /// across, through whatever rooms, corridors, corners and doors it
    /// takes, and every time they stop. Walking at their own pace it cannot
    /// gain on them, so it holds three only by setting off as soon as the
    /// walk to them is longer than three, as the engine's `Keep::leader`
    /// does by measuring on foot, and never by catching up after.
    ///
    /// One floor is enough: the gap rule is unit-tested in `rl-rules` on
    /// made-up walls, and what is asked here is that the game wires that
    /// rule to a hired follower on a real floor's corners and doors.
    #[test]
    fn a_follower_keeps_within_three_of_the_adventurer_across_a_floor_with_nothing_to_fight() {
        let (mut app, me, here) = arena(0);
        clear_floor(&mut app);
        let brenna = hired(&mut app, me, here.offset(-1, 0), "Brenna");
        let way = the_long_way(&mut app, here, 60);
        assert!(way.len() >= 30, "a long way to walk, {} cells", way.len());
        // The one floor's way has the corner and the door the walk is for.
        let map = app.world().resource::<WorldMap>();
        assert!(way.iter().any(|cell| map.opens(*cell).is_some()), "the way crosses a door: {way:?}");
        let corner = way.windows(3).any(|w| Direction::between(w[0], w[1]) != Direction::between(w[1], w[2]));
        assert!(corner, "the way turns a corner: {way:?}");
        for stretch in way.chunks(6) {
            let mut widest = 0;
            walk(&mut app, me, stretch, |app| widest = widest.max(gap(app, me, brenna)));
            assert!(widest <= LEASH, "it fell {widest} behind on the way to {:?}", stretch.last());
            pass_turns(&mut app, 2);
            assert!(gap(&app, me, brenna) <= LEASH, "{} away where they stopped, {:?}", gap(&app, me, brenna), at(&app, me));
        }
    }

    /// On hold a mercenary keeps the cell it was ordered on while a foe
    /// stands in sight out of its reach; the same mercenary set to follow
    /// goes for the same foe, which is what holding stops. The foe is
    /// staged and mindless, so no seed changes the play.
    #[test]
    fn on_hold_it_keeps_its_post_while_a_foe_is_in_sight_beyond_reach() {
        let (mut app, me, here) = arena(0);
        let brenna = hired(&mut app, me, here.offset(-2, 0), "Brenna");
        let post = at(&app, brenna);
        order(&mut app, brenna, Order::Hold);
        let goblin = still(&mut app, "goblin", here.offset(2, 0));
        pass_turns(&mut app, 1);
        let sees = app.world().get::<Viewshed>(brenna).is_some_and(|v| v.can_see(at(&app, goblin)));
        assert!(sees, "the goblin is in its sight");
        for _ in 0..6 {
            pass_turns(&mut app, 1);
            assert_eq!(at(&app, brenna), post, "held at its post");
        }
        assert_eq!(app.world().get::<Post>(brenna), Some(&Post(post)), "posted where it was ordered");
        order(&mut app, brenna, Order::Follow);
        let mut moved = false;
        for _ in 0..6 {
            pass_turns(&mut app, 1);
            if at(&app, brenna) != post {
                moved = true;
                break;
            }
        }
        assert!(moved, "a follower goes for it");
    }

    /// A follower leaves a foe beyond its leash alone, never more than
    /// three from the adventurer; set to fight freely the same mercenary
    /// chases the same foe past the leash, strikes it down, and comes back
    /// within three once nothing is left. The foe is staged and mindless,
    /// so no seed changes the play.
    #[test]
    fn fighting_freely_it_chases_a_foe_in_sight_and_comes_back_when_none_is_left_where_a_follower_lets_it_be() {
        let (mut app, me, here) = arena(0);
        let brenna = hired(&mut app, me, here.offset(-1, 0), "Brenna");
        let goblin = still(&mut app, "goblin", here.offset(5, 0));
        app.world_mut().entity_mut(goblin).insert(Health::full(1));
        let down = |app: &App| app.world().get::<Health>(goblin).is_none_or(|h| h.current <= 0);
        for _ in 0..12 {
            pass_turns(&mut app, 1);
            assert!(gap(&app, me, brenna) <= LEASH, "a follower stays within {LEASH}, at {}", gap(&app, me, brenna));
            assert!(!down(&app), "and lets it be");
        }
        order(&mut app, brenna, Order::FightFreely);
        let mut widest = 0;
        let mut turns = 0;
        while !down(&app) {
            assert!(turns < 20, "it strikes the goblin down within twenty turns");
            pass_turns(&mut app, 1);
            widest = widest.max(gap(&app, me, brenna));
            turns += 1;
        }
        assert!(widest > LEASH, "it chased past the leash, {widest}");
        pass_turns(&mut app, 10);
        assert!(gap(&app, me, brenna) <= LEASH, "and came back, {} away", gap(&app, me, brenna));
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

    /// A hired follower on the far side of a row of dart plates the
    /// adventurer found and stepped round never sets foot on one: it goes
    /// round the row's end rather than over it, and still catches up. It is
    /// the engine's `Wits::WARY`, which a sapient hire has, pricing a shown
    /// trap that can still go off in its flow fields; the game adds nothing.
    #[test]
    fn a_follower_steps_round_the_traps_the_adventurer_has_found() {
        let (mut app, me, centre) = arena(2);
        let row: Vec<Point> = (-3..=4).map(|dx| centre.offset(dx, 0)).collect();
        let map = here(&app);
        // Found, as the adventurer finds a plate: shown, no longer hidden.
        for cell in row.clone() {
            let plate = app
                .world_mut()
                .run_system_once(move |mut commands: Commands, registries: Res<Registries>| {
                    spawn_prop(&mut commands, &registries, registries.props.expect("dart plate"), cell, map)
                })
                .expect("it runs");
            app.world_mut().entity_mut(plate).remove::<Hidden>();
        }
        stand_on(&mut app, me, centre.offset(-4, -1));
        let brenna = hired(&mut app, me, centre.offset(-4, 1), "Brenna");
        for _ in 0..9 {
            app.world_mut().write_message(Intent::new(me, Bump(Direction::East)));
            for _ in 0..2 {
                app.update();
                assert!(!row.contains(&at(&app, brenna)), "it stood on the plate at {:?}", at(&app, brenna));
            }
        }
        assert_eq!(at(&app, me), centre.offset(5, -1), "the adventurer walks beside the row");
        for _ in 0..12 {
            app.world_mut().write_message(Intent::new(me, Wait));
            app.update();
            assert!(!row.contains(&at(&app, brenna)), "it stood on the plate at {:?}", at(&app, brenna));
        }
        assert!(geometry::chebyshev(at(&app, me), at(&app, brenna)) <= LEASH, "and it caught up");
    }
}
