//! Triggers: what a prop or a thing does at the moments it answers.
//!
//! A subsystem that owns a moment, the items resolver for a use, throwing
//! for a landing, combat for a shot made and a shot that struck, props for
//! a cell stepped on and a prop broken, writes [`Fired`] and nothing else.
//! [`land_triggers`] is the one system that answers: it finds the carrier's
//! [`Triggers`] for that moment, works out the cells its [`Area`] covers
//! around where the moment happened, and lands the effects on everyone
//! standing there. What a use costs the thing is the consumables' business,
//! read off the same messages.
//!
//! Moments are interned in [`Moments`] rather than enumerated, so a game
//! adds its own, an overheating gun or a charged console, with
//! [`AddMoment::add_moment`] and reports it from a system of its own.

use std::sync::Arc;

use bevy::prelude::*;
use rl_core::{Interner, Point};
use rl_rules::{Area, EffectSpec, Names, TriggerSpec};

use super::{EffectKinds, EffectWorld, Effects, Landing, Source};
use crate::cue::{Anchor, Cue, Cued, LookOf};
use crate::world::WorldMap;

/// What sets a trigger off: a thing used, a throw come to rest, an attack
/// made or landed, a cell stepped on, a prop broken, or one a game names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Moment;

/// A registered moment.
pub type MomentId = rl_core::Id<Moment>;

/// Every moment in play, in the order they were first named.
///
/// Interned rather than a closed enum so a game adds its own with one line
/// and reports it from a system of its own. The engine's are interned
/// first, so their ids are the constants on this type.
#[derive(Resource, Debug, Clone)]
pub struct Moments(Interner<Moment>);

impl Moments {
    /// A thing in the bag was used.
    pub const USE: MomentId = MomentId::from_raw(0);
    /// A thrown thing came to rest.
    pub const LAND: MomentId = MomentId::from_raw(1);
    /// An attack was made with a worn thing.
    pub const FIRE: MomentId = MomentId::from_raw(2);
    /// An attack made with a worn thing struck someone.
    pub const HIT: MomentId = MomentId::from_raw(3);
    /// Somebody stepped onto a prop's cell.
    pub const ENTERED: MomentId = MomentId::from_raw(4);
    /// A prop was broken.
    pub const DESTROYED: MomentId = MomentId::from_raw(5);

    /// The engine's moments, in the order their ids are handed out.
    pub const BUILT_IN: [&'static str; 6] = ["use", "land", "fire", "hit", "entered", "destroyed"];

    /// The id for `name`, assigning a new one if it is unseen.
    pub fn declare(&mut self, name: &str) -> MomentId {
        self.0.intern(name)
    }

    /// The id for `name`, if it has been declared.
    pub fn get(&self, name: &str) -> Option<MomentId> {
        self.0.get(name)
    }

    /// The name behind `id`.
    pub fn name(&self, id: MomentId) -> &str {
        self.0.name(id)
    }
}

impl Default for Moments {
    fn default() -> Self {
        let mut names = Interner::new();
        for name in Self::BUILT_IN {
            names.intern(name);
        }
        Self(names)
    }
}

/// Declares a moment while the app is being built.
pub trait AddMoment {
    /// Declares the moment `name`. Look its id up again with [`Moments::get`].
    fn add_moment(&mut self, name: &str) -> &mut Self;
}

impl AddMoment for App {
    fn add_moment(&mut self, name: &str) -> &mut Self {
        self.init_resource::<Moments>();
        self.world_mut().resource_mut::<Moments>().declare(name);
        self
    }
}

/// One thing a carrier does at one moment.
///
/// The list behind an `Arc`, built once per definition and shared by every
/// copy, and the fire count beside it, which is each carrier's own.
#[derive(Clone)]
pub struct Trigger {
    /// What sets it off.
    pub on: MomentId,
    /// Where it lands, around where the moment happened.
    pub area: Area,
    /// Times it may still go off; `None` for every time.
    pub fires: Option<u32>,
    /// What it lands.
    pub effects: Arc<Effects>,
    /// What shows over the cells it lands on, if anything does.
    pub look: Option<rl_rules::ability::Look>,
}

/// Written by hand, since the effects themselves are boxed trait objects:
/// what a trigger answers, where, and how often, which is what a log or a
/// failed assertion needs.
impl std::fmt::Debug for Trigger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Trigger").field("on", &self.on).field("area", &self.area).field("fires", &self.fires).field("look", &self.look).finish_non_exhaustive()
    }
}

