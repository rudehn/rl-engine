//! Mereholt's people: who they are, what they are called this run, where
//! they stand or walk, and what they have to say.
//!
//! [`Townsfolk`] is `assets/townsfolk.ron`, a [`Role`] a row: the role's
//! words, its glyph and colour, the mark it is posted at, whether it keeps
//! that post or walks, what a walk into it takes up, a keeper's greeting
//! and the lines talking says. The roles are fixed; who fills them is not.
//! [`name_everyone`] draws every role a name when the run starts, from the
//! `townsfolk` set of `assets/names.ron` through the engine's
//! [`NameSet::draw_distinct`], against every name the run has given, kept
//! in [`GivenNames`] so whatever else the run names later draws against
//! them too. [`Folk`] is the run's half: each role's name and which of its
//! lines it has said, saved with the run, so a continued run talks on
//! where it left off.
//!
//! [`gather`] stands everyone at their mark the first time the adventurer
//! comes to Mereholt; [`shops`](crate::shops) then fills the keepers'
//! shelves. A person is the engine's actor of the townsfolk's side, its
//! `Name` the drawn name, and the role is said under it on the look panel
//! by [`name_roles`]. One who keeps its post has no mind and so stands
//! still; one who wanders has a [`Roam`] mind kept to the tiles its row
//! names, and one who walks rounds the universal
//! [`Patrol`] over a route [`walk_rounds`] lays from
//! one of its props to the next, so nobody walks out of Mereholt.
//!
//! Talking is the engine's offer of [`TALK`]: [`answer_talk`] says the
//! next line not yet said whose [`When`] holds, in the speaker's own
//! colour, `Hesk: "..."`, and once every line that holds is said, the last
//! of them again. The innkeeper's row says `rumours: true`, and tells the
//! run's [`rumours`](crate::rumours) in place of lines of its own. A
//! keeper offers `trade` instead, and [`greet`] puts its greeting under
//! its name on the trade screen.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_rules::ai::brain::Tactic;
use rl_engine::rl_rules::namegen::{self, NameBook, NameSet};
use rl_engine::rl_ui::TradeView;
use ron::extensions::Extensions;
use ron::options::Options;
use serde::{Deserialize, Serialize};

use crate::armory::Armory;
use crate::bestiary::Bestiary;
use crate::content::TOWNSFOLK;
use crate::dungeon::TOWN;
use crate::shops::{POSTS, Wares};
use crate::tally::Tally;
use crate::temple::{Rites, TEMPLE};
use crate::tricks::{Patrol, PatrolRoute};

const TOWNSFOLK_RON: &str = include_str!("../assets/townsfolk.ron");
const NAMES_RON: &str = include_str!("../assets/names.ron");

/// The set in `names.ron` Mereholt's people are named from.
pub const TOWNSFOLK_SET: &str = "townsfolk";

/// The set in `names.ron` the swords for hire are named from.
pub const MERCENARY_SET: &str = "mercenary";

/// The stream the run's names are drawn from, apart from every other
/// draw, so naming the town never moves what a floor holds.
pub const NAMES_STREAM: &[u8] = b"undermere.names";

/// The verb a person who has something to say offers.
pub const TALK: &str = "talk";

/// What talking takes, in hundredths of a step: one step, the turn the
/// walk into them would have been.
pub const TALK_TIME: u32 = 100;

/// A person's health. Nothing in Mereholt is harmed, so it is never spent;
/// it is there so a person reads on the rail as the person they are.
pub const HEALTH: i32 = 20;

/// How often in a hundred a wanderer takes a step rather than standing:
/// a child drifting about the square, not a sentry pacing it.
pub const ROAM_CHANCE_PCT: u32 = 40;

/// How a person gets about.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub enum Moves {
    /// Stands at the post all run.
    Keeps,
    /// Drifts a step now and then, onto these tiles only.
    Wanders(Vec<String>),
    /// Walks from one of these props to the next, round the town.
    Rounds(String),
}

/// What a walk into a person takes up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Offer {
    /// The next of the role's lines.
    Talk,
    /// The shop of the role's [`Post`](crate::shops::Post).
    Trade,
    /// The temple's rites, beside its trade.
    Rites,
}

/// When a line may be said.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum When {
    /// Once the adventurer has been down to this floor.
    Deepest(u32),
    /// Once the monster of this name has died this run, whoever's hand it
    /// fell to.
    Slain(String),
}

impl When {
    /// Whether it holds for a run that has come to `tally`.
    pub fn holds(&self, tally: &Tally) -> bool {
        match self {
            When::Deepest(floor) => tally.deepest >= *floor,
            When::Slain(name) => tally.fallen.iter().any(|f| f == name),
        }
    }
}

/// One thing a person says.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Line {
    /// The words, with a `{role}` for that role's name this run.
    pub say: String,
    /// When it may be said; always, if `None`.
    #[serde(default)]
    pub when: Option<When>,
}

/// One role of Mereholt, as `townsfolk.ron` writes it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Role {
    /// The role in a word: what `{role}` names, one row each.
    pub role: String,
    /// The role's words, "the smith", said under the name on the look panel.
    pub called: String,
    /// The person's glyph on the map.
    pub glyph: char,
    /// `(r, g, b)` in `0..=1`: the glyph's, the person's lines', and a
    /// keeper's wares' on its trade screen.
    pub color: (f32, f32, f32),
    /// The glyph of the person's mark in `mereholt.ron`.
    pub post: char,
    /// How they get about.
    pub moves: Moves,
    /// What a walk into them takes up, in order.
    pub offers: Vec<Offer>,
    /// A keeper's line under its name on the trade screen.
    #[serde(default)]
    pub greeting: Option<String>,
    /// What talking says, in order.
    #[serde(default)]
    pub lines: Vec<Line>,
    /// Whether talking tells this run's rumours, from
    /// [`Rumours`](crate::rumours::Rumours), in place of `lines`.
    #[serde(default)]
    pub rumours: bool,
}

impl Role {
    /// The colour, as the engine takes one.
    pub fn color(&self) -> Color {
        let (r, g, b) = self.color;
        Color::srgb(r, g, b)
    }

    /// The tone the person's lines are said in: their own colour,
    /// declared with the role by [`declare_tones`].
    pub fn tone(&self) -> String {
        format!("{TOWNSFOLK} {}", self.role)
    }
}

/// Every role of Mereholt, in the file's order, which is the order they
/// are named and stood in.
///
/// Read once with the rest of the content and never changed by a run.
#[derive(Resource, Debug, Clone, Default)]
pub struct Townsfolk(Vec<Role>);

