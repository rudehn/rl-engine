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
//! Offers and the interactions that answer them, containers that stock and
//! empty, triggers that land an effect list, and hiding that a turn in
//! sight rolls away are all here. `docs/design/props.md` is the whole
//! design.

use bevy::prelude::*;
use rl_core::{Id, Interner, Point, geometry};
use rl_rules::TagId;
use rl_rules::prop::PropId;

use crate::combat::Health;
use crate::components::{Blocks, MyTurn, Position};
use crate::effects::{Effects, Fired, LandsAsItself, Moments, Remnant, Triggers};
use crate::items::{Inventory, Tagged};
use crate::places::{MapId, OnMap};
use crate::registries::Registries;
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
    /// Go through what is left of something.
    pub const SEARCH: VerbId = VerbId::from_raw(1);

    /// The engine's verbs, in the order their ids are handed out.
    pub const BUILT_IN: [&'static str; 2] = [rl_rules::prop::OPEN, rl_rules::prop::SEARCH];

    /// The id for `name`, assigning a new one if it is unseen.
    pub fn declare(&mut self, name: &str) -> VerbId {
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

/// One thing the actor holding the turn could do with a prop it can
/// reach, as the gate works it out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Offer {
    /// The prop offering it.
    pub prop: Entity,
    /// What it is called.
    pub verb: VerbId,
    /// What the turn would cost.
    pub time: u32,
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

    /// Whether `actor` is offered `verb` by `prop`, and at what cost.
    pub fn find(&self, actor: Entity, prop: Entity, verb: VerbId) -> Option<&Offer> {
        self.for_actor(actor).iter().find(|o| o.prop == prop && o.verb == verb)
    }
}

/// Whoever holds the turn, and what it carries.
type Reacher<'w, 's> = Query<'w, 's, (Entity, &'static Position, Option<&'static OnMap>, Option<&'static Inventory>), With<MyTurn>>;

/// A prop as the gate reads it.
type Offering<'w, 's> = Query<'w, 's, (Entity, &'static PropKind, &'static Position, Option<&'static OnMap>), With<Prop>>;

/// Works out what the actor holding the turn is offered by what it can
/// reach: what it stands on, and what stands beside it.
///
/// In [`DecideSet::Offer`](crate::plugin::DecideSet::Offer), beside the
/// ability gate and for the same reason: the player's screen and a mind's
/// tactic both read one answer, worked out once, rather than each walking
/// the world themselves.
pub fn offer_here(
    mut offered: ResMut<OfferedHere>,
    registries: Res<Registries>,
    verbs: Res<Verbs>,
    reacher: Reacher,
    props: Offering,
    tagged: Query<&Tagged>,
    emptied: Query<(), With<Emptied>>,
) {
    offered.actor = None;
    offered.offers.clear();
    let Ok((actor, pos, on, bag)) = reacher.single() else { return };
    offered.actor = Some(actor);
    let here = on.map(|m| m.0).unwrap_or(MapId::SURFACE);
    for (prop, kind, at, prop_on) in &props {
        if prop_on.map(|m| m.0).unwrap_or(MapId::SURFACE) != here {
            continue;
        }
        // What it stands on and what stands beside it: a body underfoot is
        // as reachable as a crate in the next cell.
        if at.0 != pos.0 && !geometry::is_adjacent(pos.0, at.0) {
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
            let wants = offer.needs.or(lock.filter(|_| verb == Verbs::OPEN));
            let refused = wants.filter(|tag| !carries(bag, &tagged, *tag)).map(Refused::Needs);
            offered.offers.push(Offer { prop, verb, time: offer.time, refused });
        }
    }
}

/// Whether the actor carries anything tagged `tag`.
fn carries(bag: Option<&Inventory>, tagged: &Query<&Tagged>, tag: TagId) -> bool {
    bag.is_some_and(|bag| bag.items.iter().any(|item| tagged.get(*item).is_ok_and(|t| t.0.contains(&tag))))
}

/// Do what a prop offers: the one action, whatever the verb.
///
/// Refused for free when the offer is not there to take: the gate worked
/// out what was possible before the key was pressed, so nothing is
/// learned by spending a turn finding out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interact {
    /// What to interact with.
    pub prop: Entity,
    /// Which of the things it offers.
    pub verb: VerbId,
}
impl Action for Interact {}

/// Somebody interacted with a prop.
///
/// The whole of how a game answers a prop of its own: a system in
/// [`TurnSet::React`](crate::plugin::TurnSet::React) reads these, keeps
/// the ones whose verb is its own, and does whatever that verb means.
/// The engine needs no registry of behaviours because a game already
/// knows how to answer a message.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interacted {
    /// Who did it.
    pub actor: Entity,
    /// What they did it to.
    pub prop: Entity,
    /// What they did.
    pub verb: VerbId,
}

/// What answering an interaction reads: what was offered, what each offer
/// lands, and which prop is which.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Answering<'w, 's> {
    offered: Res<'w, OfferedHere>,
    effects: Option<Res<'w, PropEffects>>,
    registries: Res<'w, Registries>,
    verbs: Res<'w, Verbs>,
    props: Query<'w, 's, &'static PropKind, With<Prop>>,
}

