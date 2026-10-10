//! The run's story in numbers, and the score it comes to.
//!
//! [`Tally`] is kept as the run goes, each count by a system reading what
//! a turn did in `TurnSet::React`: [`count_the_fighting`] the kills, their
//! threat, the damage and the mercenaries who fell, [`count_what_is_found`]
//! the gold and the things taken up and the potions drunk, and
//! [`remember_depth`] the deepest floor. It is saved with the run and put
//! back to nought when the run ends.
//!
//! [`score`] is the design's five lines and [`total`] what they come to;
//! [`record`] writes the run down once it is over, as one [`RecordRun`]
//! carrying the line the records list shows and its sections, the score
//! line by line, the run's numbers, and, when any fell, the mercenaries
//! lost, each a [`FallenMercenary`]. The end screen and the records screen
//! both draw that one record, so the two never disagree.

use std::collections::BTreeMap;

use bevy::ecs::message::MessageCursor;
use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_core::noun::with_article;
use serde::{Deserialize, Serialize};

use crate::bestiary::{Bestiary, Kind};
use crate::dungeon::floor_of;
use crate::mercenaries::Mercenary;
use crate::run::Adventurer;

/// Points for each floor reached, counted by the deepest.
pub const DEPTH_POINTS: i64 = 100;

/// Points for each mini-boss slain.
pub const MINIBOSS_POINTS: i64 = 500;

/// Points for a run won.
pub const VICTORY_POINTS: i64 = 5000;

/// What the run has done so far, for its score and its record.
///
/// A resource rather than the engine's `Counters`, since two of its
/// numbers are not counts: the threat slain is a sum of fractions, and the
/// deadliest kill is a name. Every field defaulted, so a save written
/// before a field existed still reads.
#[derive(Resource, Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tally {
    /// Monsters the adventurer and the mercenaries they hired killed: the
    /// run's number beside the Slain line, which counts the same deaths,
    /// so the two never read as two different fights.
    pub kills: u32,
    /// The engine's `balance::threat` of everything the adventurer and the
    /// mercenaries they hired killed, summed, and rounded only when it is
    /// scored: the gold spent on the party is the adventurer's choice, and
    /// a party that kills is the return on it.
    pub threat_slain: f32,
    /// The deadliest thing the adventurer or a mercenary they hired
    /// killed, by name, and its threat.
    pub deadliest: Option<(String, f32)>,
    /// Coins of gold found, each counted the first time the adventurer
    /// took it up.
    pub gold_found: u32,
    /// Health the blows, shots and fires of the adventurer and the
    /// mercenaries they hired took from others: the company's, as the
    /// kills beside it are, so a run whose sellswords did the killing does
    /// not read as many kills and no damage. Others means outside the
    /// party, so a blow that caught a friend is not counted.
    pub damage_dealt: u32,
    /// Health the adventurer lost, to anything, their own fire included.
    pub damage_taken: u32,
    /// Potions the adventurer drank.
    pub potions_drunk: u32,
    /// Things other than gold the adventurer took up, each the first time
    /// it came into their hands, a pile of arrows one thing.
    pub items_found: u32,
    /// Mini-bosses the adventurer killed, by their own hand: a line of the
    /// score worth [`MINIBOSS_POINTS`] each, which a hireling's blow does
    /// not buy.
    pub minibosses: u32,
    /// The deepest floor reached, nought while the run has reached none.
    pub deepest: u32,
    /// The boss whose death won the run, by name, once one has died,
    /// whoever's hand it fell to: what the record of a run won says it
    /// slew.
    pub boss_slain: Option<String>,
    /// Every mini-boss and boss that has died this run, by name, in the
    /// order they fell, whoever's hand they fell to: what a townsperson's
    /// line waiting on one being slain asks.
    pub fallen: Vec<String>,
    /// What killed the adventurer, as the record says it, noted at the
    /// death: what did may be gone by the time the record is written, a
    /// monster's last burst or a shooter that died behind its shot.
    pub slain_by: Option<String>,
    /// Every hired mercenary who died, in the order they fell: how many
    /// the run lost, and who, where and to what.
    pub mercenaries_fallen: Vec<FallenMercenary>,
}

/// A hired mercenary who died, as the end screen and the records name it.
///
/// Its name kept as a word rather than the body it left, which is gone or
/// renamed long before the run is written down; and what killed it as the
/// record says it, read at the death, for the same reason as
/// [`Tally::slain_by`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FallenMercenary {
    /// What it was called.
    pub name: String,
    /// The floor it fell on, nought for Mereholt.
    pub floor: u32,
    /// What killed it, `a goblin` or `your own hand`; `None` when nothing
    /// got the credit or what did was gone.
    pub by: Option<String>,
}

impl Tally {
    /// Counts a kill of `name`, of `threat`: one more kill, its threat
    /// added to the rest, and the deadliest, which a kill takes from an
    /// earlier one only by being deadlier.
    pub fn slay(&mut self, name: &str, threat: f32) {
        self.kills += 1;
        self.threat_slain += threat;
        if self.deadliest.as_ref().is_none_or(|(_, worst)| threat > *worst) {
            self.deadliest = Some((name.to_string(), threat));
        }
    }

    /// How many hired mercenaries the run has lost: the stats line
    /// `DESIGN.md` plans, counted off the fallen rather than kept beside
    /// them, so the two cannot disagree.
    pub fn mercenaries_lost(&self) -> u32 {
        u32::try_from(self.mercenaries_fallen.len()).unwrap_or(u32::MAX)
    }
}

/// The score of a run that came to `t`, `won` or not: the design's five
/// lines, in its order, each its name and its points.
///
/// Depth is a hundred for each floor down to the deepest, the slain the
/// threat of every kill summed and rounded, treasure the gold found, a
/// mini-boss five hundred, and the win five thousand. A line that comes to
/// nothing is still a line, so every record lists the same five.
pub fn score(t: &Tally, won: bool) -> Vec<(String, i64)> {
    vec![
        ("Depth".to_string(), DEPTH_POINTS * i64::from(t.deepest)),
        ("Slain".to_string(), t.threat_slain.round() as i64),
        ("Treasure".to_string(), i64::from(t.gold_found)),
        ("Mini-bosses".to_string(), MINIBOSS_POINTS * i64::from(t.minibosses)),
        ("Victory".to_string(), if won { VICTORY_POINTS } else { 0 }),
    ]
}

