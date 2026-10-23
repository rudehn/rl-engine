//! A whole run, saved and continued, with the game saying only what a
//! kind of thing is.
//!
//! Every game that saves used to write the same walk: every monster by
//! definition name, position, map and health; every item by definition,
//! count and level, on the ground or in whose bag; the player with its
//! bag, its slots and its statuses; the stairs; and then the same in
//! reverse, items first so bags can point at them. Five hundred lines in
//! the first game to write them, and every game would write them again.
//!
//! The engine walks the world instead. A game implements [`Saveable`] on
//! the component that marks a kind of thing it spawns, saying how to write
//! one down and how to spawn one again from what was written, and
//! registers it with [`AddSaveable::save_kind`]. Everything the engine
//! owns on that entity, where it stands, its health, what it carries and
//! wears, its statuses and which worn thing holds each held one, the side
//! a status turned it from, its stack, the transition it is, the post it
//! holds, how long it has left alight, is the engine's to
//! capture and put back, in [`EntityState`]. A
//! resource a game keeps of the run goes through [`SaveableState`] and
//! [`save_state`](AddSaveable::save_state). A kind a game makes through
//! its `ItemMaker` or its `ActorMaker` need not say how it is made again:
//! [`SaveableItem`] and [`SaveableActor`] write it as its definition, make
//! it through the maker, and put back only what else the game keeps.
//!
//! [`RunSave`] is the result: the engine's own [`EngineSave`], every kind's
//! entries in the kind's own RON, the engine's state on each, and the
//! game's resources. [`SavePlugin`] owns the loop around it: the stash kept
//! a turn behind the run so a closed window saves, and the slot deleted
//! when the run ends, so nothing is resumed past its end, and, when asked
//! with [`SavePlugin::on_arrival`], the slot written by `save_on_arrival`
//! every time the player arrives somewhere. A game's save key calls
//! [`save_run`], and its start, when [`ContinueRun`](atglyph_bevy::ContinueRun)
//! asks, calls [`load_run`] and [`RunSave::restore`]. The engine's own
//! resources are saved by the engine: [`Quests`] and [`Counters`] each
//! implement [`SaveableState`]. So are the stairs a ladder's floors are
//! laid with, [`Stair`], the engine's own kind beside the props.
//!
//! What a title needs of the slot is kept here too, in `atglyph-bevy`'s words so
//! the screen need not depend on this crate: [`SavedRun`] says what the
//! slot holds from before the first frame, a run left with [`LeaveRun`] is
//! written down by `save_on_leaving` rather than forgotten with its
//! teardown, and [`AbandonRun`] forgets the slot the moment it is triggered.

use atglyph_bevy::{
    AbandonRun, ActiveStatuses, ActorMaker, ApplyStatus, Burning, Counters, Dead, Emptied, EndOfFrame, EndRun, Ending, EngineState, Equipped, Faction, Health,
    Hidden, Inventory, ItemLevel, ItemMaker, JoinStamp, Leader, LeaveRun, MapId, OnMap, PlaceEntered, Position, Post, PropKind, Quests, Reads, Registries,
    Remains, SavedRun, SideWas, Stack, Stair, Stocked, Transition, Turns, Wearable, WorkKinds, Working,
};
use atglyph_core::{Id, Point};
use atglyph_rules::{Equipment, Work};
use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use ron::value::RawValue;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::backend::{SaveBackend, SaveError, Saves};
use crate::engine::EngineSave;
use crate::remap::{EntityRemap, SaveId};
use crate::unload::Stash;
use crate::versioned::{decode, encode};

/// A kind of thing a game spawns, as the save sees it.
///
/// Implemented on the component that marks the kind: the definition id on
/// a monster or an item, a marker on the player. `capture` writes down what
/// the game needs to spawn one again as it was made, the definition's name
/// and whatever was rolled for it; `restore` spawns that, nowhere, carrying
/// nothing, at full health, and the engine puts the rest back from
/// [`EntityState`]. Neither says anything about position, health, bags,
/// slots or statuses, which is the point.
pub trait Saveable: Component + Sized {
    /// What one is written down as.
    type Saved: Serialize + DeserializeOwned + Send + Sync + 'static;

    /// Writes `entity`, which carries `Self`, down.
    fn capture(world: &World, entity: Entity) -> Self::Saved;

    /// Spawns one again from what was written. Spawn through
    /// `world.commands()` and `world.flush()`, or straight into the world;
    /// either way the entity exists when this returns.
    fn restore(world: &mut World, saved: &Self::Saved) -> Entity;

    /// Whether remains carrying `Self` are saved as `Self`. True for what a
    /// thing is, which its remains still are; false for what is laid on
    /// remains after the death, such as the prop kind a game dresses a
    /// body in, which the engine records as part of the remains instead,
    /// so the one entity is never written down as two kinds.
    const TAKES_REMAINS: bool = true;
}

/// A resource a game keeps of a run, as the save sees it.
///
/// For what is not an entity: which regions have been stocked, the
/// tracker's state, a set of places already marked. The resource must
/// exist when the run is restored; `restore` puts the saved part back
/// into it.
pub trait SaveableState: Resource<Mutability = bevy::ecs::component::Mutable> {
    /// What is written down.
    type Saved: Serialize + DeserializeOwned + Send + Sync + 'static;

    /// The part worth saving.
    fn capture(&self) -> Self::Saved;

    /// Puts it back.
    fn restore(&mut self, saved: Self::Saved);
}

/// A prop is the engine's to spawn, so it is the engine's to save.
///
/// The first kind that is not a game's. Every other kind is a game's
/// because the engine cannot know what a monster or a sword is made of; a
/// prop it can, because the engine read the definition out of a
/// `props.ron` itself and can read it again. A game that saves gets its
/// props saved without asking, and a game with no props saves none.
///
/// What is written down is which definition it is, by name, and the part
/// of it that is this prop's own history rather than its kind's: how often
/// its triggers have left to fire, whether it has been stocked, whether it has
/// been emptied, and whether it is still unspotted. Where it stands, and
/// what a container holds, are [`EntityState`]'s like everything else.
impl Saveable for PropKind {
    type Saved = SavedProp;

    // A body a game dressed as a prop is saved as whatever it was, and its
    // prop kind with the remains.
    const TAKES_REMAINS: bool = false;

    fn capture(world: &World, entity: Entity) -> SavedProp {
        let e = world.entity(entity);
        let name = world
            .get_resource::<Registries>()
            .map(|r| r.props.name(e.get::<PropKind>().expect("a prop kind is what this is keyed by").0).to_string())
            .unwrap_or_default();
        SavedProp {
            name,
            fires: e.get::<atglyph_bevy::Triggers>().map(|t| t.0.iter().map(|t| t.fires).collect()).unwrap_or_default(),
            stocked: e.contains::<Stocked>(),
            emptied: e.contains::<Emptied>(),
            hidden: e.get::<Hidden>().map(|h| h.spot),
        }
    }

    fn restore(world: &mut World, saved: &SavedProp) -> Entity {
        let registries = world.get_resource::<Registries>().cloned().unwrap_or_default();
        let Some(id) = registries.props.id(&saved.name) else {
            warn!("a saved prop is a {:?}, which this build has no definition for; it comes back as nothing", saved.name);
            return world.spawn_empty().id();
        };
        let prop = {
            let mut commands = world.commands();
            atglyph_bevy::spawn_prop(&mut commands, &registries, id, Point::ZERO, MapId::ZERO)
        };
        world.flush();
        // Its own firings wait for its triggers, which are armed from its
        // kind's once they are built, and a load can come first.
        let mut e = world.entity_mut(prop);
        if !saved.fires.is_empty() {
            e.insert(atglyph_bevy::PendingFires(saved.fires.clone()));
        }
        if saved.stocked {
            e.insert(Stocked);
        }
        if saved.emptied {
            e.insert(Emptied);
        }
        // Spotted is the absence of `Hidden`, and `spawn_prop` puts one on
        // whatever its definition hides: a plate the player has already
        // found does not hide itself again on a continued run.
        match saved.hidden {
            Some(spot) => {
                e.insert(Hidden { spot });
            }
            None => {
                e.remove::<Hidden>();
            }
        }
        prop
    }
}

/// A prop, as the save writes it down.
#[derive(Debug, Serialize, Deserialize)]
pub struct SavedProp {
    /// Which definition, by name.
    pub name: String,
    /// Each of its triggers' firings left, in list order; `None` for a
    /// trigger that fires every time. Empty for a prop with no triggers.
    #[serde(default)]
    pub fires: Vec<Option<u32>>,
    /// Whether what it holds has already been asked for.
    #[serde(default)]
    pub stocked: bool,
    /// Whether it has been emptied.
    #[serde(default)]
    pub emptied: bool,
    /// The chance of spotting it, while it is still unspotted.
    #[serde(default)]
    pub hidden: Option<u8>,
}

/// A stair is the engine's to lay, so it is the engine's to save.
///
/// Written down as the stair it is, which way it climbs and how it looks,
/// and nothing else: where it stands and where it leads are
/// [`EntityState`]'s, as every transition's are, so the transition is
/// written once. Saved with the look it was laid with rather than read
/// again from the game's ladder, so a stair comes back as it was drawn
/// whatever the game's rules say now.
impl Saveable for Stair {
    type Saved = Stair;

    fn capture(world: &World, entity: Entity) -> Stair {
        world.get::<Stair>(entity).expect("a stair is what this is keyed by").clone()
    }

    fn restore(world: &mut World, saved: &Stair) -> Entity {
        world.spawn((saved.clone(), Name::new(saved.look.name.clone()))).id()
    }
}

/// The fact ledger is the engine's, so its saving is too: a game with a
/// [`Counters`] adds `save_state::<Counters>()` and nothing more.
impl SaveableState for Counters {
    type Saved = atglyph_rules::Ledger;

    fn capture(&self) -> atglyph_rules::Ledger {
        self.0.clone()
    }

    fn restore(&mut self, saved: atglyph_rules::Ledger) {
        self.0 = saved;
    }
}

/// The quest tracker is the engine's, so its saving is too: a game with a
/// [`Quests`] resource adds `save_state::<Quests>()` and nothing more.
impl SaveableState for Quests {
    type Saved = atglyph_rules::Tracker;

    fn capture(&self) -> atglyph_rules::Tracker {
        self.tracker.clone()
    }

    fn restore(&mut self, saved: atglyph_rules::Tracker) {
        self.tracker = saved;
    }
}

/// A kind of item a game makes through its [`ItemMaker`], as the save sees
/// it: which definition it is and at which level, which the engine writes
/// down and makes again through the maker, and whatever else the game
/// keeps of the thing.
///
/// Implemented on the component that marks the definition an item was
/// made from, and registered with [`AddSaveable::save_items`] in place of
/// [`AddSaveable::save_kind`]. The level is the thing's [`ItemLevel`],
/// nought for a thing with none; its charges, its stack and where it lies
/// are the engine's state, as on any saved kind.
///
/// On restore the thing is made at its level first, through
/// [`ItemMaker::make_at`], and only then is the game's [`Extra`](Self::Extra)
/// put back on it, since what a game keeps is often about the level, a
/// curse that holds a thing below nought or an aim at learning what it is.
pub trait SaveableItem: Component + Sized {
    /// The game's item maker, which names a definition and makes one.
    type Maker: ItemMaker;
    /// What else the game keeps of one: `()` for nothing.
    type Extra: Serialize + DeserializeOwned + Send + Sync + 'static;

    /// The definition this thing was made from.
    fn def(&self) -> Id<<Self::Maker as ItemMaker>::Def>;

    /// Writes down what else the game keeps of `entity`.
    fn capture_extra(world: &World, entity: Entity) -> Self::Extra;

    /// Puts `extra` back on `entity`, which has just been made again at its
    /// level.
    fn restore_extra(world: &mut World, entity: Entity, extra: &Self::Extra);
}

/// An item, as the save writes it through its [`SaveableItem`]: which
/// definition, by name, at which level, and the game's own extra.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct SavedItem<E> {
    /// The definition's name, as its maker names it.
    pub def: String,
    /// Its level; nought for a thing that takes none.
    #[serde(default)]
    pub level: i32,
    /// What else the game kept of it.
    pub extra: E,
}

fn capture_item<K: SaveableItem>(world: &World, entity: Entity) -> Result<Box<RawValue>, SaveError> {
    let kind = world.get::<K>(entity).expect("an item is saved by the kind it carries");
    let saved = SavedItem {
        def: world.resource::<K::Maker>().name_of(kind.def()).to_string(),
        level: world.get::<ItemLevel>(entity).map_or(0, |l| l.0),
        extra: K::capture_extra(world, entity),
    };
    RawValue::from_rust(&saved).map_err(|e| SaveError::Encode(e.to_string()))
}

fn restore_item<K: SaveableItem>(world: &mut World, saved: &RawValue) -> Result<Entity, SaveError> {
    let saved: SavedItem<K::Extra> = saved.into_rust().map_err(|e| SaveError::Decode(e.to_string()))?;
    let Some(def) = world.resource::<K::Maker>().id_of(&saved.def) else {
        warn!("a saved item is a {:?}, which this build has no definition for; it comes back as nothing", saved.def);
        return Ok(world.spawn_empty().id());
    };
    let mut queue = CommandQueue::default();
    let item = {
        let world: &World = world;
        let mut commands = Commands::new(&mut queue, world);
        world.resource::<K::Maker>().make_at(&mut commands, world.resource::<Registries>(), def, saved.level)
    };
    queue.apply(world);
    K::restore_extra(world, item, &saved.extra);
    Ok(item)
}

