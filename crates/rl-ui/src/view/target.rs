//! The targeting cursor: where an ability would land, before it does.
//!
//! Two pieces that only look like one, the way the look cursor is. The
//! cursor is behaviour the engine owns: it opens onto the nearest thing
//! the ability's [`Aim`] wants, steps with the direction keys, cycles
//! through the rest, refuses to leave the loaded window, and on confirm
//! writes the [`Use`] intent itself. The picture is a view: the footprint
//! the ability would cover from here and who it would hit, resolved by
//! [`Bystanders::land`], the call the resolver lands it with, so what is
//! shown is what will happen.
//!
//! The cursor is a modal, declared under the name `target`, so a game
//! gates its movement keys on [`no_modal`](crate::no_modal) and gets the
//! exclusion from the bag and the look cursor for free.
//!
//! A game opens it by writing [`AimAt`], which is all the code an ability
//! key needs: the engine picks the first target, runs the cursor, and
//! spends the turn. Nothing about which ability is bound to which key is
//! the engine's business.

use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_bevy::{Aimed, Bystanders, Landed, Offered};
use rl_core::Point;
use rl_render::Glyph;
use rl_rules::ability::{AbilityId, Aim, Blocked};

use crate::cursor::{self, CursorInput, CursorKeys, Steer};
use crate::modal::{ModalId, Modals};
use crate::view::Row;

/// The name the targeting cursor's modal is declared under.
pub const TARGET_MODAL: &str = "target";

/// Open the targeting cursor on an ability.
///
/// What a game writes when an ability key is pressed. The engine answers
/// it: opening the cursor, aiming it, and writing the [`Use`] when the
/// player confirms. An ability whose [`Aim`] needs no cursor is used at
/// once and no screen opens, so a game binds every ability the same way
/// and never asks which kind it is.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AimAt {
    /// Who is aiming.
    pub user: Entity,
    /// What they are aiming.
    pub ability: AbilityId,
}

/// What is being aimed, and where it would land.
#[derive(Resource, Debug, Default)]
pub struct TargetView {
    /// The ability being aimed, while the cursor is open.
    pub ability: Option<AbilityId>,
    /// Who is aiming it.
    pub user: Option<Entity>,
    /// The world tile the cursor is on.
    pub cursor: Point,
    /// Every cell the ability would cover from here.
    pub cells: Vec<Point>,
    /// The cells a projectile would fly through, landing included.
    pub path: Vec<Point>,
    /// Where a projectile would stop.
    pub landing: Option<Point>,
    /// Whether the resolver would accept this aim.
    pub legal: bool,
    /// Why it would not, empty when it would.
    pub why: Vec<Blocked>,
    /// Everyone under the footprint the ability's aim wants there.
    pub targets: Vec<Row>,
}

impl TargetView {
    /// Whether the cursor is up.
    pub fn aiming(&self) -> bool {
        self.ability.is_some()
    }
}

/// Adds the targeting cursor and keeps [`TargetView`] current.
///
/// Needs [`WorldMap`] and the abilities the game loaded. Nothing here runs
/// in a game that never added [`AbilitiesPlugin`], since without
/// [`Abilities`] there is nothing to aim. Its keys are [`CursorKeys`], the
/// ones the look cursor answers to.
pub struct TargetViewPlugin;

impl Plugin for TargetViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TargetView>().init_resource::<CursorKeys>().add_message::<AimAt>();
        // `Modals` is plain data, so this plugin makes sure it exists rather
        // than panicking when added before `UiPlugin`.
        app.init_resource::<Modals>().world_mut().resource_mut::<Modals>().declare(TARGET_MODAL);
        app.add_systems(Update, aim_cursor.in_set(EngineSet::Input)).add_systems(Update, collect_target.in_set(crate::ViewSet::Collect));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::UiPlugin>(app, "TargetViewPlugin");
    }
}

/// The id of the targeting modal, for a game gating its own systems.
///
/// # Panics
/// Panics if [`TargetViewPlugin`] was not added.
pub fn target_modal(modals: &Modals) -> ModalId {
    modals.get(TARGET_MODAL).expect("TargetViewPlugin declares the target modal")
}