/// Everything a prop or a thing does at its moments.
///
/// One component for both, since a trap and a grenade differ in which
/// moments they answer and never in how the answer lands.
#[derive(Component, Clone, Debug, Default)]
pub struct Triggers(pub Vec<Trigger>);

impl Triggers {
    /// Builds a carrier's triggers from content, once per definition.
    ///
    /// A spec with no list of its own takes `shared`, which is how a thing
    /// says what it contains once and lets each trigger deliver it. Every
    /// problem is reported, not the first: an unknown moment by its name,
    /// an unregistered effect by [`Effects::build`]'s own message, and a
    /// trigger with neither a list nor a shared one as the typo it is.
    pub fn build(specs: &[TriggerSpec], shared: &[EffectSpec], moments: &Moments, kinds: &EffectKinds, names: &Names<'_>) -> Result<Self, Vec<String>> {
        let mut errors = Vec::new();
        let shared = if shared.is_empty() {
            None
        } else {
            match Effects::build(shared, kinds, names) {
                Ok(e) => Some(Arc::new(e)),
                Err(e) => {
                    errors.extend(e);
                    None
                }
            }
        };
        let mut built = Vec::new();
        for spec in specs {
            let Some(on) = moments.get(&spec.on) else {
                errors.push(format!("no moment is registered as {:?}; the engine's are {}", spec.on, Moments::BUILT_IN.join(", ")));
                continue;
            };
            let effects = match (&spec.effects, &shared) {
                (Some(own), _) => match Effects::build(own, kinds, names) {
                    Ok(e) => Arc::new(e),
                    Err(e) => {
                        errors.extend(e);
                        continue;
                    }
                },
                (None, Some(shared)) => shared.clone(),
                (None, None) => {
                    errors.push(format!("the trigger on {:?} has no effects of its own and nothing shared to deliver", spec.on));
                    continue;
                }
            };
            built.push(Trigger { on, area: spec.area, fires: spec.fires, effects, look: spec.look });
        }
        if errors.is_empty() { Ok(Self(built)) } else { Err(errors) }
    }

    /// The triggers on `moment`, in the order they were written.
    pub fn on(&self, moment: MomentId) -> impl Iterator<Item = &Trigger> {
        self.0.iter().filter(move |t| t.on == moment)
    }
}

/// Something happened to a carrier at a moment.
///
/// The one thing a subsystem that owns a moment writes. What lands is
/// [`land_triggers`]' business, and what a use costs is the consumables'.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fired {
    /// The carrier: a prop, a thing used or thrown, a weapon.
    pub on: Entity,
    /// Which moment.
    pub moment: MomentId,
    /// Who set it off, when anyone did.
    pub by: Option<Entity>,
    /// Where it happened.
    pub at: Point,
}

/// The cells `area` covers around `at`: the one cell, or the burst of that
/// radius inside the loaded map, stopped by walls, which is what an
/// ability's ball covers once it has landed on `at`, so a grenade and a
/// fireball of one radius reach the same cells.
///
/// Not the targeting call itself: that flies a projectile first and bursts
/// where it stops, and a moment has already happened where it happened, so
/// there is nothing to fly.
pub fn area_cells(area: Area, at: Point, map: &WorldMap) -> Vec<Point> {
    match area {
        Area::Here => vec![at],
        Area::Burst { radius } => rl_grid::burst(at, radius, map.window_tiles(), |p| map.blocks_projectiles(p)),
    }
}