impl Townsfolk {
    /// Reads `assets/townsfolk.ron` against the monsters a `slain` names.
    /// Panics naming the file and every problem in it.
    pub fn load(bestiary: &Bestiary) -> Self {
        Self::read(TOWNSFOLK_RON, bestiary).unwrap_or_else(|e| panic!("assets/townsfolk.ron: {e}"))
    }

    /// Reads a townsfolk file, reporting every problem at once, each under
    /// its role: a role or a post written twice, a colour outside `0..=1`,
    /// nothing offered, talk with no line that always holds or lines with
    /// no talk, a trade with no shop or a shop with no trade, rites
    /// anywhere but beside the temple's trade, a tile to wander nobody
    /// could stand on, a `{role}` that is nobody's, a `slain` that is no
    /// monster's, and rumours with lines besides or with no talk.
    pub fn read(text: &str, bestiary: &Bestiary) -> Result<Self, ContentError> {
        let options = Options::default().with_default_extension(Extensions::IMPLICIT_SOME);
        let roles: Vec<Role> = options.from_str(text).map_err(|e| ContentError::Parse(e.to_string()))?;
        let tiles = crate::dungeon::registry();
        let mut errors = Vec::new();
        for (i, r) in roles.iter().enumerate() {
            let mut fault = |problem: String| errors.push(format!("{}: {problem}", r.role));
            if roles[..i].iter().any(|o| o.role == r.role) {
                fault("a second row of the role".into());
            }
            if let Some(other) = roles[..i].iter().find(|o| o.post == r.post) {
                fault(format!("posted at '{}', which is {}'s already", r.post, other.role));
            }
            if ![r.color.0, r.color.1, r.color.2].iter().all(|c| (0.0..=1.0).contains(c)) {
                fault(format!("a colour outside 0..=1, {:?}", r.color));
            }
            if r.offers.is_empty() {
                fault("offers nothing, so a walk into them does nothing".into());
            }
            let talks = r.offers.contains(&Offer::Talk);
            if talks && !r.rumours && !r.lines.iter().any(|l| l.when.is_none()) {
                fault("talks, with no line that always holds to say first".into());
            }
            if !talks && !r.lines.is_empty() {
                fault("has lines, and offers no talk to say them by".into());
            }
            if !talks && r.rumours {
                fault("tells rumours, and offers no talk to tell them by".into());
            }
            if r.rumours && !r.lines.is_empty() {
                fault("has lines, and tells rumours in their place".into());
            }
            let post = POSTS.iter().find(|p| p.role == r.role);
            match (r.offers.contains(&Offer::Trade), post) {
                (true, None) => fault("trades, with no shop in src/shops.rs's POSTS".into()),
                (false, Some(_)) => fault("keeps a shop in src/shops.rs's POSTS, and offers no trade".into()),
                _ => {}
            }
            if r.offers.contains(&Offer::Rites) && post.is_none_or(|p| p.keeper != TEMPLE) {
                fault("offers rites, and is not the temple's keeper".into());
            }
            if let Moves::Wanders(on) = &r.moves {
                if on.is_empty() {
                    fault("wanders onto no tile at all".into());
                }
                for tile in on.iter().filter(|t| tiles.id(t).is_none()) {
                    fault(format!("wanders onto {tile:?}, which is no tile"));
                }
            }
            for line in &r.lines {
                for named in placeholders(&line.say).filter(|n| !roles.iter().any(|o| o.role == *n)) {
                    fault(format!("says {{{named}}}, which is nobody's role"));
                }
                if let Some(When::Slain(name)) = &line.when
                    && bestiary.defs.id(name).is_none()
                {
                    fault(format!("waits for {name:?} to be slain, which is no monster"));
                }
            }
        }
        for post in POSTS.iter().filter(|p| !roles.iter().any(|r| r.role == p.role)) {
            errors.push(format!("{}: keeps a shop in src/shops.rs's POSTS, and has no row", post.role));
        }
        if !errors.is_empty() {
            return Err(ContentError::Invalid(errors));
        }
        Ok(Self(roles))
    }

    /// Every role, in the file's order.
    pub fn roles(&self) -> impl Iterator<Item = &Role> {
        self.0.iter()
    }

    /// The row of `role`.
    pub fn role(&self, role: &str) -> Option<&Role> {
        self.0.iter().find(|r| r.role == role)
    }
}

/// Every `{name}` in `text`, by the name inside the braces.
pub(crate) fn placeholders(text: &str) -> impl Iterator<Item = &str> {
    text.split('{').skip(1).filter_map(|rest| rest.split_once('}').map(|(name, _)| name))
}

/// `assets/names.ron`, every set of names the run draws from.
///
/// Read once with the rest of the content; a name drawn is the run's, in
/// [`GivenNames`], never the book's.
#[derive(Resource, Debug, Clone)]
pub struct NameSets(pub NameBook);

impl NameSets {
    /// Reads `assets/names.ron`. Panics naming the file at a set that does
    /// not read, and when the townsfolk's set or the mercenaries' is
    /// missing.
    pub fn load() -> Self {
        let book = namegen::load(NAMES_RON).unwrap_or_else(|e| panic!("assets/names.ron: {e}"));
        assert!(book.set(TOWNSFOLK_SET).is_some(), "assets/names.ron: no {TOWNSFOLK_SET:?} set to name Mereholt's people from");
        assert!(book.set(MERCENARY_SET).is_some(), "assets/names.ron: no {MERCENARY_SET:?} set to name the swords for hire from");
        Self(book)
    }

    /// The set Mereholt's people are named from.
    pub fn townsfolk(&self) -> &NameSet {
        self.0.set(TOWNSFOLK_SET).expect("checked at load")
    }

    /// The set the swords for hire are named from.
    pub fn mercenary(&self) -> &NameSet {
        self.0.set(MERCENARY_SET).expect("checked at load")
    }
}

/// Every name the run has given anybody, so no two in a run share one.
///
/// The run's, begun empty with each run and saved with it: whatever the
/// run names after the town draws against it too.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GivenNames(BTreeSet<String>);

impl GivenNames {
    /// Whether anybody this run has been given `name`.
    pub fn has(&self, name: &str) -> bool {
        self.0.contains(name)
    }

    /// Marks `name` given.
    pub fn give(&mut self, name: &str) {
        self.0.insert(name.to_string());
    }

    /// Every name given, in order.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(String::as_str)
    }
}

