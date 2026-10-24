//! The waystone: a stone carried down from Mereholt that calls the
//! Drowned Gate to whoever kneels with it, and the gate mark it leaves,
//! which the gate then leads back to.
//!
//! Used from the pack, a waystone lands [`CallGate`], which sets the
//! adventurer kneeling: the engine's `Work` of the kind [`CALLING`],
//! [`KNEEL`] turns long, the use itself the first. The engine spends the
//! rest of the turns for them, as it does any of the player's work, paced
//! to be watched, and breaks it off as it breaks off any: a blow that
//! lands, a foe coming into sight, a key pressed or a turn spent on
//! anything else, each of which [`broken_off`] says. The stone is kept,
//! since it is spent only when the call is answered, which makes it a trip
//! home from a quiet room and never an escape from a fight: a foe that
//! comes into sight ends it before it can strike.
//!
//! Answered, [`answer_the_call`] spends one stone, stands a [`GATE_MARK`]
//! where the adventurer knelt, taking up the last one so there is only ever
//! one, turns the gate in Mereholt to lead to it, and warps the adventurer
//! onto the gate's town side. The warp is the engine's `WarpRequest`, which
//! writes `Travelled` from the cell they knelt on as a way through does for
//! whoever takes it, so a reader of that message hears of this too; the
//! engine's own followers do not follow a warp, so a freed captive is left
//! at the mark, where the gate brings the adventurer back to it, while a
//! hired mercenary near them comes home beside them, by
//! [`follow_through`](crate::mercenaries::follow_through), which answers
//! that message as it answers a stair. The mark and where the gate leads
//! are the engine's to save, a prop and a way through's `Transition`; the
//! mark's light is put back on it as it appears, by
//! [`light`](crate::light), and its place in memory is the map's, which
//! keeps every still thing it draws once seen.
//!
//! In Mereholt, on its streets and on the island alike, a waystone is
//! refused before the use is spent, by [`refuse_at_home`]: the gate is a
//! walk away, and a call from beside it would only spend a stone.

use atglyph::ecs::{BreakReason, Spent, WorkBroken, Works};
use atglyph::prelude::all::*;
use atglyph::primitives::turn::BASE_ACTION_COST;
use atglyph::rules::ability::RawValue;
use atglyph::rules::work::Work;
use bevy::ecs::system::SystemState;
use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use crate::armory::{Armory, ItemKind};
use crate::dungeon::{TOWN, map_of};
use crate::stairs::Gate;

/// The waystone's name in `items.ron`.
pub const WAYSTONE: &str = "waystone";

/// The kind of work a waystone sets its user to, as the panels name it.
pub const CALLING: &str = "calling the gate";

/// How many of the adventurer's turns the gate takes to come, the use
/// itself the first: long enough that a monster in the next room arrives
/// first, short enough to wait out in a cleared one.
pub const KNEEL: u16 = 10;

/// The gate mark's name in `props.ron`.
pub const GATE_MARK: &str = "gate mark";

/// What the log says to a waystone used in Mereholt.
pub const AT_HOME: &str = "The waystone lies quiet in your hand: in Mereholt, the gate is only a walk away.";

/// Sets its user kneeling to call the gate, as [`call`] does. Written
/// `(kind: "CallGate")`, on a waystone's `use` trigger.
///
/// It begins the call and spends nothing: a use that is broken off keeps
/// its stone, so the stone is spent by [`answer_the_call`] when the gate
/// comes, never by the use.
#[derive(Debug, Clone, Copy, Default)]
pub struct CallGate;

impl Effect for CallGate {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let user = landing.user;
        world.commands.queue(move |world: &mut World| call(world, user));
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        format!("kneel {KNEEL} quiet turns with it, and the Drowned Gate takes you home to Mereholt")
    }
}

impl FromArgs for CallGate {
    const KIND: &'static str = "CallGate";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        crate::effects::moving::no_args(args, Self::KIND).map(|_| Self)
    }
}

/// Whether `who` stands in Mereholt, the island and the streets alike,
/// where the gate is a walk away.
pub fn at_home(world: &World, who: Entity) -> bool {
    world.get::<OnMap>(who).is_some_and(|m| m.0 == TOWN)
}

