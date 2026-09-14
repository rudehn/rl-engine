//! What the player can see right now, as two lists.
//!
//! The panel every roguelike grows: the actors in view with their health
//! and whose side they are on, and the things lying about. Every input is
//! already in the engine - the [`Viewshed`], the [`Position`]s, the
//! [`Health`], the faction matrix - which is why this is engine work. What
//! is not engine work is the weapon in a hand or the state of a mind, and
//! that arrives as a facet.
//!
//! Rebuilt every frame rather than on a turn boundary. A frame already
//! rewrites every cell of the map; a few dozen rows beside it is not the
//! cost worth being wrong about, and a panel that is a turn stale is the
//! kind of bug that survives to a release.

use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_render::Glyph;
use rl_rules::Relation;

use rl_core::geometry;

use crate::view::Row;

/// Everything visible, nearest first.
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

    /// How many actors are hostile to the player.
    pub fn threats(&self) -> usize {
        self.actors.iter().filter(|r| r.relation == Some(Relation::Hostile)).count()
    }

    /// Whether there is nothing to show.
    pub fn is_empty(&self) -> bool {
        self.actors.is_empty() && self.things.is_empty()
    }
}

/// Keeps [`NearbyView`] current.
///
/// Needs [`WorldMap`], to know which map the rows are on, and
/// [`CombatRules`], for the relation each row carries.
pub struct NearbyViewPlugin;

impl Plugin for NearbyViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NearbyView>()
            .needs::<CombatRules>("NearbyViewPlugin", "`CombatRules { kinds, factions }`, for the relation each row carries")
            .add_systems(Update, collect_nearby.in_set(crate::ViewSet::Collect));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::UiPlugin>(app, "NearbyViewPlugin");
    }
}

/// One candidate row: everything a collector reads off an entity.
type Seen = (Entity, &'static Position, &'static Name, &'static Glyph, Option<&'static Health>, Option<&'static Faction>, Option<&'static OnMap>, Has<Actor>);

/// Who is looking, and at what.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Around<'w, 's> {
    map: Res<'w, WorldMap>,
    rules: Res<'w, CombatRules>,
    player: Query<'w, 's, (Entity, &'static Position, &'static Viewshed, Option<&'static Faction>), With<Player>>,
    seen: Query<'w, 's, Seen, Without<Dead>>,
    aware: Query<'w, 's, &'static Aware>,
    stealth: StealthRunning<'w>,
}

/// Fills [`NearbyView`] from the player's viewshed.
pub fn collect_nearby(mut view: ResMut<NearbyView>, around: Around) {
    view.actors.clear();
    view.things.clear();
    let Ok((me, origin, viewshed, mine)) = around.player.single() else { return };
    let here = around.map.current();
    for (entity, pos, name, glyph, health, faction, on, is_actor) in &around.seen {
        if entity == me || on.map(|m| m.0).unwrap_or(MapId::SURFACE) != here || !viewshed.can_see(pos.0) {
            continue;
        }
        let mut row = Row::new(entity, name.as_str().to_string(), *glyph).at(geometry::chebyshev(origin.0, pos.0));
        row.health = health.map(|h| (h.hp, h.max));
        row.relation = match (mine, faction) {
            (Some(mine), Some(theirs)) => Some(around.rules.factions.relation(mine.0, theirs.0)),
            _ => None,
        };
        if is_actor && around.stealth.get() {
            row.aware = around.aware.get(entity).ok().map(|a| a.knows(me));
        }
        if is_actor { view.actors.push(row) } else { view.things.push(row) }
    }
    // Nearest first, then by name, so a row does not jump between two of
    // the same distance from one frame to the next.
    let order = |a: &Row, b: &Row| a.distance.cmp(&b.distance).then_with(|| a.label.cmp(&b.label));
    view.actors.sort_by(order);
    view.things.sort_by(order);
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let mut stage = Stage::new((NearbyViewPlugin, rl_bevy::StealthPlugin));
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
}
