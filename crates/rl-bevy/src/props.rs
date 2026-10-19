//! Props: what stands on a map that is neither an actor nor an item.
//!
//! Opt-in by adding [`PropsPlugin`]. A prop is a cell, a name and a look,
//! and every other part of one, what it offers, what it holds, what sets
//! it off, how hard it is to spot, is a component the definition asks for
//! and a game may leave out.
//!
//! What a prop *is* lives in `props.ron`, loaded by
//! [`rl_rules::prop`] into [`Registries::props`], so a crate is a name in
//! a registry like a monster or an item. Where one stands is the game's:
//! it spawns by id at a prefab's mark, the way it already plants stairs
//! and consoles.
//!
//! The look travels as data rather than as a `Glyph`, because glyphs
//! belong to the renderer and the renderer sits above this crate. A prop
//! spawned here carries its [`PropKind`], and whoever draws dresses it,
//! exactly as a tile is described once as a tile and once as a look.
//!
//! A prop whose definition does nothing but stand there is [`Scenery`],
//! which a list of what is in sight passes over.
//!
//! An actor offers too, through its own [`Offers`]: a captive to free, a
//! person to talk to. The gate, the refusal and the [`Interacted`] answer
//! are the ones a prop's offer has, because what can be done here is one
//! question whoever stands here.
//!
//! An offer may ask before it is taken up, [`AsksFirst`]: a yes or no,
//! [`Offering::asking`], or one of the taker's things,
//! [`Offering::choosing`], which a screen asks and only an answer turns
//! into the [`Interact`], so the act it stands for is still the engine's
//! to check and to charge, and backing out costs nothing.
//!
//! Offers and the interactions that answer them, containers that stock and
//! empty, triggers that land an effect list, and hiding that a turn in
//! sight rolls away, for a prop and for a place's
//! [`Secret`](crate::secrets::Secret) alike, are all here.
//! `docs/design/props.md` is the whole design.

use bevy::prelude::*;
use rl_core::{Id, Interner, Point, geometry};
use rl_rules::TagId;
use rl_rules::prop::PropId;

use crate::combat::{CombatRules, Faction, Health};
use crate::components::{Blocks, MyTurn, Position};
use crate::effects::{Effects, Fired, LandsAsItself, Moments, Remnant, Triggers};
use crate::items::{Inventory, Stack, Tagged};
use crate::places::{MapId, OnMap};
use crate::registries::Registries;
use crate::trade::Currency;
use crate::turn::{Action, Intent, Resolution};
use crate::world::WorldMap;

/// A thing on the map that is not an actor and not an item.
///
/// The marker every other part of a prop hangs off. On its own it is
/// something named that stands in a cell and can be looked at.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Prop;

/// Which definition a prop was spawned from.
///
/// Kept because the save writes down what a thing is and spawns it again
/// from that, and because whoever draws reads the look off the same
/// definition rather than being handed one.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PropKind(pub PropId);

/// A prop that only says what a place was for: its definition offers,
/// holds, answers and hides nothing and it cannot be broken, as
/// [`PropDef::is_scenery`](rl_rules::prop::PropDef::is_scenery) reads it.
///
/// Put on by [`spawn_prop`], so a restored run marks it again. A panel's
/// list of what is in sight leaves scenery out, and the map and the look
/// cursor still show it. A game that hangs a behaviour of its own on a
/// prop like this removes the marker, and the prop is listed again.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Scenery;

/// Whether a prop of this definition is something to do with or about:
/// it offers a verb, holds things, or hides until it is spotted, so that
/// once found it is a thing a player steps round.
///
/// Narrower than "not [`Scenery`]". A prop that can only be broken, or
/// that answers a moment nobody can see coming, is more than scenery to
/// a mind or a cursor, which may aim at it; but beside what can be acted
/// on it is one more row for every barrel and every sac, which is what a
/// list of what is in sight leaves out by this. A game that hangs an
/// [`Offers`] of its own on a prop has given it something to do, which a
/// list reads off the prop rather than off its definition.
pub fn has_business(def: &rl_rules::prop::PropDef) -> bool {
    !def.offers.is_empty() || def.container.is_some() || def.hidden.is_some()
}

/// Puts a prop of kind `id` at `at` on `map`, with everything its
/// definition asks for.
///
/// The game says where; the definition says what. Returns the entity so
/// the caller can hang its own components on it, which is how a game's
/// console is still a game's console.
pub fn spawn_prop(commands: &mut Commands, registries: &Registries, id: PropId, at: Point, map: MapId) -> Entity {
    let def = registries.props.get(id);
    let mut prop = commands.spawn((Prop, PropKind(id), Name::new(def.name.clone()), Position(at), OnMap(map)));
    if def.blocks {
        prop.insert(Blocks);
    }
    if let Some(health) = def.health {
        prop.insert(Health::full(health));
    }
    if def.container.is_some() {
        prop.insert((Container, Inventory::default()));
    }
    if let Some(secret) = def.hidden {
        prop.insert(Hidden { spot: secret.spot });
    }
    if def.is_scenery() {
        prop.insert(Scenery);
    }
    prop.id()
}

/// What an interaction is called, as an interned name. Never constructed;
/// it only types [`VerbId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Verb {}

/// What an interaction is called, for a game to filter its own out of
/// [`Interacted`]. Interned by [`Verbs`], so there is no closed list of
/// what a prop can offer.
pub type VerbId = Id<Verb>;

/// Every verb in play, in the order they were first named.
///
/// The engine's own are interned first, so their ids are the constants on
/// this type, and a game adds its own with [`AddVerb::add_verb`]. Verbs
/// are interned rather than free strings so that two games spell `open`
/// the same way, and so the engine can say there is nothing here to open
/// in its own words.
#[derive(Resource, Debug, Clone)]
pub struct Verbs(Interner<Verb>);

impl Verbs {
    /// Open what holds something.
    pub const OPEN: VerbId = VerbId::from_raw(0);

    /// The engine's verbs, in the order their ids are handed out.
    pub const BUILT_IN: [&'static str; 2] = [rl_rules::prop::OPEN, rl_rules::prop::SEARCH];

    /// The id for `name`, assigning a new one if it is unseen.
    pub(crate) fn declare(&mut self, name: &str) -> VerbId {
        self.0.intern(name)
    }

    /// The id for `name`, if it has been declared.
    pub fn get(&self, name: &str) -> Option<VerbId> {
        self.0.get(name)
    }

    /// The name behind `id`.
    pub fn name(&self, id: VerbId) -> &str {
        self.0.name(id)
    }
}

impl Default for Verbs {
    fn default() -> Self {
        let mut names = Interner::new();
        for name in Self::BUILT_IN {
            names.intern(name);
        }
        Self(names)
    }
}

/// Declares a verb while the app is being built.
///
/// A prop's definition names its verbs in words; this is what turns one
/// into the id a game filters [`Interacted`] by.
pub trait AddVerb {
    /// Declares the verb `name`. Look its id up again with [`Verbs::get`].
    fn add_verb(&mut self, name: &str) -> &mut Self;
}

impl AddVerb for App {
    fn add_verb(&mut self, name: &str) -> &mut Self {
        self.init_resource::<Verbs>();
        self.world_mut().resource_mut::<Verbs>().declare(name);
        self
    }
}

/// One thing the actor holding the turn could do with what it can reach,
/// as the gate works it out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// What offers it: a prop, or an actor carrying [`Offers`].
    pub target: Entity,
    /// What it is called.
    pub verb: VerbId,
    /// What the turn would cost.
    pub time: u32,
    /// What taking it up costs in the trade slice's
    /// [`Currency`], when it has a price.
    pub price: Option<u32>,
    /// What is asked before it is taken up, when anything is; see
    /// [`Offering::asking`] and [`Offering::choosing`].
    pub asks: Option<AsksFirst>,
    /// Why it cannot be taken up, when it cannot. An offer is listed
    /// either way, so a screen can grey a row and say why, as the ability
    /// menu does.
    pub refused: Option<Refused>,
}

/// Why an offer cannot be taken up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// It wants something the actor is not carrying.
    Needs(TagId),
    /// It has a price, and the actor carries less currency than this, the
    /// price it wants.
    Short(u32),
}

/// What an actor offers whoever stands beside it: a captive to be freed,
/// a person to be talked to.
///
/// A prop's offers are its definition's, shared by every prop of its
/// kind; an actor is spawned one at a time by the game, so what it offers
/// is its own and the game puts it on, and takes it off when the offer is
/// spent. Answered as a prop's offer with no effects is: the same gate,
/// the same reach, the same refusal, and an [`Interacted`] the game reads
/// for its verb; what taking it up does is the game's, effects included.
/// Read on anything that carries it, a prop too, but never offered to its
/// own holder. Not saved by the engine: the game puts it back on with
/// whatever it saves the actor as, since the game is what put it there.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct Offers(pub Vec<Offering>);

/// One thing an [`Offers`] names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offering {
    /// What it is called.
    pub verb: VerbId,
    /// What the turn costs, in hundredths of a step.
    pub time: u32,
    /// A tag the one taking it up must carry, or it is refused and says so.
    pub needs: Option<TagId>,
    /// What taking it up costs, in the trade slice's
    /// [`Currency`], or nothing.
    pub price: Option<u32>,
    /// What is asked before it is taken up, a yes or no or one of the
    /// taker's things, or nothing.
    pub asks: Option<AsksFirst>,
}

/// What an offer asks before it is taken up: whoever shows the offers asks
/// it, and only an answer is the [`Interact`], resolved, checked for reach
/// and charged its time as any offer is. Backing out is nothing and costs
/// nothing.
///
/// One or the other, never both, so one field rather than two that could
/// both be set: an offer that wants a thing has already asked whether, by
/// being taken up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsksFirst {
    /// A question in the game's words, answered yes or no; see
    /// [`Offering::asking`].
    YesOrNo(String),
    /// Which of the taker's things, under `prompt` in the game's words,
    /// among what `among` answers; see [`Offering::choosing`].
    Thing {
        /// What the choice is asked under.
        prompt: String,
        /// The game's system naming what may be chosen.
        among: Among,
    },
}

impl AsksFirst {
    /// The yes-or-no question, when it is one.
    pub fn question(&self) -> Option<&str> {
        match self {
            Self::YesOrNo(question) => Some(question),
            Self::Thing { .. } => None,
        }
    }
}

/// A game's system naming what a taker may choose for an offer that
/// chooses first: registered with `App::register_system`, given the
/// [`Chooser`], returning the things to choose among in the order listed.
///
/// A system rather than a list on the offer, since what may be handed
/// over is the taker's pack as it stands when the offer is taken up,
/// which no offer put on earlier can know; and rather than a tag, since
/// what a game will take is a rule over the whole thing, what it is worn
/// on, what its use does, which no one tag says.
pub type Among = bevy::ecs::system::SystemId<In<Chooser>, Vec<Entity>>;

/// Who is choosing a thing for which offer: what an offer's [`Among`] is
/// asked of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chooser {
    /// Who took the offer up, whose things are chosen among.
    pub taker: Entity,
    /// Who or what offers it.
    pub offerer: Entity,
    /// Which of its offers.
    pub verb: VerbId,
}

impl Offering {
    /// `verb`, taking `time`, open to anyone.
    pub fn new(verb: VerbId, time: u32) -> Self {
        Self { verb, time, needs: None, price: None, asks: None }
    }

    /// Asks `question` before it is taken up: whoever shows the offers
    /// asks it, yes or no, and only a yes is the [`Interact`], resolved,
    /// checked for reach and charged its time as any offer is; no is
    /// nothing and costs nothing. Never taken up by walking into whoever
    /// offers it, even when it is the only offer, as a priced one never
    /// is: a walk key cannot answer a question.
    ///
    /// Asked before the act rather than by the game after it, so the act
    /// a yes stands for is the engine's to check and to charge: an asker
    /// gone out of reach by the time of the yes is refused like any other
    /// offer out of reach, and the time it costs is its own.
    pub fn asking(mut self, question: impl Into<String>) -> Self {
        self.asks = Some(AsksFirst::YesOrNo(question.into()));
        self
    }

    /// Chooses one of the taker's things before it is taken up: whoever
    /// shows the offers opens the bag under `prompt`, choosing among what
    /// the game's `among` names, and only a pick is the [`Interact`],
    /// carrying the thing picked, resolved, checked for reach and for the
    /// thing still being carried, and charged its time as any offer is;
    /// backing out is nothing and costs nothing. Never taken up by walking
    /// into whoever offers it, as an asking one never is.
    ///
    /// Chosen before the act rather than by the game after it, for the
    /// reason [`asking`](Self::asking) is asked first: an offer resolved
    /// before the choice has spent its time on a choice the player may
    /// back out of. Asking and choosing are one or the other, the last
    /// said.
    pub fn choosing(mut self, prompt: impl Into<String>, among: Among) -> Self {
        self.asks = Some(AsksFirst::Thing { prompt: prompt.into(), among });
        self
    }

    /// Costs `price` in the trade slice's [`Currency`]:
    /// refused to anyone carrying less, and paid into the offerer's own
    /// pack, given one if it carries none, when taken up, before the game
    /// hears of it in [`Interacted`]. Never taken up by walking into whoever
    /// offers it, even when it is the only offer: a bump asks, so currency
    /// is only ever spent on a choice made on the offers screen or by the
    /// interact key.
    ///
    /// A service is a verb with a price rather than a thing pretending to
    /// be carried for an instant, and it is paid the way a deal is, so the
    /// currency stays conserved. A game that prices an offer adds
    /// [`TradePlugin`](crate::trade::TradePlugin) for its currency; one
    /// without a `Currency` is a mistake in the setup and stops play.
    pub fn priced(mut self, price: u32) -> Self {
        self.price = Some(price);
        self
    }

    /// Refused to anyone not carrying something tagged `tag`.
    pub fn needing(mut self, tag: TagId) -> Self {
        self.needs = Some(tag);
        self
    }
}

/// What the actor holding the turn could do with what it can reach.
///
/// Worked out every pass, for whoever holds the turn, the player as
/// readily as a monster: what can be done here is the same question for
/// both. Read through [`for_actor`](Self::for_actor), which answers only
/// for the actor it was worked out for, since a reader holding last
/// pass's actor would otherwise act on someone else's offers.
#[derive(Resource, Debug, Default)]
pub struct OfferedHere {
    actor: Option<Entity>,
    offers: Vec<Offer>,
}

impl OfferedHere {
    /// Who holds the turn it was worked out for, if anyone does.
    pub fn actor(&self) -> Option<Entity> {
        self.actor
    }

    /// What `actor` is offered, empty when it was worked out for someone
    /// else.
    pub fn for_actor(&self, actor: Entity) -> &[Offer] {
        if self.actor == Some(actor) { &self.offers } else { &[] }
    }

    /// What `actor` could take up now: the offers nothing refuses.
    pub fn open_to(&self, actor: Entity) -> impl Iterator<Item = &Offer> {
        self.for_actor(actor).iter().filter(|o| o.refused.is_none())
    }

    /// Whether `actor` is offered `verb` by `target`, and at what cost.
    pub fn find(&self, actor: Entity, target: Entity, verb: VerbId) -> Option<&Offer> {
        self.for_actor(actor).iter().find(|o| o.target == target && o.verb == verb)
    }
}

/// Whoever holds the turn, what it carries, and whose side it is on.
type Reacher<'w, 's> = Query<'w, 's, (Entity, &'static Position, Option<&'static OnMap>, Option<&'static Inventory>, Option<&'static Faction>), With<MyTurn>>;

