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

use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_core::{Point, geometry};
use rl_render::Glyph;
use rl_rules::Relation;
use rl_rules::damage::DamageKindId;
use rl_rules::forecast::{Combatant, Duel, duel};

use crate::cursor::{self, CursorInput, CursorKeys, Steer};
use crate::facet::Facet;
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
/// [`CombatRules`] for the damage kinds the forecast resolves through. Its
/// keys are [`CursorKeys`], the ones the targeting cursor answers to.
pub struct InspectViewPlugin;

impl Plugin for InspectViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InspectView>().init_resource::<CursorKeys>();
        // `Modals` is plain data, so this plugin makes sure it exists rather
        // than panicking when added before `UiPlugin`.
        app.init_resource::<Modals>().world_mut().resource_mut::<Modals>().declare(INSPECT_MODAL);
        app.needs::<CombatRules>("InspectViewPlugin", "`CombatRules { kinds, factions }`, the damage kinds the forecast resolves through")
            .add_systems(Update, move_cursor.in_set(EngineSet::Input))
            .add_systems(Update, collect_inspect.in_set(crate::ViewSet::Collect));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::UiPlugin>(app, "InspectViewPlugin");
    }
}

/// The id of the inspect modal, for a game gating its own systems.
///
/// # Panics
/// Panics if [`InspectViewPlugin`] was not added.
pub fn inspect_modal(modals: &Modals) -> ModalId {
    modals.get(INSPECT_MODAL).expect("InspectViewPlugin declares the inspect modal")
}

/// Where something is, and on which map.
type Standing = (&'static Position, Option<&'static OnMap>);
/// Everyone but the player, and only the living.
type OtherActors = (With<Actor>, Without<Player>, Without<Dead>);
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
    player: Query<'w, 's, (&'static Position, &'static Viewshed), With<Player>>,
    actors: Query<'w, 's, Standing, OtherActors>,
}

/// Opens, closes, steps and cycles the cursor.
///
/// Cycling walks the actors in sight by distance, so pressing it twice
/// from the same place always lands on the same second thing.
pub fn move_cursor(mut view: ResMut<InspectView>, mut modals: ResMut<Modals>, look: Look) {
    let modal = inspect_modal(&modals);
    let Ok((origin, viewshed)) = look.player.single() else { return };
    let here = look.map.current();
    let in_sight = || {
        cursor::ordered(
            origin.0,
            look.actors.iter().filter(|(pos, on)| on.map(|m| m.0).unwrap_or(MapId::SURFACE) == here && viewshed.can_see(pos.0)).map(|(pos, _)| pos.0),
        )
    };

    let toggled = look.input.just_pressed(look.input.keys().look);
    if toggled && !modals.any_open() {
        modals.open(modal);
        view.cursor = in_sight().first().copied().unwrap_or(origin.0);
        return;
    }
    if !modals.is_top(modal) {
        return;
    }
    if toggled {
        modals.close_one(modal);
        return;
    }
    match look.input.steer(&mut view.cursor, look.map.window_tiles(), in_sight) {
        Steer::Close => modals.close_one(modal),
        // Looking spends nothing, so there is nothing to confirm.
        Steer::Confirm | Steer::Moved | Steer::Stay => {}
    }
}

/// Everything the forecast reads.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Duelists<'w, 's> {
    rules: Res<'w, CombatRules>,
    stages: Res<'w, DamageStages>,
    modals: Res<'w, Modals>,
    map: Res<'w, WorldMap>,
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
    // The topmost glyph, which is the one the map drew, so the panel and
    // the map never disagree about what is being pointed at.
    let under = duelists
        .subjects
        .iter()
        .filter(|(_, pos, _, _, on)| pos.0 == view.cursor && on.map(|m| m.0).unwrap_or(MapId::SURFACE) == here)
        .max_by_key(|(_, _, _, glyph, _)| glyph.layer);
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
    view.duel = Some(duel(&asker, &other, &duelists.rules.kinds, &stages));
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
