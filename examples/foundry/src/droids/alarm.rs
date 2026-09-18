//! A probe's radar reports back to the rest of the deck. Spec section 8.5:
//! noticing an enemy is not a private fact for a droid carrying [`Alarm`],
//! it is a broadcast every droid on the same deck receives at once.
//!
//! Nothing about the alarm is a `DarkSight` matter: a line droid with no
//! radar at all still wakes to a probe's shout, since the alarm is a
//! report over the deck's own comms, not a thing seen.

use std::collections::BTreeSet;

use bevy::prelude::*;
use rl_engine::prelude::*;

/// Marks a monster whose noticing an enemy wakes every actor of its own
/// faction on the same deck, through [`sound_alarm`].
///
/// A marker rather than a field on [`Notice`](rl_engine::rl_bevy::prelude::Notice),
/// since most of a deck's droids never carry it: only the probe's radar
/// reports back.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Alarm;

/// Reacts to every [`Noticed`] whose observer carries [`Alarm`]: every
/// actor sharing the observer's [`Faction`] on the observer's map is made
/// alert to the subject at the point it was seen, through [`Aware`]'s own
/// [`Awareness::alerted_to`], the same way a blow wakes an observer that
/// never saw it coming.
///
/// Logs one line the first time a given deck's alarm sounds, kept in a
/// [`Local`] rather than a resource: nothing else in the game has a use
/// for which decks have sounded, so nothing else needs to read it.
pub fn sound_alarm(
    mut noticed: MessageReader<Noticed>,
    alarmed: Query<(&OnMap, &Faction), With<Alarm>>,
    mut droids: Query<(&mut Aware, &Faction, &OnMap)>,
    turns: Res<Turns>,
    mut log: ResMut<MessageLog>,
    mut sounded: Local<BTreeSet<MapId>>,
) {
    for ev in noticed.read() {
        let Ok((on_map, alarm_faction)) = alarmed.get(ev.observer) else { continue };
        for (mut aware, faction, droid_map) in &mut droids {
            if droid_map.0 != on_map.0 || faction.0 != alarm_faction.0 {
                continue;
            }
            let mut state = aware.of(ev.subject);
            state.alerted_to(ev.at);
            aware.0.insert(ev.subject, state);
        }
        if sounded.insert(on_map.0) {
            log.bad(format!("Deck {}: an alarm sounds.", crate::decks::deck_of(on_map.0)), turns.turn_number());
        }
    }
}