/// What the `lines` of a score come to.
pub fn total(lines: &[(String, i64)]) -> i64 {
    lines.iter().map(|(_, points)| points).sum()
}

/// Raises the tally's deepest floor on every arrival on a floor deeper
/// than any before.
pub fn remember_depth(mut entered: MessageReader<PlaceEntered>, mut tally: ResMut<Tally>) {
    for floor in entered.read().filter_map(|ev| floor_of(ev.map)) {
        if floor > tally.deepest {
            tally.deepest = floor;
        }
    }
}

/// Counts the fighting the adventurer did and took: every blow, shot and
/// burn that landed on them, every one of theirs or a hired mercenary's
/// that landed on anything outside the party, and every death either gets
/// the credit for, at its kind's threat. A blow on one of the party, a
/// mercenary, a freed captive or the adventurer, is no damage dealt
/// whoever struck it, as an area blow that caught a friend is not the
/// fighting the number is about. A boss's death is noted by name
/// whoever's hand it fell to, since the record of the run it won names
/// it, and so is whatever killed the adventurer, since the record of the
/// run it ended names that.
///
/// A death a hired mercenary gets the credit for is a kill of the run's as
/// the adventurer's is, on the kills, the deadliest and the Slain line
/// alike, so the numbers beside the score count the deaths the score
/// does; only the mini-bosses stay the adventurer's own, since that line
/// pays for the blow. A hired mercenary's own death is a loss, noted with
/// where it fell and what killed it, and never a kill, even at the
/// adventurer's hand, so hiring one to cut it down scores nothing. What a
/// freed captive or a dominated foe kills counts nowhere: the party the
/// score pays for is the one bought.
pub fn count_the_fighting(
    mut dealt: MessageReader<DamageDealt>,
    mut deaths: MessageReader<DeathEvent>,
    credit: Credit,
    kinds: Query<(&Kind, Has<crate::effects::bodies::Raised>)>,
    healths: Query<&Health>,
    bestiary: Res<Bestiary>,
    mut tally: ResMut<Tally>,
) {
    let me = credit.adventurer.single().ok();
    // A hit absorbed reports what it mended, below nought, which neither
    // side lost; what is left, grouped by who took it, in the order it
    // landed, since a weapon's extra strikes can carry more than one blow
    // to the same target in the one pass.
    let mut landed: BTreeMap<Entity, Vec<i32>> = BTreeMap::new();
    for d in dealt.read().filter(|d| d.dealt > 0) {
        let ours = (d.hit.credit.is_some() && d.hit.credit == me) || credit.hired(d.hit.credit);
        if Some(d.target) == me || (ours && !credit.in_party(d.target)) {
            landed.entry(d.target).or_default().push(d.dealt);
        }
    }
    for (target, blows) in landed {
        // `DamageDealt::dealt` is the blow's raw amount, not clamped to
        // what its target had left, so a killing blow past their last
        // health reports more than they lost. Its health now already
        // reflects every blow below with no floor of its own, so the
        // health before the first of them comes back by adding the blows
        // to it, and each is then counted for no more than was left when
        // its turn came.
        let mut before = healths.get(target).map_or(0, |h| h.current) + blows.iter().sum::<i32>();
        for amount in blows {
            let taken = amount.min(before.max(0)) as u32;
            if Some(target) == me {
                tally.damage_taken += taken;
            } else {
                tally.damage_dealt += taken;
            }
            before -= amount;
        }
    }
    for death in deaths.read() {
        // Read now, in the pass it died in, while whatever gets the credit
        // still answers to its name.
        if Some(death.entity) == me {
            tally.slain_by = credit.of(death.credit);
        }
        if let Some(fallen) = credit.fallen(death.entity, death.credit) {
            tally.mercenaries_fallen.push(fallen);
            continue;
        }
        // A raised body's second death is counted nowhere: its first
        // already paid its kill, its mini-boss and its threat.
        let Ok((kind, false)) = kinds.get(death.entity) else { continue };
        let def = bestiary.defs.get(kind.0);
        if def.boss {
            tally.boss_slain = Some(def.name.clone());
        }
        if (def.boss || def.miniboss) && !tally.fallen.contains(&def.name) {
            tally.fallen.push(def.name.clone());
        }
        if death.credit.is_some() && death.credit == me {
            tally.slay(&def.name, threat(def));
            if def.miniboss {
                tally.minibosses += 1;
            }
        } else if credit.hired(death.credit) {
            tally.slay(&def.name, threat(def));
        }
    }
}

/// How many of a thing's units the tally has counted as found already:
/// all of them, once the adventurer has held it.
///
/// On the thing rather than in a list of the things counted, so it goes
/// wherever the thing goes, the floor, a goblin's pack, a save, and a pile
/// of gold dropped and taken up again counts nothing the second time. A
/// number rather than a mark because stacks merge: a pile the adventurer
/// never held, taken into a stack they did, joins it with nothing counted,
/// and a stack copied off by a throw carries the count with it.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counted(pub u32);

/// Stacks that left the world since the last reactions ran, each with how
/// many it was and how many of those were counted.
///
/// A pile taken into a carried stack is despawned before the pass's
/// reactions read that it was taken, and its numbers would go with it;
/// [`note_departure`] writes them down on its way out, and
/// [`count_what_is_found`] reads them and forgets the rest, a potion drunk
/// to nothing or a run torn down.
#[derive(Resource, Debug, Default)]
pub struct Departed(Vec<(Entity, u32, u32)>);

/// Writes down a stack's numbers as it leaves the world, for
/// [`count_what_is_found`].
pub fn note_departure(gone: On<Remove, Stack>, stacks: Query<(&Stack, Option<&Counted>)>, mut departed: ResMut<Departed>) {
    if let Ok((stack, counted)) = stacks.get(gone.entity) {
        departed.0.push((gone.entity, stack.count, counted.map_or(0, |c| c.0)));
    }
}

