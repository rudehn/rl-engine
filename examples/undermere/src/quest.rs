//! What a run is for: the quests in `assets/quests.ron`, the one fact they
//! count, and the run won when the quest marked `victory` is done.
//!
//! [`load`] reads the file into the engine's [`Quests`] and the [`Facts`]
//! its objectives count, through the engine's quest loader, as Foundry's
//! `mission.rs` reads its charges; [`report_bosses`] writes that fact
//! whenever a monster marked `boss` dies, and the engine ends the run won
//! in the same pass when the fact finishes the quest marked `victory`. The
//! win is the file's: a later part that changes what wins a run, a boss
//! drawn from a pool or a crown to carry home, changes the file rather
//! than a system that knows which death is the last.
//!
//! Loaded once, before any run, as the content is: the engine forgets a
//! run's progress when the run ends, so the next run's quest is open again
//! without being read again.

use atglyph::prelude::*;
use bevy::prelude::*;

use crate::bestiary::{Bestiary, Kind};

/// The quests file, compiled in so the binary runs from anywhere.
const QUESTS_RON: &str = include_str!("../assets/quests.ron");

/// The fact kinds Undermere's quests count.
#[derive(Resource, Debug, Clone, Copy)]
pub struct Facts {
    /// A monster marked `boss` died, about its definition's id.
    pub boss_slain: FactKind,
}

impl Facts {
    fn new() -> Self {
        let defs = Registry::from_defs(vec![FactDef::new("boss_slain")]).expect("one fact kind, named once");
        Self { boss_slain: defs.expect("boss_slain") }
    }
}

/// What an objective counts: a closed list of the facts Undermere writes,
/// so an objective naming one nothing writes is refused when the file is
/// read rather than left open for ever.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
enum On {
    /// A monster marked `boss` died, by whatever hand.
    BossSlain,
}

impl On {
    /// The matcher the tracker counts facts with.
    fn matcher(self, facts: &Facts) -> Matcher {
        match self {
            On::BossSlain => Matcher::any(facts.boss_slain),
        }
    }
}

/// Reads `quests.ron` into the engine's [`Quests`] and the facts it
/// counts. Panics naming the file and every problem in it, since a quest
/// file that does not read is a run that cannot be won.
pub fn load(registries: &Registries) -> (Quests, Facts) {
    let facts = Facts::new();
    let defs = read(QUESTS_RON, &registries.names(), &facts).unwrap_or_else(|e| panic!("assets/quests.ron: {e}"));
    (Quests::new(defs), facts)
}

/// The quests in `text`, refused with every problem at once, and refused
/// if no quest wins the run, since Undermere always has a way to win.
fn read(text: &str, names: &Names, facts: &Facts) -> Result<Registry<QuestDef>, ContentError> {
    atglyph::rules::events::quest::load_winnable(text, names, |on: &On| on.matcher(facts))
}