/// Sets the adventurer `who` kneeling for [`KNEEL`] turns of [`CALLING`],
/// this one the first, and says so. Nothing for anyone else, for one at
/// work already, and in Mereholt, where it says why.
pub fn call(world: &mut World, who: Entity) {
    if world.get::<Player>(who).is_none() || world.get::<Working>(who).is_some() {
        return;
    }
    if at_home(world, who) {
        world.write_message(Tell::new(AT_HOME, Tones::MUTED));
        return;
    }
    let Some(kind) = world.resource::<WorkKinds>().get(CALLING) else { return };
    let mut state = SystemState::<Works>::new(world);
    if let Ok(mut works) = state.get_mut(world) {
        works.begin(who, Work::new(kind, KNEEL));
    }
    state.apply(world);
    world.write_message(Tell::new("You kneel with the waystone, and call the gate.", Tones::NOTICE));
}

/// Refuses a waystone used in Mereholt, saying why, before the use is
/// spent: the adventurer keeps the turn, as for any use refused.
///
/// In `ResolveSet::Travel`, ahead of the engine's own uses in
/// `ResolveSet::Act`, which then find the turn claimed and leave it.
pub fn refuse_at_home(
    mut uses: MessageReader<Intent<UseItem>>,
    mut resolution: Resolution,
    mut tell: MessageWriter<Tell>,
    armory: Res<Armory>,
    kinds: Query<&ItemKind>,
    players: Query<&OnMap, With<Player>>,
) {
    let Some(stone) = armory.defs.id(WAYSTONE) else { return };
    for intent in uses.read() {
        let home = players.get(intent.actor).is_ok_and(|on| on.0 == TOWN);
        if !home || !kinds.get(intent.action.0).is_ok_and(|k| k.0 == stone) || !resolution.claim(intent.actor) {
            continue;
        }
        resolution.failed(intent.actor, BASE_ACTION_COST);
        tell.write(Tell::new(AT_HOME, Tones::MUTED));
    }
}

/// Says so when the adventurer's call is broken off, whatever broke it:
/// harm, a foe in sight, the adventurer's own key or step, or the game.
/// The gate does not come, and the stone is still theirs. Nothing for a
/// death, which says enough.
///
/// In `TurnSet::React`, where the engine breaks the work off.
pub fn broken_off(mut broken: MessageReader<WorkBroken>, kinds: Res<WorkKinds>, players: Query<(), With<Player>>, mut tell: MessageWriter<Tell>) {
    let Some(calling) = kinds.get(CALLING) else { return };
    for ev in broken.read() {
        if ev.kind == calling && ev.reason != BreakReason::Died && players.contains(ev.actor) {
            tell.write(Tell::new("Your call is broken off, and the gate does not come. The waystone is still yours.", Tones::BAD));
        }
    }
}

/// Answers a finished call: [`go_home`] for the adventurer.
///
/// In `TurnSet::React`, where every game answer to a pass is read.
pub fn answer_the_call(mut commands: Commands, mut done: MessageReader<WorkDone>, kinds: Res<WorkKinds>, players: Query<(), With<Player>>) {
    let Some(calling) = kinds.get(CALLING) else { return };
    for ev in done.read().filter(|d| d.kind == calling && players.contains(d.actor)) {
        let who = ev.actor;
        commands.queue(move |world: &mut World| go_home(world, who));
    }
}

/// Takes `who` home through the gate from where they knelt: one waystone
/// spent, the gate mark moved there, the gate turned to lead to it, and a
/// warp asked for onto the gate's town side, answered in the next pass.
///
/// Nothing in Mereholt, and nothing but a line for one who carries no
/// stone any more, which kneeling leaves no turn to bring about.
pub fn go_home(world: &mut World, who: Entity) {
    let (Some(at), Some(map)) = (world.get::<Position>(who).map(|p| p.0), world.get::<OnMap>(who).map(|m| m.0)) else { return };
    if map == TOWN {
        return;
    }
    if !spend_a_stone(world, who) {
        world.write_message(Tell::new("The call fades: you carry no waystone to answer it.", Tones::MUTED));
        return;
    }
    set_mark(world, map, at);
    world.write_message(Tell::new("The Drowned Gate opens round you, and you step through it into Mereholt.", Tones::NOTICE));
    world.write_message(WarpRequest { actor: who, to: Destination::Place { map: TOWN, arrive: Arrive::Exit }, after: None });
}