/// What the gold and the things the adventurer finds are counted from.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Finds<'w, 's> {
    things: Query<'w, 's, (Option<&'static Stack>, Option<&'static Counted>, Option<&'static Tagged>)>,
    departed: ResMut<'w, Departed>,
    registries: Res<'w, Registries>,
}

/// Counts what the adventurer finds and drinks: gold by the coin and
/// anything else by the thing, each only the first time it comes into
/// their hands, and every potion they use.
///
/// Reads every pickup, the adventurer's and everyone else's, and keeps
/// each thing's [`Counted`] as it goes: a pile taken into a stack brings
/// what it had counted with it, whoever takes it, so a goblin that picked
/// up gold the adventurer dropped, and dies with it, does not hand the
/// same coins over a second time. What the adventurer takes up is counted
/// whole, so it never counts again.
pub fn count_what_is_found(
    mut commands: Commands,
    mut events: MessageReader<ItemEvent>,
    adventurer: Query<Entity, With<Adventurer>>,
    finds: Finds,
    mut tally: ResMut<Tally>,
) {
    let Finds { things, mut departed, registries } = finds;
    let me = adventurer.single().ok();
    let (gold, potion) = (registries.tags.expect("gold"), registries.tags.expect("potion"));
    let tagged = |thing: Entity, tag| things.get(thing).ok().and_then(|(_, _, tags)| tags).is_some_and(|tags| tags.contains(&tag));
    // What each thing touched this pass has counted, as it stands after
    // the pass's pickups, written back once at the end.
    let mut counted: BTreeMap<Entity, u32> = BTreeMap::new();
    let counted_on = |thing: Entity, counted: &BTreeMap<Entity, u32>| {
        counted.get(&thing).copied().unwrap_or_else(|| things.get(thing).ok().and_then(|(_, c, _)| c).map_or(0, |c| c.0))
    };
    for ev in events.read() {
        match *ev {
            ItemEvent::Used { actor, item } if Some(actor) == me && tagged(item, potion) => tally.potions_drunk += 1,
            ItemEvent::PickedUp { actor, item, merged_into } => {
                // How many it was and how many of those were counted: read
                // off it while it stands, and off its departure once a
                // merge has taken it out of the world.
                let standing = things.get(item).ok().map(|(stack, _, _)| (stack.map_or(1, |s| s.count), counted_on(item, &counted)));
                let gone = || departed.0.iter().find(|(e, _, _)| *e == item).map(|(_, count, counted)| (*count, *counted));
                let Some((count, before)) = standing.or_else(gone) else { continue };
                let before = before.min(count);
                let mine = me.is_some() && Some(actor) == me;
                let carried = if mine { count } else { before };
                match merged_into {
                    Some(into) => {
                        let already = counted_on(into, &counted);
                        counted.insert(into, already + carried);
                    }
                    None => {
                        counted.insert(item, carried);
                    }
                }
                let fresh = count - before;
                if mine && fresh > 0 {
                    if tagged(merged_into.unwrap_or(item), gold) {
                        tally.gold_found += fresh;
                    } else {
                        tally.items_found += 1;
                    }
                }
            }
            _ => {}
        }
    }
    departed.0.clear();
    for (thing, n) in counted {
        commands.entity(thing).try_insert(Counted(n));
    }
}

/// Marks every piece of the currency `who` carries as counted whole: what
/// they were just paid, by a sale or a rite's refund, is theirs, not a
/// find, however often it is dropped, stolen and won back after.
///
/// Every piece rather than the one paid into, since a payment lands in
/// whichever stack the pack already holds; exact, because a purse never
/// holds found coins not yet counted, a pickup counting them as it takes
/// them up. Nothing without a [`Currency`].
pub fn count_as_owned(world: &mut World, who: Entity) {
    let Some(currency) = world.get_resource::<Currency>().copied() else { return };
    let Some(bag) = world.get::<Inventory>(who) else { return };
    let pieces: Vec<(Entity, u32)> =
        bag.items.iter().filter(|i| currency.is(world.get::<Tagged>(**i))).map(|i| (*i, world.get::<Stack>(*i).map_or(1, |s| s.count))).collect();
    for (piece, count) in pieces {
        world.entity_mut(piece).insert(Counted(count));
    }
}

/// Keeps a deal's goods out of the tally's finds: the gold a sale pays the
/// adventurer, by [`count_as_owned`], and a thing they buy, counted whole,
/// so dropping either and taking it up again is not treasure found.
///
/// In `Update`, after the turns, as the game reads every [`Traded`]; the
/// deal is struck before them while the trade screen holds the turns, so
/// no pickup comes between the deal and this.
pub fn count_deals(world: &mut World, mut cursor: Local<MessageCursor<Traded>>) {
    let Some(messages) = world.get_resource::<Messages<Traded>>() else { return };
    let deals: Vec<Traded> = cursor.read(messages).copied().filter(|d| world.get::<Adventurer>(d.customer).is_some()).collect();
    for deal in deals {
        match deal.deal {
            Deal::Sold => count_as_owned(world, deal.customer),
            Deal::Bought => {
                let count = world.get::<Stack>(deal.item).map_or(1, |s| s.count);
                if let Ok(mut thing) = world.get_entity_mut(deal.item) {
                    thing.insert(Counted(count));
                }
            }
        }
    }
}

/// How wide a ledger line is, label to value: room for the longest label
/// and the longest value, the name of the deadliest kill, with dots
/// between, and narrower than both the end screen and the records screen,
/// so a line is never wrapped.
const LEDGER: usize = 34;

/// `rows` as a ledger: each label and its value at either end of a line,
/// dots between, so the values stand in one column on a screen that draws
/// a section's words one space apart, as the engine's do.
fn ledger(rows: &[(&str, String)]) -> String {
    let line = |(label, value): &(&str, String)| {
        let dots = LEDGER.saturating_sub(label.chars().count() + value.chars().count() + 2).max(2);
        format!("{label} {} {value}", ".".repeat(dots))
    };
    rows.iter().map(line).collect::<Vec<_>>().join("\n")
}