/// Writes [`Facts::boss_slain`] for every monster marked `boss` that dies,
/// about its definition, whoever's hand it fell to, since a boss burned to
/// death is as dead as one cut down.
///
/// The run is won the moment the boss dies: the engine feeds the fact to
/// the tracker in the same pass, after `TurnSet::React`, and ends the run
/// won there when it finishes the quest marked `victory`, so a knight's
/// blow later in the frame cannot take the win away. With no words of
/// its own: the menu's title over the ending says the king is dead, and
/// the run's record who slew him.
///
/// In `TurnSet::React`, while the dead still stand where they fell and
/// carry their [`Kind`].
pub fn report_bosses(
    mut deaths: MessageReader<DeathEvent>,
    kinds: Query<&Kind>,
    bestiary: Res<Bestiary>,
    facts: Res<Facts>,
    mut happened: MessageWriter<Happened>,
) {
    // ANCHOR: slain
    for death in deaths.read() {
        let Ok(kind) = kinds.get(death.entity) else { continue };
        if !bestiary.defs.get(kind.0).boss {
            continue;
        }
        happened.write(Happened(Fact::new(facts.boss_slain).about(u64::from(kind.0.raw()))));
    }
    // ANCHOR_END: slain
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped file reads, and its one quest is open from the start,
    /// wins the run, and asks for one boss.
    #[test]
    fn the_quest_file_reads_and_its_one_quest_wins_the_run() {
        let (quests, facts) = load(&crate::content::registries());
        assert_eq!(quests.defs.len(), 1);
        let (id, quest) = quests.defs.iter().next().expect("a quest");
        assert!(quest.victory, "slaying the king wins the run");
        assert_eq!(quests.tracker.state(id), QuestState::Open, "open from the start");
        assert_eq!(quest.objectives[0].on, Matcher::any(facts.boss_slain));
        assert_eq!(quests.tracker.progress(id, 0, &quests.defs), (0, 1), "one boss");
    }

    /// The run is won the moment the king dies: over in the very frame of
    /// the blow, before any later turn of that frame, a knight's among
    /// them, could take the win away, and not a frame later.
    #[test]
    fn the_run_is_won_in_the_frame_the_king_dies() {
        use atglyph::primitives::RunSeed;
        let mut app = crate::testing::headless(RunSeed(9));
        let me = crate::testing::arrive(&mut app, crate::dungeon::map_of(10));
        crate::testing::sturdy(&mut app, me);
        let king_id = app.world().resource::<Bestiary>().defs.expect("the Drowned King");
        let king = {
            let world = app.world_mut();
            world.query::<(Entity, &Kind)>().iter(world).find(|(_, k)| k.0 == king_id).map(|(e, _)| e).expect("the king on his throne")
        };
        let crush = app.world().resource::<Registries>().damage_kinds.expect("crush");
        app.world_mut().write_message(DamageEvent::new(king, atglyph::rules::Hit::by(me, crush, 100_000)));
        app.update();
        assert_eq!(app.world().get_resource::<Ending>().map(|e| e.outcome), Some(Outcome::Won), "won in the frame of the blow");
    }

    /// The first of the kind called `name` on the place being read.
    fn first_of(app: &mut App, name: &str) -> Entity {
        let id = app.world().resource::<Bestiary>().defs.expect(name);
        let here = crate::testing::here(app);
        let world = app.world_mut();
        let found = world.query::<(Entity, &Kind, &OnMap)>().iter(world).find(|(_, k, on)| k.0 == id && on.0 == here).map(|(e, _, _)| e);
        found.unwrap_or_else(|| panic!("a {name} on the place being read"))
    }

    /// The race the win in the king's own pass is for. A knight of his
    /// court stands at the adventurer's side, knowing them and striking
    /// every turn, and the adventurer is down to three of their forty. In
    /// one frame the adventurer fells the king and waits, and the knight's
    /// turn, later in that frame, brings a blow that kills them. The king
    /// fell first, so the run is won and its record says so; a win heard
    /// only after the frame's turns would have lost the run to that blow.
    #[test]
    fn a_blow_from_the_court_in_the_frame_the_king_falls_does_not_take_the_win_away() {
        use atglyph::primitives::RunSeed;
        use atglyph::rules::Awareness;

        use crate::testing::{arrive, at, here, lay_floor, pass_turns, settle, sturdy};
        let mut app = crate::testing::headless(RunSeed(9));
        let me = arrive(&mut app, crate::dungeon::map_of(10));
        // Sturdy only while the knight is brought to the adventurer's side
        // and takes up the fight.
        sturdy(&mut app, me);
        let beside = at(&app, me).offset(1, 0);
        lay_floor(&mut app, &[beside]);
        let knight = first_of(&mut app, "skeleton knight");
        let place = here(&app);
        app.world_mut().write_message(WarpRequest { actor: knight, to: Destination::Place { map: place, arrive: Arrive::At(beside) }, after: None });
        settle(&mut app);
        let seen = Awareness::Alert { at: at(&app, me), stale_turns: 0 };
        app.world_mut().entity_mut(knight).insert(Aware([(me, seen)].into()));
        let health = |app: &App| app.world().get::<Health>(me).map_or(0, |h| h.current);
        for _ in 0..8 {
            if health(&app) < 10_000 {
                break;
            }
            pass_turns(&mut app, 1);
        }
        assert!(health(&app) < 10_000, "the knight at the adventurer's side strikes them");
        assert!(geometry::chebyshev(at(&app, knight), at(&app, me)) == 1, "and stands beside them");

        // The frame: the adventurer down to three, the knight's next blow
        // sure to kill, the king felled by the adventurer, and a wait.
        let registries = app.world().resource::<Registries>().clone();
        let (slash, crush) = (registries.damage_kinds.expect("slash"), registries.damage_kinds.expect("crush"));
        app.world_mut().entity_mut(me).insert(Health { current: 3, max: crate::run::HEALTH });
        app.world_mut().entity_mut(knight).insert(MeleeAttack::new(slash, DiceRoll::flat(30)));
        let king = first_of(&mut app, "the Drowned King");
        app.world_mut().write_message(DamageEvent::new(king, atglyph::rules::Hit::by(me, crush, 100_000)));
        app.world_mut().write_message(Intent::new(me, Wait));
        app.update();
        assert!(health(&app) <= 0, "the knight's blow came in the frame the king fell: {}", health(&app));
        assert_eq!(app.world().get_resource::<Ending>().map(|e| e.outcome), Some(Outcome::Won), "and the run is won all the same");
        settle(&mut app);
        let record = app.world().resource::<RunRecords>().this_run.clone().expect("the win filed a record");
        assert_eq!((record.outcome, record.summary.as_str()), (RecordOutcome::Won, "Slew the Drowned King"));
    }

    /// Killing whichever of the three floor ten bosses this
    /// seed drew wins the run just as the king's own death does, since
    /// `report_bosses` reads `MonsterDef::boss` rather than a name. One
    /// seed for each of the three, found by the pure draw of the final
    /// boss, so every one is drawn and slain without an `App` for the
    /// seeds that draw a boss already played.
    #[test]
    fn killing_whichever_boss_was_drawn_wins_the_run_over_a_span_of_seeds() {
        use atglyph::primitives::RunSeed;

        use crate::dungeon::map_of;
        use crate::testing::{arrive, headless, sturdy};
        let loaded = crate::dungeon::Undermere::new(RunSeed(0));
        let mut firsts = std::collections::BTreeMap::new();
        for s in 0..60 {
            firsts.entry(loaded.reseeded(RunSeed(s)).final_boss().piece).or_insert(s);
        }
        assert_eq!(firsts.len(), 3, "a seed for each of the three final bosses within 60: {firsts:?}");
        for &s in firsts.values() {
            let mut app = headless(RunSeed(s));
            let me = arrive(&mut app, map_of(10));
            sturdy(&mut app, me);
            let boss_ids: Vec<_> = app.world().resource::<Bestiary>().defs.iter().filter(|(_, d)| d.boss).map(|(id, _)| id).collect();
            let boss = {
                let world = app.world_mut();
                world.query::<(Entity, &Kind)>().iter(world).find(|(_, k)| boss_ids.contains(&k.0)).map(|(e, _)| e).expect("a boss on the throne")
            };
            let crush = app.world().resource::<Registries>().damage_kinds.expect("crush");
            app.world_mut().write_message(DamageEvent::new(boss, atglyph::rules::Hit::by(me, crush, 100_000)));
            app.update();
            assert_eq!(app.world().get_resource::<Ending>().map(|e| e.outcome), Some(Outcome::Won), "seed {s}: won");
        }
    }

    /// A file in which nothing wins the run is refused, since a run under
    /// it could only end in death; so is an objective counting something
    /// Undermere never writes.
    #[test]
    fn a_quest_file_that_wins_nothing_or_counts_what_nobody_writes_is_refused() {
        let registries = crate::content::registries();
        let facts = Facts::new();
        let nothing_wins = r#"[(name: "rats", title: "Rats", text: "", objectives: [(text: "Kill the rat king", on: BossSlain, need: Total(1))])]"#;
        match read(nothing_wins, &registries.names(), &facts) {
            Err(ContentError::Invalid(errors)) => assert_eq!(errors, ["no quest is marked victory, so no run could be won"]),
            Err(other) => panic!("refused for the wrong reason: {other}"),
            Ok(_) => panic!("it read"),
        }
        let unknown = r#"[(name: "rats", title: "Rats", text: "", objectives: [(text: "Kill rats", on: RatSlain, need: Total(1))], victory: true)]"#;
        match read(unknown, &registries.names(), &facts) {
            Err(e) => assert!(e.to_string().contains("RatSlain"), "refused naming what it counts: {e}"),
            Ok(_) => panic!("it read"),
        }
    }
}
