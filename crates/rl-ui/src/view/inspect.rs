//! The look cursor, and the truthful picture of whatever it is over.
//!
//! Two pieces that only look like one. The cursor is behaviour the engine
//! owns: it opens onto the nearest thing worth looking at, steps with the
//! direction keys, cycles through what is in sight, and stays inside the
//! loaded window. The picture is a view: the subject's name, glyph and
//! health, and a forecast of the fight, run through
//! [`rl_rules::forecast`] so the numbers on the panel are the numbers the
//! damage pipeline will produce.
//!
//! The cursor is a modal, declared under the name `inspect`, so a game
//! gates its own movement keys on [`no_modal`](crate::no_modal) and gets
//! the exclusion for free.

use crate::controls::AddControls;
use crate::modal::AddModal;
use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_core::{Point, geometry};
use rl_render::Glyph;
use rl_rules::Relation;
use rl_rules::damage::DamageKindId;
use rl_rules::forecast::{Combatant, Duel, duel};

use crate::cursor::{CursorInput, CursorKeys, Steer};
use crate::facet::Facet;
use crate::focus::{Focus, InSight};
use crate::modal::{ModalId, Modals};
use crate::view::Row;

/// The name the inspect cursor's modal is declared under.
pub const INSPECT_MODAL: &str = "inspect";

/// Where the cursor is and what is under it.
#[derive(Resource, Debug, Default)]
pub struct InspectView {
    /// The world tile the cursor is on.
    pub cursor: Point,
    /// The topmost entity under the cursor, if any.
    pub subject: Option<Row>,
    /// How a fight with the subject is likely to go, when both sides can
    /// be read as combatants.
    pub duel: Option<Duel>,
    /// What the game added about the subject.
    pub facets: Vec<Facet>,
}

impl InspectView {
    /// Whether the cursor is over something that can be described.
    pub fn has_subject(&self) -> bool {
        self.subject.is_some()
    }
}

/// Adds the look cursor and keeps [`InspectView`] current.
///
/// Needs [`WorldMap`] for the window the cursor moves in, and
/// [`Registries`] for the damage kinds the forecast resolves through. Its
/// keys are [`CursorKeys`], the ones the targeting cursor answers to.
pub struct InspectViewPlugin;

impl Plugin for InspectViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InspectView>().init_resource::<CursorKeys>().init_resource::<Focus>();
        // `Modals` is plain data, so this plugin makes sure it exists rather
        // than panicking when added before `UiPlugin`.
        app.add_modal(INSPECT_MODAL);
        app.needs::<Registries>("InspectViewPlugin", "`Registries`, with the damage kinds the forecast resolves through")
            .needs::<CombatRules>("InspectViewPlugin", "`CombatRules { factions }`, for how the subject stands to the player")
            .add_systems(Update, move_cursor.in_set(EngineSet::Input))
            .add_systems(Update, collect_inspect.in_set(crate::ViewSet::Collect));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::UiPlugin>(app, "InspectViewPlugin");
        // Declared once the game has declared its own, so the controls
        // screen lists the game's groups first whatever order the plugins
        // were added in.
        app.add_control(crate::focus::CURSOR_GROUP, "look around", crate::controls::EngineKey::Look);
        crate::focus::declare_cursor_controls(app);
    }
}

/// The id of the inspect modal, for a game gating its own systems.
///
/// # Panics
/// Panics if [`InspectViewPlugin`] was not added.
pub fn inspect_modal(modals: &Modals) -> ModalId {
    modals.get(INSPECT_MODAL).expect("InspectViewPlugin declares the inspect modal")
}

/// Everything the panel names the thing under the cursor by.
type Subject = (Entity, &'static Position, &'static Name, &'static Glyph, Option<&'static OnMap>);
/// Anything but the player: the cursor describes what you are looking
/// at, and looking at yourself would forecast a duel with yourself.
/// What the player is is the vitals panel's question.
type NotYou = (Without<Dead>, Without<Player>);

/// Everything the cursor steers by.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Look<'w, 's> {
    input: CursorInput<'w>,
    map: Res<'w, WorldMap>,
    sight: InSight<'w, 's>,
    focus: ResMut<'w, Focus>,
}

