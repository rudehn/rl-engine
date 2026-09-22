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
    /// Armor, its own and its gear's together, if the player has any at
    /// all: `None` for one with no [`Armor`] and nothing worn that adds
    /// any, so a game without the idea draws no number for it.
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
    /// How loud it is where the player stands: the loudest noise that
    /// reached them this turn, in hundredths of a step of loudness still
    /// on it when it arrived, falling away over the turns after by
    /// [`NoiseFade`], and zero once it has faded to nothing. `None` in a game without noise,
    /// or for a player with no `Hearing`, since a deaf player is told
    /// nothing rather than told silence.
    ///
    /// Held for the turn rather than the frame: a reading that flashed for
    /// one frame and went out is a reading nobody sees.
    pub noise: Option<i32>,
}

impl VitalsView {
    /// The health bar the collector always fills first.
    pub fn health(&self) -> Option<&Bar> {
        self.bars.first()
    }
}

/// How much of a noise reading is left one turn later.
///
/// A noise is a moment, but a reading that snapped from full to nothing
/// between two turns is a reading that flickers: the probe shouts, the
/// bar fills, the next turn it is empty, and the turn after that it is
/// full again. What a player wants to see is that it is loud here and
/// getting quieter, so the reading falls away over a few turns instead.
///
/// Six tenths a turn by default, which is loud to silent in about five
/// turns. Whatever arrives in a turn outranks what is left of the last
/// one: a fresh shout fills the bar however faded the old one was.
#[derive(Resource, Debug, Clone, Copy)]
pub struct NoiseFade(pub f32);

impl Default for NoiseFade {
    fn default() -> Self {
        Self(0.6)
    }
}

/// Below this, in hundredths of a step, a fading reading is silence: a
/// bar showing a hundredth of a noise is a bar showing nothing.
const FADED_TO_NOTHING: i32 = 100;

/// Keeps [`VitalsView`] current.
///
/// Needs nothing. Badges come from the statuses in [`Registries`], read if
/// the game inserted any the way [`Lighting`] is: a game with no statuses
/// gets no badges and pays nothing for the idea, rather than being made to
/// insert an empty registry to have a health bar.
pub struct VitalsViewPlugin;

