//! Where the adventurer goes, the party goes: a hired mercenary near them
//! as they leave a map is sent after them, and one that was not waits.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_core::Grid2D;
use rl_engine::rl_grid::{OpacitySource, Terrain, fov};

use super::{LEASH, Mercenary, Order, TONE};

/// Who of the party is near enough as the adventurer leaves: where each
/// stands, on which map, under what order, and whom it follows.
type Party<'w, 's> = Query<'w, 's, (Entity, &'static Mercenary, &'static Leader, &'static Position, &'static OnMap), Without<Dead>>;

/// How far from the cell the adventurer left a mercenary may stand and
/// still come through with them: one past the [`LEASH`]. A follower sets
/// off before the walk to them is longer than the leash, but it moves on
/// its own turn, after theirs, and the commonest way to take a stair is to
/// walk onto it and take it at once, so the stair is taken with the
/// follower a step short of where it is going. Holding the travel to the
/// leash itself would leave a follower behind for a reason the player
/// never chose.
pub const FOLLOW_REACH: i32 = LEASH + 1;

/// Sends after the adventurer, on their own [`Travelled`], every hired
/// mercenary on the map they left that is not holding, stood within
/// [`FOLLOW_REACH`] of the cell they left, and had a clear line to it:
/// each by a [`WarpRequest`] after them to a free cell beside where they
/// landed, which the narrator tells as following them,
/// answered in the next pass, the nearest of them first and then by
/// entity, so who gets the cell beside them never depends on the order a
/// query ran in. Every one of the party left on that map gets a line in
/// the mercenaries' tone saying why: no free cell beside the landing, too
/// far behind, no line to where the adventurer went, or holding; a player
/// who learned it only from the rail on the next floor would have lost
/// one without being told.
///
/// The game's rather than the engine's [`Wits::FOLLOWS_THROUGH`], which
/// takes only whoever stood beside the adventurer through, the same pass,
/// where the party follows from further and around the one who holds; and
/// a mercenary never carries [`Wits::FLEES_THROUGH`], since
/// [`obey`](super::obey) takes it off: a hurt one fleeing down a stair
/// alone would be lost to the run for a reason the player never chose. This answers the stairs, the
/// gate and a waystone's warp alike, since each writes the same
/// `Travelled` from the cell the adventurer left. One left behind keeps
/// its `Leader` and is frozen where it stands, as everything on a map
/// nobody reads is, and is sent after them again the next time they leave
/// that map with it near.
///
/// The line is asked of the map left, not of the adventurer's sight,
/// which the warp has already marked to be cast on the map they arrived
/// on; and it is a line rather than what the light shows, so a hireling
/// two steps away in the dark is not left for the dark. In
/// `TurnSet::React`, where a reaction's warp is resolved before the
/// player acts again.
pub fn follow_through(
    mut travelled: MessageReader<Travelled>,
    party: Party,
    ground: Ground,
    places: Query<(&Position, &OnMap)>,
    tones: Res<Tones>,
    mut warps: MessageWriter<WarpRequest>,
    mut tell: MessageWriter<Tell>,
) {
    for went in travelled.read() {
        let Ok((landed, on)) = places.get(went.actor) else { continue };
        let (landed, to) = (landed.0, on.0);
        let tone = tones.get(TONE).unwrap_or(Tones::TEXT);
        let mut near: Vec<(i32, Entity, Point, &str)> = Vec::new();
        let mut left: Vec<(Entity, String)> = Vec::new();
        for (e, hired, leader, pos, on) in &party {
            if leader.0 != went.actor || on.0 != went.from {
                continue;
            }
            let d = geometry::chebyshev(pos.0, went.left);
            let why = if hired.order == Order::Hold {
                "holds where you left them"
            } else if d > FOLLOW_REACH {
                "is too far behind to follow you"
            } else if !ground.in_line(went.from, went.left, pos.0) {
                "did not see where you went, and stays behind"
            } else {
                near.push((d, e, pos.0, hired.name.as_str()));
                continue;
            };
            left.push((e, format!("{} {why}.", hired.name)));
        }
        for (mercenary, name, cell) in land_beside(near, landed, went.left, |p| ground.is_free(to, p)) {
            match cell {
                Some(cell) => {
                    warps.write(WarpRequest::new(mercenary, Destination::Place { map: to, arrive: Arrive::At(cell) }).after(went.actor));
                }
                None => left.push((mercenary, format!("{name} stays behind: there is no room beside you."))),
            }
        }
        left.sort_by_key(|(e, _)| *e);
        for (_, line) in left {
            tell.write(Tell::new(line, tone));
        }
    }
}

