//! The lifts between decks, and the line the log gives each deck as it is
//! entered.
//!
//! A deck's builder reports an entry and a farthest exit; this puts a lift
//! up on the entry and a lift down on the exit the first time the deck is
//! entered, the way delve's stairs are laid, so the way on is always as
//! far from where the commando arrived as the deck allows. Deck one has no
//! lift up, since the slice has nothing above it, and the last deck has no
//! lift down, since the slice has nothing below it.

use bevy::prelude::*;
use rl_engine::prelude::*;

use crate::decks::{DECKS, deck_of, map_of};

/// What a lift is drawn in: the hatch's amber, so a way on reads as part
/// of the same machinery as a door.
const LIFT: Color = Color::srgb(0.95, 0.8, 0.35);

/// What the log says on entering `deck`: its name, and what its light
/// means for the commando.
pub fn deck_line(deck: u32) -> String {
    match deck {
        1 => "Deck 1: the upper assembly hall. The work lights are still on.".to_string(),
        2 => "Deck 2: the lower assembly hall. The lights are out down here.".to_string(),
        DECKS => format!("Deck {DECKS}: the reactor deck, dark. Find the console and set the charge."),
        n => format!("Deck {n}."),
    }
}

/// Lays a deck's lifts the first time it is entered, and names the deck in
/// the log every time. On the run's first arrival it also says how to see
/// the keys, under the deck's own line: in `run::start` it would be
/// written before the warp lands and read as if it came first.
pub fn link_decks(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, turns: Res<Turns>, help: Res<ControlsKeys>, mut log: ResMut<MessageLog>) {
    for ev in entered.read() {
        let deck = deck_of(ev.map);
        log.notice(deck_line(deck), turns.turn_number());
        if !ev.first {
            continue;
        }
        if deck == 1 {
            log.muted(format!("Press {} for the controls.", help.toggle.label()), turns.turn_number());
        }
        if deck > 1 {
            let up = Transition { to: Destination::Place { map: map_of(deck - 1), arrive: Arrive::Exit } };
            commands.spawn((Position(ev.entry), OnMap(ev.map), up, Name::new("lift up"), Glyph::new('<', LIFT).on_layer(1)));
        }
        if let Some(exit) = ev.exit.filter(|_| deck < DECKS) {
            let down = Transition { to: Destination::Place { map: map_of(deck + 1), arrive: Arrive::Entry } };
            commands.spawn((Position(exit), OnMap(ev.map), down, Name::new("lift down"), Glyph::new('>', LIFT).on_layer(1)));
        }
    }
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_core::RunSeed;

    use super::*;

    fn player(app: &mut App) -> Entity {
        app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap()
    }

    /// Where the lift leading to `deck` stands on the current deck.
    fn lift_to(app: &mut App, deck: u32) -> Option<Point> {
        let here = app.world().resource::<WorldMap>().current();
        let mut q = app.world_mut().query::<(&Position, &OnMap, &Transition)>();
        q.iter(app.world()).find(|(_, on, t)| on.0 == here && matches!(t.to, Destination::Place { map, .. } if map == map_of(deck))).map(|(p, _, _)| p.0)
    }

    #[test]
    fn going_through_each_lift_down_reaches_the_next_deck_and_the_last_has_none_over_a_span_of_seeds() {
        for s in 0..6 {
            let mut app = crate::testing::headless(RunSeed(s));
            crate::testing::settle(&mut app);
            let me = player(&mut app);
            assert_eq!(lift_to(&mut app, 0), None, "seed {s}: deck one has no lift up");
            for deck in 1..DECKS {
                let down = lift_to(&mut app, deck + 1).unwrap_or_else(|| panic!("seed {s}: deck {deck} has a lift down"));
                app.world_mut().get_mut::<Position>(me).unwrap().0 = down;
                crate::testing::settle(&mut app);
                app.world_mut().write_message(Intent::new(me, GoThrough));
                crate::testing::settle(&mut app);
                assert_eq!(deck_of(app.world().resource::<WorldMap>().current()), deck + 1, "seed {s}: the lift on deck {deck} goes down");
                assert!(lift_to(&mut app, deck).is_some(), "seed {s}: deck {} has a lift back up", deck + 1);
            }
            assert_eq!(lift_to(&mut app, DECKS + 1), None, "seed {s}: the last deck has no lift down");
        }
    }

    #[test]
    fn the_log_names_the_first_deck_and_then_says_how_to_see_the_keys() {
        let mut app = crate::testing::headless(RunSeed(1));
        crate::testing::settle(&mut app);
        let lines: Vec<String> = app.world().resource::<MessageLog>().iter().map(|e| e.text.clone()).collect();
        let at = |line: &str| lines.iter().position(|l| l.starts_with(line)).unwrap_or_else(|| panic!("{line:?} not in {lines:#?}"));
        assert!(at("Deck 1:") < at("Press ? for the controls."), "{lines:#?}");
    }
}
