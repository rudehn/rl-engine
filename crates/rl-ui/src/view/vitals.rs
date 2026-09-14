//! The player's own numbers: bars, badges and where it is standing.
//!
//! This is the view that stops a game writing its status line by hand.
//! Health, armor, the turn, the position and the badge for every status
//! are all engine state; a game that wants to add a purse or a hunger
//! clock pushes a [`Bar`] or a facet rather than rebuilding the string.

use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_core::Point;

use crate::facet::Facet;
use crate::tone::Tones;
use crate::view::Bar;

/// What the player is, as a panel reads it.
#[derive(Resource, Debug, Default)]
pub struct VitalsView {
    /// The player entity, while there is one.
    pub entity: Option<Entity>,
    /// What the game called the player, empty if it gave it no [`Name`].
    pub label: String,
    /// Health first; a game's annotate system pushes whatever else it
    /// tracks after it.
    pub bars: Vec<Bar>,
    /// Armor, if the player has any.
    pub armor: Option<i32>,
    /// One per active status that has a badge glyph, in the order they
    /// were applied.
    pub badges: Vec<Facet>,
    /// What the game added.
    pub facets: Vec<Facet>,
    /// Whole turns elapsed.
    pub turn: u32,
    /// Where the player is standing.
    pub position: Point,
    /// Whether anything at odds with the player is watching it, by the rule
    /// the minds act on: an observer that keeps track knows about it, and one
    /// that sees on sight can see it. `None` for a player that cannot hide,
    /// or in a game without stealth.
    pub seen: Option<bool>,
}

impl VitalsView {
    /// The health bar the collector always fills first.
    pub fn health(&self) -> Option<&Bar> {
        self.bars.first()
    }
}

/// Keeps [`VitalsView`] current.
///
/// Needs nothing. Badges come from [`StatusRules`], which is an opt-in
/// subsystem the way [`Lighting`] is: a game with no
/// statuses gets no badges and pays nothing for the idea, rather than
/// being made to insert an empty registry to have a health bar.
pub struct VitalsViewPlugin;

impl Plugin for VitalsViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VitalsView>().add_systems(Update, collect_vitals.in_set(crate::ViewSet::Collect));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::UiPlugin>(app, "VitalsViewPlugin");
    }
}

/// What the collector reads off the player.
type Vitals = (Entity, &'static Position, Option<&'static Name>, Option<&'static Health>, Option<&'static Armor>, Option<&'static Afflicted>, Has<Stealth>);

/// What the player is made of.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Me<'w, 's> {
    turns: Res<'w, Turns>,
    statuses: Option<Res<'w, StatusRules>>,
    facets: ResMut<'w, crate::facet::Facets>,
    watchers: Watchers<'w, 's>,
    player: Query<'w, 's, Vitals, With<Player>>,
}

/// Fills [`VitalsView`] from the player.
pub fn collect_vitals(mut view: ResMut<VitalsView>, mut me: Me) {
    view.bars.clear();
    view.badges.clear();
    view.facets.clear();
    let Ok((entity, pos, name, health, armor, afflicted, hides)) = me.player.single() else {
        view.entity = None;
        return;
    };
    view.entity = Some(entity);
    view.label = name.map(|n| n.as_str().to_string()).unwrap_or_default();
    view.position = pos.0;
    view.turn = me.turns.turn_number();
    view.armor = armor.map(|a| a.0);
    view.seen = (hides && me.watchers.running()).then(|| me.watchers.watched(entity));
    if let Some(health) = health {
        // Tone by how close to death, so a panel needs no thresholds of
        // its own and every panel agrees on when it is bad.
        let tone = match health.hp * 4 {
            n if n <= health.max => Tones::BAD,
            n if n <= health.max * 2 => Tones::NOTICE,
            _ => Tones::GOOD,
        };
        view.bars.push(Bar::new("health", health.hp, health.max, tone));
    }
    let Some(statuses) = &me.statuses else { return };
    for active in afflicted.into_iter().flat_map(|a| a.iter()) {
        if let Some(glyph) = statuses.defs.get(active.id).badge {
            view.badges.push(me.facets.facet("badge", glyph.to_string()).toned(Tones::NOTICE));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;

    #[test]
    fn the_health_bar_darkens_as_the_player_does_and_the_whereabouts_follow() {
        let mut stage = Stage::new(VitalsViewPlugin);
        stage.tick();
        {
            let view = stage.app.world().resource::<VitalsView>();
            assert_eq!(view.label, "you");
            let health = view.health().expect("a player with health has a bar");
            assert_eq!((health.value, health.max), (30, 30));
            assert_eq!(health.tone, Tones::GOOD);
            assert_eq!(view.armor, Some(0));
            assert_eq!(view.position, stage.at);
        }
        let player = stage.player;
        stage.app.world_mut().get_mut::<Health>(player).unwrap().hp = 15;
        stage.tick();
        assert_eq!(stage.app.world().resource::<VitalsView>().health().unwrap().tone, Tones::NOTICE, "half is worth noticing");
        stage.app.world_mut().get_mut::<Health>(player).unwrap().hp = 5;
        stage.tick();
        assert_eq!(stage.app.world().resource::<VitalsView>().health().unwrap().tone, Tones::BAD, "a sixth is bad news");
    }

    #[test]
    fn seen_is_none_for_a_player_that_cannot_hide_and_tracks_every_watcher_otherwise() {
        let mut stage = Stage::new((VitalsViewPlugin, rl_bevy::StealthPlugin));
        stage.tick();
        assert_eq!(stage.app.world().resource::<VitalsView>().seen, None, "no Stealth on the player");

        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert(Stealth::default());
        stage.tick();
        assert_eq!(stage.app.world().resource::<VitalsView>().seen, Some(false));

        let guard = stage.actor("guard", 'g', 3, 0);
        stage.app.world_mut().entity_mut(guard).insert(Notice::default());
        let at = stage.at;
        stage.app.world_mut().get_mut::<Aware>(guard).unwrap().0.insert(player, rl_rules::Awareness::Alert { at, stale_turns: 0 });
        stage.tick();
        assert_eq!(stage.app.world().resource::<VitalsView>().seen, Some(true));
    }

    #[test]
    fn a_status_with_a_badge_shows_one_and_a_status_without_shows_none() {
        let mut stage = Stage::new(VitalsViewPlugin);
        let defs = rl_rules::Registry::from_defs(vec![
            rl_rules::StatusDef { badge: Some('v'), ..rl_rules::StatusDef::new("venom") },
            rl_rules::StatusDef::new("quiet"),
        ])
        .unwrap();
        let (venom, quiet) = (defs.expect("venom"), defs.expect("quiet"));
        stage.app.insert_resource(StatusRules { defs });
        let player = stage.player;
        stage.app.world_mut().write_message(Afflict { target: player, status: venom, turns: 5, by: None });
        stage.app.world_mut().write_message(Afflict { target: player, status: quiet, turns: 5, by: None });
        stage.tick();
        stage.tick();

        let badges: Vec<&str> = stage.app.world().resource::<VitalsView>().badges.iter().map(|b| b.text.as_str()).collect();
        assert_eq!(badges, vec!["v"], "one badge glyph, from the status that has one");
    }
}
