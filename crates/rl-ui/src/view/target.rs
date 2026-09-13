//! The targeting cursor: where an ability would land, before it does.
//!
//! Two pieces that only look like one, the way the look cursor is. The
//! cursor is behaviour the engine owns: it opens onto the nearest thing
//! the ability's [`Aim`] wants, steps with the direction keys, cycles
//! through the rest, refuses to leave the loaded window, and on confirm
//! writes the [`Use`] intent itself. The picture is a view: the footprint
//! the ability would cover from here, resolved by the same
//! [`footprint`](rl_grid::footprint) the resolver will use, so what is
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
use rl_core::Point;
use rl_grid::{Footprint, footprint};
use rl_render::Glyph;
use rl_rules::ability::{AbilityId, Aim, Blocked};

use crate::cursor;
use crate::keys::DirectionKeys;
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

/// Keys the targeting cursor answers to.
#[derive(Resource, Debug, Clone)]
pub struct TargetKeys {
    /// Spends the turn on the aim.
    pub confirm: KeyCode,
    /// And so does this, because a player reaching for one reaches for
    /// the other.
    pub also_confirm: KeyCode,
    /// Puts the cursor away, spending nothing.
    pub cancel: KeyCode,
    /// Jumps to the next thing the ability wants.
    pub next: KeyCode,
}

impl Default for TargetKeys {
    fn default() -> Self {
        Self { confirm: KeyCode::Enter, also_confirm: KeyCode::Space, cancel: KeyCode::Escape, next: KeyCode::Tab }
    }
}

/// Adds the targeting cursor and keeps [`TargetView`] current.
///
/// Needs [`WorldMap`] and the abilities the game loaded. Nothing here runs
/// in a game that never added [`AbilitiesPlugin`], since without
/// [`Abilities`] there is nothing to aim.
pub struct TargetViewPlugin;

impl Plugin for TargetViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TargetView>().init_resource::<TargetKeys>().add_message::<AimAt>();
        app.world_mut().resource_mut::<Modals>().declare(TARGET_MODAL);
        app.add_systems(OnEnter(EngineState::Playing), rl_bevy::needs::<WorldMap>("TargetViewPlugin"))
            .add_systems(Update, aim_cursor.in_set(EngineSet::Input))
            .add_systems(Update, collect_target.in_set(crate::ViewSet::Collect));
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
    keys: Res<'w, ButtonInput<KeyCode>>,
    binds: Res<'w, TargetKeys>,
    steps: Res<'w, DirectionKeys>,
    map: Res<'w, WorldMap>,
    abilities: Option<Res<'w, Abilities>>,
    occupancy: Res<'w, Occupancy>,
    rules: Option<Res<'w, CombatRules>>,
    users: Query<'w, 's, (&'static Position, Option<&'static Viewshed>, Option<&'static Faction>)>,
    others: Query<'w, 's, &'static Faction, Without<Dead>>,
}

