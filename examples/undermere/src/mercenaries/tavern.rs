//! The candidates at the Sunken Lantern's tables: who waits each visit,
//! what each says of itself, and `hire` kept off while the party is full.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_core::noun::{Countability, counted_as, with_the};
use rl_engine::rl_ui::view::offers::OffersView;
use rl_engine::rl_ui::view::sum_of;
use serde::{Deserialize, Serialize};

use super::*;
use crate::armory::Armory;
use crate::bestiary::{Bestiary, spawn_monster};
use crate::dungeon::TOWN;
use crate::finds::GOLD;
use crate::shops::Ware;
use crate::tally::Tally;
use crate::townsfolk::{GivenNames, NameSets, TALK, TALK_TIME};

/// How many times the adventurer has come to Mereholt this run, which is
/// the next visit's number: nought before the first arrival.
///
/// The run's, begun again with each run, so the same seed meets the same
/// candidates on the same visit however the visits between were spent.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tavern {
    /// Arrivals in Mereholt so far.
    pub visits: u64,
}

/// A sword for hire waiting at the Sunken Lantern, and what it asks.
///
/// The fee is kept here as well as on the `hire` offer, since the offer is
/// taken off while the party is full and has to be put back at the same
/// price. Waiting is all a candidate does: it has no mind, so it keeps to
/// its table until it is hired or the adventurer leaves and comes back to
/// three others.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate {
    /// What hiring it costs, in gold coins.
    pub fee: u32,
}

/// What making a candidate reads, borrowed from wherever the caller holds
/// it: a system's resources, or a whole world for a restore.
pub struct Makers<'a> {
    /// The kind it is.
    pub bestiary: &'a Bestiary,
    /// Its kit's things.
    pub armory: &'a Armory,
    /// The slots it wears its kit in, and its side.
    pub registries: &'a Registries,
    /// `hire` and `talk`.
    pub verbs: &'a Verbs,
}

impl<'a> Makers<'a> {
    /// The makers as `world` holds them.
    pub fn of(world: &'a World) -> Self {
        Self { bestiary: world.resource(), armory: world.resource(), registries: world.resource(), verbs: world.resource() }
    }
}

/// Makes a candidate called `name` at `at` in Mereholt, carrying `kit` and
/// asking `fee`: the [`MERCENARY`] kind on the townsfolk's side, with no
/// mind, wearing what of its kit it can wear, one thing to a slot and the
/// first of each kind, and offering [`HIRE`] at the fee and
/// [`TALK`].
///
/// `hire` comes first, so the offers screen opens on it. It is the
/// engine's priced offer: never taken up by a walk into the candidate, and
/// paid into the candidate's pack before the game hears of it in
/// `Interacted`.
pub fn spawn_candidate(commands: &mut Commands, makers: &Makers, name: &str, kit: &[Ware], fee: u32, at: Point) -> Entity {
    let Makers { bestiary, armory, registries, verbs } = makers;
    let mut pack = Vec::new();
    let mut worn = Equipment::with_slot_count(registries.slots.len());
    for ware in kit {
        let shape = armory.parts(ware.def).wear.clone();
        for thing in make_kit(commands, armory, std::slice::from_ref(ware)) {
            // A second of a kind, or a thing whose slot is taken, is
            // carried rather than worn.
            if let Some(shape) = &shape {
                let _ = worn.equip(thing, shape);
            }
            pack.push(thing);
        }
    }
    let def = bestiary.defs.get(bestiary.defs.expect(MERCENARY));
    let candidate = spawn_monster(commands, def, at, TOWN, registries, false);
    commands.entity(candidate).remove::<Mind>().insert((waiting_as(verbs, name, fee), Inventory { items: pack }, Equipped(worn)));
    candidate
}

/// Makes `candidate`, restored by a save as its kind, the candidate it was
/// saved as: called `name`, asking `fee`, with no mind, and offering
/// [`HIRE`] at the fee and [`TALK`]. What it carries and wears is the
/// engine's to put back, and its kit's [`OwnKit`] the item's own save.
///
/// `hire` is offered even while the party is full, since who follows the
/// adventurer is put back only after this: [`keep_hire_offered`] takes it
/// off at the start of the first pass, before anyone could take it up.
pub fn wait_again(world: &mut World, candidate: Entity, name: &str, fee: u32) {
    let waiting = waiting_as(world.resource::<Verbs>(), name, fee);
    world.entity_mut(candidate).remove::<Mind>().insert(waiting);
}