/// Mereholt's people as this run has them: each role's name, and which of
/// its lines it has said.
///
/// The run's, drawn when it starts and saved with it, so the smith is the
/// same somebody all run and somebody else the next, and a line said once
/// is not said again until every line that holds has been.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Folk {
    /// Each role's name this run.
    pub names: BTreeMap<String, String>,
    /// Which of each role's lines it has said, by their place in its row.
    pub said: BTreeMap<String, BTreeSet<usize>>,
}

impl Folk {
    /// The name `role` has this run.
    pub fn name(&self, role: &str) -> Option<&str> {
        self.names.get(role).map(String::as_str)
    }

    /// `text` with every `{role}` said as that role's name this run.
    pub fn say(&self, text: &str) -> String {
        self.names.iter().fold(text.to_string(), |text, (role, name)| text.replace(&format!("{{{role}}}"), name))
    }

    /// Which of `lines` `role` says when talked to, and marks it said: the
    /// first not yet said whose [`When`] `holds`, or once each of those
    /// has been, the last that holds again. `None` for a role with nothing
    /// to say. `lines` are the role's own, or a teller's rumours.
    pub fn speak(&mut self, role: &str, lines: &[Line], holds: impl Fn(&When) -> bool) -> Option<usize> {
        let said = self.said.entry(role.to_string()).or_default();
        let line = next_line(lines, said, holds)?;
        said.insert(line);
        Some(line)
    }
}

/// Which of `lines` talking says next, given which are `said` and what
/// `holds`: the first not yet said that holds, or the last that holds.
pub fn next_line(lines: &[Line], said: &BTreeSet<usize>, holds: impl Fn(&When) -> bool) -> Option<usize> {
    let holding: Vec<usize> = (0..lines.len()).filter(|i| lines[*i].when.as_ref().is_none_or(&holds)).collect();
    holding.iter().copied().find(|i| !said.contains(i)).or(holding.last().copied())
}

/// Draws a name for every role in `townsfolk`, in the file's order, from
/// the run's [`NAMES_STREAM`], each distinct from every name in `given`
/// and given in turn. Panics when the set cannot find one, which a test
/// over a span of seeds keeps from happening.
pub fn name_everyone(townsfolk: &Townsfolk, names: &NameSets, seed: &Seed, given: &mut GivenNames) -> Folk {
    let mut rng = seed.stream(NAMES_STREAM, 0);
    let set = names.townsfolk();
    let mut folk = Folk::default();
    for role in townsfolk.roles() {
        let name = set.draw_distinct(&mut rng, |n| given.has(n)).unwrap_or_else(|| panic!("the townsfolk's names ran dry naming the {}", role.role));
        given.give(&name);
        folk.names.insert(role.role.clone(), name);
    }
    folk
}

/// Declares the tone each role's lines are said in, coloured as its row
/// says, so the log draws a person's words in the person's own colour.
pub fn declare_tones(app: &mut App, townsfolk: &Townsfolk) {
    for role in townsfolk.roles() {
        app.add_tone(&role.tone(), role.color());
    }
}

/// One of Mereholt's people, by role: what the save writes them down as,
/// and what everything else here finds their row by.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct Townsperson(pub String);

/// What making a person reads: the content their offers, their side and a
/// keeper's shop are made from.
pub struct Kit<'a> {
    /// The sides, for the townsfolk's.
    pub registries: &'a Registries,
    /// The verbs, for `talk`, `trade` and the rites.
    pub verbs: &'a Verbs,
    /// The shops, for a keeper's `Trader`.
    pub wares: &'a Wares,
    /// The items, for what a keeper's `Trader` buys.
    pub armory: &'a Armory,
    /// The temple's rites, for the priest's offers.
    pub rites: &'a Rites,
}

impl<'a> Kit<'a> {
    /// The kit as `world` holds it, for a restore.
    pub fn of(world: &'a World) -> Self {
        Self { registries: world.resource(), verbs: world.resource(), wares: world.resource(), armory: world.resource(), rites: world.resource() }
    }
}

/// What [`gather`] reads to make the people.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Makings<'w> {
    registries: Res<'w, Registries>,
    verbs: Res<'w, Verbs>,
    wares: Res<'w, Wares>,
    armory: Res<'w, Armory>,
    rites: Res<'w, Rites>,
}

impl Makings<'_> {
    fn kit(&self) -> Kit<'_> {
        Kit { registries: &self.registries, verbs: &self.verbs, wares: &self.wares, armory: &self.armory, rites: &self.rites }
    }
}

/// Makes the person of `role`, called `name`, nowhere yet: an actor of the
/// townsfolk's side, in the role's glyph and colour, offering what the row
/// offers, a keeper's shop with an empty bag for its shelf, and a mind for
/// one who walks. The caller stands them somewhere, so a first arrival and
/// a continued run make the same person.
pub fn spawn_person(commands: &mut Commands, kit: &Kit, role: &Role, name: &str) -> Entity {
    let mut offers = Vec::new();
    for offer in &role.offers {
        match offer {
            Offer::Talk => offers.push(Offering::new(kit.verbs.get(TALK).expect("UndermerePlugin declares talk"), TALK_TIME)),
            Offer::Trade => offers.push(Offering::new(kit.verbs.get(TRADE).expect("TradePlugin declares trade"), crate::shops::TRADE_TIME)),
            Offer::Rites => offers.extend(kit.rites.offerings(kit.verbs)),
        }
    }
    let mut person = commands.spawn((
        (Townsperson(role.role.clone()), Actor, Blocks, Name::new(name.to_string()), Glyph::new(role.glyph, role.color()).on_layer(5)),
        (Health::full(HEALTH), Faction(kit.registries.factions.expect(TOWNSFOLK)), Offers(offers)),
    ));
    if let Some(post) = POSTS.iter().find(|p| p.role == role.role) {
        person.insert((crate::shops::keeper(*post, kit.wares, kit.armory), Inventory::default()));
    }
    match &role.moves {
        Moves::Keeps => {}
        Moves::Wanders(_) => {
            person.insert(Mind(Arc::new(Brain::new().then(Roam { chance_pct: ROAM_CHANCE_PCT }))));
        }
        Moves::Rounds(_) => {
            person.insert(Mind(Arc::new(Brain::new().then(Patrol))));
        }
    }
    person.id()
}