/// A carrier whose triggers land as its own doing: a plate, a barrel.
///
/// Without it a landing's user is whoever set the moment off, which is
/// right for a thing used, thrown or fired, since the stim is the
/// drinker's and the grenade the thrower's. A trap is not the doing of
/// whoever stepped on it: credited to them, the log would say they hurt
/// themselves, and a droid a plate killed would have killed itself.
/// [`Fired::by`] still names who set it off, for a game that asks.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct LandsAsItself;

/// A carrier that exists only to land its triggers once, for something that
/// is gone by the time they land: what a broken prop leaves for its
/// `destroyed` moment, since the prop itself is despawned at the end of the
/// frame it died in.
///
/// The moment is written on the carrier rather than sent as a message,
/// because it lands a pass later and a pass may be a long time coming:
/// the loop holds while something is shown, and a message nobody reads for
/// two frames is gone, where an entity stays. [`report_remnants`] reports
/// it in the pass that lands it, and [`land_triggers`] despawns it once it
/// has.
#[derive(Component, Debug, Clone, Copy)]
pub struct Remnant {
    /// The moment it answers.
    pub moment: MomentId,
    /// Who set it off, when anyone did.
    pub by: Option<Entity>,
    /// Where it happened.
    pub at: Point,
}

/// Reports every [`Remnant`]'s moment, in the pass that lands it.
///
/// In [`ResolveSet::Triggers`](crate::plugin::ResolveSet::Triggers) ahead
/// of [`land_triggers`], so the report and the landing are one pass
/// however long the remnant waited for it.
pub fn report_remnants(remnants: Query<(Entity, &Remnant), With<Triggers>>, mut fired: MessageWriter<Fired>) {
    for (on, remnant) in &remnants {
        fired.write(Fired { on, moment: remnant.moment, by: remnant.by, at: remnant.at });
    }
}

/// Lands every carrier's triggers for each moment reported this pass.
///
/// In [`ResolveSet::Triggers`](crate::plugin::ResolveSet::Triggers), after
/// every system that reports a moment in the same pass and before fire,
/// gas, statuses and damage, so whatever a trigger starts is resolved in
/// the pass that set it off. Every actor under the footprint is a target,
/// the one who set it off included: a grenade does not ask whose it was.
/// The user is whoever set it off, or the carrier when it is
/// [`LandsAsItself`] or nobody did.
pub fn land_triggers(
    mut commands: Commands,
    mut fired: MessageReader<Fired>,
    mut carriers: Query<(&mut Triggers, Has<Remnant>, Has<LandsAsItself>)>,
    alive: Query<(), (With<crate::combat::Health>, Without<crate::combat::Dead>)>,
    mut world: EffectWorld,
) {
    for f in fired.read() {
        let Ok((mut triggers, remnant, itself)) = carriers.get_mut(f.on) else { continue };
        let user = if itself { f.on } else { f.by.unwrap_or(f.on) };
        for trigger in triggers.0.iter_mut().filter(|t| t.on == f.moment) {
            if trigger.fires == Some(0) {
                continue;
            }
            let cells = area_cells(trigger.area, f.at, world.map());
            let mut targets: Vec<Entity> = Vec::new();
            for cell in &cells {
                for who in world.occupancy().at(*cell) {
                    if alive.contains(*who) && !targets.contains(who) {
                        targets.push(*who);
                    }
                }
            }
            let landing = Landing {
                user,
                source: Source::Trigger { on: f.on, moment: f.moment },
                origin: f.at,
                aim: f.at,
                cells,
                path: Vec::new(),
                landed_at: Some(f.at),
                targets,
            };
            // Cued before any effect runs, as an ability's burst is, so a
            // cue an effect adds plays after it.
            if let Some(look) = trigger.look {
                let on = landing.cells.iter().map(|c| anchor(&landing, &world, *c)).collect();
                let from = Some(anchor(&landing, &world, f.at));
                world.cues.write(Cued { actor: user, cue: Cue::Burst { on, look: LookOf::Given(look), from } });
            }
            trigger.effects.land(&landing, &mut world);
            if let Some(left) = trigger.fires.as_mut() {
                *left -= 1;
            }
        }
        if remnant {
            commands.entity(f.on).despawn();
        }
    }
}

