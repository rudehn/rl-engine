//! The line the log gives each deck as it is entered, and the lift out.
//!
//! The lifts between decks are the engine's: the decks are a ladder,
//! [`Foundry::ladder`](crate::decks::Foundry), and the engine lays a lift
//! up on a deck's entry and a lift down on its farthest exit when the deck
//! is built. The last deck has no lift down, since there is nothing below
//! it, and deck one has no lift up: it has the [`LiftOut`] instead, where
//! the commando came in, which this lays on deck one's first arrival. It
//! leads nowhere the engine could take anyone and is answered by
//! [`ride_out`].

use bevy::prelude::*;
use rl_engine::prelude::*;

use crate::decks::{DECKS, deck_of};

/// What a lift is drawn in: the hatch's amber, so a way on reads as part
/// of the same machinery as a door.
pub const LIFT: Rgb = Rgb::new(242, 204, 89);

/// The lift out on deck one, where the commando came in: what a save
/// writes down as the lift out, and draws again as one.
///
/// Carries no `Transition`: there is nowhere in the foundry it leads, so
/// the engine refuses a `GoThrough` on it and gives the turn back, and
/// [`ride_out`] answers the refusal with the mission's own words.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct LiftOut;

impl LiftOut {
    /// How the lift out is drawn: a lift up, in the lifts' amber.
    pub fn glyph() -> Glyph {
        Glyph::new('<', Color::srgb_u8(LIFT.r, LIFT.g, LIFT.b)).on_layer(1)
    }
}

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

