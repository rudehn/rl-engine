//! What a test needs to stand an engine app up without a window.
//!
//! Every test of a system that runs in the loop needs the same three
//! things before it can ask its question: a map the player can stand on,
//! combat rules for anything that fights, and a way to press a key the way
//! a keyboard does. Each test module used to build its own, so the engine
//! carried nine copies of one world and three of one key player, and a
//! change to how a world is set up meant editing all of them. These are the
//! one copy, public so the engine's crates share it.
//!
//! Hidden, and outside semver: the engine's own fixtures, such as
//! [`ground`], [`two_sides`] and [`headless_app`], change whenever the
//! engine's tests want them to. A game's tests read the names they use
//! from `atglyph::testing`, which lists the stable ones.
//!
//! [`headless_app`] is the app itself; this
//! module is what goes into it, and what a test does with it once it runs:
//! [`settle`] and [`pass_turns`] move it on, [`let_time_pass`] jumps its
//! clock where only the clock is the question, [`player`], [`here`] and
//! [`at`] find where things stand, [`stand_on`], [`go_through`],
//! [`take_up`], [`put_down`], [`give`], [`equip`], [`unequip`], [`kill`],
//! [`lay`], [`lay_tile`] and [`wall_off`] set a scene up the way the
//! engine's own intents and warps would, and [`name_of`] and [`pack`] read
//! it back. None names a game's content: a kind of harm or a tile is the
//! test's to name. [`seeds`], [`first_seeds`] and [`par_seeds`] play a generation test over
//! a range of seeds on every core. [`ambiguity`] is the other thing every
//! game's tests want and none should write twice: the check that a game's
//! systems touching the same data are ordered, or allowed unordered for a
//! stated reason.

pub mod ambiguity;

use atglyph_core::turn::BASE_ACTION_COST;
use atglyph_core::{Point, RunSeed, TurnQueue};
use atglyph_grid::{Terrain, TileId, TileRegistry};
use atglyph_rules::damage::{DamageKind, DamageKindId, SubtractArmor};
use atglyph_rules::faction::FactionDef;
use atglyph_rules::{FactionId, Registry};
use bevy::input::InputSystems;
use bevy::prelude::*;

use crate::combat::{CombatRules, DamageStages};
use crate::cue::CueWait;
use crate::prelude::{
    Arrive, DamageEvent, Destination, DropItem, Equip, GoThrough, Intent, Inventory, MapId, OnMap, PickUp, PlaceBuild, Player, Position, Stack, Unequip, Wait,
    WarpRequest,
};
use crate::registries::Registries;
use crate::turn::Turns;
use crate::world::WorldMap;

/// A headless app with the engine's core and no window, for the engine's
/// own tests and its crates' tests. A game's tests stand an app up with
/// `atglyph::testing::windowless` or `headless`.
pub fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin, crate::plugin::CorePlugin));
    app
}

/// The seed range a generation test plays, and the cores it plays them on:
/// the same helpers the generator's own tests use, for a game's.
pub use atglyph_mapgen::testing::{QUICK, first_seeds, par_seeds, seeds};

/// The seed every test world and stream is built from, so a test that
/// fails fails the same way twice.
pub const TEST_SEED: RunSeed = RunSeed(5);

/// How many tiles a side [`ground`] is, its walled edge included.
const GROUND: i32 = 48;

/// Installs open ground over the standard tiles as map zero, makes it the
/// map being read, and returns where to stand: its middle, with floor for
/// twenty-three tiles every way, so sight reaches as far as its range and a
/// test's actors stand wherever it puts them. Its edge is wall, so a step
/// off it is something a test can bump into.
///
/// Map zero because an entity with no [`OnMap`] stands there, so a test
/// spawns its actors with a [`Position`] and nothing else. Inserts the
/// [`WorldMap`].
pub fn ground(app: &mut App) -> Point {
    ground_with(app, TileRegistry::standard())
}

