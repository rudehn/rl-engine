//! Foundry's mission: four charges, chained one deck's reactor to the
//! next, decks three, six and nine, then the core on deck ten, each
//! opening a pick of its own once its console is set.
//!
//! [`load`] turns `assets/quests.ron` into a [`Quests`] and the
//! [`Facts`] kinds it counts, through the engine's quest loader; Foundry's
//! objective vocabulary is `On::ChargeSet` and `On::LiftOut`, and the
//! engine ends the run won, with the file's `ending`, in the pass the lift
//! out is ridden. [`spawn_console_on_arrival`] plants the
//! reactor console on the `R` mark of any deck that reports one, beside the
//! loot the engine scatters on the same arrival

use bevy::prelude::*;
use rl_engine::prelude::*;

const QUESTS_RON: &str = include_str!("../assets/quests.ron");

/// The one fact kind Foundry's mission counts.
#[derive(Resource)]
pub struct Facts {
    /// `ChargeSet(deck)`: the reactor on `deck` was charged.
    pub charge_set: FactKind,
    /// `LiftOut`: the lift out on deck one was ridden, with the core charged.
    pub lift_out: FactKind,
}

impl Facts {
    fn new() -> Self {
        let defs = Registry::from_defs(vec![FactDef::new("charge_set"), FactDef::new("lift_out")]).unwrap();
        Self { charge_set: defs.expect("charge_set"), lift_out: defs.expect("lift_out") }
    }
}

/// What an objective counts: the deck whose reactor was charged, or the
/// ride out, so a deck nothing built a reactor on is a parse-time typo
/// rather than an objective that can never finish.
#[derive(Debug, Clone, serde::Deserialize)]
enum On {
    /// The reactor on this deck was charged.
    ChargeSet(u32),
    /// The lift out on deck one was ridden.
    LiftOut,
}

impl On {
    /// The matcher the tracker counts facts with.
    fn matcher(&self, facts: &Facts) -> Matcher {
        match self {
            On::ChargeSet(deck) => Matcher::any(facts.charge_set).about(*deck as u64),
            On::LiftOut => Matcher::any(facts.lift_out),
        }
    }
}

/// Loads `quests.ron` and its one fact kind. Panics listing every problem,
/// since a broken mission file is a game that cannot start.
pub fn load(registries: &Registries) -> (Quests, Facts) {
    let facts = Facts::new();
    let names = registries.names();
    let defs = tasks(QUESTS_RON, &names, &facts).unwrap_or_else(|e| panic!("assets/quests.ron: {e}"));
    (Quests::new(defs), facts)
}

/// The tasks in `text`, refused with every problem at once, and refused
/// if none wins the run, since a mission always ends somewhere.
fn tasks(text: &str, names: &Names, facts: &Facts) -> Result<Registry<QuestDef>, ContentError> {
    rl_engine::rl_rules::events::quest::load_winnable(text, names, |on: &On| on.matcher(facts))
}

/// Loads the mission fresh every run, the way `run::start` loads the
/// roster and the armory: seed-independent, but a game resource all the
/// same, so a second run never carries the first one's progress.
pub fn start(mut commands: Commands, registries: Res<Registries>) {
    let (quests, facts) = load(&registries);
    commands.insert_resource(quests);
    commands.insert_resource(facts);
}

/// The verb Foundry's console offers, declared once so [`answer_charge`]
/// finds its id.
pub const CHARGE: &str = "charge";

/// The four charge quests, in the order `quests.ron` chains them. Named
/// once here so the pick and the climb's own win condition read the same
/// list rather than each spelling it out.
pub const CHARGE_QUESTS: [&str; 4] = ["first_charge", "second_charge", "third_charge", "core_charge"];

/// The last charge, which the lift out waits on.
pub const CORE_QUEST: &str = "core_charge";

/// Puts a reactor console on the `R` mark of any deck that reports one,
/// the moment it is first entered, beside the loot the engine scatters on
/// the same arrival: reads the same
/// [`PlaceEntered`] the way that system and `droids::populate_deck` do,
/// and is unordered against both, since none of the three ever shares a
/// tile-claiming concern with either of the others.
///
/// Which decks have one is the builder's business, not this system's: it
/// asks the map rather than the deck number, so adding or moving a
/// reactor is a change to `decks.rs` alone.
pub fn spawn_console_on_arrival(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, map: Res<WorldMap>, registries: Res<Registries>) {
    for ev in entered.read() {
        if !ev.first {
            continue;
        }
        let Some(place) = map.place(ev.map) else { continue };
        let Some(spot) = place.spots.iter().find(|s| s.tag == 'R' as u32) else { continue };
        let Some(id) = registries.props.id("reactor console") else { continue };
        spawn_prop(&mut commands, &registries, id, spot.at, ev.map);
    }
}

/// What answering the charge reports: the fact for the tracker to count,
/// and the line it leaves in the log.
#[derive(bevy::ecs::system::SystemParam)]
pub struct ChargeReport<'w> {
    facts: Res<'w, Facts>,
    happened: MessageWriter<'w, Happened>,
    tell: MessageWriter<'w, Tell>,
}

