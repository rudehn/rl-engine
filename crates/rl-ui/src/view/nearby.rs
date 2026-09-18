//! What the player can see right now, as two lists, and which row of them
//! is picked out.
//!
//! The panel every roguelike grows: the actors in view with their health
//! and whose side they are on, and the things lying about. Every input is
//! already in the engine - the [`Viewshed`], the [`Position`]s, the
//! [`Health`], the faction matrix - which is why this is engine work. What
//! is not engine work is the weapon in a hand or the state of a mind, and
//! that arrives as a facet.
//!
//! The rows come in [`InSight`]'s order, the order the look and targeting
//! cursors step through, and the row picked out is the [`Focus`] those
//! cursors move. With nothing open, [`browse`] steps the same focus down
//! the rows on the cursors' "next" key, so the list can be walked without
//! opening a cursor at all, and a cursor opened afterwards opens on the row
//! that was picked out.
//!
//! Rebuilt every frame rather than on a turn boundary. A frame already
//! rewrites every cell of the map; a few dozen rows beside it is not the
//! cost worth being wrong about, and a panel that is a turn stale is the
//! kind of bug that survives to a release.

use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_render::Glyph;
use rl_rules::Relation;

use crate::focus::{Focus, InSight, Sighting, browse};
use crate::view::Row;

/// Everything visible, in list order, and the row picked out.
///
/// Actors and things are separate lists because a panel almost always
/// wants them under separate headings, and because only one of them can
/// hurt you.
#[derive(Resource, Debug, Default)]
pub struct NearbyView {
    /// Actors, nearest first. Never includes the player.
    pub actors: Vec<Row>,
    /// Items and props on visible ground, nearest first.
    pub things: Vec<Row>,
    /// The row picked out, and where it stands: what [`Focus`] names, when
    /// that is in either list.
    pub focused: Option<Sighting>,
}

impl NearbyView {
    /// Every row in both lists, for an annotate system that does not care
    /// which is which.
    pub fn rows_mut(&mut self) -> impl Iterator<Item = &mut Row> {
        self.actors.iter_mut().chain(self.things.iter_mut())
    }

    /// The row for `entity`, in either list.
    pub fn row_mut(&mut self, entity: Entity) -> Option<&mut Row> {
        self.rows_mut().find(|r| r.entity == entity)
    }

    /// Whether `row` is the one picked out.
    pub fn is_focused(&self, row: &Row) -> bool {
        self.focused.is_some_and(|s| s.entity == row.entity)
    }

    /// How many actors are hostile to the player.
    pub fn threats(&self) -> usize {
        self.actors.iter().filter(|r| r.relation == Some(Relation::Hostile)).count()
    }

    /// Whether there is nothing to show.
    pub fn is_empty(&self) -> bool {
        self.actors.is_empty() && self.things.is_empty()
    }
}

/// Keeps [`NearbyView`] current, and walks its rows with the cursors'
/// "next" key while nothing is open.
///
/// Needs [`WorldMap`], to know which map the rows are on, and
/// [`CombatRules`], for the relation each row carries.
pub struct NearbyViewPlugin;

impl Plugin for NearbyViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NearbyView>().init_resource::<Focus>().init_resource::<crate::CursorKeys>();
        app.needs::<CombatRules>("NearbyViewPlugin", "`CombatRules::new(&sides)`, for the relation each row carries")
            // Before either cursor reads the same keys, so the key that puts
            // a cursor away is not read again as letting go of what it left
            // picked out.
            .add_systems(Update, browse.in_set(EngineSet::Input).before(crate::view::inspect::move_cursor).before(crate::view::target::aim_cursor))
            .add_systems(Update, collect_nearby.in_set(crate::ViewSet::Collect));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::UiPlugin>(app, "NearbyViewPlugin");
        // After the game's own, so its groups are listed first.
        crate::focus::declare_focus_controls(app);
    }
}

/// What a row is read off, once the list says it is in sight.
type Seen = (&'static Name, &'static Glyph, Option<&'static Health>, Option<&'static Faction>);

/// Who is looking, and at what.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Around<'w, 's> {
    rules: Res<'w, CombatRules>,
    focus: Res<'w, Focus>,
    sight: InSight<'w, 's>,
    seen: Query<'w, 's, Seen>,
    factions: Query<'w, 's, &'static Faction>,
    watchers: Watchers<'w, 's>,
    noise: rl_bevy::NoiseRunning<'w>,
    heard: Query<'w, 's, &'static rl_bevy::Heard>,
}