/// Everything that could offer something, as the gate reads it: a prop by
/// its definition, and anything carrying [`Offers`] by its own.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Offerers<'w, 's> {
    props: Query<'w, 's, (Entity, &'static PropKind, &'static Position, Option<&'static OnMap>), With<Prop>>,
    others: Query<'w, 's, (Entity, &'static Offers, &'static Position, Option<&'static OnMap>)>,
    emptied: Query<'w, 's, (), With<Emptied>>,
    /// How sides stand, so a foe's offers are kept from whoever it is
    /// hostile to. Without rules nobody is anybody's foe.
    rules: Option<Res<'w, CombatRules>>,
    sides: Query<'w, 's, &'static Faction>,
}

/// Works out what the actor holding the turn is offered by what it can
/// reach: what it stands on, and what stands beside it, a prop by its
/// definition and anything carrying [`Offers`] by its own.
///
/// In [`DecideSet::Offer`](crate::plugin::DecideSet::Offer), beside the
/// ability gate and for the same reason: the player's screen and a mind's
/// tactic both read one answer, worked out once, rather than each walking
/// the world themselves. Asked of where everything stands at that moment,
/// so an offer walks off with whoever makes it, and a price is weighed
/// against what the actor carries now.
pub fn offer_here(
    mut offered: ResMut<OfferedHere>,
    registries: Res<Registries>,
    verbs: Res<Verbs>,
    reacher: Reacher,
    offerers: Offerers,
    pieces: Query<(Option<&Tagged>, Option<&Stack>)>,
    currency: Option<Res<Currency>>,
) {
    let Offerers { props, others, emptied, rules, sides } = &offerers;
    offered.actor = None;
    offered.offers.clear();
    let Ok((actor, pos, on, bag, side)) = reacher.single() else { return };
    offered.actor = Some(actor);
    let here = on.map(|m| m.0).unwrap_or(MapId::ZERO);
    // What it stands on and what stands beside it: a body underfoot is as
    // reachable as a crate in the next cell.
    let in_reach =
        |at: &Position, there: Option<&OnMap>| there.map(|m| m.0).unwrap_or(MapId::ZERO) == here && (at.0 == pos.0 || geometry::is_adjacent(pos.0, at.0));
    // Counted by the trade slice's own rule, so an offer and a deal never
    // disagree over what the actor can pay. With no currency there is
    // nothing to pay with: `check_priced_offers` has already stopped play
    // if anything here has a price.
    let purse = bag.zip(currency.as_deref()).map_or(0, |(bag, currency)| currency.count(bag, |i| pieces.get(i).unwrap_or((None, None))));
    // What it lacks first, then what it cannot pay: a key it does not
    // carry is the refusal no purse answers.
    let refused = |wants: Option<TagId>, price: Option<u32>| {
        wants.filter(|tag| !carries(bag, &pieces, *tag)).map(Refused::Needs).or_else(|| price.filter(|p| purse < *p).map(Refused::Short))
    };
    for (prop, kind, at, prop_on) in props {
        if !in_reach(at, prop_on) {
            continue;
        }
        let def = registries.props.get(kind.0);
        let done = emptied.contains(prop);
        // A lock is written on what it shuts rather than on the verb, so
        // it is folded in here: opening a locked thing wants its key as
        // surely as an offer that asked for one itself.
        let lock = def.container.as_ref().and_then(|c| c.locked);
        for offer in &def.offers {
            let Some(verb) = verbs.get(&offer.verb) else { continue };
            if done && verb == Verbs::OPEN {
                continue;
            }
            let refused = refused(offer.needs.or(lock.filter(|_| verb == Verbs::OPEN)), None);
            offered.offers.push(Offer { target: prop, verb, time: offer.time, price: None, asks: None, refused });
        }
    }
    // Never the turn holder's own: what a person offers is offered to
    // whoever stands by them, not to themselves. Nor to a foe: a captive's
    // captor is not offered to free it, and a hostile trader does not
    // trade. Decided here, once, so the interact key, the offers screen
    // and the bump all agree.
    let hostile = |other: Entity| match (rules.as_deref(), side, sides.get(other).ok()) {
        (Some(rules), Some(mine), Some(theirs)) => rules.factions.relation(theirs.0, mine.0) == rl_rules::Relation::Hostile,
        _ => false,
    };
    for (other, offers, at, other_on) in others {
        if other == actor || !in_reach(at, other_on) || hostile(other) {
            continue;
        }
        for offer in &offers.0 {
            let refused = refused(offer.needs, offer.price);
            offered.offers.push(Offer { target: other, verb: offer.verb, time: offer.time, price: offer.price, asks: offer.asks.clone(), refused });
        }
    }
}

/// Whether the actor carries anything tagged `tag`.
fn carries(bag: Option<&Inventory>, pieces: &Query<(Option<&Tagged>, Option<&Stack>)>, tag: TagId) -> bool {
    bag.is_some_and(|bag| bag.items.iter().any(|item| pieces.get(*item).is_ok_and(|(t, _)| t.is_some_and(|t| t.0.contains(&tag)))))
}

/// Stops play when something offers at a price and the game has no
/// [`Currency`] to pay it in.
///
/// A price is in the trade slice's currency, so a priced offer in a game
/// without one is a mistake in the setup, and it is said loudly, in the
/// words a missing need is said in, rather than refused for ever to a
/// player who could never learn why. Offers are put on during play, so it
/// is checked as each is put on, just before the gate reads it, rather
/// than once when play begins.
fn check_priced_offers(currency: Option<Res<Currency>>, offers: Query<&Offers, Changed<Offers>>, verbs: Res<Verbs>) {
    if currency.is_some() {
        return;
    }
    let priced: Vec<&str> = offers.iter().flat_map(|o| &o.0).filter(|o| o.price.is_some()).map(|o| verbs.name(o.verb)).collect();
    assert!(
        priced.is_empty(),
        "play cannot go on: one piece is missing.\n  PropsPlugin needs {}: an offer of {priced:?} has a price, paid in the trade slice's currency; add TradePlugin and insert `Currency(registries.tags.expect(\"...\"))`, the tag every piece of the currency carries",
        std::any::type_name::<Currency>()
    );
}

/// Do what a prop or an actor offers: the one action, whatever the verb
/// and whoever offers it.
///
/// Refused for free when the offer is not there to take: the gate worked
/// out what was possible before the key was pressed, so nothing is
/// learned by spending a turn finding out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interact {
    /// What to interact with: a prop, or an actor that offers.
    pub target: Entity,
    /// Which of the things it offers.
    pub verb: VerbId,
    /// The thing of the actor's own chosen for an offer that chooses
    /// first, [`Offering::choosing`]; `None` for every other offer. A
    /// thing no longer carried, a thing for an offer that chooses none, or
    /// none for one that does, is refused, for nothing.
    pub thing: Option<Entity>,
}
impl Action for Interact {}

/// Somebody interacted with a prop, or with an actor that offers.
///
/// The whole of how a game answers a prop or a person of its own: a system in
/// [`TurnSet::React`](crate::plugin::TurnSet::React) reads these, keeps
/// the ones whose verb is its own, and does whatever that verb means.
/// The engine needs no registry of behaviours because a game already
/// knows how to answer a message.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interacted {
    /// Who did it.
    pub actor: Entity,
    /// What they did it to: a prop, or an actor that offers.
    pub target: Entity,
    /// What they did.
    pub verb: VerbId,
    /// The thing of theirs they chose, for an offer that chooses first;
    /// `None` for every other.
    pub thing: Option<Entity>,
}

/// What answering an interaction reads: what was offered, what each offer
/// lands, and which of what offers is a prop.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Answering<'w, 's> {
    offered: Res<'w, OfferedHere>,
    effects: Option<Res<'w, PropEffects>>,
    registries: Res<'w, Registries>,
    verbs: Res<'w, Verbs>,
    props: Query<'w, 's, &'static PropKind, With<Prop>>,
    /// What a priced offer is paid in.
    currency: Option<Res<'w, Currency>>,
    /// What each actor carries, for the thing a choosing offer was taken
    /// up with.
    bags: Query<'w, 's, &'static Inventory>,
}

/// Resolves an interaction against what the gate offered.
pub fn resolve_interactions(
    mut intents: MessageReader<Intent<Interact>>,
    mut done: MessageWriter<Interacted>,
    mut resolution: Resolution,
    what: Answering,
    mut world: crate::effects::EffectWorld,
) {
    let Answering { offered, effects, registries, verbs, props, currency, bags } = &what;
    for intent in intents.read() {
        let Some(offer) = offered.find(intent.actor, intent.action.target, intent.action.verb) else {
            continue;
        };
        // A choosing offer is taken up with one of the actor's things, still
        // carried, since the bag chose it a moment ago and anything may
        // have moved it since; any other offer with none.
        let chooses = matches!(offer.asks, Some(AsksFirst::Thing { .. }));
        let fits = match intent.action.thing {
            Some(thing) => chooses && bags.get(intent.actor).is_ok_and(|bag| bag.contains(thing)),
            None => !chooses,
        };
        if offer.refused.is_some() || !fits || !resolution.claim(intent.actor) {
            continue;
        }
        resolution.done(intent.actor, offer.time);
        // The price, from the taker's pack into the offerer's own, in a
        // command applied at the end of `Resolve`, so a game answering the
        // offer in `React` finds it already paid, and paid as a deal pays,
        // so the currency stays a pile somebody carries. No currency and a
        // price never meet here: `check_priced_offers` stops play first.
        if let (Some(price), Some(&currency)) = (offer.price, currency.as_deref()) {
            let (taker, offerer) = (intent.actor, intent.action.target);
            world.commands.queue(move |w: &mut World| {
                crate::trade::pay(w, taker, offerer, price, currency);
            });
        }
        // What the offer carried, when it carried anything: an offer with
        // no effects, which an actor's always is, is answered by the game,
        // from the message below.
        if let (Some(effects), Ok(kind)) = (effects.as_deref(), props.get(intent.action.target)) {
            let which = registries.props.get(kind.0).offers.iter().position(|o| verbs.get(&o.verb) == Some(intent.action.verb));
            let at = world.position(intent.action.target).unwrap_or_default();
            if let Some(which) = which
                && let Some(effects) = effects.offer(kind.0, which)
            {
                effects.land_on(crate::effects::Source::Offer(intent.action.target), intent.action.target, at, vec![intent.actor], &mut world);
            }
        }
        done.write(Interacted { actor: intent.actor, target: intent.action.target, verb: intent.action.verb, thing: intent.action.thing });
    }
}

/// A prop that holds things.
///
/// Its [`Inventory`] is what it holds, so everything that already knows
/// how to read a bag reads this one too. The engine rolls how many of each
/// row and asks with [`FillContainer`], because items are a game's own
/// registry and only a game can make one; with
/// [`LootPlugin`](crate::loot::LootPlugin) the engine answers it too,
/// through the game's [`ItemMaker`](crate::loot::ItemMaker).
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Container;

/// A container that has been emptied.
///
/// Only put on when the definition gives an opened look, because it is
/// what stops the `open` offer: a crate that cannot show it is done keeps
/// offering, and the screen says it is empty, rather than quietly
/// refusing a crate that looks exactly like a full one.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Emptied;

/// Props' own stream: what a container holds, and what is spotted.
///
/// Its own, like stealth's and combat's, so stocking a crate never nudges
/// a later blow.
#[derive(Resource)]
pub struct PropRng(pub rand::rngs::StdRng);

impl crate::seed::Stream for PropRng {
    fn for_run(seed: rl_core::RunSeed) -> Self {
        Self(seed.rng(rl_core::SeedDomain::new(b"props"), 0))
    }
}

/// Asks for one row of what `prop` holds: `count` of a fixed item, or
/// `count` draws of anything carrying a tag, `band` bands deeper than
/// where the container stands.
///
/// The engine rolled the count from the definition and its own stream.
/// [`LootPlugin`](crate::loot::LootPlugin) answers it in
/// [`PropSet::Fill`]; a game without one answers a fixed item itself,
/// there too, spawning what it names and pushing the entities onto the
/// prop's [`Inventory`], and cannot be asked for a tag, which needs a
/// loot table to draw from.
#[derive(Message, Debug, Clone)]
pub struct FillContainer {
    /// Which container.
    pub prop: Entity,
    /// What goes in: an item by the name its own registry knows it by, or
    /// a tag.
    pub what: rl_rules::prop::Stock,
    /// How many of the item, or how many draws of the tag, already rolled.
    pub count: u32,
    /// For a tag, how many bands deeper than the container's own it is
    /// drawn at.
    pub band: i32,
}

/// A container whose contents have been asked for, so they are asked for
/// once however many frames it takes for the run's streams to exist.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Stocked;

/// A container waiting to be filled, and which definition says with what.
type Unstocked = (Entity, &'static PropKind);

/// A stocked container whose bag has just changed, or that has just been
/// stocked, and what it was made from.
///
/// Only a [`Stocked`] one, because a container put down mid-turn, as a
/// prefab's slot is, holds nothing until the next frame's
/// [`PropSet::Stock`] asks for its contents, and a bag that was never
/// filled is not one just emptied. Any pass in between may run the check,
/// and the order of unordered systems is not something to lean on.
/// `Added<Stocked>` as well, so a container that rolled nothing is shown
/// done as it is stocked whenever it was put down, rather than only when
/// the check happened to run before it was stocked.
type JustEmptied<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static PropKind, &'static Inventory),
    (With<Container>, With<Stocked>, Without<Emptied>, Or<(Changed<Inventory>, Added<Stocked>)>),
>;

/// Refuses play when a container asks for a kind of thing by tag and no
/// [`LootPlugin`](crate::loot::LootPlugin) is there to draw it, rather
/// than leaving every such crate empty.
fn check_tagged_contents(registries: Option<Res<Registries>>, answered: Option<Res<crate::loot::ContainersAnswered>>) {
    let Some(registries) = registries else { return };
    if answered.is_some() {
        return;
    }
    let asking: Vec<&str> = registries
        .props
        .iter()
        .filter(|(_, def)| def.container.as_ref().is_some_and(|c| c.contents.iter().any(|r| matches!(r.what, rl_rules::prop::Stock::Tag(_)))))
        .map(|(_, def)| def.name.as_str())
        .collect();
    assert!(
        asking.is_empty(),
        "PropsPlugin: {asking:?} ask for things by tag, which is drawn from a loot table, and no LootPlugin was added to draw them; add LootPlugin"
    );
}

/// Rolls what each new container holds and asks the game for it.
///
/// Keyed on [`Stocked`] rather than on `Added`, because a stream is
/// derived from the run's seed and may not exist on the frame a place is
/// built: a container waits a frame rather than being quietly left empty
/// forever.
pub fn stock_containers(
    mut commands: Commands,
    mut asks: MessageWriter<FillContainer>,
    rng: Option<ResMut<PropRng>>,
    registries: Res<Registries>,
    fresh: Query<Unstocked, (With<Container>, Without<Stocked>)>,
) {
    use rand::Rng;
    let Some(mut rng) = rng else { return };
    for (prop, kind) in &fresh {
        commands.entity(prop).insert(Stocked);
        let Some(container) = registries.props.get(kind.0).container.as_ref() else { continue };
        for roll in &container.contents {
            let count = if roll.min >= roll.max { roll.max } else { rng.0.random_range(roll.min..=roll.max) };
            if count > 0 {
                asks.write(FillContainer { prop, what: roll.what.clone(), count, band: roll.band });
            }
        }
    }
}

/// Take something out of what is open, or take all of it.
///
/// One turn either way: rummaging is rummaging, and a crate emptied a
/// piece at a time would otherwise cost more than one taken whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Take {
    /// What to take from.
    pub from: Entity,
    /// Which one, or everything when it is `None`.
    pub item: Option<Entity>,
}
impl Action for Take {}

/// Moves what was taken into the taker's bag, merging stacks the way the
/// ground does.
///
/// Refused for free when the container is out of reach or holds nothing,
/// which is what the screen already knows, so nothing is spent finding
/// out.
pub fn resolve_takes(
    mut commands: Commands,
    mut intents: MessageReader<Intent<Take>>,
    mut events: MessageWriter<crate::items::ItemEvent>,
    mut resolution: Resolution,
    mut takers: Query<(&Position, &mut Inventory), Without<Container>>,
    mut held: Query<(&Position, &mut Inventory, Option<&mut crate::items::Equipped>), With<Container>>,
    stacks: Query<&crate::items::Stack>,
) {
    for intent in intents.read() {
        let actor = intent.actor;
        let Ok((at, _)) = takers.get(actor) else { continue };
        let Ok((chest, _, _)) = held.get(intent.action.from) else { continue };
        if chest.0 != at.0 && !geometry::is_adjacent(at.0, chest.0) {
            continue;
        }
        let taking: Vec<Entity> = {
            let Ok((_, contents, _)) = held.get(intent.action.from) else { continue };
            match intent.action.item {
                Some(one) => contents.items.iter().copied().filter(|i| *i == one).collect(),
                None => contents.items.clone(),
            }
        };
        if taking.is_empty() || !resolution.claim(actor) {
            continue;
        }
        if let Ok((_, mut contents, worn)) = held.get_mut(intent.action.from) {
            contents.items.retain(|i| !taking.contains(i));
            // What a thing wears is what it still carries: a worn thing taken
            // out of a body comes off it, or the body goes on listing armor
            // that is now in someone's bag, and a body stood back up wears it.
            if let Some(mut worn) = worn {
                for item in &taking {
                    worn.unequip(*item);
                }
            }
        }
        if let Ok((_, mut bag)) = takers.get_mut(actor) {
            for item in taking {
                let merged_into = stacks.get(item).ok().and_then(|s| bag.items.iter().copied().find(|c| stacks.get(*c).is_ok_and(|t| t.key == s.key)));
                match merged_into {
                    Some(into) => {
                        let add = stacks.get(item).map(|s| s.count).unwrap_or(1);
                        commands.entity(into).entry::<crate::items::Stack>().and_modify(move |mut s| s.count += add);
                        commands.entity(item).despawn();
                    }
                    None => bag.items.push(item),
                }
                events.write(crate::items::ItemEvent::PickedUp { actor, item, merged_into });
            }
        }
        resolution.done(actor, rl_core::turn::BASE_ACTION_COST);
    }
}