/// [`ground`] over `tiles`, which must hold a `wall` and a `floor`, for a
/// test that registers a tile of its own.
pub fn ground_with(app: &mut App, tiles: TileRegistry) -> Point {
    let mut terrain = Terrain::filled(GROUND, GROUND, tiles.expect("floor"));
    let wall = tiles.expect("wall");
    for i in 0..GROUND {
        for p in [Point::new(i, 0), Point::new(i, GROUND - 1), Point::new(0, i), Point::new(GROUND - 1, i)] {
            terrain.set(p, wall);
        }
    }
    let start = Point::new(GROUND / 2, GROUND / 2);
    let build = PlaceBuild {
        terrain,
        entry: start,
        exit: None,
        spots: Vec::new(),
        locks: Vec::new(),
        secrets: Vec::new(),
        ambient: atglyph_grid::Light::DARK,
        peaceful: false,
    };
    let mut map = WorldMap::new(tiles.tables());
    map.install_place(MapId::ZERO, build);
    map.switch_to(MapId::ZERO);
    map.visit(MapId::ZERO);
    app.insert_resource(map);
    start
}

/// How many chunks a side [`streamed_ground`] is, and how many tiles a
/// side each chunk is.
pub const STREAMED_CHUNKS: i32 = 8;
/// A chunk's side on [`streamed_ground`].
pub const STREAMED_CHUNK: i32 = 16;

/// Open ground built a chunk at a time: map zero, [`STREAMED_CHUNKS`]
/// chunks a side of [`STREAMED_CHUNK`] tiles, keeping the chunks one each
/// way round the player's loaded, floor everywhere but a wall round the
/// whole place. A test that walks a streamed place, or leaves part of one
/// behind, stands on this.
#[derive(Debug, Clone)]
pub struct StreamedGround(pub TileRegistry);

impl StreamedGround {
    /// The layout every streamed ground has, its entry at its middle.
    pub fn layout() -> crate::places::Streamed {
        let side = STREAMED_CHUNKS * STREAMED_CHUNK;
        crate::places::Streamed {
            chunk: STREAMED_CHUNK,
            chunks: (STREAMED_CHUNKS, STREAMED_CHUNKS),
            reach: 1,
            entry: Point::new(side / 2, side / 2),
            ambient: atglyph_grid::Light::DARK,
            peaceful: false,
        }
    }
}

impl crate::places::PlaceRules for StreamedGround {
    fn build(&self, map: MapId, _: atglyph_core::PlaceSeed) -> Result<PlaceBuild, atglyph_mapgen::BuildError> {
        Err(atglyph_mapgen::BuildError::new("streamed ground", format!("{map:?} is streamed, never built whole")))
    }

    fn tiles(&self) -> atglyph_grid::TileTables {
        self.0.tables()
    }

    fn streamed(&self, _: MapId) -> Option<crate::places::Streamed> {
        Some(Self::layout())
    }

    fn build_chunk(&self, _: MapId, chunk: Point, _: atglyph_core::PlaceSeed) -> Result<Terrain, atglyph_mapgen::BuildError> {
        let (wall, floor) = (self.0.expect("wall"), self.0.expect("floor"));
        let edge = STREAMED_CHUNKS * STREAMED_CHUNK - 1;
        let origin = Point::new(chunk.x * STREAMED_CHUNK, chunk.y * STREAMED_CHUNK);
        Ok(Terrain::from_fn(STREAMED_CHUNK, STREAMED_CHUNK, |p| {
            let at = p + origin;
            if at.x == 0 || at.y == 0 || at.x == edge || at.y == edge { wall } else { floor }
        }))
    }
}

/// Installs [`StreamedGround`] over the standard tiles as map zero, its
/// rules as the game's and the [`TEST_SEED`] as the run's, makes it the
/// map being read with its chunks loaded round the entry, and returns the
/// entry. Inserts the [`WorldMap`].
pub fn streamed_ground(app: &mut App) -> Point {
    let rules = StreamedGround(TileRegistry::standard());
    let layout = StreamedGround::layout();
    let mut map = WorldMap::new(rules.0.tables());
    map.install_streamed(MapId::ZERO, layout);
    map.switch_to(MapId::ZERO);
    map.visit(MapId::ZERO);
    let seed = atglyph_core::PlaceSeed::of(TEST_SEED, 0);
    map.stream(MapId::ZERO, layout.entry, |chunk| crate::places::PlaceRules::build_chunk(&rules, MapId::ZERO, chunk, seed)).expect("open ground builds");
    app.insert_resource(map).insert_resource(crate::places::PlaceRulesRes(Box::new(rules))).insert_resource(crate::seed::Seed(TEST_SEED));
    layout.entry
}

/// Two sides at war and the one kind of damage between them, as
/// [`two_sides`] registered them.
#[derive(Debug, Clone, Copy)]
pub struct TwoSides {
    /// The player's side.
    pub ours: FactionId,
    /// Everyone the player's side is hostile to, and who is hostile back.
    pub theirs: FactionId,
    /// The one damage kind, `kinetic`, armored.
    pub kind: DamageKindId,
}