/// A kind of actor a game makes through its [`ActorMaker`], as the save
/// sees it: which definition it is, which the engine writes down and makes
/// again through the maker, and whatever else the game keeps of the one
/// actor.
///
/// Implemented on the component that marks the definition an actor was
/// made from, and registered with [`AddSaveable::save_actors`] in place of
/// [`AddSaveable::save_kind`]. Where it stands, its health, what it
/// carries and wears, its statuses and the post it holds are the engine's
/// state, as on any saved kind.
///
/// On restore it is made first, nowhere, through [`ActorMaker::make_at`],
/// exactly as its kind is made, and only then is the game's
/// [`Extra`](Self::Extra) put back on it, since what a game keeps is
/// often a change to what it was made as: a side it was turned to, a mind
/// it was given, a look it lost.
pub trait SaveableActor: Component + Sized {
    /// The game's actor maker, which names a definition and makes one.
    type Maker: ActorMaker;
    /// What else the game keeps of one: `()` for nothing.
    type Extra: Serialize + DeserializeOwned + Send + Sync + 'static;

    /// The definition this actor was made from.
    fn def(&self) -> Id<<Self::Maker as ActorMaker>::Def>;

    /// Writes down what else the game keeps of `entity`.
    fn capture_extra(world: &World, entity: Entity) -> Self::Extra;

    /// Puts `extra` back on `entity`, which has just been made again.
    fn restore_extra(world: &mut World, entity: Entity, extra: &Self::Extra);

    /// Whether remains carrying `Self` are saved as `Self`, as
    /// [`Saveable::TAKES_REMAINS`] says for a kind: true for what an actor
    /// is, which its remains still are.
    const TAKES_REMAINS: bool = true;
}

/// An actor, as the save writes it through its [`SaveableActor`]: which
/// definition, by name, and the game's own extra.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct SavedActor<E> {
    /// The definition's name, as its maker names it.
    pub def: String,
    /// What else the game kept of it.
    pub extra: E,
}

fn capture_actor<K: SaveableActor>(world: &World, entity: Entity) -> Result<Box<RawValue>, SaveError> {
    let kind = world.get::<K>(entity).expect("an actor is saved by the kind it carries");
    let saved = SavedActor { def: world.resource::<K::Maker>().name_of(kind.def()).to_string(), extra: K::capture_extra(world, entity) };
    RawValue::from_rust(&saved).map_err(|e| SaveError::Encode(e.to_string()))
}

fn restore_actor<K: SaveableActor>(world: &mut World, saved: &RawValue) -> Result<Entity, SaveError> {
    let saved: SavedActor<K::Extra> = saved.into_rust().map_err(|e| SaveError::Decode(e.to_string()))?;
    let Some(def) = world.resource::<K::Maker>().id_of(&saved.def) else {
        warn!("a saved actor is a {:?}, which this build has no definition for; it comes back as nothing", saved.def);
        return Ok(world.spawn_empty().id());
    };
    let mut queue = CommandQueue::default();
    let actor = {
        let world: &World = world;
        let mut commands = Commands::new(&mut queue, world);
        // Nowhere and on no place: the engine puts it back where it was.
        world.resource::<K::Maker>().make_at(&mut commands, world.resource::<Registries>(), def, Point::ZERO, MapId::ZERO)
    };
    queue.apply(world);
    K::restore_extra(world, actor, &saved.extra);
    Ok(actor)
}

/// Registers what a run is made of.
pub trait AddSaveable {
    /// Saves every entity carrying `K`, by `K`'s own account of it.
    fn save_kind<K: Saveable>(&mut self) -> &mut Self;

    /// Saves every item carrying `K` as its definition, its level and the
    /// game's extra, made again through `K`'s maker.
    fn save_items<K: SaveableItem>(&mut self) -> &mut Self;

    /// Saves every actor carrying `K` as its definition and the game's
    /// extra, made again through `K`'s maker.
    fn save_actors<K: SaveableActor>(&mut self) -> &mut Self;

    /// Saves the resource `R`, by its own account of itself.
    fn save_state<R: SaveableState>(&mut self) -> &mut Self;
}

impl AddSaveable for App {
    fn save_kind<K: Saveable>(&mut self) -> &mut Self {
        self.init_resource::<SaveRegistry>();
        let label = short_name::<K>();
        let mut registry = self.world_mut().resource_mut::<SaveRegistry>();
        if registry.kinds.iter().any(|k| k.label == label) {
            panic!("two saved kinds are both called {label}; the save tells them apart by that name");
        }
        registry.kinds.push(KindEntry { label, collect: collect::<K>, capture: capture_kind::<K>, restore: restore_kind::<K> });
        self
    }

    fn save_items<K: SaveableItem>(&mut self) -> &mut Self {
        self.init_resource::<SaveRegistry>();
        let label = short_name::<K>();
        let mut registry = self.world_mut().resource_mut::<SaveRegistry>();
        if registry.kinds.iter().any(|k| k.label == label) {
            panic!("two saved kinds are both called {label}; the save tells them apart by that name");
        }
        registry.kinds.push(KindEntry { label, collect: collect_items::<K>, capture: capture_item::<K>, restore: restore_item::<K> });
        self
    }

    fn save_actors<K: SaveableActor>(&mut self) -> &mut Self {
        self.init_resource::<SaveRegistry>();
        let label = short_name::<K>();
        let mut registry = self.world_mut().resource_mut::<SaveRegistry>();
        if registry.kinds.iter().any(|k| k.label == label) {
            panic!("two saved kinds are both called {label}; the save tells them apart by that name");
        }
        registry.kinds.push(KindEntry { label, collect: collect_actors::<K>, capture: capture_actor::<K>, restore: restore_actor::<K> });
        self
    }

    fn save_state<R: SaveableState>(&mut self) -> &mut Self {
        self.init_resource::<SaveRegistry>();
        let label = short_name::<R>();
        let mut registry = self.world_mut().resource_mut::<SaveRegistry>();
        if registry.states.iter().any(|s| s.label == label) {
            panic!("two saved resources are both called {label}; the save tells them apart by that name");
        }
        registry.states.push(StateEntry { label, capture: capture_state::<R>, restore: restore_state::<R> });
        self
    }
}

/// `T`'s name without its path, which is what a kind is filed under.
fn short_name<T>() -> &'static str {
    std::any::type_name::<T>().rsplit("::").next().unwrap_or("kind")
}

/// Every kind and resource a game registered.
#[derive(Resource, Default)]
pub(crate) struct SaveRegistry {
    kinds: Vec<KindEntry>,
    states: Vec<StateEntry>,
}

/// One kind, with its account of itself made into plain functions.
struct KindEntry {
    label: &'static str,
    collect: fn(&mut World) -> Vec<Entity>,
    capture: fn(&World, Entity) -> Result<Box<RawValue>, SaveError>,
    restore: fn(&mut World, &RawValue) -> Result<Entity, SaveError>,
}

/// One resource, likewise.
struct StateEntry {
    label: &'static str,
    capture: fn(&World) -> Result<Option<Box<RawValue>>, SaveError>,
    restore: fn(&mut World, &RawValue) -> Result<(), SaveError>,
}

/// Every living `K` still in play, in spawn order, so a save is the same
/// bytes for the same run whatever order the archetypes are in. The dead
/// and the spent are kept to the end of their frame only so the log can
/// name them, and are gone by the next.
fn collect<K: Saveable>(world: &mut World) -> Vec<Entity> {
    let mut found: Vec<Entity> = world.query_filtered::<Entity, (With<K>, Without<Dead>, Without<atglyph_bevy::Spent>)>().iter(world).collect();
    if !K::TAKES_REMAINS {
        found.retain(|e| world.get::<Remains>(*e).is_none());
    }
    found.sort_by_key(|e| (e.index(), *e));
    found
}

/// Every actor carrying `K` still in play, as [`collect`] finds a kind's,
/// its remains among them as its `TAKES_REMAINS` says.
fn collect_actors<K: SaveableActor>(world: &mut World) -> Vec<Entity> {
    let mut found: Vec<Entity> = world.query_filtered::<Entity, (With<K>, Without<Dead>, Without<atglyph_bevy::Spent>)>().iter(world).collect();
    if !K::TAKES_REMAINS {
        found.retain(|e| world.get::<Remains>(*e).is_none());
    }
    found.sort_by_key(|e| (e.index(), *e));
    found
}

/// Every item carrying `K` still in play, as [`collect`] finds a kind's.
fn collect_items<K: SaveableItem>(world: &mut World) -> Vec<Entity> {
    let mut found: Vec<Entity> = world.query_filtered::<Entity, (With<K>, Without<Dead>, Without<atglyph_bevy::Spent>)>().iter(world).collect();
    found.sort_by_key(|e| (e.index(), *e));
    found
}

fn capture_kind<K: Saveable>(world: &World, entity: Entity) -> Result<Box<RawValue>, SaveError> {
    RawValue::from_rust(&K::capture(world, entity)).map_err(|e| SaveError::Encode(e.to_string()))
}

fn restore_kind<K: Saveable>(world: &mut World, saved: &RawValue) -> Result<Entity, SaveError> {
    let saved: K::Saved = saved.into_rust().map_err(|e| SaveError::Decode(e.to_string()))?;
    Ok(K::restore(world, &saved))
}

fn capture_state<R: SaveableState>(world: &World) -> Result<Option<Box<RawValue>>, SaveError> {
    world.get_resource::<R>().map(|r| RawValue::from_rust(&r.capture()).map_err(|e| SaveError::Encode(e.to_string()))).transpose()
}

fn restore_state<R: SaveableState>(world: &mut World, saved: &RawValue) -> Result<(), SaveError> {
    let saved: R::Saved = saved.into_rust().map_err(|e| SaveError::Decode(e.to_string()))?;
    match world.get_resource_mut::<R>() {
        Some(mut r) => {
            r.restore(saved);
            Ok(())
        }
        None => Err(SaveError::Decode(format!("the save holds a {}, and the game has none to put it back into", short_name::<R>()))),
    }
}

/// What the engine owns on a saved entity.
///
/// Every field is optional and empty by default, so a stairway saves as a
/// position and a transition and nothing else, and a kind that gains a bag
/// later reads an old save unchanged.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EntityState {
    /// Where it stands or lies, and on which map; `None` for something
    /// carried.
    #[serde(default)]
    pub at: Option<(Point, MapId)>,
    /// Health left and the most it has.
    #[serde(default)]
    pub health: Option<(i32, i32)>,
    /// What it carries, in bag order.
    #[serde(default)]
    pub bag: Vec<SaveId>,
    /// How many slots it has and which carried things are in them, in slot
    /// order. Put back by each item's own shape, so no slot is named.
    #[serde(default)]
    pub worn: Option<(usize, Vec<SaveId>)>,
    /// Its statuses by registered name, with the turns left on each: the
    /// ones no worn thing holds.
    #[serde(default)]
    pub statuses: Vec<(String, u32)>,
    /// Its statuses a worn thing holds, by registered name, with the turns
    /// left on each and the thing that holds it. A field of its own rather
    /// than a third member of `statuses`, so a save written before a status
    /// could be held reads unchanged.
    #[serde(default)]
    pub held: Vec<(String, u32, SaveId)>,
    /// The side it goes back to, by registered name, while a status has it
    /// fighting for another: its [`SideWas`]. Put back as its side, so the
    /// status, asked for again, turns it from there as it did the first
    /// time, and a game that keeps sides of its own never has to.
    #[serde(default)]
    pub side_was: Option<String>,
    /// How many, for a stack.
    #[serde(default)]
    pub stack: Option<u32>,
    /// Where it leads, for a transition.
    #[serde(default)]
    pub transition: Option<Transition>,
    /// What is left of it, when it died and stayed: the clock reading it
    /// died on, and who got the credit if that one was saved too. A game
    /// spawns the thing alive again from its own record, which is all it
    /// ever wrote down; this is what tells the engine to lay it back down
    /// dead.
    #[serde(default)]
    pub remains: Option<(u32, Option<SaveId>)>,
    /// The prop kind the remains were dressed as, by registered name: what
    /// a game made of a body after the death, which a kind that saves what
    /// the thing was knows nothing of.
    #[serde(default)]
    pub remains_as: Option<String>,
    /// Where it was posted, for an actor that holds a post, so a guard still
    /// walks back to its cell after a load.
    #[serde(default)]
    pub post: Option<Point>,
    /// What it was in the middle of, for an actor at work.
    #[serde(default)]
    pub working: Option<SavedWork>,
    /// How long it has left alight, for something that was burning. The
    /// burning cell under it is in [`EngineSave`], but that holds only the
    /// two turns the thing last lent it, so without this a crate that
    /// caught mid-run would come back out after two turns rather than
    /// after what it had left.
    #[serde(default)]
    pub burning: Option<SavedBurning>,
    /// The one it follows, for an actor with a [`Leader`], when that one
    /// was saved too. A kind's brain is content made again from its
    /// definition, so the leader is kept here, by save id, and not in it.
    #[serde(default)]
    pub leader: Option<SaveId>,
    /// Where it came in the order followers joined, its [`JoinStamp`], kept
    /// beside the leader so a party listed in that order reads the same
    /// after a load. `None` from a save written before it was kept, and the
    /// follower is then stamped afresh, after everyone saved with one.
    #[serde(default)]
    pub joined: Option<u64>,
}

/// How long something alight has left, as a save keeps it. An enum of its
/// own rather than an `Option` inside the field's, so "alight for good" and
/// "not alight" cannot be read as each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SavedBurning {
    /// This many turns.
    For(u32),
    /// It never goes out.
    Forever,
}

/// Work an actor was in the middle of, as a save keeps it: the kind by its
/// word, so the order kinds were declared in never matters, and the target
/// by save id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedWork {
    /// The word the kind was declared by.
    pub kind: String,
    /// What it was done to, if anything.
    pub target: Option<SaveId>,
    /// Turns done.
    pub done: u16,
    /// Turns it takes.
    pub needed: u16,
}

