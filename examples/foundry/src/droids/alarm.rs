//! A probe's radar reports back to the rest of the deck. Spec section 8.5:
//! noticing an enemy is not a private fact for a droid carrying [`Alarm`],
//! it sounds a klaxon every droid on the deck can hear.
//!
//! The alarm is a noise of the game's own, carried by the engine's
//! hearing: loud enough to reach every open corner of a deck, stopped by
//! walls and muffled by a shut door the way any sound is. A droid that
//! hears it comes to where the probe sounded it, not to where the enemy
//! stands; what it finds there is the ordinary sight and notice roll, so
//! an alarm answered is not yet an enemy caught.
//!
//! Nothing about the alarm is a `DarkSight` matter: a line droid with no
//! radar at all still hears a probe's klaxon, since hearing is not seeing.

use std::collections::BTreeSet;

use bevy::prelude::*;
use rl_engine::prelude::*;

/// The name the alarm's sound is declared under, with
/// [`AddSound::add_sound`](rl_engine::rl_bevy::AddSound) in
/// [`FoundryPlugin`](crate::plugin::FoundryPlugin).
pub const ALARM_SOUND: &str = "alarm";

/// How far the klaxon carries, in steps of open deck: past the far corner
/// of a 70 by 40 deck by any way round it, so spec section 8.5's "wakes the
/// deck" still holds wherever the probe stands.
pub const ALARM_LOUDNESS: i32 = 80;

/// How loud the engine's own actions are on a deck. A blow or a shot
/// carries ten steps, so a firefight draws the droids in earshot; steps,
/// doors and a thrown thing landing make no sound worth hearing over the
/// machinery, and a shut bulkhead takes three steps off anything that
/// passes it.
pub const NOISE: NoiseRules = NoiseRules { step: 0, strike: 10, door: 0, landing: 0, door_muffle: 3 };

/// Marks a monster whose noticing an enemy sounds the deck's alarm,
/// through [`sound_alarm`].
///
/// A marker rather than a field on [`Notice`](rl_engine::rl_bevy::prelude::Notice),
/// since most of a deck's droids never carry it: only the probe's radar
/// reports back.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Alarm;

/// The decks whose alarm has sounded this run, so each logs its line once.
///
/// A resource [`run::start`](crate::run::start) inserts fresh, not a
/// `Local`: a deck's [`MapId`] is the same in every run, so a record that
/// outlived its run would keep a second run started from the menu from
/// ever logging the alarm on a deck that sounded in the first.
#[derive(Resource, Debug, Default)]
pub struct Sounded(pub BTreeSet<MapId>);

/// Sounds the alarm for every [`Noticed`] whose observer carries
/// [`Alarm`]: a [`MakeNoise`] of [`ALARM_SOUND`] where the probe stands, as
/// loud as [`ALARM_LOUDNESS`]. Whoever hears it comes to look; the engine's
/// hearing decides who that is.
///
/// Logs one line the first time a given deck's alarm sounds in a run,
/// kept in [`Sounded`].
pub fn sound_alarm(
    mut noticed: MessageReader<Noticed>,
    alarmed: Query<(&Position, &OnMap), With<Alarm>>,
    sounds: Res<Sounds>,
    mut noise: MessageWriter<MakeNoise>,
    mut tell: MessageWriter<Tell>,
    mut sounded: ResMut<Sounded>,
) {
    let alarm = sounds.get(ALARM_SOUND).expect("FoundryPlugin declares the alarm's sound");
    for ev in noticed.read() {
        let Ok((at, on_map)) = alarmed.get(ev.observer) else { continue };
        noise.write(MakeNoise { at: at.0, loudness: ALARM_LOUDNESS, sound: alarm, maker: Some(ev.observer) });
        if sounded.0.insert(on_map.0) {
            tell.write(Tell::new(format!("Deck {}: an alarm sounds.", crate::decks::deck_of(on_map.0)), Tones::BAD));
        }
    }
}
