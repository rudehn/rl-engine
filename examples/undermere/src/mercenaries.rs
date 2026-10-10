//! The swords for hire at the Sunken Lantern: one kind, its names, and
//! what each carries and asks.
//!
//! There is one kind, [`MERCENARY`] in `monsters.ron` with the trait of
//! the same name, and what makes one unlike the next is its kit and its
//! name, not its kind. The names are the `mercenary` set of `names.ron`,
//! [`NameSets::mercenary`](crate::townsfolk::NameSets::mercenary).
//! [`Mercenaries`] is `assets/mercenaries.ron` read against the
//! [`Armory`]: the [`Fee`], the parts of a kit, each a chance and rows the
//! engine's [`LootTable`] draws at a band, and the levels a thing that
//! takes them rolls on top of its kind's plain one. The band is the
//! deepest floor the run has reached, nought on the first evening, so a
//! later visit can find what the first could not.
//!
//! [`Mercenaries::kit`] and [`Mercenaries::fee`] are the pure half, tested
//! without an `App`: a kit at a band from whatever generator it is handed,
//! and what a kit costs. [`make_kit`] makes a kit's things to be carried.
//!
//! [`seat_candidates`] seats three of them at the tavern's tables on every
//! arrival in Mereholt, drawn from the run's [`TAVERN_STREAM`] at the
//! visit's number, which [`Tavern`] counts; the last visit's that nobody
//! hired are gone first. Each is a [`Candidate`] made by
//! [`spawn_candidate`]: the one kind on the townsfolk's side, with no mind,
//! wearing its kit and offering [`HIRE`] at its fee beside talk. `hire` is
//! the engine's priced offer, so a walk into a candidate opens the offers
//! screen, an adventurer short of the fee is refused there, and one who
//! takes it up has paid before the game hears of it: whatever hiring then
//! does answers the `Interacted` it sends, in `TurnSet::React`.
//! [`keep_hire_offered`] takes `hire` off every candidate while the party
//! is full. What a candidate holds is said as the adventurer knows it, by
//! the name each thing carries, when talked to by [`answer_candidates`]
//! and on the look panel by [`show_candidates`].
//!
//! Undermere's own rather than a keeper's [`Wares`](crate::shops::Wares):
//! a kit is a part at a time, each on its own chance, where a shelf is a
//! count of draws; and a kit's level may be below nought, which the
//! engine's [`LevelTable`] refuses for a find, so the levels are a loot
//! table whose item is the level. A part's rows are the engine's spawn
//! file, read by the reader `item_spawns.ron` is, so a row is written and
//! checked the one way.

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_core::noun::{Countability, counted_as, with_the};
use rl_engine::rl_rules::loot::{self, LootRow, LootTable};
use rl_engine::rl_ui::view::offers::OffersView;
use rl_engine::rl_ui::view::sum_of;
use ron::extensions::Extensions;
use ron::options::Options;
use ron::value::RawValue;
use serde::{Deserialize, Serialize};

use crate::armory::{Armory, ItemDef, shape_of, spawn_item_at, spawn_items};
use crate::bestiary::{Bestiary, spawn_monster};
use crate::dungeon::{FLOORS, TOWN};
use crate::finds::GOLD;
use crate::shops::Ware;
use crate::tally::Tally;
use crate::townsfolk::{GivenNames, NameSets, TALK, TALK_TIME};

const MERCENARIES_RON: &str = include_str!("../assets/mercenaries.ron");

/// The one kind of hireling, by its name in `monsters.ron`.
pub const MERCENARY: &str = "mercenary";

/// What a candidate asks to be hired, `fee` in `mercenaries.ron`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fee {
    /// What a candidate carrying nothing worth anything asks.
    pub base: u32,
    /// The percent of its kit's worth it asks on top.
    pub share_pct: u32,
}

/// One part of a kit: whether a candidate carries one, and which.
#[derive(Debug, Clone)]
struct Part {
    /// What the file calls it.
    name: String,
    /// Percent of candidates carrying one.
    chance: u32,
    /// Which one, drawn at the band.
    rows: LootTable<Id<ItemDef>>,
}