/// Spends one waystone from `who`'s pack, as a spent charge is spent: one
/// fewer in a stack of more, the last one `Spent`, taken out of play at
/// the end of the pass. False when they carry none.
fn spend_a_stone(world: &mut World, who: Entity) -> bool {
    let Some(stone) = world.resource::<Armory>().defs.id(WAYSTONE) else { return false };
    let bag = world.get::<Inventory>(who).map(|b| b.items.clone()).unwrap_or_default();
    let Some(carried) = bag.into_iter().find(|i| world.get::<ItemKind>(*i).is_some_and(|k| k.0 == stone)) else { return false };
    match world.get_mut::<Stack>(carried) {
        Some(mut stack) if stack.count > 1 => stack.count -= 1,
        _ => {
            world.entity_mut(carried).insert(Spent);
        }
    }
    true
}

/// Stands the one gate mark at `at` on `map`, taking up any other, and
/// turns the gate in Mereholt to lead to it.
pub fn set_mark(world: &mut World, map: MapId, at: Point) {
    world.resource_scope(|world, registries: Mut<Registries>| {
        let Some(mark) = registries.props.id(GATE_MARK) else { return };
        let old: Vec<Entity> = world.query::<(Entity, &PropKind)>().iter(world).filter(|(_, k)| k.0 == mark).map(|(e, _)| e).collect();
        for e in old {
            world.despawn(e);
        }
        let mut queue = CommandQueue::default();
        spawn_prop(&mut Commands::new(&mut queue, world), &registries, mark, at, map);
        queue.apply(world);
    });
    let to = gate_to(Some((map, at)));
    let mut gates = world.query_filtered::<(&OnMap, &mut Transition), With<Gate>>();
    for (on, mut transition) in gates.iter_mut(world) {
        if on.0 == TOWN {
            transition.to = to;
        }
    }
}

/// Where the gate in Mereholt leads with the gate mark at `mark`: onto the
/// mark, or beside it when someone stands there, the engine's landing
/// finding the nearest free cell; with none, to floor one's way in.
pub fn gate_to(mark: Option<(MapId, Point)>) -> Destination {
    match mark {
        Some((map, at)) => Destination::Place { map, arrive: Arrive::At(at) },
        None => Destination::Place { map: map_of(1), arrive: Arrive::Entry },
    }
}

/// Where the gate mark stands, read for [`gate_to`] when the gate is laid.
#[derive(bevy::ecs::system::SystemParam)]
pub struct GateMarks<'w, 's> {
    registries: Res<'w, Registries>,
    props: Query<'w, 's, (&'static PropKind, &'static Position, &'static OnMap)>,
}

impl GateMarks<'_, '_> {
    /// The gate mark's place and cell, if a waystone has left one.
    pub fn mark(&self) -> Option<(MapId, Point)> {
        let mark = self.registries.props.id(GATE_MARK)?;
        self.props.iter().find(|(k, _, _)| k.0 == mark).map(|(_, p, on)| (on.0, p.0))
    }
}

#[cfg(test)]
mod tests {
    use atglyph::primitives::RunSeed;

    use super::*;
    use crate::testing::{adventurer, arena_on, continued, give, go_through, headless, here, make, monster, pack, said, settle, stand_on};

    /// The waystones `who` carries.
    fn stones(app: &App, who: Entity) -> u32 {
        pack(app, who).iter().filter(|(name, _)| name == WAYSTONE).map(|(_, n)| n).sum()
    }

    /// The first waystone in `who`'s pack.
    fn stone(app: &App, who: Entity) -> Entity {
        let world = app.world();
        let id = world.resource::<Armory>().defs.expect(WAYSTONE);
        let bag = &world.get::<Inventory>(who).expect("a pack").items;
        *bag.iter().find(|i| world.get::<ItemKind>(**i).is_some_and(|k| k.0 == id)).expect("a waystone in the pack")
    }