/// Marks a container that has been emptied, when its definition says what
/// an emptied one looks like.
///
/// Read only once it is [`Stocked`], which is safe because what it holds
/// lands in [`PropSet::Fill`], the same frame and before any turn: a
/// game answering [`FillContainer`] itself answers it there.
///
/// What it looks like afterwards is the renderer's to apply, from the
/// same definition, the way a full one's look is.
pub fn close_emptied_containers(mut commands: Commands, registries: Res<Registries>, emptied: JustEmptied) {
    for (prop, kind, contents) in &emptied {
        if !contents.items.is_empty() {
            continue;
        }
        if registries.props.get(kind.0).container.as_ref().is_some_and(|c| c.opened.is_some()) {
            commands.entity(prop).insert(Emptied);
        }
    }
}

/// Every prop's effects, built once from the registry: what each offer
/// lands, and each kind's [`Triggers`], which [`arm_props`] copies onto
/// every prop of that kind.
///
/// Built on the first frame rather than in the plugin, because a game
/// registers its own effects and moments while the app is being built and
/// the last of them must be in before the first prop is read.
#[derive(Resource, Default)]
pub struct PropEffects {
    /// Per prop id, its triggers, shared by every prop of the kind.
    triggers: Vec<Triggers>,
    /// Per prop id, per offer, what taking it up lands.
    offers: Vec<Vec<Effects>>,
}

impl PropEffects {
    /// What the `which`th offer of `prop` lands, if it was built at all.
    fn offer(&self, prop: PropId, which: usize) -> Option<&Effects> {
        self.offers.get(prop.index()).and_then(|o| o.get(which))
    }
}

/// Builds every prop's effects and triggers, once, and says loudly what
/// would not build.
///
/// A definition naming an effect or a moment nobody registered is a content
/// mistake that would otherwise be a trap that silently does nothing, which
/// is the worst kind of trap. A trap whose triggers would not build is left
/// unarmed rather than half armed.
pub fn build_prop_effects(
    mut commands: Commands,
    built: Option<Res<PropEffects>>,
    registries: Res<Registries>,
    kinds: Option<Res<crate::effects::EffectKinds>>,
    moments: Option<Res<crate::effects::Moments>>,
    sounds: Option<Res<crate::noise::Sounds>>,
) {
    if built.is_some() {
        return;
    }
    use crate::noise::SoundNames;
    // With the sounds in play when the game has any, so a plate that
    // sounds an alarm names its sound as it names a status.
    let names = match sounds.as_deref() {
        Some(sounds) => registries.names().sounds(sounds),
        None => registries.names(),
    };
    let (no_kinds, no_moments) = (crate::effects::EffectKinds::default(), crate::effects::Moments::default());
    let kinds = kinds.as_deref().unwrap_or(&no_kinds);
    let moments = moments.as_deref().unwrap_or(&no_moments);
    let mut effects = PropEffects::default();
    let mut errors = Vec::new();
    for (_, def) in registries.props.iter() {
        let triggers = match Triggers::build(&def.triggers, &[], moments, kinds, &names) {
            Ok(t) => t,
            Err(mine) => {
                errors.extend(mine.into_iter().map(|e| format!("{} triggers: {e}", def.name)));
                Triggers::default()
            }
        };
        effects.triggers.push(triggers);
        let mut offers = Vec::new();
        for o in &def.offers {
            offers.push(match Effects::build(&o.effects, kinds, &names) {
                Ok(e) => e,
                Err(mine) => {
                    errors.extend(mine.into_iter().map(|e| format!("{} offer {:?}: {e}", def.name, o.verb)));
                    Effects::default()
                }
            });
        }
        effects.offers.push(offers);
    }
    for e in &errors {
        error!("props.ron: {e}");
    }
    commands.insert_resource(effects);
}

/// A restored prop's own firings, waiting for its triggers.
///
/// A save brings a prop back before anything has built its kind's
/// triggers, so what it had left to fire is kept here and put onto them by
/// [`arm_props`] the moment it is armed.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct PendingFires(pub Vec<Option<u32>>);

/// Props not yet given their triggers.
type Unarmed<'w, 's> = Query<'w, 's, (Entity, &'static PropKind, Option<&'static PendingFires>), (With<Prop>, Without<Triggers>)>;

/// Props with triggers, where they stand.
type Armed<'w, 's> = Query<'w, 's, (Entity, &'static Position, Option<&'static OnMap>), (With<Prop>, With<Triggers>)>;

/// Puts each new prop's triggers on it, from its kind's.
///
/// Every copy shares its kind's effect lists and keeps its own count of
/// times left to fire, so springing one cable spends nothing of another.
/// A kind with none is armed with an empty list, which answers nothing and
/// keeps this from looking at the prop again.
pub fn arm_props(mut commands: Commands, built: Option<Res<PropEffects>>, fresh: Unarmed) {
    let Some(built) = built else { return };
    for (prop, kind, pending) in &fresh {
        let mut triggers = built.triggers.get(kind.0.index()).cloned().unwrap_or_default();
        if let Some(PendingFires(fires)) = pending {
            for (trigger, left) in triggers.0.iter_mut().zip(fires) {
                trigger.fires = *left;
            }
        }
        // A prop's triggers are its own doing, whoever set them off.
        commands.entity(prop).insert((triggers, LandsAsItself)).remove::<PendingFires>();
    }
}

/// Reports the `entered` moment on every armed prop a step or a swap lands
/// someone on.
///
/// Reads [`Stepped`](crate::turn::Stepped), which the move resolver writes
/// for every step it lets through, so anything that walks sets off a
/// plate: the player, a droid, a rat. Whether it could see the plate never
/// comes into it, which is the point of a hidden one. And reads
/// [`Swapped`](crate::bump::Swapped) for both who changed places, each at
/// the cell it now stands on, since a swap is no step but both still come
/// to stand on a plate: one that gave way to a passer-by is put onto
/// whatever the passer-by stood on, and the passer-by onto what it did.
pub fn report_entered(
    mut steps: MessageReader<crate::turn::Stepped>,
    mut swaps: MessageReader<crate::bump::Swapped>,
    standing: Query<&Position>,
    props: Armed,
    map: Res<WorldMap>,
    mut fired: MessageWriter<Fired>,
) {
    let here = map.current();
    let swapped = swaps.read().flat_map(|s| [s.actor, s.with]).filter_map(|who| standing.get(who).ok().map(|at| (who, at.0)));
    let entered: Vec<(Entity, Point)> = steps.read().map(|step| (step.actor, step.to)).chain(swapped).collect();
    for (who, to) in entered {
        for (prop, at, on) in &props {
            if at.0 == to && on.map(|m| m.0).unwrap_or(MapId::ZERO) == here {
                fired.write(Fired { on: prop, moment: Moments::ENTERED, by: Some(who), at: to });
            }
        }
    }
}

/// Reports the `destroyed` moment for every armed prop that was broken.
///
/// A death is only known once damage has been applied, so this is read in
/// [`TurnSet::React`](crate::plugin::TurnSet::React) and lands the pass
/// after. The prop itself is despawned at the end of the frame, which may
/// be before then, so what it does goes on a [`Remnant`] that carries its
/// triggers, its name and whatever else [`RemnantKeeps`](crate::effects::RemnantKeeps)
/// names, is reported in its place, and is despawned at
/// the end of the frame they land in. A prop with nothing to do when
/// broken leaves none.
pub fn report_destroyed(
    mut commands: Commands,
    mut deaths: MessageReader<crate::combat::DeathEvent>,
    props: Query<&Triggers, With<Prop>>,
    keeps: Res<crate::effects::RemnantKeeps>,
) {
    for death in deaths.read() {
        let Ok(triggers) = props.get(death.entity) else { continue };
        if triggers.on(Moments::DESTROYED).next().is_none() {
            continue;
        }
        let remnant = Remnant { moment: Moments::DESTROYED, by: death.credit, at: death.at };
        let carrier = commands.spawn((triggers.clone(), remnant, LandsAsItself)).id();
        keeps.copy(&mut commands, death.entity, carrier);
    }
}

/// A prop as a mind reads it: where it stands, on which map, and whose it
/// is. The side is what a body carries, from whoever died.
type Standing = (Entity, &'static Position, Option<&'static OnMap>, Option<&'static crate::combat::Faction>);

/// Tells the mind holding the turn what stands where it can see.
///
/// Props' contribution to a mind's knowledge, in
/// [`PerceiveSet::Annotate`](crate::plugin::PerceiveSet::Annotate). No
/// wits are asked: seeing a crate or a body takes none, and what a mind
/// may do about one is its game's business rather than a capability the
/// engine has a word for. A prop nobody has spotted is left out, so a
/// mind never walks to a plate the player cannot see either.
pub fn perceive_props(mut thinking: ResMut<crate::minds::Thinking>, sight: crate::minds::Sight, standing: Query<Standing, (With<Prop>, Without<Hidden>)>) {
    if thinking.actor().is_none() {
        return;
    }
    let mut seen = Vec::new();
    for (prop, at, on, side) in &standing {
        if !sight.perceives(&thinking, at.0, on) {
            continue;
        }
        seen.push(rl_rules::ai::PropView { id: prop, pos: at.0, side: side.map(|f| f.0) });
    }
    if let Some(snapshot) = thinking.snapshot_mut() {
        snapshot.props.extend(seen);
    }
}

/// A prop nobody has spotted yet.
///
/// Put on from the definition when the prop is spawned and taken off when
/// it is spotted, so "hidden" is one fact in one place: whatever draws and
/// whatever lists skip a prop that has one, and everything else treats it
/// as the ordinary prop it is. A hidden trap still springs; not seeing it
/// is the point.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hidden {
    /// Percent chance a turn to spot it while it is in sight, before the
    /// roll's nearness, keenness and stillness.
    pub spot: u8,
}

/// A finder's own eye for what is hidden: points added to the percent
/// chance of every roll it makes to spot something, a hidden prop or a
/// [`Secret`](crate::secrets::Secret).
///
/// On the finder rather than rewritten into every hidden thing's chance,
/// so what makes someone keen, a worn thing or a status, is the game's to
/// keep on one entity, and taking it off takes its points with it. It adds
/// to a chance of nought too: something nobody could spot is found by the
/// keen. Not saved by the engine; a game keeps it as it keeps whatever it
/// is derived from.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Keen(pub u8);

/// How the spotting roll weighs a chance: by how near the hidden thing is,
/// and by whether the turn was spent standing still.
///
/// Set with [`PropsPlugin::spotting`], or replaced as a resource. The
/// default is the roll with neither, whole at any distance in sight and
/// the same whatever the turn was spent on, so a game that never asks
/// spots exactly as it always did.
///
/// Nearness is what makes a secret found by being near it, not by
/// glancing down a hall at it. Stillness is the lever a player who
/// suspects a wall has: there is no search action, since pressing a key at
/// every wall is a chore, and walking past still rolls, but the wait key
/// they already rest with searches harder.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpotRules {
    /// Within this many cells, Chebyshev, the chance is whole.
    pub near: u32,
    /// Past [`near`](Self::near) and out to this many cells it is half;
    /// beyond, there is none. Never less than `near`, which a debug build
    /// asserts on the first roll: a band that ends before it begins is a
    /// typo, read as no half band at all.
    pub far: u32,
    /// What a turn the finder spent waiting multiplies the chance by.
    pub still: u32,
}

impl Default for SpotRules {
    fn default() -> Self {
        Self { near: u32::MAX, far: u32::MAX, still: 1 }
    }
}

impl SpotRules {
    /// The chance, as `(in, out of)`, of spotting something of `spot`
    /// percent from `distance` cells away, by a finder `keen` points
    /// keener, on a turn it `waited` or did anything else; `None` when
    /// there is none at all.
    ///
    /// The percent and the keenness are summed and held to certainty
    /// first, so a keen eye never makes half a chance more than half;
    /// then halved past [`near`](Self::near), then multiplied for
    /// stillness and held to certainty again. A ratio rather than a
    /// rounded percent, so half of a five is two and a half and not two.
    pub(crate) fn chance(&self, spot: u8, keen: u8, distance: u32, waited: bool) -> Option<(u32, u32)> {
        debug_assert!(self.far >= self.near, "SpotRules far {} is less than near {}: the half band ends before it begins", self.far, self.near);
        let out_of = if distance <= self.near {
            100
        } else if distance <= self.far {
            200
        } else {
            return None;
        };
        let percent = (u32::from(spot) + u32::from(keen)).min(100);
        let chance = percent.saturating_mul(if waited { self.still } else { 1 }).min(out_of);
        (chance > 0).then_some((chance, out_of))
    }
}

/// Something hidden was spotted: a prop, or a place's secret.
///
/// One message for both, because what a game does with either is the
/// same, a line in the log and perhaps a sound, and a log that read two
/// would say the same thing twice over. A secret is one message for its
/// whole group, as it is one roll.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spotted {
    /// What.
    pub what: Finding,
    /// Who spotted it.
    pub by: Entity,
    /// Where: the prop's cell, or the cell of the secret nearest whoever
    /// spotted it.
    pub at: Point,
}

/// What was [`Spotted`].
///
/// The two kinds of hidden thing the engine has, not a list of contents:
/// what a prop or a tile is stays its registry's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finding {
    /// A hidden prop, an ordinary one from now on.
    Prop(Entity),
    /// A [`Secret`](crate::secrets::Secret)'s group, every cell of it now
    /// the tile it is. `is` is the tile on [`Spotted::at`], which for a
    /// door is the door.
    Secret {
        /// The group, as the place's secrets number it.
        group: u32,
        /// The tile it turned out to be.
        is: rl_grid::TileId,
    },
}

/// A hidden prop, and where it is.
type Unspotted<'w, 's> = Query<'w, 's, (Entity, &'static Position, Option<&'static OnMap>, &'static Hidden)>;

/// The player, as the roll reads it: who, where, what it sees, how keen,
/// and the turn it holds, whose being newly dealt is what starts a roll.
type Finder<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Position, Option<&'static OnMap>, &'static crate::components::Viewshed, Option<&'static Keen>, Ref<'static, MyTurn>),
    With<crate::components::Player>,
>;

/// Rolls for the player to spot what is hidden in sight: hidden props, and
/// the [`Secret`](crate::secrets::Secret)s of the place it stands in.
///
/// In [`DecideSet::Notice`](crate::plugin::DecideSet::Notice), beside the
/// stealth roll it resembles, and only for the player: a mind that will
/// not interact with a prop has nothing to do with having seen one, a trap
/// catches whoever steps on it either way, and a secret is wall to every
/// mind until the player finds it, so a hidden room is never a monster's
/// back door.
///
/// Once a turn, on the pass that deals the player its turn. The turn loop
/// runs a pass a frame while the player is deciding and one for every
/// monster that acts between, so a roll made every pass would find things
/// faster the longer a player looked at the screen and the more monsters
/// were awake.
///
/// Each roll is a chance from `SpotRules::chance`: the thing's own
/// percent with the player's [`Keen`] added, weighed by the distance to it
/// and multiplied when the player's last turn was a [`Wait`]. A secret is
/// rolled for once for its whole group, while any cell of it is in sight,
/// at the distance to its nearest cell, so a wide mouth is found no faster
/// than a one-cell door; found, every cell of it is laid as what it is.
/// Either way one [`Spotted`] says what was found, by whom and where.
///
/// Every roll comes from [`PropRng`], props first and then secrets by
/// group, so neither nudges a later blow. A secret is found through
/// `Commands`, which lands it at the pass's next sync point, so this
/// system only reads the map and orders against nothing that does.
///
/// [`Wait`]: crate::turn::Wait
pub fn spot_hidden_props(
    mut commands: Commands,
    mut spotted: MessageWriter<Spotted>,
    mut dice: SpotDice,
    mut stillness: Stillness,
    map: Res<WorldMap>,
    player: Finder,
    hidden: Unspotted,
) {
    let held = player.single().ok();
    stillness.note(held.as_ref().map(|(who, ..)| *who));
    let Some((who, stands, on, sight, keen, turn)) = held else { return };
    if !turn.is_added() {
        return;
    }
    let still = stillness.take();
    let here = on.map(|m| m.0).unwrap_or(MapId::ZERO);
    if here != map.current() {
        return;
    }
    let keen = keen.map_or(0, |k| k.0);
    let from = stands.0;
    for (prop, at, prop_on, secret) in &hidden {
        if prop_on.map(|m| m.0).unwrap_or(MapId::ZERO) != here || !sight.can_see(at.0) {
            continue;
        }
        if !dice.roll(secret.spot, keen, geometry::chebyshev(from, at.0), still) {
            continue;
        }
        commands.entity(prop).remove::<Hidden>();
        spotted.write(Spotted { what: Finding::Prop(prop), by: who, at: at.0 });
    }
    for nearest in groups_in_sight(map.secrets(here), sight, from) {
        if !dice.roll(nearest.spot, keen, geometry::chebyshev(from, nearest.at), still) {
            continue;
        }
        commands.queue(move |world: &mut World| {
            world.resource_mut::<WorldMap>().find_secret(here, nearest.at);
        });
        spotted.write(Spotted { what: Finding::Secret { group: nearest.group, is: nearest.is }, by: who, at: nearest.at });
    }
}