impl Aiming<'_, '_> {
    /// The cells this ability's aim wants, nearest to the user first.
    ///
    /// The same question the mind's tactic asks, so the cursor opens on
    /// what a monster would have picked and a player who just presses
    /// confirm gets the sensible shot.
    fn candidates(&self, user: Entity, from: Point, aim: Aim, sight: Option<&Viewshed>) -> Vec<Point> {
        if !aim.needs_cursor() {
            return vec![from];
        }
        let mine = self.users.get(user).ok().and_then(|(_, _, f)| f).map(|f| f.0);
        let wanted = self.occupancy.iter().filter(|(p, who)| {
            if *who == user || !sight.is_none_or(|s| s.can_see(*p)) {
                return false;
            }
            let relation = match (&self.rules, mine, self.others.get(*who).ok()) {
                (Some(rules), Some(mine), Some(theirs)) => Some(rules.factions.relation(mine, theirs.0)),
                _ => None,
            };
            aim.wants(relation)
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
        let Ok((from, sight, _)) = aiming.users.get(request.user) else { continue };
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
    if aiming.keys.just_pressed(aiming.binds.cancel) {
        close(&mut view, &mut modals, modal);
        return;
    }
    if aiming.keys.just_pressed(aiming.binds.confirm) || aiming.keys.just_pressed(aiming.binds.also_confirm) {
        // Refused aims are the resolver's to report, not the cursor's: a
        // player who insists gets the refusal in the log with its reason,
        // which is better than a key that does nothing.
        uses.write(Intent::new(user, Use { ability, aim: view.cursor }));
        close(&mut view, &mut modals, modal);
        return;
    }
    let Ok((from, sight, _)) = aiming.users.get(user) else {
        close(&mut view, &mut modals, modal);
        return;
    };
    if aiming.keys.just_pressed(aiming.binds.next) {
        let candidates = aiming.candidates(user, from.0, abilities.get(ability).aim, sight);
        if let Some(next) = cursor::next_of(&candidates, view.cursor) {
            view.cursor = next;
        }
        return;
    }
    if let Some(step) = aiming.steps.just_pressed(&aiming.keys) {
        view.cursor = cursor::stepped(view.cursor, step, aiming.map.window_tiles());
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
type Standing = (Entity, Option<&'static Name>, Option<&'static Glyph>, Option<&'static Health>, Option<&'static Faction>);

/// What the footprint is resolved against.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Reach<'w, 's> {
    map: Res<'w, WorldMap>,
    occupancy: Res<'w, Occupancy>,
    abilities: Option<Res<'w, Abilities>>,
    offered: Option<Res<'w, Offered>>,
    rules: Option<Res<'w, CombatRules>>,
    users: Query<'w, 's, (&'static Position, Option<&'static Viewshed>, Option<&'static Faction>)>,
    subjects: Query<'w, 's, Standing, Without<Dead>>,
}

/// Fills [`TargetView`] from wherever the cursor is.
///
/// The footprint comes from the same call the resolver will make, with the
/// same blocker, so the cells lit on the map are the cells that will be
/// hit. A preview computed any other way is a preview that drifts.
pub fn collect_target(mut view: ResMut<TargetView>, reach: Reach) {
    view.cells.clear();
    view.path.clear();
    view.targets.clear();
    view.why.clear();
    view.landing = None;
    view.legal = false;
    let (Some(ability), Some(user), Some(abilities)) = (view.ability, view.user, reach.abilities.as_deref()) else { return };
    let Ok((from, sight, mine)) = reach.users.get(user) else { return };
    let def = abilities.get(ability);

    let stops = |p: Point| p != from.0 && (reach.map.blocks_projectiles(p) || reach.occupancy.is_occupied(p));
    let Footprint { cells, path, landing } = footprint(def.mode, from.0, view.cursor, reach.map.window_tiles(), stops);

    // Every reason the resolver would refuse, worked out before the turn
    // is spent rather than after: what the gate said about the ability,
    // then what this particular aim adds.
    if let Some(offered) = reach.offered.as_deref()
        && offered.actor == Some(user)
        && let Some((_, why)) = offered.refused.iter().find(|(id, _)| *id == ability)
    {
        view.why.extend(why.iter().copied());
    }
    if def.sight && sight.is_some_and(|s| !s.can_see(view.cursor)) {
        view.why.push(Blocked::NoTarget);
    }
    if cells.is_empty() {
        view.why.push(Blocked::NoTarget);
    }
    view.legal = view.why.is_empty();

    for cell in &cells {
        for who in reach.occupancy.at(*cell) {
            let Ok((entity, name, glyph, health, theirs)) = reach.subjects.get(*who) else { continue };
            if entity == user && def.aim != Aim::SelfOnly {
                continue;
            }
            let relation = match (&reach.rules, mine, theirs) {
                (Some(rules), Some(mine), Some(theirs)) => Some(rules.factions.relation(mine.0, theirs.0)),
                _ => None,
            };
            if def.aim != Aim::SelfOnly && !def.aim.wants(relation) {
                continue;
            }
            let label = name.map(|n| n.as_str().to_string()).unwrap_or_default();
            // A blank glyph draws nothing and invents no colour, the same
            // choice as the empty label above.
            let glyph = glyph.copied().unwrap_or_else(|| Glyph::new(' ', Color::WHITE));
            let mut row = Row::new(entity, label, glyph).at(rl_core::geometry::chebyshev(from.0, *cell));
            row.health = health.map(|h| (h.hp, h.max));
            row.relation = relation;
            view.targets.push(row);
        }
    }
    view.cells = cells;
    view.path = path;
    view.landing = landing;
}

#[cfg(test)]
pub(crate) mod harness {
    use super::*;
    use rl_rules::ability::Lookup;
    use rl_rules::content::Registry;
    use rl_rules::{DamageKind, StatDef};

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
    }

    impl Lookup for Content {
        fn stat(&self, n: &str) -> Option<rl_rules::StatId> {
            self.stats.id(n)
        }
        fn status(&self, _: &str) -> Option<rl_rules::StatusId> {
            None
        }
        fn tag(&self, _: &str) -> Option<rl_rules::TagId> {
            None
        }
        fn slot(&self, _: &str) -> Option<rl_rules::SlotId> {
            None
        }
        fn damage(&self, n: &str) -> Option<rl_rules::damage::DamageKindId> {
            self.kinds.id(n)
        }
    }

    /// Two abilities: one that flies and bursts, one that wants no cursor.
    pub const ABILITIES: &str = r#"[
        (name: "bolt", mode: Bolt(range: 6), costs: [Pool("focus", 5)], effects: [(kind: "Harm", args: (kind: "kinetic", roll: "3"))]),
        (name: "burst", aim: Ground, mode: Ball(range: 6, radius: 1), effects: [(kind: "Harm", args: (kind: "kinetic", roll: "2"))]),
        (name: "steel", aim: SelfOnly, mode: Own, effects: []),
        (name: "dear", mode: Bolt(range: 6), costs: [Pool("focus", 99)], effects: []),
    ]"#;

    /// Inserts everything an ability needs into `app`, before play begins.
    pub fn abilities(app: &mut App) {
        let content = Content::new();
        let defs = rl_rules::ability::load(ABILITIES, &content).expect("the abilities load");
        let kinds = app.world().resource::<EffectKinds>();
        let built = Abilities::build(defs, kinds, &content).expect("the effects build");
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
        stage.app.world_mut().entity_mut(player).insert((Grants(vec![bolt, burst, steel, dear]), Known::new(), pools, Cooldowns::new()));
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