/// Everything `mercenaries.ron` holds: the fee, the parts of a kit, and
/// the levels its things are rolled at.
#[derive(Resource, Debug, Clone)]
pub struct Mercenaries {
    fee: Fee,
    parts: Vec<Part>,
    levels: LootTable<i32>,
}

impl Mercenaries {
    /// Reads `assets/mercenaries.ron` against `registries` and `armory`.
    /// Panics naming the file and every problem in it.
    pub fn load(registries: &Registries, armory: &Armory) -> Self {
        Self::read(MERCENARIES_RON, registries, armory).unwrap_or_else(|e| panic!("assets/mercenaries.ron: {e}"))
    }

    /// Reads a mercenaries file: every problem in it at once, each under
    /// the part or the level row it is in, an item `items.ron` has not
    /// among them.
    pub fn read(text: &str, registries: &Registries, armory: &Armory) -> Result<Self, ContentError> {
        let options = Options::default().with_default_extension(Extensions::IMPLICIT_SOME);
        let file: AuthoredFile = options.from_str(text).map_err(|e| ContentError::Parse(e.to_string()))?;
        let names = registries.names().with("item", &armory.defs);
        let tags = |d: &ItemDef| d.tags.iter().map(|t| t.id()).collect();
        let mut errors = Vec::new();
        let mut parts: Vec<Part> = Vec::new();
        for a in file.parts {
            let at = a.name.clone();
            if parts.iter().any(|p| p.name == a.name) {
                errors.push(format!("{at}: a second part of the name"));
            }
            if !(1..=100).contains(&a.chance) {
                errors.push(format!("{at}: a chance of {} percent", a.chance));
            }
            match loot::load(a.rows.get_ron(), &names, &armory.defs, tags) {
                Ok(rows) => {
                    if rows.rows().is_empty() {
                        errors.push(format!("{at}: no rows, so never carried"));
                    }
                    for row in rows.rows() {
                        floors(&format!("{at}: {}", armory.defs.get(row.item).name), row.bands, &mut errors);
                    }
                    parts.push(Part { name: a.name, chance: a.chance, rows });
                }
                Err(ContentError::Invalid(problems)) => errors.extend(problems.into_iter().map(|e| format!("{at}: {e}"))),
                Err(other) => errors.push(format!("{at}: {other}")),
            }
        }
        let mut levels = Vec::new();
        for (i, a) in file.levels.into_iter().enumerate() {
            let at = format!("levels: row {}", i + 1);
            floors(&at, a.bands, &mut errors);
            if a.weight == 0 {
                errors.push(format!("{at}: a weight of nothing is never drawn"));
            }
            levels.push(LootRow::new(a.level).bands(a.bands.0, a.bands.1).weight(a.weight));
        }
        if !errors.is_empty() {
            return Err(ContentError::Invalid(errors));
        }
        Ok(Self { fee: file.fee, parts, levels: LootTable::new(levels, |level| level.to_string()) })
    }

    /// Rolls one candidate's kit at `band`, the deepest floor the run has
    /// reached, from `rng`: each part in the file's order, carried on its
    /// own chance, which one by its rows at the band, and a level for a
    /// thing that takes levels by the level rows at the band on top of
    /// its kind's [plain level](Armory::plain_level). A part with no row
    /// at the band is not carried, and a thing that takes no levels draws
    /// none and is at nought.
    pub fn kit(&self, band: u32, armory: &Armory, rng: &mut impl Rng) -> Vec<Ware> {
        let band = band as i32;
        let mut kit = Vec::new();
        for part in &self.parts {
            if rng.random_range(0..100) >= part.chance {
                continue;
            }
            let Some((def, count)) = part.rows.pick(band, rng) else { continue };
            let level = armory.plain_level(def).map_or(0, |plain| plain + self.levels.pick(band, rng).map_or(0, |(level, _)| level));
            kit.push(Ware { def, count, level });
        }
        kit
    }