/// Inserts combat rules and [`Registries`] for a test that fights: the two sides of [`TwoSides`],
/// mutually hostile, one armored damage kind, armor subtracted, and
/// [`TEST_SEED`] as the run's seed.
pub fn two_sides(app: &mut App) -> TwoSides {
    let kinds = Registry::from_defs(vec![DamageKind::new("kinetic")]).expect("one kind");
    let kind = kinds.expect("kinetic");
    let names = Registry::from_defs(vec![FactionDef::new("ours"), FactionDef::new("theirs")]).expect("two sides");
    let (ours, theirs) = (names.expect("ours"), names.expect("theirs"));
    app.insert_resource(CombatRules::new().hostile(ours, theirs))
        .insert_resource(Registries { damage_kinds: kinds, factions: names, ..Default::default() })
        .insert_resource(DamageStages(vec![Box::new(SubtractArmor)]))
        .insert_resource(crate::seed::Seed(TEST_SEED));
    TwoSides { ours, theirs, kind }
}

/// Plays keys into [`ButtonInput<KeyCode>`] the way a keyboard does.
///
/// A key pressed on `ButtonInput` from outside the schedule is wiped in
/// `PreUpdate` before any system sees it, because Bevy clears
/// `just_pressed` at the top of every frame. This presses what
/// [`KeyScript`] holds after that clearing, and lets it go the frame after.
/// Adds Bevy's `InputPlugin` if the app has none.
pub struct KeyScriptPlugin;

impl Plugin for KeyScriptPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<bevy::input::InputPlugin>() {
            app.add_plugins(bevy::input::InputPlugin);
        }
        app.init_resource::<KeyScript>().add_systems(PreUpdate, play_keys.after(InputSystems));
    }
}

/// Keys to press on the next frame, and keys to keep down.
#[derive(Resource, Debug, Default)]
pub struct KeyScript {
    next: Vec<KeyCode>,
    held: Vec<KeyCode>,
    pinned: Vec<KeyCode>,
}

impl KeyScript {
    /// Queues `key` to be pressed on the next frame and released on the
    /// one after.
    pub fn press(&mut self, key: KeyCode) {
        self.next.push(key);
    }

    /// Queues `key` to be pressed on the next frame and kept down until
    /// [`release`](Self::release): what a finger resting on a key does,
    /// one `just_pressed` and then `pressed` for as long as it rests.
    pub fn hold(&mut self, key: KeyCode) {
        self.next.push(key);
        self.pinned.push(key);
    }

    /// Lets go of a key held with [`hold`](Self::hold), on the next frame.
    pub fn release(&mut self, key: KeyCode) {
        self.pinned.retain(|k| *k != key);
        self.held.push(key);
    }
}

fn play_keys(mut script: ResMut<KeyScript>, mut keys: ResMut<ButtonInput<KeyCode>>) {
    let pinned = script.pinned.clone();
    for k in std::mem::take(&mut script.held) {
        if !pinned.contains(&k) {
            keys.release(k);
        }
    }
    let next = std::mem::take(&mut script.next);
    for k in &next {
        keys.press(*k);
    }
    script.held = next;
}

/// Presses `key` for one frame and lets it go on the next, running both.
///
/// Needs [`KeyScriptPlugin`].
pub fn press(app: &mut App, key: KeyCode) {
    app.world_mut().resource_mut::<KeyScript>().press(key);
    app.update();
    app.update();
}

/// Two frames: enough for one resolved action, and whatever it sets off,
/// to settle, and for a run begun at startup to deal the player a turn.
pub fn settle(app: &mut App) {
    app.update();
    app.update();
}

/// The player, the run begun first if it has not been.
///
/// # Panics
/// Panics when even a begun run stood no player anywhere.
pub fn player(app: &mut App) -> Entity {
    let find = |app: &mut App| app.world_mut().query_filtered::<Entity, With<Player>>().iter(app.world()).next();
    if let Some(me) = find(app) {
        return me;
    }
    settle(app);
    find(app).expect("the run put the player somewhere")
}

/// Waits the player forward `n` whole turns, one frame each.
pub fn pass_turns(app: &mut App, n: u32) {
    let me = player(app);
    for _ in 0..n {
        app.world_mut().write_message(Intent::new(me, Wait));
        app.update();
    }
}