/// Where `place` is, as a record says it: on a floor, or in Mereholt.
fn whereabouts(place: MapId) -> String {
    match floor_of(place) {
        Some(floor) => format!("on floor {floor}"),
        None => "in Mereholt".to_string(),
    }
}

/// The run in a line, for the records list and the end screen: the boss
/// slain, or what killed the adventurer and where, `by` being whatever
/// gets the credit as a record says it.
pub fn summary(outcome: Outcome, by: Option<&str>, place: MapId, tally: &Tally) -> String {
    match (outcome, by) {
        (Outcome::Won, _) => match &tally.boss_slain {
            Some(boss) => format!("Slew {}", with_article(boss)),
            None => format!("Won {}", whereabouts(place)),
        },
        (Outcome::Died { .. }, Some(by)) => format!("Slain by {by} {}", whereabouts(place)),
        (Outcome::Died { .. }, None) => format!("Died {}", whereabouts(place)),
        (Outcome::Abandoned, _) => format!("Gave up {}", whereabouts(place)),
    }
}

/// The record's two sections: the score line by line, then its total set
/// off from the lines by a rule of the ledger's own dots, the way a sum is
/// ruled off under the column it adds; and the run's numbers, `turns` long.
///
/// No seed among them: the end screen says it under the run's summary, in
/// the engine's words, and a second line saying it again is noise.
pub fn sections(lines: &[(String, i64)], tally: &Tally, turns: u32) -> Vec<RecordSection> {
    let points: Vec<(&str, String)> = lines.iter().map(|(name, points)| (name.as_str(), points.to_string())).collect();
    let score = [ledger(&points), ".".repeat(LEDGER), ledger(&[("Total", total(lines).to_string())])].join("\n");
    let none = || "none".to_string();
    let run: Vec<(&str, String)> = vec![
        ("Turns", turns.to_string()),
        ("Deepest floor", if tally.deepest == 0 { none() } else { tally.deepest.to_string() }),
        ("Kills", tally.kills.to_string()),
        ("Deadliest", tally.deadliest.as_ref().map_or_else(none, |(name, _)| with_article(name))),
        ("Damage dealt", tally.damage_dealt.to_string()),
        ("Damage taken", tally.damage_taken.to_string()),
        ("Potions drunk", tally.potions_drunk.to_string()),
        ("Items found", tally.items_found.to_string()),
        ("Gold found", tally.gold_found.to_string()),
        ("Mercenaries lost", tally.mercenaries_lost().to_string()),
    ];
    let mut sections = vec![RecordSection { heading: "Score".to_string(), body: score }, RecordSection { heading: "The run".to_string(), body: ledger(&run) }];
    if !tally.mercenaries_fallen.is_empty() {
        let fallen: Vec<String> = tally.mercenaries_fallen.iter().map(fallen_line).collect();
        sections.push(RecordSection { heading: FALLEN.to_string(), body: fallen.join("\n") });
    }
    sections
}

/// The heading the mercenaries a run lost are listed under, below its
/// numbers, and only when it lost any.
pub const FALLEN: &str = "The fallen";

/// One fallen mercenary as the record lists it, in the words the run's own
/// summary uses: `Brenna, slain by a goblin on floor 3`.
fn fallen_line(fallen: &FallenMercenary) -> String {
    let place = if fallen.floor == 0 { "in Mereholt".to_string() } else { format!("on floor {}", fallen.floor) };
    match &fallen.by {
        Some(by) => format!("{}, slain by {by} {place}", fallen.name),
        None => format!("{}, died {place}", fallen.name),
    }
}

/// Who gets the credit for a death: what a record says killed the
/// adventurer or one of their mercenaries, and whether a kill is the
/// party's.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Credit<'w, 's> {
    adventurer: Query<'w, 's, Entity, With<Adventurer>>,
    names: Query<'w, 's, &'static Name>,
    party: Query<'w, 's, (&'static Mercenary, Option<&'static OnMap>)>,
    leaders: Query<'w, 's, &'static Leader>,
}

impl Credit<'_, '_> {
    /// Whether `by` is a mercenary the adventurer hired, whose kills and
    /// blows count as the adventurer's do, but on the Mini-bosses line.
    fn hired(&self, by: Option<Entity>) -> bool {
        by.is_some_and(|by| self.party.contains(by))
    }

    /// Whether `who` is one of the adventurer's party: the adventurer, or
    /// anyone following them, hired or freed. By [`Leader`], as the party
    /// rail lists it, so the two never disagree on who is a friend.
    fn in_party(&self, who: Entity) -> bool {
        let me = self.adventurer.single().ok();
        Some(who) == me || self.leaders.get(who).is_ok_and(|l| Some(l.0) == me)
    }

    /// `dead` as a fallen mercenary, killed by `by`, when it was one the
    /// adventurer hired; read in the pass it died in, while it still is.
    fn fallen(&self, dead: Entity, by: Option<Entity>) -> Option<FallenMercenary> {
        let (hired, on) = self.party.get(dead).ok()?;
        let floor = on.and_then(|on| floor_of(on.0)).unwrap_or(0);
        Some(FallenMercenary { name: hired.name.clone(), floor, by: self.of(by) })
    }

    /// Whoever `by` is, as the summary says it: a goblin, the Drowned
    /// King, or the adventurer's own hand; `None` when nothing gets the
    /// credit or what did is gone.
    fn of(&self, by: Option<Entity>) -> Option<String> {
        let by = by?;
        if self.adventurer.single().ok() == Some(by) {
            return Some("your own hand".to_string());
        }
        self.names.get(by).ok().map(|name| with_article(name.as_str()))
    }
}