/// The spotting roll's dice: the chance [`SpotRules`] gives, drawn from
/// [`PropRng`].
#[derive(bevy::ecs::system::SystemParam)]
pub struct SpotDice<'w> {
    rules: Res<'w, SpotRules>,
    rng: ResMut<'w, PropRng>,
}

impl SpotDice<'_> {
    /// Whether something of `spot` percent, `distance` cells away, is
    /// spotted by a finder `keen` points keener that `waited` or not. No
    /// draw is made where there is no chance, so a thing out of reach
    /// moves the stream no more than one out of sight.
    fn roll(&mut self, spot: u8, keen: u8, distance: i32, waited: bool) -> bool {
        use rand::Rng;
        let distance = u32::try_from(distance).unwrap_or(0);
        self.rules.chance(spot, keen, distance, waited).is_some_and(|(chance, out_of)| self.rng.0.random_ratio(chance, out_of))
    }
}

/// Whether the player's last turn was spent waiting, as the spotting roll
/// reads it.
///
/// Read from the [`Wait`](crate::turn::Wait) the player asked for, on the
/// pass that spends it, while the player still holds the turn it spends,
/// and kept until its next turn is dealt, which is when the roll is made.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Stillness<'w, 's> {
    waits: MessageReader<'w, 's, crate::turn::Intent<crate::turn::Wait>>,
    waited: Local<'s, bool>,
}

impl Stillness<'_, '_> {
    /// Notes whether `who`, the player while it holds a turn, asked to
    /// wait since this was last read; with nobody, only reads past them.
    fn note(&mut self, who: Option<Entity>) {
        let asked = self.waits.read().any(|w| Some(w.actor) == who);
        *self.waited |= asked;
    }

    /// Whether it waited, forgotten for the turn after.
    fn take(&mut self) -> bool {
        std::mem::take(&mut *self.waited)
    }
}

/// Every group of `secrets` with a cell in `sight`, as its cell in sight
/// nearest `from`, in group order.
///
/// A `BTreeMap` by group, so the groups are rolled for in the same order
/// on every run of a seed, whatever order their cells were listed in.
/// Nearest by Chebyshev distance, the one the roll weighs, and among
/// cells at the same distance by the straight line, so the cell named is
/// the one square ahead rather than whichever was listed first. Only a
/// cell in sight is measured: the end of a wide mouth round a corner is
/// nearer than the end the finder looks at, and weighing it would find
/// the mouth from closer than anything of it that is seen.
fn groups_in_sight(secrets: &[crate::secrets::Secret], sight: &crate::components::Viewshed, from: Point) -> Vec<crate::secrets::Secret> {
    let mut groups: std::collections::BTreeMap<u32, crate::secrets::Secret> = std::collections::BTreeMap::new();
    for s in secrets.iter().filter(|s| sight.can_see(s.at)) {
        let nearest = groups.entry(s.group).or_insert(*s);
        if nearness(from, s.at) < nearness(from, nearest.at) {
            *nearest = *s;
        }
    }
    groups.into_values().collect()
}

/// How near `at` is to `from`: Chebyshev first, then the straight line.
fn nearness(from: Point, at: Point) -> (i32, i32) {
    let (dx, dy) = (at.x - from.x, at.y - from.y);
    (geometry::chebyshev(from, at), dx * dx + dy * dy)
}

/// Set once a prop with nothing to show has been reported, so the report
/// is made once and not per crate.
#[derive(Resource, Default)]
pub struct ToldPropsAreBare;

/// Reports a prop that nothing can draw or name.
///
/// A prop is seen through its `Name` and its glyph: the nearby rail, the
/// look cursor and the map view all read those and nothing else, so one
/// without them is on the map and invisible to the player. That is a
/// spawn bug every time, never a choice, and it is quiet unless it is
/// said out loud here.
///
/// In `PostUpdate` and in no set, because a prop is often put down while
/// a place is built, before play begins, and `Added` matches for one frame
/// only: a report that waits for the run to start is a report that never
/// happens.
pub fn report_bare_props(mut commands: Commands, told: Option<Res<ToldPropsAreBare>>, bare: Query<Entity, (Added<Prop>, Without<Name>)>) {
    if told.is_some() || bare.is_empty() {
        return;
    }
    commands.init_resource::<ToldPropsAreBare>();
    error!("a prop was spawned with no `Name`, so nothing can list it or look at it; spawn props with `spawn_prop`, which names them from their definition");
}

/// The stages of putting props in place, inside
/// [`EngineSet::Stream`](crate::plugin::EngineSet::Stream).
///
/// A game answers [`FillContainer`] in [`PropSet::Fill`], which is what
/// makes what goes into a crate land in the frame the crate was put down
/// rather than in that frame or the next depending on which way the
/// executor happened to run two systems. A named set rather than
/// `.after(stock_containers)`, because a game orders itself against the
/// engine's phases and never against the engine's functions.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropSet {
    /// The engine's: build each prop's effects, and ask for what a
    /// container holds.
    Stock,
    /// The game's: spawn what was asked for and put it in.
    Fill,
}

/// Props: things on the map that are neither actors nor items.
///
/// Opt-in. Without it a game may still spawn whatever it likes; with it,
/// the engine owns what a prop is and, in later slices, what one offers,
/// holds, and does to whoever sets it off.
pub struct PropsPlugin;

impl PropsPlugin {
    /// Props, with the spotting roll weighed by `rules`: how near a hidden
    /// thing must be, and what a turn spent waiting is worth. Plain
    /// [`PropsPlugin`] keeps [`SpotRules::default`], whole at any distance
    /// in sight and the same whatever the turn was spent on.
    ///
    /// Inserted as [`SpotRules`], so a game may also replace it later.
    pub fn spotting(rules: SpotRules) -> impl Plugin {
        move |app: &mut App| {
            app.insert_resource(rules);
            app.add_plugins(PropsPlugin);
        }
    }
}

impl Plugin for PropsPlugin {
    fn build(&self, app: &mut App) {
        use crate::plugin::{DecideSet, Needs, Reads, ResolveSet, Turn};
        use crate::seed::AddStream;
        use crate::turn::AddAction;
        crate::effects::ensure(app);
        app.needs::<Registries>("PropsPlugin", "`Registries`, with `props` loaded from a `props.ron`")
            .init_resource::<Verbs>()
            .init_resource::<OfferedHere>()
            .add_message::<Interacted>()
            .add_action::<Interact>()
            .add_message::<FillContainer>()
            .add_message::<Spotted>()
            .init_resource::<SpotRules>()
            // What props read that belongs to plugins a game may have left
            // out: taking out of a container says so in items' own words,
            // and a prop that bursts hears its own death. A game may have
            // props without items or combat, and then these queues simply
            // stay empty. What an effect writes is `EffectsPlugin`'s.
            .reads::<crate::items::ItemEvent>()
            .reads::<crate::combat::DeathEvent>()
            .add_action::<Take>()
            .add_stream::<PropRng>("PropsPlugin")
            // In `EngineSet::Stream`, where the world is brought in: a prop
            // is put down as a place is built, and stocking it, building
            // its effects and reporting a bare one are all of that phase
            // rather than of a turn. In a named set rather than bare
            // `Update` so they are ordered against everything a game runs
            // in the phases around them, an exclusive system included.
            .configure_sets(Update, (PropSet::Stock, PropSet::Fill).chain().in_set(crate::plugin::EngineSet::Stream))
            // In `PostUpdate`, and in no set: a prop is often put down while
            // a place is built, before play begins, and `Added` matches for
            // one frame only, so a report inside a phase that waits for the
            // run to start is a report that never happens. Its own schedule
            // rather than bare `Update`, where it would be unordered against
            // whatever exclusive system a game runs there.
            .add_systems(PostUpdate, report_bare_props)
            // Beside the ability gate, unordered and saying so: a price
            // counts the stacks a pack holds, and that gate reaches them
            // through the ability resolver's mutable query though it only
            // reads them, so neither writes `Stack` here; `docs/TODO.md`
            // holds giving the gate a read-only param instead.
            .add_systems(Turn, (check_priced_offers, offer_here).chain().in_set(DecideSet::Offer).ambiguous_with(crate::ability::offer_abilities))
            .add_systems(Turn, (resolve_interactions, resolve_takes).in_set(ResolveSet::Act))
            .add_systems(Turn, close_emptied_containers.in_set(crate::plugin::ReactSet::Settle))
            // Outside the turn loop: a container is stocked the frame it is
            // put down, which is while a place is being built and before
            // anyone holds a turn.
            .add_systems(Update, (build_prop_effects, arm_props, stock_containers).chain().in_set(PropSet::Stock))
            .add_systems(Turn, spot_hidden_props.in_set(DecideSet::Notice))
            .add_systems(Turn, perceive_props.in_set(crate::plugin::PerceiveSet::Annotate))
            // Armed again at the start of every pass, for a prop a reaction put
            // down mid-frame and a step reached before the next frame's stock.
            .add_systems(Turn, arm_props.in_set(crate::plugin::TurnSet::Schedule))
            // At the end of the travel it reads, after every step, swap and
            // warp, so every action in the pass is ordered after it and a
            // plate's report is in before `ResolveSet::Triggers` lands it.
            .add_systems(Turn, report_entered.in_set(ResolveSet::Travel).after(crate::places::resolve_warps))
            .add_systems(Turn, report_destroyed.in_set(crate::plugin::TurnSet::React))
            .add_systems(OnEnter(crate::state::EngineState::Playing), check_tagged_contents);
    }

    fn finish(&self, app: &mut App) {
        crate::plugin::depends_on::<crate::plugin::CorePlugin>(app, "PropsPlugin");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::headless_app;
    use crate::turn::Occupancy;
    use rl_rules::Names;

    /// Two props: one that stands in the way and can be broken, one that
    /// lies flat and cannot.
    const PROPS: &str = r#"#![enable(implicit_some)]
        [
            (name: "supply crate", glyph: '&', color: (r: 190, g: 165, b: 115), blocks: true, health: 6,
             container: (contents: [(item: "slug", count: (8, 12))]), offers: [(verb: "open", time: 200)]),
            (name: "pressure plate", glyph: '^', color: (r: 230, g: 140, b: 51)),
        ]"#;

    fn arena() -> (App, Point) {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, PropsPlugin));
        let start = crate::testing::ground(&mut app);
        let registries = Registries { props: rl_rules::prop::load(PROPS, &Names::new()).expect("the props load"), ..Default::default() };
        app.insert_resource(registries);
        app.insert_resource(crate::seed::Seed(crate::testing::TEST_SEED));
        (app, start)
    }

    /// What a definition says is what stands there, and the parts it does
    /// not ask for are not put on.
    #[test]
    fn a_prop_is_spawned_with_what_its_definition_asks_for_and_nothing_else() {
        let (mut app, start) = arena();
        let registries = app.world().resource::<Registries>().clone();
        let (crate_id, plate_id) = (registries.props.expect("supply crate"), registries.props.expect("pressure plate"));
        let (supply, plate) = {
            let mut commands = app.world_mut().commands();
            (
                spawn_prop(&mut commands, &registries, crate_id, start.offset(1, 0), MapId::ZERO),
                spawn_prop(&mut commands, &registries, plate_id, start.offset(2, 0), MapId::ZERO),
            )
        };
        app.update();

        let w = app.world();
        assert_eq!(w.get::<Name>(supply).map(|n| n.as_str().to_string()), Some("supply crate".into()), "named from its definition");
        assert_eq!(w.get::<Position>(supply).map(|p| p.0), Some(start.offset(1, 0)), "where the game put it");
        assert!(w.get::<Blocks>(supply).is_some(), "a crate stands in the way");
        assert_eq!(w.get::<Health>(supply).map(|h| h.max), Some(6), "and can be broken");
        assert!(w.get::<Blocks>(plate).is_none(), "a plate does not");
        assert!(w.get::<Health>(plate).is_none(), "and nothing breaks it");
    }

    /// A prop that blocks is in the way of everything that reads the
    /// index, with no work of its own: the engine already knows what
    /// `Blocks` at a cell means.
    #[test]
    fn a_blocking_prop_is_in_the_index_like_anything_else_that_blocks() {
        let (mut app, start) = arena();
        let registries = app.world().resource::<Registries>().clone();
        let id = registries.props.expect("supply crate");
        let at = start.offset(1, 0);
        let supply = spawn_prop(&mut app.world_mut().commands(), &registries, id, at, MapId::ZERO);
        app.world_mut().resource_mut::<NextState<crate::state::EngineState>>().set(crate::state::EngineState::Playing);
        app.update();
        app.update();
        assert_eq!(app.world().resource::<Occupancy>().first_at(at), Some(supply), "the crate holds the cell");
    }

    /// A prop's trigger that makes a noise is built against the sounds the
    /// game declared, so a plate that sounds an alarm is armed like any
    /// other, where a sound nobody declared would leave it unarmed.
    #[test]
    fn a_props_trigger_naming_a_declared_sound_builds_when_the_game_has_noise() {
        let mut app = headless_app();
        let rules = crate::noise::NoiseRules { step: 0, strike: 0, door: 0, landing: 0, door_muffle: 0 };
        app.add_plugins((crate::fov::FovPlugin, PropsPlugin, crate::noise::NoisePlugin::new(rules)));
        crate::noise::AddSound::add_sound(&mut app, "chime");
        crate::testing::ground(&mut app);
        let text = r#"#![enable(implicit_some)] [(name: "alarm plate", glyph: '^', color: (r: 1, g: 2, b: 3),
            triggers: [(on: "entered", effects: [(kind: "Noise", args: (sound: "chime", loudness: 9))])])]"#;
        let props = rl_rules::prop::load(text, &Names::new()).expect("the props load");
        let plate = props.expect("alarm plate");
        app.insert_resource(Registries { props, ..Default::default() });
        app.insert_resource(crate::seed::Seed(crate::testing::TEST_SEED));
        app.world_mut().resource_mut::<NextState<crate::state::EngineState>>().set(crate::state::EngineState::Playing);
        app.update();
        app.update();
        let built = app.world().resource::<PropEffects>().triggers.get(plate.index()).map(|t| t.0.len());
        assert_eq!(built, Some(1), "the plate's one trigger is built");
    }

    /// A prop nothing can name is a prop the player never sees, so it is
    /// said out loud, once.
    #[test]
    fn a_prop_with_nothing_to_show_is_reported_once() {
        let (mut app, start) = arena();
        app.world_mut().spawn((Prop, Position(start.offset(1, 0))));
        app.world_mut().spawn((Prop, Position(start.offset(2, 0))));
        app.update();
        assert!(app.world().contains_resource::<ToldPropsAreBare>(), "a prop nothing can list is reported");
    }
}

#[cfg(test)]
mod offers {
    use super::*;
    use crate::components::{Actor, Blocks, Player, Viewshed};
    use crate::items::{Inventory, Item, Tagged};
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use crate::turn::{Intent, Turns};
    use rl_rules::{Names, Registry, TagDef};

    /// A crate that opens, a console with a verb of the game's own, a
    /// cache that wants a cutter, a workbench offering two things, and a
    /// lathe that offers nothing and is only scenery.
    const PROPS: &str = r#"#![enable(implicit_some)]
        [
            (name: "supply crate", glyph: '&', color: (r: 190, g: 165, b: 115), blocks: true,
             container: (contents: [(item: "slug", count: 1)]), offers: [(verb: "open", time: 200)]),
            (name: "reactor console", glyph: '%', color: (r: 89, g: 217, b: 230), blocks: true,
             offers: [(verb: "charge", time: 300)]),
            (name: "locked cache", glyph: '&', color: (r: 204, g: 204, b: 217), blocks: true,
             container: (contents: [(item: "slug", count: 1)], locked: "cutter"), offers: [(verb: "open", time: 250)]),
            (name: "workbench", glyph: 'T', color: (r: 150, g: 120, b: 90), blocks: true,
             offers: [(verb: "search", time: 100), (verb: "charge", time: 100)]),
            (name: "lathe", glyph: '0', color: (r: 150, g: 160, b: 170), blocks: true),
        ]"#;

    fn arena() -> (App, Point) {
        let mut app = headless_app();
        // Bump is CorePlugin's, which `headless_app` already added.
        app.add_plugins((crate::fov::FovPlugin, PropsPlugin, crate::items::ItemsPlugin));
        app.add_verb("charge");
        let start = crate::testing::ground(&mut app);
        let tags = Registry::from_defs(vec![TagDef::new("cutter")]).expect("one tag");
        let props = rl_rules::prop::load(PROPS, &Names::new().tags(&tags)).expect("the props load");
        app.insert_resource(Registries { props, tags, ..Default::default() });
        app.insert_resource(crate::seed::Seed(crate::testing::TEST_SEED));
        (app, start)
    }

