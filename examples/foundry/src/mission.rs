//! The first slice's one task: set a charge on the reactor console, deck
//! three, then pick an upgrade.
//!
//! [`load`] turns `assets/quests.ron` into a [`Quests`] and the one
//! [`Facts`] kind it counts, the way `examples/corsair/src/quests.rs`
//! does for its own, larger vocabulary; Foundry's objective vocabulary is
//! one variant, `On::ChargeSet`. [`spawn_console_on_arrival`] plants
//! [`Console`] at deck three's `R` mark, beside the loot Task 9 scatters
//! on the same first arrival. [`resolve_set_charge`] is [`SetCharge`]'s
//! resolver: adjacent to an unset console, it spends three whole turns
//! and reports the fact; anywhere else, it fails for one. [`offer_the_pick`]
//! is the hinge to `upgrades`: the moment the tracker reports the mission
//! done, it opens the pick and nothing else in this module ever touches
//! an upgrade directly.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_core::Id;

const QUESTS_RON: &str = include_str!("../assets/quests.ron");

/// How long setting the charge takes, once adjacent to an unset console:
/// three whole turns.
const CHARGE_COST: u32 = 3 * BASE_ACTION_COST;

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

/// Marks the reactor console entity `spawn_console_on_arrival` plants at
/// deck three's `R` mark.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Console;

/// A console `resolve_set_charge` has already charged. Standing beside it
/// again sets nothing more: the objective only ever needs one.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Spent;

/// Spend a charge on the reactor console beside you. The player's own
/// action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetCharge;
impl Action for SetCharge {}

/// Plants [`Console`] at deck three's `R` mark the moment it is first
/// entered, beside the loot `loot::scatter_on_arrival` plants on the same
/// arrival: reads the same [`PlaceEntered`] the way that system and
/// `droids::populate_deck` do, and is unordered against both, since none
/// of the three ever shares a tile-claiming concern with either of the
/// others.
pub fn spawn_console_on_arrival(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, map: Res<WorldMap>) {
    for ev in entered.read() {
        if !ev.first || crate::decks::deck_of(ev.map) != 3 {
            continue;
        }
        let Some(place) = map.place(ev.map) else { continue };
        let Some(spot) = place.spots.iter().find(|s| s.tag == 'R' as u32) else { continue };
        commands.spawn((Console, Position(spot.at), OnMap(ev.map), Name::new("reactor console"), Glyph::new('R', Color::srgb(0.95, 0.65, 0.25)).on_layer(2)));
    }
}

/// One console, as [`resolve_set_charge`] reads it: where it is, which
/// map it is on, and whether it has already been spent.
type ConsoleRow<'w> = (Entity, &'w Position, Option<&'w OnMap>, Has<Spent>);

/// What resolving [`SetCharge`] reads: the actor holding the turn, and
/// every console on its map that has not already been spent.
#[derive(bevy::ecs::system::SystemParam)]
pub struct ChargeWorld<'w, 's> {
    holders: Query<'w, 's, (&'static Position, Option<&'static OnMap>), With<MyTurn>>,
    consoles: Query<'w, 's, ConsoleRow<'static>, With<Console>>,
}

/// What resolving [`SetCharge`] reports: the fact the charge set for the
/// tracker to count, and the line it leaves in the log. Its own param
/// struct rather than four more arguments on `resolve_set_charge`, the
/// way `loot::Layout` exists so `plan_scatter` stays under clippy's own
/// limit on a function's parameter list.
#[derive(bevy::ecs::system::SystemParam)]
pub struct ChargeReport<'w> {
    facts: Res<'w, Facts>,
    turns: Res<'w, Turns>,
    happened: MessageWriter<'w, Happened>,
    log: ResMut<'w, MessageLog>,
}