/// Everything the cursor steers by.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Aiming<'w, 's> {
    input: CursorInput<'w>,
    map: Res<'w, WorldMap>,
    abilities: Option<Res<'w, Abilities>>,
    occupancy: Res<'w, Occupancy>,
    bystanders: Bystanders<'w, 's>,
    users: Query<'w, 's, (&'static Position, Option<&'static Viewshed>)>,
    living: Query<'w, 's, &'static Health, Without<Dead>>,
}

impl Aiming<'_, '_> {
    /// The cells this ability is worth pointing at, nearest to the user
    /// first.
    ///
    /// [`Aim::worth_aiming_at`] is the rule the mind's tactic aims by, so
    /// the cursor opens on what a monster would have picked, the user's own
    /// cell included when there is a hurt user to mend, and a player who
    /// just presses confirm gets the sensible shot.
    fn candidates(&self, user: Entity, from: Point, aim: Aim, sight: Option<&Viewshed>) -> Vec<Point> {
        if !aim.needs_cursor() {
            return vec![from];
        }
        let wanted = self.occupancy.iter().filter(|(p, who)| {
            let Ok(health) = self.living.get(*who) else { return false };
            let seen = *who == user || sight.is_none_or(|s| s.can_see(*p));
            seen && aim.worth_aiming_at(self.bystanders.relation(user, *who), *who == user, health.hp < health.max)
        });
        cursor::ordered(from, wanted.map(|(p, _)| p))
    }
}

/// Opens, steps, cycles, confirms and cancels the cursor.
pub fn aim_cursor(
    mut view: ResMut<TargetView>,
    mut modals: ResMut<Modals>,
    mut requests: MessageReader<AimAt>,
    mut uses: MessageWriter<Intent<Use>>,
    aiming: Aiming,
) {
    let modal = target_modal(&modals);
    let Some(abilities) = aiming.abilities.as_deref() else { return };

    for request in requests.read() {
        let Ok((from, sight)) = aiming.users.get(request.user) else { continue };
        let def = abilities.get(request.ability);
        // An ability that wants no cursor is used where it stands. A game
        // binds every ability the same way and never asks which kind it is.
        if !def.aim.needs_cursor() {
            uses.write(Intent::new(request.user, Use { ability: request.ability, aim: from.0 }));
            continue;
        }
        let candidates = aiming.candidates(request.user, from.0, def.aim, sight);
        view.ability = Some(request.ability);
        view.user = Some(request.user);
        view.cursor = candidates.first().copied().unwrap_or(from.0);
        modals.open(modal);
    }

    if !modals.is_top(modal) {
        return;
    }
    let (Some(ability), Some(user)) = (view.ability, view.user) else {
        // The cursor is up with nothing in it, which can only mean the
        // aimer is gone. Put it away rather than leave input trapped.
        modals.close_one(modal);
        return;
    };
    let Ok((from, sight)) = aiming.users.get(user) else {
        // The aimer left the world: there is nothing to aim from.
        close(&mut view, &mut modals, modal);
        return;
    };
    let aim = abilities.get(ability).aim;
    match aiming.input.steer(&mut view.cursor, aiming.map.window_tiles(), || aiming.candidates(user, from.0, aim, sight)) {
        Steer::Close => close(&mut view, &mut modals, modal),
        Steer::Confirm => {
            // Refused aims are the resolver's to report, not the cursor's: a
            // player who insists gets the refusal in the log with its reason,
            // which is better than a key that does nothing.
            uses.write(Intent::new(user, Use { ability, aim: view.cursor }));
            close(&mut view, &mut modals, modal);
        }
        Steer::Moved | Steer::Stay => {}
    }
}

/// Puts the cursor away and forgets what it was aiming.
fn close(view: &mut TargetView, modals: &mut Modals, modal: ModalId) {
    view.ability = None;
    view.user = None;
    view.cells.clear();
    view.path.clear();
    view.targets.clear();
    view.why.clear();
    modals.close_one(modal);
}