    fn player_at(app: &mut App, at: Point) -> Entity {
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(at), Viewshed::new(8), OnMap(MapId::ZERO))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        player
    }

    fn put(app: &mut App, name: &str, at: Point) -> Entity {
        let registries = app.world().resource::<Registries>().clone();
        let id = registries.props.expect(name);
        let prop = spawn_prop(&mut app.world_mut().commands(), &registries, id, at, MapId::ZERO);
        app.update();
        prop
    }

    /// A prop whose definition does nothing is marked as scenery when it
    /// is put down, and one that offers something is not.
    #[test]
    fn a_prop_that_does_nothing_is_put_down_as_scenery_and_one_that_offers_is_not() {
        let (mut app, start) = arena();
        let lathe = put(&mut app, "lathe", start.offset(1, 0));
        let console = put(&mut app, "reactor console", start.offset(2, 0));
        assert!(app.world().entity(lathe).contains::<Scenery>());
        assert!(!app.world().entity(console).contains::<Scenery>());
    }

    /// Something to do with it or about it: what offers, holds or hides
    /// is, and what can only be broken, or answers a moment in plain
    /// sight, is not, though neither of those is scenery.
    #[test]
    fn a_prop_has_business_when_it_offers_holds_or_hides_and_not_when_it_can_only_be_broken() {
        let text = r#"#![enable(implicit_some)] [
            (name: "chest", glyph: '=', color: (r: 1, g: 1, b: 1), container: (contents: []), offers: [(verb: "open", time: 100)]),
            (name: "lever", glyph: '/', color: (r: 1, g: 1, b: 1), offers: [(verb: "pull", time: 100)]),
            (name: "trap", glyph: '^', color: (r: 1, g: 1, b: 1), hidden: (spot: 10), triggers: [(on: "entered", effects: [])]),
            (name: "barrel", glyph: '0', color: (r: 1, g: 1, b: 1), health: 4),
            (name: "sac", glyph: 'o', color: (r: 1, g: 1, b: 1), health: 1, triggers: [(on: "destroyed", effects: [])]),
            (name: "plate", glyph: '^', color: (r: 1, g: 1, b: 1), triggers: [(on: "entered", effects: [])]),
            (name: "lathe", glyph: '0', color: (r: 1, g: 1, b: 1)),
        ]"#;
        let props = rl_rules::prop::load(text, &Names::new()).expect("the props load");
        let business = |name: &str| has_business(props.get(props.expect(name)));
        for name in ["chest", "lever", "trap"] {
            assert!(business(name), "{name} is something to do with or about");
        }
        for name in ["barrel", "sac", "plate", "lathe"] {
            assert!(!business(name), "{name} is not");
        }
        assert!(!props.get(props.expect("sac")).is_scenery(), "and a sac is still no scenery, so a cursor steps onto it");
    }

    /// The gate answers for whoever holds the turn, about what it can
    /// reach: beside it or underfoot, and nothing further off.
    #[test]
    fn the_gate_offers_what_the_turn_holder_can_reach_and_no_more() {
        let (mut app, start) = arena();
        let beside = put(&mut app, "supply crate", start.offset(1, 0));
        let underfoot = put(&mut app, "reactor console", start);
        let far = put(&mut app, "workbench", start.offset(4, 0));
        let player = player_at(&mut app, start);

        let offered = app.world().resource::<OfferedHere>();
        let props: Vec<Entity> = offered.for_actor(player).iter().map(|o| o.target).collect();
        assert!(props.contains(&beside), "what stands beside it");
        assert!(props.contains(&underfoot), "and what it stands on");
        assert!(!props.contains(&far), "but not what is four cells away");
        assert!(offered.for_actor(Entity::from_raw_u32(9999).unwrap()).is_empty(), "and nothing at all for anyone else");
    }

    /// An offer that wants something names what: the screen greys the row
    /// and says why, rather than the player spending a turn to find out.
    #[test]
    fn an_offer_that_wants_something_is_listed_and_refused_until_it_is_carried() {
        let (mut app, start) = arena();
        let cache = put(&mut app, "locked cache", start.offset(1, 0));
        let player = player_at(&mut app, start);
        let cutter = app.world().resource::<Registries>().tags.expect("cutter");

        let refused = app.world().resource::<OfferedHere>().find(player, cache, Verbs::OPEN).expect("it is listed").refused;
        assert_eq!(refused, Some(Refused::Needs(cutter)), "listed, and refused for a reason it can name");

        let tool = app.world_mut().spawn((Item, Tagged(vec![cutter]))).id();
        app.world_mut().entity_mut(player).insert(Inventory { items: vec![tool] });
        app.update();
        assert_eq!(
            app.world().resource::<OfferedHere>().find(player, cache, Verbs::OPEN).expect("still listed").refused,
            None,
            "and open once the cutter is in the bag"
        );
    }

    /// The offer's time is what the turn costs, and the interaction is
    /// announced for whoever answers that verb.
    #[test]
    fn an_interaction_costs_what_the_offer_said_and_says_what_was_done() {
        let (mut app, start) = arena();
        let console = put(&mut app, "reactor console", start.offset(1, 0));
        let player = player_at(&mut app, start);
        let charge = app.world().resource::<Verbs>().get("charge").expect("the game's verb");

        #[derive(Resource, Default)]
        struct Seen(Vec<Interacted>);
        app.init_resource::<Seen>().add_systems(crate::plugin::Turn, |mut seen: ResMut<Seen>, mut done: MessageReader<Interacted>| {
            seen.0.extend(done.read().copied());
        });

        let before = app.world().resource::<Turns>().now();
        app.world_mut().write_message(Intent::new(player, Interact { target: console, verb: charge, thing: None }));
        app.update();

        assert_eq!(app.world().resource::<Turns>().now(), before + 300, "the turn cost what the offer said");
        assert_eq!(
            app.world().resource::<Seen>().0,
            vec![Interacted { actor: player, target: console, verb: charge, thing: None }],
            "and the game is told what was done"
        );
    }

    /// Nothing is spent finding out what the gate already worked out.
    #[test]
    fn an_interaction_nobody_offered_costs_nothing() {
        let (mut app, start) = arena();
        let far = put(&mut app, "supply crate", start.offset(5, 0));
        let player = player_at(&mut app, start);

        let before = app.world().resource::<Turns>().now();
        app.world_mut().write_message(Intent::new(player, Interact { target: far, verb: Verbs::OPEN, thing: None }));
        app.update();
        assert_eq!(app.world().resource::<Turns>().now(), before, "a turn is not spent learning it was out of reach");
    }

    /// Walking into a crate opens it, the way walking into a door opens
    /// that; walking into one that offers two things does not guess.
    #[test]
    fn a_bump_into_a_prop_takes_up_its_one_offer_and_never_chooses_between_two() {
        let (mut app, start) = arena();
        let supply = put(&mut app, "supply crate", start.offset(1, 0));
        let player = player_at(&mut app, start);

        #[derive(Resource, Default)]
        struct Seen(Vec<Interacted>);
        app.init_resource::<Seen>().add_systems(crate::plugin::Turn, |mut seen: ResMut<Seen>, mut done: MessageReader<Interacted>| {
            seen.0.extend(done.read().copied());
        });

        app.world_mut().write_message(Intent::new(player, crate::bump::Bump(rl_core::Direction::East)));
        app.update();
        assert_eq!(
            app.world().resource::<Seen>().0,
            vec![Interacted { actor: player, target: supply, verb: Verbs::OPEN, thing: None }],
            "the walk key opened the crate"
        );
        assert_eq!(app.world().get::<Position>(player).map(|p| p.0), Some(start), "and the player did not move");

        // A bench offering two things is left for a screen to ask about.
        let (mut app, start) = arena();
        let _bench = put(&mut app, "workbench", start.offset(1, 0));
        let player = player_at(&mut app, start);
        app.init_resource::<Seen>().add_systems(crate::plugin::Turn, |mut seen: ResMut<Seen>, mut done: MessageReader<Interacted>| {
            seen.0.extend(done.read().copied());
        });
        app.world_mut().write_message(Intent::new(player, crate::bump::Bump(rl_core::Direction::East)));
        app.update();
        assert!(app.world().resource::<Seen>().0.is_empty(), "a walk key does not choose between two offers");
    }

    /// Asked for: the next turn dealt is spent in the decide stage.
    #[derive(Resource)]
    struct SpendNext;

    /// Spends the turn of whoever holds it in the decide stage, before any
    /// intent is read, as a status that holds spends one; once.
    fn spend_next(
        mut commands: Commands,
        mut acting: ResMut<crate::turn::Acting>,
        mut done: MessageWriter<crate::turn::ActionDone>,
        holders: Query<Entity, With<crate::components::MyTurn>>,
    ) {
        for actor in &holders {
            acting.claim_decision(actor);
            if acting.claim_action(actor) {
                done.write(crate::turn::ActionDone { actor, cost: rl_core::turn::BASE_ACTION_COST });
            }
        }
        commands.remove_resource::<SpendNext>();
    }

    #[derive(Resource, Default)]
    struct Asked(Vec<crate::bump::Bumped>);

    fn record_asked(mut bumps: MessageReader<crate::bump::Bumped>, mut asked: ResMut<Asked>) {
        asked.0.extend(bumps.read().copied());
    }

    /// A key that finds its turn already spent comes to nothing: not the
    /// question a bench offering two things raises, which would open a
    /// screen for a turn that is gone.
    #[test]
    fn a_bump_by_an_actor_whose_turn_is_already_spent_asks_nothing_of_a_prop() {
        let (mut app, start) = arena();
        let _bench = put(&mut app, "workbench", start.offset(1, 0));
        let player = player_at(&mut app, start);
        app.init_resource::<Asked>().insert_resource(SpendNext).add_systems(
            crate::plugin::Turn,
            (spend_next.run_if(resource_exists::<SpendNext>).in_set(crate::plugin::DecideSet::Game), record_asked.in_set(crate::plugin::TurnSet::Record)),
        );
        app.world_mut().write_message(Intent::new(player, crate::bump::Bump(rl_core::Direction::East)));
        app.update();
        assert!(app.world().resource::<Asked>().0.is_empty(), "no question asked of the bench");
        assert_eq!(app.world().resource::<Turns>().now(), rl_core::turn::BASE_ACTION_COST, "the turn was spent once, as a wait");
        assert_eq!(app.world().get::<Position>(player).map(|p| p.0), Some(start), "and the player stayed where it was");
    }
}

#[cfg(test)]
mod actor_offers {
    use super::*;
    use crate::components::{Actor, Blocks, Player, Viewshed};
    use crate::items::{Inventory, Item, Stack, Tagged};
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use crate::trade::Currency;
    use crate::turn::{Intent, Turns};
    use rl_rules::{Registry, TagDef};