impl Plugin for VitalsViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VitalsView>()
            .init_resource::<NoiseFade>()
            // The reading is what reached the player, and a game with no
            // noise simply hears nothing.
            .reads::<rl_bevy::NoiseHeard>()
            .add_systems(Update, collect_vitals.in_set(crate::ViewSet::Collect));
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
    /// What reached the player this frame, and whether noise runs at all.
    noise: rl_bevy::NoiseRunning<'w>,
    heard: MessageReader<'w, 's, rl_bevy::NoiseHeard>,
    fade: Res<'w, NoiseFade>,
    ears: Query<'w, 's, (), (With<Player>, With<rl_bevy::Hearing>)>,
    registries: Option<Res<'w, Registries>>,
    facets: ResMut<'w, crate::facet::Facets>,
    watchers: Watchers<'w, 's>,
    loadout: Loadout<'w, 's>,
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
    // A turn's own loudest arrival, kept until the turn changes: the
    // flood runs inside the pass and this reads it after, so a reading
    // that reset every frame would be blank whenever the player looked.
    let turn = me.turns.turn_number();
    // Read whatever arrived either way, so a game that turns its ears off
    // does not come back to a backlog of old noises.
    let loudest = me.heard.read().filter(|ev| ev.listener == entity).map(|ev| ev.left).max();
    let listening = me.noise.get() && me.ears.contains(entity);
    view.noise = match (listening, view.turn == turn, view.noise) {
        // Nothing to say: no noise in the game, or a player with no ears.
        (false, ..) => None,
        // The same turn, seen again: the loudest of it stands.
        (true, true, held) => Some(held.unwrap_or(0).max(loudest.unwrap_or(0))),
        // A fresh turn: what is left of the last one fades, and whatever
        // arrives in this one outranks it.
        (true, false, held) => {
            let left = (held.unwrap_or(0) as f32 * me.fade.0) as i32;
            let left = if left < FADED_TO_NOTHING { 0 } else { left };
            Some(left.max(loudest.unwrap_or(0)))
        }
    };
    view.turn = turn;
    // What a blow meets, gear included; shown whenever the player has the
    // idea of armor at all, even at zero.
    let total = me.loadout.armor(entity);
    view.armor = (armor.is_some() || total != 0).then_some(total);
    view.seen = (hides && me.watchers.running()).then(|| me.watchers.watched(entity));
    if let Some(health) = health {
        // Tone by how close to death, so a panel needs no thresholds of
        // its own and every panel agrees on when it is bad.
        let tone = match health.current * 4 {
            n if n <= health.max => Tones::BAD,
            n if n <= health.max * 2 => Tones::NOTICE,
            _ => Tones::GOOD,
        };
        // A killing blow can carry health below zero; the bar reads empty
        // rather than a negative count on the screen the run ends on.
        view.bars.push(Bar::new("health", health.current.max(0), health.max, tone));
    }
    let Some(registries) = &me.registries else { return };
    for active in afflicted.into_iter().flat_map(|a| a.iter()) {
        if let Some(glyph) = registries.statuses.get(active.id).badge {
            view.badges.push(me.facets.facet("badge", glyph.to_string()).toned(Tones::NOTICE));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;

    /// A noise is a moment, but the reading of it is not: it falls away
    /// over the turns after rather than snapping to nothing, which is
    /// what made a probe's alarm flicker between full and empty.
    #[test]
    fn a_noise_reading_fades_over_the_turns_after_it_rather_than_going_out_at_once() {
        let rules = rl_bevy::NoiseRules { step: 0, strike: 0, door: 0, landing: 0, door_muffle: 0 };
        let mut stage = Stage::new((VitalsViewPlugin, rl_bevy::NoisePlugin::new(rules)));
        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert(rl_bevy::Hearing(rl_rules::HearingStats { threshold: 0, memory: 6 }));
        stage.tick();
        let sound = stage.app.world_mut().resource_mut::<rl_bevy::Sounds>().declare("klaxon");
        stage.app.world_mut().write_message(rl_bevy::MakeNoise { at: stage.at.offset(1, 0), loudness: 80, sound, maker: None });
        stage.tick();
        let shout = stage.app.world().resource::<VitalsView>().noise.expect("the klaxon was heard");
        assert!(shout > 1000, "a klaxon next door is loud: {shout}");

        // Quiet turns from here: it falls, and keeps falling, and is not
        // silent the moment the klaxon stops.
        // Ten of them: a klaxon at eighty is the loudest thing a game
        // here makes, and it takes that long to die away, which is the
        // point of a klaxon.
        let mut readings = vec![shout];
        for _ in 0..10 {
            let me = stage.player;
            stage.app.world_mut().write_message(rl_bevy::Intent::new(me, rl_bevy::Wait));
            stage.tick();
            readings.push(stage.app.world().resource::<VitalsView>().noise.expect("still a reading"));
        }
        assert!(readings[1] > 0 && readings[1] < shout, "the turn after is quieter, not silent: {readings:?}");
        assert!(readings.windows(2).all(|w| w[1] <= w[0]), "and it only ever falls: {readings:?}");
        assert_eq!(readings.last(), Some(&0), "until it is silence: {readings:?}");

        // And a fresh shout fills it again, however faded it had got.
        stage.app.world_mut().write_message(rl_bevy::MakeNoise { at: stage.at.offset(1, 0), loudness: 80, sound, maker: None });
        stage.tick();
        assert_eq!(stage.app.world().resource::<VitalsView>().noise, Some(shout), "a fresh klaxon reads as loud as the first");
    }

    /// How loud it is where the player stands, held for the turn: a
    /// reading that went out the frame after the noise is a reading
    /// nobody sees. Louder near, quieter far.
    #[test]
    fn the_noise_reading_is_the_loudest_thing_that_reached_the_player_this_turn() {
        let rules = rl_bevy::NoiseRules { step: 0, strike: 0, door: 0, landing: 0, door_muffle: 0 };
        let mut stage = Stage::new_with((VitalsViewPlugin, rl_bevy::NoisePlugin::new(rules)), |_| {});
        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert(rl_bevy::Hearing(rl_rules::HearingStats { threshold: 0, memory: 6 }));
        stage.tick();
        assert_eq!(stage.app.world().resource::<VitalsView>().noise, Some(0), "a quiet turn reads zero, not nothing");

        let sound = stage.app.world_mut().resource_mut::<rl_bevy::Sounds>().declare("clatter");
        let near = stage.at.offset(2, 0);
        stage.app.world_mut().write_message(rl_bevy::MakeNoise { at: near, loudness: 10, sound, maker: None });
        stage.tick();
        let close = stage.app.world().resource::<VitalsView>().noise.expect("something was heard");
        assert!(close > 0, "a clatter two tiles off reads loud: {close}");

        // The same clatter further off, in a turn of its own: within one
        // turn the loudest stands, which is the next assertion's business.
        let player_again = stage.player;
        stage.app.world_mut().write_message(rl_bevy::Intent::new(player_again, rl_bevy::Wait));
        stage.tick();
        let far = stage.at.offset(7, 0);
        stage.app.world_mut().write_message(rl_bevy::MakeNoise { at: far, loudness: 10, sound, maker: None });
        stage.tick();
        let distant = stage.app.world().resource::<VitalsView>().noise.expect("and that was heard too");
        assert!(distant < close, "further off is quieter: {close} then {distant}");

        // A player that cannot hear is told nothing rather than silence.
        let mut deaf = Stage::new(VitalsViewPlugin);
        deaf.tick();
        assert_eq!(deaf.app.world().resource::<VitalsView>().noise, None, "no noise running, no reading");
    }

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
        stage.app.world_mut().get_mut::<Health>(player).unwrap().current = 15;
        stage.tick();
        assert_eq!(stage.app.world().resource::<VitalsView>().health().unwrap().tone, Tones::NOTICE, "half is worth noticing");
        stage.app.world_mut().get_mut::<Health>(player).unwrap().current = 5;
        stage.tick();
        assert_eq!(stage.app.world().resource::<VitalsView>().health().unwrap().tone, Tones::BAD, "a sixth is bad news");
    }

    #[test]
    fn a_blow_that_carries_health_below_zero_reads_as_an_empty_bar() {
        let mut stage = Stage::new(VitalsViewPlugin);
        let player = stage.player;
        stage.app.world_mut().get_mut::<Health>(player).unwrap().current = -3;
        stage.tick();
        let view = stage.app.world().resource::<VitalsView>();
        assert_eq!(view.health().map(|h| (h.value, h.max)), Some((0, 30)));
    }

    #[test]
    fn seen_is_none_for_a_player_that_cannot_hide_and_tracks_every_watcher_otherwise() {
        // Stealth is decided in the minds' pass, so it needs the minds.
        let mut stage = Stage::new((VitalsViewPlugin, rl_bevy::MindsPlugin, rl_bevy::StealthPlugin));
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
        stage.app.world_mut().resource_mut::<Registries>().statuses = defs;
        let player = stage.player;
        stage.app.world_mut().write_message(Afflict { target: player, status: venom, turns: 5, by: None });
        stage.app.world_mut().write_message(Afflict { target: player, status: quiet, turns: 5, by: None });
        stage.tick();
        stage.tick();

        let badges: Vec<&str> = stage.app.world().resource::<VitalsView>().badges.iter().map(|b| b.text.as_str()).collect();
        assert_eq!(badges, vec!["v"], "one badge glyph, from the status that has one");
    }
}
