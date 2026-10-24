//! Seeded Undermere runs, each driven by a fixed script and hashed.
//!
//! A fingerprint tripwire, not a property: the numbers below are what
//! these runs came to on the day they were written, and the test fails
//! when any change moves one. Nothing here has a property to state. It
//! pins the whole game at once, the town and the floors the seed builds,
//! the monsters and the loot it puts on them, how they sleep, wake, hunt
//! and fight, the light the adventurer sees by and what the tally counts,
//! so a quiet change to the order of a roll, a spawn or a system is a red
//! test rather than a run that plays differently in someone's hands. A
//! change that means to move them re-baselines the numbers on purpose and
//! says why in its commit.

use std::collections::BTreeSet;

use atglyph::prelude::all::*;
use atglyph::primitives::RunSeed;
use bevy::prelude::*;
use undermere::bestiary::Kind;
use undermere::stairs::Gate;
use undermere::tally::{Tally, score};

/// Folds one value into a running FNV-1a hash.
fn fold(hash: &mut u64, value: i64) {
    for byte in value.to_le_bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

/// The run as the fingerprint reads it: the clock and the place being
/// read, then every actor's place and health in the order of where they
/// stand, by place, row and column, then the tally, its score's lines and
/// its counts.
///
/// Where they stand rather than which entity they are: Bevy numbers every
/// entity from one allocator, which counts whatever is spawned anywhere
/// and hands a freed number to the next spawn, so a light made a moment
/// earlier or a body laid a pass later renumbers every monster after it,
/// and that is not a change to the run. Two things that block cannot
/// share a cell; anything else that can, a body or one that does not
/// block, is put after by its health, and two alike in all four are
/// folded the same whichever comes first. The tally's threat is read as
/// the score reads it, rounded, so a float summed in another order on
/// another machine is not a different run.
fn fingerprint(app: &mut App) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    fold(&mut hash, i64::from(app.world().resource::<Turns>().now()));
    fold(&mut hash, i64::from(app.world().resource::<WorldMap>().current().0));
    let world = app.world_mut();
    let mut actors: Vec<(u32, i32, i32, i32)> =
        world.query::<(&Position, &OnMap, &Health)>().iter(world).map(|(p, on, h)| (on.0.0, p.0.y, p.0.x, h.current)).collect();
    actors.sort();
    for (rank, (map, y, x, hp)) in actors.into_iter().enumerate() {
        fold(&mut hash, rank as i64);
        fold(&mut hash, i64::from(map));
        fold(&mut hash, i64::from(x));
        fold(&mut hash, i64::from(y));
        fold(&mut hash, i64::from(hp));
    }
    let tally = world.resource::<Tally>().clone();
    for (_, points) in score(&tally, false) {
        fold(&mut hash, points);
    }
    for count in [tally.kills, tally.damage_dealt, tally.damage_taken, tally.potions_drunk, tally.items_found, tally.gold_found, tally.deepest] {
        fold(&mut hash, i64::from(count));
    }
    hash
}

/// The ground the script walks: the engine's own reading of the place
/// for a mover that opens doors, less every cell a prop stands in the
/// way on, which a walk into would come to a question or an interaction
/// rather than a step.
struct Ground<'a, V> {
    view: V,
    map: &'a WorldMap,
    props: &'a BTreeSet<Point>,
}

impl<V: Grid2D> Grid2D for Ground<'_, V> {
    fn width(&self) -> i32 {
        self.view.width()
    }

    fn height(&self) -> i32 {
        self.view.height()
    }
}

impl<V: CostSource> CostSource for Ground<'_, V> {
    fn cost_idx(&self, idx: usize) -> Option<u32> {
        let at = self.map.to_world(self.view.idx_point(idx));
        if self.props.contains(&at) { None } else { self.view.cost_idx(idx) }
    }

    // The engine's own corner rule, which the opening view answers.
    fn lets_diagonal_past(&self, p: Point) -> bool {
        self.view.lets_diagonal_past(p)
    }
}