/// Opens, closes, steps and cycles the cursor.
///
/// It opens on what is picked out, while that is still in sight, and on the
/// top of the nearby list otherwise: the nearest actor, or the nearest thing
/// when nobody is about. Cycling walks that same list in the same order,
/// things as well as actors, and the [`Focus`] moves with the cursor.
pub fn move_cursor(mut view: ResMut<InspectView>, mut modals: ResMut<Modals>, mut look: Look) {
    let modal = inspect_modal(&modals);
    let Some((_, origin)) = look.sight.viewer() else { return };

    let toggled = look.input.just_pressed(look.input.keys().look);
    if toggled && !modals.any_open() {
        modals.open(modal);
        let list = look.sight.list();
        let opens_on = look.focus.within(&list).or(list.first()).copied();
        view.cursor = opens_on.map_or(origin, |s| s.at);
        look.focus.set(opens_on.map(|s| s.entity));
        return;
    }
    if !modals.is_top(modal) {
        return;
    }
    if toggled {
        modals.close_one(modal);
        return;
    }
    // Steered on a copy, so the list can be borrowed while the focus moves,
    // and written back only when it did.
    let mut focus = *look.focus;
    let steer = look.input.steer(&mut view.cursor, &mut focus, look.map.window_tiles(), || look.sight.list());
    if focus != *look.focus {
        *look.focus = focus;
    }
    match steer {
        Steer::Close => modals.close_one(modal),
        // Looking spends nothing, so there is nothing to confirm.
        Steer::Confirm | Steer::Moved | Steer::Stay => {}
    }
}

/// Everything the forecast reads.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Duelists<'w, 's> {
    rules: Res<'w, CombatRules>,
    registries: Res<'w, Registries>,
    stages: Res<'w, DamageStages>,
    modals: Res<'w, Modals>,
    map: Res<'w, WorldMap>,
    focus: Res<'w, Focus>,
    player: Query<'w, 's, (&'static Position, Fighter, Option<&'static Faction>), With<Player>>,
    subjects: Query<'w, 's, Subject, NotYou>,
    fighters: Query<'w, 's, (Fighter, Option<&'static Faction>)>,
}

/// What a side of a duel is made of.
type Fighter =
    (Option<&'static Health>, Option<&'static Armor>, Option<&'static Speed>, Option<&'static Resists>, Option<&'static MeleeAttack>, Option<&'static Strikes>);

/// Every roll one blow lands, the main one first.
fn strikes_of(melee: Option<&MeleeAttack>, extra: Option<&Strikes>) -> Vec<(DamageKindId, rl_core::DiceRoll)> {
    let mut all: Vec<(DamageKindId, rl_core::DiceRoll)> = melee.map(|m| (m.kind, m.dice)).into_iter().collect();
    all.extend(extra.map(|s| s.0.iter().copied()).into_iter().flatten());
    all
}