/// Lets `turns` whole turns of the clock pass at once, then runs one
/// frame, so what is written in elapsed time sees the span without the
/// turns being played.
///
/// For a test whose property is a clock: a refill, a regeneration, a
/// conjured thing's end, anything counted by comparing
/// [`Turns::now`](TurnQueue::now) or [`Turns::turn_number`] against a
/// reading kept from before. Playing those turns instead deals every one
/// of them to every actor, which is the cost this exists to skip.
///
/// What moves: the clock, by `turns` times [`BASE_ACTION_COST`], and every
/// queued actor's due time by the same span, so nobody gains or loses a
/// turn on anyone else and ties keep the order they were queued in. An
/// actor holding the turn is out of the queue and still holds it, now at
/// the later reading. The one frame is what lets a system that reads the
/// clock see it: the engine's charge refill, and a game's own.
///
/// What does not: nothing counted per whole turn or per action. No
/// [`TurnEnd`](crate::TurnEnd) is sent for the span, so statuses, fire,
/// gases and the light stand as they were; nobody is dealt a turn in it,
/// so no mind decides, no one moves and no work is done. A test of those
/// plays its turns with [`pass_turns`]. A system that keeps its own last
/// reading counts from the first frame it ran, so the app must have run a
/// frame in play before the jump for it to count the whole span. And the
/// frame after is an ordinary one: when nobody holds the turn, the queue
/// deals at the new reading as it would at any.
///
/// # Panics
/// Panics when a [`CueWait`] is up: no pass runs while it is, so the one
/// frame would show the jump to nothing that reads the clock.
pub fn let_time_pass(app: &mut App, turns: u32) {
    assert!(!app.world().resource::<CueWait>().is_waiting(), "a turn hold is up, so no pass would run to see the time let pass");
    let span = turns * BASE_ACTION_COST;
    let mut clock = app.world_mut().resource_mut::<Turns>();
    let later: Vec<(Entity, u32)> = in_schedule_order(&clock).into_iter().map(|(who, due)| (who, due + span)).collect();
    // Requeued in the order the scheduler would deal them, which is what
    // keeps a tie as it was: `import` breaks ties by the order it is given,
    // and `export` gives the heap's order, not the schedule's. Importing
    // also tells the clock the turns are already counted, which is why no
    // `TurnEnd` is sent for them.
    let now = clock.now() + span;
    clock.import(now, later);
    app.update();
}

/// Every entry of `queue`, earliest first and ties in the order they were
/// queued, read from a copy dealt to the end.
fn in_schedule_order(queue: &TurnQueue<Entity>) -> Vec<(Entity, u32)> {
    let mut queue = queue.clone();
    let mut order = Vec::new();
    while let Some(due) = queue.peek_time() {
        // An overdue entry is due before the reading; the clock only runs
        // forward, so it is left where it is for that one.
        queue.set_now(due.max(queue.now()));
        order.push((queue.pop_due(|_| true).expect("the head is due once the clock reads its time"), due));
    }
    order
}

/// The place being read, which is the one the player stands in.
pub fn here(app: &App) -> MapId {
    app.world().resource::<WorldMap>().current()
}

/// Where `who` stands.
///
/// # Panics
/// Panics when `who` stands nowhere.
pub fn at(app: &App, who: Entity) -> Point {
    app.world().get::<Position>(who).expect("standing somewhere").0
}

/// Moves `who` to `to` on the place being read, through the engine's own
/// warp, so what it stood on and what it stands on are both kept right,
/// and lets the move settle.
pub fn stand_on(app: &mut App, who: Entity, to: Point) {
    let here = here(app);
    app.world_mut().write_message(WarpRequest { actor: who, to: Destination::Place { map: here, arrive: Arrive::At(to) }, after: None });
    settle(app);
}

/// Takes the way through `who` stands on, as the key does.
pub fn go_through(app: &mut App, who: Entity) {
    app.world_mut().write_message(Intent::new(who, GoThrough));
    settle(app);
}

/// Takes up whatever lies where `who` stands, as the key does.
pub fn take_up(app: &mut App, who: Entity) {
    app.world_mut().write_message(Intent::new(who, PickUp));
    settle(app);
}

/// Puts `thing` down where `who` stands, as the pack's key does.
pub fn put_down(app: &mut App, who: Entity, thing: Entity) {
    app.world_mut().write_message(Intent::new(who, DropItem::all(thing)));
    settle(app);
}