    /// A world with props on, a verb of the game's own, `free`, a tag for
    /// what a lock might want, and a tag `scrip` a test may make the
    /// currency.
    fn arena() -> (App, Point, VerbId, TagId) {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, PropsPlugin, crate::items::ItemsPlugin));
        app.add_verb("free");
        let start = crate::testing::ground(&mut app);
        let tags = Registry::from_defs(vec![TagDef::new("key"), TagDef::new("scrip")]).expect("two tags");
        let key = tags.expect("key");
        app.insert_resource(Registries { tags, ..Default::default() });
        app.insert_resource(crate::seed::Seed(crate::testing::TEST_SEED));
        let free = app.world().resource::<Verbs>().get("free").expect("declared");
        (app, start, free, key)
    }

    fn someone(app: &mut App, at: Point, offers: Offers) -> Entity {
        app.world_mut().spawn((Actor, Blocks, Position(at), OnMap(MapId::ZERO), Name::new("captive"), offers)).id()
    }

    fn player_at(app: &mut App, at: Point, offers: Offers) -> Entity {
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(at), Viewshed::new(8), OnMap(MapId::ZERO), offers)).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        player
    }

    fn offered(app: &App, actor: Entity) -> Vec<Offer> {
        app.world().resource::<OfferedHere>().for_actor(actor).to_vec()
    }

    #[derive(Resource, Default)]
    struct Seen(Vec<Interacted>);

    fn watch(app: &mut App) {
        app.init_resource::<Seen>().add_systems(crate::plugin::Turn, |mut seen: ResMut<Seen>, mut done: MessageReader<Interacted>| {
            seen.0.extend(done.read().copied());
        });
    }

    /// Someone beside the turn holder offers what its own `Offers` names,
    /// someone two cells off offers nothing, and the turn holder is not
    /// offered what it offers others.
    #[test]
    fn an_actor_beside_the_turn_holder_offers_what_it_names_and_one_further_off_does_not() {
        let (mut app, start, free, _) = arena();
        let beside = someone(&mut app, start.offset(1, 1), Offers(vec![Offering::new(free, 200)]));
        someone(&mut app, start.offset(2, 0), Offers(vec![Offering::new(free, 200)]));
        let player = player_at(&mut app, start, Offers(vec![Offering::new(free, 100)]));
        assert_eq!(offered(&app, player), vec![Offer { target: beside, verb: free, time: 200, price: None, asks: None, refused: None }]);
    }

    /// The offer is asked of where the actor stands when the gate runs,
    /// never remembered from where it stood: walked off, it offers
    /// nothing and taking it up costs nothing; back beside, it offers
    /// again.
    #[test]
    fn what_an_actor_offers_moves_with_it() {
        let (mut app, start, free, _) = arena();
        let captive = someone(&mut app, start.offset(1, 0), Offers(vec![Offering::new(free, 200)]));
        let player = player_at(&mut app, start, Offers::default());
        watch(&mut app);
        assert_eq!(offered(&app, player).len(), 1, "beside, it offers");

        app.world_mut().entity_mut(captive).insert(Position(start.offset(4, 0)));
        app.update();
        assert!(offered(&app, player).is_empty(), "walked off, it offers nothing");
        let before = app.world().resource::<Turns>().now();
        app.world_mut().write_message(Intent::new(player, Interact { target: captive, verb: free, thing: None }));
        app.update();
        assert_eq!(app.world().resource::<Turns>().now(), before, "and reaching for it costs nothing");
        assert!(app.world().resource::<Seen>().0.is_empty(), "nor does anything happen");

        app.world_mut().entity_mut(captive).insert(Position(start.offset(-1, 0)));
        app.update();
        assert_eq!(offered(&app, player).iter().map(|o| o.target).collect::<Vec<_>>(), vec![captive], "beside again, it offers again");
    }

    /// An actor's offer that wants something is listed and refused for
    /// free until it is carried, then costs its time and is answered with
    /// the same message a prop's is.
    #[test]
    fn an_actors_offer_is_refused_until_its_need_is_carried_and_then_answered_as_a_props_is() {
        let (mut app, start, free, key) = arena();
        let captive = someone(&mut app, start.offset(1, 0), Offers(vec![Offering::new(free, 200).needing(key)]));
        let player = player_at(&mut app, start, Offers::default());
        watch(&mut app);
        let refused = app.world().resource::<OfferedHere>().find(player, captive, free).expect("listed").refused;
        assert_eq!(refused, Some(Refused::Needs(key)), "listed, and refused for a reason it can name");

        let before = app.world().resource::<Turns>().now();
        app.world_mut().write_message(Intent::new(player, Interact { target: captive, verb: free, thing: None }));
        app.update();
        assert_eq!(app.world().resource::<Turns>().now(), before, "a refused offer costs nothing");

        let tool = app.world_mut().spawn((Item, Tagged(vec![key]))).id();
        app.world_mut().entity_mut(player).insert(Inventory { items: vec![tool] });
        app.update();
        let before = app.world().resource::<Turns>().now();
        app.world_mut().write_message(Intent::new(player, Interact { target: captive, verb: free, thing: None }));
        app.update();
        assert_eq!(app.world().resource::<Turns>().now(), before + 200, "carried, it costs what the offer said");
        assert_eq!(app.world().resource::<Seen>().0, vec![Interacted { actor: player, target: captive, verb: free, thing: None }], "and the game is told");
    }

    /// An offer that chooses first is taken up only with a thing the taker
    /// still carries, charged its own time, and the game hears the thing;
    /// none at all, one no longer carried, or one given to an offer that
    /// chooses none, is refused for nothing.
    #[test]
    fn a_choosing_offer_is_taken_up_only_with_a_thing_still_carried_and_heard_with_it() {
        let (mut app, start, free, _) = arena();
        let among = app.register_system(|_: In<Chooser>| Vec::new());
        let captive = someone(&mut app, start.offset(1, 0), Offers(vec![Offering::new(free, 200).choosing("Free them with which?", among)]));
        let plain = someone(&mut app, start.offset(-1, 0), Offers(vec![Offering::new(free, 100)]));
        let player = player_at(&mut app, start, Offers::default());
        watch(&mut app);
        let (kept, gone) = (app.world_mut().spawn(Item).id(), app.world_mut().spawn(Item).id());
        app.world_mut().entity_mut(player).insert(Inventory { items: vec![kept] });
        app.update();
        let now = |app: &App| app.world().resource::<Turns>().now();
        let before = now(&app);
        for (target, thing) in [(captive, None), (captive, Some(gone)), (plain, Some(kept))] {
            app.world_mut().write_message(Intent::new(player, Interact { target, verb: free, thing }));
            app.update();
            assert_eq!(now(&app), before, "{thing:?} for {target:?} is refused for nothing");
        }
        assert!(app.world().resource::<Seen>().0.is_empty(), "and nothing is heard");

        app.world_mut().write_message(Intent::new(player, Interact { target: captive, verb: free, thing: Some(kept) }));
        app.update();
        assert_eq!(now(&app), before + 200, "a thing carried costs the offer's own time");
        assert_eq!(app.world().resource::<Seen>().0, vec![Interacted { actor: player, target: captive, verb: free, thing: Some(kept) }], "heard with it");
    }

    /// The arena with `scrip` as the currency, and a stack of `count` of it.
    fn scrip(app: &mut App, count: u32) -> Entity {
        let scrip = app.world().resource::<Registries>().tags.expect("scrip");
        app.insert_resource(Currency(scrip));
        app.world_mut().spawn((Item, Tagged(vec![scrip]), Stack { key: 1, count })).id()
    }

    /// How much currency `who` carries, counted by the trade slice's rule.
    fn purse(world: &World, who: Entity) -> u32 {
        let currency = *world.resource::<Currency>();
        world.get::<Inventory>(who).map_or(0, |bag| currency.count(bag, |i| (world.get::<Tagged>(i), world.get::<Stack>(i))))
    }

    /// An offer with a price is listed to a taker carrying less, refused
    /// with the price it wants, and costs nothing reached for; one priced
    /// at exactly what the taker carries is open.
    #[test]
    fn an_offer_whose_taker_carries_less_than_its_price_is_refused_and_says_how_much() {
        let (mut app, start, free, _) = arena();
        let coins = scrip(&mut app, 30);
        let dear = someone(&mut app, start.offset(1, 0), Offers(vec![Offering::new(free, 200).priced(50)]));
        let fair = someone(&mut app, start.offset(-1, 0), Offers(vec![Offering::new(free, 200).priced(30)]));
        let player = player_at(&mut app, start, Offers::default());
        app.world_mut().entity_mut(player).insert(Inventory { items: vec![coins] });
        app.update();
        watch(&mut app);
        let here = app.world().resource::<OfferedHere>();
        let dear_offer = here.find(player, dear, free).expect("listed").clone();
        assert_eq!((dear_offer.price, dear_offer.refused), (Some(50), Some(Refused::Short(50))), "listed with its price, and refused saying how much");
        assert_eq!(here.find(player, fair, free).map(|o| o.refused), Some(None), "30 carried meets a price of 30");

        let before = app.world().resource::<Turns>().now();
        app.world_mut().write_message(Intent::new(player, Interact { target: dear, verb: free, thing: None }));
        app.update();
        assert_eq!(app.world().resource::<Turns>().now(), before, "a price not met costs nothing");
        assert!(app.world().resource::<Seen>().0.is_empty(), "and nothing is done");
        assert_eq!(purse(app.world(), player), 30, "nor paid");
    }

    #[derive(Resource, Default)]
    struct Paid(Vec<(u32, u32)>);

    /// Taken up, the price leaves the taker's pack for the offerer's own,
    /// one made for an offerer that carried none, and the game's answer in
    /// `React` finds it already paid.
    #[test]
    fn taken_up_the_price_leaves_the_takers_currency_for_the_offerers_pack_before_the_answer_runs() {
        let (mut app, start, free, _) = arena();
        let coins = scrip(&mut app, 80);
        let offerer = someone(&mut app, start.offset(1, 0), Offers(vec![Offering::new(free, 200).priced(50)]));
        let player = player_at(&mut app, start, Offers::default());
        app.world_mut().entity_mut(player).insert(Inventory { items: vec![coins] });
        app.update();
        app.init_resource::<Paid>().add_systems(
            crate::plugin::Turn,
            (move |mut done: MessageReader<Interacted>,
                   bags: Query<&Inventory>,
                   pieces: Query<(Option<&Tagged>, Option<&Stack>)>,
                   currency: Res<Currency>,
                   mut paid: ResMut<Paid>| {
                let count = |who: Entity| bags.get(who).map_or(0, |bag| currency.count(bag, |i| pieces.get(i).unwrap_or((None, None))));
                for _ in done.read() {
                    paid.0.push((count(player), count(offerer)));
                }
            })
            .in_set(crate::plugin::TurnSet::React),
        );
        assert!(app.world().get::<Inventory>(offerer).is_none(), "the offerer carries nothing yet");

        let before = app.world().resource::<Turns>().now();
        app.world_mut().write_message(Intent::new(player, Interact { target: offerer, verb: free, thing: None }));
        app.update();
        assert_eq!(app.world().resource::<Turns>().now(), before + 200, "it costs its time");
        assert_eq!(app.world().resource::<Paid>().0, vec![(30, 50)], "the answer ran with the 50 already moved");
        assert_eq!((purse(app.world(), player), purse(app.world(), offerer)), (30, 50), "and nothing was made or lost");
        let refused = app.world().resource::<OfferedHere>().find(player, offerer, free).and_then(|o| o.refused);
        assert_eq!(refused, Some(Refused::Short(50)), "30 left does not meet it a second time");
    }

    /// An offer with no price is listed with none, needs no currency in the
    /// game at all, and taking it up moves nothing the taker carries.
    #[test]
    fn an_offer_with_no_price_needs_no_currency_and_takes_nothing_from_the_taker() {
        let (mut app, start, free, _) = arena();
        let scrip = app.world().resource::<Registries>().tags.expect("scrip");
        let coins = app.world_mut().spawn((Item, Tagged(vec![scrip]), Stack { key: 1, count: 80 })).id();
        let captive = someone(&mut app, start.offset(1, 0), Offers(vec![Offering::new(free, 200)]));
        let player = player_at(&mut app, start, Offers::default());
        app.world_mut().entity_mut(player).insert(Inventory { items: vec![coins] });
        app.update();
        watch(&mut app);
        assert_eq!(offered(&app, player), vec![Offer { target: captive, verb: free, time: 200, price: None, asks: None, refused: None }]);
        app.world_mut().write_message(Intent::new(player, Interact { target: captive, verb: free, thing: None }));
        app.update();
        assert_eq!(app.world().resource::<Seen>().0.len(), 1, "taken up");
        assert_eq!(app.world().get::<Stack>(coins).map(|s| s.count), Some(80), "and the taker's pile is untouched");
        assert!(app.world().get::<Inventory>(captive).is_none(), "nor is anything handed over");
    }

    /// A priced offer in a game with no currency to pay it in is a mistake
    /// in the setup, and stops play saying what is missing.
    #[test]
    #[should_panic(expected = "PropsPlugin needs rl_bevy::trade::Currency")]
    fn a_priced_offer_in_a_game_without_a_currency_stops_play_and_says_what_is_missing() {
        let (mut app, start, free, _) = arena();
        someone(&mut app, start.offset(4, 0), Offers(vec![Offering::new(free, 200).priced(50)]));
        player_at(&mut app, start, Offers::default());
    }

    /// A foe's offers are kept from whoever it is hostile to, in the gate,
    /// so no key can take one up peacefully; someone of no side beside it
    /// still offers.
    #[test]
    fn an_actor_offers_nothing_to_one_it_is_hostile_to_and_reaching_for_it_is_refused() {
        let (mut app, start, free, _) = arena();
        let sides = crate::testing::two_sides(&mut app);
        let foe = someone(&mut app, start.offset(1, 0), Offers(vec![Offering::new(free, 200)]));
        app.world_mut().entity_mut(foe).insert(crate::combat::Faction(sides.theirs));
        let neutral = someone(&mut app, start.offset(-1, 0), Offers(vec![Offering::new(free, 200)]));
        let player = player_at(&mut app, start, Offers::default());
        app.world_mut().entity_mut(player).insert(crate::combat::Faction(sides.ours));
        app.update();
        watch(&mut app);
        assert_eq!(offered(&app, player).iter().map(|o| o.target).collect::<Vec<_>>(), vec![neutral], "the foe offers nothing, the neutral does");

        let before = app.world().resource::<Turns>().now();
        app.world_mut().write_message(Intent::new(player, Interact { target: foe, verb: free, thing: None }));
        app.update();
        assert_eq!(app.world().resource::<Turns>().now(), before, "reaching for a foe's offer costs nothing");
        assert!(app.world().resource::<Seen>().0.is_empty(), "and does nothing");
    }

    /// Who reaches for the captive, once, when it holds the turn.
    #[derive(Resource)]
    struct Reach(Entity, Entity, VerbId);

    fn reach_once(mut commands: Commands, reach: Res<Reach>, holding: Query<(), With<MyTurn>>, mut intents: MessageWriter<Intent<Interact>>) {
        if holding.contains(reach.0) {
            intents.write(Intent::new(reach.0, Interact { target: reach.1, verb: reach.2, thing: None }));
            commands.remove_resource::<Reach>();
        }
    }

    /// What a game whose verb takes several turns does with its message:
    /// it begins work on what was interacted with.
    #[derive(Resource)]
    struct Freeing(rl_rules::work::WorkKindId);

    fn begin_freeing(mut done: MessageReader<Interacted>, freeing: Res<Freeing>, mut works: crate::work::Works) {
        for ev in done.read() {
            works.begin(ev.actor, rl_rules::work::Work::new(freeing.0, 50).on(ev.target));
        }
    }

    #[derive(Resource, Default)]
    struct Broken(Vec<crate::work::WorkBroken>);

    /// An actor's verb that takes several turns takes them by the path a
    /// prop's would: the game answers the interaction by beginning work on
    /// the one interacted with, and a blow on the worker breaks it off.
    #[test]
    fn work_begun_from_an_actors_offer_is_broken_off_by_a_blow_on_the_worker() {
        use crate::work::{AddWork, BreakReason, WorkKinds, Working};
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, PropsPlugin, crate::combat::CombatPlugin));
        app.add_verb("free").add_work("freeing");
        let start = crate::testing::ground(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        let free = app.world().resource::<Verbs>().get("free").expect("declared");
        let kind = app.world().resource::<WorkKinds>().get("freeing").expect("declared");
        let worker = app.world_mut().spawn((Actor, Blocks, Position(start.offset(4, 0)), crate::combat::Health::full(10))).id();
        let captive = someone(&mut app, start.offset(5, 0), Offers(vec![Offering::new(free, 100)]));
        app.insert_resource(Reach(worker, captive, free)).insert_resource(Freeing(kind)).init_resource::<Broken>().add_systems(
            crate::plugin::Turn,
            (
                reach_once.run_if(resource_exists::<Reach>).in_set(crate::plugin::DecideSet::Game),
                begin_freeing.in_set(crate::plugin::TurnSet::React),
                (|mut seen: ResMut<Broken>, mut broken: MessageReader<crate::work::WorkBroken>| seen.0.extend(broken.read().copied()))
                    .in_set(crate::plugin::TurnSet::Cleanup),
            ),
        );
        let player = player_at(&mut app, start, Offers::default());
        for _ in 0..6 {
            if app.world().get::<MyTurn>(player).is_some() {
                app.world_mut().write_message(Intent::new(player, crate::turn::Wait));
            }
            app.update();
        }
        let work = app.world().get::<Working>(worker).map(|w| w.0);
        assert_eq!(work.and_then(|w| w.target), Some(captive), "the worker is at work on the captive");
        assert!(work.is_some_and(|w| w.done >= 2), "and has carried it on past the turn it began");

        app.world_mut().write_message(crate::combat::DamageEvent::new(worker, rl_rules::Hit::from_source(None, sides.kind, 2)));
        app.update();
        let reasons: Vec<BreakReason> = app.world().resource::<Broken>().0.iter().map(|b| b.reason).collect();
        assert_eq!(reasons, vec![BreakReason::Hurt], "a blow broke it off");
        assert!(app.world().get::<Working>(worker).is_none());
    }
}

#[cfg(test)]
mod containers {
    use super::*;
    use crate::components::{Actor, Blocks, Player, Viewshed};
    use crate::items::{Item, Stack};
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use crate::turn::{Intent, Turns};
    use rl_rules::Names;

    /// A crate that shows when it is done, and one that cannot.
    const PROPS: &str = r#"#![enable(implicit_some)]
        [
            (name: "supply crate", glyph: '&', color: (r: 190, g: 165, b: 115), blocks: true,
             container: (contents: [(item: "slug", count: 2), (item: "medkit", count: 1)], opened: (glyph: '"', color: (r: 128, g: 115, b: 90))),
             offers: [(verb: "open", time: 200)]),
            (name: "open bin", glyph: 'u', color: (r: 150, g: 150, b: 150), blocks: true,
             container: (contents: [(item: "slug", count: 1)]),
             offers: [(verb: "open", time: 100)]),
        ]"#;

    struct Chest {
        app: App,
        player: Entity,
        prop: Entity,
    }