/// Fills [`InspectView`] from whatever the cursor is over.
pub fn collect_inspect(mut view: ResMut<InspectView>, duelists: Duelists) {
    view.subject = None;
    view.duel = None;
    view.facets.clear();
    if !duelists.modals.is_open(inspect_modal(&duelists.modals)) {
        return;
    }
    let Ok((origin, mine, my_faction)) = duelists.player.single() else { return };
    let here = duelists.map.current();
    // What the cursor picked out, when that is here, so Tab onto the second
    // of two things on one tile describes the second. Otherwise the topmost
    // glyph, the one the map drew, so the panel and the map never disagree
    // about what is being pointed at.
    let focused = duelists.focus.get();
    let under = duelists
        .subjects
        .iter()
        .filter(|(_, pos, _, _, on)| pos.0 == view.cursor && on.map(|m| m.0).unwrap_or(MapId::SURFACE) == here)
        .max_by_key(|(entity, _, _, glyph, _)| (Some(*entity) == focused, glyph.layer));
    let Some((entity, pos, name, glyph, _)) = under else { return };
    let mut row = Row::new(entity, name.as_str().to_string(), *glyph).at(geometry::chebyshev(origin.0, pos.0));
    let theirs = duelists.fighters.get(entity).ok();
    if let Some(((health, _, _, _, _, _), _)) = theirs {
        row.health = health.map(|h| (h.hp, h.max));
    }
    if let (Some(mine_f), Some((_, Some(theirs_f)))) = (my_faction, theirs) {
        row.relation = Some(duelists.rules.factions.relation(mine_f.0, theirs_f.0));
    }
    view.subject = Some(row);

    let Some((subject, _)) = theirs else { return };
    let none = rl_rules::Resistances::new();
    let (my_health, my_armor, my_speed, my_resists, my_melee, my_extra) = mine;
    let (their_health, their_armor, their_speed, their_resists, their_melee, their_extra) = subject;
    let (Some(my_health), Some(their_health)) = (my_health, their_health) else { return };
    let my_strikes = strikes_of(my_melee, my_extra);
    let their_strikes = strikes_of(their_melee, their_extra);
    let asker = Combatant {
        health: my_health.hp,
        armor: my_armor.map(|a| a.0).unwrap_or(0),
        speed: my_speed.map(|s| s.0).unwrap_or(100),
        resists: my_resists.map(|r| &r.0).unwrap_or(&none),
        strikes: &my_strikes,
    };
    let other = Combatant {
        health: their_health.hp,
        armor: their_armor.map(|a| a.0).unwrap_or(0),
        speed: their_speed.map(|s| s.0).unwrap_or(100),
        resists: their_resists.map(|r| &r.0).unwrap_or(&none),
        strikes: &their_strikes,
    };
    let stages: Vec<&dyn rl_rules::DamageStage<Entity>> = duelists.stages.0.iter().map(|s| s.as_ref() as &dyn rl_rules::DamageStage<Entity>).collect();
    view.duel = Some(duel(&asker, &other, &duelists.registries.damage_kinds, &stages));
}

