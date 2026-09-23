//! Foundry's mission: four charges, chained one deck's reactor to the
//! next, decks three, six and nine, then the core on deck ten, each
//! opening a pick of its own once its console is set.
//!
//! [`load`] turns `assets/quests.ron` into a [`Quests`] and the one
//! [`Facts`] kind it counts, the way `examples/corsair/src/quests.rs`
//! does for its own, larger vocabulary; Foundry's objective vocabulary is
//! one variant, `On::ChargeSet`. [`spawn_console_on_arrival`] plants the
//! reactor console on the `R` mark of any deck that reports one, beside the
//! loot `loot::scatter_on_arrival` plants on the same arrival

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_core::Id;

const QUESTS_RON: &str = include_str!("../assets/quests.ron");

/// The one fact kind Foundry's mission counts.
#[derive(Resource)]
pub struct Facts {
    /// `ChargeSet(deck)`: the reactor on `deck` was charged.
    pub charge_set: FactKind,
}

impl Facts {
    fn new() -> Self {
        let defs = Registry::from_defs(vec![FactDef::new("charge_set")]).unwrap();
        Self { charge_set: defs.expect("charge_set") }
    }
}

/// A task as authored in `quests.ron`.
#[derive(Debug, Clone, serde::Deserialize)]
struct QuestRon {
    name: String,
    title: String,
    text: String,
    #[serde(default)]
    after: Vec<String>,
    objectives: Vec<ObjectiveRon>,
    #[serde(default)]
    victory: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ObjectiveRon {
    text: String,
    on: On,
    need: Need,
}

/// What an objective counts. One variant for the whole slice: the deck
/// whose reactor was charged, so a deck nothing built a reactor on is a
/// parse-time typo rather than an objective that can never finish.
#[derive(Debug, Clone, serde::Deserialize)]
enum On {
    /// The reactor on this deck was charged.
    ChargeSet(u32),
}

impl On {
    /// The matcher the tracker counts facts with.
    fn matcher(&self, facts: &Facts) -> Matcher {
        let On::ChargeSet(deck) = self;
        Matcher::any(facts.charge_set).about(*deck as u64)
    }
}

impl Named for QuestRon {
    fn name(&self) -> &str {
        &self.name
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

/// The tasks in `text`, the way `examples/corsair/src/quests.rs`'s own
/// `tasks` builds theirs.
fn tasks(text: &str, names: &Names, facts: &Facts) -> Result<Registry<QuestDef>, ContentError> {
    let authored: Registry<QuestRon> = names.load(text)?;
    authored.validate(|q, all| {
        if let Some(a) = q.after.iter().find(|a| all.id(a).is_none()) {
            return Err(format!("after unknown quest {a:?}"));
        }
        if q.objectives.is_empty() {
            return Err("no objectives".into());
        }
        Ok(())
    })?;
    let defs = authored
        .iter()
        .map(|(_, q)| QuestDef {
            name: q.name.clone(),
            title: q.title.clone(),
            text: q.text.clone(),
            after: q.after.iter().map(|a| Id::from_raw(authored.expect(a).raw())).collect(),
            objectives: q.objectives.iter().map(|o| Objective { text: o.text.clone(), on: o.on.matcher(facts), need: o.need }).collect(),
            victory: q.victory,
        })
        .collect();
    Registry::from_defs(defs)
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

/// Puts a reactor console on the `R` mark of any deck that reports one,
/// the moment it is first entered, beside the loot
/// `loot::scatter_on_arrival` plants on the same arrival: reads the same
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
        let Ok((_, on)) = consoles.get(ev.prop) else { continue };
        let deck = crate::decks::deck_of(on.map(|m| m.0).unwrap_or(MapId::SURFACE));
        // The glyph comes off with the kind, so the renderer dresses it
        // again from the definition of a console already used.
        commands.entity(ev.prop).remove::<Glyph>().insert(PropKind(spent));
        report.happened.write(Happened(Fact::new(report.facts.charge_set).about(u64::from(deck))));
        report.tell.write(Tell::new("You set the charge. The reactor stirs.", Tones::GOOD));
    }
}
// ANCHOR_END: charge

/// Reacts to the tracker's own report of a charge quest finishing: never
/// a mere `Progress`, and never a quest that is not one of the four
/// charges, so a later mission of any kind cannot open this pick. One
/// frame behind the fact that finished it (`rl_bevy::events`'s own doc on
/// [`QuestChange`]), which is why this is a plain `Update` system rather
/// than anything in `TurnSet`: nothing here is itself a reaction to a
/// turn, only to what the tracker made of one after the fact. Runs before
/// `EngineSet::Input`, so the key handlers find the pick open.
pub fn offer_the_pick(
    mut changes: MessageReader<QuestChange>,
    quests: Res<Quests>,
    mut modals: ResMut<Modals>,
    mut screen: ResMut<crate::upgrades::ChoiceScreen>,
    mut choosing: ResMut<crate::upgrades::Choosing>,
    taken: Res<crate::upgrades::Taken>,
) {
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
        app.world().resource::<OfferedHere>().for_actor(player).iter().find(|o| o.verb == verb).copied()
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
        app.world_mut().write_message(Intent::new(player, Interact { prop: offer.prop, verb: offer.verb }));
        crate::testing::settle(&mut app);
        assert_eq!(crate::testing::clock(&app) - before, 300, "a charge takes three turns to set");
        assert!(crate::testing::quest_done(&app, "first_charge"));
        assert!(app.world().resource::<crate::upgrades::Choosing>().0.is_some(), "and the choice is offered");
        // The console itself is now the spent kind, which is a kind with no
        // offers, so nothing can charge it again. Read off the prop rather
        // than off the gate: the gate answers per pass, and with the pick
        // up no pass is run.
        let spent = app.world().resource::<Registries>().props.expect("spent reactor console");
        assert_eq!(app.world().get::<PropKind>(offer.prop).map(|k| k.0), Some(spent), "and a console already charged is a spent one");
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

    /// Every charge deck gets its console on the arrival that builds it,
    /// which is what makes one system serve four reactors instead of the
    /// one deck three used to name.
    #[test]
    fn every_charge_deck_stands_a_console_on_its_reactor_mark_over_a_span_of_seeds() {
        for s in 0..4u64 {
            for deck in [3u32, 6, 9, 10] {
                let mut app = crate::testing::headless(RunSeed(s));
                crate::testing::arrive_on(&mut app, deck);
                let world = app.world_mut();
                let console_id = world.resource::<Registries>().props.expect("reactor console");
                let here = crate::decks::map_of(deck);
                let mut q = world.query::<(&OnMap, &PropKind)>();
                let consoles = q.iter(world).filter(|(on, kind)| on.0 == here && kind.0 == console_id).count();
                assert_eq!(consoles, 1, "seed {s}, deck {deck}: one console");
            }
        }
    }
}