/// What makes a candidate one: its name, its [`Candidate`] fee and its
/// offers, one set for one seated and one restored.
fn waiting_as(verbs: &Verbs, name: &str, fee: u32) -> (Name, Candidate, Offers) {
    (Name::new(name.to_string()), Candidate { fee }, candidate_offers(verbs, fee))
}

/// What a candidate offers: [`HIRE`] at `fee`, first, so the offers screen
/// opens on it, then [`TALK`]. One list for a candidate seated and for a
/// mercenary let go, which waits to be hired again.
pub fn candidate_offers(verbs: &Verbs, fee: u32) -> Offers {
    Offers(vec![
        Offering::new(verbs.get(HIRE).expect("UndermerePlugin declares hire"), HIRE_TIME).priced(fee),
        Offering::new(verbs.get(TALK).expect("UndermerePlugin declares talk"), TALK_TIME),
    ])
}

/// Declares the tone a mercenary's words are said in, the colour of its
/// kind in `monsters.ron`.
pub fn declare_tone(app: &mut App, bestiary: &Bestiary) {
    let (r, g, b) = bestiary.defs.get(bestiary.defs.expect(MERCENARY)).color;
    app.add_tone(TONE, Color::srgb(r, g, b));
}

/// What [`seat_candidates`] reads to draw and make the candidates.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Drawing<'w> {
    seed: Res<'w, Seed>,
    tally: Res<'w, Tally>,
    mercenaries: Res<'w, Mercenaries>,
    names: Res<'w, NameSets>,
    map: Res<'w, WorldMap>,
    bestiary: Res<'w, Bestiary>,
    armory: Res<'w, Armory>,
    registries: Res<'w, Registries>,
    verbs: Res<'w, Verbs>,
}

/// Seats three new candidates at the tavern's tables on every arrival in
/// Mereholt, the first and every return, by the gate or a waystone: the
/// last visit's that nobody hired, and everything they carried, are gone
/// first, so a candidate passed over is not there to be hired later.
///
/// Visit `n`'s are drawn from the run's [`TAVERN_STREAM`] at `n`, one
/// [`TABLE`] mark at a time in the order the town lists them: a kit by
/// [`Mercenaries::kit`] at the deepest floor the run has reached, a name
/// from the `mercenary` set distinct from every name the run has given,
/// which it is then given, and the fee its kit is worth. The same seed and
/// the same visit find the same three.
///
/// In the arrival chain, after the keepers' shelves. A town with no mark
/// for them is a broken `mereholt.ron`, and panics naming it.
pub fn seat_candidates(
    mut commands: Commands,
    mut entered: MessageReader<PlaceEntered>,
    waiting: Query<(Entity, Option<&Inventory>), With<Candidate>>,
    mut tavern: ResMut<Tavern>,
    mut given: ResMut<GivenNames>,
    drawing: Drawing,
) {
    if !entered.read().any(|ev| ev.map == TOWN) {
        return;
    }
    for (candidate, bag) in &waiting {
        for thing in bag.into_iter().flat_map(|b| &b.items) {
            commands.entity(*thing).despawn();
        }
        commands.entity(candidate).despawn();
    }
    let visit = tavern.visits;
    tavern.visits += 1;
    let place = drawing.map.place(TOWN).expect("Mereholt is built when it is entered");
    let tables: Vec<Point> = place.spots.iter().filter(|s| s.tag == TABLE as u32).map(|s| s.at).collect();
    assert!(!tables.is_empty(), "mereholt.ron marks no '{TABLE}' for the candidates to wait at");
    let makers = Makers { bestiary: &drawing.bestiary, armory: &drawing.armory, registries: &drawing.registries, verbs: &drawing.verbs };
    let mut rng = drawing.seed.stream(TAVERN_STREAM, visit);
    for at in tables {
        let kit = drawing.mercenaries.kit(drawing.tally.deepest, &drawing.armory, &mut rng);
        let name = drawing.names.mercenary().draw_distinct(&mut rng, |n| given.has(n)).expect("the mercenaries' names never run dry");
        given.give(&name);
        let fee = drawing.mercenaries.fee(&kit, &drawing.armory);
        spawn_candidate(&mut commands, &makers, &name, &kit, fee, at);
    }
}