    /// What a candidate carrying `kit` asks: the fee's base, plus its share
    /// of what everything in the kit is worth with its kind and its level
    /// told, as [`Armory::told_worth`] prices it. A thing below nought is
    /// worth nothing, so a cursed sword adds nothing to its carrier's
    /// price; the share rounds down.
    pub fn fee(&self, kit: &[Ware], armory: &Armory) -> u32 {
        let worth: u64 = kit.iter().map(|w| u64::from(armory.told_worth(w.def, w.level).unwrap_or(0)) * u64::from(w.count)).sum();
        let share = worth * u64::from(self.fee.share_pct) / 100;
        self.fee.base.saturating_add(u32::try_from(share).unwrap_or(u32::MAX))
    }
}

/// Makes the things of `kit`, placed nowhere, for a candidate to carry:
/// each that takes levels one at a time at its own, so one below nought
/// is cursed and will not come off once worn, as a thing found below is,
/// and the rest as [`spawn_items`] makes them. Made as told rather than
/// found, never unfamiliar, since a hireling says what it is selling.
pub fn make_kit(commands: &mut Commands, armory: &Armory, kit: &[Ware], registries: &Registries) -> Vec<Entity> {
    let mut made = Vec::new();
    for w in kit {
        if armory.takes_levels(w.def) {
            made.extend((0..w.count).map(|_| spawn_item_at(commands, armory, w.def, w.level, registries)));
        } else {
            made.extend(spawn_items(commands, armory, w.def, w.count, registries));
        }
    }
    made
}

/// The stream a visit's candidates are drawn from, the visit's number its
/// index: apart from every other draw, so who waits in the tavern never
/// moves what a floor holds or a shelf stocks.
pub const TAVERN_STREAM: &[u8] = b"undermere.tavern";

/// The glyph of the marks in `mereholt.ron` a candidate waits at, one
/// beside each of the tavern's tables: as many candidates a visit as there
/// are marks.
pub const TABLE: char = 'x';

/// The verb a candidate offers to be hired by.
pub const HIRE: &str = "hire";

/// What hiring takes, in hundredths of a step: one step, as talking does.
pub const HIRE_TIME: u32 = 100;

/// How many may follow the adventurer before no candidate offers
/// [`HIRE`]: a party of three is as many as a corridor fights in.
pub const PARTY: usize = 2;

/// The tone a mercenary's words are said in, its kind's own colour,
/// declared by [`declare_tone`].
pub const TONE: &str = "mercenary";