/// Kills `victim` with a blow of the damage kind `kind` names, past what
/// anything survives, credited to `by` or to nobody, the way a burn or a
/// fall would be, and lets the death settle.
///
/// # Panics
/// Panics on a kind the game never registered.
pub fn kill(app: &mut App, victim: Entity, by: Option<Entity>, kind: &str) {
    let kind = app.world().resource::<Registries>().damage_kinds.expect(kind);
    app.world_mut().write_message(DamageEvent::new(victim, atglyph_rules::Hit::from_source(by, kind, 100_000)));
    settle(app);
}

/// Puts `thing` in `who`'s pack.
///
/// # Panics
/// Panics when `who` carries no pack.
pub fn give(app: &mut App, who: Entity, thing: Entity) {
    app.world_mut().get_mut::<Inventory>(who).expect("a pack").items.push(thing);
}

/// Puts `thing` in `who`'s pack and on, by `Equip`, as a key would: for
/// the player while it holds the turn.
pub fn equip(app: &mut App, who: Entity, thing: Entity) {
    give(app, who, thing);
    app.world_mut().write_message(Intent::new(who, Equip(thing)));
    settle(app);
}

/// Takes `thing` off `who`, by `Unequip`, as a key would, and lets it
/// settle.
pub fn unequip(app: &mut App, who: Entity, thing: Entity) {
    app.world_mut().write_message(Intent::new(who, Unequip(thing)));
    settle(app);
}

/// What `thing` is called.
///
/// # Panics
/// Panics on a thing with no `Name`.
pub fn name_of(app: &App, thing: Entity) -> String {
    app.world().get::<Name>(thing).expect("a name").as_str().to_string()
}

/// What `who` carries, by name and count, in pack order.
///
/// # Panics
/// Panics when `who` carries no pack.
pub fn pack(app: &App, who: Entity) -> Vec<(String, u32)> {
    let world = app.world();
    let bag = &world.get::<Inventory>(who).expect("a pack").items;
    bag.iter().map(|item| (name_of(app, *item), world.get::<Stack>(*item).map_or(1, |s| s.count))).collect()
}

/// Lays `thing` on the floor of the place being read, at `to`.
pub fn lay(app: &mut App, thing: Entity, to: Point) {
    let here = here(app);
    app.world_mut().entity_mut(thing).insert((Position(to), OnMap(here)));
}

/// Paints `cells` of the place being read with `tile`, so a test has the
/// ground it wants there whatever the place built.
pub fn lay_tile(app: &mut App, cells: &[Point], tile: TileId) {
    let mut map = app.world_mut().resource_mut::<WorldMap>();
    for p in cells {
        map.set_tile(*p, tile);
    }
}