// ANCHOR: record
/// Writes the run down once it is over: one [`RecordRun`] with its
/// score, its summary and its two sections, which the engine completes
/// with the outcome, the seed and the clock, files, and shows on the end
/// screen.
///
/// On entering `EngineState::Over`, the one door every ending comes
/// through, the boss's death and the adventurer's alike, and the frame the
/// engine's `Ending` is first there to read. Nothing in the run moves once
/// it is over, so the tally read here is the run's last word, and the
/// adventurer still stands where it ended.
pub fn record(ending: Option<Res<Ending>>, tally: Res<Tally>, map: Option<Res<WorldMap>>, credit: Credit, mut records: MessageWriter<RecordRun>) {
    let (Some(ending), Some(map)) = (ending, map) else { return };
    let by = match ending.outcome {
        // As noted at the death, since what did may be gone by now; asked
        // again only of an ending no death came through.
        Outcome::Died { by } => tally.slain_by.clone().or_else(|| credit.of(by)),
        _ => None,
    };
    let lines = score(&tally, ending.outcome == Outcome::Won);
    records.write(RecordRun {
        score: total(&lines),
        summary: summary(ending.outcome, by.as_deref(), map.current(), &tally),
        sections: sections(&lines, &tally, ending.turn),
    });
}
// ANCHOR_END: record

#[cfg(test)]
mod tests {
    use rand::{Rng, SeedableRng};
    use rl_engine::rl_core::RunSeed;

    use super::*;
    use crate::dungeon::map_of;
    use crate::testing::{
        adventurer, arrive, at, clear_floor, clear_monsters, continued, headless, kill, lay, lay_floor, make, monster, name_of, pass_turns, put_down, settle,
        stand_on, sturdy, take_up,
    };

    /// The points a ledger line stands for: the number at its end.
    fn points(line: &str) -> i64 {
        line.rsplit(' ').next().and_then(|n| n.parse().ok()).unwrap_or_else(|| panic!("a line ending in its points: {line:?}"))
    }

    /// Over a span of tallies, won and lost: the five lines in the
    /// design's order, each by its own rule, a total that is their sum,
    /// and a record whose Score section lists the same lines and then,
    /// set off from them by a row that is no line of points, the same
    /// total as its score.
    #[test]
    fn the_score_is_the_sum_of_its_lines() {
        for s in 0..64 {
            let mut rng = rand::rngs::StdRng::seed_from_u64(s);
            let t = Tally {
                kills: rng.random_range(0..300),
                threat_slain: rng.random_range(0.0..20_000.0),
                gold_found: rng.random_range(0..10_000),
                minibosses: rng.random_range(0..3),
                deepest: rng.random_range(0..=10),
                ..Tally::default()
            };
            for won in [false, true] {
                let lines = score(&t, won);
                let names: Vec<&str> = lines.iter().map(|(name, _)| name.as_str()).collect();
                assert_eq!(names, ["Depth", "Slain", "Treasure", "Mini-bosses", "Victory"], "tally {s}");
                let expected = [
                    100 * i64::from(t.deepest),
                    t.threat_slain.round() as i64,
                    i64::from(t.gold_found),
                    500 * i64::from(t.minibosses),
                    if won { 5000 } else { 0 },
                ];
                assert_eq!(lines.iter().map(|(_, p)| *p).collect::<Vec<_>>(), expected, "tally {s}, won {won}");
                assert_eq!(total(&lines), expected.iter().sum::<i64>(), "tally {s}, won {won}");
                let written = &sections(&lines, &t, 100)[0];
                let rows: Vec<&str> = written.body.lines().collect();
                assert_eq!((written.heading.as_str(), rows.len()), ("Score", 7), "five lines, a row setting them off, and the total: {rows:?}");
                assert_eq!(rows[..5].iter().map(|r| points(r)).collect::<Vec<_>>(), expected, "{rows:?}");
                assert!(!rows[5].chars().any(char::is_alphanumeric), "the total is set off from the lines: {rows:?}");
                assert!(rows[6].starts_with("Total ") && points(rows[6]) == total(&lines), "{rows:?}");
            }
        }
    }

    /// Three kills on floor one, a rat, an ogre and another rat: three
    /// kills, their threat summed, and the ogre the deadliest, which the
    /// second rat does not take from it. A jackal that dies with nobody to
    /// credit is no kill of the adventurer's.
    #[test]
    fn a_kill_adds_its_threat_and_the_deadliest_is_kept() {
        let mut app = headless(RunSeed(6));
        let me = arrive(&mut app, map_of(1));
        sturdy(&mut app, me);
        clear_monsters(&mut app);
        let here = at(&app, me);
        let around = [here.offset(1, 0), here.offset(0, 1), here.offset(-1, 0), here.offset(0, -1)];
        lay_floor(&mut app, &around);
        let threat_of = |app: &App, name: &str| {
            let bestiary = app.world().resource::<Bestiary>();
            threat(bestiary.defs.get(bestiary.defs.expect(name)))
        };
        let (rat, ogre) = (threat_of(&app, "rat"), threat_of(&app, "ogre"));
        assert!(ogre > rat, "an ogre is the deadlier: {ogre} against {rat}");
        for (name, at) in [("rat", around[0]), ("ogre", around[1]), ("rat", around[2])] {
            let victim = monster(&mut app, name, at);
            kill(&mut app, victim, Some(me));
        }
        let jackal = monster(&mut app, "jackal", around[3]);
        kill(&mut app, jackal, None);
        let tally = app.world().resource::<Tally>();
        assert_eq!(tally.kills, 3, "the jackal died at nobody's hand");
        assert!((tally.threat_slain - (rat * 2.0 + ogre)).abs() < 1e-3, "{} for two rats and an ogre", tally.threat_slain);
        assert_eq!(tally.deadliest, Some(("ogre".to_string(), ogre)), "the ogre, kept past the rat after it");
    }