    /// A player beside a container of `name`, already stocked with what
    /// the game was asked for.
    fn chest(name: &str) -> Chest {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, PropsPlugin, crate::items::ItemsPlugin));
        let start = crate::testing::ground(&mut app);
        let props = rl_rules::prop::load(PROPS, &Names::new()).expect("the props load");
        app.insert_resource(Registries { props, ..Default::default() });
        app.insert_resource(crate::seed::Seed(crate::testing::TEST_SEED));

        // The game's half, where an answer is due.
        app.add_systems(Update, answer_asks.in_set(PropSet::Fill));

        let registries = app.world().resource::<Registries>().clone();
        let id = registries.props.expect(name);
        let prop = spawn_prop(&mut app.world_mut().commands(), &registries, id, start.offset(1, 0), MapId::ZERO);
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), OnMap(MapId::ZERO), Inventory::default())).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        for _ in 0..3 {
            app.update();
        }
        Chest { app, player, prop }
    }

    /// The game's half: spawns what it was asked for and puts it in.
    fn answer_asks(mut commands: Commands, mut asks: MessageReader<FillContainer>, mut bags: Query<&mut Inventory>) {
        for ask in asks.read() {
            let rl_rules::prop::Stock::Item(name) = &ask.what else { continue };
            let mut items = Vec::new();
            for _ in 0..ask.count {
                let mut item = commands.spawn((Item, Name::new(name.clone())));
                if name == "slug" {
                    item.insert(Stack { key: 1, count: 1 });
                }
                items.push(item.id());
            }
            if let Ok(mut bag) = bags.get_mut(ask.prop) {
                bag.items.extend(items);
            }
        }
    }

    fn bag_of(app: &App, who: Entity) -> Vec<Entity> {
        app.world().get::<Inventory>(who).map(|b| b.items.clone()).unwrap_or_default()
    }

    /// A container that asks for a kind of thing by tag has nothing to
    /// draw it from without a loot table, and is refused when play begins
    /// rather than left empty.
    #[test]
    #[should_panic(expected = "no LootPlugin was added")]
    fn a_container_asking_by_tag_without_loot_is_refused_when_play_begins() {
        let mut app = headless_app();
        app.add_plugins(PropsPlugin);
        let tags = rl_rules::Registry::from_defs(vec![rl_rules::TagDef::new("weapon")]).unwrap();
        let props = rl_rules::prop::load(
            r#"#![enable(implicit_some)] [(name: "locker", glyph: '&', color: (r: 1, g: 2, b: 3), offers: [(verb: "open")], container: (contents: [(tag: "weapon", count: 1)]))]"#,
            &rl_rules::Names::new().tags(&tags),
        )
        .unwrap();
        app.insert_resource(Registries { tags, props, ..Default::default() }).insert_resource(crate::seed::Seed(crate::testing::TEST_SEED));
        crate::testing::ground(&mut app);
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
    }

    /// The engine rolls how many and the game spawns what: neither knows
    /// the other's half.
    #[test]
    fn a_container_is_stocked_by_the_game_from_counts_the_engine_rolled() {
        let it = chest("supply crate");
        let held = bag_of(&it.app, it.prop);
        assert_eq!(held.len(), 3, "two slugs and a medkit, as the definition asked");
        let names: Vec<String> = held.iter().filter_map(|i| it.app.world().get::<Name>(*i).map(|n| n.as_str().to_string())).collect();
        assert_eq!(names.iter().filter(|n| *n == "slug").count(), 2);
        assert_eq!(names.iter().filter(|n| *n == "medkit").count(), 1);
    }

    /// Taking one moves it into the bag and merges stacks the way the
    /// ground does; taking all empties the thing in one turn.
    #[test]
    fn taking_moves_things_into_the_bag_and_taking_all_costs_one_turn() {
        let mut it = chest("supply crate");
        let first = bag_of(&it.app, it.prop)[0];
        it.app.world_mut().write_message(Intent::new(it.player, Take { from: it.prop, item: Some(first) }));
        it.app.update();
        assert_eq!(bag_of(&it.app, it.player), vec![first], "the one taken is carried");
        assert_eq!(bag_of(&it.app, it.prop).len(), 2, "and the rest is still in the crate");

        let before = it.app.world().resource::<Turns>().now();
        it.app.world_mut().write_message(Intent::new(it.player, Take { from: it.prop, item: None }));
        it.app.update();
        assert!(bag_of(&it.app, it.prop).is_empty(), "taking all empties it");
        assert_eq!(it.app.world().resource::<Turns>().now(), before + rl_core::turn::BASE_ACTION_COST, "for one turn");
        // Two slugs went in, and they are one stack of two.
        let carried: Vec<u32> = bag_of(&it.app, it.player).iter().filter_map(|i| it.app.world().get::<Stack>(*i).map(|s| s.count)).collect();
        assert_eq!(carried, vec![2], "the stacks merged, as they do off the ground");
    }

    /// A worn thing taken out of a body comes off it: what a body wears
    /// is what it still carries, so nothing looted is still worn by what
    /// it was looted from, and nothing brought back to life wears it.
    #[test]
    fn a_worn_thing_taken_from_a_container_is_no_longer_worn_by_it() {
        let mut it = chest("supply crate");
        let first = bag_of(&it.app, it.prop)[0];
        let mut worn = rl_rules::Equipment::with_slot_count(1);
        worn.equip(first, &rl_rules::EquipShape::in_slot(rl_rules::SlotId::from_raw(0))).expect("one empty slot");
        it.app.world_mut().entity_mut(it.prop).insert(crate::items::Equipped(worn));
        it.app.world_mut().write_message(Intent::new(it.player, Take { from: it.prop, item: Some(first) }));
        it.app.update();
        assert_eq!(bag_of(&it.app, it.player), vec![first], "taken");
        let still = it.app.world().get::<crate::items::Equipped>(it.prop).expect("the container still has its slots");
        assert!(!still.contains(first), "and no longer worn by what it was taken from");
    }

    /// A crate that can show it is done stops offering; one that cannot
    /// keeps offering, so the player is never refused by something that
    /// looks exactly like a full crate.
    #[test]
    fn an_emptied_container_stops_offering_only_when_it_can_show_it_is_done() {
        let mut it = chest("supply crate");
        it.app.world_mut().write_message(Intent::new(it.player, Take { from: it.prop, item: None }));
        it.app.update();
        it.app.update();
        assert!(it.app.world().get::<Emptied>(it.prop).is_some(), "it is done, and says so");
        assert!(it.app.world().resource::<OfferedHere>().find(it.player, it.prop, Verbs::OPEN).is_none(), "and offers nothing more");

        let mut bin = chest("open bin");
        bin.app.world_mut().write_message(Intent::new(bin.player, Take { from: bin.prop, item: None }));
        bin.app.update();
        bin.app.update();
        assert!(bin.app.world().get::<Emptied>(bin.prop).is_none(), "a bin that cannot show it is done is not marked");
        assert!(
            bin.app.world().resource::<OfferedHere>().find(bin.player, bin.prop, Verbs::OPEN).is_some(),
            "and still offers, for the screen to say it is empty"
        );
    }

    /// The container a turn's reaction put down, as a prefab's slot does.
    #[derive(Resource, Default)]
    struct Raised(Option<Entity>);

    /// Puts a crate down once, mid-turn, the way `PrefabSet::Fill` stands
    /// up a slot's container on the arrival that built the place.
    fn raise_a_crate(mut commands: Commands, registries: Res<Registries>, mut raised: ResMut<Raised>, start: Query<&Position, With<Player>>) {
        if raised.0.is_some() {
            return;
        }
        let Ok(at) = start.single() else { return };
        let id = registries.props.expect("supply crate");
        raised.0 = Some(spawn_prop(&mut commands, &registries, id, at.0.offset(0, 1), MapId::ZERO));
    }

    /// A container put down mid-turn holds nothing until the next frame
    /// stocks it, and the emptied check in the same pass must not read
    /// that bag, which was never filled, as one just emptied. Pinned in
    /// the order that once lost a vault's prize: the reaction that puts it
    /// down runs, and its commands land, before the check.
    #[test]
    fn a_container_put_down_mid_turn_is_never_marked_emptied_before_its_stock_lands() {
        let mut it = chest("supply crate");
        it.app.init_resource::<Raised>();
        it.app.add_systems(crate::plugin::Turn, raise_a_crate.in_set(crate::plugin::TurnSet::React).before(close_emptied_containers));
        for _ in 0..3 {
            it.app.update();
        }
        let raised = it.app.world().resource::<Raised>().0.expect("a crate was put down");
        assert_eq!(bag_of(&it.app, raised).len(), 3, "its stock landed");
        assert!(it.app.world().get::<Emptied>(raised).is_none(), "and it was never marked emptied");
    }

    /// Nothing is spent on what the screen already knows is impossible.
    #[test]
    fn taking_from_out_of_reach_or_from_nothing_costs_no_turn() {
        let mut it = chest("supply crate");
        let elsewhere = it.app.world_mut().spawn((Prop, Container, Inventory::default(), Position(rl_core::Point::new(0, 0)))).id();
        let before = it.app.world().resource::<Turns>().now();
        it.app.world_mut().write_message(Intent::new(it.player, Take { from: elsewhere, item: None }));
        it.app.update();
        assert_eq!(it.app.world().resource::<Turns>().now(), before, "out of reach costs nothing");
    }
}

#[cfg(test)]
mod traps {
    use super::*;
    use crate::combat::{CombatPlugin, DamageEvent, Health};
    use crate::components::{Actor, Blocks, Player, Viewshed};
    use crate::effects::AddEngineEffects;
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use crate::turn::{Intent, Step};

    /// A plate that harms whoever steps on it, once; a leak that harms
    /// every time; a hidden plate; and a barrel that bursts when broken.
    const PROPS: &str = r#"#![enable(implicit_some)]
        [
            (name: "pressure plate", glyph: '^', color: (r: 230, g: 140, b: 51),
             triggers: [(on: "entered", fires: 1, effects: [(kind: "Harm", args: (kind: "kinetic", roll: "3"))])]),
            (name: "coolant leak", glyph: '~', color: (r: 150, g: 200, b: 210),
             triggers: [(on: "entered", fires: 3, effects: [(kind: "Harm", args: (kind: "kinetic", roll: "1"))])]),
            (name: "hidden plate", glyph: '^', color: (r: 230, g: 140, b: 51), hidden: (spot: 100),
             triggers: [(on: "entered", fires: 1, effects: [(kind: "Harm", args: (kind: "kinetic", roll: "2"))])]),
            (name: "fuel barrel", glyph: '0', color: (r: 200, g: 120, b: 60), blocks: true, health: 3,
             triggers: [(on: "destroyed", area: Burst(radius: 1), effects: [(kind: "Harm", args: (kind: "kinetic", roll: "5"))])]),
            (name: "silent plate", glyph: '^', color: (r: 100, g: 100, b: 100),
             triggers: [(on: "entered", fires: 1, effects: [(kind: "Nonesuch", args: ())])]),
        ]"#;

    struct Deck {
        app: App,
        player: Entity,
        at: Point,
    }

    fn deck() -> Deck {
        let mut app = headless_app();
        // No `AbilitiesPlugin`: a game may have traps and no abilities at
        // all, and this proves it. The effects a trap lands are registered
        // on their own, and props ask for the stream they roll from.
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, PropsPlugin));
        app.add_engine_effects();
        let at = crate::testing::ground(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        let props = {
            let registries = app.world().resource::<Registries>().clone();
            rl_rules::prop::load(PROPS, &registries.names()).expect("the props load")
        };
        app.world_mut().resource_mut::<Registries>().props = props;
        let player = app
            .world_mut()
            .spawn((Actor, Player, Blocks, Position(at), Viewshed::new(8), OnMap(MapId::ZERO), Health::full(20), crate::combat::Faction(sides.ours)))
            .id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        Deck { app, player, at }
    }

    fn put(deck: &mut Deck, name: &str, at: Point) -> Entity {
        let registries = deck.app.world().resource::<Registries>().clone();
        let id = registries.props.expect(name);
        let prop = spawn_prop(&mut deck.app.world_mut().commands(), &registries, id, at, MapId::ZERO);
        deck.app.update();
        prop
    }

    fn walk(deck: &mut Deck, dir: rl_core::Direction) {
        let player = deck.player;
        deck.app.world_mut().write_message(Intent::new(player, Step(dir)));
        deck.app.update();
    }

    fn health(deck: &Deck) -> i32 {
        deck.app.world().get::<Health>(deck.player).map(|h| h.current).unwrap_or_default()
    }

    /// How many more times `prop`'s first trigger may go off.
    fn fires_left(deck: &Deck, prop: Entity) -> Option<u32> {
        deck.app.world().get::<crate::effects::Triggers>(prop).and_then(|t| t.0.first()).and_then(|t| t.fires)
    }

    /// Every moment reported, copied out as it is written.
    #[derive(Resource, Default)]
    struct Reports(Vec<crate::effects::Fired>);

    fn keep(mut fired: MessageReader<crate::effects::Fired>, mut reports: ResMut<Reports>) {
        reports.0.extend(fired.read().copied());
    }

    fn breaks(deck: &mut Deck, prop: Entity) {
        let kind = deck.app.world().resource::<Registries>().damage_kinds.expect("kinetic");
        deck.app.world_mut().write_message(DamageEvent::new(prop, rl_rules::Hit::from_source(None, kind, 99)));
    }

    /// A trap is data: the definition names an effect the engine already
    /// has, and nothing in Rust knows what a pressure plate is.
    #[test]
    fn a_plate_written_in_ron_harms_whoever_steps_on_it_and_fires_as_often_as_it_says() {
        let mut deck = deck();
        let plate = {
            let at = deck.at.offset(1, 0);
            put(&mut deck, "pressure plate", at)
        };
        walk(&mut deck, rl_core::Direction::East);
        assert_eq!(health(&deck), 17, "the plate landed its effect on the one who stepped on it");
        assert_eq!(fires_left(&deck, plate), Some(0), "and has none left");

        // Off and back on: a plate that fires once does not fire again.
        walk(&mut deck, rl_core::Direction::West);
        walk(&mut deck, rl_core::Direction::East);
        assert_eq!(health(&deck), 17, "a sprung plate is spent");
    }

    /// `fires` is a count, not a flag: a leak keeps leaking until it has
    /// run out of times.
    #[test]
    fn a_trigger_that_fires_three_times_fires_three_times_and_no_more() {
        let mut deck = deck();
        let leak = {
            let at = deck.at.offset(1, 0);
            put(&mut deck, "coolant leak", at)
        };
        for _ in 0..4 {
            walk(&mut deck, rl_core::Direction::East);
            walk(&mut deck, rl_core::Direction::West);
        }
        assert_eq!(fires_left(&deck, leak), Some(0), "three times, however often it was walked over");
        assert_eq!(health(&deck), 17, "and three points of harm");
    }

    /// Not seeing it is the point.
    #[test]
    fn a_hidden_plate_springs_on_someone_who_never_saw_it() {
        let mut deck = deck();
        // Out of sight of the roll until it is stepped on: the roll needs the
        // player's viewshed, and a plate two cells off is spotted at once at
        // a hundred percent, so this one is put down and walked onto in the
        // same pass.
        let plate = {
            let at = deck.at.offset(1, 0);
            put(&mut deck, "hidden plate", at)
        };
        deck.app.world_mut().entity_mut(plate).insert(Hidden { spot: 0 });
        walk(&mut deck, rl_core::Direction::East);
        assert_eq!(health(&deck), 18, "it sprang, unseen");
        assert!(deck.app.world().get::<Hidden>(plate).is_some(), "and is still unspotted: springing is not seeing");
    }

    /// A prop in sight is rolled for once a turn: a certainty is found on
    /// the first turn the player is dealt with it in sight, and not on the
    /// frames the player spends deciding, and a prop nobody can spot stays
    /// hidden however long it is looked at.
    #[test]
    fn a_hidden_prop_in_sight_is_spotted_by_a_roll_and_never_without_one() {
        let mut deck = deck();
        // Spotted at a hundred percent, but only once the player's next
        // turn comes round: the frames before it are no turns at all.
        let certain = {
            let at = deck.at.offset(2, 0);
            put(&mut deck, "hidden plate", at)
        };
        for _ in 0..3 {
            deck.app.update();
        }
        assert!(deck.app.world().get::<Hidden>(certain).is_some(), "three frames of the player deciding roll nothing");
        let player = deck.player;
        deck.app.world_mut().write_message(Intent::new(player, crate::turn::Wait));
        deck.app.update();
        assert!(deck.app.world().get::<Hidden>(certain).is_none(), "spotted, and from then on an ordinary prop");

        // The same plate, with no chance of being spotted at all.
        let never = {
            let at = deck.at.offset(3, 0);
            put(&mut deck, "hidden plate", at)
        };
        deck.app.world_mut().entity_mut(never).insert(Hidden { spot: 0 });
        for _ in 0..5 {
            deck.app.world_mut().write_message(Intent::new(player, crate::turn::Wait));
            deck.app.update();
        }
        assert!(deck.app.world().get::<Hidden>(never).is_some(), "five turns of looking at it and still unseen");
    }

    /// Keenness is the finder's, and a hidden prop's roll takes it as a
    /// secret's does: a plate nobody could spot is spotted by the keen.
    #[test]
    fn a_keen_finder_spots_a_prop_nobody_else_could() {
        let mut deck = deck();
        let plate = {
            let at = deck.at.offset(3, 0);
            put(&mut deck, "hidden plate", at)
        };
        deck.app.world_mut().entity_mut(plate).insert(Hidden { spot: 0 });
        let player = deck.player;
        deck.app.world_mut().entity_mut(player).insert(Keen(100));
        deck.app.world_mut().write_message(Intent::new(player, crate::turn::Wait));
        deck.app.update();
        assert!(deck.app.world().get::<Hidden>(plate).is_none(), "spotted at what the finder adds");
    }

    /// A plate harms in the pass the step was taken: the step, the report,
    /// the landing and the damage are all one pass.
    #[test]
    fn a_plate_harms_whoever_steps_on_it_in_the_pass_they_stepped() {
        let mut deck = deck();
        {
            let at = deck.at.offset(1, 0);
            put(&mut deck, "pressure plate", at);
        }
        let player = deck.player;
        deck.app.world_mut().write_message(Intent::new(player, Step(rl_core::Direction::East)));
        deck.app.world_mut().run_schedule(crate::plugin::Turn);
        assert_eq!(health(&deck), 17, "harmed in the one pass");
    }

    /// A prop with health dies like anything else, and what it does when
    /// it dies is one more trigger, which bursts on whoever stands in its
    /// radius, the player beside it included.
    #[test]
    fn a_barrel_bursts_on_everyone_in_its_radius_when_it_is_broken() {
        let mut deck = deck();
        let barrel = {
            let at = deck.at.offset(1, 0);
            put(&mut deck, "fuel barrel", at)
        };
        breaks(&mut deck, barrel);
        deck.app.update();
        deck.app.update();
        assert!(deck.app.world().get_entity(barrel).is_err(), "the barrel is gone");
        assert_eq!(health(&deck), 15, "and its burst caught the player beside it");
    }

    /// A barrel that bursts into another sets it off once, a pass later,
    /// and never inside its own pass.
    #[test]
    fn a_chain_of_barrels_goes_off_one_after_another_and_each_once() {
        let mut deck = deck();
        let first = {
            let at = deck.at.offset(3, 0);
            put(&mut deck, "fuel barrel", at)
        };
        let second = {
            let at = deck.at.offset(4, 0);
            put(&mut deck, "fuel barrel", at)
        };
        deck.app.init_resource::<Reports>().add_systems(bevy::app::PostUpdate, keep);
        breaks(&mut deck, first);
        let player = deck.player;
        for _ in 0..4 {
            deck.app.world_mut().write_message(Intent::new(player, crate::turn::Wait));
            deck.app.update();
        }
        let bursts = deck.app.world().resource::<Reports>().0.iter().filter(|f| f.moment == crate::effects::Moments::DESTROYED).count();
        assert_eq!(bursts, 2, "each barrel went off once");
        assert!(deck.app.world().get_entity(second).is_err(), "the second was broken by the first");
        assert_eq!(health(&deck), 20, "and the player, three cells off, was never in either burst");
    }

    /// Every hit written, copied out as it is written.
    #[derive(Resource, Default)]
    struct Hits(Vec<DamageEvent>);

    fn keep_hits(mut damage: MessageReader<DamageEvent>, mut hits: ResMut<Hits>) {
        hits.0.extend(damage.read().cloned());
    }

    /// A trap strikes as itself. Whoever stepped on it set it off, and is
    /// named on the report, but the harm is the plate's: credited to the
    /// one who stepped on it, the log would read that you hurt yourself,
    /// and a droid a plate killed would have killed itself.
    #[test]
    fn a_plate_strikes_as_itself_and_not_as_whoever_stepped_on_it() {
        let mut deck = deck();
        let plate = {
            let at = deck.at.offset(1, 0);
            put(&mut deck, "pressure plate", at)
        };
        deck.app.init_resource::<Hits>().add_systems(bevy::app::PostUpdate, keep_hits);
        deck.app.init_resource::<Reports>().add_systems(bevy::app::PostUpdate, keep);
        walk(&mut deck, rl_core::Direction::East);
        let hits = &deck.app.world().resource::<Hits>().0;
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!((hits[0].hit.attacker, hits[0].hit.credit), (Some(plate), Some(plate)), "the plate struck, and is credited");
        let stepped = deck.app.world().resource::<Reports>().0.iter().find(|f| f.moment == crate::effects::Moments::ENTERED).map(|f| f.by);
        assert_eq!(stepped, Some(Some(deck.player)), "and the report still says who stepped on it");
    }

    /// A swap is no step, but both who change places come to stand on a
    /// new cell: a plate under each is entered, the one by whoever was
    /// swapped onto it, so a passer-by a neutral gave way to cannot walk
    /// over a plate the neutral stood on without setting it off.
    #[test]
    fn a_swap_sets_off_the_plates_both_who_changed_places_are_put_onto() {
        let mut deck = deck();
        let (mine, theirs) = (deck.at, deck.at.offset(1, 0));
        let under_me = put(&mut deck, "pressure plate", mine);
        let under_them = put(&mut deck, "pressure plate", theirs);
        let us = deck.app.world().get::<crate::combat::Faction>(deck.player).copied().expect("a side");
        let other = deck.app.world_mut().spawn((Actor, Blocks, Position(theirs), OnMap(MapId::ZERO), Health::full(20), us)).id();
        deck.app.init_resource::<Reports>().add_systems(bevy::app::PostUpdate, keep);
        for _ in 0..8 {
            if deck.app.world().get::<Position>(deck.player).map(|p| p.0) == Some(theirs) {
                break;
            }
            if deck.app.world().get::<crate::components::MyTurn>(deck.player).is_some() {
                let player = deck.player;
                deck.app.world_mut().write_message(Intent::new(player, crate::bump::Swap(other)));
            }
            deck.app.update();
        }
        assert_eq!(deck.app.world().get::<Position>(other).map(|p| p.0), Some(mine), "the two changed places");
        let entered: Vec<(Entity, Option<Entity>)> =
            deck.app.world().resource::<Reports>().0.iter().filter(|f| f.moment == crate::effects::Moments::ENTERED).map(|f| (f.on, f.by)).collect();
        assert!(entered.contains(&(under_them, Some(deck.player))), "the one who swapped entered the plate it was put onto: {entered:?}");
        assert!(entered.contains(&(under_me, Some(other))), "and the one swapped with entered the other: {entered:?}");
    }

    /// A barrel broken just before the loop stops to show something still
    /// bursts when it runs again. What it does when broken lands the pass
    /// after, and a message would not outlive a long enough hold; the
    /// remnant that carries it does.
    #[test]
    fn a_barrel_broken_before_the_loop_is_held_still_bursts_when_it_runs_again() {
        let mut deck = deck();
        let barrel = {
            let at = deck.at.offset(1, 0);
            put(&mut deck, "fuel barrel", at)
        };
        breaks(&mut deck, barrel);
        deck.app.update();
        assert_eq!(health(&deck), 20, "broken, and not yet burst");
        {
            let mut hold = deck.app.world_mut().resource_mut::<crate::cue::TurnHold>();
            hold.watch();
            hold.hold();
        }
        for _ in 0..4 {
            deck.app.update();
            // What a windowed frame's message update does, which the
            // headless app does not: a message nobody reads for two frames
            // is gone.
            deck.app.world_mut().resource_mut::<Messages<crate::effects::Fired>>().update();
        }
        assert_eq!(health(&deck), 20, "nothing moves while the loop is held");
        deck.app.world_mut().resource_mut::<crate::cue::TurnHold>().release();
        deck.app.update();
        assert_eq!(health(&deck), 15, "and the burst lands as soon as it runs");
        let remnants = deck.app.world_mut().query::<&crate::effects::Remnant>().iter(deck.app.world()).count();
        assert_eq!(remnants, 0, "and what carried it is gone");
    }

    /// Adding the effects plugin by hand, after a plugin that already
    /// added it, is harmless: the order a game lists its plugins in never
    /// matters, and the trap lands once.
    #[test]
    fn effects_added_by_hand_after_props_is_harmless_and_a_trap_lands_once() {
        let mut deck = {
            let mut app = headless_app();
            app.add_plugins((crate::fov::FovPlugin, CombatPlugin, PropsPlugin, crate::effects::EffectsPlugin));
            app.add_engine_effects();
            let at = crate::testing::ground(&mut app);
            let sides = crate::testing::two_sides(&mut app);
            let props = {
                let registries = app.world().resource::<Registries>().clone();
                rl_rules::prop::load(PROPS, &registries.names()).expect("the props load")
            };
            app.world_mut().resource_mut::<Registries>().props = props;
            let player = app
                .world_mut()
                .spawn((Actor, Player, Blocks, Position(at), Viewshed::new(8), OnMap(MapId::ZERO), Health::full(20), crate::combat::Faction(sides.ours)))
                .id();
            app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
            app.update();
            app.update();
            Deck { app, player, at }
        };
        {
            let at = deck.at.offset(1, 0);
            put(&mut deck, "pressure plate", at);
        }
        walk(&mut deck, rl_core::Direction::East);
        assert_eq!(health(&deck), 17, "harmed once, not once per copy of the plugin");
    }

    /// A trap naming an effect nobody registered would otherwise be a trap
    /// that silently does nothing, which is the worst kind.
    #[test]
    fn a_trap_naming_an_effect_nobody_registered_is_reported_and_lands_nothing() {
        let mut deck = deck();
        let plate = {
            let at = deck.at.offset(1, 0);
            put(&mut deck, "silent plate", at)
        };
        walk(&mut deck, rl_core::Direction::East);
        assert_eq!(health(&deck), 20, "nothing landed");
        assert!(deck.app.world().get::<crate::effects::Triggers>(plate).is_none_or(|t| t.0.is_empty()), "a trigger that would not build arms nothing");
    }
}