    /// Uses `who`'s waystone, as the pack's use key does.
    fn use_stone(app: &mut App, who: Entity) {
        let stone = stone(app, who);
        app.world_mut().write_message(Intent::new(who, UseItem(stone)));
        settle(app);
    }

    /// Runs frames until the adventurer stands in Mereholt, or long enough
    /// that ten turns of kneeling would long be done.
    fn wait_for_home(app: &mut App) {
        for _ in 0..60 {
            if here(app) == TOWN {
                break;
            }
            app.update();
        }
        settle(app);
    }

    /// The gate in Mereholt: where it stands and where it leads.
    fn gate(app: &mut App) -> (Point, Destination) {
        let world = app.world_mut();
        let mut q = world.query_filtered::<(&OnMap, &Position, &Transition), With<Gate>>();
        let found = q.iter(world).find(|(on, _, _)| on.0 == TOWN).map(|(_, p, t)| (p.0, t.to));
        found.expect("the gate stands in Mereholt")
    }

    /// Every gate mark in the run, on whatever place: where it stands.
    fn marks(app: &mut App) -> Vec<(MapId, Point)> {
        let mark = app.world().resource::<Registries>().props.expect(GATE_MARK);
        let world = app.world_mut();
        world.query::<(&PropKind, &OnMap, &Position)>().iter(world).filter(|(k, _, _)| k.0 == mark).map(|(_, on, p)| (on.0, p.0)).collect()
    }

    /// Every `Travelled` of the run, kept for a test to read.
    #[derive(Resource, Default)]
    struct Went(Vec<Travelled>);

    fn record_travels(mut travelled: MessageReader<Travelled>, mut went: ResMut<Went>) {
        went.0.extend(travelled.read().copied());
    }

    /// Every step of the run, kept for a test to read.
    #[derive(Resource, Default)]
    struct Steps(Vec<Stepped>);

    fn record_steps(mut stepped: MessageReader<Stepped>, mut steps: ResMut<Steps>) {
        steps.0.extend(stepped.read().copied());
    }

    /// Every work broken off in the run, kept for a test to read.
    #[derive(Resource, Default)]
    struct Broken(Vec<WorkBroken>);

    fn record_broken(mut broken: MessageReader<WorkBroken>, mut seen: ResMut<Broken>) {
        seen.0.extend(broken.read().copied());
    }

    /// A foe coming into sight while the adventurer kneels breaks the call
    /// off before it can strike: they look up, as from any work, the stone
    /// is kept, no mark is left and the log says so.
    #[test]
    fn a_foe_coming_into_sight_while_kneeling_breaks_the_call_off_and_the_stone_is_kept() {
        for s in 0..3 {
            let (mut app, me, knelt) = arena_on(s, 2);
            app.init_resource::<Broken>().add_systems(Turn, record_broken.in_set(TurnSet::Cleanup));
            let stone = stone(&app, me);
            app.world_mut().write_message(Intent::new(me, UseItem(stone)));
            app.update();
            monster(&mut app, "goblin", knelt.offset(3, 0));
            for _ in 0..30 {
                app.update();
            }
            let reasons: Vec<BreakReason> = app.world().resource::<Broken>().0.iter().map(|b| b.reason).collect();
            assert_eq!(reasons, [BreakReason::FoeSighted], "seed {s}: broken off by the sight of it, not by its blow");
            assert!(said(&app).iter().any(|l| l.starts_with("Your call is broken off")), "seed {s}: {:#?}", said(&app));
            assert_eq!((here(&app), stones(&app, me)), (map_of(2), 1), "seed {s}: still below, the stone kept");
            assert!(marks(&mut app).is_empty(), "seed {s}: no mark left");
        }
    }

    #[test]
    fn the_kit_holds_one_waystone() {
        let mut app = headless(RunSeed(0));
        let me = adventurer(&mut app);
        assert_eq!(stones(&app, me), 1, "{:?}", pack(&app, me));
    }