/// The step from `from` toward `goal` on the place being read, the
/// shortest way round whatever props stand in it and through its doors,
/// or `None` when nothing reaches it.
fn toward(world: &mut World, from: Point, goal: Point) -> Option<Direction> {
    let here = world.resource::<WorldMap>().current();
    let props: BTreeSet<Point> =
        world.query_filtered::<(&Position, &OnMap), (With<Prop>, With<Blocks>)>().iter(world).filter(|(_, on)| on.0 == here).map(|(p, _)| p.0).collect();
    let map = world.resource::<WorldMap>();
    let (from, goal) = (map.to_local(from)?, map.to_local(goal)?);
    let ground = Ground { view: map.opening_view(), map, props: &props };
    let mut flood = DijkstraMap::covering(&ground);
    flood.build(&ground, [goal], PathRules::default());
    Direction::between(from, flood.descend(from)?)
}

/// What the script does with one of the adventurer's turns.
enum Choice {
    /// A blow at the monster that way.
    Strike(Direction),
    /// Through the way on underfoot.
    Through,
    /// Whatever lies underfoot, taken up.
    TakeUp,
    /// A step on the way to the way on.
    Walk(Direction),
    /// Nothing to do: nowhere to go.
    Wait,
}

/// The script's choice for the adventurer's turn, first that applies: a
/// blow at a foe beside them, the first clockwise from north when
/// there are several, so which one is chosen is a fact about the board
/// rather than about the order they were spawned in; the way on, when
/// they stand on it; whatever lies where they stand, the first time they
/// stand there; and otherwise the next step to the way on, the gate in
/// Mereholt and the stairs down below.
fn decide(world: &mut World, me: Entity, tried: &mut BTreeSet<(MapId, Point)>) -> Choice {
    let here = world.resource::<WorldMap>().current();
    let at = world.get::<Position>(me).expect("standing somewhere").0;
    // `With<Actor>` rather than `Without<Dead>`: every monster now leaves
    // remains once it dies, still `Kind` and still lying beside whoever
    // killed it, and `Dead` itself lasts only the one pass between dying
    // and being laid down as a body; `Actor` is what a body no longer is.
    //
    // A foe by the rule a bump strikes by, so a blow is a blow: a captive
    // shackled beside the way, or one freed and following, is no foe, and
    // walking into either is freeing it or changing places, which the
    // script struck at for good once a captive stood beside its path.
    let mine = world.get::<Faction>(me).expect("on the adventurer's side").0;
    let beside: Vec<(Point, Faction)> = world
        .query_filtered::<(&Position, &OnMap, &Faction), (With<Kind>, With<Actor>)>()
        .iter(world)
        .filter(|(p, on, _)| on.0 == here && geometry::chebyshev(at, p.0) <= 1)
        .map(|(p, _, side)| (p.0, *side))
        .collect();
    let sides = &world.resource::<CombatRules>().factions;
    let foe =
        beside.into_iter().filter(|(_, side)| sides.is_hostile(mine, side.0)).filter_map(|(p, _)| Direction::between(at, p)).min_by_key(|dir| dir.index());
    if let Some(dir) = foe {
        return Choice::Strike(dir);
    }
    let way_on = if here == undermere::dungeon::TOWN {
        world.query_filtered::<(&Position, &OnMap), With<Gate>>().iter(world).find(|(_, on)| on.0 == here).map(|(p, _)| p.0)
    } else {
        world.query::<(&Position, &OnMap, &Stair)>().iter(world).find(|(_, on, stair)| on.0 == here && stair.climb == Climb::Down).map(|(p, _, _)| p.0)
    };
    if way_on == Some(at) {
        return Choice::Through;
    }
    let lying = world.query_filtered::<(&Position, &OnMap), With<Item>>().iter(world).any(|(p, on)| on.0 == here && p.0 == at);
    if lying && tried.insert((here, at)) {
        return Choice::TakeUp;
    }
    match way_on.and_then(|goal| toward(world, at, goal)) {
        Some(dir) => Choice::Walk(dir),
        None => Choice::Wait,
    }
}

/// How much health the script's adventurer has: far more than a real
/// one's forty, since the delve is to last the whole script rather than
/// end in a death on the way down, which ends the turns it is dealt.
const STURDY: i32 = 2000;