    /// A pile of gold counts the first time the adventurer takes it up and
    /// never again: dropped and taken up, or dropped and taken into a
    /// stack of fresh gold, it adds nothing, though every coin is carried.
    /// And a thing the adventurer set out with is never a find, where a
    /// dagger off the floor is one.
    #[test]
    fn gold_is_counted_once_however_often_it_is_picked_up() {
        let mut app = headless(RunSeed(5));
        let me = arrive(&mut app, map_of(1));
        sturdy(&mut app, me);
        clear_monsters(&mut app);
        clear_floor(&mut app);
        // This test is about a pile found below, not the kit's own purse,
        // which `the_kits_purse_is_not_counted_as_treasure` already
        // covers: taken clean out of the pack, so a fresh pile is the only
        // gold the adventurer ever carries here.
        let purse =
            app.world().get::<Inventory>(me).expect("a pack").items.iter().copied().find(|i| name_of(&app, *i) == "gold coin").expect("the kit's purse");
        app.world_mut().get_mut::<Inventory>(me).expect("a pack").items.retain(|i| *i != purse);
        app.world_mut().despawn(purse);
        let here = at(&app, me);
        let beside = here.offset(1, 0);
        lay_floor(&mut app, &[beside]);
        let pile = |app: &mut App, coins: u32, at: Point| {
            let gold = make(app, "gold coin");
            app.world_mut().get_mut::<Stack>(gold).expect("gold stacks").count = coins;
            lay(app, gold, at);
            gold
        };
        let found = |app: &App| app.world().resource::<Tally>().gold_found;
        let carried = |app: &App| {
            let world = app.world();
            let bag = &world.get::<Inventory>(me).expect("a pack").items;
            bag.iter().filter(|i| name_of(app, **i) == "gold coin").map(|i| world.get::<Stack>(*i).map_or(1, |s| s.count)).sum::<u32>()
        };

        let first = pile(&mut app, 10, here);
        take_up(&mut app, me);
        assert_eq!((found(&app), carried(&app)), (10, 10), "a pile nobody had held counts whole");
        put_down(&mut app, me, first);
        take_up(&mut app, me);
        assert_eq!((found(&app), carried(&app)), (10, 10), "dropped and taken up again, it counts nothing");

        put_down(&mut app, me, first);
        pile(&mut app, 5, beside);
        stand_on(&mut app, me, beside);
        take_up(&mut app, me);
        assert_eq!((found(&app), carried(&app)), (15, 5), "a fresh pile counts, taken up into a pack with no gold");
        stand_on(&mut app, me, here);
        take_up(&mut app, me);
        assert_eq!((found(&app), carried(&app)), (15, 15), "and the first pile, taken into it, counts nothing, though every coin is carried");

        let things = |app: &App| app.world().resource::<Tally>().items_found;
        let sword = app.world().get::<Inventory>(me).expect("a pack").items.iter().copied().find(|i| name_of(&app, *i) == "sword").expect("the kit's sword");
        put_down(&mut app, me, sword);
        take_up(&mut app, me);
        assert_eq!(things(&app), 0, "the sword the adventurer set out with is no find");
        let dagger = make(&mut app, "dagger");
        lay(&mut app, dagger, here);
        take_up(&mut app, me);
        assert_eq!(things(&app), 1, "a dagger off the floor is");
    }

    /// The fifty gold the adventurer walks out of Mereholt with is their
    /// own from the first turn, never a find: a fresh run's tally counts
    /// no treasure before anything is picked up off a floor.
    #[test]
    fn the_kits_purse_is_not_counted_as_treasure() {
        let mut app = headless(RunSeed(0));
        adventurer(&mut app);
        assert_eq!(app.world().resource::<Tally>().gold_found, 0, "the kit's purse is carried, not found");
    }

    /// A sale pays the adventurer in gold out of the trader's own purse:
    /// it rises in the adventurer's pack, but it is paid, not found, so
    /// the tally counts none of it as treasure, then or after: dropped and
    /// taken up again, or stolen by a monkey and taken back off its body.
    /// And a thing bought is no find either, dropped and taken up.
    #[test]
    fn gold_paid_for_a_sale_is_not_counted_as_treasure() {
        let mut app = headless(RunSeed(1));
        let me = arrive(&mut app, map_of(1));
        sturdy(&mut app, me);
        clear_monsters(&mut app);
        clear_floor(&mut app);
        let here = at(&app, me);
        let weapon = app.world().resource::<Registries>().tags.expect("weapon");
        let dagger = make(&mut app, "dagger");
        app.world_mut().get_mut::<Inventory>(me).expect("a pack").items.push(dagger);
        let trade = app.world().resource::<Verbs>().get(TRADE).expect("TradePlugin declares it");
        let till = make(&mut app, "gold coin");
        app.world_mut().get_mut::<Stack>(till).expect("gold stacks").count = 1000;
        let wares = make(&mut app, "dagger");
        app.world_mut().entity_mut(wares).insert(Worth(5));
        let trader = app
            .world_mut()
            .spawn((
                Actor,
                Blocks,
                Position(here.offset(1, 0)),
                Inventory { items: vec![till, wares] },
                Trader { buys: vec![weapon], pays_pct: 25 },
                Offers(vec![Offering::new(trade, 0)]),
            ))
            .id();
        app.update();
        let gold = |app: &App| -> Vec<Entity> {
            let bag = &app.world().get::<Inventory>(me).expect("a pack").items;
            bag.iter().copied().filter(|i| name_of(app, *i) == "gold coin").collect()
        };
        let coins = |app: &App| -> u32 { gold(app).iter().map(|i| app.world().get::<Stack>(*i).map_or(1, |s| s.count)).sum() };
        let found = |app: &App| app.world().resource::<Tally>().gold_found;
        let things = |app: &App| app.world().resource::<Tally>().items_found;
        app.world_mut().write_message(Intent::new(me, Interact { target: trader, verb: trade }));
        settle(&mut app);
        let before = coins(&app);
        app.world_mut().write_message(Sell { customer: me, trader, item: dagger, count: 1 });
        app.update();
        let paid = coins(&app);
        assert!(paid > before, "paid for the dagger: {before} before, {paid} after");
        assert_eq!(found(&app), 0, "paid, not found");

        app.world_mut().write_message(Buy { customer: me, trader, item: wares, count: 1 });
        app.update();
        let bought =
            app.world().get::<Inventory>(me).expect("a pack").items.iter().copied().find(|i| name_of(&app, *i) == "dagger").expect("the dagger bought");
        app.world_mut().resource_mut::<Trading>().close();
        app.update();
        put_down(&mut app, me, bought);
        take_up(&mut app, me);
        assert_eq!(things(&app), 0, "a thing bought, dropped and taken up, is no find");

        let purse = gold(&app);
        assert_eq!(purse.len(), 1, "the sale's coins joined the kit's purse");
        let carried = coins(&app);
        put_down(&mut app, me, purse[0]);
        take_up(&mut app, me);
        assert_eq!((found(&app), coins(&app)), (0, carried), "dropped and taken up, the sale's coins are still no treasure");

        // Nothing in the pack but the purse, so a monkey's theft takes it.
        app.world_mut().get_mut::<Inventory>(me).expect("a pack").items.retain(|i| purse.contains(i));
        app.world_mut().entity_mut(me).remove::<Equipped>();
        let beside = here.offset(0, 1);
        lay_floor(&mut app, &[beside]);
        let monkey = monster(&mut app, "monkey", beside);
        assert!(crate::tricks::steal(app.world_mut(), monkey, me), "the monkey takes the purse");
        assert_eq!(coins(&app), 0, "the purse is gone");
        kill(&mut app, monkey, Some(me));
        stand_on(&mut app, me, beside);
        take_up(&mut app, me);
        assert_eq!((found(&app), coins(&app)), (0, carried), "stolen and won back, the sale's coins are still no treasure");
    }