impl EntityState {
    /// The engine's state on `entity`, ids through `remap`.
    fn of(world: &World, entity: Entity, remap: &mut EntityRemap) -> Self {
        let e = world.entity(entity);
        let registries = world.get_resource::<Registries>();
        let at = e.get::<Position>().map(|p| (p.0, e.get::<OnMap>().map(|m| m.0).unwrap_or(MapId::ZERO)));
        let health = e.get::<Health>().map(|h| (h.current, h.max));
        let bag = e.get::<Inventory>().map(|b| b.items.iter().map(|i| remap.save_id(*i)).collect()).unwrap_or_default();
        let worn = e.get::<Equipped>().map(|w| {
            let mut items: Vec<Entity> = Vec::new();
            for (_, item) in w.0.worn() {
                if !items.contains(&item) {
                    items.push(item);
                }
            }
            (w.0.slot_count(), items.into_iter().map(|i| remap.save_id(i)).collect())
        });
        let (mut statuses, mut held) = (Vec::new(), Vec::new());
        if let (Some(afflicted), Some(registries)) = (e.get::<ActiveStatuses>(), registries) {
            for s in afflicted.iter() {
                let name = registries.statuses.name(s.id).to_string();
                match s.held_by.and_then(Entity::try_from_bits) {
                    Some(holder) => held.push((name, s.turns, remap.save_id(holder))),
                    None => statuses.push((name, s.turns)),
                }
            }
        }
        let side_was = match (e.get::<SideWas>(), registries) {
            (Some(was), Some(registries)) => Some(registries.factions.name(was.0.0).to_string()),
            _ => None,
        };
        let remains = e.get::<Remains>().map(|r| (r.since, r.credit.map(|c| remap.save_id(c))));
        let remains_as = match (remains, e.get::<PropKind>(), registries) {
            (Some(_), Some(kind), Some(registries)) => Some(registries.props.name(kind.0).to_string()),
            _ => None,
        };
        let working = match (e.get::<Working>(), world.get_resource::<WorkKinds>()) {
            (Some(w), Some(kinds)) => {
                Some(SavedWork { kind: kinds.name(w.0.kind).to_string(), target: w.0.target.map(|t| remap.save_id(t)), done: w.0.done, needed: w.0.needed })
            }
            _ => None,
        };
        Self {
            at,
            health,
            bag,
            worn,
            statuses,
            held,
            side_was,
            stack: e.get::<Stack>().map(|s| s.count),
            transition: e.get::<Transition>().copied(),
            remains,
            remains_as,
            post: e.get::<Post>().map(|p| p.0),
            // By id only when the leader has one already: one that was not
            // saved, a body gone at the end of its frame, is not given an id
            // that would come back bound to nothing.
            leader: e.get::<Leader>().and_then(|l| remap.saved(l.0)),
            joined: e.get::<JoinStamp>().map(|j| j.0),
            working,
            // One with no turns left has burnt out and is only waiting for
            // the removal that was queued with it.
            burning: e.get::<Burning>().and_then(|b| match b.turns {
                Some(0) => None,
                Some(turns) => Some(SavedBurning::For(turns)),
                None => Some(SavedBurning::Forever),
            }),
        }
    }

    /// Puts this back on `entity`, the other entities through `remap`.
    fn restore(&self, world: &mut World, entity: Entity, remap: &EntityRemap) {
        // A held status whose thing did not come back is left out rather
        // than put back loose: it lasted only while the thing was worn, and
        // the thing is not there to wear.
        let statuses: Vec<(atglyph_rules::StatusId, u32, Option<Entity>)> = match world.get_resource::<Registries>() {
            Some(registries) => {
                let loose = self.statuses.iter().filter_map(|(name, turns)| Some((registries.statuses.id(name)?, *turns, None)));
                let held = self.held.iter().filter_map(|(name, turns, holder)| Some((registries.statuses.id(name)?, *turns, Some(remap.entity(*holder)?))));
                loose.chain(held).collect()
            }
            None => Vec::new(),
        };
        let side_was = self.side_was.as_deref().and_then(|name| world.get_resource::<Registries>()?.factions.id(name));
        let bag: Vec<Entity> = self.bag.iter().filter_map(|id| remap.entity(*id)).collect();
        let worn = self.worn.as_ref().map(|(slots, items)| {
            let mut worn = Equipment::with_slot_count(*slots);
            for item in items.iter().filter_map(|id| remap.entity(*id)) {
                match world.get::<Wearable>(item) {
                    Some(shape) => {
                        if let Err(e) = worn.equip(item, &shape.0) {
                            warn!("{item:?} could not be worn again: {e}");
                        }
                    }
                    None => warn!("{item:?} was worn when saved and cannot be worn now"),
                }
            }
            worn
        });
        // Work on something that did not come back is dropped: there is
        // nothing left to work on. A kind the game no longer declares is
        // dropped the same way, since nothing would ever finish it.
        let working = self.working.as_ref().and_then(|saved| {
            let kind = world.get_resource::<WorkKinds>()?.get(&saved.kind)?;
            let target = match saved.target {
                Some(id) => Some(remap.entity(id)?),
                None => None,
            };
            Some(Working(Work { kind, target, done: saved.done, needed: saved.needed }))
        });
        let Ok(mut target) = world.get_entity_mut(entity) else { return };
        if let Some((at, map)) = self.at {
            target.insert((Position(at), OnMap(map)));
        }
        if let Some((hp, max)) = self.health {
            target.insert(Health { current: hp, max });
        }
        if !bag.is_empty() || self.worn.is_some() {
            target.insert(Inventory { items: bag });
        }
        if let Some(worn) = worn {
            target.insert(Equipped(worn));
        }
        if let Some(count) = self.stack
            && let Some(mut stack) = target.get_mut::<Stack>()
        {
            stack.count = count;
        }
        if let Some(transition) = self.transition {
            target.insert(transition);
        }
        if let Some(at) = self.post {
            target.insert(Post(at));
        }
        if let Some(side) = side_was {
            target.insert(Faction(side));
        }
        // The stamp with the leader, in one insert, so the leader's hook
        // finds it there and does not stamp the follower afresh.
        match (self.leader.and_then(|id| remap.entity(id)), self.joined) {
            (Some(leader), Some(joined)) => drop(target.insert((JoinStamp(joined), Leader(leader)))),
            (Some(leader), None) => drop(target.insert(Leader(leader))),
            (None, _) => {}
        }
        if let Some(working) = working {
            target.insert(working);
        }
        match self.burning {
            Some(SavedBurning::For(turns)) => drop(target.insert(Burning::for_turns(turns))),
            Some(SavedBurning::Forever) => drop(target.insert(Burning::forever())),
            None => {}
        }
        // Last, and after the health a kind's own spawn gave it: a game
        // writes down what a thing is, never that it is dead, so what
        // came back is a living one, and this lays it down again exactly
        // as the death did. The killer is put back only if it was saved
        // too, since credit for a blow is not worth keeping an entity
        // alive for.
        if let Some((since, credit)) = self.remains {
            let credit = credit.and_then(|id| remap.entity(id));
            // The one lay-down a death also goes through, so the body keeps
            // a twin to be stood back up from, and is named as what is left
            // of what it was from the one place the wording lives.
            atglyph_bevy::remains::lay_down(world, entity, since, credit);
            // Then dressed as the prop the game made of it, which it was
            // saved as part of the remains rather than as a second kind.
            let dressed = self.remains_as.as_deref().and_then(|name| world.get_resource::<Registries>()?.props.id(name));
            if let (Some(kind), Ok(mut target)) = (dressed, world.get_entity_mut(entity)) {
                target.insert(PropKind(kind));
            }
        }
        // By request, so each status installs its modifiers the way it did
        // the first time. Only where statuses are resolved at all.
        if world.get_resource::<Messages<ApplyStatus>>().is_some() {
            for (status, turns, held_by) in statuses {
                world.write_message(ApplyStatus { target: entity, status, turns, by: None, held_by });
            }
        }
    }
}

/// Every entity of one kind, in the kind's own RON.
#[derive(Debug, Serialize, Deserialize)]
pub struct KindSave {
    /// The kind, by the name of its component.
    pub kind: String,
    /// Each one, by save id.
    pub entries: Vec<(SaveId, Box<RawValue>)>,
}

/// A run, whole.
///
/// The `format` is this shape's own version, checked on top of the game's:
/// a game bumps its [`SavePlugin::version`] when its kinds change shape,
/// and the engine bumps `RunSave::FORMAT` when this does.
#[derive(Debug, Serialize, Deserialize)]
pub struct RunSave {
    /// The shape this was written in.
    pub format: u32,
    /// The engine's own state.
    pub engine: EngineSave,
    /// The game's entities, kind by kind.
    pub kinds: Vec<KindSave>,
    /// The engine's state on each of them.
    pub entities: Vec<(SaveId, EntityState)>,
    /// The game's resources, by the name of each.
    pub state: Vec<(String, Box<RawValue>)>,
}

impl RunSave {
    /// The shape of this struct. Bump it when an old save would parse
    /// wrongly into the new one.
    pub(crate) const FORMAT: u32 = 1;

    /// Writes the run down: every registered kind and resource, the
    /// engine's state on each entity, then the engine's own.
    pub(crate) fn capture(world: &mut World) -> Result<Self, SaveError> {
        let registry = world.remove_resource::<SaveRegistry>().unwrap_or_default();
        let result = Self::capture_with(world, &registry);
        world.insert_resource(registry);
        result
    }

    fn capture_with(world: &mut World, registry: &SaveRegistry) -> Result<Self, SaveError> {
        let mut remap = EntityRemap::new();
        let mut kinds = Vec::new();
        // Which kind wrote each entity down: one entity written as two
        // would be spawned twice on the way back, the second overwriting
        // the first's place in every bag and slot, so it is refused here.
        let mut claimed: std::collections::BTreeMap<Entity, &str> = std::collections::BTreeMap::new();
        for kind in &registry.kinds {
            let mut entries = Vec::new();
            for entity in (kind.collect)(world) {
                if let Some(first) = claimed.insert(entity, kind.label) {
                    return Err(SaveError::Encode(format!("an entity is saved both as {first} and as {}; give it one kind", kind.label)));
                }
                entries.push((remap.save_id(entity), (kind.capture)(world, entity)?));
            }
            kinds.push(KindSave { kind: kind.label.to_string(), entries });
        }
        // The state after every kind has an id, so a bag can name what is
        // in it whichever kind was walked first.
        let bound: Vec<(SaveId, Entity)> = remap.bound().collect();
        let mut entities = Vec::with_capacity(bound.len());
        for (id, entity) in bound {
            entities.push((id, EntityState::of(world, entity, &mut remap)));
        }
        let mut state = Vec::new();
        for entry in &registry.states {
            if let Some(saved) = (entry.capture)(world)? {
                state.push((entry.label.to_string(), saved));
            }
        }
        let engine = EngineSave::capture(world, &mut remap);
        Ok(Self { format: Self::FORMAT, engine, kinds, entities, state })
    }

    /// Spawns the run again into `world`, whose content resources must
    /// already be inserted: every kind through its own `restore`, then the
    /// engine's state on each, the game's resources, and the engine's own.
    ///
    /// The seed is the engine's: read it from `engine.seed` before
    /// building the world a continued run stands in.
    pub fn restore(&self, world: &mut World) -> Result<(), SaveError> {
        let registry = world.remove_resource::<SaveRegistry>().unwrap_or_default();
        let result = self.restore_with(world, &registry);
        world.insert_resource(registry);
        result
    }

    fn restore_with(&self, world: &mut World, registry: &SaveRegistry) -> Result<(), SaveError> {
        let mut remap = EntityRemap::new();
        for saved in &self.kinds {
            let Some(kind) = registry.kinds.iter().find(|k| k.label == saved.kind) else {
                return Err(SaveError::Decode(format!("the save holds {} entities, a kind this game does not save", saved.kind)));
            };
            for (id, entry) in &saved.entries {
                let entity = (kind.restore)(world, entry)?;
                remap.bind(*id, entity);
            }
        }
        for (id, state) in &self.entities {
            if let Some(entity) = remap.entity(*id) {
                state.restore(world, entity, &remap);
            }
        }
        for (label, saved) in &self.state {
            let Some(entry) = registry.states.iter().find(|s| s.label == label) else {
                return Err(SaveError::Decode(format!("the save holds a {label}, a resource this game does not save")));
            };
            (entry.restore)(world, saved)?;
        }
        self.engine.restore(world, &remap);
        Ok(())
    }

    /// What was saved of the resource `R`, read before restoring, for the
    /// part of a game's start that comes before the world exists: the
    /// world's size, say.
    #[cfg(test)]
    pub(crate) fn state<R: SaveableState>(&self) -> Option<R::Saved> {
        let label = short_name::<R>();
        self.state.iter().find(|(l, _)| l == label).and_then(|(_, raw)| raw.into_rust().ok())
    }

    /// How many of `kind`, a [`Saveable`] or a [`SaveableItem`], were
    /// saved, for a line in the log.
    #[cfg(test)]
    pub(crate) fn count_of<K: Component>(&self) -> usize {
        let label = short_name::<K>();
        self.kinds.iter().find(|k| k.kind == label).map_or(0, |k| k.entries.len())
    }

    /// The whole turn the run was saved on.
    pub fn turn(&self) -> u32 {
        self.engine.now / atglyph_core::turn::BASE_ACTION_COST
    }
}

/// The slot a game's runs are saved to, and the version its shape is at.
#[derive(Resource, Debug, Clone)]
pub struct SaveSlot {
    /// The slot's name.
    pub slot: String,
    /// The game's version of its kinds' shapes, matched exactly.
    pub version: u32,
}