// ANCHOR: charge
/// Answers the `charge` verb: reports the fact the tracker counts, says
/// so, and leaves the console reading as spent.
///
/// The engine has already decided the charge was possible, spent the
/// three turns the offer asked for, and landed whatever effects the offer
/// carried, which for this one is none. What is left is the part no
/// effect could express: a fact about this run. The console then becomes
/// a different kind of prop, one with no offer and a duller glyph, which
/// is how it stops being something to charge twice.
pub fn answer_charge(
    mut commands: Commands,
    mut done: MessageReader<Interacted>,
    verbs: Res<Verbs>,
    registries: Res<Registries>,
    mut report: ChargeReport,
    consoles: Query<(&PropKind, Option<&OnMap>)>,
) {
    let Some(charge) = verbs.get(CHARGE) else { return };
    let Some(spent) = registries.props.id("spent reactor console") else { return };
    for ev in done.read() {
        if ev.verb != charge {
            continue;
        }
        let Ok((_, on)) = consoles.get(ev.target) else { continue };
        let deck = crate::decks::deck_of(on.map(|m| m.0).unwrap_or(MapId::ZERO));
        // The glyph comes off with the kind, so the renderer dresses it
        // again from the definition of a console already used.
        commands.entity(ev.target).remove::<Glyph>().insert(PropKind(spent));
        report.happened.write(Happened(Fact::new(report.facts.charge_set).about(u64::from(deck))));
        report.tell.write(Tell::new("You set the charge. The reactor stirs.", Tones::GOOD));
    }
}
// ANCHOR_END: charge

/// Reacts to the tracker's own report of a charge quest finishing: never
/// a mere `Progress`, and never a quest that is not one of the four
/// charges, so a later mission of any kind cannot open this pick. The
/// tracker reports it in the pass the fact landed, and this reads it at
/// the head of the next frame, a plain `Update` system rather than
/// anything in `TurnSet`: a screen opens between frames, not between
/// passes, and nothing here is itself a reaction to a turn. Runs before
/// `EngineSet::Input`, so the key handlers find the pick open.
///
/// The mission is loaded as a run begins, so on the title before the
/// first run there is none, and nothing to answer.
pub fn offer_the_pick(
    mut changes: MessageReader<QuestChange>,
    quests: Option<Res<Quests>>,
    mut modals: ResMut<Modals>,
    mut screen: ResMut<crate::upgrades::ChoiceScreen>,
    mut choosing: ResMut<crate::upgrades::Choosing>,
    taken: Res<crate::upgrades::Taken>,
) {
    let Some(quests) = quests else { return };
    let charges = CHARGE_QUESTS.map(|name| quests.defs.expect(name));
    for change in changes.read() {
        if let Change::QuestDone { quest, .. } = change.0
            && charges.contains(&quest)
        {
            crate::upgrades::offer(&mut modals, &mut screen, &mut choosing, &taken);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rl_engine::rl_core::RunSeed;

    /// Whatever the console offers the player, and to whom.
    fn charge_offered(app: &App, player: Entity) -> Option<Offer> {
        let verb = app.world().resource::<Verbs>().get(CHARGE)?;
        app.world().resource::<OfferedHere>().for_actor(player).iter().find(|o| o.verb == verb).cloned()
    }

    #[test]
    fn setting_the_charge_takes_three_turns_and_completes_the_quest() {
        let mut app = crate::testing::headless(RunSeed(2));
        let player = crate::testing::beside_the_console(&mut app);
        crate::testing::settle(&mut app);
        let offer = charge_offered(&app, player).expect("standing beside it, the console offers its charge");
        // A difference, not an absolute reading: the clock already carries
        // whatever it cost to reach deck three at all (a real run's own
        // stair transitions, once Foundry has them, are `GoThrough`'s
        // normal `BASE_ACTION_COST`), and the engine only promises the
        // player's first turn precedes every monster's, never that nothing
        // moves before this one action does.
        let before = crate::testing::clock(&app);
        app.world_mut().write_message(Intent::new(player, Interact { target: offer.target, verb: offer.verb, thing: None }));
        crate::testing::settle(&mut app);
        assert_eq!(crate::testing::clock(&app) - before, 300, "a charge takes three turns to set");
        assert!(crate::testing::quest_done(&app, "first_charge"));
        assert!(app.world().resource::<crate::upgrades::Choosing>().0.is_some(), "and the choice is offered");
        // The console itself is now the spent kind, which is a kind with no
        // offers, so nothing can charge it again. Read off the prop rather
        // than off the gate: the gate answers per pass, and with the pick
        // up no pass is run.
        let spent = app.world().resource::<Registries>().props.expect("spent reactor console");
        assert_eq!(app.world().get::<PropKind>(offer.target).map(|k| k.0), Some(spent), "and a console already charged is a spent one");
    }

    /// Deck one has no console, so nothing offers a charge: the engine
    /// answers "there is nothing here to do" before a turn is spent, which
    /// is what replaced this game's own refusal message.
    #[test]
    fn there_is_no_charge_to_set_where_there_is_no_console() {
        let mut app = crate::testing::headless(RunSeed(3));
        crate::testing::settle(&mut app);
        let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        assert!(charge_offered(&app, player).is_none(), "deck one offers no charge");
        assert!(!crate::testing::quest_done(&app, "first_charge"));
    }

    /// The four charges are one chain: each opens only once the one above
    /// it is done, so a commando cannot charge the core first by riding
    /// the lifts straight down.
    #[test]
    fn the_core_charge_stays_shut_until_the_three_reactors_above_it_are_charged() {
        let mut app = crate::testing::headless(RunSeed(2));
        crate::testing::settle(&mut app);
        let quests = app.world().resource::<Quests>();
        assert_eq!(quests.tracker.state(quests.defs.expect("first_charge")), QuestState::Open, "the first charge is open from the start");
        assert_eq!(quests.tracker.state(quests.defs.expect("core_charge")), QuestState::Locked, "the core waits on the three above it");
    }
}