    /// The tally is saved with the run: continued from the save the
    /// arrival on floor two wrote, it is the tally that was saved, and the
    /// gold already counted stays counted, so dropping it and taking it up
    /// again adds nothing.
    #[test]
    fn the_tally_survives_a_save_and_continue() {
        let mut app = headless(RunSeed(10));
        let me = arrive(&mut app, map_of(1));
        sturdy(&mut app, me);
        clear_monsters(&mut app);
        clear_floor(&mut app);
        let here = at(&app, me);
        lay_floor(&mut app, &[here.offset(1, 0)]);
        let gold = make(&mut app, "gold coin");
        app.world_mut().get_mut::<Stack>(gold).expect("gold stacks").count = 12;
        lay(&mut app, gold, here);
        take_up(&mut app, me);
        let rat = monster(&mut app, "rat", here.offset(1, 0));
        kill(&mut app, rat, Some(me));
        arrive(&mut app, map_of(2));
        let before = app.world().resource::<Tally>().clone();
        assert_eq!((before.kills, before.gold_found, before.deepest), (1, 12, 2), "{before:?}");
        let text = app.world().resource::<Saves>().load(crate::save::SLOT).expect("the slot reads").expect("the arrival wrote the run down");

        let mut continued = continued(&text);
        assert_eq!(*continued.world().resource::<Tally>(), before, "the tally as it was saved");
        let me = adventurer(&mut continued);
        let purse = continued.world().get::<Inventory>(me).expect("a pack").items.iter().copied().find(|i| name_of(&continued, *i) == "gold coin");
        put_down(&mut continued, me, purse.expect("the gold, carried"));
        take_up(&mut continued, me);
        assert_eq!(continued.world().resource::<Tally>().gold_found, 12, "gold counted before the save is not counted again after it");
    }

