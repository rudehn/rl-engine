//! The lifts between decks, and the line the log gives each deck as it is
//! entered.
//!
//! A deck's builder reports an entry and a farthest exit; this puts a lift
//! up on the entry and a lift down on the exit the first time the deck is
//! entered, the way delve's stairs are laid, so the way on is always as
//! far from where the commando arrived as the deck allows. The last deck
//! has no lift down, since there is nothing below it, and deck one has no
//! lift up: it has the [`LiftOut`] instead, where the commando came in,
//! which leads nowhere the engine could take anyone and is answered by
//! [`ride_out`].

use bevy::prelude::*;
use rl_engine::prelude::*;

use crate::decks::{DECKS, deck_of, map_of};

/// What a lift is drawn in: the hatch's amber, so a way on reads as part
/// of the same machinery as a door.
pub const LIFT: Color = Color::srgb(0.95, 0.8, 0.35);

/// A lift between decks, or the lift out: what a save writes down as a
/// lift and draws again as one.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Lift;

/// The lift out on deck one, where the commando came in.
///
/// Carries no `Transition`: there is nowhere in the foundry it leads, so
/// the engine refuses a `GoThrough` on it and gives the turn back, and
/// [`ride_out`] answers the refusal with the mission's own words.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct LiftOut;

/// What the log says on entering `deck`: its name, and what its light
/// means for the commando.
pub fn deck_line(deck: u32) -> String {
    match deck {
        1 => "Deck 1: the upper assembly hall. The work lights are still on.".to_string(),
        2 => "Deck 2: the lower assembly hall. The lights are out down here.".to_string(),
        3 => "Deck 3: the first reactor deck, dark. Find the console and set the charge.".to_string(),
        4 | 5 => format!("Deck {deck}: fabrication. Furnace glow, and the air is worse."),
        6 => "Deck 6: the second reactor deck.".to_string(),
        7 | 8 => format!("Deck {deck}: the reactor ring. Nothing down here was built for people."),
        9 => "Deck 9: the third reactor deck.".to_string(),
        DECKS => "Deck 10: the core.".to_string(),
        n => format!("Deck {n}."),
    }
}

/// Lays a deck's lifts the first time it is entered, and names the deck in
/// the log every time. On the run's first arrival it also says how to see
/// the keys, under the deck's own line: in `run::start` it would be
/// written before the warp lands and read as if it came first.
pub fn link_decks(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, help: Res<ControlsKeys>, mut tell: MessageWriter<Tell>) {
    for ev in entered.read() {
        let deck = deck_of(ev.map);
        tell.write(Tell::new(deck_line(deck), Tones::NOTICE));
        if !ev.first {
            continue;
        }
        if deck == 1 {
            tell.write(Tell::new(format!("Press {} for the controls.", help.toggle.label()), Tones::MUTED));
        }
        if deck > 1 {
            let up = Transition { to: Destination::Place { map: map_of(deck - 1), arrive: Arrive::Exit } };
            commands.spawn((Position(ev.entry), OnMap(ev.map), Lift, up, Name::new("lift up"), Glyph::new('<', LIFT).on_layer(1)));
        } else {
            // The way out, where the commando came in.
            commands.spawn((Position(ev.entry), OnMap(ev.map), Lift, LiftOut, Name::new("lift out"), Glyph::new('<', LIFT).on_layer(1)));
        }
        if let Some(exit) = ev.exit.filter(|_| deck < DECKS) {
            let down = Transition { to: Destination::Place { map: map_of(deck + 1), arrive: Arrive::Entry } };
            commands.spawn((Position(exit), OnMap(ev.map), Lift, down, Name::new("lift down"), Glyph::new('>', LIFT).on_layer(1)));
        }
    }
}

/// The player, where it stands, for [`ride_out`].
type Rider<'w, 's> = Query<'w, 's, (Entity, &'static Position, Option<&'static OnMap>), With<Player>>;

/// What riding the lift out answers with: a line when the core is not
/// charged, and the fact the last quest counts when it is.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Mission<'w> {
    quests: Res<'w, Quests>,
    facts: Res<'w, crate::mission::Facts>,
    happened: MessageWriter<'w, Happened>,
    tell: MessageWriter<'w, Tell>,
}

impl Mission<'_> {
    fn answer(&mut self) {
        let core = self.quests.defs.expect(crate::mission::CORE_QUEST);
        if self.quests.tracker.state(core) == QuestState::Done {
            self.happened.write(Happened(Fact::new(self.facts.lift_out)));
        } else {
            self.tell.write(Tell::new("The lift will not move until the core is charged.", Tones::MUTED));
        }
    }
}