    /// With no gate mark yet, the gate leads to floor one's way in, as it
    /// always did.
    #[test]
    fn with_no_mark_the_gate_leads_to_floor_ones_way_in() {
        for s in 0..3 {
            let mut app = headless(RunSeed(s));
            adventurer(&mut app);
            assert!(marks(&mut app).is_empty(), "seed {s}");
            assert_eq!(gate(&mut app).1, Destination::Place { map: map_of(1), arrive: Arrive::Entry }, "seed {s}");
        }
    }

    /// Anywhere in Mereholt, the gate's own cell on the island among them,
    /// a waystone is refused, says why, keeps its stone and costs no turn.
    #[test]
    fn a_waystone_used_in_mereholt_is_refused_and_says_why() {
        for on_the_gate in [false, true] {
            let mut app = headless(RunSeed(1));
            let me = adventurer(&mut app);
            if on_the_gate {
                let (gate_cell, _) = gate(&mut app);
                stand_on(&mut app, me, gate_cell);
            }
            let before = app.world().resource::<Turns>().now();
            let lines = said(&app).len();
            use_stone(&mut app, me);
            assert!(app.world().get::<Working>(me).is_none(), "on the gate {on_the_gate}: not kneeling");
            assert_eq!(stones(&app, me), 1, "on the gate {on_the_gate}: the stone kept");
            assert_eq!(app.world().resource::<Turns>().now(), before, "on the gate {on_the_gate}: no turn spent");
            assert_eq!(&said(&app)[lines..], [AT_HOME.to_string()], "on the gate {on_the_gate}: says why, once");
        }
    }

    /// A blow while kneeling breaks the call off: a goblin three cells off
    /// walks up and strikes, the adventurer stays where they knelt, the
    /// stone is still in the pack, the log says so, and no gate comes
    /// however long they wait.
    #[test]
    fn a_blow_while_kneeling_breaks_the_call_off_and_the_stone_is_kept() {
        for s in 0..3 {
            let (mut app, me, knelt) = arena_on(s, 2);
            monster(&mut app, "goblin", knelt.offset(3, 0));
            use_stone(&mut app, me);
            for _ in 0..30 {
                app.update();
            }
            assert!(said(&app).iter().any(|l| l.starts_with("Your call is broken off")), "seed {s}: {:#?}", said(&app));
            assert!(app.world().get::<Working>(me).is_none(), "seed {s}: the kneeling broken off");
            assert_eq!((here(&app), stones(&app, me)), (map_of(2), 1), "seed {s}: still below, the stone kept");
            assert!(marks(&mut app).is_empty(), "seed {s}: no mark left");
        }
    }

    /// A step asked for while kneeling is taken: the adventurer stands up
    /// and walks, the call is left off, the stone is kept and no gate
    /// comes, so no key the player meant is swallowed by the kneeling.
    #[test]
    fn a_step_asked_for_while_kneeling_leaves_the_kneeling_off_and_is_taken() {
        let (mut app, me, knelt) = arena_on(4, 2);
        app.init_resource::<Steps>().add_systems(Turn, record_steps.in_set(TurnSet::Record));
        app.init_resource::<Broken>().add_systems(Turn, record_broken.in_set(TurnSet::Cleanup));
        use_stone(&mut app, me);
        assert!(app.world().get::<Working>(me).is_some(), "kneeling");
        app.world_mut().write_message(Intent::new(me, Bump(Direction::East)));
        settle(&mut app);
        assert_eq!(app.world().resource::<Steps>().0.iter().filter(|s| s.actor == me).count(), 1, "the step taken");
        assert_eq!(crate::testing::at(&app, me), knelt.offset(1, 0), "and the adventurer stands where it led");
        let reasons: Vec<BreakReason> = app.world().resource::<Broken>().0.iter().map(|b| b.reason).collect();
        assert_eq!(reasons, [BreakReason::LeftOff], "the call left off");
        wait_for_home(&mut app);
        assert_eq!((here(&app), stones(&app, me)), (map_of(2), 1), "still below, the stone kept");
    }