    /// The text of section `heading` of `record`, one row a line.
    fn rows<'a>(record: &'a RunRecord, heading: &str) -> Vec<&'a str> {
        record.sections.iter().find(|s| s.heading == heading).map(|s| s.body.lines().collect()).unwrap_or_default()
    }

    /// A goblin's blow kills the adventurer on floor three: the run ends,
    /// and its record says the goblin did it and where, lists the score
    /// line by line and then the run's numbers, and comes to a score its
    /// lines add up to. The record is kept beyond the run, in its slot.
    #[test]
    fn a_death_ends_in_a_record_naming_what_killed_and_where() {
        let mut app = headless(RunSeed(8));
        let me = arrive(&mut app, map_of(3));
        clear_monsters(&mut app);
        let beside = at(&app, me).offset(1, 0);
        lay_floor(&mut app, &[beside]);
        let goblin = monster(&mut app, "goblin", beside);
        kill(&mut app, me, Some(goblin));
        settle(&mut app);
        assert!(matches!(app.world().resource::<Ending>().outcome, Outcome::Died { .. }), "the run is over");
        let record = app.world().resource::<RunRecords>().this_run.clone().expect("the run's end filed a record");
        assert_eq!((record.outcome, record.summary.as_str()), (RecordOutcome::Died, "Slain by a goblin on floor 3"));
        assert_eq!(record.sections.iter().map(|s| s.heading.as_str()).collect::<Vec<_>>(), ["Score", "The run"]);
        let score_rows = rows(&record, "Score");
        assert_eq!(score_rows[..5].iter().map(|r| points(r)).sum::<i64>(), record.score, "the lines add up to the score: {score_rows:?}");
        assert_eq!((points(score_rows[0]), points(score_rows[6])), (300, record.score), "three floors down, and the total: {score_rows:?}");
        let run = rows(&record, "The run");
        let labels: Vec<&str> = run.iter().map(|r| r.split(" .").next().unwrap_or_default()).collect();
        assert_eq!(
            labels,
            ["Turns", "Deepest floor", "Kills", "Deadliest", "Damage dealt", "Damage taken", "Potions drunk", "Items found", "Gold found", "Mercenaries lost"],
            "{run:?}"
        );
        assert!(run[1].ends_with(" 3"), "floor three: {run:?}");
        let kept = app.world().resource::<Saves>().load(rl_engine::rl_save::RECORDS_SLOT).expect("the slot reads").expect("the records were written down");
        assert!(kept.contains("Slain by a goblin on floor 3"), "kept beyond the run: {kept}");
    }

    /// A monster that bursts as it dies, and takes the adventurer with it,
    /// is named on the record by what it was, though its body was gone
    /// before the burst landed and the record is written a frame later.
    #[test]
    fn a_death_by_a_dying_monsters_burst_is_recorded_in_its_name() {
        use rl_engine::rl_bevy::{EffectKinds, Moments, Triggers};
        let mut app = headless(RunSeed(8));
        let me = arrive(&mut app, map_of(3));
        clear_monsters(&mut app);
        let beside = at(&app, me).offset(1, 0);
        lay_floor(&mut app, &[beside]);
        let goblin = monster(&mut app, "goblin", beside);
        let burst = r#"[(on: "died", area: Burst(radius: 1), effects: [(kind: "Harm", args: (kind: "crush", roll: "100000"))])]"#;
        let specs: Vec<rl_engine::rl_rules::TriggerSpec> = rl_engine::rl_rules::Names::new().load_list(burst).expect("the burst parses");
        let built = {
            let world = app.world();
            Triggers::build(&specs, &[], world.resource::<Moments>(), world.resource::<EffectKinds>(), &world.resource::<Registries>().names())
                .expect("the burst builds")
        };
        app.world_mut().entity_mut(goblin).insert(built);
        kill(&mut app, goblin, Some(me));
        pass_turns(&mut app, 2);
        settle(&mut app);
        assert!(matches!(app.world().resource::<Ending>().outcome, Outcome::Died { .. }), "its burst ended the run");
        let record = app.world().resource::<RunRecords>().this_run.clone().expect("the run's end filed a record");
        assert_eq!(record.summary, "Slain by a goblin on floor 3");
    }

    /// The end screen says the run's seed under its summary, in the
    /// engine's words, and the engine keeps it on the record: so the
    /// record's own sections never say it a second time.
    #[test]
    fn the_record_says_the_seed_once() {
        let mut app = headless(RunSeed(8));
        let me = arrive(&mut app, map_of(2));
        kill(&mut app, me, None);
        settle(&mut app);
        let record = app.world().resource::<RunRecords>().this_run.clone().expect("the run's end filed a record");
        assert_eq!(record.seed, 8, "the engine's own account of the run carries its seed");
        let again: Vec<&str> = record.sections.iter().flat_map(|s| s.body.lines()).filter(|row| row.split(" .").next() == Some("Seed")).collect();
        assert!(again.is_empty(), "and no section of the game's says it again: {again:?}");
    }

    /// The Drowned King falls on floor ten to the adventurer's blow: the
    /// quest is done, the run is won, and its record says who was slain,
    /// with the victory's five thousand among its lines and the king the
    /// deadliest kill.
    #[test]
    fn slaying_the_king_wins_the_run() {
        let mut app = headless(RunSeed(9));
        let me = arrive(&mut app, map_of(10));
        sturdy(&mut app, me);
        let king_id = app.world().resource::<Bestiary>().defs.expect("the Drowned King");
        let king = {
            let world = app.world_mut();
            world.query::<(Entity, &Kind)>().iter(world).find(|(_, k)| k.0 == king_id).map(|(e, _)| e).expect("the king on his throne")
        };
        kill(&mut app, king, Some(me));
        settle(&mut app);
        settle(&mut app);
        assert_eq!(*app.world().resource::<State<EngineState>>().get(), EngineState::Over, "the run is over");
        assert_eq!(app.world().resource::<Ending>().outcome, Outcome::Won, "and won");
        let quests = app.world().resource::<Quests>();
        assert!(quests.defs.iter().all(|(id, q)| !q.victory || quests.tracker.state(id) == QuestState::Done), "the victory quest is done");
        let record = app.world().resource::<RunRecords>().this_run.clone().expect("the win filed a record");
        assert_eq!((record.outcome, record.summary.as_str()), (RecordOutcome::Won, "Slew the Drowned King"));
        let score_rows = rows(&record, "Score");
        assert!(score_rows[4].starts_with("Victory ") && points(score_rows[4]) == 5000, "{score_rows:?}");
        assert_eq!(score_rows[..5].iter().map(|r| points(r)).sum::<i64>(), record.score, "{score_rows:?}");
        let run = rows(&record, "The run");
        assert!(run[3].starts_with("Deadliest ") && run[3].ends_with(" the Drowned King"), "{run:?}");
    }

    /// A killing blow is counted for no more than the health it found, on
    /// either side: `kill` deals a hundred thousand, which the tally used
    /// to read whole, so the end screen said "Damage taken 44" on a run
    /// of forty health.
    #[test]
    fn a_killing_blow_is_counted_for_no_more_than_the_health_it_found() {
        let mut app = headless(RunSeed(8));
        let me = arrive(&mut app, map_of(3));
        clear_monsters(&mut app);
        let beside = at(&app, me).offset(1, 0);
        lay_floor(&mut app, &[beside]);
        let goblin = monster(&mut app, "goblin", beside);
        let goblin_health = app.world().get::<Health>(goblin).unwrap().current;
        kill(&mut app, goblin, Some(me));
        assert_eq!(app.world().resource::<Tally>().damage_dealt, goblin_health as u32, "the goblin's own health, not the blow's hundred thousand");

        let my_health = app.world().get::<Health>(me).unwrap().current;
        let archer = monster(&mut app, "goblin", beside);
        kill(&mut app, me, Some(archer));
        assert_eq!(app.world().resource::<Tally>().damage_taken, my_health as u32, "the adventurer's own health, not the blow's hundred thousand");
    }

    /// Two blows that land in the same pass, as a weapon's extra strikes
    /// would carry, are each counted for no more than was left of the
    /// target when its own turn came, and not for the full pair where the
    /// first alone used up everything there was.
    #[test]
    fn two_blows_landing_in_the_same_pass_are_each_counted_for_no_more_than_was_left() {
        let mut app = headless(RunSeed(8));
        let me = arrive(&mut app, map_of(3));
        clear_monsters(&mut app);
        // Bare hands and no armor, so a blow of a crush kind lands for
        // exactly what it says: the kit's leather would otherwise eat
        // part of each one, hiding the overshoot this test is about.
        app.world_mut().entity_mut(me).insert((Health::full(10), Armor(0)));
        app.world_mut().entity_mut(me).remove::<Equipped>();
        let crush = app.world().resource::<Registries>().damage_kinds.expect("crush");
        let hit = |amount| DamageEvent::new(me, rl_engine::rl_rules::Hit::from_source(None, crush, amount));
        app.world_mut().write_message(hit(6));
        app.world_mut().write_message(hit(6));
        settle(&mut app);
        assert_eq!(app.world().resource::<Tally>().damage_taken, 10, "the ten it had, not the twelve the two blows carried");
    }
}