/// Saving a run and continuing it, and everything else a game keeps
/// between launches, owned by the engine.
///
/// Inserts [`Saves`], the backend, unless the game inserted its own: the
/// platform's default under the game's name, files in a directory of the
/// game's own beside the executable or browser storage, or memory when the run is
/// [`scripted`](atglyph_bevy::replay::scripted), a replay, a recording or a
/// capture, which plays keys against a seed of its own and so must neither
/// start on the player's save nor write over it. Keeps the [`Stash`] a
/// turn behind the run and writes it when the game is left, the page
/// closed or the window's button pressed, so a run left without saying so
/// is saved; and deletes the slot when the run ends or is given up, so
/// nothing is continued past its end. With
/// [`on_arrival`](Self::on_arrival) it also writes the slot whenever the
/// player arrives somewhere. Registers no key: a game's save key calls
/// [`save_run`].
///
/// For a title: inserts [`SavedRun`], which is how a title knows there is
/// a slot to continue from at all, reads the slot into it before the first
/// frame and keeps it current as the slot is written and deleted, writes a
/// run left with [`LeaveRun`] down rather than forgetting it with its
/// teardown, and forgets the slot the moment [`AbandonRun`] is triggered.
///
/// Apart from the run, in slots of their own that no run's ending forgets:
/// what the player chose in the [settings](crate::settings), and, in a
/// game that keeps them with `RecordsPlugin`, every run's
/// [record](crate::records).
pub struct SavePlugin {
    slot: String,
    version: u32,
    on_arrival: bool,
}

impl SavePlugin {
    /// Saves the game called `name`: its run to the slot `name`, at
    /// version one, in the player's saves of that name, which name its
    /// directory on the desktop and its browser storage on the web.
    ///
    /// # Panics
    ///
    /// When `name` is not a plain name, ASCII letters, digits, `-` and
    /// `_`, since it names the run's slot and the game's save directory;
    /// or when it is `settings` or [`RECORDS_SLOT`](crate::RECORDS_SLOT),
    /// the slots the run shares its saves with, so a game named like one
    /// would write its run over them. A game's name is a constant, so
    /// either is the game's setup gone wrong, said here rather than at the
    /// first save.
    pub fn new(name: impl Into<String>) -> Self {
        let slot = name.into();
        if let Err(e) = crate::backend::check_slot(&slot) {
            panic!("a game's saves are named by a plain name: {e}");
        }
        assert!(
            slot != crate::settings::SETTINGS_SLOT && slot != crate::RECORDS_SLOT,
            "a game cannot be called {slot:?}: its run would be saved over the engine's own slot of that name"
        );
        Self { slot, version: 1, on_arrival: false }
    }

    /// Where a run that is, or is not, `scripted` saves, when the game
    /// inserted no [`Saves`] of its own.
    fn saves(&self, scripted: bool) -> Saves {
        if scripted { Saves::new(crate::backend::MemoryBackend::default()) } else { Saves::platform_default(&self.slot) }
    }

    /// The game's version of its saved shapes. Bump it, and say why next to
    /// the call, when an old save would parse wrongly.
    pub fn version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }

    /// Also writes the run to the slot on every frame in which the player
    /// arrived somewhere, so each place entered is a save point and a
    /// crash loses at most the place in hand. Off unless asked: a game
    /// without places, or one that saves on a key, is unchanged.
    pub fn on_arrival(mut self) -> Self {
        self.on_arrival = true;
        self
    }
}

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<Saves>() {
            app.insert_resource(self.saves(atglyph_bevy::replay::scripted()));
        }
        app.init_resource::<SaveRegistry>()
            .save_kind::<PropKind>()
            .save_kind::<Stair>()
            .init_resource::<Stash>()
            .init_resource::<Leaving>()
            // Empty until `finish` reads the slot, and there from the
            // start, even without a backend, since it says a slot exists.
            .init_resource::<SavedRun>()
            .insert_resource(SaveSlot { slot: self.slot.clone(), version: self.version })
            .reads::<LeaveRun>()
            // Exclusive, so it conflicts with everything else in `Last`; the
            // engine's own are ordered against it by `EndOfFrame`, and what
            // is left unordered is Bevy's bookkeeping, the frame count and
            // the like, which reads nothing a save writes.
            .add_systems(Last, refresh_stash.in_set(EndOfFrame::Save).ambiguous_with_all())
            // Exclusive too, and ambiguous with Bevy's bookkeeping for the
            // same reason; in the save stage, so the run is written down
            // before the restart stage tears it down.
            .add_systems(Last, (note_leaving, save_on_leaving.ambiguous_with_all()).chain().in_set(EndOfFrame::Save).before(refresh_stash))
            .add_systems(EndRun, forget_save.run_if(given_up))
            .add_systems(OnEnter(EngineState::Over), forget_save)
            .add_observer(forget_abandoned);
        if self.on_arrival {
            // Exclusive too, and ambiguous with Bevy's bookkeeping for the
            // same reason as the stash.
            app.init_resource::<ArrivedThisFrame>()
                .add_systems(Last, (note_arrivals, save_on_arrival.ambiguous_with_all()).chain().in_set(EndOfFrame::Save).before(refresh_stash));
        }
        crate::unload::build(app);
        crate::settings::build(app);
    }

    /// Reads what the slot holds into [`SavedRun`] once every plugin has
    /// built and before the first frame, so a title's first look at it,
    /// and the first key read against it, are the truth. A slot this build
    /// cannot read is left where it is, for a new run to ask about. Then
    /// the settings and the records are recalled, and on the web the
    /// page's handler for the stash is installed.
    fn finish(&self, app: &mut App) {
        let world = app.world_mut();
        // Without a backend there is nothing to read, and play beginning
        // says what is missing.
        if !world.contains_resource::<Saves>() {
            return;
        }
        let saved = match load_run(world) {
            Ok(Some(_)) => SavedRun::Readable,
            Ok(None) => SavedRun::Empty,
            Err(e) => {
                warn!("the saved run cannot be read, so it cannot be continued: {e}");
                SavedRun::Unreadable
            }
        };
        world.insert_resource(saved);
        crate::settings::finish(app);
        crate::records::finish(app);
        crate::unload::finish(app);
    }
}

/// Whether a run still in play is left for the title this frame: written
/// down in [`EndOfFrame::Save`] and kept through the teardown that follows,
/// rather than forgotten with it as a run given up is.
#[derive(Resource, Default)]
struct Leaving(bool);

/// Notes a [`LeaveRun`] that leaves a run still in play: playing, and with
/// no [`Ending`], which a run that ended this frame has before the state
/// leaves playing. A run that is over has nothing to continue, and its
/// teardown forgets its save as a restart's does.
///
/// A reader of its own, as [`note_arrivals`] is, and assigned every frame,
/// so a leaving is forgotten by the next.
fn note_leaving(mut left: MessageReader<LeaveRun>, state: Res<State<EngineState>>, ending: Option<Res<Ending>>, mut leaving: ResMut<Leaving>) {
    leaving.0 = left.read().count() > 0 && *state.get() == EngineState::Playing && ending.is_none();
}

/// Writes the run down as it stands when it is left for the title, so
/// Continue picks it up where it was left.
///
/// A save of its own rather than the [`Stash`], which follows the run a
/// whole turn at a time: a quick player's half turn, or anything that took
/// no time, would be lost between leaving and continuing. Stashed as well,
/// by [`save_run`], so a window closed at the title writes the same run
/// back. A save that fails is logged, and the slot keeps what it held.
pub(crate) fn save_on_leaving(world: &mut World) {
    if !world.resource::<Leaving>().0 {
        return;
    }
    if let Err(e) = save_run(world) {
        error!("the run could not be written down as it was left: {e}");
    }
}

/// Whether the run torn down this frame was given up rather than left for
/// the title: only a run given up is forgotten with its teardown.
fn given_up(leaving: Res<Leaving>) -> bool {
    !leaving.0
}

/// Forgets the saved run when a title abandons it for a new one. An
/// observer, so the slot is gone before the trigger returns and the run
/// the title begins in the same command never sees it.
fn forget_abandoned(_: On<AbandonRun>, saves: Res<Saves>, slot: Res<SaveSlot>, stash: Res<Stash>, saved: ResMut<SavedRun>) {
    forget_save(saves, slot, stash, saved);
}

/// Whether the frame held an arrival, for `save_on_arrival`.
#[derive(Resource, Default)]
struct ArrivedThisFrame(bool);

/// Notes an arrival: a reader of its own rather than a cursor inside the
/// exclusive system that writes the save, so the message is read the way
/// every other reader reads it.
fn note_arrivals(mut entered: MessageReader<PlaceEntered>, mut arrived: ResMut<ArrivedThisFrame>) {
    arrived.0 |= entered.read().count() > 0;
}

/// Writes the run once for a frame that held an arrival, while playing.
///
/// At the end of the frame, in [`EndOfFrame::Save`], because by then the
/// warp and whatever the place was filled with have all landed, and before
/// a restart could tear the run down. Not once the run is over: the end
/// deleted the save, and writing it back would let the run be continued
/// past it. A save that fails is logged and the run goes on, as a stash
/// that fails is.
pub(crate) fn save_on_arrival(world: &mut World) {
    if !std::mem::take(&mut world.resource_mut::<ArrivedThisFrame>().0) {
        return;
    }
    if *world.resource::<State<EngineState>>().get() != EngineState::Playing {
        return;
    }
    if let Err(e) = save_run(world) {
        error!("the run could not be saved on arrival: {e}");
    }
}

/// Writes the run to the slot, and stashes it for the bridge that writes
/// on the way out. [`SavedRun`] says the slot holds a run once it does.
pub fn save_run(world: &mut World) -> Result<(), SaveError> {
    let text = encode_run(world)?;
    let slot = world.resource::<SaveSlot>().slot.clone();
    world.resource::<Saves>().persist(&slot, &text)?;
    world.resource::<Stash>().stash(&slot, &text);
    world.insert_resource(SavedRun::Readable);
    Ok(())
}

/// Reads the slot, if there is a save this build can read: the game's
/// version and the engine's format both matched.
pub fn load_run(world: &World) -> Result<Option<RunSave>, SaveError> {
    let slot = world.resource::<SaveSlot>();
    let Some(text) = world.resource::<Saves>().load(&slot.slot)? else { return Ok(None) };
    let save: RunSave = decode(slot.version, &text)?;
    if save.format != RunSave::FORMAT {
        return Err(SaveError::Version { found: save.format, expected: RunSave::FORMAT });
    }
    Ok(Some(save))
}

/// The run as one versioned blob.
fn encode_run(world: &mut World) -> Result<String, SaveError> {
    let save = RunSave::capture(world)?;
    let version = world.resource::<SaveSlot>().version;
    encode(version, &save)
}

/// Keeps the [`Stash`] one turn behind the run at most, so a tab or a
/// window closed on the run loses no more than the turn in hand.
///
/// Once a turn rather than once a frame: the run is only different after
/// a turn, and encoding it is the whole of the cost.
pub(crate) fn refresh_stash(world: &mut World, mut last: Local<Option<u32>>) {
    if *world.resource::<State<EngineState>>().get() != EngineState::Playing {
        return;
    }
    let turn = world.resource::<Turns>().turn_number();
    if *last == Some(turn) {
        return;
    }
    *last = Some(turn);
    match encode_run(world) {
        Ok(text) => {
            let slot = world.resource::<SaveSlot>().slot.clone();
            world.resource::<Stash>().stash(&slot, &text);
        }
        Err(e) => warn!("the run could not be stashed: {e}"),
    }
}