/// Anything the cursor might land on, as a row is built from it.
///
/// The name and the glyph are optional here and required by the other
/// views, and the difference matters: a nameless or glyphless row is one a
/// list can leave out, but such a target is one the ability will hit
/// anyway. Dropping it would make the banner say "nothing" over a monster
/// about to be burned, so `targets` counts what the footprint catches and
/// the cosmetics are filled in where they exist.
type Standing = (Entity, &'static Position, Option<&'static Name>, Option<&'static Glyph>, Option<&'static Health>);

/// What the footprint is resolved against.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Reach<'w, 's> {
    map: Res<'w, WorldMap>,
    occupancy: Res<'w, Occupancy>,
    abilities: Option<Res<'w, Abilities>>,
    offered: Option<Res<'w, Offered>>,
    bystanders: Bystanders<'w, 's>,
    users: Query<'w, 's, (&'static Position, Option<&'static Viewshed>)>,
    subjects: Query<'w, 's, Standing>,
}

/// Fills [`TargetView`] from wherever the cursor is.
///
/// Through [`Bystanders::land`], the call the resolver lands the use with,
/// so the cells lit on the map, the names in the banner and whether it
/// reads as refused are what will happen when the player confirms. A
/// preview computed any other way is a preview that drifts.
pub fn collect_target(mut view: ResMut<TargetView>, reach: Reach) {
    view.cells.clear();
    view.path.clear();
    view.targets.clear();
    view.why.clear();
    view.landing = None;
    view.legal = false;
    let (Some(ability), Some(user), Some(abilities)) = (view.ability, view.user, reach.abilities.as_deref()) else { return };
    let Ok((from, sight)) = reach.users.get(user) else { return };
    let def = abilities.get(ability);
    let aimed = Aimed { user, ability, def, origin: from.0, aim: view.cursor, sees_aim: sight.map(|s| s.can_see(view.cursor)) };
    let Landed { landing, refused } = reach.bystanders.land(aimed, &reach.map, &reach.occupancy);

    // Every reason the resolver would refuse, before the turn is spent
    // rather than after: what the gate said about the ability, then what
    // this particular aim adds.
    if let Some(offered) = reach.offered.as_deref() {
        view.why.extend(offered.why_for(user, ability).iter().copied());
    }
    view.why.extend(refused);
    view.legal = view.why.is_empty();

    for who in &landing.targets {
        let Ok((entity, at, name, glyph, health)) = reach.subjects.get(*who) else { continue };
        let label = name.map(|n| n.as_str().to_string()).unwrap_or_default();
        // A blank glyph draws nothing and invents no colour, the same
        // choice as the empty label above.
        let glyph = glyph.copied().unwrap_or_else(|| Glyph::new(' ', Color::WHITE));
        let mut row = Row::new(entity, label, glyph).at(rl_core::geometry::chebyshev(from.0, at.0));
        row.health = health.map(|h| (h.hp, h.max));
        row.relation = reach.bystanders.relation(user, entity);
        view.targets.push(row);
    }
    view.cells = landing.cells;
    view.path = landing.path;
    view.landing = landing.landed_at;
}

#[cfg(test)]
pub(crate) mod harness {
    use super::*;
    use rl_rules::content::Registry;
    use rl_rules::{DamageKind, Names, StatDef};

    /// The registries an ability file names, over the one damage kind the
    /// stage has.
    pub struct Content {
        pub stats: Registry<StatDef>,
        pub kinds: Registry<DamageKind>,
    }

    impl Content {
        pub fn new() -> Self {
            Self { stats: Registry::from_defs(vec![StatDef::new("focus", 20)]).unwrap(), kinds: Registry::from_defs(vec![DamageKind::new("kinetic")]).unwrap() }
        }