/// A run of `seed` played by the script: the adventurer, made sturdy,
/// sets out from Mereholt for the gate and takes every way down they come
/// to, for `turns` of their own turns, as [`decide`] chooses each. Comes
/// to the run's fingerprint and its tally.
fn run(seed: u64, turns: usize) -> (u64, Tally) {
    let mut app = undermere::testing::headless(RunSeed(seed));
    let me = undermere::testing::adventurer(&mut app);
    app.world_mut().entity_mut(me).insert(Health::full(STURDY));
    let mut tried = BTreeSet::new();
    let mut taken = 0;
    for _ in 0..turns * 4 {
        if taken == turns {
            break;
        }
        // The turns of work the adventurer took up, freeing a captive
        // they walked into, are the work's to spend: the turn is held
        // while it goes on, and an intent written into it leaves it off.
        if app.world().get::<MyTurn>(me).is_some() && app.world().get::<Working>(me).is_none() {
            let world = app.world_mut();
            match decide(world, me, &mut tried) {
                Choice::Strike(dir) | Choice::Walk(dir) => {
                    world.write_message(Intent::new(me, Bump(dir)));
                }
                Choice::Through => {
                    world.write_message(Intent::new(me, GoThrough));
                }
                Choice::TakeUp => {
                    world.write_message(Intent::new(me, PickUp));
                }
                Choice::Wait => {
                    world.write_message(Intent::new(me, Wait));
                }
            }
            taken += 1;
        }
        app.update();
    }
    assert_eq!(taken, turns, "seed {seed}: the adventurer was dealt every one of their turns");
    let tally = app.world().resource::<Tally>().clone();
    (fingerprint(&mut app), tally)
}

/// How many of the adventurer's turns each run is: enough for the script
/// to go down through the Drowned Town and the palace grounds into
/// the first of the kingdom's foundations, so three strata's building, people and light are pinned.
const TURNS: usize = 400;

/// Fingerprint tripwire: the same seed and the same script come to the
/// same run, and these runs come to these numbers. Re-baseline
/// deliberately, saying why in the commit.
///
/// Seed one twice and four and three once, each an app of its own, so the
/// four play side by side. Seed four stands where seed two did: once the
/// strata were reviewed, seed two's script came to floor six in its four
/// hundred turns without a kill, which is a walk and not a delve.
#[test]
fn fingerprint_tripwire_a_scripted_delve_from_mereholt_comes_to_the_same_run_every_time_on_three_seeds() {
    let seeds = [1, 4, 3, 1];
    let runs: Vec<(u64, Tally)> = std::thread::scope(|scope| {
        let playing: Vec<_> = seeds.map(|seed| scope.spawn(move || run(seed, TURNS))).into_iter().collect();
        playing.into_iter().map(|run| run.join().expect("the run played to its end")).collect()
    });
    for (seed, (_, tally)) in seeds.iter().zip(&runs) {
        assert!(tally.deepest >= 3, "seed {seed}: the script went down three floors at least, so the fingerprint is of a delve: {tally:?}");
        assert!(tally.kills > 0, "seed {seed}: and fought on the way, so it is not of a walk: {tally:?}");
    }
    let prints: Vec<u64> = runs.iter().map(|(print, _)| *print).collect();
    assert_eq!(prints[0], prints[3], "one seed, two runs, one fingerprint");
    assert!(prints[0] != prints[1] && prints[1] != prints[2] && prints[0] != prints[2], "another seed is another run: {prints:?}");
    assert_eq!(
        prints[..3],
        [12_358_914_591_356_714_561, 5_905_805_868_923_652_255, 12_680_218_757_316_006_782],
        "fingerprint tripwire: re-baselined once for the strata, the biomes on other floors and the bestiary moved with them \
         and the palace floors retuned, and for the script striking only a foe, so seed one's captive, shackled beside its \
         path on floor one, is walked past rather than struck at for good; and once more for the strata's review, the war \
         band and the ogre's feeding ground back on floors five and six, the floors four to six retuned and the flooding \
         vault holding one channel, with seed four for seed two; and once more on meeting main's statuses written on the status \
         and the traits a worn thing lends, through which the palace floors' withering dead now play; and once more for the \
         palace grounds' first wave, the royal wood and the palace gardens drawn on floors three and four beside the library \
         and the cathedral with their beasts in the spawn table and their landmarks the floor is grown round, and the pool \
         and the population drawing from a larger file; and once more for the drowned town's first wave, the old town and \
         the harbour drawn beside the halls, the cellars and the barracks on floors one and two, the barracks grown round the \
         gatehouse and the harbour round the sunken ship, and the harbour crab in the spawn table; and once more for the \
         seed branch, the engine's streams named under rl. and every floor grown and filled from its own place seed; and once more for the rename, the engine's streams named under atglyph. instead of rl."
    );
}