/// Fills [`NearbyView`] from the player's viewshed, in [`InSight`]'s order.
pub fn collect_nearby(mut view: ResMut<NearbyView>, around: Around) {
    view.actors.clear();
    view.things.clear();
    view.focused = None;
    let Some((me, _)) = around.sight.viewer() else { return };
    let mine = around.factions.get(me).ok();
    let list = around.sight.list();
    view.focused = around.focus.within(&list).copied();
    for sighting in list {
        let Ok((name, glyph, health, faction)) = around.seen.get(sighting.entity) else { continue };
        let mut row = Row::new(sighting.entity, name.as_str().to_string(), *glyph).at(sighting.distance);
        row.health = health.map(|h| (h.current, h.max));
        row.relation = match (mine, faction) {
            (Some(mine), Some(theirs)) => Some(around.rules.factions.relation(mine.0, theirs.0)),
            _ => None,
        };
        if sighting.actor && around.watchers.running() && around.watchers.is_watcher(sighting.entity) {
            row.aware = Some(around.watchers.sees(sighting.entity, me));
        }
        if sighting.actor && around.noise.get() {
            row.heard = around.heard.get(sighting.entity).ok().map(|h| h.is_alert());
        }
        if sighting.actor { view.actors.push(row) } else { view.things.push(row) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cursor::CursorKeys;
    use crate::facet::Facets;
    use crate::harness::Stage;

    /// A game's annotate system: notes the one entity it was told about.
    #[derive(Resource)]
    struct Noted(Entity);

    fn note(mut view: ResMut<NearbyView>, mut facets: ResMut<Facets>, noted: Res<Noted>) {
        if let Some(row) = view.row_mut(noted.0) {
            row.facets.push(facets.facet("wielding", "a rusty blade"));
        }
    }

    #[test]
    fn the_rows_are_what_is_in_sight_nearest_first_and_never_the_player() {
        let mut stage = Stage::new(NearbyViewPlugin);
        stage.actor("far one", 'f', 6, 0);
        stage.actor("near one", 'n', 2, 0);
        stage.thing("a coin", '$', 1, 0);
        stage.tick();

        let view = stage.app.world().resource::<NearbyView>();
        let names: Vec<&str> = view.actors.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(names, vec!["near one", "far one"], "nearest first");
        assert!(!names.contains(&"you"), "the player is not in its own list");
        assert_eq!(view.actors[0].distance, 2);
        assert_eq!(view.actors[0].health, Some((10, 10)));
        assert_eq!(view.actors[0].relation, Some(Relation::Hostile));
        assert_eq!(view.things.len(), 1, "the coin is a thing, not an actor");
        assert_eq!(view.things[0].label, "a coin");
        assert_eq!(view.things[0].health, None);
        assert_eq!(view.threats(), 2);
    }

    #[test]
    fn a_row_says_whether_it_has_noticed_the_player_only_when_stealth_is_running() {
        // Stealth is decided in the minds' pass, so it needs the minds.
        let mut stage = Stage::new((NearbyViewPlugin, rl_bevy::MindsPlugin, rl_bevy::StealthPlugin));
        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert(Stealth::default());
        let hunting = stage.actor("hunting", 'h', 2, 0);
        let idle = stage.actor("idle", 'i', 3, 0);
        let blunt = stage.actor("blunt", 'b', 4, 0);
        for watcher in [hunting, idle] {
            stage.app.world_mut().entity_mut(watcher).insert(Notice(rl_rules::NoticeStats { certain: 1, chance_pct: 0, lit_bonus: 0, memory: 6 }));
        }
        let at = stage.at;
        stage.app.world_mut().get_mut::<Aware>(hunting).unwrap().0.insert(player, rl_rules::Awareness::Alert { at, stale_turns: 0 });
        stage.tick();

        let view = stage.app.world().resource::<NearbyView>();
        let aware = |e: Entity| view.actors.iter().find(|r| r.entity == e).map(|r| r.aware);
        assert_eq!(aware(hunting), Some(Some(true)));
        assert_eq!(aware(idle), Some(Some(false)));
        assert_eq!(aware(blunt), Some(None), "something with no Notice sees on sight and says nothing");

        let mut plain = Stage::new(NearbyViewPlugin);
        plain.actor("anyone", 'a', 2, 0);
        plain.tick();
        assert_eq!(plain.app.world().resource::<NearbyView>().actors[0].aware, None, "no stealth, no reading");
    }

    #[test]
    fn a_row_says_whether_it_is_going_to_look_at_a_sound_only_when_noise_is_running() {
        let rules = rl_bevy::NoiseRules { step: 0, strike: 0, door: 0, landing: 0, door_muffle: 0 };
        let mut stage = Stage::new((NearbyViewPlugin, rl_bevy::NoisePlugin::new(rules)));
        let listening = stage.actor("listening", 'l', 2, 0);
        let quiet = stage.actor("quiet", 'q', 3, 0);
        let deaf = stage.actor("deaf", 'd', 4, 0);
        let ear = rl_bevy::Hearing(rl_rules::HearingStats { threshold: 0, memory: 6 });
        for listener in [listening, quiet] {
            stage.app.world_mut().entity_mut(listener).insert(ear);
        }
        let at = stage.at;
        stage.app.world_mut().get_mut::<rl_bevy::Heard>(listening).unwrap().0 = rl_rules::Awareness::Alert { at, stale_turns: 0 };
        stage.tick();

        let view = stage.app.world().resource::<NearbyView>();
        let heard = |e: Entity| view.actors.iter().find(|r| r.entity == e).map(|r| r.heard);
        assert_eq!(heard(listening), Some(Some(true)));
        assert_eq!(heard(quiet), Some(Some(false)));
        assert_eq!(heard(deaf), Some(None), "something with no Hearing hears nothing and says nothing");

        let mut plain = Stage::new(NearbyViewPlugin);
        let anyone = plain.actor("anyone", 'a', 2, 0);
        plain.app.world_mut().entity_mut(anyone).insert(ear);
        plain.tick();
        assert_eq!(plain.app.world().resource::<NearbyView>().actors[0].heard, None, "no noise, no reading, Hearing or not");
    }

    #[test]
    fn something_out_of_sight_is_not_a_row() {
        let mut stage = Stage::new(NearbyViewPlugin);
        stage.actor("in sight", 'i', 3, 0);
        stage.actor("over the hill", 'o', 40, 40);
        stage.tick();

        let view = stage.app.world().resource::<NearbyView>();
        let names: Vec<&str> = view.actors.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(names, vec!["in sight"]);
    }

    #[test]
    fn a_game_facet_lands_on_the_row_it_names_and_on_no_other() {
        let mut stage = Stage::new(NearbyViewPlugin);
        let armed = stage.actor("armed one", 'a', 2, 0);
        stage.actor("bare one", 'b', 3, 0);
        stage.app.insert_resource(Noted(armed));
        stage.app.add_systems(Update, note.in_set(crate::ViewSet::Annotate));
        stage.tick();

        let view = stage.app.world().resource::<NearbyView>();
        let armed_row = view.actors.iter().find(|r| r.entity == armed).expect("the armed one is in sight");
        assert_eq!(armed_row.facets.len(), 1);
        assert_eq!(armed_row.facets[0].text, "a rusty blade");
        let bare = view.actors.iter().find(|r| r.label == "bare one").expect("in sight");
        assert!(bare.facets.is_empty(), "a facet does not spill onto its neighbour");
    }

    #[test]
    fn the_rows_are_rebuilt_each_frame_rather_than_appended_to() {
        let mut stage = Stage::new(NearbyViewPlugin);
        stage.actor("one", 'o', 2, 0);
        stage.tick();
        stage.tick();
        stage.tick();
        assert_eq!(stage.app.world().resource::<NearbyView>().actors.len(), 1, "three frames, one row");
    }

    #[test]
    fn tab_with_nothing_open_walks_the_rows_in_the_order_they_are_printed_and_escape_lets_go() {
        let mut stage = Stage::new(NearbyViewPlugin);
        let far = stage.actor("far one", 'f', 6, 0);
        let coin = stage.thing("a coin", '$', 1, 0);
        let near = stage.actor("near one", 'n', 2, 0);
        stage.tick();
        let focused = |stage: &Stage| stage.app.world().resource::<NearbyView>().focused.map(|s| s.entity);
        assert_eq!(focused(&stage), None, "nothing is picked out until the key");

        let keys = CursorKeys::default();
        for (want, why) in [(near, "the top row"), (far, "the next actor"), (coin, "then the things"), (near, "and round again")] {
            stage.press(keys.next);
            assert_eq!(focused(&stage), Some(want), "{why}");
        }
        let at = stage.app.world().resource::<NearbyView>().focused.map(|s| s.at);
        assert_eq!(at, Some(stage.at.offset(2, 0)), "and the view says where it stands, for the map");
        stage.press(keys.close);
        assert_eq!(focused(&stage), None, "let go");
    }

    #[test]
    fn a_row_that_leaves_sight_is_no_longer_picked_out() {
        let mut stage = Stage::new(NearbyViewPlugin);
        let one = stage.actor("one", 'o', 2, 0);
        stage.tick();
        stage.press(CursorKeys::default().next);
        assert_eq!(stage.app.world().resource::<NearbyView>().focused.map(|s| s.entity), Some(one));
        let far = stage.at.offset(40, 40);
        stage.app.world_mut().get_mut::<Position>(one).unwrap().0 = far;
        stage.tick();
        stage.tick();
        assert_eq!(stage.app.world().resource::<NearbyView>().focused, None, "out of sight, so nothing is highlighted");
    }
}