        fn names(&self) -> Names<'_> {
            Names::new().stats(&self.stats).damage_kinds(&self.kinds)
        }
    }

    /// Two abilities: one that flies and bursts, one that wants no cursor.
    pub const ABILITIES: &str = r#"[
        (name: "bolt", mode: Bolt(range: 6), costs: [Pool("focus", 5)], effects: [(kind: "Harm", args: (kind: "kinetic", roll: "3"))]),
        (name: "burst", aim: Ground, mode: Ball(range: 6, radius: 1), effects: [(kind: "Harm", args: (kind: "kinetic", roll: "2"))]),
        (name: "steel", aim: SelfOnly, mode: Own, effects: []),
        (name: "dear", mode: Bolt(range: 6), costs: [Pool("focus", 99)], effects: []),
        (name: "salve", aim: Ally, mode: Ball(range: 6, radius: 1), effects: [(kind: "Mend", args: (kind: "kinetic", roll: "2"))]),
    ]"#;

    /// Inserts everything an ability needs into `app`, before play begins.
    pub fn abilities(app: &mut App) {
        let content = Content::new();
        let built = Abilities::load(ABILITIES, app.world().resource::<EffectKinds>(), &content.names()).expect("the abilities load and build");
        app.insert_resource(built);
        app.insert_resource(AbilityRng::for_run(rl_core::RunSeed(3)));
        app.insert_resource(StatRules(content.stats));
    }

    /// Gives the player the abilities and something to spend on them.
    pub fn arm(stage: &mut crate::harness::Stage) -> (AbilityId, AbilityId, AbilityId) {
        let (bolt, burst, steel) = {
            let a = stage.app.world().resource::<Abilities>();
            (a.expect("bolt"), a.expect("burst"), a.expect("steel"))
        };
        let dear = stage.app.world().resource::<Abilities>().expect("dear");
        let mut pools = Pools::new();
        pools.set(rl_rules::StatId::from_raw(0), 20);
        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert((Grants(vec![bolt, burst, steel, dear]), pools));
        stage.tick();
        stage.tick();
        (bolt, burst, steel)
    }
}

#[cfg(test)]
mod tests {
    use super::harness::{abilities, arm};
    use super::*;
    use crate::harness::Stage;
    use rl_bevy::AddEngineEffects;

    fn staged() -> Stage {
        let mut stage = Stage::new_with((AbilitiesPlugin, TargetViewPlugin), |app| {
            app.add_engine_effects();
            abilities(app);
        });
        stage.tick();
        stage
    }

    fn intents(stage: &mut Stage) -> Vec<Use> {
        stage.app.world_mut().resource_mut::<Messages<Intent<Use>>>().drain().map(|i| i.action).collect()
    }

    fn aim_at(stage: &mut Stage, ability: AbilityId) {
        let user = stage.player;
        stage.app.world_mut().write_message(AimAt { user, ability });
        stage.tick();
    }

    /// The cursor opens on the nearest thing the ability wants, which is
    /// the same choice a mind's tactic would have made.
    #[test]
    fn the_cursor_opens_on_the_nearest_thing_the_aim_wants() {
        let mut stage = staged();
        let (bolt, _, _) = arm(&mut stage);
        stage.actor("far one", 'f', 4, 0);
        stage.actor("near one", 'n', 2, 0);
        stage.tick();

        aim_at(&mut stage, bolt);
        let at = stage.at;
        let view = stage.app.world().resource::<TargetView>();
        assert_eq!(view.cursor, at.offset(2, 0), "the nearer of the two");
        assert!(view.aiming());
        assert!(stage.app.world().resource::<Modals>().any_open(), "and it holds input");
    }

    /// Cycling walks what the ability wants, and a direction key steps
    /// off it a cell at a time.
    #[test]
    fn tab_cycles_the_targets_and_a_direction_key_steps() {
        let mut stage = staged();
        let (bolt, _, _) = arm(&mut stage);
        stage.actor("far one", 'f', 4, 0);
        stage.actor("near one", 'n', 2, 0);
        stage.tick();
        aim_at(&mut stage, bolt);
        let at = stage.at;

        stage.press(KeyCode::Tab);
        assert_eq!(stage.app.world().resource::<TargetView>().cursor, at.offset(4, 0));
        stage.press(KeyCode::Tab);
        assert_eq!(stage.app.world().resource::<TargetView>().cursor, at.offset(2, 0), "round the end");
        stage.press(KeyCode::ArrowUp);
        assert_eq!(stage.app.world().resource::<TargetView>().cursor, at.offset(2, -1), "stepped off the target");
    }