/// Resolves an interaction against what the gate offered.
pub fn resolve_interactions(
    mut intents: MessageReader<Intent<Interact>>,
    mut done: MessageWriter<Interacted>,
    mut resolution: Resolution,
    what: Answering,
    mut world: crate::effects::EffectWorld,
) {
    let Answering { offered, effects, registries, verbs, props } = &what;
    for intent in intents.read() {
        let Some(offer) = offered.find(intent.actor, intent.action.prop, intent.action.verb) else {
            continue;
        };
        if offer.refused.is_some() || !resolution.claim(intent.actor) {
            continue;
        }
        resolution.done(intent.actor, offer.time);
        // What the offer carried, when it carried anything: an offer with
        // no effects is answered by the game, from the message below.
        if let (Some(effects), Ok(kind)) = (effects.as_deref(), props.get(intent.action.prop)) {
            let which = registries.props.get(kind.0).offers.iter().position(|o| verbs.get(&o.verb) == Some(intent.action.verb));
            let at = world.position(intent.action.prop).unwrap_or_default();
            if let Some(which) = which
                && let Some(effects) = effects.offer(kind.0, which)
            {
                effects.land_on(crate::effects::Source::Offer(intent.action.prop), intent.action.prop, at, vec![intent.actor], &mut world);
            }
        }
        done.write(Interacted { actor: intent.actor, prop: intent.action.prop, verb: intent.action.verb });
    }
}

/// A prop that holds things.
///
/// Its [`Inventory`] is what it holds, so everything that already knows
/// how to read a bag reads this one too. What goes in is the game's: the
/// engine rolls how many of each and asks with [`FillContainer`], because
/// items are a game's own registry and only a game can spawn one.
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

/// Asks the game to put `count` of the item named `item` into `prop`.
///
/// The engine rolled the count from the definition and its own stream;
/// spawning is the game's, since only it knows what a "slug" is. A game
/// answers in [`TurnSet::React`](crate::plugin::TurnSet::React) or in any
/// `Update` system, spawns what it spawns anywhere else, and pushes the
/// entities onto the prop's [`Inventory`].
#[derive(Message, Debug, Clone)]
pub struct FillContainer {
    /// Which container.
    pub prop: Entity,
    /// What goes in, by the name its own registry knows it by.
    pub item: String,
    /// How many, already rolled.
    pub count: u32,
}

/// A container whose contents have been asked for, so they are asked for
/// once however many frames it takes for the run's streams to exist.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Stocked;