/// Takes [`HIRE`] off every candidate while [`PARTY`] already follow the
/// adventurer, and puts it back, at the candidate's fee, once fewer do.
/// Whether the party has room is the game's to say, so it is said before
/// the engine could take a price: a full party is never asked to pay.
///
/// A follower is a [`Mercenary`] whose `Leader` is the adventurer: a
/// freed captive follows them too, and is no hired hand to count.
///
/// In `TurnSet::Schedule`, at the start of every pass, so whatever joined
/// or left in the last pass's reactions is counted before anyone decides.
pub fn keep_hire_offered(
    mut candidates: Query<(&Candidate, &mut Offers)>,
    followers: Query<&Leader, With<Mercenary>>,
    players: Query<Entity, With<Player>>,
    verbs: Res<Verbs>,
) {
    let Some(hire) = verbs.get(HIRE) else { return };
    let following = players.single().map_or(0, |me| followers.iter().filter(|l| l.0 == me).count());
    let full = following >= PARTY;
    for (candidate, mut offers) in &mut candidates {
        let offered = offers.0.iter().any(|o| o.verb == hire);
        if full && offered {
            offers.0.retain(|o| o.verb != hire);
        } else if !full && !offered {
            offers.0.insert(0, Offering::new(hire, HIRE_TIME).priced(candidate.fee));
        }
    }
}

/// How a candidate holds one thing of its kit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Held {
    /// In the main hand.
    Wields,
    /// In any other slot.
    Wears,
    /// In the pack.
    Carries,
}

impl Held {
    /// The verb, as the look panel says it.
    pub fn verb(self) -> &'static str {
        match self {
            Held::Wields => "wields",
            Held::Wears => "wears",
            Held::Carries => "carries",
        }
    }
}