/// Answers a `GoThrough` on the lift out, which the engine has already
/// refused and given the turn back for.
///
/// Before the core is charged it says so; after, it reports the fact the
/// last quest counts, and finishing that quest is what wins. Reads the
/// pass's own `Intent<GoThrough>` beside the refusal, because a refusal
/// does not say what was refused, and the player may stand on the lift out
/// when something else of theirs is refused.
pub fn ride_out(
    mut refused: MessageReader<ActionRefused>,
    mut going: MessageReader<Intent<GoThrough>>,
    rider: Rider,
    outs: Query<(&Position, Option<&OnMap>), With<LiftOut>>,
    mut mission: Mission,
) {
    let asked: Vec<Entity> = going.read().map(|i| i.actor).collect();
    for r in refused.read() {
        let Ok((me, at, on)) = rider.get(r.actor) else { continue };
        if !asked.contains(&me) {
            continue;
        }
        let map = on.map(|m| m.0).unwrap_or(MapId::SURFACE);
        if outs.iter().any(|(p, m)| p.0 == at.0 && m.map(|m| m.0).unwrap_or(MapId::SURFACE) == map) {
            mission.answer();
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

#[cfg(test)]
mod lift_out {
    use rl_engine::rl_bevy::{Ending, Outcome};
    use rl_engine::rl_core::{Direction, RunSeed};

    use super::*;
    use crate::mission::Facts;

    fn me(app: &mut App) -> Entity {
        app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap()
    }

    fn go_through(app: &mut App) {
        let player = me(app);
        app.world_mut().write_message(Intent::new(player, GoThrough));
        crate::testing::settle(app);
    }

    fn said(app: &App, line: &str) -> bool {
        app.world().resource::<MessageLog>().iter().any(|e| e.text == line)
    }

    /// Sets every charge, the way the consoles report them.
    fn charge_everything(app: &mut App) {
        let kind = app.world().resource::<Facts>().charge_set;
        for deck in [3u64, 6, 9, 10] {
            app.world_mut().write_message(Happened(Fact::new(kind).about(deck)));
            app.update();
        }
    }

    #[test]
    fn the_lift_out_stands_where_the_commando_came_in() {
        let mut app = crate::testing::headless(RunSeed(4));
        crate::testing::settle(&mut app);
        let player = me(&mut app);
        let at = app.world().get::<Position>(player).unwrap().0;
        let outs: Vec<Point> = app.world_mut().query_filtered::<&Position, With<LiftOut>>().iter(app.world()).map(|p| p.0).collect();
        assert_eq!(outs, [at], "one lift out, under the commando's feet at the start");
    }

    #[test]
    fn before_the_core_the_lift_out_refuses_with_a_line_and_costs_no_turn() {
        let mut app = crate::testing::headless(RunSeed(4));
        crate::testing::settle(&mut app);
        let before = crate::testing::clock(&app);
        go_through(&mut app);
        assert!(said(&app, "The lift will not move until the core is charged."));
        assert_eq!(crate::testing::clock(&app), before, "no turn spent");
        assert_eq!(*app.world().resource::<State<EngineState>>().get(), EngineState::Playing, "and the run goes on");
    }

    #[test]
    fn after_the_core_the_lift_out_wins_the_run() {
        let mut app = crate::testing::headless(RunSeed(4));
        crate::testing::settle(&mut app);
        charge_everything(&mut app);
        go_through(&mut app);
        crate::testing::settle(&mut app);
        assert_eq!(*app.world().resource::<State<EngineState>>().get(), EngineState::Over, "the run is over");
        assert!(app.world().get_resource::<Ending>().is_some_and(|e| matches!(e.outcome, Outcome::Won)), "and won");
    }

    /// Only the lift out answers: going through open floor beside it is the
    /// engine's refusal and nothing more.
    #[test]
    fn going_through_open_floor_is_the_engines_refusal_alone() {
        let mut app = crate::testing::headless(RunSeed(4));
        crate::testing::settle(&mut app);
        let player = me(&mut app);
        let at = app.world().get::<Position>(player).unwrap().0;
        let dir = Direction::ALL
            .into_iter()
            .find(|d| {
                let (dx, dy) = d.delta();
                let p = at.offset(dx, dy);
                app.world().resource::<WorldMap>().is_walkable(p) && !app.world().resource::<Occupancy>().is_occupied(p)
            })
            .expect("somewhere to step");
        app.world_mut().write_message(Intent::new(player, Step(dir)));
        crate::testing::settle(&mut app);
        assert_ne!(app.world().get::<Position>(player).unwrap().0, at, "stepped off the lift");
        go_through(&mut app);
        assert!(!said(&app, "The lift will not move until the core is charged."), "only the lift out answers");
    }
}
