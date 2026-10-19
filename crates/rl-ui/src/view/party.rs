//! Who follows the player, how each is, and where.
//!
//! The nearby rail lists what is in sight, so a follower out of it, round a
//! corner or left on another floor, drops off the rail at the moment a
//! player most wants to know how it is doing. This view lists every actor
//! whose [`Leader`] is the player, wherever it stands, with its health,
//! and marks the ones on another map as elsewhere rather than giving them a
//! distance that would mean nothing.
//!
//! The rows are the shared [`Row`], so a game says what the engine cannot
//! know, such as the order it gave a follower, the way it does on the
//! nearby rail: a [`Facet`](crate::Facet) pushed in
//! [`ViewSet::Annotate`](crate::ViewSet::Annotate).

use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_render::Glyph;

use crate::view::Row;

/// Every follower of the player, in the order they joined.
///
/// In the order of [`Joined`], the stamp a follower takes with its
/// `Leader`, rather than by distance or by entity: a list that reshuffled
/// as followers moved would make a player read every name again to find
/// the one they were watching, and `rl-save` keeps the stamp, so the order
/// is the same after a load.
#[derive(Resource, Debug, Default)]
pub struct PartyView {
    /// One row per follower: its name, glyph, health and, when it is on
    /// the player's map, its distance.
    pub rows: Vec<Row>,
    /// The followers on a map other than the player's. A row's distance
    /// means nothing for them, so a panel says this instead.
    pub elsewhere: Vec<Entity>,
}

impl PartyView {
    /// Every row, for an annotate system.
    pub fn rows_mut(&mut self) -> impl Iterator<Item = &mut Row> {
        self.rows.iter_mut()
    }

    /// The row for `entity`, if it follows the player.
    pub fn row_mut(&mut self, entity: Entity) -> Option<&mut Row> {
        self.rows.iter_mut().find(|r| r.entity == entity)
    }

    /// Whether `row` is a follower on another map.
    pub fn is_elsewhere(&self, row: &Row) -> bool {
        self.elsewhere.contains(&row.entity)
    }

    /// Whether nobody follows the player.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// Keeps [`PartyView`] current.
///
/// Needs nothing: with no [`Leader`] on anyone the view is empty, which is
/// what a game with no followers should draw.
pub struct PartyViewPlugin;

impl Plugin for PartyViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PartyView>().add_systems(Update, collect_party.in_set(crate::ViewSet::Collect));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::UiPlugin>(app, "PartyViewPlugin");
    }
}

/// What a follower's row is read off.
type Follower = (
    Entity,
    &'static Leader,
    &'static Name,
    &'static Glyph,
    Option<&'static Health>,
    Option<&'static Position>,
    Option<&'static OnMap>,
    Option<&'static Joined>,
);

/// Where the player stands, for which map is here and how far a follower is.
type Here = (Entity, Option<&'static Position>, Option<&'static OnMap>);