/// What [`KitSeen`] reads of one thing: its name, its stack, whether it is
/// said with no `a`, and its tags, for whether it is the currency.
type ThingSeen = (&'static Name, Option<&'static Stack>, Has<MassNoun>, Option<&'static Tagged>);

/// What a candidate's kit looks like to the adventurer: each thing by the
/// name it carries, which [`keep_names`](crate::lore::keep_names) keeps to
/// what the adventurer knows, so a potion whose kind they have not learned
/// is named by its face and a sword by the level the candidate told.
#[derive(bevy::ecs::system::SystemParam)]
pub struct KitSeen<'w, 's> {
    bags: Query<'w, 's, (&'static Inventory, Option<&'static Equipped>)>,
    things: Query<'w, 's, ThingSeen>,
    registries: Res<'w, Registries>,
    currency: Option<Res<'w, Currency>>,
}

impl KitSeen<'_, '_> {
    /// Each thing `who` holds, in its pack's order, and how: `a +1 spear`,
    /// `leather armor`, `6 darts`. Its purse is its own business and is
    /// left out.
    pub fn of(&self, who: Entity) -> Vec<(Held, String)> {
        let Ok((bag, worn)) = self.bags.get(who) else { return Vec::new() };
        let hand = self.registries.slots.id("main hand");
        let mut held = Vec::new();
        for thing in &bag.items {
            let Ok((name, stack, mass, tags)) = self.things.get(*thing) else { continue };
            if self.currency.as_deref().is_some_and(|c| c.is(tags)) {
                continue;
            }
            let how = match worn {
                Some(w) if hand.is_some_and(|h| w.in_slot(h) == Some(*thing)) => Held::Wields,
                Some(w) if w.contains(*thing) => Held::Wears,
                _ => Held::Carries,
            };
            let countability = if mass { Countability::Mass } else { Countability::Count };
            held.push((how, counted_as(name.as_str(), stack.map_or(1, |s| s.count), countability)));
        }
        held
    }
}

/// The facet key a row's kit is said under, on the look panel, the rail
/// and the company alike; the rail and the company draw it beneath the
/// row, since it does not fit beside a name.
pub const KIT_FACET: &str = "kit";

/// What `held` says on a row, a line a verb, wielded first, then worn,
/// then carried: `wields a +1 spear`, `wears leather armor and a cap`,
/// `carries 6 darts`. A line a verb rather than one sentence, so a rail
/// with room wraps each whole, and one without joins them with its own
/// mark and clips the end, which is what is carried.
pub fn kit_lines(held: &[(Held, String)]) -> Vec<String> {
    [Held::Wields, Held::Wears, Held::Carries]
        .into_iter()
        .filter_map(|how| {
            let things: Vec<String> = held.iter().filter(|(h, _)| *h == how).map(|(_, said)| said.clone()).collect();
            (!things.is_empty()).then(|| format!("{} {}", how.verb(), and_list(&things)))
        })
        .collect()
}

/// Says under each row of the rail and of the company what that one
/// holds, by [`kit_lines`], each thing named as the adventurer knows it:
/// a goblin that picked up a sword is a goblin with a sword, and a
/// hireling's kit is in sight without opening the look panel.
///
/// Only what wields and wears, an [`Equipped`]: a keeper's pack is its
/// shelf, which the shop shows, and a rat holds nothing. In
/// `ViewSet::Annotate`; order among the game's other notes does not
/// matter, since the panels draw this one beneath the row.
pub fn show_kit(
    rail: Option<ResMut<NearbyView>>,
    party: Option<ResMut<PartyView>>,
    holders: Query<(), (With<Equipped>, Without<Trader>)>,
    kits: KitSeen,
    mut facets: ResMut<Facets>,
) {
    let mut note = |row: &mut Row| {
        if holders.contains(row.entity) {
            for line in kit_lines(&kits.of(row.entity)) {
                row.facets.push(facets.facet(KIT_FACET, line).toned(Tones::MUTED));
            }
        }
    };
    if let Some(mut view) = rail {
        view.actors.iter_mut().for_each(&mut note);
    }
    if let Some(mut view) = party {
        view.rows_mut().for_each(&mut note);
    }
}

/// `items` as a sentence lists them: `a`, `a and b`, `a, b and c`.
pub(super) fn and_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// What a candidate says when talked to: what it carries, as the
/// adventurer would name each thing, and what it asks.
pub fn candidate_says(held: &[(Held, String)], fee: u32) -> String {
    let things: Vec<String> = held.iter().map(|(_, said)| said.clone()).collect();
    let carry = if things.is_empty() { "I carry nothing but my fists.".to_string() } else { format!("I carry {}.", and_list(&things)) };
    format!("{carry} Hire me for {}.", sum_of(GOLD, fee))
}

/// Says what a candidate says when talked to, by [`candidate_says`], in
/// the mercenaries' own tone, `Brenna: "..."`. The engine has already
/// spent the talk's turn.
///
/// In `TurnSet::React`, beside the townsfolk's own talk, which passes a
/// candidate over.
pub fn answer_candidates(
    mut done: MessageReader<Interacted>,
    verbs: Res<Verbs>,
    candidates: Query<(&Candidate, &Name)>,
    kits: KitSeen,
    tones: Res<Tones>,
    mut tell: MessageWriter<Tell>,
) {
    let Some(talk) = verbs.get(TALK) else { return };
    for ev in done.read().filter(|ev| ev.verb == talk) {
        let Ok((candidate, name)) = candidates.get(ev.target) else { continue };
        let words = candidate_says(&kits.of(ev.target), candidate.fee);
        tell.write(Tell::new(format!("{}: \"{words}\"", name.as_str()), tones.get(TONE).unwrap_or(Tones::TEXT)));
    }
}

/// Says on the look panel what a candidate holds, each thing by the name
/// the adventurer knows it by, `wields a +1 spear`, and what it asks,
/// where a fight's forecast would stand, which the engine runs nowhere in
/// Mereholt's peace; and
/// on the rail that it is for hire, in place of what it knows of the
/// adventurer, as a townsperson's role is.
///
/// In `ViewSet::Annotate`, after the kind's own words, `a sword for hire`,
/// so the kit is read under them.
pub fn show_candidates(
    look: Option<ResMut<InspectView>>,
    rail: Option<ResMut<NearbyView>>,
    candidates: Query<&Candidate>,
    kits: KitSeen,
    mut facets: ResMut<Facets>,
) {
    if let Some(mut view) = look
        && let Some((who, candidate)) = view.subject.as_ref().and_then(|s| candidates.get(s.entity).ok().map(|c| (s.entity, c)))
    {
        for (how, said) in kits.of(who) {
            view.facets.push(facets.facet(KIT_FACET, format!("{} {said}", how.verb())));
        }
        view.facets.push(facets.facet("fee", format!("asks {}", sum_of(GOLD, candidate.fee))));
    }
    if let Some(mut view) = rail {
        for row in view.actors.iter_mut().filter(|r| candidates.contains(r.entity)) {
            (row.doing, row.alert) = (None, None);
            row.facets.push(facets.facet("role", "for hire"));
        }
    }
}

/// Lists talking on the offers screen as `talk to Brenna`, over the
/// engine's `talk Brenna`, as `trade with` reads for a keeper.
///
/// In `ViewSet::Annotate`, over the engine's rows.
pub fn talk_rows(mut view: ResMut<OffersView>, verbs: Res<Verbs>, named: Query<&Name>) {
    let Some(talk) = verbs.get(TALK) else { return };
    for row in view.rows.iter_mut().filter(|r| r.verb == talk) {
        if let Ok(name) = named.get(row.target) {
            row.label = format!("{TALK} to {}", with_the(name.as_str()));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use rl_engine::rl_core::noun::with_article;

    use super::*;
    use crate::dungeon::map_of;
    use crate::lore::Lore;
    use crate::mercenaries::fixtures::{beside, in_mereholt, offered, take, waiting};
    use crate::testing::{arrive, at, gold, make, name_of, pack, pass_turns, said, set_gold, stand_on, still};

    /// A candidate as a test compares them: where it stands, its name, its
    /// fee and what it carries.
    type Seated = (Point, String, u32, Vec<(String, u32)>);

    /// Each candidate waiting, as a test compares them.
    fn seated(app: &mut App) -> Vec<Seated> {
        let all = waiting(app);
        all.into_iter().map(|c| (at(app, c), name_of(app, c), app.world().get::<Candidate>(c).expect("a candidate").fee, pack(app, c))).collect()
    }

    /// Down to the first floor and home again: one more visit.
    fn visit(app: &mut App) {
        arrive(app, map_of(1));
        arrive(app, TOWN);
    }

    /// Where the town marks a table for a candidate.
    fn tables(app: &App) -> Vec<Point> {
        let place = app.world().resource::<WorldMap>().place(TOWN).expect("Mereholt is built");
        place.spots.iter().filter(|s| s.tag == TABLE as u32).map(|s| s.at).collect()
    }

    /// The first visit seats one candidate at each of the tavern's three
    /// tables, over a span of seeds: the mercenary kind on the townsfolk's
    /// side, standing still with no mind, wearing its weapon, named by a
    /// name the run has given, and offering `hire` at the fee its kit is
    /// worth and then `talk`.
    #[test]
    fn the_first_visit_seats_a_candidate_at_each_of_the_three_tables_offering_hire_at_its_fee_and_talk_over_a_span_of_seeds() {
        for seed in 0..6 {
            let (mut app, _) = in_mereholt(seed);
            let mut marks = tables(&app);
            marks.sort();
            assert_eq!(marks.len(), 3, "three tables marked");
            let candidates = waiting(&mut app);
            assert_eq!(candidates.iter().map(|c| at(&app, *c)).collect::<Vec<_>>(), marks, "seed {seed}: one at each table");
            let world = app.world();
            let (bestiary, registries) = (world.resource::<Bestiary>(), world.resource::<Registries>());
            for c in candidates {
                assert_eq!(world.get::<crate::bestiary::Kind>(c).map(|k| k.0), bestiary.defs.id(MERCENARY), "seed {seed}: the one kind");
                assert_eq!(world.get::<Faction>(c).map(|f| f.0), Some(registries.factions.expect(crate::content::TOWNSFOLK)), "seed {seed}: of the town");
                assert!(world.get::<Mind>(c).is_none(), "seed {seed}: waits rather than walks");
                assert!(world.resource::<GivenNames>().has(&name_of(&app, c)), "seed {seed}: its name given");
                let hand = world.get::<Equipped>(c).and_then(|e| e.in_slot(registries.slots.expect("main hand")));
                assert!(hand.is_some(), "seed {seed}: it wields its weapon");
                let fee = world.get::<Candidate>(c).expect("a candidate").fee;
                assert_eq!(offered(&app, c), [(HIRE.to_string(), Some(fee)), (TALK.to_string(), None)], "seed {seed}");
            }
        }
    }

    /// Every visit seats three others, and the last visit's are gone with
    /// everything they carried; and a run of the same seed seats the same
    /// three on the same visit, kit, fee and all, over a span of seeds.
    #[test]
    fn each_visit_seats_three_new_candidates_and_the_same_seed_seats_the_same_three_on_the_same_visit_over_a_span_of_seeds() {
        for seed in 0..4 {
            let (mut app, _) = in_mereholt(seed);
            let first = seated(&mut app);
            let kit: Vec<Entity> = waiting(&mut app).iter().flat_map(|c| app.world().get::<Inventory>(*c).expect("a pack").items.clone()).collect();
            visit(&mut app);
            let second = seated(&mut app);
            assert_eq!(second.len(), 3, "seed {seed}: three again");
            assert!(second.iter().all(|s| !first.iter().any(|f| f.1 == s.1)), "seed {seed}: none of the first three: {first:?} then {second:?}");
            assert!(kit.iter().all(|thing| app.world().get_entity(*thing).is_err()), "seed {seed}: what they carried went with them");
            assert_eq!(app.world().resource::<Tavern>().visits, 2, "seed {seed}: two visits");

            let (mut again, _) = in_mereholt(seed);
            assert_eq!(seated(&mut again), first, "seed {seed}: the same first three");
            visit(&mut again);
            assert_eq!(seated(&mut again), second, "seed {seed}: the same second three");
        }
    }

    /// Over many visits no candidate is called what one of Mereholt's
    /// people is, or what any candidate before it was, on any seed of a
    /// span.
    #[test]
    fn no_candidate_is_named_as_a_townsperson_or_an_earlier_candidate_over_many_visits_over_a_span_of_seeds() {
        for seed in 0..3 {
            let (mut app, _) = in_mereholt(seed);
            let folk: BTreeSet<String> = app.world().resource::<crate::townsfolk::Folk>().names.values().cloned().collect();
            let mut named: Vec<String> = Vec::new();
            for n in 0..8 {
                if n > 0 {
                    visit(&mut app);
                }
                named.extend(seated(&mut app).into_iter().map(|s| s.1));
            }
            let distinct: BTreeSet<&String> = named.iter().collect();
            assert_eq!(distinct.len(), named.len(), "seed {seed}: no name twice: {named:?}");
            assert!(named.iter().all(|n| !folk.contains(n)), "seed {seed}: none a townsperson's: {named:?} against {folk:?}");
        }
    }

    /// Taking up `hire` with gold enough spends exactly the fee, out of the
    /// adventurer's purse and out of play, since the one hired keeps none
    /// of it to drop; with a coin too few it is refused as wanting the
    /// fee, and neither purse moves.
    #[test]
    fn taking_up_hire_spends_exactly_the_fee_and_is_refused_when_the_gold_is_short() {
        for seed in 0..3 {
            let (mut app, me) = in_mereholt(seed);
            let candidate = waiting(&mut app)[0];
            let fee = app.world().get::<Candidate>(candidate).expect("a candidate").fee;
            set_gold(&mut app, me, fee + 7);
            take(&mut app, me, candidate, HIRE);
            assert_eq!((gold(&app, me), gold(&app, candidate)), (7, 0), "seed {seed}: the fee, and no more");

            let (mut app, me) = in_mereholt(seed);
            let candidate = waiting(&mut app)[0];
            let fee = app.world().get::<Candidate>(candidate).expect("a candidate").fee;
            set_gold(&mut app, me, fee - 1);
            take(&mut app, me, candidate, HIRE);
            let hire = app.world().resource::<Verbs>().get(HIRE).expect("a verb");
            let offer = app.world().resource::<OfferedHere>().find(me, candidate, hire).cloned();
            assert_eq!(offer.and_then(|o| o.refused), Some(Refused::Short(fee)), "seed {seed}: wants the fee");
            assert_eq!((gold(&app, me), gold(&app, candidate)), (fee - 1, 0), "seed {seed}: neither purse moved");
        }
    }

    /// While two mercenaries follow the adventurer no candidate offers
    /// `hire`, and talking is all that is left; once one stops following,
    /// every candidate offers it again, first and at its own fee.
    #[test]
    fn hire_is_gone_from_every_candidate_while_two_follow_and_back_when_one_stops() {
        let (mut app, me) = in_mereholt(3);
        let candidates = waiting(&mut app);
        let fees: Vec<u32> = candidates.iter().map(|c| app.world().get::<Candidate>(*c).expect("a candidate").fee).collect();
        let from = at(&app, me);
        let followers = [still(&mut app, MERCENARY, from.offset(-2, 0)), still(&mut app, MERCENARY, from.offset(2, 0))];
        for (follower, name) in followers.iter().zip(["Brenna", "Tolvar"]) {
            app.world_mut().entity_mut(*follower).insert(Mercenary { name: name.to_string(), fee: 0, order: Order::Follow });
        }
        app.world_mut().entity_mut(followers[0]).insert(Leader(me));
        pass_turns(&mut app, 1);
        assert!(candidates.iter().all(|c| offered(&app, *c).iter().any(|(v, _)| v == HIRE)), "one following leaves room");
        app.world_mut().entity_mut(followers[1]).insert(Leader(me));
        pass_turns(&mut app, 1);
        for c in &candidates {
            assert_eq!(offered(&app, *c), [(TALK.to_string(), None)], "two following: talk alone");
        }
        app.world_mut().entity_mut(followers[0]).remove::<Leader>();
        pass_turns(&mut app, 1);
        for (c, fee) in candidates.iter().zip(fees) {
            assert_eq!(offered(&app, *c), [(HIRE.to_string(), Some(fee)), (TALK.to_string(), None)], "one stopped: hire back at its fee");
        }
    }

    /// A candidate given a potion whose kind the adventurer has not learned
    /// names it by its face, on the look panel and when talked to, with
    /// what it wields and its fee; never by its kind. And the look gives
    /// no fight forecast for somebody nothing in Mereholt can harm.
    #[test]
    fn the_look_panel_and_talk_name_a_candidates_unknown_potion_by_its_face_beside_its_kit_and_fee() {
        let (mut app, me) = in_mereholt(5);
        app.add_plugins(rl_engine::rl_ui::InspectViewPlugin)
            .add_systems(Update, (crate::bestiary_lore::annotate_inspect, show_candidates).chain().in_set(ViewSet::Annotate));
        let candidate = waiting(&mut app)[0];
        let kind = "potion of extra healing";
        let lore = app.world().resource::<Lore>();
        let face = lore.face(kind).expect("a potion wears a face").called.clone();
        assert!(!lore.knows(kind), "not yet known");
        let potion = make(&mut app, kind);
        app.world_mut().get_mut::<Inventory>(candidate).expect("a pack").items.push(potion);
        let cell = beside(&mut app, candidate);
        stand_on(&mut app, me, cell);
        let modal = rl_engine::rl_ui::view::inspect::inspect_modal(app.world().resource::<Modals>());
        app.world_mut().resource_mut::<Modals>().open(modal);
        app.world_mut().resource_mut::<InspectView>().cursor = at(&app, candidate);
        app.update();
        let fee = app.world().get::<Candidate>(candidate).expect("a candidate").fee;
        let look = app.world().resource::<InspectView>();
        assert_eq!(look.subject.as_ref().map(|s| s.entity), Some(candidate), "the look is on the candidate");
        let facets: Vec<&str> = look.facets.iter().map(|f| f.text.as_str()).collect();
        assert_eq!(facets.first(), Some(&"a sword for hire"), "its kind's own words first: {facets:?}");
        assert!(facets.iter().any(|f| f.starts_with("wields ")), "what it wields: {facets:?}");
        assert!(facets.contains(&format!("carries {}", with_article(&face)).as_str()), "the potion by its face, {face:?}: {facets:?}");
        assert_eq!(facets.last(), Some(&format!("asks {fee} gold coins").as_str()), "and its fee last: {facets:?}");
        assert!(!facets.iter().any(|f| f.contains(kind)), "never its kind: {facets:?}");
        assert!(look.duel.is_none() && look.odds.is_none(), "and no fight forecast");

        app.world_mut().resource_mut::<Modals>().close();
        take(&mut app, me, candidate, TALK);
        let line = said(&app).last().cloned().expect("something said");
        let name = name_of(&app, candidate);
        assert!(line.starts_with(&format!("{name}: \"I carry ")), "{line}");
        assert!(line.contains(&with_article(&face)) && !line.contains(kind), "the potion by its face: {line}");
        assert!(line.ends_with(&format!("Hire me for {fee} gold coins.\"")), "{line}");
        let told = app.world().resource::<MessageLog>().iter().last().map(|l| l.tone).expect("a line");
        assert_eq!(Some(told), app.world().resource::<Tones>().get(TONE), "in the mercenaries' own tone");
    }

    /// A row's kit is a line a verb, wielded, worn and carried in that
    /// order whatever the pack's, each listing its things as a sentence
    /// does; nothing held is no line at all.
    #[test]
    fn a_rows_kit_is_a_line_a_verb_wielded_then_worn_then_carried() {
        let held = |things: &[(Held, &str)]| things.iter().map(|(how, t)| (*how, t.to_string())).collect::<Vec<_>>();
        assert!(kit_lines(&[]).is_empty());
        let kit = held(&[(Held::Carries, "6 darts"), (Held::Wears, "leather armor"), (Held::Wields, "a +1 spear"), (Held::Wears, "a cap")]);
        assert_eq!(kit_lines(&kit), ["wields a +1 spear", "wears leather armor and a cap", "carries 6 darts"]);
        assert_eq!(kit_lines(&held(&[(Held::Carries, "a potion"), (Held::Carries, "a scroll")])), ["carries a potion and a scroll"]);
    }

    /// On the rail, a candidate in sight says what it holds beneath its
    /// row, each thing as the adventurer knows it: an unknown potion by its
    /// face. A hireling in the company says the same, and the rail does
    /// not list it, since the company does.
    #[test]
    fn the_rail_and_the_company_say_what_each_one_holds_naming_an_unknown_potion_by_its_face() {
        let (mut app, me) = in_mereholt(5);
        app.add_plugins((rl_engine::rl_ui::NearbyViewPlugin, rl_engine::rl_ui::view::PartyViewPlugin))
            .add_systems(Update, (show_orders, show_kit).in_set(ViewSet::Annotate));
        let [candidate, hireling] = [waiting(&mut app)[0], waiting(&mut app)[1]];
        let kind = "potion of extra healing";
        let face = app.world().resource::<Lore>().face(kind).expect("a potion wears a face").called.clone();
        let potion = make(&mut app, kind);
        app.world_mut().get_mut::<Inventory>(candidate).expect("a pack").items.push(potion);
        set_gold(&mut app, me, 1000);
        take(&mut app, me, hireling, HIRE);
        assert_eq!(app.world().get::<Leader>(hireling).map(|l| l.0), Some(me), "hired");
        let cell = beside(&mut app, candidate);
        stand_on(&mut app, me, cell);
        app.update();

        let kit = KIT_FACET.to_string();
        let lines = |row: &Row, facets: &Facets| -> Vec<String> { row.facets.iter().filter(|f| facets.name(f.key) == kit).map(|f| f.text.clone()).collect() };
        let facets = app.world().resource::<Facets>();
        let rail = app.world().resource::<NearbyView>();
        let row = rail.actors.iter().find(|r| r.entity == candidate).expect("the candidate is in sight");
        let said = lines(row, facets);
        assert!(said.first().is_some_and(|l| l.starts_with("wields ")), "what it wields first: {said:?}");
        assert!(said.last().is_some_and(|l| l.starts_with("carries ") && l.contains(&with_article(&face))), "the potion by its face, {face:?}: {said:?}");
        assert!(!said.iter().any(|l| l.contains(kind)), "never its kind: {said:?}");
        assert!(row.facets.iter().filter(|f| facets.name(f.key) == kit).all(|f| f.tone == Tones::MUTED), "said muted, under the name");
        assert!(!rail.lists(hireling), "the hireling is the company's to list");

        let party = app.world().resource::<PartyView>();
        let row = party.rows.iter().find(|r| r.entity == hireling).expect("the hireling follows");
        assert!(lines(row, facets).first().is_some_and(|l| l.starts_with("wields ")), "{:?}", row.facets);
    }

    /// A candidate's line lists its kit as a sentence does, and one with
    /// nothing says so.
    #[test]
    fn a_candidate_says_its_kit_as_a_sentence_lists_it_and_what_it_asks() {
        let held = |things: &[&str]| things.iter().map(|t| (Held::Carries, t.to_string())).collect::<Vec<_>>();
        assert_eq!(candidate_says(&held(&[]), 20), "I carry nothing but my fists. Hire me for 20 gold coins.");
        assert_eq!(candidate_says(&held(&["a sword"]), 21), "I carry a sword. Hire me for 21 gold coins.");
        assert_eq!(
            candidate_says(&held(&["a +1 spear", "leather armor", "6 darts"]), 127),
            "I carry a +1 spear, leather armor and 6 darts. Hire me for 127 gold coins."
        );
    }
}