/// Stands every one of Mereholt's people at their mark the first time the
/// adventurer arrives there, by this run's names, in the file's order, so
/// the run's entities come out the same every time.
///
/// In the arrival chain, after the slots are filled and before the shops
/// fill the keepers' shelves. A mark the town lacks is a broken
/// `mereholt.ron`, and panics naming it.
pub fn gather(
    mut commands: Commands,
    mut entered: MessageReader<PlaceEntered>,
    map: Res<WorldMap>,
    townsfolk: Res<Townsfolk>,
    folk: Res<Folk>,
    makings: Makings,
) {
    if !entered.read().any(|ev| ev.map == TOWN && ev.first) {
        return;
    }
    let place = map.place(TOWN).expect("Mereholt is built when it is entered");
    let kit = makings.kit();
    for role in townsfolk.roles() {
        let at =
            place.spots.iter().find(|s| s.tag == role.post as u32).unwrap_or_else(|| panic!("mereholt.ron marks no '{}' for {}", role.post, role.called)).at;
        let name = folk.name(&role.role).unwrap_or(&role.called);
        let person = spawn_person(&mut commands, &kit, role, name);
        commands.entity(person).insert((Position(at), OnMap(TOWN)));
    }
}

/// The cells beside a wanderer it may step onto by its row, as a
/// [`Sense`]: open or not, since whether
/// one is free is the mind's own to ask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnBeat(pub Vec<Point>);

/// Senses the turn holder's [`OnBeat`]: every cell beside it whose tile is
/// one its row [`Moves::Wanders`] onto. Nothing for anybody else, so
/// [`Roam`] passes them over.
///
/// In `PerceiveSet::Annotate`, beside the engine's own senses.
pub fn sense_beats(mut thinking: ResMut<Thinking>, people: Query<(&Position, &OnMap, &Townsperson)>, townsfolk: Res<Townsfolk>, map: Res<WorldMap>) {
    let Some(actor) = thinking.actor() else { return };
    let Ok((at, on, person)) = people.get(actor) else { return };
    let Some(Moves::Wanders(onto)) = townsfolk.role(&person.0).map(|r| &r.moves) else { return };
    if on.0 != map.current() {
        return;
    }
    let names = &map.tables().names;
    let on_beat = |p: &Point| map.tile(*p).and_then(|t| names.get(t.index())).is_some_and(|name| onto.contains(name));
    let beat = (0..8).map(|i| at.0 + Direction::from_index(i).offset()).filter(on_beat).collect();
    if let Some(snapshot) = thinking.snapshot_mut() {
        snapshot.add_sense(OnBeat(beat));
    }
}

/// Drift over a beat: some chance of a step onto a free cell of
/// [`OnBeat`], never straight back while another is free, and otherwise
/// a wait. The engine's `Wander` with a fence round it: a child let loose
/// with `Wander` walks the causeway to the gate, and into every house.
/// Leaves the turn to the next tactic for anyone with no beat.
#[derive(Debug, Clone, Copy)]
pub struct Roam {
    /// Chance per turn of a step, in percent.
    pub chance_pct: u32,
}

impl Tactic<Entity> for Roam {
    fn name(&self) -> &'static str {
        "roam"
    }

    fn evaluate(&self, ctx: &mut TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
        let beat = &ctx.snapshot.sense::<OnBeat>()?.0;
        if ctx.rng.random_range(0..100) >= self.chance_pct {
            return Some(Decision::Wait);
        }
        let free: Vec<Point> = beat.iter().copied().filter(|p| (ctx.can_step)(*p)).collect();
        let ahead: Vec<Point> = free.iter().copied().filter(|p| Some(*p) != ctx.snapshot.came_from).collect();
        let choice = if ahead.is_empty() { free } else { ahead };
        if choice.is_empty() {
            return Some(Decision::Wait);
        }
        Some(Decision::Step(choice[ctx.rng.random_range(0..choice.len())]))
    }
}

/// Lays a round on everyone whose row walks [`Moves::Rounds`] and has
/// none yet: one waypoint beside each of the props it names on their
/// place, by [`rounds`], starting from the one nearest. Laid when wanted
/// rather than saved, so a continued run's lamplighter sets out again from
/// wherever they stood.
///
/// In `TurnSet::Schedule`, at the start of every pass, so a round is laid
/// the pass after the props are stood.
pub fn walk_rounds(
    mut commands: Commands,
    people: Query<(Entity, &Townsperson, &Position, &OnMap), Without<PatrolRoute>>,
    props: Query<(&Position, &OnMap, &PropKind, Has<Blocks>)>,
    townsfolk: Res<Townsfolk>,
    registries: Res<Registries>,
    map: Res<WorldMap>,
) {
    for (person, who, at, on) in &people {
        let Some(Moves::Rounds(prop)) = townsfolk.role(&who.0).map(|r| &r.moves) else { continue };
        let stops: Vec<Point> = props.iter().filter(|(_, o, k, _)| o.0 == on.0 && registries.props.get(k.0).name == *prop).map(|(p, ..)| p.0).collect();
        let walls: Vec<Point> = props.iter().filter(|(_, o, _, blocks)| o.0 == on.0 && *blocks).map(|(p, ..)| p.0).collect();
        let waypoints = rounds(&stops, |p| map.is_walkable_on(on.0, p) && !walls.contains(&p));
        let Some(nearest) = (0..waypoints.len()).min_by_key(|i| rl_engine::rl_core::geometry::chebyshev(waypoints[*i], at.0)) else { continue };
        if waypoints.len() >= 2 {
            commands.entity(person).insert(PatrolRoute { waypoints, at: nearest });
        }
    }
}

/// A round of `stops`: beside each, the cell `open` allows nearest their
/// middle, so a lamplighter stands on the street side of a lamp post; and
/// the stops in order round that middle, so the round is a loop and not a
/// zigzag. A stop with no open cell beside it is left out.
pub fn rounds(stops: &[Point], open: impl Fn(Point) -> bool) -> Vec<Point> {
    if stops.is_empty() {
        return Vec::new();
    }
    let n = stops.len() as i32;
    let middle = Point::new(stops.iter().map(|p| p.x).sum::<i32>() / n, stops.iter().map(|p| p.y).sum::<i32>() / n);
    let far = |p: Point| (p.x - middle.x).pow(2) + (p.y - middle.y).pow(2);
    let mut beside: Vec<Point> =
        stops.iter().filter_map(|stop| (0..8).map(|i| *stop + Direction::from_index(i).offset()).filter(|p| open(*p)).min_by_key(|p| far(*p))).collect();
    let angle = |p: &Point| f64::from(p.y - middle.y).atan2(f64::from(p.x - middle.x));
    beside.sort_by(|a, b| angle(a).total_cmp(&angle(b)));
    beside
}