/// Walls `cells` of the place being read in with the first tile nobody can
/// walk on, so a test has a wall where it wants one.
pub fn wall_off(app: &mut App, cells: &[Point]) {
    let mut map = app.world_mut().resource_mut::<WorldMap>();
    let count = map.tables().names.len() as u16;
    for cell in cells {
        for id in 0..count {
            map.set_tile(*cell, TileId(id));
            if !map.is_walkable(*cell) {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{Actor, Blocks, MyTurn, Player, Position, RevealsMap, Viewshed};
    use crate::consumable::{Consumable, WhenEmpty};
    use crate::plugin::Turn;
    use crate::state::EngineState;
    use crate::testing::headless_app;
    use crate::turn::TurnEnd;

    /// The start `ground` hands back is floor, with floor for seven tiles
    /// every way, and the player stood there is dealt a turn.
    #[test]
    fn the_ground_stands_a_player_on_open_floor() {
        let mut app = headless_app();
        app.add_plugins(crate::fov::FovPlugin);
        let start = ground(&mut app);
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(6), RevealsMap)).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        assert!(app.world().get::<MyTurn>(player).is_some(), "the player holds a turn");
        let map = app.world().resource::<WorldMap>();
        for dy in -7..=7 {
            for dx in -7..=7 {
                assert!(map.is_walkable(start.offset(dx, dy)), "({dx}, {dy}) from the start is floor");
            }
        }
    }

    /// A player on [`ground`] holding the first turn, play begun, and
    /// `plugins` added before it began.
    fn holding_the_turn<M>(plugins: impl bevy::app::Plugins<M>) -> (App, Entity) {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, plugins));
        two_sides(&mut app);
        let start = ground(&mut app);
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(6), RevealsMap)).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        settle(&mut app);
        assert!(app.world().get::<MyTurn>(player).is_some(), "the player holds the turn");
        (app, player)
    }

    /// Every queued entry, earliest first and ties in the order they were
    /// queued: the order the scheduler deals them in.
    fn queued(app: &App) -> Vec<(Entity, u32)> {
        in_schedule_order(app.world().resource::<Turns>())
    }

    #[test]
    fn letting_time_pass_moves_the_clock_and_every_queued_actor_by_the_same_span() {
        let (mut app, player) = holding_the_turn(());
        let start = at(&app, player);
        let [a, b, c] = [1, 2, 3].map(|dx| app.world_mut().spawn((Actor, Position(start.offset(dx, 2)))).id());
        let now = app.world().resource::<Turns>().now();
        // `b` before `c` at the same time, so the tie is the order they
        // were queued in, and `a` queued last but due first, which leaves
        // the heap holding `c` ahead of `b`: a jump that requeued them in
        // the heap's order would swap the tie.
        app.world_mut().resource_mut::<Turns>().import(now, [(b, now + 70), (c, now + 70), (a, now + 30)]);
        let before = queued(&app);
        assert_eq!(before, vec![(a, now + 30), (b, now + 70), (c, now + 70)]);

        let_time_pass(&mut app, 50);

        assert_eq!(app.world().resource::<Turns>().now(), now + 5000, "the clock is fifty turns on");
        assert_eq!(queued(&app), vec![(a, now + 5030), (b, now + 5070), (c, now + 5070)], "everyone is due fifty turns on, in the same order");
        assert!(app.world().get::<MyTurn>(player).is_some(), "the player still holds the turn it held");
    }

    #[test]
    fn letting_time_pass_recharges_what_recharges_by_the_clock() {
        for (turns, full) in [(25, true), (24, false)] {
            let (mut app, _) = holding_the_turn(crate::items::ItemsPlugin);
            let mut wand = Consumable::new(3, WhenEmpty::Kept).recharging(2500);
            wand.left = 2;
            let wand = app.world_mut().spawn(wand).id();
            settle(&mut app);

            let_time_pass(&mut app, turns);

            let left = app.world().get::<Consumable>(wand).expect("still there").left;
            assert_eq!(left == 3, full, "after {turns} turns of a refill every 2500, {left} of 3 charges");
        }
    }

    #[test]
    fn letting_time_pass_ends_no_whole_turn_so_what_ticks_per_turn_stands_still() {
        #[derive(Resource, Default)]
        struct Ended(usize);
        fn count(mut ends: MessageReader<TurnEnd>, mut ended: ResMut<Ended>) {
            ended.0 += ends.read().count();
        }
        let (mut app, _) = holding_the_turn(());
        app.init_resource::<Ended>().add_systems(Turn, count);
        let turn = app.world().resource::<Turns>().turn_number();

        let_time_pass(&mut app, 50);

        assert_eq!(app.world().resource::<Turns>().turn_number(), turn + 50);
        assert_eq!(app.world().resource::<Ended>().0, 0, "no turn ended for the span let pass");
        pass_turns(&mut app, 1);
        assert_eq!(app.world().resource::<Ended>().0, 1, "the next turn played ends as any turn does");
    }

    #[test]
    #[should_panic(expected = "a turn hold is up")]
    fn letting_time_pass_while_a_turn_hold_is_up_is_refused() {
        let (mut app, _) = holding_the_turn(());
        let mut cue_wait = app.world_mut().resource_mut::<CueWait>();
        cue_wait.watch();
        cue_wait.wait();
        let_time_pass(&mut app, 1);
    }

    /// A scripted key reads as just pressed to a system in `Update`, once.
    #[test]
    fn a_scripted_key_is_just_pressed_for_one_frame() {
        #[derive(Resource, Default)]
        struct Seen(u32);
        fn count(keys: Res<ButtonInput<KeyCode>>, mut seen: ResMut<Seen>) {
            if keys.just_pressed(KeyCode::Space) {
                seen.0 += 1;
            }
        }
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, KeyScriptPlugin)).init_resource::<Seen>().add_systems(Update, count);
        press(&mut app, KeyCode::Space);
        app.update();
        assert_eq!(app.world().resource::<Seen>().0, 1);
    }
}