    /// What the cursor shows is what the resolver will do: the same
    /// footprint call, so a burst covers the disc it will burst into.
    #[test]
    fn the_footprint_shown_is_the_footprint_that_will_land() {
        let mut stage = staged();
        let (_, burst, _) = arm(&mut stage);
        stage.actor("them", 't', 3, 0);
        stage.tick();
        aim_at(&mut stage, burst);
        let at = stage.at;

        let view = stage.app.world().resource::<TargetView>();
        let burst_at = at.offset(3, 0);
        assert_eq!(view.cursor, burst_at);
        // A disc of one is the five-cell plus the geometry draws, and the
        // point of resolving it here is that this is that exact call.
        let expected = rl_core::geometry::disc(burst_at, 1).collect::<Vec<_>>();
        let mut got = view.cells.clone();
        got.sort_by_key(|p| (p.y, p.x));
        let mut want = expected;
        want.sort_by_key(|p| (p.y, p.x));
        assert_eq!(got, want, "the shape the resolver will burst into");
        assert_eq!(view.path.last(), Some(&burst_at), "the flight ends where it bursts");
        assert_eq!(view.targets.len(), 1);
        assert_eq!(view.targets[0].label, "them");
        assert!(view.legal);
    }

    /// An aim the resolver would refuse says so before the turn is spent,
    /// and says why.
    #[test]
    fn an_aim_that_would_be_refused_reads_as_illegal_with_its_reason() {
        let mut stage = staged();
        let (bolt, _, _) = arm(&mut stage);
        stage.actor("them", 't', 2, 0);
        stage.tick();

        // Out of the ability's range: the bolt lands short, so nothing is
        // under the cursor.
        aim_at(&mut stage, bolt);
        let at = stage.at;
        stage.app.world_mut().resource_mut::<TargetView>().cursor = at.offset(2, -3);
        stage.tick();
        let view = stage.app.world().resource::<TargetView>();
        assert!(view.legal, "an empty cell in reach is still a legal aim for a bolt");

        // The ability nobody can pay for: refused before it is pointed
        // anywhere, from the same gate the resolver runs.
        let dear = stage.app.world().resource::<Abilities>().expect("dear");
        aim_at(&mut stage, dear);
        stage.tick();
        let view = stage.app.world().resource::<TargetView>();
        assert!(!view.legal, "it cannot be paid for");
        assert!(view.why.iter().any(|w| matches!(w, Blocked::Cannot(_))), "{:?}", view.why);
    }

    /// The property the preview exists for, over many layouts: who the
    /// banner lists and whether it reads as refused are exactly what the
    /// resolver does once the player confirms.
    #[test]
    fn the_preview_names_exactly_who_the_resolver_hits() {
        use rl_core::seed::position_hash;
        for seed in 0..12u64 {
            // A small deterministic stream: one hash per draw.
            let mut draw = 0;
            let mut roll = |n: i32| {
                draw += 1;
                (position_hash(seed, draw, n) % n as u64) as i32
            };
            let mut stage = staged();
            let (bolt, burst, _) = arm(&mut stage);
            let salve = stage.app.world().resource::<Abilities>().expect("salve");
            let player = stage.player;
            stage.app.world_mut().get_mut::<Grants>(player).expect("grants").0.push(salve);
            if roll(2) == 0 {
                stage.app.world_mut().get_mut::<Health>(player).expect("health").hp = 12;
            }
            let mut taken = vec![(0, 0)];
            for i in 0..6 {
                let (dx, dy) = loop {
                    let at = (roll(9) - 4, roll(9) - 4);
                    if !taken.contains(&at) {
                        break at;
                    }
                };
                taken.push((dx, dy));
                let who = stage.actor(&format!("them {i}"), 't', dx, dy);
                if roll(2) == 0 {
                    stage.app.world_mut().entity_mut(who).insert(Faction(rl_rules::FactionId::from_raw(0)));
                }
                if roll(2) == 0 {
                    stage.app.world_mut().get_mut::<Health>(who).expect("health").hp = 4;
                }
            }
            stage.tick();
            stage.tick();

            for _ in 0..6 {
                let ability = [bolt, burst, salve][roll(3) as usize];
                let aim = stage.at.offset(roll(13) - 6, roll(13) - 6);
                let user = stage.player;
                stage.app.world_mut().write_message(AimAt { user, ability });
                stage.tick();
                stage.app.world_mut().resource_mut::<TargetView>().cursor = aim;
                stage.tick();
                let view = stage.app.world().resource::<TargetView>();
                assert!(view.aiming(), "seed {seed}: the cursor is up");
                let legal = view.legal;
                let mut shown: Vec<Entity> = view.targets.iter().map(|r| r.entity).collect();
                shown.sort();

                let _ = stage.app.world_mut().resource_mut::<Messages<AbilityEvent>>().drain().count();
                stage.press(KeyCode::Enter);
                let outcomes: Vec<AbilityEvent> = stage.app.world_mut().resource_mut::<Messages<AbilityEvent>>().drain().collect();
                match outcomes.as_slice() {
                    [AbilityEvent::Used { targets, .. }] => {
                        let mut hit = targets.clone();
                        hit.sort();
                        assert!(legal, "seed {seed}: the preview refused an aim the resolver took");
                        assert_eq!(hit, shown, "seed {seed}: the banner listed other than who was hit");
                    }
                    [AbilityEvent::Refused { why, .. }] => assert!(!legal, "seed {seed}: the resolver refused ({why:?}) an aim the preview called legal"),
                    other => panic!("seed {seed}: expected one outcome, got {other:?}"),
                }
            }
        }
    }