/// Where a cue plays on `cell`: on whoever stands there, so it follows
/// them, or on the cell.
fn anchor(landing: &Landing, world: &EffectWorld<'_, '_>, cell: Point) -> Anchor {
    landing.targets.iter().find(|t| world.position(**t) == Some(cell)).map(|t| Anchor::on(*t, cell)).unwrap_or(Anchor::cell(cell))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{CombatPlugin, DamageDealt, Health};
    use crate::components::{Actor, Blocks, Position};
    use crate::effects::AddEngineEffects;
    use crate::plugin::headless_app;
    use crate::registries::Registries;
    use crate::state::EngineState;

    /// A world with combat and the engine's effects, open land to stand on,
    /// and one damage kind, `kinetic`, as combat's own tests build it.
    fn floor() -> (App, Point) {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, crate::world::StreamingPlugin, crate::effects::EffectsPlugin));
        app.add_engine_effects();
        let start = crate::testing::surface(&mut app);
        crate::testing::two_sides(&mut app);
        // The window streams in around the player, so one stands a few
        // cells off, with no health, which no burst in these tests reaches.
        app.world_mut().spawn((Actor, crate::components::Player, Position(start.offset(0, -6))));
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        (app, start)
    }

    fn harm(roll: &str) -> Vec<rl_rules::EffectSpec> {
        ron::from_str(&format!(r#"[(kind: "Harm", args: (kind: "kinetic", roll: "{roll}"))]"#)).unwrap()
    }

    fn build(app: &App, specs: &str, shared: &[rl_rules::EffectSpec]) -> Result<Triggers, Vec<String>> {
        let specs: Vec<rl_rules::TriggerSpec> =
            ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME).from_str(specs).unwrap();
        let world = app.world();
        let registries = world.resource::<Registries>();
        Triggers::build(&specs, shared, world.resource::<Moments>(), world.resource::<EffectKinds>(), &registries.names())
    }

    /// Reports `moment` on `on` at `at` and runs one pass of the turn loop,
    /// which is where a moment is landed.
    fn fire(app: &mut App, on: Entity, moment: MomentId, at: Point) {
        app.world_mut().write_message(Fired { on, moment, by: None, at });
        app.world_mut().run_schedule(crate::plugin::Turn);
        app.update();
    }

    fn health(app: &App, e: Entity) -> i32 {
        app.world().get::<Health>(e).unwrap().current
    }

    fn stand(app: &mut App, at: Point) -> Entity {
        let e = app.world_mut().spawn((Actor, Blocks, Position(at), Health::full(10))).id();
        app.update();
        e
    }

    #[test]
    fn a_moment_nobody_registered_fails_the_build_naming_it() {
        let (app, _) = floor();
        let errs = build(&app, r#"[(on: "explode")]"#, &harm("1")).expect_err("an unknown moment refuses");
        assert!(errs.iter().any(|e| e.contains("\"explode\"")), "{errs:?}");
    }

    #[test]
    fn a_trigger_with_neither_a_list_nor_a_shared_one_fails_the_build() {
        let (app, _) = floor();
        assert!(build(&app, r#"[(on: "use")]"#, &[]).is_err(), "a trigger that does nothing is a typo");
    }

    #[test]
    fn a_trigger_with_no_list_of_its_own_lands_the_shared_one() {
        let (mut app, at) = floor();
        let triggers = build(&app, r#"[(on: "use")]"#, &harm("3")).unwrap();
        let under = stand(&mut app, at);
        let carrier = app.world_mut().spawn(triggers).id();
        fire(&mut app, carrier, Moments::USE, at);
        assert_eq!(health(&app, under), 7);
    }

    #[test]
    fn here_lands_on_the_one_cell_and_nobody_beside_it() {
        let (mut app, at) = floor();
        let triggers = build(&app, r#"[(on: "use")]"#, &harm("3")).unwrap();
        let under = stand(&mut app, at);
        let beside = stand(&mut app, at.offset(1, 0));
        let carrier = app.world_mut().spawn(triggers).id();
        fire(&mut app, carrier, Moments::USE, at);
        assert_eq!((health(&app, under), health(&app, beside)), (7, 10));
    }

    #[test]
    fn a_burst_lands_on_everyone_within_its_radius_and_nobody_past_it() {
        let (mut app, at) = floor();
        let triggers = build(&app, r#"[(on: "land", area: Burst(radius: 1))]"#, &harm("3")).unwrap();
        let near = stand(&mut app, at.offset(1, 0));
        let far = stand(&mut app, at.offset(3, 0));
        let carrier = app.world_mut().spawn(triggers).id();
        fire(&mut app, carrier, Moments::LAND, at);
        assert_eq!((health(&app, near), health(&app, far)), (7, 10));
    }

    /// Every cue written, copied out as it is written.
    #[derive(Resource, Default)]
    struct Cues(Vec<crate::cue::Cue>);

    fn keep_cues(mut cued: MessageReader<crate::cue::Cued>, mut cues: ResMut<Cues>) {
        cues.0.extend(cued.read().map(|c| c.cue.clone()));
    }

    /// A trigger with a look is seen where it lands, spreading out from
    /// there over every cell it covers, the way an ability with a look is;
    /// one with none cues
    /// nothing, as a trap nobody should see go off does not.
    #[test]
    fn a_trigger_with_a_look_bursts_over_its_cells_and_one_without_shows_nothing() {
        let (mut app, at) = floor();
        app.init_resource::<Cues>().add_systems(bevy::app::PostUpdate, keep_cues);
        let seen = build(&app, r#"[(on: "land", area: Burst(radius: 1), look: (glyph: '*', color: (r: 255, g: 128, b: 0)))]"#, &harm("3")).unwrap();
        let carrier = app.world_mut().spawn(seen).id();
        fire(&mut app, carrier, Moments::LAND, at);
        let look = rl_rules::ability::Look { glyph: '*', color: rl_grid::Rgb::new(255, 128, 0) };
        let cues = std::mem::take(&mut app.world_mut().resource_mut::<Cues>().0);
        let [crate::cue::Cue::Burst { on, look: shown, from }] = cues.as_slice() else { panic!("one burst: {cues:?}") };
        assert_eq!(*shown, crate::cue::LookOf::Given(look));
        assert_eq!(from.map(|a| a.at), Some(at), "going off where it landed");
        let mut cells: Vec<Point> = on.iter().map(|a| a.at).collect();
        let mut expected = area_cells(rl_rules::Area::Burst { radius: 1 }, at, app.world().resource::<WorldMap>());
        cells.sort();
        expected.sort();
        assert_eq!(cells, expected, "over every cell it covers");

        let unseen = build(&app, r#"[(on: "land", area: Burst(radius: 1))]"#, &harm("3")).unwrap();
        let carrier = app.world_mut().spawn(unseen).id();
        fire(&mut app, carrier, Moments::LAND, at);
        assert!(app.world().resource::<Cues>().0.is_empty(), "no look, nothing shown");
    }

    /// A wall between the burst and someone standing behind it shelters
    /// them; someone the same distance off in the open is caught.
    #[test]
    fn a_burst_stops_at_a_wall_and_catches_whoever_stands_in_the_open() {
        let (mut app, at) = floor();
        app.world_mut().resource_mut::<WorldMap>().set_tile(at.offset(1, 0), rl_grid::TileId(1));
        let triggers = build(&app, r#"[(on: "land", area: Burst(radius: 2))]"#, &harm("3")).unwrap();
        let sheltered = stand(&mut app, at.offset(2, 0));
        let open = stand(&mut app, at.offset(-2, 0));
        let carrier = app.world_mut().spawn(triggers).id();
        fire(&mut app, carrier, Moments::LAND, at);
        assert_eq!((health(&app, sheltered), health(&app, open)), (10, 7));
    }

    /// A burst reaches exactly what an ability's ball of the same radius
    /// reaches, because it is the same call.
    #[test]
    fn a_burst_covers_the_cells_an_abilitys_ball_of_that_radius_covers_once_it_lands() {
        let (app, at) = floor();
        let map = app.world().resource::<WorldMap>();
        // A real ball, thrown from two cells off and landing on `at`, so the
        // comparison cannot pass on two empty lists.
        let ball = rl_grid::footprint(
            rl_grid::TargetMode::Ball { range: 5, radius: 2 },
            at.offset(-2, 0),
            at,
            map.window_tiles(),
            |_| false,
            |p| map.blocks_projectiles(p),
        );
        assert_eq!(ball.landing, Some(at), "the ball landed where it was aimed");
        assert!(ball.cells.len() > 9, "and covers a disc, not a cell");
        let mut burst = area_cells(rl_rules::Area::Burst { radius: 2 }, at, map);
        let mut cells = ball.cells.clone();
        burst.sort();
        cells.sort();
        assert_eq!(burst, cells);
        assert_eq!(area_cells(rl_rules::Area::Here, at, map), vec![at]);
    }

    #[test]
    fn a_trigger_fires_as_often_as_it_says_per_carrier_and_not_per_definition() {
        let (mut app, at) = floor();
        let triggers = build(&app, r#"[(on: "entered", fires: 1)]"#, &harm("1")).unwrap();
        let victim = stand(&mut app, at);
        let one = app.world_mut().spawn(triggers.clone()).id();
        let two = app.world_mut().spawn(triggers).id();
        fire(&mut app, one, Moments::ENTERED, at);
        fire(&mut app, one, Moments::ENTERED, at);
        assert_eq!(health(&app, victim), 9, "the first carrier went off once");
        fire(&mut app, two, Moments::ENTERED, at);
        assert_eq!(health(&app, victim), 8, "and springing it spent nothing of the second");
    }

    #[test]
    fn a_moment_a_game_registered_lands_like_one_of_the_engines() {
        let (mut app, at) = floor();
        app.add_moment("overheat");
        let overheat = app.world().resource::<Moments>().get("overheat").unwrap();
        let triggers = build(&app, r#"[(on: "overheat")]"#, &harm("2")).unwrap();
        let wielder = stand(&mut app, at);
        let gun = app.world_mut().spawn(triggers).id();
        fire(&mut app, gun, overheat, at);
        assert_eq!(health(&app, wielder), 8);
    }

    #[test]
    fn a_moment_the_carrier_has_no_trigger_for_lands_nothing() {
        let (mut app, at) = floor();
        let triggers = build(&app, r#"[(on: "use")]"#, &harm("3")).unwrap();
        let under = stand(&mut app, at);
        let carrier = app.world_mut().spawn(triggers).id();
        fire(&mut app, carrier, Moments::LAND, at);
        assert_eq!(health(&app, under), 10);
    }

    #[test]
    fn two_triggers_on_one_moment_land_in_the_order_they_were_written() {
        let (mut app, at) = floor();
        let triggers = build(
            &app,
            r#"[(on: "use", effects: [(kind: "Harm", args: (kind: "kinetic", roll: "2"))]), (on: "use", effects: [(kind: "Harm", args: (kind: "kinetic", roll: "5"))])]"#,
            &[],
        )
        .unwrap();
        stand(&mut app, at);
        app.init_resource::<DealtLog>().add_systems(bevy::app::PostUpdate, keep_dealt);
        let carrier = app.world_mut().spawn(triggers).id();
        fire(&mut app, carrier, Moments::USE, at);
        app.update();
        assert_eq!(app.world().resource::<DealtLog>().0, vec![2, 5]);
    }

    #[derive(Resource, Default)]
    struct DealtLog(Vec<i32>);

    fn keep_dealt(mut dealt: MessageReader<DamageDealt>, mut log: ResMut<DealtLog>) {
        log.0.extend(dealt.read().map(|d| d.dealt));
    }
}