/// What `UNDERMERE_NEAR` names to stand the adventurer in the tavern beside
/// the candidates, for a photograph of them.
pub const NEAR_CANDIDATES: &str = "candidates";

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
        let shape = shape_of(armory.defs.get(ware.def), registries);
        for thing in make_kit(commands, armory, std::slice::from_ref(ware), registries) {
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
    let offers = vec![
        Offering::new(verbs.get(HIRE).expect("UndermerePlugin declares hire"), HIRE_TIME).priced(fee),
        Offering::new(verbs.get(TALK).expect("UndermerePlugin declares talk"), TALK_TIME),
    ];
    commands.entity(candidate).remove::<Mind>().insert((
        Name::new(name.to_string()),
        Candidate { fee },
        Inventory { items: pack },
        Equipped(worn),
        Offers(offers),
    ));
    candidate
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
/// A follower is anything whose `Leader` is the adventurer.
///
/// In `TurnSet::Schedule`, at the start of every pass, so whatever joined
/// or left in the last pass's reactions is counted before anyone decides.
pub fn keep_hire_offered(mut candidates: Query<(&Candidate, &mut Offers)>, followers: Query<&Leader>, players: Query<Entity, With<Player>>, verbs: Res<Verbs>) {
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

/// `items` as a sentence lists them: `a`, `a and b`, `a, b and c`.
fn and_list(items: &[String]) -> String {
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
/// the adventurer knows it by, `wields a +1 spear`, and what it asks, in
/// place of a fight's forecast, since nothing in Mereholt is harmed; and
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
        let forecast = facets.declare("forecast");
        view.facets.retain(|f| f.key != forecast);
        (view.duel, view.odds) = (None, None);
        for (how, said) in kits.of(who) {
            view.facets.push(facets.facet("kit", format!("{} {said}", how.verb())));
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

/// Pushes onto `errors` when `bands` is not a range of bands from nought,
/// the first evening, to the deepest floor.
fn floors(at: &str, bands: (i32, i32), errors: &mut Vec<String>) {
    let (first, last) = bands;
    let deepest = FLOORS as i32;
    if first > last || first < 0 || last > deepest {
        errors.push(format!("{at}: bands {first} to {last} are not a range within 0 to {deepest}"));
    }
}

/// The file as written: the fee, the parts with their rows left raw so the
/// engine's spawn reader reads them, and the level rows.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredFile {
    fee: Fee,
    parts: Vec<AuthoredPart>,
    levels: Vec<AuthoredLevel>,
}

/// A part, as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredPart {
    name: String,
    chance: u32,
    rows: Box<RawValue>,
}

/// A level row, as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredLevel {
    bands: (i32, i32),
    level: i32,
    weight: u32,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use bevy::ecs::world::CommandQueue;
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use rl_engine::rl_core::RunSeed;
    use rl_engine::rl_rules::prefab::{Pick, Slot};

    use rl_engine::rl_core::noun::with_article;

    use super::*;
    use crate::content::Library;
    use crate::dungeon::map_of;
    use crate::lore::Lore;
    use crate::testing::{adventurer, arrive, at, gold, headless, make, name_of, pack, pass_turns, said, set_gold, settle, stand_on, still};
    use crate::townsfolk::NAMES_STREAM;

    fn library() -> Library {
        Library::load(&crate::content::effect_kinds(), &Moments::default())
    }

    /// The kits of a span of seeds at `band`, each from a generator of its
    /// own seed.
    fn kits(library: &Library, band: u32) -> Vec<Vec<Ware>> {
        (0..2000).map(|s| library.mercenaries.kit(band, &library.armory, &mut StdRng::seed_from_u64(s))).collect()
    }

    fn named(library: &Library, w: &Ware) -> String {
        library.armory.defs.get(w.def).name.clone()
    }

    fn tagged(library: &Library, w: &Ware, tag: &str) -> bool {
        let tag = library.registries.tags.expect(tag);
        library.armory.defs.get(w.def).tags.iter().any(|t| t.id() == tag)
    }

    fn refused(library: &Library, text: &str) -> String {
        match Mercenaries::read(text, &library.registries, &library.armory) {
            Err(e) => e.to_string(),
            Ok(_) => panic!("expected the fixture refused"),
        }
    }

    /// The mercenaries' set never runs dry: on every seed of a span it
    /// draws a hundred names, each distinct from every one before it, each
    /// one the set itself accepts, every word in title case; and about one
    /// in four carries a byname.
    #[test]
    fn the_mercenary_set_draws_a_hundred_distinct_names_on_every_seed_of_a_span() {
        let names = library().names;
        let set = names.mercenary();
        let mut bynamed = 0;
        for s in 0..200 {
            let mut rng = Seed(RunSeed(s)).stream(NAMES_STREAM, 1);
            let mut drawn: BTreeSet<String> = BTreeSet::new();
            for n in 0..100 {
                let name = set.draw_distinct(&mut rng, |taken| drawn.contains(taken)).unwrap_or_else(|| panic!("seed {s}: dry at name {n}"));
                assert!(set.accepts(&name), "seed {s}: {name}");
                for word in name.split(' ') {
                    assert!(word.starts_with(char::is_uppercase) && word.chars().skip(1).all(char::is_lowercase), "seed {s}: {name} in title case");
                }
                bynamed += u32::from(name.contains(' '));
                drawn.insert(name);
            }
        }
        assert!((4000..=6000).contains(&bynamed), "{bynamed} of 20000 with a byname");
    }

    /// Every kit has a weapon; about four in five have armor and about one
    /// in two something to drink or throw, each within a few in a hundred,
    /// at the first evening's band and the deepest alike.
    #[test]
    fn a_kit_always_has_a_weapon_armor_about_four_times_in_five_and_a_consumable_about_one_in_two_over_a_span_of_seeds() {
        let library = library();
        for band in [0, FLOORS] {
            let (mut armor, mut consumable) = (0, 0);
            for kit in kits(&library, band) {
                assert_eq!(kit.iter().filter(|w| tagged(&library, w, "weapon")).count(), 1, "band {band}: one weapon in {kit:?}");
                armor += kit.iter().filter(|w| tagged(&library, w, "armor")).count();
                consumable += kit.iter().filter(|w| !tagged(&library, w, "weapon") && !tagged(&library, w, "armor")).count();
            }
            assert!((1520..=1680).contains(&armor), "band {band}: {armor} of 2000 with armor");
            assert!((900..=1100).contains(&consumable), "band {band}: {consumable} of 2000 with a consumable");
        }
    }

    /// A broadsword, chain mail and a level of two wait for the third
    /// floor: never in a kit on the first evening, and each in some kit of
    /// a span once the run has been down to the third.
    #[test]
    fn a_later_band_reaches_a_broadsword_chain_mail_and_a_level_two_the_first_evening_never_does_over_a_span_of_seeds() {
        let library = library();
        let reached = |band: u32| {
            let things: Vec<Ware> = kits(&library, band).into_iter().flatten().collect();
            let has = |name: &str| things.iter().any(|w| named(&library, w) == name);
            (has("broadsword"), has("chain mail"), things.iter().any(|w| w.level == 2))
        };
        assert_eq!(reached(0), (false, false, false), "the first evening");
        assert_eq!(reached(3), (true, true, true), "once floor three is reached");
    }

    /// A fee is its base, twenty, plus half of what its kit is worth told:
    /// a sword one better is its price and a hundred, armor and a potion at
    /// plain their prices, darts their price each; a cursed sword adds
    /// nothing, and an empty kit costs the base. Over a span of seeds no
    /// candidate costs less than the base or more than its kit's whole
    /// worth and the base together.
    #[test]
    fn a_fee_is_its_base_plus_its_share_of_its_kits_worth() {
        let library = library();
        let (merc, armory) = (&library.mercenaries, &library.armory);
        let ware = |name: &str, count: u32, level: i32| Ware { def: armory.defs.expect(name), count, level };
        let kit = [ware("sword", 1, 1), ware("leather armor", 1, 0), ware("dart", 6, 0), ware("potion of healing", 1, 0)];
        assert_eq!(merc.fee(&kit, armory), 20 + (60 + 100 + 40 + 6 * 3 + 40) / 2);
        assert_eq!(merc.fee(&[ware("sword", 1, -1)], armory), 20, "a cursed sword is worth nothing");
        assert_eq!(merc.fee(&[], armory), 20, "nothing worth anything");
        for kit in kits(&library, 5) {
            let worth: u32 = kit.iter().map(|w| armory.told_worth(w.def, w.level).unwrap_or(0) * w.count).sum();
            assert_eq!(merc.fee(&kit, armory), 20 + worth / 2, "{kit:?}");
        }
    }

    /// A thing rolled below nought is made cursed as a thing found below
    /// is: at its level and stuck once worn; one at nought or better is
    /// not. Over a span of seeds about one weapon in five is rolled below.
    #[test]
    fn a_level_below_nought_is_cursed_as_one_found_below_is_over_a_span_of_seeds() {
        let library = library();
        let mut world = World::new();
        let mut cursed = 0;
        for kit in kits(&library, 0) {
            let mut queue = CommandQueue::default();
            let made = make_kit(&mut Commands::new(&mut queue, &world), &library.armory, &kit, &library.registries);
            queue.apply(&mut world);
            let levelled: Vec<&Ware> = kit.iter().filter(|w| library.armory.takes_levels(w.def)).collect();
            let things: Vec<Entity> = made.into_iter().filter(|&e| world.get::<Enchant>(e).is_some()).collect();
            assert_eq!(things.len(), levelled.len(), "one thing a levelled ware in {kit:?}");
            for (w, thing) in levelled.iter().zip(things) {
                let level = world.get::<Enchant>(thing).map(|e| e.level);
                assert_eq!(level, Some(w.level), "{}", named(&library, w));
                assert_eq!(world.get::<Stuck>(thing).is_some(), w.level < 0, "{} at {}", named(&library, w), w.level);
                cursed += u32::from(w.level < 0 && tagged(&library, w, "weapon"));
            }
        }
        assert!((330..=470).contains(&cursed), "{cursed} of 2000 weapons below nought");
    }

    /// The mercenary is one kind on the adventurer's footing: their health,
    /// sapient wits that use and wear what they carry, and running at a
    /// quarter of its health.
    #[test]
    fn the_mercenary_is_one_kind_on_the_adventurers_health_that_thinks_and_runs_at_a_quarter() {
        let library = library();
        let def = library.bestiary.defs.get(library.bestiary.defs.expect(MERCENARY));
        assert_eq!(def.hp, crate::run::HEALTH);
        assert!(def.folded.wits.has(Wits::SAPIENT) && def.folded.wits.has(Wits::USES) && def.folded.wits.has(Wits::EQUIPS), "{:?}", def.folded.wits);
        assert_eq!(def.folded.flee_at, Some(25));
    }

    /// The mercenary is never drawn as a floor's monster: no row of the
    /// spawn table names it at any band, no role a piece's slot draws from
    /// does, which draws by that table anyway, and no piece stands one by
    /// its kind; it is only ever met in town.
    #[test]
    fn the_mercenary_is_never_drawn_as_a_floors_monster() {
        let library = library();
        let merc = library.bestiary.defs.expect(MERCENARY);
        for band in -1..=FLOORS as i32 + 1 {
            assert!(library.bestiary.spawns.at(band).all(|row| row.item != merc), "drawn at band {band}");
        }
        let undermere = crate::dungeon::Undermere::new(RunSeed(0));
        let prefabs = crate::prefabs::load(undermere.tiles());
        assert!(prefabs.roles().iter().all(|(_, role)| !role.members.contains(&merc)), "a member of a role");
        for (_, piece) in prefabs.defs().iter() {
            for (glyph, slot) in piece.slots() {
                assert!(!matches!(slot, Slot::Monster { pick: Pick::Kind(kind), .. } if *kind == merc), "{} stands one at {glyph:?}", piece.name);
            }
        }
    }

    /// An item `items.ron` has not is a load error naming it and its part.
    #[test]
    fn every_item_mercenaries_ron_names_exists_or_the_load_names_it() {
        let library = library();
        let error = refused(
            &library,
            r#"(fee: (base: 20, share_pct: 50), parts: [(name: "weapon", chance: 100, rows: [(item: "sword of legend", bands: (0, 10), weight: 1)])], levels: [])"#,
        );
        assert!(error.contains("weapon") && error.contains("sword of legend"), "{error}");
    }

    /// A chance outside one to a hundred, a second part of one name, a
    /// band past the deepest floor or before the first evening, and a
    /// weight of nothing are each refused, naming where.
    #[test]
    fn a_bad_chance_a_second_part_of_a_name_a_band_past_the_floors_and_a_weight_of_nothing_are_refused_naming_each() {
        let library = library();
        let error = refused(
            &library,
            r#"(fee: (base: 20, share_pct: 50), parts: [
                (name: "weapon", chance: 0, rows: [(item: "sword", bands: (0, 11), weight: 1)]),
                (name: "weapon", chance: 50, rows: [(item: "dagger", bands: (0, 10), weight: 0)]),
            ], levels: [(bands: (-1, 10), level: 0, weight: 0)])"#,
        );
        for said in
            ["weapon: a chance of 0 percent", "weapon: a second part", "weapon: sword: bands 0 to 11", "weight of nothing", "levels: row 1: bands -1 to 10"]
        {
            assert!(error.contains(said), "{said:?} not in:\n{error}");
        }
    }

    /// A run begun in Mereholt, its first visit's candidates seated.
    fn in_mereholt(seed: u64) -> (App, Entity) {
        let mut app = headless(RunSeed(seed));
        let me = adventurer(&mut app);
        (app, me)
    }

    /// The candidates waiting, by where they stand.
    fn waiting(app: &mut App) -> Vec<Entity> {
        let world = app.world_mut();
        let mut found: Vec<(Point, Entity)> = world.query_filtered::<(&Position, Entity), With<Candidate>>().iter(world).map(|(p, e)| (p.0, e)).collect();
        found.sort();
        found.into_iter().map(|(_, e)| e).collect()
    }

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

    /// A free cell beside `who` the adventurer could stand on.
    fn beside(app: &mut App, who: Entity) -> Point {
        let there = at(app, who);
        let world = app.world_mut();
        let taken: Vec<Point> = world.query_filtered::<&Position, With<Blocks>>().iter(world).map(|p| p.0).collect();
        let map = app.world().resource::<WorldMap>();
        (0..8).map(|i| there + Direction::from_index(i).offset()).find(|p| map.is_walkable_on(TOWN, *p) && !taken.contains(p)).expect("ground beside them")
    }

    /// Stands the adventurer beside `who` and takes up `verb` on it, as the
    /// offers screen's choice does.
    fn take(app: &mut App, me: Entity, who: Entity, verb: &str) {
        let cell = beside(app, who);
        stand_on(app, me, cell);
        let verb = app.world().resource::<Verbs>().get(verb).expect("a verb");
        app.world_mut().write_message(Intent::new(me, Interact { target: who, verb }));
        settle(app);
    }

    /// The verbs `who` offers, each with its price.
    fn offered(app: &App, who: Entity) -> Vec<(String, Option<u32>)> {
        let verbs = app.world().resource::<Verbs>();
        let offers = app.world().get::<Offers>(who).expect("offers");
        offers.0.iter().map(|o| (verbs.name(o.verb).to_string(), o.price)).collect()
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
    /// adventurer's purse and into the candidate's pack; with a coin too
    /// few it is refused as wanting the fee, and neither purse moves.
    #[test]
    fn taking_up_hire_spends_exactly_the_fee_and_is_refused_when_the_gold_is_short() {
        for seed in 0..3 {
            let (mut app, me) = in_mereholt(seed);
            let candidate = waiting(&mut app)[0];
            let fee = app.world().get::<Candidate>(candidate).expect("a candidate").fee;
            set_gold(&mut app, me, fee + 7);
            take(&mut app, me, candidate, HIRE);
            assert_eq!((gold(&app, me), gold(&app, candidate)), (7, fee), "seed {seed}: the fee, and no more");

            let (mut app, me) = in_mereholt(seed);
            let candidate = waiting(&mut app)[0];
            let fee = app.world().get::<Candidate>(candidate).expect("a candidate").fee;
            set_gold(&mut app, me, fee - 1);
            take(&mut app, me, candidate, HIRE);
            let hire = app.world().resource::<Verbs>().get(HIRE).expect("a verb");
            let offer = app.world().resource::<OfferedHere>().find(me, candidate, hire).copied();
            assert_eq!(offer.and_then(|o| o.refused), Some(Refused::Short(fee)), "seed {seed}: wants the fee");
            assert_eq!((gold(&app, me), gold(&app, candidate)), (fee - 1, 0), "seed {seed}: neither purse moved");
        }
    }

    /// While two follow the adventurer no candidate offers `hire`, and
    /// talking is all that is left; once one stops following, every
    /// candidate offers it again, first and at its own fee.
    #[test]
    fn hire_is_gone_from_every_candidate_while_two_follow_and_back_when_one_stops() {
        let (mut app, me) = in_mereholt(3);
        let candidates = waiting(&mut app);
        let fees: Vec<u32> = candidates.iter().map(|c| app.world().get::<Candidate>(*c).expect("a candidate").fee).collect();
        let from = at(&app, me);
        let followers = [still(&mut app, MERCENARY, from.offset(-2, 0)), still(&mut app, MERCENARY, from.offset(2, 0))];
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