// ANCHOR: lifts
/// Names the deck in the log every time it is entered, and on deck one's
/// first arrival lays the lift out where the commando came in, the way up
/// the decks' ladder leaves to the game. On the run's first arrival it
/// also says how to see the keys, under the deck's own line: in
/// `run::start` it would be written before the warp lands and read as if
/// it came first.
pub fn link_decks(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, help: Res<ControlsKeys>, mut tell: MessageWriter<Tell>) {
    for ev in entered.read() {
        let deck = deck_of(ev.map);
        tell.write(Tell::new(deck_line(deck), Tones::NOTICE));
        if !ev.first || deck != 1 {
            continue;
        }
        tell.write(Tell::new(format!("Press {} for the controls.", help.toggle.label()), Tones::MUTED));
        commands.spawn((Position(ev.entry), OnMap(ev.map), LiftOut, Name::new("lift out"), LiftOut::glyph()));
    }
}
// ANCHOR_END: lifts

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
        let map = on.map(|m| m.0).unwrap_or(MapId::ZERO);
        if outs.iter().any(|(p, m)| p.0 == at.0 && m.map(|m| m.0).unwrap_or(MapId::ZERO) == map) {
            mission.answer();
        }
    }
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_core::RunSeed;

    use super::*;
    use crate::decks::map_of;
    use crate::testing::{par_seeds, seeds};

    fn player(app: &mut App) -> Entity {
        app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap()
    }

    /// Where the lift leading to `deck` stands on the current deck.
    fn lift_to(app: &mut App, deck: u32) -> Option<Point> {
        let here = app.world().resource::<WorldMap>().current();
        let mut q = app.world_mut().query::<(&Position, &OnMap, &Transition)>();
        q.iter(app.world()).find(|(_, on, t)| on.0 == here && matches!(t.to, Destination::Place { map, .. } if map == map_of(deck))).map(|(p, _, _)| p.0)
    }

    /// One way through standing on a deck: where, where it leads, and what
    /// it is, a lift of the engine's that climbs one way, or the lift out.
    type Way = (Point, Option<Destination>, Option<Climb>, bool);

    /// Every way through standing on `deck`, in the order they stand.
    fn ways_on(app: &mut App, deck: u32) -> Vec<Way> {
        let world = app.world_mut();
        let mut q = world.query::<(&Position, &OnMap, Option<&Transition>, Option<&Stair>, Has<LiftOut>)>();
        let mut ways: Vec<Way> = q
            .iter(world)
            .filter(|(_, on, t, _, out)| on.0 == map_of(deck) && (t.is_some() || *out))
            .map(|(p, _, t, stair, out)| (p.0, t.map(|t| t.to), stair.map(|s| s.climb), out))
            .collect();
        ways.sort_by_key(|(p, _, c, _)| (*p, *c));
        ways
    }

    /// What every deck should have, by the ladder: a lift up at its entry
    /// to the deck above's exit, but deck one, whose way up is the lift
    /// out; and a lift down at its exit to the deck below's entry, but the
    /// core.
    fn ladder_of(app: &App, deck: u32) -> Vec<Way> {
        let place = app.world().resource::<WorldMap>().place(map_of(deck)).expect("the deck was built");
        let mut ways = vec![match deck {
            1 => (place.entry, None, None, true),
            _ => (place.entry, Some(Destination::Place { map: map_of(deck - 1), arrive: Arrive::Exit }), Some(Climb::Up), false),
        }];
        if deck < DECKS {
            let exit = place.exit.expect("a deck above the core has a far exit");
            ways.push((exit, Some(Destination::Place { map: map_of(deck + 1), arrive: Arrive::Entry }), Some(Climb::Down), false));
        }
        ways.sort_by_key(|(p, _, c, _)| (*p, *c));
        ways
    }

    /// The run's backstop on foot: from deck one down every lift to the
    /// core, over a span of seeds, every deck the commando rides down to
    /// keeps what its build promised once its arrival has filled it. Each
    /// deck has exactly one way up and, but the core, one way down, each to
    /// the deck beside it and nothing else that leads anywhere, deck one's
    /// way up the lift out; riding the lift down lands on the next deck
    /// with a lift back up. What the commando could reach before the props
    /// were put down it can reach after, the core's console among them; every
    /// prop slot holds the prop its piece names; everything scattered lies
    /// on walkable ground; a charge deck stands one console; and every guard
    /// a post's slots name stands at its slot, a kind its role admits,
    /// holding that cell as its post, but on the cell the commando arrived
    /// on, which the engine skips. A run continued from the save the last
    /// arrival wrote has every deck's lifts again, where they stood and
    /// leading where they led, once. Each half has a pass or a plugin that
    /// owns it on a deck of its own making; this is the real decks, ridden.
    #[test]
    fn every_deck_ridden_down_to_is_joined_reachable_furnished_and_stocked_through_a_save_over_a_span_of_seeds() {
        par_seeds(seeds(0..20), |s| {
            let mut app = crate::testing::headless(RunSeed(s));
            crate::testing::settle(&mut app);
            let me = player(&mut app);
            let mut seen = Vec::new();
            for deck in 1..=DECKS {
                let at = format!("seed {s}, deck {deck}");
                assert_eq!(deck_of(app.world().resource::<WorldMap>().current()), deck, "{at}: the lift down from deck {} goes down", deck - 1);
                let ways = ways_on(&mut app, deck);
                assert_eq!(ways, ladder_of(&app, deck), "{at}");
                seen.push(ways);
                no_prop_seals_off_what_the_deck_could_reach(&mut app, deck, &at);
                every_prop_slot_holds_its_prop(&mut app, deck, &at);
                every_scattered_item_is_walkable(&mut app, deck, &at);
                if matches!(deck, 3 | 6 | 9 | 10) {
                    a_charge_deck_stands_one_console(&mut app, deck, &at);
                }
                every_guard_holds_its_post(&mut app, deck, &at);
                let Some(down) = lift_to(&mut app, deck + 1) else { continue };
                app.world_mut().get_mut::<Position>(me).unwrap().0 = down;
                crate::testing::settle(&mut app);
                app.world_mut().write_message(Intent::new(me, GoThrough));
                crate::testing::settle(&mut app);
                assert!(lift_to(&mut app, deck).is_some(), "{at}: deck {} has a lift back up", deck + 1);
            }
            assert_eq!(lift_to(&mut app, DECKS + 1), None, "seed {s}: the last deck has no lift down");
            let text = app.world().resource::<rl_engine::rl_save::Saves>().load(crate::save::SLOT).unwrap().expect("the arrival wrote the run down");
            let mut again = crate::testing::continued(&text);
            for deck in 1..=DECKS {
                assert_eq!(ways_on(&mut again, deck), seen[deck as usize - 1], "seed {s}: deck {deck}, continued");
            }
        });
    }

    /// Where the commando stands now.
    fn standing_at(app: &mut App) -> Point {
        let world = app.world_mut();
        world.query_filtered::<&Position, With<Player>>().single(world).expect("the commando stands somewhere").0
    }

    /// What the commando could reach before the props were put down, it
    /// can reach after: a crate in a doorway is a room sealed shut and
    /// everything in it lost for the run, by the same flood the game
    /// places by, in which a diagonal that cuts a corner is no way past.
    fn no_prop_seals_off_what_the_deck_could_reach(app: &mut App, deck: u32, at: &str) {
        let here = map_of(deck);
        let blockers: Vec<Point> = {
            let world = app.world_mut();
            let mut q = world.query_filtered::<(&Position, Option<&OnMap>), (With<Prop>, With<Blocks>)>();
            q.iter(world).filter(|(_, on)| on.map(|m| m.0).unwrap_or(MapId::ZERO) == here).map(|(p, _)| p.0).collect()
        };
        let from = standing_at(app);
        let map = app.world().resource::<WorldMap>();
        let (open, with_props) = (crate::props::reachable(map, from, &[]), crate::props::reachable(map, from, &blockers));
        let lost: Vec<Point> = open.difference(&with_props).filter(|p| !blockers.contains(p)).copied().collect();
        assert!(lost.is_empty(), "{at}: props cut off {} cells, first {:?}", lost.len(), lost.first());
    }

    /// Every prop slot, an armory's `A`, a store's two `L`s and a guard
    /// post's locker, holds the prop its piece names: a slot skips its prop
    /// only on the arrival cell, a cell walled over or one already filled,
    /// and none of those is ever meant to happen on a deck.
    fn every_prop_slot_holds_its_prop(app: &mut App, deck: u32, at: &str) {
        use rl_engine::rl_rules::prefab::Slot;
        let here = map_of(deck);
        let standing: Vec<(Point, PropId)> = {
            let world = app.world_mut();
            let mut q = world.query::<(&PropKind, &Position, Option<&OnMap>)>();
            q.iter(world).filter(|(.., on)| on.map_or(MapId::ZERO, |m| m.0) == here).map(|(kind, p, _)| (p.0, kind.0)).collect()
        };
        let prefabs = app.world().resource::<Prefabs<crate::droids::MonsterDef>>();
        let place = app.world().resource::<WorldMap>().place(here).expect("the deck is built");
        let mut slots = 0;
        for spot in &place.spots {
            let Some(Slot::Prop(kind)) = spot.prefab.zip(char::from_u32(spot.tag)).and_then(|(key, c)| prefabs.slot(key, c)) else { continue };
            let glyph = char::from_u32(spot.tag).unwrap_or('?');
            assert!(standing.contains(&(spot.at, *kind)), "{at}: '{glyph}' at {:?} holds no prop of its kind", spot.at);
            slots += 1;
        }
        assert!(slots >= 3, "{at}: an armory's locker and a store's two crates at least, not {slots}");
    }

    /// Everything the deck scattered lies on walkable ground, and there is
    /// something.
    fn every_scattered_item_is_walkable(app: &mut App, deck: u32, at: &str) {
        let here = map_of(deck);
        let positions: Vec<Point> = {
            let world = app.world_mut();
            let mut q = world.query_filtered::<(&Position, &OnMap), With<Item>>();
            q.iter(world).filter(|(_, on)| on.0 == here).map(|(p, _)| p.0).collect()
        };
        assert!(!positions.is_empty(), "{at}: nothing scattered");
        let map = app.world().resource::<WorldMap>();
        if let Some(p) = positions.iter().find(|p| !map.is_walkable(**p)) {
            panic!("{at}: an item at {p:?} on a wall");
        }
    }

    /// A charge deck stands exactly one console, on its reactor's mark.
    fn a_charge_deck_stands_one_console(app: &mut App, deck: u32, at: &str) {
        let world = app.world_mut();
        let console = world.resource::<Registries>().props.expect("reactor console");
        let here = map_of(deck);
        let consoles = world.query::<(&OnMap, &PropKind)>().iter(world).filter(|(on, kind)| on.0 == here && kind.0 == console).count();
        assert_eq!(consoles, 1, "{at}: one console");
    }

    /// Every guard a post's slots name was stood there, a kind its role
    /// admits, holding that cell as its post, but on the cell the commando
    /// arrived on, which the engine skips. Read by the post each guard
    /// holds rather than where it stands: the ride down spent a turn, and a
    /// sentry that saw the commando arrive has already left its cell.
    fn every_guard_holds_its_post(app: &mut App, deck: u32, at: &str) {
        use rl_engine::rl_rules::prefab::{Pick, Slot};
        let here = map_of(deck);
        let entry = app.world().resource::<WorldMap>().place(here).expect("the deck is built").entry;
        let guards: Vec<(Id<crate::droids::MonsterDef>, Post)> = {
            let world = app.world_mut();
            let mut q = world.query::<(&crate::droids::Kind, &Post, &OnMap)>();
            q.iter(world).filter(|(.., on)| on.0 == here).map(|(kind, post, _)| (kind.0, *post)).collect()
        };
        let prefabs = app.world().resource::<Prefabs<crate::droids::MonsterDef>>();
        let place = app.world().resource::<WorldMap>().place(here).expect("the deck is built");
        for spot in place.spots.iter().filter(|s| s.at != entry) {
            let glyph = char::from_u32(spot.tag).unwrap_or(' ');
            let Some(Slot::Monster { pick, .. }) = spot.prefab.and_then(|key| prefabs.slot(key, glyph)) else { continue };
            let held: Vec<Id<crate::droids::MonsterDef>> = guards.iter().filter(|(_, post)| *post == Post(spot.at)).map(|(kind, _)| *kind).collect();
            assert_eq!(held.len(), 1, "{at}: one monster posted at '{glyph}' {:?}", spot.at);
            if let Pick::Role(role) = pick {
                assert!(prefabs.roles().get(*role).fits(held[0]), "{at}: '{glyph}' holds a monster outside {:?}", prefabs.roles().get(*role).name);
            }
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
        let line = "The lift will not move until the core is charged.";
        let times: u32 = app.world().resource::<MessageLog>().iter().filter(|e| e.text == line).map(|e| e.count).sum();
        assert_eq!(times, 1, "said once for one press");
        assert_eq!(crate::testing::clock(&app), before, "no turn spent");
        assert_eq!(*app.world().resource::<State<EngineState>>().get(), EngineState::Playing, "and the run goes on");
    }

    #[test]
    fn after_the_core_the_lift_out_wins_the_run() {
        let mut app = crate::testing::headless(RunSeed(4));
        crate::testing::settle(&mut app);
        charge_everything(&mut app);
        /// How often the lift out was reported, from now on.
        #[derive(Resource, Default)]
        struct Rode(usize);
        let out = app.world().resource::<Facts>().lift_out;
        app.init_resource::<Rode>().add_systems(Last, move |mut rode: ResMut<Rode>, mut read: MessageReader<Happened>| {
            rode.0 += read.read().filter(|h| h.0.kind == out).count();
        });
        go_through(&mut app);
        crate::testing::settle(&mut app);
        assert_eq!(app.world().resource::<Rode>().0, 1, "the ride out reported once for one press");
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