/// A container waiting to be filled, and which definition says with what.
type Unstocked = (Entity, &'static PropKind);

/// A container whose bag has just changed, and what it was made from.
type JustEmptied<'w, 's> = Query<'w, 's, (Entity, &'static PropKind, &'static Inventory), (With<Container>, Without<Emptied>, Changed<Inventory>)>;

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
                asks.write(FillContainer { prop, item: roll.item.clone(), count });
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
    mut held: Query<(&Position, &mut Inventory), With<Container>>,
    stacks: Query<&crate::items::Stack>,
) {
    for intent in intents.read() {
        let actor = intent.actor;
        let Ok((at, _)) = takers.get(actor) else { continue };
        let Ok((chest, _)) = held.get(intent.action.from) else { continue };
        if chest.0 != at.0 && !geometry::is_adjacent(at.0, chest.0) {
            continue;
        }
        let taking: Vec<Entity> = {
            let Ok((_, contents)) = held.get(intent.action.from) else { continue };
            match intent.action.item {
                Some(one) => contents.items.iter().copied().filter(|i| *i == one).collect(),
                None => contents.items.clone(),
            }
        };
        if taking.is_empty() || !resolution.claim(actor) {
            continue;
        }
        if let Ok((_, mut contents)) = held.get_mut(intent.action.from) {
            contents.items.retain(|i| !taking.contains(i));
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
) {
    if built.is_some() {
        return;
    }
    let names = registries.names();
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

/// Reports the `entered` moment on every armed prop a step lands on.
///
/// Reads [`Stepped`](crate::turn::Stepped), which the move resolver writes
/// for every step it lets through, so anything that walks sets off a
/// plate: the player, a droid, a rat. Whether it could see the plate never
/// comes into it, which is the point of a hidden one.
pub fn report_entered(mut steps: MessageReader<crate::turn::Stepped>, props: Armed, map: Res<WorldMap>, mut fired: MessageWriter<Fired>) {
    let here = map.current();
    for step in steps.read() {
        for (prop, at, on) in &props {
            if at.0 == step.to && on.map(|m| m.0).unwrap_or(MapId::SURFACE) == here {
                fired.write(Fired { on: prop, moment: Moments::ENTERED, by: Some(step.actor), at: step.to });
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
/// triggers and its name, is reported in its place, and is despawned once
/// they land. A prop with nothing to do when broken leaves none.
pub fn report_destroyed(mut commands: Commands, mut deaths: MessageReader<crate::combat::DeathEvent>, props: Query<(&Triggers, Option<&Name>), With<Prop>>) {
    for death in deaths.read() {
        let Ok((triggers, name)) = props.get(death.entity) else { continue };
        if triggers.on(Moments::DESTROYED).next().is_none() {
            continue;
        }
        let remnant = Remnant { moment: Moments::DESTROYED, by: death.credit, at: death.at };
        let mut carrier = commands.spawn((triggers.clone(), remnant, LandsAsItself));
        if let Some(name) = name {
            carrier.insert(name.clone());
        }
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
    /// Percent chance a turn to spot it while it is in sight.
    pub spot: u8,
}

/// A hidden prop was spotted.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spotted {
    /// Which prop.
    pub prop: Entity,
    /// Who spotted it.
    pub by: Entity,
    /// Where it is.
    pub at: Point,
}

/// A hidden prop, and where it is.
type Unspotted<'w, 's> = Query<'w, 's, (Entity, &'static Position, Option<&'static OnMap>, &'static Hidden)>;

/// Rolls for the player to spot what is hidden in sight.
///
/// In [`DecideSet::Notice`](crate::plugin::DecideSet::Notice), beside the
/// stealth roll it resembles, and only for the player: a mind that will
/// not interact with a prop has nothing to do with having seen one, and a
/// trap catches whoever steps on it either way.
///
/// There is no searching. Spending turns pressing a key at every wall is a
/// chore the genre has spent twenty years removing, so a prop in sight is
/// rolled for each turn and found or not.
pub fn spot_hidden_props(
    mut commands: Commands,
    mut spotted: MessageWriter<Spotted>,
    mut rng: ResMut<PropRng>,
    map: Res<crate::world::WorldMap>,
    player: Query<(Entity, &crate::components::Viewshed, Option<&OnMap>), With<crate::components::Player>>,
    hidden: Unspotted,
) {
    use rand::Rng;
    let Ok((who, sight, on)) = player.single() else { return };
    let here = on.map(|m| m.0).unwrap_or(MapId::SURFACE);
    if here != map.current() {
        return;
    }
    for (prop, at, prop_on, secret) in &hidden {
        if prop_on.map(|m| m.0).unwrap_or(MapId::SURFACE) != here || !sight.can_see(at.0) {
            continue;
        }
        if secret.spot == 0 || !rng.0.random_ratio(u32::from(secret.spot.min(100)), 100) {
            continue;
        }
        commands.entity(prop).remove::<Hidden>();
        spotted.write(Spotted { prop, by: who, at: at.0 });
    }
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
            .add_systems(Turn, offer_here.in_set(DecideSet::Offer))
            .add_systems(Turn, (resolve_interactions, resolve_takes).in_set(ResolveSet::Act))
            .add_systems(Turn, close_emptied_containers.in_set(crate::plugin::TurnSet::React))
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
            .add_systems(Turn, report_destroyed.in_set(crate::plugin::TurnSet::React));
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
             container: (contents: [("slug", 8, 12)]), offers: [(verb: "open", time: 200)]),
            (name: "pressure plate", glyph: '^', color: (r: 230, g: 140, b: 51)),
        ]"#;

    fn arena() -> (App, Point) {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, PropsPlugin, crate::world::StreamingPlugin));
        let start = crate::testing::surface(&mut app);
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
                spawn_prop(&mut commands, &registries, crate_id, start.offset(1, 0), MapId::SURFACE),
                spawn_prop(&mut commands, &registries, plate_id, start.offset(2, 0), MapId::SURFACE),
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
        let supply = spawn_prop(&mut app.world_mut().commands(), &registries, id, at, MapId::SURFACE);
        app.world_mut().resource_mut::<NextState<crate::state::EngineState>>().set(crate::state::EngineState::Playing);
        app.update();
        app.update();
        assert_eq!(app.world().resource::<Occupancy>().first_at(at), Some(supply), "the crate holds the cell");
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

    /// A crate that opens, a console with a verb of the game's own, and a
    /// cache that wants a cutter.
    const PROPS: &str = r#"#![enable(implicit_some)]
        [
            (name: "supply crate", glyph: '&', color: (r: 190, g: 165, b: 115), blocks: true,
             container: (contents: [("slug", 1, 1)]), offers: [(verb: "open", time: 200)]),
            (name: "reactor console", glyph: '%', color: (r: 89, g: 217, b: 230), blocks: true,
             offers: [(verb: "charge", time: 300)]),
            (name: "locked cache", glyph: '&', color: (r: 204, g: 204, b: 217), blocks: true,
             container: (contents: [("slug", 1, 1)], locked: "cutter"), offers: [(verb: "open", time: 250)]),
            (name: "workbench", glyph: 'T', color: (r: 150, g: 120, b: 90), blocks: true,
             offers: [(verb: "search", time: 100), (verb: "charge", time: 100)]),
        ]"#;

    fn arena() -> (App, Point) {
        let mut app = headless_app();
        // Bump is CorePlugin's, which `headless_app` already added.
        app.add_plugins((crate::fov::FovPlugin, PropsPlugin, crate::items::ItemsPlugin, crate::world::StreamingPlugin));
        app.add_verb("charge");
        let start = crate::testing::surface(&mut app);
        let tags = Registry::from_defs(vec![TagDef::new("cutter")]).expect("one tag");
        let props = rl_rules::prop::load(PROPS, &Names::new().tags(&tags)).expect("the props load");
        app.insert_resource(Registries { props, tags, ..Default::default() });
        app.insert_resource(crate::seed::Seed(crate::testing::TEST_SEED));
        (app, start)
    }

    fn player_at(app: &mut App, at: Point) -> Entity {
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(at), Viewshed::new(8), OnMap(MapId::SURFACE))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        player
    }

    fn put(app: &mut App, name: &str, at: Point) -> Entity {
        let registries = app.world().resource::<Registries>().clone();
        let id = registries.props.expect(name);
        let prop = spawn_prop(&mut app.world_mut().commands(), &registries, id, at, MapId::SURFACE);
        app.update();
        prop
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
        let props: Vec<Entity> = offered.for_actor(player).iter().map(|o| o.prop).collect();
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
        app.world_mut().write_message(Intent::new(player, Interact { prop: console, verb: charge }));
        app.update();

        assert_eq!(app.world().resource::<Turns>().now(), before + 300, "the turn cost what the offer said");
        assert_eq!(app.world().resource::<Seen>().0, vec![Interacted { actor: player, prop: console, verb: charge }], "and the game is told what was done");
    }

    /// Nothing is spent finding out what the gate already worked out.
    #[test]
    fn an_interaction_nobody_offered_costs_nothing() {
        let (mut app, start) = arena();
        let far = put(&mut app, "supply crate", start.offset(5, 0));
        let player = player_at(&mut app, start);

        let before = app.world().resource::<Turns>().now();
        app.world_mut().write_message(Intent::new(player, Interact { prop: far, verb: Verbs::OPEN }));
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
        assert_eq!(app.world().resource::<Seen>().0, vec![Interacted { actor: player, prop: supply, verb: Verbs::OPEN }], "the walk key opened the crate");
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
             container: (contents: [("slug", 2, 2), ("medkit", 1, 1)], opened: (glyph: '"', color: (r: 128, g: 115, b: 90))),
             offers: [(verb: "open", time: 200)]),
            (name: "open bin", glyph: 'u', color: (r: 150, g: 150, b: 150), blocks: true,
             container: (contents: [("slug", 1, 1)]),
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
        app.add_plugins((crate::fov::FovPlugin, PropsPlugin, crate::items::ItemsPlugin, crate::world::StreamingPlugin));
        let start = crate::testing::surface(&mut app);
        let props = rl_rules::prop::load(PROPS, &Names::new()).expect("the props load");
        app.insert_resource(Registries { props, ..Default::default() });
        app.insert_resource(crate::seed::Seed(crate::testing::TEST_SEED));

        // The game's half: spawn what it was asked for and put it in.
        app.add_systems(Update, |mut commands: Commands, mut asks: MessageReader<FillContainer>, mut bags: Query<&mut Inventory>| {
            for ask in asks.read() {
                let mut items = Vec::new();
                for _ in 0..ask.count {
                    let mut item = commands.spawn((Item, Name::new(ask.item.clone())));
                    if ask.item == "slug" {
                        item.insert(Stack { key: 1, count: 1 });
                    }
                    items.push(item.id());
                }
                if let Ok(mut bag) = bags.get_mut(ask.prop) {
                    bag.items.extend(items);
                }
            }
        });

        let registries = app.world().resource::<Registries>().clone();
        let id = registries.props.expect(name);
        let prop = spawn_prop(&mut app.world_mut().commands(), &registries, id, start.offset(1, 0), MapId::SURFACE);
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), OnMap(MapId::SURFACE), Inventory::default())).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        for _ in 0..3 {
            app.update();
        }
        Chest { app, player, prop }
    }

    fn bag_of(app: &App, who: Entity) -> Vec<Entity> {
        app.world().get::<Inventory>(who).map(|b| b.items.clone()).unwrap_or_default()
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
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, PropsPlugin, crate::world::StreamingPlugin));
        app.add_engine_effects();
        let at = crate::testing::surface(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        let props = {
            let registries = app.world().resource::<Registries>().clone();
            rl_rules::prop::load(PROPS, &registries.names()).expect("the props load")
        };
        app.world_mut().resource_mut::<Registries>().props = props;
        let player = app
            .world_mut()
            .spawn((Actor, Player, Blocks, Position(at), Viewshed::new(8), OnMap(MapId::SURFACE), Health::full(20), crate::combat::Faction(sides.ours)))
            .id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        Deck { app, player, at }
    }

    fn put(deck: &mut Deck, name: &str, at: Point) -> Entity {
        let registries = deck.app.world().resource::<Registries>().clone();
        let id = registries.props.expect(name);
        let prop = spawn_prop(&mut deck.app.world_mut().commands(), &registries, id, at, MapId::SURFACE);
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

    /// A prop in sight is rolled for: a certainty is found on the first
    /// pass it is in sight for, and a prop nobody can spot stays hidden
    /// however long it is looked at.
    #[test]
    fn a_hidden_prop_in_sight_is_spotted_by_a_roll_and_never_without_one() {
        let mut deck = deck();
        // Spotted at a hundred percent: in sight, so the pass that follows
        // putting it down finds it.
        let certain = {
            let at = deck.at.offset(2, 0);
            put(&mut deck, "hidden plate", at)
        };
        assert!(deck.app.world().get::<Hidden>(certain).is_none(), "spotted, and from then on an ordinary prop");

        // The same plate, with no chance of being spotted at all.
        let never = {
            let at = deck.at.offset(3, 0);
            put(&mut deck, "hidden plate", at)
        };
        deck.app.world_mut().entity_mut(never).insert(Hidden { spot: 0 });
        let player = deck.player;
        for _ in 0..5 {
            deck.app.world_mut().write_message(Intent::new(player, crate::turn::Wait));
            deck.app.update();
        }
        assert!(deck.app.world().get::<Hidden>(never).is_some(), "five turns of looking at it and still unseen");
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
            app.add_plugins((crate::fov::FovPlugin, CombatPlugin, PropsPlugin, crate::effects::EffectsPlugin, crate::world::StreamingPlugin));
            app.add_engine_effects();
            let at = crate::testing::surface(&mut app);
            let sides = crate::testing::two_sides(&mut app);
            let props = {
                let registries = app.world().resource::<Registries>().clone();
                rl_rules::prop::load(PROPS, &registries.names()).expect("the props load")
            };
            app.world_mut().resource_mut::<Registries>().props = props;
            let player = app
                .world_mut()
                .spawn((Actor, Player, Blocks, Position(at), Viewshed::new(8), OnMap(MapId::SURFACE), Health::full(20), crate::combat::Faction(sides.ours)))
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