#[cfg(test)]
mod spotting {
    use rl_core::{Direction, RunSeed};
    use rl_grid::{Terrain, TileRegistry};

    use super::*;
    use crate::components::{Actor, Player, Viewshed};
    use crate::places::PlaceBuild;
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use crate::turn::{Intent, Step, Wait};

    const PLACE: MapId = MapId(1);
    /// Where the player stands: the west end of the hall.
    const START: Point = Point::new(1, 3);
    /// A cell past the wall across the hall, which the player never sees.
    const BEYOND: Point = Point::new(20, 3);

    struct Rig {
        app: App,
        player: Entity,
        floor: rl_grid::TileId,
    }

    /// A hall walled round with a wall across it at x = 17, the player at
    /// its west end, and each `(cell, spot, group)` walled over and hidden
    /// as floor.
    fn rig(seed: u64, rules: SpotRules, hide: &[(Point, u8, u32)]) -> Rig {
        let tiles = TileRegistry::standard();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        let mut terrain = Terrain::filled(24, 7, wall);
        for x in 1..23 {
            for y in 1..6 {
                terrain.set(Point::new(x, y), floor);
            }
        }
        for y in 0..7 {
            terrain.set(Point::new(17, y), wall);
        }
        let secrets = hide
            .iter()
            .map(|&(at, spot, group)| {
                terrain.set(at, wall);
                rl_mapgen::Secret { at, is: floor, spot, group, joins: (0, 1) }
            })
            .collect();
        let mut map = WorldMap::new(tiles.tables());
        map.install_place(
            PLACE,
            PlaceBuild { terrain, entry: START, exit: None, spots: Vec::new(), locks: Vec::new(), secrets, ambient: rl_grid::Light::DARK, peaceful: false },
        );
        map.switch_to(PLACE);
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::combat::CombatPlugin, PropsPlugin::spotting(rules)));
        crate::testing::two_sides(&mut app);
        app.insert_resource(map).insert_resource(crate::seed::Seed(RunSeed(seed)));
        let player = app.world_mut().spawn((Actor, Player, crate::components::Blocks, Position(START), OnMap(PLACE), Viewshed::new(30))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        Rig { app, player, floor }
    }

    impl Rig {
        fn wait(&mut self) {
            let player = self.player;
            self.app.world_mut().write_message(Intent::new(player, Wait));
            self.app.update();
        }

        fn step(&mut self, dir: Direction) {
            let player = self.player;
            self.app.world_mut().write_message(Intent::new(player, Step(dir)));
            self.app.update();
        }

        fn found(&self, at: Point) -> bool {
            self.app.world().resource::<WorldMap>().tile(at) == Some(self.floor)
        }

        /// How many waited turns until `at` is found, up to `most`.
        fn turns_to_find(&mut self, at: Point, most: u32) -> Option<u32> {
            (1..=most).find(|_| {
                self.wait();
                self.found(at)
            })
        }
    }

    /// How many of `seeds` find `at`, hidden at `spot`, on the first turn.
    fn found_at_once(seeds: std::ops::Range<u64>, rules: SpotRules, at: Point, spot: u8) -> usize {
        seeds
            .filter(|&seed| {
                let mut rig = rig(seed, rules, &[(at, spot, 0)]);
                rig.wait();
                rig.found(at)
            })
            .count()
    }

    #[test]
    fn the_chance_is_whole_near_half_out_to_far_and_none_beyond_with_keen_added_and_stillness_multiplying() {
        let rules = SpotRules { near: 2, far: 7, still: 3 };
        assert_eq!(rules.chance(6, 0, 2, false), Some((6, 100)), "whole within near");
        assert_eq!(rules.chance(5, 0, 3, false), Some((5, 200)), "half past it, two and a half and not two");
        assert_eq!(rules.chance(6, 0, 7, false), Some((6, 200)), "half out to far");
        assert_eq!(rules.chance(100, 50, 8, true), None, "and none beyond, however keen and still");
        assert_eq!(rules.chance(6, 15, 1, false), Some((21, 100)), "keenness adds");
        assert_eq!(rules.chance(0, 15, 1, false), Some((15, 100)), "even to nought");
        assert_eq!(rules.chance(0, 0, 1, true), None, "and nought with none is never");
        assert_eq!(rules.chance(90, 30, 5, false), Some((100, 200)), "held to certainty before it is halved");
        assert_eq!(rules.chance(6, 0, 1, true), Some((18, 100)), "a waited turn triples");
        assert_eq!(rules.chance(50, 0, 1, true), Some((100, 100)), "to certainty and no further");
        assert_eq!(SpotRules::default().chance(6, 0, 900, true), Some((6, 100)), "and the default weighs neither");
    }

    #[test]
    fn a_secret_in_sight_is_found_at_its_chance_and_becomes_its_tile_and_one_out_of_sight_never() {
        let pillar = Point::new(6, 3);
        let mut rig = rig(1, SpotRules::default(), &[(pillar, 100, 0), (BEYOND, 100, 1)]);
        for _ in 0..3 {
            rig.app.update();
        }
        assert!(!rig.found(pillar), "frames spent deciding are no turns");
        rig.wait();
        assert!(rig.found(pillar), "a certainty in sight is found on the turn and is the floor it was");
        assert_eq!(rig.app.world().resource::<WorldMap>().secrets(PLACE).len(), 1);
        for _ in 0..5 {
            rig.wait();
        }
        assert!(!rig.found(BEYOND), "a certainty out of sight is never found");

        // At a half: found on the first turn on about half the seeds.
        let found = found_at_once(0..60, SpotRules::default(), pillar, 50);
        assert!((15..=45).contains(&found), "{found} of 60 at a half");
    }

    #[test]
    fn past_near_a_secret_is_found_at_half_its_chance_and_past_far_never() {
        let rules = SpotRules { near: 2, far: 7, still: 1 };
        assert_eq!(found_at_once(0..10, rules, START.offset(2, 0), 100), 10, "within near, a certainty is certain");
        let half = found_at_once(0..60, rules, START.offset(5, 0), 100);
        assert!((15..=45).contains(&half), "{half} of 60 past near");
        let mut far = rig(0, rules, &[(START.offset(8, 0), 100, 0)]);
        assert_eq!(far.turns_to_find(START.offset(8, 0), 10), None, "past far, ten turns of certainty find nothing");
    }

    #[test]
    fn a_finder_s_keenness_adds_to_every_roll() {
        let pillar = Point::new(6, 3);
        let mut dull = rig(0, SpotRules::default(), &[(pillar, 0, 0)]);
        assert_eq!(dull.turns_to_find(pillar, 10), None, "a nought is never found");
        let mut keen = rig(0, SpotRules::default(), &[(pillar, 0, 0)]);
        let player = keen.player;
        keen.app.world_mut().entity_mut(player).insert(Keen(100));
        assert_eq!(keen.turns_to_find(pillar, 1), Some(1), "but by the keen, at what they add");
    }

    #[test]
    fn a_waited_turn_multiplies_the_chance_and_a_moved_turn_does_not() {
        let rules = SpotRules { still: 100, ..SpotRules::default() };
        let pillar = Point::new(8, 3);
        assert_eq!(found_at_once(0..20, rules, pillar, 1), 20, "a one in a hundred, waited on a hundredfold, is certain");
        let moved = (0..20)
            .filter(|&seed| {
                let mut rig = rig(seed, rules, &[(pillar, 1, 0)]);
                rig.step(Direction::East);
                rig.found(pillar)
            })
            .count();
        assert!(moved <= 3, "walked past, it is one in a hundred: {moved} of 20");
    }

    #[test]
    fn one_cell_of_a_group_found_finds_it_all_and_a_wide_mouth_is_found_no_faster_than_a_door() {
        let rules = SpotRules { near: 2, far: 7, still: 3 };
        let mouth = [Point::new(5, 2), Point::new(5, 3), Point::new(5, 4)];
        for seed in 0..20 {
            let mut door = rig(seed, rules, &[(mouth[1], 20, 0)]);
            let mut wide = rig(seed, rules, &mouth.map(|at| (at, 20, 0)));
            let (one, three) = (door.turns_to_find(mouth[1], 40), wide.turns_to_find(mouth[1], 40));
            assert!(one.is_some(), "seed {seed}: found within forty turns");
            assert_eq!(one, three, "seed {seed}: one roll for the group, at its nearest cell");
            assert!(mouth.iter().all(|&at| wide.found(at)), "seed {seed}: and every cell of it found together");
        }
    }

    #[derive(Resource, Default)]
    struct Heard(Vec<Spotted>);

    #[test]
    fn a_secret_s_group_is_spotted_in_one_message_naming_its_nearest_cell_and_what_it_is() {
        let mouth = [Point::new(5, 2), Point::new(5, 3), Point::new(5, 4)];
        let mut rig = rig(0, SpotRules::default(), &mouth.map(|at| (at, 100, 4)));
        rig.app.init_resource::<Heard>().add_systems(PostUpdate, |mut heard: ResMut<Heard>, mut spotted: MessageReader<Spotted>| {
            heard.0.extend(spotted.read().copied());
        });
        rig.wait();
        let floor = rig.floor;
        let heard = &rig.app.world().resource::<Heard>().0;
        assert_eq!(heard, &[Spotted { what: Finding::Secret { group: 4, is: floor }, by: rig.player, at: mouth[1] }]);
    }

    #[test]
    fn props_and_secrets_draw_from_one_stream_so_a_secret_moves_no_later_blow() {
        use rand::RngCore;
        let draws = |hide: &[(Point, u8, u32)]| {
            let mut rig = rig(7, SpotRules::default(), hide);
            for _ in 0..5 {
                rig.wait();
            }
            let world = rig.app.world_mut();
            (world.resource_mut::<PropRng>().0.next_u64(), world.resource_mut::<crate::combat::CombatRng>().0.next_u64())
        };
        let (bare_props, bare_blows) = draws(&[]);
        let (props, blows) = draws(&[(Point::new(6, 3), 1, 0)]);
        assert_ne!(props, bare_props, "the secret was rolled for from props' stream");
        assert_eq!(blows, bare_blows, "and the next blow is the one it would have been");
    }

    /// A group is weighed by its nearest cell the finder sees: the end of
    /// a mouth round a corner, nearer but unseen, is never the one named.
    #[test]
    fn a_group_is_measured_from_its_nearest_cell_in_sight_and_never_one_out_of_it() {
        let secret = |x: i32, group: u32| crate::secrets::Secret { at: Point::new(x, 3), is: rl_grid::TileId(0), shows: rl_grid::TileId(1), spot: 5, group };
        let mut sight = Viewshed::new(20);
        sight.visible = rl_grid::BitGrid::new(20, 7);
        for x in [6, 9] {
            sight.visible.insert(Point::new(x, 3));
        }
        let near_unseen = [secret(3, 0), secret(6, 0), secret(12, 1), secret(9, 1)];
        let found = groups_in_sight(&near_unseen, &sight, START);
        assert_eq!(found.iter().map(|s| (s.group, s.at.x)).collect::<Vec<_>>(), [(0, 6), (1, 9)]);
        assert!(groups_in_sight(&[secret(3, 2)], &sight, START).is_empty(), "a group with no cell in sight is not rolled for");
    }

    #[test]
    #[should_panic(expected = "less than near")]
    #[cfg(debug_assertions)]
    fn spot_rules_whose_far_is_nearer_than_near_are_refused() {
        SpotRules { near: 7, far: 2, still: 1 }.chance(10, 0, 1, false);
    }
}