    /// Confirming spends the turn on the aim; cancelling spends nothing.
    #[test]
    fn confirm_writes_the_use_and_cancel_writes_nothing() {
        let mut stage = staged();
        let (bolt, _, _) = arm(&mut stage);
        stage.actor("them", 't', 2, 0);
        stage.tick();
        let at = stage.at;

        aim_at(&mut stage, bolt);
        let _ = intents(&mut stage);
        stage.press(KeyCode::Escape);
        assert!(intents(&mut stage).is_empty(), "cancelled");
        assert!(!stage.app.world().resource::<TargetView>().aiming());
        assert!(!stage.app.world().resource::<Modals>().any_open(), "and input came back");

        aim_at(&mut stage, bolt);
        let _ = intents(&mut stage);
        stage.press(KeyCode::Enter);
        assert_eq!(intents(&mut stage), vec![Use { ability: bolt, aim: at.offset(2, 0) }]);
        assert!(!stage.app.world().resource::<TargetView>().aiming(), "and the cursor went away");
    }

    /// An ability that wants no cursor is used where it stands, so a game
    /// binds every ability the same way.
    #[test]
    fn an_ability_that_needs_no_cursor_opens_no_screen() {
        let mut stage = staged();
        let (_, _, steel) = arm(&mut stage);
        let at = stage.at;

        aim_at(&mut stage, steel);
        assert_eq!(intents(&mut stage), vec![Use { ability: steel, aim: at }]);
        assert!(!stage.app.world().resource::<Modals>().any_open(), "nothing opened");
        assert!(!stage.app.world().resource::<TargetView>().aiming());
    }

    /// Every ability known, in a stable order, with the reasons the gate
    /// gave for the ones that cannot be used.
    #[test]
    fn the_menu_lists_what_is_known_and_why_it_cannot_be_used() {
        use crate::view::ability::{AbilityView, AbilityViewPlugin};
        let mut stage = Stage::new_with((AbilitiesPlugin, AbilityViewPlugin), |app| {
            app.add_engine_effects();
            abilities(app);
        });
        stage.tick();
        arm(&mut stage);
        stage.tick();

        let view = stage.app.world().resource::<AbilityView>();
        let names: Vec<&str> = view.rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(names, vec!["bolt", "burst", "steel", "dear"], "registration order, so a bound key stays bound");
        assert_eq!(view.ready(), 3);
        let dear = view.rows.iter().find(|r| r.label == "dear").expect("the dear one");
        assert!(!dear.ready());
        assert!(matches!(dear.blocked.first(), Some(Blocked::Cannot(_))), "{:?}", dear.blocked);
    }
}