/// Says what talking to one of Mereholt's people says: their next line,
/// by [`Folk::speak`], or for one whose row tells rumours the run's next
/// rumour, by [`Rumours::told`](crate::rumours::Rumours::told) over the
/// run's seed, with every `{role}` that role's name, written to
/// the log in the speaker's own colour as `Name: "line"`. The engine has
/// already spent the talk's turn; everything else `Interacted` reports is
/// left for whatever reads it.
///
/// In `TurnSet::React`, the set every game system answering `Interacted`
/// reads from.
pub fn answer_talk(mut done: MessageReader<Interacted>, verbs: Res<Verbs>, people: Query<(&Townsperson, &Name)>, said: Said) {
    let Said { townsfolk, rumours, seed, mut folk, tally, tones, mut tell } = said;
    let Some(talk) = verbs.get(TALK) else { return };
    for ev in done.read().filter(|ev| ev.verb == talk) {
        let Ok((person, name)) = people.get(ev.target) else { continue };
        let Some(role) = townsfolk.role(&person.0) else { continue };
        let lines: Cow<[Line]> = if role.rumours { Cow::Owned(rumours.told(seed.0)) } else { Cow::Borrowed(&role.lines) };
        let Some(line) = folk.speak(&role.role, &lines, |when| when.holds(&tally)) else { continue };
        let words = folk.say(&lines[line].say);
        let tone = tones.get(&role.tone()).unwrap_or(Tones::TEXT);
        tell.write(Tell::new(format!("{}: \"{words}\"", name.as_str()), tone));
    }
}

/// What [`answer_talk`] reads and writes besides the talk itself.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Said<'w> {
    townsfolk: Res<'w, Townsfolk>,
    rumours: Res<'w, crate::rumours::Rumours>,
    seed: Res<'w, Seed>,
    folk: ResMut<'w, Folk>,
    tally: Res<'w, Tally>,
    tones: Res<'w, Tones>,
    tell: MessageWriter<'w, Tell>,
}

/// Says a person's role beside their name, `Hesk`, "the smith", since the
/// name is the run's and says nothing of what they do: under it on the
/// look panel, where a fight's forecast would stand, which the engine runs
/// nowhere in Mereholt's peace; and after it on the rail, in place of what
/// the person knows
/// of the adventurer, since a lamplighter on their round is not a sentry
/// who has not noticed anyone.
///
/// In `ViewSet::Annotate`, reading what the look panel's and the rail's
/// collectors put there this frame; each view is read only where the game
/// draws it.
pub fn name_roles(look: Option<ResMut<InspectView>>, rail: Option<ResMut<NearbyView>>, people: Query<&Townsperson>, roles: Roles) {
    let Roles { townsfolk, mut facets } = roles;
    let role_of = |entity: Entity| people.get(entity).ok().and_then(|p| townsfolk.role(&p.0));
    if let Some(mut view) = look
        && let Some(role) = view.subject.as_ref().and_then(|s| role_of(s.entity))
    {
        view.facets.insert(0, facets.facet("role", role.called.clone()));
    }
    if let Some(mut view) = rail {
        for row in &mut view.actors {
            let Some(role) = role_of(row.entity) else { continue };
            (row.doing, row.alert) = (None, None);
            row.facets.push(facets.facet("role", role.called.clone()));
        }
    }
}

/// What [`name_roles`] reads besides the views.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Roles<'w> {
    townsfolk: Res<'w, Townsfolk>,
    facets: ResMut<'w, Facets>,
}