/// Whether the subject is something the player is at odds with, for a
/// panel deciding whether the forecast is worth printing.
pub fn is_a_threat(view: &InspectView) -> bool {
    view.subject.as_ref().is_some_and(|s| s.relation == Some(Relation::Hostile))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use rl_rules::forecast::Outlook;

    fn stage() -> Stage {
        let mut stage = Stage::new(InspectViewPlugin);
        stage.tick();
        stage
    }

    #[test]
    fn the_cursor_opens_on_the_nearest_actor_and_closes_back_to_the_world() {
        let mut stage = stage();
        stage.actor("far one", 'f', 6, 0);
        stage.actor("near one", 'n', 2, 0);
        stage.tick();
        assert!(!stage.app.world().resource::<Modals>().any_open(), "nothing is open until the key");

        stage.press(CursorKeys::default().look);
        let view = stage.app.world().resource::<InspectView>();
        assert_eq!(view.cursor, stage.at.offset(2, 0), "it opens on the nearest thing worth looking at");
        assert_eq!(view.subject.as_ref().map(|s| s.label.as_str()), Some("near one"));
        assert!(stage.app.world().resource::<Modals>().any_open(), "and the world does not have the keys");

        stage.press(CursorKeys::default().close);
        assert!(!stage.app.world().resource::<Modals>().any_open());
        assert!(stage.app.world().resource::<InspectView>().subject.is_none(), "a closed cursor describes nothing");
    }

    #[test]
    fn cycling_walks_the_actors_in_sight_by_distance_and_comes_back_round() {
        let mut stage = stage();
        stage.actor("far one", 'f', 6, 0);
        stage.actor("near one", 'n', 2, 0);
        stage.tick();
        stage.press(CursorKeys::default().look);
        let seen = |stage: &Stage| stage.app.world().resource::<InspectView>().subject.as_ref().map(|s| s.label.clone());
        assert_eq!(seen(&stage).as_deref(), Some("near one"));
        stage.press(CursorKeys::default().next);
        assert_eq!(seen(&stage).as_deref(), Some("far one"));
        stage.press(CursorKeys::default().next);
        assert_eq!(seen(&stage).as_deref(), Some("near one"), "and round again");
    }

    #[test]
    fn tab_stops_on_things_as_well_as_actors_in_the_order_the_nearby_list_prints() {
        let mut stage = stage();
        stage.actor("near one", 'n', 2, 0);
        stage.thing("a coin", '$', 1, 0);
        stage.tick();
        stage.press(CursorKeys::default().look);
        let seen = |stage: &Stage| stage.app.world().resource::<InspectView>().subject.as_ref().map(|s| s.label.clone());
        assert_eq!(seen(&stage).as_deref(), Some("near one"), "actors first, though the coin is nearer");
        stage.press(CursorKeys::default().next);
        assert_eq!(seen(&stage).as_deref(), Some("a coin"));
        stage.press(CursorKeys::default().next);
        assert_eq!(seen(&stage).as_deref(), Some("near one"), "and round again");
    }

    #[test]
    fn tab_onto_the_second_of_two_things_on_one_tile_describes_that_one() {
        let mut stage = stage();
        stage.actor("crab", 'c', 1, 0);
        stage.thing("a coin", '$', 1, 0);
        stage.tick();
        stage.press(CursorKeys::default().look);
        let seen = |stage: &Stage| stage.app.world().resource::<InspectView>().subject.as_ref().map(|s| s.label.clone());
        assert_eq!(seen(&stage).as_deref(), Some("crab"), "the one drawn on top");
        stage.press(CursorKeys::default().next);
        assert_eq!(stage.app.world().resource::<InspectView>().cursor, stage.at.offset(1, 0), "the same tile");
        assert_eq!(seen(&stage).as_deref(), Some("a coin"), "but the coin beneath it");
    }

    #[test]
    fn the_cursor_opens_on_the_row_picked_out_and_leaves_picked_out_what_it_was_on() {
        let mut stage = Stage::new((InspectViewPlugin, crate::NearbyViewPlugin));
        stage.actor("near one", 'n', 2, 0);
        let far = stage.actor("far one", 'f', 6, 0);
        stage.tick();
        let keys = CursorKeys::default();
        stage.press(keys.next);
        stage.press(keys.next);
        assert_eq!(stage.app.world().resource::<Focus>().get(), Some(far), "Tab with nothing open walks the list");

        stage.press(keys.look);
        assert_eq!(stage.app.world().resource::<InspectView>().subject.as_ref().map(|s| s.label.as_str()), Some("far one"), "opened on the row picked out");
        stage.press(keys.close);
        assert_eq!(stage.app.world().resource::<Focus>().get(), Some(far), "closing keeps it picked out");

        stage.press(keys.look);
        stage.press(KeyCode::ArrowRight);
        stage.press(keys.close);
        assert_eq!(stage.app.world().resource::<Focus>().get(), None, "the cursor was left on bare ground");
    }

    #[test]
    fn a_step_moves_one_tile_and_the_subject_changes_with_it() {
        let mut stage = stage();
        stage.actor("east of you", 'e', 1, 0);
        stage.tick();
        stage.press(CursorKeys::default().look);
        assert_eq!(stage.app.world().resource::<InspectView>().subject.as_ref().map(|s| s.label.clone()).as_deref(), Some("east of you"));
        stage.press(KeyCode::ArrowRight);
        let view = stage.app.world().resource::<InspectView>();
        assert_eq!(view.cursor, stage.at.offset(2, 0));
        assert!(view.subject.is_none(), "one tile further there is nothing");
    }

    #[test]
    fn the_cursor_over_nothing_describes_nothing_and_never_yourself() {
        let mut stage = stage();
        stage.press(CursorKeys::default().look);
        // Nothing in sight, so the cursor opens on the player's own tile.
        let view = stage.app.world().resource::<InspectView>();
        assert_eq!(view.cursor, stage.at);
        assert!(view.subject.is_none(), "you are not something you look at");
        assert!(view.duel.is_none(), "and never a duel with yourself");
    }

    #[test]
    fn the_forecast_runs_both_ways_through_the_games_own_mitigation() {
        let mut stage = stage();
        stage.actor("a weakling", 'w', 1, 0);
        stage.tick();
        stage.press(CursorKeys::default().look);
        let view = stage.app.world().resource::<InspectView>();
        let duel = view.duel.expect("two combatants make a duel");
        // The player rolls 1d6 into no armor and has 30 health; the
        // weakling rolls 1d4 into no armor and has 10.
        assert_eq!(duel.turns_to_fell, Some(3), "10 health at 3.5 a blow is three blows");
        assert_eq!(duel.turns_to_fall, Some(12), "30 health at 2.5 a blow is twelve");
        assert_eq!(duel.outlook, Outlook::Easy);
    }
}