    /// Ten quiet turns of kneeling bring the adventurer home: onto the
    /// gate in Mereholt, the stone spent, a gate mark where they knelt and
    /// the gate leading to it, the trip told as a travel from that cell;
    /// and all of it through a save and a continue, after which the gate
    /// takes them back down onto the mark.
    #[test]
    fn finished_the_call_brings_the_adventurer_to_the_gate_and_the_gate_leads_to_the_mark_through_a_save() {
        // One seed: the arena is staged, and the seed only picks which cell the adventurer kneels on.
        let s = 0;
        let (mut app, me, knelt) = arena_on(s, 3);
        app.init_resource::<Went>().add_systems(Turn, record_travels.in_set(TurnSet::Record));
        use_stone(&mut app, me);
        wait_for_home(&mut app);
        let (gate_cell, leads) = gate(&mut app);
        assert_eq!((here(&app), crate::testing::at(&app, me)), (TOWN, gate_cell), "seed {s}: on the gate in Mereholt");
        assert_eq!(stones(&app, me), 0, "seed {s}: the stone spent");
        assert_eq!(marks(&mut app), [(map_of(3), knelt)], "seed {s}: one mark, where they knelt");
        assert_eq!(leads, Destination::Place { map: map_of(3), arrive: Arrive::At(knelt) }, "seed {s}: the gate leads to it");
        let went = &app.world().resource::<Went>().0;
        assert!(went.contains(&Travelled { actor: me, from: map_of(3), left: knelt, to: TOWN, after: None }), "seed {s}: {went:?}");
        assert!(said(&app).iter().any(|l| l.starts_with("The Drowned Gate opens round you")), "seed {s}");

        let text = app.world().resource::<Saves>().load(crate::save::SLOT).expect("the slot reads").expect("the arrival saved the run");
        let mut carried_on = continued(&text);
        let me = adventurer(&mut carried_on);
        assert_eq!(gate(&mut carried_on), (gate_cell, leads), "seed {s}: the gate still leads to the mark");
        assert_eq!(marks(&mut carried_on), [(map_of(3), knelt)], "seed {s}: the mark kept");
        go_through(&mut carried_on, me);
        assert_eq!((here(&carried_on), crate::testing::at(&carried_on, me)), (map_of(3), knelt), "seed {s}: back down onto the mark");
    }

    /// A second call moves the mark: the first is taken up, the one left
    /// stands where the adventurer knelt the second time, and the gate
    /// leads there.
    #[test]
    fn a_second_call_moves_the_mark_and_takes_up_the_old_one() {
        let (mut app, me, first) = arena_on(5, 3);
        use_stone(&mut app, me);
        wait_for_home(&mut app);
        go_through(&mut app, me);
        assert_eq!((here(&app), crate::testing::at(&app, me)), (map_of(3), first), "back on the mark");
        let second = first.offset(2, 1);
        stand_on(&mut app, me, second);
        let another = make(&mut app, WAYSTONE);
        give(&mut app, me, another);
        use_stone(&mut app, me);
        wait_for_home(&mut app);
        assert_eq!(here(&app), TOWN, "home again");
        assert_eq!(marks(&mut app), [(map_of(3), second)], "one mark, moved");
        assert_eq!(gate(&mut app).1, Destination::Place { map: map_of(3), arrive: Arrive::At(second) });
    }

    /// The priest keeps three waystones on the shelf at forty gold coins
    /// each, the only thing the temple sells or buys.
    #[test]
    fn the_priest_keeps_three_waystones_at_forty_gold_coins() {
        let mut app = headless(RunSeed(2));
        adventurer(&mut app);
        settle(&mut app);
        let world = app.world_mut();
        let priest = world.query::<(Entity, &crate::shops::Shopkeeper)>().iter(world).find(|(_, k)| k.0.keeper == crate::temple::TEMPLE).map(|(e, _)| e);
        let priest = priest.expect("the priest");
        let wares: Vec<(String, u32, Option<u32>)> = pack(&app, priest)
            .into_iter()
            .zip(app.world().get::<Inventory>(priest).expect("a bag").items.clone())
            .filter(|((name, _), _)| name != "gold coin")
            .map(|((name, count), item)| (name, count, app.world().get::<Worth>(item).map(|w| w.0)))
            .collect();
        assert_eq!(wares, [(WAYSTONE.to_string(), 3, Some(40))]);
    }
}