/// Puts a keeper's greeting on the trade screen, the line under their
/// name, in their own colour as their talk is said.
///
/// In `ViewSet::Annotate`, over the engine's trader row.
pub fn greet(mut view: ResMut<TradeView>, people: Query<&Townsperson>, townsfolk: Res<Townsfolk>, tones: Res<Tones>, mut facets: ResMut<Facets>) {
    let Some(trader) = view.trader.as_mut() else { return };
    let Some(role) = people.get(trader.entity).ok().and_then(|p| townsfolk.role(&p.0)) else { return };
    let Some(greeting) = &role.greeting else { return };
    let tone = tones.get(&role.tone()).unwrap_or(Tones::TEXT);
    trader.facets.push(facets.facet("greeting", format!("\"{greeting}\"")).toned(tone));
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_core::RunSeed;
    use rl_engine::rl_ui::TradeViewPlugin;

    use super::*;
    use crate::content::Library;
    use crate::testing::{adventurer, arrive, at, continued, headless, name_of, pass_turns, said, settle, stand_on, talk_to};

    fn library() -> Library {
        Library::load(&crate::content::effect_kinds(), &Moments::default())
    }

    /// A run begun in Mereholt, its people stood.
    fn in_mereholt(seed: u64) -> (App, Entity) {
        let mut app = headless(RunSeed(seed));
        let me = adventurer(&mut app);
        (app, me)
    }

    /// The person of `role`.
    fn person(app: &mut App, role: &str) -> Entity {
        let world = app.world_mut();
        let found = world.query::<(Entity, &Townsperson)>().iter(world).find(|(_, p)| p.0 == role).map(|(e, _)| e);
        found.unwrap_or_else(|| panic!("nobody is the {role}"))
    }

    /// A tally that has been to `deepest` and seen `fallen` die.
    fn tally(deepest: u32, fallen: &[&str]) -> Tally {
        Tally { deepest, fallen: fallen.iter().map(|f| f.to_string()).collect(), ..Tally::default() }
    }

    /// What `role` says when talked to `times` times running, in a run that
    /// has come to `tally`, by the shipped file.
    fn talks(folk: &mut Folk, townsfolk: &Townsfolk, role: &str, tally: &Tally, times: usize) -> Vec<usize> {
        let role = townsfolk.role(role).expect("a role of the file");
        (0..times).map(|_| folk.speak(&role.role, &role.lines, |w| w.holds(tally)).expect("something to say")).collect()
    }

    /// The shipped file reads, with every role of the design in it, five
    /// keepers and six townsfolk, and every one of them named by a set the
    /// names file has.
    #[test]
    fn the_townsfolk_file_reads_with_every_role_of_the_design() {
        let library = library();
        let roles: Vec<&str> = library.townsfolk.roles().map(|r| r.role.as_str()).collect();
        let design = ["innkeeper", "smith", "apothecary", "curio", "priest", "gatewarden", "lamplighter", "drunk", "gravedigger", "fishwife", "child"];
        assert_eq!(roles, design);
        assert!(library.names.0.set(TOWNSFOLK_SET).is_some());
    }

    /// The townsfolk's set never runs dry: on every seed of a span it draws
    /// a hundred names, each distinct from every one before it, each one
    /// the set itself accepts, in title case and between three and ten
    /// letters, so a run that names many more than the town is never short.
    #[test]
    fn the_townsfolk_set_draws_a_hundred_distinct_names_on_every_seed_of_a_span() {
        let names = NameSets::load();
        let set = names.townsfolk();
        for s in 0..200 {
            let mut rng = Seed(RunSeed(s)).stream(NAMES_STREAM, 0);
            let mut drawn: BTreeSet<String> = BTreeSet::new();
            for n in 0..100 {
                let name = set.draw_distinct(&mut rng, |taken| drawn.contains(taken)).unwrap_or_else(|| panic!("seed {s}: dry at name {n}"));
                assert!(set.accepts(&name) && (3..=10).contains(&name.chars().count()), "seed {s}: {name}");
                assert!(name.starts_with(char::is_uppercase) && name.chars().skip(1).all(char::is_lowercase), "seed {s}: {name} in title case");
                drawn.insert(name);
            }
        }
    }

    /// Every role is named apart from every other and from every name the
    /// run gave before, and each name drawn is given in turn, over a span
    /// of seeds; and the smith is somebody else from one run to the next.
    #[test]
    fn every_role_is_named_apart_from_each_other_and_from_every_name_given_before_over_a_span_of_seeds() {
        let library = library();
        let mut smiths = BTreeSet::new();
        for s in 0..200 {
            let seed = Seed(RunSeed(s));
            // The names the run would have drawn had nobody been named
            // before, given already, as a hireling's might have been: the
            // same stream then meets every one of them on its way.
            let before: Vec<String> = name_everyone(&library.townsfolk, &library.names, &seed, &mut GivenNames::default()).names.into_values().collect();
            let mut given = GivenNames::default();
            before.iter().for_each(|n| given.give(n));
            let folk = name_everyone(&library.townsfolk, &library.names, &seed, &mut given);
            let names: BTreeSet<&String> = folk.names.values().collect();
            assert_eq!(names.len(), 11, "seed {s}: eleven names, none twice: {:?}", folk.names);
            assert!(names.iter().all(|n| !before.contains(n)), "seed {s}: none given before: {:?} against {before:?}", folk.names);
            assert!(names.iter().all(|n| given.has(n)), "seed {s}: each given now");
            smiths.insert(folk.name("smith").expect("named").to_string());
        }
        assert!(smiths.len() > 100, "the smith is somebody else run to run: {} names in 200 runs", smiths.len());
    }

    /// A line's `when` holds it back until it holds: the child asks for a
    /// bone, and asks again, until the adventurer has been down to the
    /// third floor; then asks whether it was dark, and once the goblin
    /// king has fallen, whether he was big.
    #[test]
    fn a_lines_when_holds_it_back_until_it_holds() {
        let library = library();
        let mut folk = Folk::default();
        assert_eq!(talks(&mut folk, &library.townsfolk, "child", &tally(2, &[]), 3), [0, 0, 0], "nothing holds but the first");
        assert_eq!(talks(&mut folk, &library.townsfolk, "child", &tally(3, &[]), 2), [1, 1], "the third floor reached");
        let king = "Grubnak the goblin king";
        assert_eq!(talks(&mut folk, &library.townsfolk, "child", &tally(3, &["the Widow"]), 1), [1], "another fallen is no king");
        assert_eq!(talks(&mut folk, &library.townsfolk, "child", &tally(3, &[king]), 2), [2, 2], "the king fallen");
        let mut early = Folk::default();
        assert_eq!(talks(&mut early, &library.townsfolk, "child", &tally(1, &[king]), 3), [0, 2, 2], "fallen without the third floor: the dark waits");
    }

    /// Talking says each line that holds once, in order, and then the last
    /// of them again and again; a line that comes to hold later is said
    /// next, and then it is the one repeated.
    #[test]
    fn once_every_line_that_holds_is_said_the_last_of_them_is_said_again() {
        let library = library();
        let mut folk = Folk::default();
        assert_eq!(talks(&mut folk, &library.townsfolk, "gatewarden", &tally(1, &[]), 4), [0, 1, 1, 1]);
        assert_eq!(talks(&mut folk, &library.townsfolk, "gatewarden", &tally(5, &[]), 2), [2, 2]);
        assert_eq!(talks(&mut folk, &library.townsfolk, "drunk", &tally(0, &[]), 3), [0, 1, 1], "each person keeps their own count");
    }

    /// A file's problems, every one under its role: a role twice, a post
    /// taken, a colour out of range, nothing offered, talk with no line
    /// that always holds, lines with no talk, rites off the temple, a
    /// trade with no shop, a tile nobody stands on, a `{role}` that is
    /// nobody's and a `slain` that is no monster.
    #[test]
    fn a_townsfolk_file_with_each_mistake_is_refused_naming_each() {
        let library = library();
        let keepers = r#"
            (role: "smith", called: "the smith", glyph: 's', color: (1.0, 0.5, 0.2), post: 'S', moves: Keeps, offers: [Trade]),
            (role: "apothecary", called: "the apothecary", glyph: 'a', color: (0.5, 0.8, 0.5), post: 'Y', moves: Keeps, offers: [Trade]),
            (role: "curio", called: "the curio dealer", glyph: 'c', color: (0.7, 0.5, 0.9), post: 'D', moves: Keeps, offers: [Trade]),
            (role: "priest", called: "the priest", glyph: 'p', color: (0.6, 0.8, 1.0), post: 'P', moves: Keeps, offers: [Trade, Rites]),
        "#;
        let mistakes = r#"
            (role: "a", called: "the a", glyph: 'a', color: (1.0, 1.0, 1.0), post: 'A', moves: Keeps, offers: [Talk], lines: [(say: "x", when: deepest(2))]),
            (role: "a", called: "the a", glyph: 'a', color: (1.5, 1.0, 1.0), post: 'S', moves: Keeps, offers: []),
            (role: "b", called: "the b", glyph: 'b', color: (1.0, 1.0, 1.0), post: 'B', moves: Wanders(["quicksand"]), offers: [Rites], lines: [(say: "hi {nobody}")], rumours: true),
            (role: "c", called: "the c", glyph: 'c', color: (1.0, 1.0, 1.0), post: 'C', moves: Keeps, offers: [Trade, Talk], lines: [(say: "y"), (say: "z", when: slain("Nobody"))]),
        "#;
        let text = format!("[{keepers}{mistakes}]");
        let errors = match Townsfolk::read(&text, &library.bestiary) {
            Err(ContentError::Invalid(errors)) => errors,
            other => panic!("expected the file refused, got {other:?}"),
        };
        let expected = [
            "a: talks, with no line that always holds to say first",
            "a: a second row of the role",
            "a: posted at 'S', which is smith's already",
            "a: a colour outside 0..=1, (1.5, 1.0, 1.0)",
            "a: offers nothing, so a walk into them does nothing",
            "b: has lines, and offers no talk to say them by",
            "b: tells rumours, and offers no talk to tell them by",
            "b: has lines, and tells rumours in their place",
            "b: offers rites, and is not the temple's keeper",
            "b: wanders onto \"quicksand\", which is no tile",
            "b: says {nobody}, which is nobody's role",
            "c: trades, with no shop in src/shops.rs's POSTS",
            "c: waits for \"Nobody\" to be slain, which is no monster",
        ];
        for e in expected {
            assert!(errors.iter().any(|x| x == e), "{e:?} not in {errors:#?}");
        }
        assert_eq!(errors.len(), expected.len(), "{errors:#?}");
    }

    /// On the first arrival in Mereholt every role stands at its mark, by
    /// this run's name for it, in its own glyph and colour, on the
    /// townsfolk's side; those who talk offer talk alone, over a span of
    /// seeds.
    #[test]
    fn everyone_stands_at_their_mark_by_this_runs_name_in_their_own_glyph_over_a_span_of_seeds() {
        for s in 0..3 {
            let (mut app, _) = in_mereholt(s);
            let townsfolk = app.world().resource::<Townsfolk>().clone();
            let folk = app.world().resource::<Folk>().clone();
            let spots = app.world().resource::<WorldMap>().place(TOWN).expect("Mereholt is built").spots.clone();
            let talk = app.world().resource::<Verbs>().get(TALK).expect("declared");
            let side = app.world().resource::<Registries>().factions.expect(TOWNSFOLK);
            for role in townsfolk.roles() {
                let who = person(&mut app, &role.role);
                let world = app.world();
                let mark = spots.iter().find(|spot| spot.tag == role.post as u32).expect("the mark is in mereholt.ron").at;
                assert_eq!(world.get::<Position>(who).map(|p| p.0), Some(mark), "seed {s}: the {} at their mark", role.role);
                assert_eq!(world.get::<OnMap>(who).map(|m| m.0), Some(TOWN));
                assert_eq!(Some(name_of(&app, who).as_str()), folk.name(&role.role), "seed {s}: the {} by this run's name", role.role);
                let glyph = world.get::<Glyph>(who).expect("drawn");
                assert_eq!((glyph.ch, glyph.fg), (role.glyph, role.color()), "seed {s}: the {} in their glyph", role.role);
                assert_eq!(world.get::<Faction>(who).map(|f| f.0), Some(side));
                if role.offers == [Offer::Talk] {
                    let offers: Vec<VerbId> = world.get::<Offers>(who).expect("offers").0.iter().map(|o| o.verb).collect();
                    assert_eq!(offers, [talk], "seed {s}: the {} offers talk alone", role.role);
                }
            }
            let world = app.world_mut();
            assert_eq!(world.query::<&Townsperson>().iter(world).count(), 11, "seed {s}: one person a role");
        }
    }

    /// Walking into one of the townsfolk talks to them, since talk is the
    /// one offer and it has no price: their first line is written to the
    /// log as `Name: "line"` in their own tone, and it takes the turn the
    /// step would have.
    #[test]
    fn walking_into_a_townsperson_talks_and_writes_their_line_in_their_tone() {
        let (mut app, me) = in_mereholt(1);
        let warden = person(&mut app, "gatewarden");
        let there = at(&app, warden);
        let beside = {
            let map = app.world().resource::<WorldMap>();
            (0..8).map(|i| there + Direction::from_index(i).offset()).find(|p| map.is_walkable_on(TOWN, *p)).expect("ground beside the warden")
        };
        stand_on(&mut app, me, beside);
        let before = app.world().resource::<Turns>().now();
        let toward = Direction::between(beside, there).expect("beside");
        app.world_mut().write_message(Intent::new(me, Bump(toward)));
        settle(&mut app);
        let name = name_of(&app, warden);
        let line = format!("{name}: \"The stones let anyone through. It's the coming back they're particular about.\"");
        let log = app.world().resource::<MessageLog>();
        let written = log.iter().find(|l| l.text == line).unwrap_or_else(|| panic!("{line:?} not in {:#?}", said(&app)));
        let tone = app.world().resource::<Tones>().get("townsfolk gatewarden").expect("declared");
        assert_eq!(written.tone, tone, "in the warden's own tone");
        assert_eq!(app.world().resource::<Palette>().get(tone), app.world().resource::<Townsfolk>().role("gatewarden").expect("a row").color());
        assert_eq!(at(&app, me), beside, "talked to, not walked over");
        assert_eq!(app.world().resource::<Turns>().now() - before, TALK_TIME, "the talk took a turn");
    }

    /// A line's `{gatewarden}` says the gatewarden's name this run.
    #[test]
    fn a_lines_gatewarden_says_the_gatewardens_name_this_run() {
        for s in 0..3 {
            let (mut app, _) = in_mereholt(s);
            let warden = person(&mut app, "gatewarden");
            let said = app.world().resource::<Folk>().say("The well's tasted of eggs since spring. {gatewarden} says the lake has a mother.");
            assert_eq!(said, format!("The well's tasted of eggs since spring. {} says the lake has a mother.", name_of(&app, warden)));
        }
    }

    /// The child never steps off the square's cobbles and the lamplighter
    /// walks from lamp post to lamp post, neither ever leaving Mereholt,
    /// over many turns and a span of seeds.
    #[test]
    fn the_wanderers_keep_to_mereholt_the_child_to_the_square_and_the_lamplighter_to_the_lamps_over_many_turns() {
        for s in 0..3 {
            let (mut app, _) = in_mereholt(s);
            let (child, lighter) = (person(&mut app, "child"), person(&mut app, "lamplighter"));
            let cobbles = app.world().resource::<WorldMap>().tables().names.iter().position(|n| n == "cobbles").expect("a tile");
            let lamps: Vec<Point> = {
                let world = app.world_mut();
                let props = world.resource::<Registries>().props.clone();
                world.query::<(&Position, &PropKind)>().iter(world).filter(|(_, k)| props.get(k.0).name == "lamp post").map(|(p, _)| p.0).collect()
            };
            let mut squares: BTreeSet<(i32, i32)> = BTreeSet::new();
            let mut lit: BTreeSet<(i32, i32)> = BTreeSet::new();
            for turn in 0..300 {
                pass_turns(&mut app, 1);
                let world = app.world();
                for who in [child, lighter] {
                    assert_eq!(world.get::<OnMap>(who).map(|m| m.0), Some(TOWN), "seed {s}, turn {turn}: in Mereholt");
                }
                let here = at(&app, child);
                let tile = world.resource::<WorldMap>().tile(here).map(|t| t.index());
                assert_eq!(tile, Some(cobbles), "seed {s}, turn {turn}: the child on the cobbles at {here:?}");
                squares.insert((here.x, here.y));
                let walker = at(&app, lighter);
                lit.extend(lamps.iter().filter(|l| rl_engine::rl_core::geometry::chebyshev(**l, walker) <= 1).map(|l| (l.x, l.y)));
            }
            assert!(squares.len() > 5, "seed {s}: the child wandered: {squares:?}");
            assert_eq!(lit.len(), lamps.len(), "seed {s}: the lamplighter came by every lamp post: {lit:?} of {lamps:?}");
        }
    }

    /// A run saved in Mereholt and continued has the same people by the
    /// same names, every name the run gave, and each person's place in
    /// their lines: the gatewarden, who said the first before the save,
    /// says the second after it, and the keepers trade and the townsfolk
    /// talk as before.
    #[test]
    fn names_and_what_each_has_said_survive_a_save_and_continue() {
        for s in 0..2 {
            let (mut app, me) = in_mereholt(s);
            let first = talk_to(&mut app, me, "gatewarden");
            assert!(first.ends_with("particular about.\""), "seed {s}: {first}");
            arrive(&mut app, crate::dungeon::map_of(1));
            arrive(&mut app, TOWN);
            let text = app.world().resource::<Saves>().load(crate::save::SLOT).expect("the slot reads").expect("the arrival wrote the run down");
            let (folk, given) = (app.world().resource::<Folk>().clone(), app.world().resource::<GivenNames>().clone());
            let offers = |app: &mut App| -> BTreeMap<String, (String, usize)> {
                let world = app.world_mut();
                world.query::<(&Townsperson, &Name, &Offers)>().iter(world).map(|(p, n, o)| (p.0.clone(), (n.as_str().to_string(), o.0.len()))).collect()
            };
            let before = offers(&mut app);
            let mut back = continued(&text);
            assert_eq!(*back.world().resource::<Folk>(), folk, "seed {s}: the same names, and what each said");
            assert_eq!(*back.world().resource::<GivenNames>(), given, "seed {s}: every name the run gave");
            assert_eq!(offers(&mut back), before, "seed {s}: everyone back by name, with their offers");
            let me = adventurer(&mut back);
            let second = talk_to(&mut back, me, "gatewarden");
            assert!(second.ends_with("the gate will find you.\""), "seed {s}: talks on: {second}");
        }
    }

    /// The trade screen's line under a keeper's name is their greeting,
    /// in their own tone, quoted as said.
    #[test]
    fn the_trade_screen_says_the_smiths_greeting_under_their_name() {
        let (mut app, me) = in_mereholt(2);
        app.add_plugins(TradeViewPlugin).add_systems(Update, greet.in_set(ViewSet::Annotate));
        let smith = person(&mut app, "smith");
        let trade = app.world().resource::<Verbs>().get(TRADE).expect("declared");
        let there = at(&app, smith);
        let beside = {
            let map = app.world().resource::<WorldMap>();
            (0..8).map(|i| there + Direction::from_index(i).offset()).find(|p| map.is_walkable_on(TOWN, *p)).expect("ground beside the smith")
        };
        stand_on(&mut app, me, beside);
        app.world_mut().write_message(Intent::new(me, Interact { target: smith, verb: trade, thing: None }));
        settle(&mut app);
        let view = app.world().resource::<TradeView>();
        let trader = view.trader.as_ref().expect("the shop is open");
        assert_eq!(trader.label, name_of(&app, smith), "headed by the smith's name");
        let lines: Vec<&str> = trader.facets.iter().map(|f| f.text.as_str()).collect();
        assert_eq!(lines, ["\"Steel, leather, and arrows by the bundle. I buy what's straight and sell what's sharp.\""]);
        let tone = app.world().resource::<Tones>().get("townsfolk smith").expect("declared");
        assert_eq!(trader.facets[0].tone, tone, "in the smith's own tone");
    }

    /// A person's role is said beside their name: under it on the look
    /// panel, with no fight forecast for somebody nothing in Mereholt can
    /// harm, and after it on the rail, with nothing said of what they know
    /// of the adventurer.
    #[test]
    fn the_look_panel_and_the_rail_say_a_persons_role_beside_their_name() {
        let (mut app, me) = in_mereholt(2);
        app.add_plugins((rl_engine::rl_ui::InspectViewPlugin, rl_engine::rl_ui::NearbyViewPlugin)).add_systems(Update, name_roles.in_set(ViewSet::Annotate));
        let smith = person(&mut app, "smith");
        let there = at(&app, smith);
        let beside = {
            let map = app.world().resource::<WorldMap>();
            (0..8).map(|i| there + Direction::from_index(i).offset()).find(|p| map.is_walkable_on(TOWN, *p)).expect("ground beside the smith")
        };
        stand_on(&mut app, me, beside);
        let modal = rl_engine::rl_ui::view::inspect::inspect_modal(app.world().resource::<Modals>());
        app.world_mut().resource_mut::<Modals>().open(modal);
        app.world_mut().resource_mut::<InspectView>().cursor = there;
        app.update();
        let look = app.world().resource::<InspectView>();
        assert_eq!(look.subject.as_ref().map(|s| s.entity), Some(smith), "the look is on the smith");
        assert_eq!(look.facets.first().map(|f| f.text.as_str()), Some("the smith"), "the role first under the name");
        assert!(look.duel.is_none() && look.odds.is_none(), "and no fight forecast");
        let rail = app.world().resource::<NearbyView>();
        let row = rail.actors.iter().find(|r| r.entity == smith).expect("the smith on the rail");
        assert_eq!(row.facets.iter().map(|f| f.text.as_str()).collect::<Vec<_>>(), ["the smith"]);
        assert_eq!((row.doing, row.alert), (None, None), "nothing said of what the smith knows");
    }
}