/// Resolves [`SetCharge`]: adjacent to an unspent [`Console`] on the
/// actor's own map, it charges `CHARGE_COST` (three whole turns),
/// spends the console, and reports [`Facts::charge_set`] through
/// [`Happened`] for the tracker to count. Anywhere else, or a console
/// already spent, fails at [`BASE_ACTION_COST`] with a log line and
/// nothing to show for it: the rule every resolver in this game follows,
/// spelled out in `crates/rl-bevy/src/turn.rs`'s own doc on
/// [`Resolution::failed`].
pub fn resolve_set_charge(
    mut commands: Commands,
    mut intents: MessageReader<Intent<SetCharge>>,
    mut resolution: Resolution,
    world: ChargeWorld,
    mut report: ChargeReport,
) {
    for intent in intents.read() {
        if !resolution.claim(intent.actor) {
            continue;
        }
        let found = world.holders.get(intent.actor).ok().and_then(|(pos, on)| {
            let map = on.map(|m| m.0).unwrap_or(MapId::SURFACE);
            world.consoles.iter().find(|(_, cp, cm, spent)| !spent && cm.map(|m| m.0).unwrap_or(MapId::SURFACE) == map && geometry::is_adjacent(pos.0, cp.0))
        });
        let Some((console, _, cm, _)) = found else {
            report.log.bad("There is nothing here to set a charge on.", report.turns.turn_number());
            resolution.failed(intent.actor, BASE_ACTION_COST);
            continue;
        };
        commands.entity(console).insert(Spent);
        let deck = crate::decks::deck_of(cm.map(|m| m.0).unwrap_or(MapId::SURFACE));
        report.happened.write(Happened(Fact::new(report.facts.charge_set).about(deck as u64)));
        report.log.good("You set the charge. The reactor stirs.", report.turns.turn_number());
        resolution.done(intent.actor, CHARGE_COST);
    }
}

/// Reacts to the tracker's own report of the mission finishing: never a
/// mere `Progress`, and never any quest but `first_charge`, since a later
/// slice's own mission must not open this same pick a second time. One
/// frame behind the fact that finished it (`rl_bevy::events`'s own doc on
/// [`QuestChange`]), which is why this is a plain `Update` system rather
/// than anything in `TurnSet`: nothing here is itself a reaction to a
/// turn, only to what the tracker made of one after the fact.
pub fn offer_the_pick(
    mut changes: MessageReader<QuestChange>,
    quests: Res<Quests>,
    mut modals: ResMut<Modals>,
    mut screen: ResMut<crate::upgrades::ChoiceScreen>,
    mut choosing: ResMut<crate::upgrades::Choosing>,
) {
    let first_charge = quests.defs.expect("first_charge");
    for change in changes.read() {
        if let Change::QuestDone { quest, .. } = change.0
            && quest == first_charge
        {
            crate::upgrades::offer(&mut modals, &mut screen, &mut choosing);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rl_engine::rl_core::RunSeed;

    #[test]
    fn setting_the_charge_takes_three_turns_and_completes_the_quest() {
        let mut app = crate::testing::headless(RunSeed(2));
        let player = crate::testing::beside_the_console(&mut app);
        // A difference, not an absolute reading: the clock already carries
        // whatever it cost to reach deck three at all (a real run's own
        // stair transitions, once Foundry has them, are `GoThrough`'s
        // normal `BASE_ACTION_COST`), and `run::admit_the_player`'s own
        // fix only guarantees the player's first turn precedes every
        // monster's, never that nothing moves before this one action does.
        let before = crate::testing::clock(&app);
        app.world_mut().write_message(Intent::new(player, SetCharge));
        crate::testing::settle(&mut app);
        assert_eq!(crate::testing::clock(&app) - before, 300, "a charge takes three turns to set");
        assert!(crate::testing::quest_done(&app, "first_charge"));
        assert!(app.world().resource::<crate::upgrades::Choosing>().0.is_some(), "and the choice is offered");
    }

    #[test]
    fn setting_a_charge_with_nothing_beside_you_reports_nothing() {
        let mut app = crate::testing::headless(RunSeed(3));
        crate::testing::settle(&mut app);
        let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        app.world_mut().write_message(Intent::new(player, SetCharge));
        crate::testing::settle(&mut app);
        assert!(!crate::testing::quest_done(&app, "first_charge"), "deck one has no console to charge");
    }
}