/// The cell each of `near` lands on beside `landed`, the nearest to the
/// cell `left` first and then by entity, so who gets the cell beside the
/// adventurer never depends on the order a query ran in; `None` for one
/// every cell is gone for. Each of `near` is how far it stood from `left`,
/// who it is, where it stood and what it carries through to its answer,
/// its name for the line that says it stayed. A pure function, so the
/// order and the distinctness are tested without a world: `free` says
/// whether a cell is open ground nobody stands on.
fn land_beside<T>(mut near: Vec<(i32, Entity, Point, T)>, landed: Point, left: Point, free: impl Fn(Point) -> bool) -> Vec<(Entity, T, Option<Point>)> {
    near.sort_by_key(|(d, e, ..)| (*d, *e));
    let mut taken: Vec<Point> = Vec::new();
    near.into_iter()
        .map(|(_, mercenary, stood, carried)| {
            let cell = cell_beside(landed, stood - left, &taken, &free);
            taken.extend(cell);
            (mercenary, carried, cell)
        })
        .collect()
}

/// The free cell beside `landed` a mercenary that stood `offset` from the
/// adventurer lands on: the one on the same side of them if it is free,
/// and otherwise the nearest to it, then in the order of
/// [`Direction::ALL`]. Free means `free` and not already given to one sent
/// before it in the same arrival, whose warp has yet to land. `None` when
/// every cell beside the landing is taken.
fn cell_beside(landed: Point, offset: Point, taken: &[Point], free: &impl Fn(Point) -> bool) -> Option<Point> {
    let side = landed + Point::new(offset.x.signum(), offset.y.signum());
    let mut open: Vec<(i32, usize, Point)> = Direction::ALL
        .iter()
        .enumerate()
        .map(|(i, dir)| (i, landed + dir.offset()))
        .filter(|(_, p)| free(*p) && !taken.contains(p))
        .map(|(i, p)| ((p.x - side.x).pow(2) + (p.y - side.y).pow(2), i, p))
        .collect();
    open.sort();
    open.first().map(|(.., p)| *p)
}

/// The maps the party is read off and landed on, current or not: what
/// blocks a line on the map left, and who stands where on the one arrived
/// on.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Ground<'w> {
    map: Res<'w, WorldMap>,
    occupancy: Res<'w, Occupancy>,
}

impl Ground<'_> {
    /// Whether `cell` on `on` is free to land on: walkable, with nobody
    /// standing there.
    fn is_free(&self, on: MapId, cell: Point) -> bool {
        self.map.is_walkable_on(on, cell) && !self.occupancy.is_occupied_on(on, cell)
    }

    /// Whether `there` is in an unbroken line from `from` on `on`, through
    /// what blocks sight on that map as it stands. True on a map that is
    /// no built place, which Undermere never travels from: a mercenary is
    /// not left for a map this cannot read.
    fn in_line(&self, on: MapId, from: Point, there: Point) -> bool {
        let Some(place) = self.map.place(on) else { return true };
        let lines = Lines { terrain: &place.terrain, opaque: &self.map.tables().opaque };
        // Sight reaches a disc, and a corner of the reach's square lies
        // past a disc of the reach's radius: twice it holds the whole
        // square.
        fov::can_see(&lines, from, there, 2 * FOLLOW_REACH)
    }
}

/// A built place's tiles read for what blocks sight, whether or not it is
/// the map being read.
struct Lines<'a> {
    terrain: &'a Terrain,
    opaque: &'a [bool],
}

impl Grid2D for Lines<'_> {
    fn width(&self) -> i32 {
        self.terrain.width()
    }

    fn height(&self) -> i32 {
        self.terrain.height()
    }
}