/// Deletes the slot and clears the stash: the run is over, or given up,
/// and continuing it would be continuing past its end. [`SavedRun`] says
/// the slot is empty once it is; a delete that failed leaves the slot, and
/// what it says, as they were.
pub(crate) fn forget_save(saves: Res<Saves>, slot: Res<SaveSlot>, stash: Res<Stash>, mut saved: ResMut<SavedRun>) {
    stash.clear();
    match saves.delete(&slot.slot) {
        Ok(()) => *saved = SavedRun::Empty,
        Err(e) => error!("could not delete the save: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{MemoryBackend, SaveBackend};
    use atglyph_bevy::prelude::*;
    use atglyph_bevy::{FovPlugin, ItemsPlugin};
    use atglyph_core::RunSeed;
    use atglyph_rules::{EquipShape, SlotDef, StatusDef};

    /// A game's kinds: a person by name, a thing by name and count of
    /// notches, and a marker for the one that is the player.
    #[derive(Component)]
    struct Person(String);

    impl Saveable for Person {
        type Saved = String;
        fn capture(world: &World, entity: Entity) -> String {
            world.get::<Person>(entity).unwrap().0.clone()
        }
        fn restore(world: &mut World, saved: &String) -> Entity {
            world.spawn((Actor, Blocks, Person(saved.clone()), Health::full(20))).id()
        }
    }

    #[derive(Component)]
    struct Thing {
        def: String,
        notches: u8,
    }

    impl Saveable for Thing {
        type Saved = (String, u8);
        fn capture(world: &World, entity: Entity) -> (String, u8) {
            let t = world.get::<Thing>(entity).unwrap();
            (t.def.clone(), t.notches)
        }
        fn restore(world: &mut World, (def, notches): &(String, u8)) -> Entity {
            let mut e = world.spawn((Item, Thing { def: def.clone(), notches: *notches }));
            if def == "ring" {
                e.insert(Wearable(EquipShape::in_slot(atglyph_rules::SlotId::from_raw(0))));
            }
            if def == "coin" {
                e.insert(Stack { key: 1, count: 1 });
            }
            e.id()
        }
    }

    #[derive(Component)]
    struct You;

    impl Saveable for You {
        type Saved = ();
        fn capture(_: &World, _: Entity) {}
        fn restore(world: &mut World, _: &()) -> Entity {
            world.spawn((Actor, Player, Blocks, You, Viewshed::new(6), RevealsMap, Health::full(30))).id()
        }
    }

    #[derive(Resource, Default)]
    struct Stocked(Vec<u32>);

    impl SaveableState for Stocked {
        type Saved = Vec<u32>;
        fn capture(&self) -> Vec<u32> {
            self.0.clone()
        }
        fn restore(&mut self, saved: Vec<u32>) {
            self.0 = saved;
        }
    }

    fn game(saves: Saves) -> (App, Point) {
        game_with(saves, SavePlugin::new("run").version(3))
    }

    fn game_with(saves: Saves, plugin: SavePlugin) -> (App, Point) {
        let mut app = atglyph_bevy::testing::headless_app();
        app.add_plugins((FovPlugin, CombatPlugin, ItemsPlugin, RemainsPlugin, atglyph_bevy::PropsPlugin));
        atglyph_bevy::AddWork::add_work(&mut app, "mending");
        let start = atglyph_bevy::testing::ground(&mut app);
        atglyph_bevy::testing::two_sides(&mut app);
        {
            let mut registries = app.world_mut().resource_mut::<Registries>();
            registries.slots = atglyph_rules::Registry::from_defs(vec![SlotDef::new("finger")]).unwrap();
            registries.statuses =
                atglyph_rules::Registry::from_defs(vec![StatusDef::new("dazed"), StatusDef::new("turned").sides_with(atglyph_rules::FactionId::from_raw(1))])
                    .unwrap();
        }
        app.insert_resource(saves).insert_resource(Seed(RunSeed(9))).init_resource::<Stocked>();
        app.add_plugins(plugin).save_kind::<Thing>().save_kind::<Person>().save_kind::<You>().save_state::<Stocked>();
        app.finish();
        app.cleanup();
        (app, start)
    }

    fn play(app: &mut App) {
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
    }

    /// A game's item definition, which the engine never reads.
    struct Blade(&'static str);

    impl atglyph_rules::Named for Blade {
        fn name(&self) -> &str {
            self.0
        }
    }

    /// A game's item maker, making every blade from one set of parts that
    /// takes a level: a damage a level on a blow of 1d6.
    #[derive(Resource)]
    struct Blades {
        defs: atglyph_rules::Registry<Blade>,
        parts: std::sync::Arc<atglyph_bevy::ItemParts>,
        table: atglyph_rules::LootTable<Id<Blade>>,
    }

    impl Blades {
        fn new() -> Self {
            let blow =
                atglyph_rules::parts::Melee { roll: atglyph_core::DiceRoll::new(1, 6), kind: Id::from_raw(0).into(), look: None, shape: None, unaware: None };
            let parts = atglyph_bevy::ItemParts {
                attacks: atglyph_bevy::Attacks { melee: Some(blow), ..Default::default() },
                level: Some(atglyph_rules::parts::Level { damage: Some(1), ..Default::default() }),
                ..Default::default()
            };
            let defs = atglyph_rules::Registry::from_defs(vec![Blade("knife"), Blade("sabre")]).unwrap();
            Self { defs, parts: std::sync::Arc::new(parts), table: atglyph_rules::LootTable::default() }
        }
    }

    impl ItemMaker for Blades {
        type Def = Blade;
        fn make(&self, _: &mut Commands, _: &Registries, _: Id<Blade>, _: u32, _: atglyph_bevy::Provenance, _: &mut rand::rngs::StdRng) -> Vec<Entity> {
            Vec::new()
        }
        fn make_at(&self, commands: &mut Commands, _: &Registries, def: Id<Blade>, level: i32) -> Entity {
            let mut e = commands.spawn(BladeKind(def));
            atglyph_bevy::insert_parts(&mut e, &self.parts, level);
            e.id()
        }
        fn id_of(&self, name: &str) -> Option<Id<Blade>> {
            self.defs.id(name)
        }
        fn name_of(&self, def: Id<Blade>) -> &str {
            self.defs.name(def)
        }
        fn table(&self) -> &atglyph_rules::LootTable<Id<Blade>> {
            &self.table
        }
        fn band(&self, _: atglyph_bevy::LootArea) -> i32 {
            0
        }
    }

    /// The game's marker of what a blade was made from.
    #[derive(Component)]
    struct BladeKind(Id<Blade>);

    /// What the game keeps of a blade beyond its level, and the level the
    /// blade stood at when that was put back on it.
    #[derive(Component, Debug, PartialEq, Eq)]
    struct Notched {
        notches: u8,
        put_back_at: Option<i32>,
    }

    impl SaveableItem for BladeKind {
        type Maker = Blades;
        type Extra = u8;
        fn def(&self) -> Id<Blade> {
            self.0
        }
        fn capture_extra(world: &World, entity: Entity) -> u8 {
            world.get::<Notched>(entity).map_or(0, |n| n.notches)
        }
        fn restore_extra(world: &mut World, entity: Entity, notches: &u8) {
            let put_back_at = world.get::<ItemLevel>(entity).map(|l| l.0);
            world.entity_mut(entity).insert(Notched { notches: *notches, put_back_at });
        }
    }

    /// An item saved as its definition and level comes back made by the
    /// game's maker at that level, its blow a level's worth stronger and
    /// never two, and the game's own extra goes back on after, onto a thing
    /// already at its level.
    #[test]
    fn an_item_is_saved_as_its_definition_and_level_and_its_extra_is_put_back_after_the_level() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game(Saves(backend.clone()));
        app.insert_resource(Blades::new()).save_items::<BladeKind>();
        let sabre = app.world().resource::<Blades>().defs.expect("sabre");
        let made = {
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let world = app.world();
            let mut commands = Commands::new(&mut queue, world);
            let made = world.resource::<Blades>().make_at(&mut commands, &Registries::default(), sabre, 2);
            commands.entity(made).insert((Position(start.offset(1, 0)), OnMap(MapId::ZERO), Notched { notches: 3, put_back_at: None }));
            queue.apply(app.world_mut());
            made
        };
        app.world_mut().spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30)));
        play(&mut app);
        assert_eq!(app.world().get::<MeleeAttack>(made).map(|m| m.dice.bonus), Some(2));
        save_run(app.world_mut()).unwrap();

        let (mut back, _) = game(Saves(backend));
        back.insert_resource(Blades::new()).save_items::<BladeKind>();
        let save = load_run(back.world()).unwrap().expect("a save");
        assert_eq!(save.count_of::<BladeKind>(), 1);
        save.restore(back.world_mut()).unwrap();
        let w = back.world_mut();
        let found: Vec<(Option<i32>, Option<i32>, Option<&Notched>)> = w
            .query_filtered::<(Option<&ItemLevel>, Option<&MeleeAttack>, Option<&Notched>), With<BladeKind>>()
            .iter(w)
            .map(|(level, blow, notched)| (level.map(|l| l.0), blow.map(|b| b.dice.bonus), notched))
            .collect();
        assert_eq!(found, vec![(Some(2), Some(2), Some(&Notched { notches: 3, put_back_at: Some(2) }))]);
    }

    /// A game's monster definition, which the engine never reads.
    struct Beast(&'static str);

    impl atglyph_rules::Named for Beast {
        fn name(&self) -> &str {
            self.0
        }
    }

    /// A game's actor maker, making every beast from its own parts: twelve
    /// health, on the far side.
    #[derive(Resource)]
    struct Beasts {
        defs: atglyph_rules::Registry<Beast>,
        parts: atglyph_bevy::ActorParts<Blade>,
        table: atglyph_rules::BandedTable<Id<Beast>>,
    }

    impl Beasts {
        fn new() -> Self {
            let parts = atglyph_bevy::ActorParts { hp: Some(12), faction: Some(atglyph_rules::FactionId::from_raw(1)), ..Default::default() };
            Self { defs: atglyph_rules::Registry::from_defs(vec![Beast("hound"), Beast("crow")]).unwrap(), parts, table: atglyph_rules::BandedTable::default() }
        }
    }

    impl ActorMaker for Beasts {
        type Def = Beast;
        fn make_at(&self, commands: &mut Commands, registries: &Registries, def: Id<Beast>, at: Point, map: MapId) -> Entity {
            let mut e = commands.spawn((Actor, Blocks, Position(at), OnMap(map), BeastKind(def)));
            self.parts.insert(&mut e, registries);
            e.id()
        }
        fn id_of(&self, name: &str) -> Option<Id<Beast>> {
            self.defs.id(name)
        }
        fn name_of(&self, def: Id<Beast>) -> &str {
            self.defs.name(def)
        }
        fn table(&self) -> &atglyph_rules::BandedTable<Id<Beast>> {
            &self.table
        }
        fn band(&self, _: MapId) -> i32 {
            0
        }
    }

    /// The game's marker of what a beast was made from.
    #[derive(Component)]
    struct BeastKind(Id<Beast>);

    /// What the game keeps of a beast beyond its definition: the side it
    /// was turned to, and the side it stood on when that was put back.
    #[derive(Component, Debug, PartialEq, Eq)]
    struct Turned {
        to: u32,
        was: Option<atglyph_rules::FactionId>,
    }

    impl SaveableActor for BeastKind {
        type Maker = Beasts;
        type Extra = Option<u32>;
        fn def(&self) -> Id<Beast> {
            self.0
        }
        fn capture_extra(world: &World, entity: Entity) -> Option<u32> {
            world.get::<Turned>(entity).map(|t| t.to)
        }
        fn restore_extra(world: &mut World, entity: Entity, turned: &Option<u32>) {
            if let Some(to) = *turned {
                let was = world.get::<atglyph_bevy::Faction>(entity).map(|f| f.0);
                world.entity_mut(entity).insert(Turned { to, was });
            }
        }
    }

    /// An actor saved as its definition comes back made by the game's
    /// maker, with the health it had left put back by the engine, and the
    /// game's own extra goes back on after, onto an actor already made as
    /// its kind.
    #[test]
    fn an_actor_is_saved_as_its_definition_and_its_extra_is_put_back_after_it_is_made() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game(Saves(backend.clone()));
        app.insert_resource(Beasts::new()).save_actors::<BeastKind>();
        let crow = app.world().resource::<Beasts>().defs.expect("crow");
        let made = {
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let world = app.world();
            let mut commands = Commands::new(&mut queue, world);
            let made = world.resource::<Beasts>().make_at(&mut commands, world.resource::<Registries>(), crow, start.offset(3, 0), MapId::ZERO);
            commands.entity(made).insert(Turned { to: 7, was: None });
            queue.apply(app.world_mut());
            made
        };
        app.world_mut().get_mut::<Health>(made).unwrap().current = 5;
        app.world_mut().spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30)));
        play(&mut app);
        save_run(app.world_mut()).unwrap();

        let (mut back, _) = game(Saves(backend));
        back.insert_resource(Beasts::new()).save_actors::<BeastKind>();
        let save = load_run(back.world()).unwrap().expect("a save");
        assert_eq!(save.count_of::<BeastKind>(), 1);
        save.restore(back.world_mut()).unwrap();
        let w = back.world_mut();
        let found: Vec<_> = w
            .query::<(&BeastKind, &Position, &Health, Option<&Turned>)>()
            .iter(w)
            .map(|(kind, at, health, turned)| (kind.0, at.0, (health.current, health.max), turned))
            .collect();
        assert_eq!(found, vec![(crow, start.offset(3, 0), (5, 12), Some(&Turned { to: 7, was: Some(atglyph_rules::FactionId::from_raw(1)) }))]);
    }

    /// The engine's own kind: a game writes down nothing about its props
    /// and gets them back whole, where they stood, holding what they held,
    /// with a sprung trap still sprung and a found plate still found.
    #[test]
    fn props_are_saved_by_the_engine_with_each_ones_own_history() {
        const PROPS: &str = r#"#![enable(implicit_some)]
            [
                (name: "supply crate", glyph: '&', color: (r: 190, g: 165, b: 115), blocks: true,
                 container: (contents: [(item: "coin", count: 1)]), offers: [(verb: "open", time: 200)]),
                (name: "pressure plate", glyph: '^', color: (r: 230, g: 140, b: 51),
                 hidden: (spot: 40), triggers: [(on: "entered", fires: 2, effects: [(kind: "Teleport")])]),
            ]"#;

        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game(Saves(backend.clone()));
        atglyph_bevy::AddEngineEffects::add_engine_effects(&mut app);
        let props = atglyph_rules::prop::load(PROPS, &atglyph_rules::Names::new()).expect("the props load");
        app.world_mut().resource_mut::<Registries>().props = props.clone();
        let (crate_id, plate_id) = (props.expect("supply crate"), props.expect("pressure plate"));

        let coin = app.world_mut().spawn((Item, Thing { def: "coin".into(), notches: 0 }, Stack { key: 1, count: 5 })).id();
        let registries = app.world().resource::<Registries>().clone();
        let (chest, plate) = {
            let mut commands = app.world_mut().commands();
            (
                atglyph_bevy::spawn_prop(&mut commands, &registries, crate_id, start.offset(1, 0), MapId::ZERO),
                atglyph_bevy::spawn_prop(&mut commands, &registries, plate_id, start.offset(0, 1), MapId::ZERO),
            )
        };
        app.world_mut().flush();
        // `Stocked` here is the test's own saved resource, so the prop's is named in full.
        app.world_mut().entity_mut(chest).insert((Inventory { items: vec![coin] }, atglyph_bevy::Stocked));
        app.world_mut().spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30)));
        play(&mut app);
        // One of the plate's two firings spent, and the player has found it.
        app.world_mut().get_mut::<atglyph_bevy::Triggers>(plate).expect("the plate is armed").0[0].fires = Some(1);
        app.world_mut().entity_mut(plate).remove::<Hidden>();
        save_run(app.world_mut()).unwrap();

        let (mut back, _) = game(Saves(backend));
        atglyph_bevy::AddEngineEffects::add_engine_effects(&mut back);
        back.world_mut().resource_mut::<Registries>().props = props;
        let save = load_run(back.world()).unwrap().expect("a save");
        assert_eq!(save.count_of::<PropKind>(), 2, "both props were written down, by the engine");
        save.restore(back.world_mut()).unwrap();
        play(&mut back);

        let w = back.world_mut();
        let mut found: Vec<(String, Point, u32, bool)> = w
            .query::<(&PropKind, &Position, Option<&atglyph_bevy::Triggers>, Option<&Hidden>)>()
            .iter(w)
            .map(|(kind, at, triggers, hidden)| {
                let left = triggers.and_then(|t| t.0.first()).and_then(|t| t.fires).unwrap_or_default();
                (w.resource::<Registries>().props.name(kind.0).to_string(), at.0, left, hidden.is_some())
            })
            .collect();
        found.sort();
        assert_eq!(
            found,
            vec![("pressure plate".to_string(), start.offset(0, 1), 1, false), ("supply crate".to_string(), start.offset(1, 0), 0, false)],
            "each prop is back where it stood, with its own history: one firing left of two, and no longer hidden"
        );

        let w = back.world_mut();
        let held: Vec<u32> = w
            .query_filtered::<&Inventory, With<PropKind>>()
            .iter(w)
            .flat_map(|bag| bag.items.clone())
            .filter_map(|item| w.get::<Stack>(item).map(|s| s.count))
            .collect();
        assert_eq!(held, vec![5], "and the crate still holds the coins it held");
    }

    /// Remains a game dressed as a prop of its own come back as that
    /// prop and as what they were, once: saved as their own kind, with the
    /// prop kind part of what the engine records of the remains.
    #[test]
    fn remains_dressed_as_a_prop_come_back_as_that_prop_and_only_once() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game(Saves(backend.clone()));
        let props =
            atglyph_rules::prop::load(r#"[(name: "wreck", glyph: '%', color: (r: 1, g: 2, b: 3))]"#, &atglyph_rules::Names::new()).expect("the props load");
        app.world_mut().resource_mut::<Registries>().props = props;
        let wreck = app.world().resource::<Registries>().props.expect("wreck");
        let me = app.world_mut().spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30))).id();
        let ada = app.world_mut().spawn((Actor, Blocks, Person("Ada".into()), Position(start.offset(0, 3)), Health::full(20), LeavesRemains)).id();
        play(&mut app);
        let kind = app.world().resource::<Registries>().damage_kinds.expect("kinetic");
        app.world_mut().write_message(DamageEvent::new(ada, atglyph_rules::Hit::by(me, kind, 99)));
        app.update();
        app.update();
        // What a game does with what is left: a prop of its own.
        app.world_mut().entity_mut(ada).insert(PropKind(wreck));
        save_run(app.world_mut()).unwrap();

        let (mut back, _) = game(Saves(backend));
        back.world_mut().resource_mut::<Registries>().props = app.world().resource::<Registries>().props.clone();
        load_run(back.world()).unwrap().expect("a save").restore(back.world_mut()).unwrap();
        play(&mut back);
        let w = back.world_mut();
        let dressed: Vec<(bool, bool)> = w.query::<(Has<Remains>, Has<Person>, &PropKind)>().iter(w).map(|(r, p, _)| (r, p)).collect();
        assert_eq!(dressed, vec![(true, true)], "one wreck, and it is still Ada's remains");
    }

    /// A save that would write one entity down as two kinds is refused,
    /// not written: restoring it would spawn the thing twice.
    #[test]
    fn an_entity_two_kinds_claim_is_refused_rather_than_saved_twice() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game(Saves(backend.clone()));
        app.world_mut().spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30)));
        app.world_mut().spawn((Person("both".into()), Thing { def: "ring".into(), notches: 0 }));
        play(&mut app);
        let err = save_run(app.world_mut()).expect_err("two kinds, one entity");
        assert!(err.to_string().contains("Person") && err.to_string().contains("Thing"), "names both kinds: {err}");
        assert!(!backend.exists("run"), "and nothing was written");
    }

    /// A body saved is a body continued: the game writes down a person,
    /// as it always did, and the engine is what remembers that this one
    /// is lying dead on the floor, so nothing about a game's own record
    /// of its kinds has to learn the word.
    #[test]
    fn remains_are_still_remains_when_the_run_is_continued() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game(Saves(backend.clone()));
        let me = app.world_mut().spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30))).id();
        let ada = app.world_mut().spawn((Actor, Blocks, Person("Ada".into()), Position(start.offset(0, 3)), Health::full(20), LeavesRemains)).id();
        play(&mut app);
        let kind = app.world().resource::<Registries>().damage_kinds.expect("kinetic");
        app.world_mut().write_message(DamageEvent::new(ada, atglyph_rules::Hit::by(me, kind, 99)));
        app.update();
        app.update();
        let since = app.world().get::<Remains>(ada).expect("Ada is remains").since;
        save_run(app.world_mut()).unwrap();

        let (mut back, _) = game(Saves(backend));
        load_run(back.world()).unwrap().expect("a save").restore(back.world_mut()).unwrap();
        play(&mut back);
        let w = back.world_mut();
        let (ada2, at) = w.query_filtered::<(Entity, &Position), With<Person>>().single(w).map(|(e, p)| (e, p.0)).expect("Ada came back");
        let w = back.world();
        assert_eq!(w.get::<Remains>(ada2).map(|r| r.since), Some(since), "still remains, dead on the same turn");
        assert_eq!(at, start.offset(0, 3), "lying where she fell");
        assert!(w.get::<Health>(ada2).is_none(), "and not brought back to life by her own record of herself");
        assert!(w.get::<Actor>(ada2).is_none(), "nor dealt turns again");
        assert!(w.get::<atglyph_bevy::remains::Life>(ada2).is_some(), "and she keeps a life to return to, taken as she was laid down again");
    }

    /// A body continued from a save stands up as surely as one that just
    /// fell, from the living thing the game's own record respawned.
    #[test]
    fn a_body_continued_from_a_save_can_be_revived() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game(Saves(backend.clone()));
        let me = app.world_mut().spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30))).id();
        let ada = app.world_mut().spawn((Actor, Blocks, Person("Ada".into()), Position(start.offset(0, 3)), Health::full(20), LeavesRemains)).id();
        play(&mut app);
        let kind = app.world().resource::<Registries>().damage_kinds.expect("kinetic");
        app.world_mut().write_message(DamageEvent::new(ada, atglyph_rules::Hit::by(me, kind, 99)));
        app.update();
        app.update();
        save_run(app.world_mut()).unwrap();

        let (mut back, _) = game(Saves(backend));
        load_run(back.world()).unwrap().expect("a save").restore(back.world_mut()).unwrap();
        play(&mut back);
        let w = back.world_mut();
        let ada2 = w.query_filtered::<Entity, With<Person>>().single(w).expect("Ada came back");
        assert!(atglyph_bevy::remains::revive(back.world_mut(), ada2, 10), "and can be stood up");
        back.update();
        assert_eq!(back.world().get::<Health>(ada2).map(|h| h.current), Some(10));
        assert!(back.world().get::<Actor>(ada2).is_some(), "an actor again");
    }

    /// Work saved is work continued, on the same target and as far along;
    /// work on a target that was not saved is dropped, since there is
    /// nothing left to work on.
    #[test]
    fn work_saved_is_work_continued_and_work_on_something_unsaved_is_dropped() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game(Saves(backend.clone()));
        app.world_mut().spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30)));
        let kind = app.world().resource::<atglyph_bevy::WorkKinds>().get("mending").unwrap();
        let bo = app.world_mut().spawn((Actor, Blocks, Person("Bo".into()), Position(start.offset(1, 3)), Health::full(20))).id();
        let loose = app.world_mut().spawn(Position(start.offset(4, 4))).id();
        let mut work = atglyph_rules::Work::new(kind, 9).on(bo);
        work.done = 4;
        let ada =
            app.world_mut().spawn((Actor, Blocks, Person("Ada".into()), Position(start.offset(0, 3)), Health::full(20), atglyph_bevy::Working(work))).id();
        app.world_mut().spawn((
            Actor,
            Blocks,
            Person("Cy".into()),
            Position(start.offset(4, 3)),
            Health::full(20),
            atglyph_bevy::Working(atglyph_rules::Work::new(kind, 9).on(loose)),
        ));
        play(&mut app);
        // However far play carried it, what comes back is what was saved.
        let saved = app.world().get::<atglyph_bevy::Working>(ada).expect("Ada is at work when saved").0;
        save_run(app.world_mut()).unwrap();

        let (mut back, _) = game(Saves(backend));
        load_run(back.world()).unwrap().expect("a save").restore(back.world_mut()).unwrap();
        let w = back.world_mut();
        let people: Vec<(Entity, String)> = w.query::<(Entity, &Person)>().iter(w).map(|(e, p)| (e, p.0.clone())).collect();
        let who = |name: &str| people.iter().find(|(_, n)| n == name).unwrap().0;
        let w = back.world();
        let ada = w.get::<atglyph_bevy::Working>(who("Ada")).expect("Ada is still at work").0;
        assert_eq!((ada.kind, ada.target, ada.done, ada.needed), (kind, Some(who("Bo")), saved.done, 9), "on Bo, as far in as when saved");
        assert!(w.get::<atglyph_bevy::Working>(who("Cy")).is_none(), "Cy's work was on something that was not saved");
    }

    /// The player's own long action is saved as anyone's work is, and a
    /// run loaded in the middle of one carries it on with no key pressed.
    #[test]
    fn a_run_saved_while_the_player_works_carries_the_work_on_when_loaded() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game(Saves(backend.clone()));
        let kind = app.world().resource::<atglyph_bevy::WorkKinds>().get("mending").unwrap();
        let mut work = atglyph_rules::Work::new(kind, 6);
        work.done = 2;
        let me = app
            .world_mut()
            .spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30), atglyph_bevy::Working(work)))
            .id();
        // Slow enough that no turn of it passes before the save.
        app.insert_resource(atglyph_bevy::WorkPace::every(std::time::Duration::from_secs(3600)));
        play(&mut app);
        let saved = app.world().get::<atglyph_bevy::Working>(me).expect("at work when saved").0;
        save_run(app.world_mut()).unwrap();

        let (mut back, _) = game(Saves(backend));
        back.insert_resource(atglyph_bevy::WorkPace::every_frame());
        load_run(back.world()).unwrap().expect("a save").restore(back.world_mut()).unwrap();
        let w = back.world_mut();
        let me = w.query_filtered::<Entity, With<Player>>().single(w).expect("the player came back");
        let at = |app: &App| app.world().get::<atglyph_bevy::Working>(me).map(|w| (w.0.done, w.0.needed));
        assert_eq!(at(&back), Some((saved.done, 6)), "at work, as far in as when saved");
        let clock = back.world().resource::<Turns>().now();
        play(&mut back);
        for _ in 0..6 {
            back.update();
        }
        assert_eq!(at(&back), None, "and carried on to the end with no key pressed");
        assert_eq!(
            back.world().resource::<Turns>().now() - clock,
            u32::from(6 - saved.done) * atglyph_core::turn::BASE_ACTION_COST,
            "a turn for each it had left"
        );
    }

    /// An actor posted somewhere is posted there again after a load, by
    /// the engine: the game's own record of it says nothing of a post, and
    /// a guard that forgot its cell would wander off the first time it
    /// had nothing better to do.
    #[test]
    fn a_posted_actor_keeps_its_post_when_the_run_is_continued() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game(Saves(backend.clone()));
        app.world_mut().spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30)));
        let post = start.offset(2, 3);
        app.world_mut().spawn((Actor, Blocks, Person("Ada".into()), Position(start.offset(0, 3)), Health::full(20), Post(post)));
        play(&mut app);
        save_run(app.world_mut()).unwrap();

        let (mut back, _) = game(Saves(backend));
        load_run(back.world()).unwrap().expect("a save").restore(back.world_mut()).unwrap();
        play(&mut back);
        let w = back.world_mut();
        let posted = w.query_filtered::<&Post, With<Person>>().single(w).copied().expect("Ada came back");
        assert_eq!(posted, Post(post), "posted on the same cell");
    }

    /// A follower follows the same leader after a load, by the engine: its
    /// brain is the game's content made again from a definition and holds
    /// no one, and a game's record cannot name an entity. One whose leader
    /// was not saved comes back following nobody rather than a stranger.
    #[test]
    fn a_follower_keeps_its_leader_when_the_run_is_continued_and_one_whose_leader_was_not_saved_follows_nobody() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game(Saves(backend.clone()));
        app.world_mut().spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30)));
        let chief = app.world_mut().spawn((Actor, Blocks, Person("Ada".into()), Position(start.offset(0, 3)), Health::full(20))).id();
        app.world_mut().spawn((Actor, Blocks, Person("Bo".into()), Position(start.offset(1, 3)), Health::full(20), Leader(chief)));
        let stranger = app.world_mut().spawn((Actor, Blocks, Position(start.offset(4, 3)), Health::full(20))).id();
        app.world_mut().spawn((Actor, Blocks, Person("Cy".into()), Position(start.offset(3, 3)), Health::full(20), Leader(stranger)));
        play(&mut app);
        save_run(app.world_mut()).unwrap();

        let (mut back, _) = game(Saves(backend));
        load_run(back.world()).unwrap().expect("a save").restore(back.world_mut()).unwrap();
        play(&mut back);
        let w = back.world_mut();
        let people: Vec<(Entity, String)> = w.query::<(Entity, &Person)>().iter(w).map(|(e, p)| (e, p.0.clone())).collect();
        let who = |name: &str| people.iter().find(|(_, n)| n == name).unwrap().0;
        let w = back.world();
        assert_eq!(w.get::<Leader>(who("Bo")), Some(&Leader(who("Ada"))), "Bo follows Ada again, as she came back");
        assert_eq!(w.get::<Leader>(who("Cy")), None, "Cy's leader was not saved, so Cy follows nobody");
        assert_eq!(w.get::<Leader>(who("Ada")), None, "and the leader follows nobody");
    }

    /// The order a leader's followers joined in comes back with them, even
    /// where it is not the order they were made in, which is the order a
    /// load makes them again in.
    #[test]
    fn followers_keep_the_order_they_joined_in_when_the_run_is_continued() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game(Saves(backend.clone()));
        let me = app.world_mut().spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30))).id();
        let early = app.world_mut().spawn((Actor, Blocks, Person("Early".into()), Position(start.offset(1, 3)), Health::full(20))).id();
        app.world_mut().spawn((Actor, Blocks, Person("Late".into()), Position(start.offset(3, 3)), Health::full(20), Leader(me)));
        app.world_mut().entity_mut(early).insert(Leader(me));
        play(&mut app);
        save_run(app.world_mut()).unwrap();

        let (mut back, _) = game(Saves(backend));
        load_run(back.world()).unwrap().expect("a save").restore(back.world_mut()).unwrap();
        play(&mut back);
        let w = back.world_mut();
        let mut order: Vec<(JoinStamp, String)> = w.query::<(&JoinStamp, &Person)>().iter(w).map(|(j, p)| (*j, p.0.clone())).collect();
        order.sort();
        let names: Vec<&str> = order.iter().map(|(_, n)| n.as_str()).collect();
        assert_eq!(names, vec!["Late", "Early"], "the one made later joined first, and still comes first");
    }

    /// Something alight comes back alight with the turns it had left, and a
    /// fire that never goes out comes back as one. The burning cell under
    /// it is saved with the engine's fields, but only the thing itself
    /// knows how long it will go on feeding that cell.
    #[test]
    fn a_burning_thing_keeps_the_turns_it_had_left_when_the_run_is_continued() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game(Saves(backend.clone()));
        app.world_mut().spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30)));
        app.world_mut().spawn((Item, Thing { def: "crate".into(), notches: 0 }, Position(start.offset(2, 0)), Burning::for_turns(7)));
        app.world_mut().spawn((Item, Thing { def: "brazier".into(), notches: 0 }, Position(start.offset(3, 0)), Burning::forever()));
        app.world_mut().spawn((Item, Thing { def: "ash".into(), notches: 0 }, Position(start.offset(4, 0)), Burning::for_turns(0)));
        app.world_mut().spawn((Item, Thing { def: "coin".into(), notches: 0 }, Position(start.offset(5, 0))));
        play(&mut app);
        save_run(app.world_mut()).unwrap();

        let (mut back, _) = game(Saves(backend));
        load_run(back.world()).unwrap().expect("a save").restore(back.world_mut()).unwrap();
        let w = back.world_mut();
        let mut alight: Vec<(String, Option<u32>)> = w.query::<(&Thing, &Burning)>().iter(w).map(|(t, b)| (t.def.clone(), b.turns)).collect();
        alight.sort();
        assert_eq!(alight, vec![("brazier".to_string(), None), ("crate".to_string(), Some(7))], "what was alight, for as long, and nothing else");
    }

    /// The whole of a run comes back: every kind by its own account, and
    /// on each the engine's state, so a worn ring is worn, a bag holds what
    /// it held, a stack keeps its count, a status its turns, a stairway its
    /// destination, and a resource its contents, with nothing but three
    /// kinds and a resource written by the game.
    #[test]
    fn a_run_saved_by_kind_is_continued_whole() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game(Saves(backend.clone()));
        let ring =
            app.world_mut().spawn((Item, Thing { def: "ring".into(), notches: 2 }, Wearable(EquipShape::in_slot(atglyph_rules::SlotId::from_raw(0))))).id();
        let coins = app.world_mut().spawn((Item, Thing { def: "coin".into(), notches: 0 }, Stack { key: 1, count: 14 })).id();
        let dropped = app.world_mut().spawn((Item, Thing { def: "coin".into(), notches: 0 }, Stack { key: 1, count: 3 }, Position(start.offset(2, 0)))).id();
        let mut worn = Equipment::with_slot_count(1);
        worn.equip(ring, &EquipShape::in_slot(atglyph_rules::SlotId::from_raw(0))).unwrap();
        let me = app
            .world_mut()
            .spawn((
                Actor,
                Player,
                Blocks,
                You,
                Position(start),
                Viewshed::new(6),
                RevealsMap,
                Health { current: 17, max: 30 },
                Inventory { items: vec![ring, coins] },
                Equipped(worn),
            ))
            .id();
        let other = app.world_mut().spawn((Actor, Blocks, Person("Ada".into()), Position(start.offset(0, 3)), Health { current: 5, max: 20 })).id();
        app.world_mut().spawn((Position(start.offset(1, 1)), Transition { to: Destination::Place { map: MapId(4), arrive: Arrive::Entry } }));
        app.world_mut().resource_mut::<Stocked>().0 = vec![3, 8];
        play(&mut app);
        let dazed = app.world().resource::<Registries>().statuses.expect("dazed");
        app.world_mut().write_message(ApplyStatus { target: me, status: dazed, turns: 6, by: None, held_by: None });
        app.world_mut().write_message(Intent::new(me, Wait));
        app.update();
        let clock = app.world().resource::<Turns>().now();
        save_run(app.world_mut()).unwrap();
        assert!(backend.exists("run"));
        let _ = (other, dropped);

        let (mut back, _) = game(Saves(backend));
        let save = load_run(back.world()).unwrap().expect("a save to continue");
        assert_eq!(save.state::<Stocked>(), Some(vec![3, 8]), "a resource can be read before the world exists");
        assert_eq!(save.engine.seed, RunSeed(9));
        assert_eq!((save.count_of::<Thing>(), save.count_of::<Person>(), save.count_of::<You>()), (3, 1, 1));
        save.restore(back.world_mut()).unwrap();
        play(&mut back);
        let w = back.world_mut();
        let me2 = w.query_filtered::<Entity, With<You>>().single(w).unwrap();
        let w = back.world();
        assert_eq!(w.get::<Position>(me2).map(|p| p.0), Some(start));
        assert_eq!(w.get::<Health>(me2).map(|h| (h.current, h.max)), Some((17, 30)));
        let bag = w.get::<Inventory>(me2).expect("a bag").items.clone();
        assert_eq!(bag.len(), 2);
        let (ring2, coins2) = (bag[0], bag[1]);
        assert_eq!(w.get::<Thing>(ring2).map(|t| (t.def.as_str(), t.notches)), Some(("ring", 2)), "the kind's own account came back");
        assert_eq!(w.get::<Equipped>(me2).and_then(|e| e.slot_of(ring2)), Some(atglyph_rules::SlotId::from_raw(0)), "and it is worn again");
        assert_eq!(w.get::<Stack>(coins2).map(|s| s.count), Some(14), "the stack kept its count");
        assert!(w.get::<Position>(coins2).is_none(), "carried, so nowhere");
        assert!(w.get::<ActiveStatuses>(me2).is_some_and(|a| a.has(dazed)), "the status was put back by request");
        assert_eq!(w.resource::<Stocked>().0, vec![3, 8]);
        assert_eq!(w.resource::<Turns>().now(), clock, "the clock came back with the engine's state");
        assert!(w.get::<MyTurn>(me2).is_some(), "and the player holds the turn again");
        let w = back.world_mut();
        let people: Vec<(String, Point, i32)> = w.query::<(&Person, &Position, &Health)>().iter(w).map(|(p, at, h)| (p.0.clone(), at.0, h.current)).collect();
        assert_eq!(people, vec![("Ada".into(), start.offset(0, 3), 5)]);
        let ground: Vec<(u32, Point)> = w.query_filtered::<(&Stack, &Position), With<Thing>>().iter(w).map(|(s, p)| (s.count, p.0)).collect();
        assert_eq!(ground, vec![(3, start.offset(2, 0))], "the dropped coins lie where they lay");
        let stairs: Vec<(Point, Transition)> = w.query::<(&Position, &Transition)>().iter(w).map(|(p, t)| (p.0, *t)).collect();
        assert!(stairs.is_empty(), "a transition nobody registered a kind for is not the engine's to spawn");
    }

    /// A stair the engine laid comes back from a save as it was laid,
    /// once, standing where it stood and leading where it led, with
    /// nothing written by the game: the engine saves its own kind, and the
    /// transition with it rather than beside it.
    #[test]
    fn a_stair_the_engine_laid_comes_back_once_where_it_stood_leading_where_it_led() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game(Saves(backend.clone()));
        app.world_mut().spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30)));
        let stair = Stair { climb: atglyph_bevy::Climb::Down, look: atglyph_bevy::StairLook::new('>', atglyph_grid::Rgb::new(235, 219, 158), "stairs down") };
        let to = Transition { to: Destination::Place { map: MapId(4), arrive: Arrive::Entry } };
        app.world_mut().spawn((Position(start.offset(1, 1)), to, stair.clone(), Name::new("stairs down")));
        play(&mut app);
        save_run(app.world_mut()).unwrap();

        let (mut back, _) = game(Saves(backend));
        let save = load_run(back.world()).unwrap().expect("a save to continue");
        assert_eq!(save.count_of::<Stair>(), 1, "written down once, as a stair");
        save.restore(back.world_mut()).unwrap();
        play(&mut back);
        let w = back.world_mut();
        let stairs: Vec<(Point, Transition, Stair, String)> =
            w.query::<(&Position, &Transition, &Stair, &Name)>().iter(w).map(|(p, t, s, n)| (p.0, *t, s.clone(), n.as_str().to_string())).collect();
        assert_eq!(stairs, vec![(start.offset(1, 1), to, stair, "stairs down".to_string())]);
        assert_eq!(w.query::<&Transition>().iter(w).count(), 1, "and no second transition beside it");
    }

    /// A trader's stock is its pack, so it comes back as any actor's pack
    /// does: the same things in the same order, a pile with its count,
    /// with nothing written by the game about what it sells.
    #[test]
    fn a_stocked_traders_inventory_survives_a_save_as_any_actors_does() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game(Saves(backend.clone()));
        app.world_mut().spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30)));
        let till = app.world_mut().spawn((Item, Thing { def: "coin".into(), notches: 0 }, Stack { key: 1, count: 40 })).id();
        let ware = app.world_mut().spawn((Item, Thing { def: "ring".into(), notches: 1 }, atglyph_bevy::Worth(25))).id();
        app.world_mut().spawn((
            Actor,
            Blocks,
            Person("Ada".into()),
            Position(start.offset(1, 0)),
            Health::full(20),
            Inventory { items: vec![ware, till] },
            atglyph_bevy::Trader { buys: Vec::new(), pays_pct: 25 },
        ));
        play(&mut app);
        save_run(app.world_mut()).unwrap();

        let (mut back, _) = game(Saves(backend));
        load_run(back.world()).unwrap().expect("a save").restore(back.world_mut()).unwrap();
        play(&mut back);
        let w = back.world_mut();
        let stock = w.query_filtered::<&Inventory, With<Person>>().single(w).expect("the trader came back").items.clone();
        let w = back.world();
        let things: Vec<(&str, u8, Option<u32>)> =
            stock.iter().map(|i| w.get::<Thing>(*i).map(|t| (t.def.as_str(), t.notches, w.get::<Stack>(*i).map(|s| s.count))).unwrap()).collect();
        assert_eq!(things, vec![("ring", 1, None), ("coin", 0, Some(40))], "its stock, in order, the pile with its count");
    }

    /// A game with the player wearing a ring in its one slot, `dazed` on
    /// them held by the ring, and, when `saved_ring`, the ring a saved
    /// kind; saved, and continued in a fresh game that has begun play.
    fn continued_wearing_a_ring_that_holds_a_status(saved_ring: bool) -> (App, Entity, atglyph_rules::StatusId) {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game(Saves(backend.clone()));
        let finger = EquipShape::in_slot(atglyph_rules::SlotId::from_raw(0));
        let ring = app.world_mut().spawn((Item, Wearable(finger.clone()))).id();
        if saved_ring {
            app.world_mut().entity_mut(ring).insert(Thing { def: "ring".into(), notches: 0 });
        }
        let mut worn = Equipment::with_slot_count(1);
        worn.equip(ring, &finger).unwrap();
        let me = app
            .world_mut()
            .spawn((
                Actor,
                Player,
                Blocks,
                You,
                Position(start),
                Viewshed::new(6),
                RevealsMap,
                Health::full(30),
                Inventory { items: vec![ring] },
                Equipped(worn),
            ))
            .id();
        play(&mut app);
        let dazed = app.world().resource::<Registries>().statuses.expect("dazed");
        app.world_mut().write_message(ApplyStatus { target: me, status: dazed, turns: 20, by: None, held_by: Some(ring) });
        app.world_mut().write_message(Intent::new(me, Wait));
        app.update();
        assert!(app.world().get::<ActiveStatuses>(me).is_some_and(|a| a.has(dazed)), "held, and on");
        save_run(app.world_mut()).unwrap();

        let (mut back, _) = game(Saves(backend));
        load_run(back.world()).unwrap().expect("a save to continue").restore(back.world_mut()).unwrap();
        play(&mut back);
        let w = back.world_mut();
        let me = w.query_filtered::<Entity, With<You>>().single(w).unwrap();
        (back, me, dazed)
    }

    /// A status a worn thing holds comes back held by that thing, not
    /// loose: still on after the continue, and ended by taking the thing
    /// off, as it would have been before the save.
    #[test]
    fn a_held_status_comes_back_held_by_its_thing_and_ends_when_it_comes_off() {
        let (mut back, me, dazed) = continued_wearing_a_ring_that_holds_a_status(true);
        let w = back.world();
        let ring = w.get::<Inventory>(me).expect("a bag").items[0];
        assert!(w.get::<Equipped>(me).is_some_and(|e| e.slot_of(ring).is_some()), "the ring is worn again");
        let held: Vec<Option<u64>> = w.get::<ActiveStatuses>(me).expect("afflicted").iter().filter(|s| s.id == dazed).map(|s| s.held_by).collect();
        assert_eq!(held, vec![Some(ring.to_bits())], "and holds the status it held");
        back.world_mut().write_message(Intent::new(me, Unequip(ring)));
        back.update();
        assert!(back.world().get::<ActiveStatuses>(me).is_some_and(|a| !a.has(dazed)), "taken off, and it ended");
    }

    /// A held status whose thing did not come back does not come back
    /// either, rather than coming back loose and running its time.
    #[test]
    fn a_held_status_whose_thing_was_not_saved_does_not_come_back() {
        let (back, me, dazed) = continued_wearing_a_ring_that_holds_a_status(false);
        assert!(back.world().get::<ActiveStatuses>(me).is_some_and(|a| !a.has(dazed)));
    }

    /// The slot is refused whole when the game's version or the engine's
    /// format is not the one this build writes.
    #[test]
    fn a_save_of_another_version_or_format_is_refused() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game(Saves(backend.clone()));
        app.world_mut().spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30)));
        play(&mut app);
        save_run(app.world_mut()).unwrap();
        let text = backend.load("run").unwrap().unwrap();
        backend.persist("run", &text.replacen("version: 3", "version: 2", 1)).unwrap();
        assert!(matches!(load_run(app.world()), Err(SaveError::Version { found: 2, expected: 3 })));
        backend.persist("run", &text.replacen("format: 1", "format: 9", 1)).unwrap();
        assert!(matches!(load_run(app.world()), Err(SaveError::Version { found: 9, expected: 1 })));
    }

    /// An arrival somewhere while playing, as a warp into a place reports it.
    fn arrive(app: &mut App, at: Point) {
        app.world_mut().write_message(atglyph_bevy::PlaceEntered { map: atglyph_bevy::MapId::ZERO, first: true, entry: at, exit: None });
        app.update();
    }

    /// With `on_arrival()`, arriving somewhere writes the run, so each place
    /// entered is a save point; without it, and once the run is over,
    /// nothing is written.
    #[test]
    fn a_run_is_saved_on_arrival_when_asked_and_not_otherwise() {
        for asked in [true, false] {
            let backend = std::sync::Arc::new(MemoryBackend::default());
            let plugin = SavePlugin::new("run").version(3);
            let (mut app, start) = game_with(Saves(backend.clone()), if asked { plugin.on_arrival() } else { plugin });
            app.world_mut().spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30)));
            play(&mut app);
            assert!(!backend.exists("run"), "asked: {asked}: nothing written before an arrival");
            arrive(&mut app, start);
            assert_eq!(backend.exists("run"), asked, "asked: {asked}");
        }
    }

    /// An arrival after the run ended writes nothing: the end deleted the
    /// save, and writing it back would let the run be continued past it.
    #[test]
    fn an_arrival_after_the_run_is_over_writes_nothing() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game_with(Saves(backend.clone()), SavePlugin::new("run").version(3).on_arrival());
        app.world_mut().spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30)));
        play(&mut app);
        app.world_mut().write_message(RunOver::died(None));
        app.update();
        app.update();
        arrive(&mut app, start);
        assert!(!backend.exists("run"));
    }

    /// The fact ledger is the engine's, so its saving is too.
    #[test]
    fn counters_come_back_as_they_were_counted() {
        let counters = atglyph_rules::Registry::from_defs(vec![atglyph_rules::CounterDef::new("kills")]).unwrap();
        let mut ledger = atglyph_rules::Ledger::new(&counters);
        ledger.add(counters.expect("kills"), 3);
        let saved = atglyph_bevy::Counters(ledger.clone()).capture();
        let mut fresh = atglyph_bevy::Counters(atglyph_rules::Ledger::default());
        fresh.restore(saved);
        assert_eq!(fresh.0, ledger);
    }

    /// The stash follows the run a turn at a time, and the run's end
    /// clears it and deletes the slot, so a window closed after dying
    /// writes nothing back and nothing is continued past the end.
    #[test]
    fn the_stash_follows_the_run_and_the_end_of_the_run_forgets_the_save() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game(Saves(backend.clone()));
        let me = app.world_mut().spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30))).id();
        play(&mut app);
        assert_eq!(app.world().resource::<Stash>().pending().as_deref(), Some("run"), "stashed by the first turn");
        save_run(app.world_mut()).unwrap();
        assert!(backend.exists("run"));
        assert_eq!(*app.world().resource::<SavedRun>(), SavedRun::Readable, "and the slot says it holds a run");
        app.world_mut().write_message(RunOver::died(None));
        let _ = me;
        app.update();
        app.update();
        assert_eq!(*app.world().resource::<State<EngineState>>().get(), EngineState::Over);
        assert_eq!(app.world().resource::<Stash>().pending(), None, "cleared with the save");
        assert!(!backend.exists("run"), "deleted");
        assert_eq!(*app.world().resource::<SavedRun>(), SavedRun::Empty, "and the slot says it is empty");
    }

    /// What the slot is read as when the game starts with `backend` holding
    /// whatever an earlier launch left: before the first frame, so a
    /// title's first look at it is the truth.
    fn read_at_startup(backend: &std::sync::Arc<MemoryBackend>) -> SavedRun {
        let (app, _) = game(Saves(backend.clone()));
        *app.world().resource::<SavedRun>()
    }

    /// A run played and written down, as an earlier launch leaves one.
    fn a_run_left_in(backend: &std::sync::Arc<MemoryBackend>) {
        let (mut app, start) = game(Saves(backend.clone()));
        app.world_mut().spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30)));
        play(&mut app);
        save_run(app.world_mut()).unwrap();
    }

    #[test]
    fn an_empty_slot_reads_empty() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        assert_eq!(read_at_startup(&backend), SavedRun::Empty);
    }

    #[test]
    fn a_saved_run_reads_readable() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        a_run_left_in(&backend);
        assert_eq!(read_at_startup(&backend), SavedRun::Readable);
    }

    #[test]
    fn a_slot_of_another_version_reads_unreadable() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        a_run_left_in(&backend);
        let text = backend.load("run").unwrap().unwrap();
        backend.persist("run", &text.replacen("version: 3", "version: 2", 1)).unwrap();
        assert_eq!(read_at_startup(&backend), SavedRun::Unreadable);
        assert!(backend.exists("run"), "and it is left where it is, for a new run to ask about");
    }

    /// Abandoning forgets the run before the trigger returns, with no frame
    /// run in between: a title begins the new run in the same command, and
    /// a window closed after that writes nothing back.
    #[test]
    fn abandoning_deletes_the_slot_and_clears_the_stash() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game(Saves(backend.clone()));
        app.world_mut().spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30)));
        play(&mut app);
        save_run(app.world_mut()).unwrap();
        assert_eq!(*app.world().resource::<SavedRun>(), SavedRun::Readable, "a run to abandon");
        app.world_mut().trigger(AbandonRun);
        assert!(!backend.exists("run"), "the slot is gone");
        assert_eq!(app.world().resource::<Stash>().pending(), None, "and the stash with it");
        assert_eq!(*app.world().resource::<SavedRun>(), SavedRun::Empty, "and the slot says so");
    }

    /// The run's clock, as a save of it holds it.
    fn clock_in(text: &str) -> u32 {
        decode::<RunSave>(3, text).expect("a run this build reads").engine.now
    }

    /// What the stash would write if the window were closed now.
    fn stashed(app: &App) -> String {
        let scratch = MemoryBackend::default();
        app.world().resource::<Stash>().flush(&scratch).unwrap();
        scratch.load("run").unwrap().expect("a run is stashed")
    }

    /// Leaving for the title writes the run down as it stands. The stash
    /// follows the run a whole turn at a time, so a quick player's half
    /// turn is in neither the slot nor the stash until the run is left, and
    /// then it is in both: Continue, or a window closed at the title, picks
    /// the run up where it was left.
    #[test]
    fn leaving_a_run_writes_it_down_first() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game(Saves(backend.clone()));
        let me = app.world_mut().spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30), Speed(200))).id();
        play(&mut app);
        app.world_mut().write_message(Intent::new(me, Wait));
        app.update();
        assert_eq!(app.world().resource::<Turns>().now(), 50, "a quick wait is half a turn");
        assert_eq!(clock_in(&stashed(&app)), 0, "which the stash has not followed");
        app.world_mut().write_message(LeaveRun);
        app.update();
        let slot = backend.load("run").unwrap().expect("the run left is in the slot");
        assert_eq!(clock_in(&slot), 50, "as it was left, not as the stash had it");
        assert_eq!(stashed(&app), slot, "and the stash holds the same, for a window closed at the title");
        assert_eq!(*app.world().resource::<SavedRun>(), SavedRun::Readable);
    }

    /// A run that ends in the frame it is left has nothing to continue: it
    /// is not written down, and the save it had is forgotten with it, as
    /// its end would have forgotten it had the state got that far.
    #[test]
    fn a_run_that_ends_in_the_frame_it_is_left_is_forgotten() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game(Saves(backend.clone()));
        app.world_mut().spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30)));
        play(&mut app);
        save_run(app.world_mut()).unwrap();
        app.world_mut().write_message(RunOver::died(None));
        app.world_mut().write_message(LeaveRun);
        app.update();
        assert!(!backend.exists("run"), "nothing to continue");
        assert_eq!(app.world().resource::<Stash>().pending(), None, "nor to write back on the way out");
        assert_eq!(*app.world().resource::<SavedRun>(), SavedRun::Empty);
    }

    /// A game, or a test, that inserted a backend of its own keeps it: the
    /// plugin inserts the player's saves only where there are none, so a
    /// headless test's memory backend is never swapped for the disk.
    #[test]
    fn a_backend_the_game_inserted_is_the_one_its_runs_are_saved_through() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        let mut app = atglyph_bevy::testing::headless_app();
        app.insert_resource(Saves(backend.clone())).add_plugins(SavePlugin::new("warren"));
        app.world().resource::<Saves>().persist("warren", "kept").unwrap();
        assert_eq!(backend.load("warren").unwrap().as_deref(), Some("kept"));
    }

    /// A game's run is saved to the slot of its name, beside the settings
    /// and the records in the same saves, so a game named like either
    /// would write its run over them, and is refused when it is made.
    #[test]
    #[should_panic(expected = "the engine's own")]
    fn a_game_named_like_the_settings_slot_is_refused() {
        let _ = SavePlugin::new(crate::settings::SETTINGS_SLOT);
    }

    /// As with the settings, so with the records.
    #[test]
    #[should_panic(expected = "the engine's own")]
    fn a_game_named_like_the_records_slot_is_refused() {
        let _ = SavePlugin::new(crate::RECORDS_SLOT);
    }

    /// A game's name is its run's slot, so it is a plain name too.
    #[test]
    #[should_panic(expected = "plain name")]
    fn a_game_named_with_a_path_is_refused() {
        let _ = SavePlugin::new("../escape");
    }

    /// A scripted run's saves are in memory, of their own: what it writes
    /// is not in the player's saves of the same name.
    #[test]
    fn a_scripted_run_never_touches_the_players_save() {
        let slot = format!("atglyph-save-unit-{}", std::process::id());
        let real = Saves::platform_default(&slot);
        let scripted = SavePlugin::new(slot.clone()).saves(true);
        scripted.persist(&slot, "a scripted run").unwrap();
        assert_eq!(real.load(&slot).unwrap(), None, "the player's slot is as it was");
        assert!(scripted.exists(&slot), "and the scripted run has one of its own");
    }

    /// A body a status turned comes back on the side it goes back to, the
    /// status turns it again, and when the status ends it is back where it
    /// began: in a game that saves no side of its own at all.
    #[test]
    fn a_body_a_status_turned_is_turned_again_after_a_continue_and_goes_back_when_it_ends() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game(Saves(backend.clone()));
        let (ours, theirs) = (atglyph_rules::FactionId::from_raw(0), atglyph_rules::FactionId::from_raw(1));
        let me = app.world_mut().spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30))).id();
        let them = app.world_mut().spawn((Actor, Blocks, Person("a".into()), Position(start.offset(3, 0)), Health::full(20), Faction(ours))).id();
        play(&mut app);
        let turned = app.world().resource::<Registries>().statuses.expect("turned");
        app.world_mut().write_message(ApplyStatus { target: them, status: turned, turns: 20, by: None, held_by: None });
        app.world_mut().write_message(Intent::new(me, Wait));
        app.update();
        assert_eq!(app.world().get::<Faction>(them), Some(&Faction(theirs)), "turned before the save");
        save_run(app.world_mut()).unwrap();

        let (mut back, _) = game(Saves(backend));
        load_run(back.world()).unwrap().expect("a save to continue").restore(back.world_mut()).unwrap();
        play(&mut back);
        let w = back.world_mut();
        let them = w.query_filtered::<Entity, With<Person>>().single(w).unwrap();
        let me = w.query_filtered::<Entity, With<You>>().single(w).unwrap();
        back.world_mut().write_message(Intent::new(me, Wait));
        back.update();
        assert_eq!(back.world().get::<Faction>(them), Some(&Faction(theirs)), "turned again");
        assert_eq!(back.world().get::<SideWas>(them), Some(&SideWas(Faction(ours))), "with its own side to go back to");
        back.world_mut().write_message(RemoveStatus { target: them, status: turned });
        back.world_mut().write_message(Intent::new(me, Wait));
        back.update();
        assert_eq!(back.world().get::<Faction>(them), Some(&Faction(ours)), "and back on it when the status ends");
    }
}