/// Fills [`PartyView`] from every living actor whose [`Leader`] is the
/// player.
///
/// A living actor, because remains keep the `Leader` the actor had and a
/// body is not following anyone. One without a [`Joined`] yet, taken on
/// this very frame, goes last, which is where its stamp will put it.
pub(crate) fn collect_party(mut view: ResMut<PartyView>, player: Query<Here, With<Player>>, followers: Query<Follower, (With<Actor>, Without<Dead>)>) {
    view.rows.clear();
    view.elsewhere.clear();
    let Ok((me, at, on)) = player.single() else { return };
    let here = on.map_or(MapId::ZERO, |m| m.0);
    let mut party: Vec<_> = followers.iter().filter(|f| f.1.0 == me).collect();
    party.sort_by_key(|f| (f.7.map_or(u64::MAX, |j| j.0), f.0));
    for (entity, _, name, glyph, health, pos, map, _) in party {
        let mut row = Row::new(entity, name.as_str(), *glyph);
        // A killing blow can carry health below zero for the rest of its
        // frame; the row reads empty rather than a negative count.
        row.health = health.map(|h| (h.current.max(0), h.max));
        if map.map_or(MapId::ZERO, |m| m.0) != here {
            view.elsewhere.push(entity);
        } else if let (Some(from), Some(to)) = (at, pos) {
            row.distance = rl_core::geometry::chebyshev(from.0, to.0);
        }
        view.rows.push(row);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;

    /// Gives `entity` the player as its leader, and runs a frame.
    fn follow(stage: &mut Stage, entity: Entity) {
        let me = stage.player;
        stage.app.world_mut().entity_mut(entity).insert(Leader(me));
        stage.tick();
    }

    fn labels(stage: &Stage) -> Vec<String> {
        stage.app.world().resource::<PartyView>().rows.iter().map(|r| r.label.clone()).collect()
    }

    #[test]
    fn a_follower_has_a_row_with_its_health_and_distance_and_one_that_follows_nobody_has_none() {
        let mut stage = Stage::new(PartyViewPlugin);
        let ally = stage.actor("ally", 'a', 3, 0);
        stage.actor("stranger", 's', 2, 0);
        follow(&mut stage, ally);

        let view = stage.app.world().resource::<PartyView>();
        assert_eq!(view.rows.len(), 1, "the stranger follows nobody: {:?}", view.rows);
        assert_eq!(view.rows[0].label, "ally");
        assert_eq!(view.rows[0].glyph.ch, 'a');
        assert_eq!(view.rows[0].health, Some((10, 10)));
        assert_eq!(view.rows[0].distance, 3);
        assert!(!view.is_elsewhere(&view.rows[0]));
    }

    #[test]
    fn a_follower_of_someone_else_has_no_row() {
        let mut stage = Stage::new(PartyViewPlugin);
        let chief = stage.actor("chief", 'c', 4, 0);
        let hand = stage.actor("hand", 'h', 5, 0);
        stage.app.world_mut().entity_mut(hand).insert(Leader(chief));
        stage.tick();
        assert!(stage.app.world().resource::<PartyView>().is_empty());
    }

    /// Out of sight, and on another floor, a follower still has a row: the
    /// view is about who follows, not about what is in view.
    #[test]
    fn a_follower_on_another_map_has_a_row_marked_elsewhere_that_keeps_its_health() {
        let mut stage = Stage::new(PartyViewPlugin);
        let left = stage.actor("left behind", 'l', 40, 40);
        stage.app.world_mut().entity_mut(left).insert(OnMap(MapId(7)));
        stage.app.world_mut().get_mut::<Health>(left).unwrap().current = 4;
        follow(&mut stage, left);

        let view = stage.app.world().resource::<PartyView>();
        assert_eq!(view.rows.len(), 1);
        assert!(view.is_elsewhere(&view.rows[0]), "on map 7, not the player's");
        assert_eq!(view.rows[0].health, Some((4, 10)), "and it keeps its health");
        assert_eq!(view.rows[0].distance, 0, "a distance across maps means nothing");
    }

    #[test]
    fn rows_keep_the_order_their_followers_joined_in_whatever_the_distance_or_the_entity() {
        let mut stage = Stage::new(PartyViewPlugin);
        let first_made = stage.actor("made first", 'f', 6, 0);
        let near = stage.actor("near", 'n', 1, 0);
        let far = stage.actor("far", 'r', 8, 0);
        follow(&mut stage, near);
        follow(&mut stage, far);
        follow(&mut stage, first_made);
        assert_eq!(labels(&stage), vec!["near", "far", "made first"]);

        let beside = stage.at.offset(1, 1);
        stage.app.world_mut().get_mut::<Position>(far).unwrap().0 = beside;
        stage.tick();
        stage.tick();
        assert_eq!(labels(&stage), vec!["near", "far", "made first"], "moving does not reshuffle it, frame after frame");

        stage.app.world_mut().entity_mut(near).remove::<Leader>();
        stage.tick();
        follow(&mut stage, near);
        assert_eq!(labels(&stage), vec!["far", "made first", "near"], "one that leaves and comes back joins at the end");
    }

    #[test]
    fn a_dead_follower_and_its_remains_have_no_row() {
        let mut stage = Stage::new(PartyViewPlugin);
        let gone = stage.actor("gone", 'g', 2, 0);
        follow(&mut stage, gone);
        assert_eq!(labels(&stage), vec!["gone"]);
        stage.app.world_mut().entity_mut(gone).remove::<Actor>();
        stage.tick();
        assert!(labels(&stage).is_empty(), "a body keeps its Leader, and is not following");
    }

    #[test]
    fn a_game_facet_lands_on_the_follower_it_names() {
        fn order(mut view: ResMut<PartyView>, mut facets: ResMut<crate::Facets>) {
            for row in view.rows_mut() {
                row.facets.push(facets.facet("order", "holding"));
            }
        }
        let mut stage = Stage::new(PartyViewPlugin);
        stage.app.add_systems(Update, order.in_set(crate::ViewSet::Annotate));
        let ally = stage.actor("ally", 'a', 2, 0);
        follow(&mut stage, ally);
        let view = stage.app.world().resource::<PartyView>();
        assert_eq!(view.rows[0].facets.iter().map(|f| f.text.as_str()).collect::<Vec<_>>(), vec!["holding"]);
    }

    #[test]
    fn the_rows_are_rebuilt_each_frame_rather_than_appended_to() {
        let mut stage = Stage::new(PartyViewPlugin);
        let ally = stage.actor("ally", 'a', 2, 0);
        follow(&mut stage, ally);
        stage.tick();
        stage.tick();
        assert_eq!(labels(&stage).len(), 1);
    }
}