impl OpacitySource for Lines<'_> {
    fn is_opaque_idx(&self, idx: usize) -> bool {
        self.opaque[self.terrain.get_idx(idx).index()]
    }
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_core::RunSeed;

    use super::*;
    use crate::dungeon::{TOWN, map_of};
    use crate::mercenaries::fixtures::hired;
    use crate::mercenaries::obey;
    use crate::testing::Way;
    use crate::testing::{
        adventurer, arrive, at, clear_floor, go_through, headless, here, lay_floor, pass_turns, said, settle, stand_on, still, sturdy, wall_off,
    };

    /// How far round the cell the adventurer leaves from the staging lays
    /// open floor: one past the reach, so a mercenary can stand both just
    /// inside it and just outside it.
    const OPEN: i32 = FOLLOW_REACH + 1;

    /// Every travel of the run, with where the traveller stood the moment
    /// it landed, before it could take a step on the far side.
    #[derive(Resource, Default)]
    struct Landings(Vec<(Entity, MapId, Point)>);

    fn record_landings(mut travelled: MessageReader<Travelled>, standing: Query<&Position>, mut landings: ResMut<Landings>) {
        for went in travelled.read() {
            if let Ok(p) = standing.get(went.actor) {
                landings.0.push((went.actor, went.to, p.0));
            }
        }
    }

    /// Where `who` last landed, and on which map.
    fn landed(app: &App, who: Entity) -> Option<(MapId, Point)> {
        app.world().resource::<Landings>().0.iter().rev().find(|(e, ..)| *e == who).map(|(_, map, p)| (*map, *p))
    }

    /// A run on `floor` (nought for Mereholt) with the adventurer on the
    /// way `way` of it, nothing else alive on any place, and open ground
    /// round it: where they stand.
    fn on_the_way(seed: u64, floor: u32, way: Way) -> (App, Entity, Point) {
        let mut app = headless(RunSeed(seed));
        app.init_resource::<Landings>().add_systems(Turn, record_landings.in_set(TurnSet::Record));
        let me = adventurer(&mut app);
        if floor > 0 {
            arrive(&mut app, map_of(floor));
        }
        let cell = way_cell(&mut app, way);
        stand_on(&mut app, me, cell);
        open_ground(&mut app, me, cell);
        (app, me, cell)
    }

    /// Where the way `way` stands on the place being read.
    fn way_cell(app: &mut App, want: Way) -> Point {
        let here = here(app);
        crate::testing::way_on(app.world_mut(), here, want).expect("the place has that way")
    }

    /// Nothing alive on any place but the adventurer and the party, nothing
    /// lying here, and open floor with nothing standing on it within
    /// [`OPEN`] of `centre`, but the ways through, which keep their tiles.
    fn open_ground(app: &mut App, me: Entity, centre: Point) {
        sturdy(app, me);
        clear_floor(app);
        let here = here(app);
        let world = app.world_mut();
        let others: Vec<Entity> = world
            .query_filtered::<Entity, (Or<(With<crate::bestiary::Kind>, With<crate::townsfolk::Townsperson>)>, Without<Mercenary>)>()
            .iter(world)
            .collect();
        for other in others {
            world.despawn(other);
        }
        let ways: Vec<Point> = crate::testing::ways(world, here).into_iter().map(|(p, ..)| p).collect();
        let props: Vec<Entity> = world
            .query_filtered::<(Entity, &Position), With<Prop>>()
            .iter(world)
            .filter(|(_, p)| geometry::chebyshev(p.0, centre) <= OPEN)
            .map(|(e, _)| e)
            .collect();
        for prop in props {
            world.despawn(prop);
        }
        let terrain = &world.resource::<WorldMap>().place(here).expect("a built place").terrain;
        let (w, h) = (terrain.width(), terrain.height());
        let ground: Vec<Point> = geometry::square(centre, OPEN).filter(|p| p.x > 0 && p.y > 0 && p.x < w - 1 && p.y < h - 1 && !ways.contains(p)).collect();
        lay_floor(app, &ground);
        settle(app);
    }

    /// The open cells exactly `d` from `centre`, nobody on them, in a
    /// fixed order.
    fn ring(app: &App, centre: Point, d: i32) -> Vec<Point> {
        let here = here(app);
        let (map, taken) = (app.world().resource::<WorldMap>(), app.world().resource::<Occupancy>());
        let mut cells: Vec<Point> = geometry::square(centre, d)
            .filter(|p| geometry::chebyshev(*p, centre) == d && map.is_walkable_on(here, *p) && !taken.is_occupied_on(here, *p))
            .collect();
        cells.sort();
        cells
    }

    /// Which map `who` is on.
    fn map_of_actor(app: &App, who: Entity) -> MapId {
        app.world().get::<OnMap>(who).expect("on a map").0
    }

    /// Puts `who` under `order`, as the order key does for the party.
    fn order(app: &mut App, who: Entity, order: Order) {
        app.world_mut().get_mut::<Mercenary>(who).expect("a mercenary").order = order;
        obey(app.world_mut(), who, order);
    }

    /// Whether `who` came through beside the adventurer: on their map, and
    /// landed on a cell next to the one they landed on.
    fn came_beside(app: &App, who: Entity, me: Entity) -> bool {
        let (Some((there, p)), Some((mine, q))) = (landed(app, who), landed(app, me)) else { return false };
        there == mine && map_of_actor(app, who) == mine && geometry::chebyshev(p, q) == 1
    }

    /// Whether `who` stayed where it stood on `map`, still led by `me`.
    fn stayed(app: &App, who: Entity, map: MapId, stood: Point, me: Entity) -> bool {
        map_of_actor(app, who) == map && at(app, who) == stood && app.world().get::<Leader>(who).map(|l| l.0) == Some(me)
    }

    #[test]
    fn it_comes_down_the_stair_beside_the_adventurer_from_four_cells_away_and_is_left_from_five_or_on_hold_and_the_log_says_so_over_a_span_of_seeds() {
        for s in 0..4 {
            let (mut app, me, stair) = on_the_way(s, 1, Way::Down);
            let (four, five, two) = (ring(&app, stair, 4)[0], ring(&app, stair, 5)[0], ring(&app, stair, 2)[0]);
            let near = hired(&mut app, me, four, "Brenna");
            let far = hired(&mut app, me, five, "Corwin");
            let holding = hired(&mut app, me, two, "Dagny");
            order(&mut app, holding, Order::Hold);
            go_through(&mut app, me);
            assert_eq!(here(&app), map_of(2), "seed {s}: the adventurer went down");
            assert!(came_beside(&app, near, me), "seed {s}: from four cells away, a step behind mid-walk, it came down beside them");
            assert!(stayed(&app, far, map_of(1), five, me), "seed {s}: from five it was left where it stood, still theirs");
            assert!(stayed(&app, holding, map_of(1), two, me), "seed {s}: on hold it kept its post, still theirs");
            assert!(said(&app).iter().any(|l| l == "Corwin is too far behind to follow you."), "seed {s}: {:?}", said(&app));
            assert!(said(&app).iter().any(|l| l == "Dagny holds where you left them."), "seed {s}: {:?}", said(&app));
            assert!(!said(&app).iter().any(|l| l.starts_with("Brenna") && l != "Brenna follows you."), "seed {s}: {:?}", said(&app));
            assert!(said(&app).iter().any(|l| l == "Brenna follows you."), "seed {s}: the one that came is told of as following: {:?}", said(&app));
            assert!(!said(&app).iter().any(|l| l.ends_with("leaves.")), "seed {s}: and nobody as leaving: {:?}", said(&app));
        }
    }

    #[test]
    fn one_within_reach_with_no_line_to_where_the_adventurer_stood_is_left_where_it_stood_and_the_log_says_so_over_a_span_of_seeds() {
        for s in 0..3 {
            let (mut app, me, stair) = on_the_way(s, 1, Way::Down);
            let two = ring(&app, stair, 2)[0];
            let walls = ring(&app, stair, 1);
            wall_off(&mut app, &walls);
            let brenna = hired(&mut app, me, two, "Brenna");
            go_through(&mut app, me);
            assert_eq!(here(&app), map_of(2), "seed {s}: the adventurer went down");
            assert!(stayed(&app, brenna, map_of(1), two, me), "seed {s}: walled off from them, it was left where it stood");
            assert!(said(&app).iter().any(|l| l == "Brenna did not see where you went, and stays behind."), "seed {s}: {:?}", said(&app));
        }
    }

    #[test]
    fn it_comes_through_the_gate_and_a_waystones_warp_as_through_a_stair_over_a_span_of_seeds() {
        for s in 0..3 {
            let (mut app, me, gate) = on_the_way(s, 0, Way::Gate);
            let two = ring(&app, gate, 2)[0];
            let brenna = hired(&mut app, me, two, "Brenna");
            go_through(&mut app, me);
            assert_eq!(here(&app), map_of(1), "seed {s}: through the gate onto floor one");
            assert!(came_beside(&app, brenna, me), "seed {s}: and it came through beside them");

            // Kneeling with a waystone on floor three, ten turns of it, the
            // mercenary keeping by them all the while on open ground.
            arrive(&mut app, map_of(3));
            let knelt = at(&app, me);
            open_ground(&mut app, me, knelt);
            let two = ring(&app, knelt, 2)[0];
            stand_on(&mut app, brenna, two);
            let stone = waystone(&app, me);
            app.world_mut().write_message(Intent::new(me, UseItem(stone)));
            for _ in 0..60 {
                if here(&app) == TOWN {
                    break;
                }
                app.update();
            }
            settle(&mut app);
            assert_eq!(here(&app), TOWN, "seed {s}: the call brought the adventurer home");
            assert!(came_beside(&app, brenna, me), "seed {s}: and it came home through the gate beside them");
        }
    }

    /// The first waystone in `who`'s pack, which every run sets out with.
    fn waystone(app: &App, who: Entity) -> Entity {
        let world = app.world();
        let id = world.resource::<crate::armory::Armory>().defs.expect(crate::waystone::WAYSTONE);
        let bag = &world.get::<Inventory>(who).expect("a pack").items;
        *bag.iter().find(|i| world.get::<crate::armory::ItemKind>(**i).is_some_and(|k| k.0 == id)).expect("a waystone in the pack")
    }

    #[test]
    fn one_left_behind_waits_where_it_stood_and_follows_when_the_adventurer_comes_back_for_it_and_leaves_again_over_a_span_of_seeds() {
        for s in 0..3 {
            let (mut app, me, down) = on_the_way(s, 1, Way::Down);
            let five = ring(&app, down, 5)[0];
            let brenna = hired(&mut app, me, five, "Brenna");
            go_through(&mut app, me);
            assert!(stayed(&app, brenna, map_of(1), five, me), "seed {s}: left where it stood, still theirs");
            pass_turns(&mut app, 5);
            assert!(stayed(&app, brenna, map_of(1), five, me), "seed {s}: and frozen there while the adventurer is below");

            // Back up the stair, which lands them on the way down again.
            go_through(&mut app, me);
            assert_eq!((here(&app), at(&app, me)), (map_of(1), down), "seed {s}: back on floor one, on the way down");
            pass_turns(&mut app, 3);
            assert!(geometry::chebyshev(at(&app, brenna), down) <= LEASH, "seed {s}: it closed on them once they were back");
            go_through(&mut app, me);
            assert_eq!(here(&app), map_of(2), "seed {s}: down again");
            assert!(came_beside(&app, brenna, me), "seed {s}: and this time it came with them");
        }
    }

    /// Stands something that never moves on every open cell beside floor
    /// two's way in but `spare` of them, then brings the adventurer back
    /// onto floor one's way down with open ground round it: where that is.
    fn crowd_the_landing(app: &mut App, me: Entity, spare: usize) -> Point {
        arrive(app, map_of(2));
        let entry = at(app, me);
        open_ground(app, me, entry);
        for cell in ring(app, entry, 1).into_iter().skip(spare) {
            still(app, "goblin", cell);
        }
        settle(app);
        arrive(app, map_of(1));
        let down = way_cell(app, Way::Down);
        stand_on(app, me, down);
        let world = app.world_mut();
        let floor_one: Vec<Entity> =
            world.query_filtered::<(Entity, &OnMap), With<crate::bestiary::Kind>>().iter(world).filter(|(_, on)| on.0 == map_of(1)).map(|(e, _)| e).collect();
        for monster in floor_one {
            world.despawn(monster);
        }
        let ground: Vec<Point> = geometry::square(down, OPEN).filter(|p| *p != down).collect();
        lay_floor(app, &ground);
        settle(app);
        down
    }

    #[test]
    fn with_every_cell_beside_the_landing_taken_it_stays_where_it_stood_and_the_log_says_so() {
        for s in 0..3 {
            let mut app = headless(RunSeed(s));
            app.init_resource::<Landings>().add_systems(Turn, record_landings.in_set(TurnSet::Record));
            let me = adventurer(&mut app);
            sturdy(&mut app, me);
            let down = crowd_the_landing(&mut app, me, 0);
            let beside = ring(&app, down, 1)[0];
            let brenna = hired(&mut app, me, beside, "Brenna");
            go_through(&mut app, me);
            assert_eq!(here(&app), map_of(2), "seed {s}: the adventurer went down");
            assert!(stayed(&app, brenna, map_of(1), beside, me), "seed {s}: no room below, so it stayed, still theirs");
            assert!(said(&app).iter().any(|l| l == "Brenna stays behind: there is no room beside you."), "seed {s}: {:?}", said(&app));
        }
    }

    /// The `n`th entity, for a test of the pure landing order.
    fn entity(n: u32) -> Entity {
        Entity::from_raw_u32(n).expect("a valid index")
    }

    /// Every cell beside `landed` free: `cells` to leave open, the rest of
    /// the eight neighbours taken.
    fn only(landed: Point, cells: &[Point]) -> impl Fn(Point) -> bool + use<> {
        let open: Vec<Point> = cells.iter().map(|c| landed + *c).collect();
        move |p| open.contains(&p)
    }

    /// Two mercenaries with the whole ring beside the landing free each
    /// land beside the adventurer, on a cell of their own.
    #[test]
    fn two_mercenaries_land_on_distinct_cells_beside_the_adventurer() {
        let (landed, left) = (Point::new(20, 20), Point::new(5, 5));
        let everywhere = |p: Point| geometry::chebyshev(p, landed) == 1;
        let near = vec![(1, entity(1), left + Point::new(1, 0), "Brenna"), (2, entity(2), left + Point::new(2, 0), "Corwin")];
        let placed = land_beside(near, landed, left, everywhere);
        let cells: Vec<Point> = placed.iter().map(|(.., c)| c.expect("a cell beside the landing")).collect();
        assert_ne!(cells[0], cells[1], "on cells of their own");
        assert!(cells.iter().all(|c| geometry::chebyshev(*c, landed) == 1), "both beside the adventurer");
    }

    /// With one cell left beside the landing, the one that stood nearer
    /// the way takes it though it was listed, and hired, second, and the
    /// farther gets none, its name carried through for the line that says
    /// it stayed.
    #[test]
    fn with_one_cell_left_beside_the_landing_the_nearer_takes_it_even_listed_second_and_the_farther_gets_none() {
        let (landed, left) = (Point::new(20, 20), Point::new(5, 5));
        let free = only(landed, &[Point::new(1, 0)]);
        let far = (2, entity(1), left + Point::new(2, 0), "Corwin");
        let near = (1, entity(2), left + Point::new(1, 0), "Brenna");
        let placed = land_beside(vec![far, near], landed, left, free);
        assert_eq!(placed, vec![(entity(2), "Brenna", Some(landed + Point::new(1, 0))), (entity(1), "Corwin", None)]);
    }

    /// Two that stood as near the way as each other: the lower entity
    /// takes the one cell left, whichever order they are listed in, so who
    /// lands never depends on the order a query ran in.
    #[test]
    fn as_near_as_each_other_the_lower_entity_takes_the_one_cell_left() {
        let (landed, left) = (Point::new(20, 20), Point::new(5, 5));
        let free = only(landed, &[Point::new(1, 0)]);
        let (low, high) = if entity(3) < entity(7) { (entity(3), entity(7)) } else { (entity(7), entity(3)) };
        let (a, b) = ((2, high, left + Point::new(2, 0), ()), (2, low, left + Point::new(0, 2), ()));
        for order in [vec![a, b], vec![b, a]] {
            let placed = land_beside(order, landed, left, &free);
            assert_eq!(placed, vec![(low, (), Some(landed + Point::new(1, 0))), (high, (), None)], "whatever order the query ran in");
        }
    }

    /// A mercenary lands on the cell beside the adventurer on the side it
    /// stood on when that cell is free, on the free cell nearest it when it
    /// is not, never on one already given in the same arrival, and nowhere
    /// when every cell beside the landing is taken.
    #[test]
    fn a_mercenary_lands_on_its_own_side_of_the_adventurer_when_that_cell_is_free_and_on_the_nearest_otherwise() {
        let landed = Point::new(20, 20);
        let all = |p: Point| geometry::chebyshev(p, landed) == 1;
        assert_eq!(cell_beside(landed, Point::new(-3, 0), &[], &all), Some(landed + Point::new(-1, 0)), "its own side");
        let without_west = |p: Point| all(p) && p != landed + Point::new(-1, 0);
        let there = cell_beside(landed, Point::new(-3, 0), &[], &without_west).expect("a cell");
        assert_eq!(geometry::chebyshev(there, landed + Point::new(-1, 0)), 1, "the nearest to its own side, {there:?}");
        assert_eq!(
            cell_beside(landed, Point::new(-3, 0), &[landed + Point::new(-1, 0)], &all).map(|p| p != landed + Point::new(-1, 0)),
            Some(true),
            "one given away is not given twice"
        );
        assert_eq!(cell_beside(landed, Point::new(1, 0), &[], &|_| false), None, "none when every cell is gone");
    }

    /// Two mercenaries come down the stair beside the adventurer through
    /// the real map and occupancy; the order and the distinct cells when
    /// room is short are the unit tests of `land_beside`, the log when
    /// there is none is the crowded-landing test above.
    #[test]
    fn two_mercenaries_come_down_the_stair_beside_the_adventurer_on_cells_of_their_own() {
        let (mut app, me, stair) = on_the_way(0, 1, Way::Down);
        let (one, two) = (ring(&app, stair, 1)[0], ring(&app, stair, 2)[0]);
        let brenna = hired(&mut app, me, one, "Brenna");
        let corwin = hired(&mut app, me, two, "Corwin");
        go_through(&mut app, me);
        assert!(came_beside(&app, brenna, me) && came_beside(&app, corwin, me), "both came down beside them");
        assert_ne!(at(&app, brenna), at(&app, corwin), "on cells of their own");
    }

    /// A mercenary never flees through a way, under any order and whatever
    /// it was given; following through, the other half of `travels`, is
    /// its kind's to have, and the kind's own row has neither.
    #[test]
    fn a_mercenary_never_carries_the_wit_to_flee_through_a_way_under_any_order() {
        let (mut app, me, here) = crate::testing::arena(0);
        let row = crate::testing::monster(&mut app, crate::mercenaries::MERCENARY, here.offset(2, 0));
        let wits = |app: &App, who: Entity| app.world().get::<Intelligence>(who).expect("a mind's wits").0;
        assert!(!wits(&app, row).has(Wits::FOLLOWS_THROUGH) && !wits(&app, row).has(Wits::FLEES_THROUGH), "the kind's own row travels neither way");
        let brenna = hired(&mut app, me, here.offset(-2, 0), "Brenna");
        app.world_mut().get_mut::<Intelligence>(brenna).expect("wits").0 = Wits::SAPIENT.with(Wits::TRAVELS);
        for o in [Order::Follow, Order::Hold, Order::FightFreely, Order::Follow] {
            order(&mut app, brenna, o);
            assert!(!wits(&app, brenna).has(Wits::FLEES_THROUGH), "under {o:?} it does not flee through a way, whatever it was given before");
            assert!(wits(&app, brenna).has(Wits::FOLLOWS_THROUGH), "under {o:?} what it was given of following is left alone");
        }
    }
}
