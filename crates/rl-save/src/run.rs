//! A whole run, saved and continued, with the game saying only what a
//! kind of thing is.
//!
//! Every game that saves used to write the same walk: every monster by
//! definition name, position, map and health; every item by definition,
//! count and enchant, on the ground or in whose bag; the player with its
//! bag, its slots and its statuses; the stairs; and then the same in
//! reverse, items first so bags can point at them. Five hundred lines in
//! Corsair, and every game would write them again.
//!
//! The engine walks the world instead. A game implements [`Saveable`] on
//! the component that marks a kind of thing it spawns, saying how to write
//! one down and how to spawn one again from what was written, and
//! registers it with [`AddSaveable::save_kind`]. Everything the engine
//! owns on that entity, where it stands, its health, what it carries and
//! wears, its statuses, its stack, the transition it is, is the engine's
//! to capture and put back, in [`EntityState`]. A resource a game keeps of
//! the run goes through [`SaveableState`] and
//! [`save_state`](AddSaveable::save_state).
//!
//! [`RunSave`] is the result: the engine's own [`EngineSave`], every kind's
//! entries in the kind's own RON, the engine's state on each, and the
//! game's resources. [`SavePlugin`] owns the loop around it: the stash kept
//! a turn behind the run so a closed window saves, and the slot deleted
//! when the run ends, so nothing is resumed past its end. A game's save key
//! calls [`save_run`], and its start calls [`load_run`] and
//! [`RunSave::restore`].

use bevy::prelude::*;
use rl_bevy::{
    Afflict, Afflicted, Dead, Emptied, EndRun, EngineState, Equipped, Health, Hidden, Inventory, MapId, Needs, OnMap, Position, PropKind, Quests, Registries,
    Remains, Stack, Stocked, Transition, Turns, WasLiving, Wearable,
};
use rl_core::Point;
use rl_rules::Equipment;
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

    fn capture(world: &World, entity: Entity) -> SavedProp {
        let e = world.entity(entity);
        let name = world
            .get_resource::<Registries>()
            .map(|r| r.props.name(e.get::<PropKind>().expect("a prop kind is what this is keyed by").0).to_string())
            .unwrap_or_default();
        SavedProp {
            name,
            fires: e.get::<rl_bevy::Triggers>().map(|t| t.0.iter().map(|t| t.fires).collect()).unwrap_or_default(),
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
            rl_bevy::spawn_prop(&mut commands, &registries, id, Point::ZERO, MapId::SURFACE)
        };
        world.flush();
        // Its own firings wait for its triggers, which are armed from its
        // kind's once they are built, and a load can come first.
        let mut e = world.entity_mut(prop);
        if !saved.fires.is_empty() {
            e.insert(rl_bevy::PendingFires(saved.fires.clone()));
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

/// The quest tracker is the engine's, so its saving is too: a game with a
/// [`Quests`] resource adds `save_state::<Quests>()` and nothing more.
impl SaveableState for Quests {
    type Saved = rl_rules::Tracker;

    fn capture(&self) -> rl_rules::Tracker {
        self.tracker.clone()
    }

    fn restore(&mut self, saved: rl_rules::Tracker) {
        self.tracker = saved;
    }
}

/// Registers what a run is made of.
pub trait AddSaveable {
    /// Saves every entity carrying `K`, by `K`'s own account of it.
    fn save_kind<K: Saveable>(&mut self) -> &mut Self;

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
pub struct SaveRegistry {
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

/// Every living `K`, in spawn order, so a save is the same bytes for the
/// same run whatever order the archetypes are in.
fn collect<K: Component>(world: &mut World) -> Vec<Entity> {
    let mut found: Vec<Entity> = world.query_filtered::<Entity, (With<K>, Without<Dead>)>().iter(world).collect();
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
    /// Its statuses by registered name, with the turns left on each.
    #[serde(default)]
    pub statuses: Vec<(String, u32)>,
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
}

impl EntityState {
    /// The engine's state on `entity`, ids through `remap`.
    fn of(world: &World, entity: Entity, remap: &mut EntityRemap) -> Self {
        let e = world.entity(entity);
        let registries = world.get_resource::<Registries>();
        let at = e.get::<Position>().map(|p| (p.0, e.get::<OnMap>().map(|m| m.0).unwrap_or(MapId::SURFACE)));
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
        let statuses = match (e.get::<Afflicted>(), registries) {
            (Some(afflicted), Some(registries)) => afflicted.iter().map(|s| (registries.statuses.name(s.id).to_string(), s.turns)).collect(),
            _ => Vec::new(),
        };
        let remains = e.get::<Remains>().map(|r| (r.since, r.credit.map(|c| remap.save_id(c))));
        Self { at, health, bag, worn, statuses, stack: e.get::<Stack>().map(|s| s.count), transition: e.get::<Transition>().copied(), remains }
    }

    /// Puts this back on `entity`, the other entities through `remap`.
    fn restore(&self, world: &mut World, entity: Entity, remap: &EntityRemap) {
        let statuses: Vec<(rl_rules::StatusId, u32)> = match world.get_resource::<Registries>() {
            Some(registries) => self.statuses.iter().filter_map(|(name, turns)| registries.statuses.id(name).map(|id| (id, *turns))).collect(),
            None => Vec::new(),
        };
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
        // Last, and after the health a kind's own spawn gave it: a game
        // writes down what a thing is, never that it is dead, so what
        // came back is a living one, and this lays it down again exactly
        // as the death did. The killer is put back only if it was saved
        // too, since credit for a blow is not worth keeping an entity
        // alive for.
        if let Some((since, credit)) = self.remains {
            let credit = credit.and_then(|id| remap.entity(id));
            if let Ok(mut target) = world.get_entity_mut(entity) {
                target.remove::<WasLiving>().insert((rl_bevy::Prop, Remains { since, credit }));
            }
            // And named as what is left of what it was, from the one place
            // the wording lives: a game's record says what it was.
            rl_bevy::remains::name_as_remains(world, entity);
        }
        // By request, so each status installs its modifiers the way it did
        // the first time. Only where statuses are resolved at all.
        if world.get_resource::<Messages<Afflict>>().is_some() {
            for (status, turns) in statuses {
                world.write_message(Afflict { target: entity, status, turns, by: None });
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
/// and the engine bumps [`RunSave::FORMAT`] when this does.
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
    pub const FORMAT: u32 = 1;

    /// Writes the run down: every registered kind and resource, the
    /// engine's state on each entity, then the engine's own.
    pub fn capture(world: &mut World) -> Result<Self, SaveError> {
        let registry = world.remove_resource::<SaveRegistry>().unwrap_or_default();
        let result = Self::capture_with(world, &registry);
        world.insert_resource(registry);
        result
    }

    fn capture_with(world: &mut World, registry: &SaveRegistry) -> Result<Self, SaveError> {
        let mut remap = EntityRemap::new();
        let mut kinds = Vec::new();
        for kind in &registry.kinds {
            let mut entries = Vec::new();
            for entity in (kind.collect)(world) {
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
    pub fn state<R: SaveableState>(&self) -> Option<R::Saved> {
        let label = short_name::<R>();
        self.state.iter().find(|(l, _)| l == label).and_then(|(_, raw)| raw.into_rust().ok())
    }

    /// How many of `kind` were saved, for a line in the log.
    pub fn count_of<K: Saveable>(&self) -> usize {
        let label = short_name::<K>();
        self.kinds.iter().find(|k| k.kind == label).map_or(0, |k| k.entries.len())
    }

    /// The whole turn the run was saved on.
    pub fn turn(&self) -> u32 {
        self.engine.now / rl_core::turn::BASE_ACTION_COST
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

/// Saving a run and continuing it, owned by the engine.
///
/// Needs [`Saves`], the backend. Keeps the [`Stash`] a turn behind the run
/// so a window closed on it saves, and deletes the slot when the run ends
/// or is given up, so nothing is continued past its end. Registers no key:
/// a game's save key calls [`save_run`].
pub struct SavePlugin {
    slot: String,
    version: u32,
}

impl SavePlugin {
    /// Saves to `slot`, at version one.
    pub fn new(slot: impl Into<String>) -> Self {
        Self { slot: slot.into(), version: 1 }
    }

    /// The game's version of its saved shapes. Bump it, and say why next to
    /// the call, when an old save would parse wrongly.
    pub fn version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }
}

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SaveRegistry>()
            .save_kind::<PropKind>()
            .init_resource::<Stash>()
            .insert_resource(SaveSlot { slot: self.slot.clone(), version: self.version })
            .needs::<Saves>("SavePlugin", "`Saves`, the backend runs are written through, such as `Saves::platform_default(\"my-game\")`")
            .add_systems(Last, refresh_stash)
            .add_systems(EndRun, forget_save)
            .add_systems(OnEnter(EngineState::Over), forget_save);
    }
}

/// Writes the run to the slot, and stashes it for the bridge that writes
/// on the way out.
pub fn save_run(world: &mut World) -> Result<(), SaveError> {
    let text = encode_run(world)?;
    let slot = world.resource::<SaveSlot>().slot.clone();
    world.resource::<Saves>().persist(&slot, &text)?;
    world.resource::<Stash>().stash(&slot, &text);
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
pub fn refresh_stash(world: &mut World, mut last: Local<Option<u32>>) {
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
/// and continuing it would be continuing past its end.
pub fn forget_save(saves: Res<Saves>, slot: Res<SaveSlot>, stash: Res<Stash>) {
    stash.clear();
    if let Err(e) = saves.delete(&slot.slot) {
        error!("could not delete the save: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{MemoryBackend, SaveBackend};
    use rl_bevy::prelude::*;
    use rl_bevy::{FovPlugin, ItemsPlugin, StreamingPlugin};
    use rl_core::RunSeed;
    use rl_rules::{EquipShape, SlotDef, StatusDef};

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
                e.insert(Wearable(EquipShape::in_slot(rl_rules::SlotId::from_raw(0))));
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
        let mut app = rl_bevy::plugin::headless_app();
        app.add_plugins((FovPlugin, StreamingPlugin, CombatPlugin, StatusPlugin, ItemsPlugin, RemainsPlugin, rl_bevy::PropsPlugin));
        let start = rl_bevy::testing::surface(&mut app);
        rl_bevy::testing::two_sides(&mut app);
        {
            let mut registries = app.world_mut().resource_mut::<Registries>();
            registries.slots = rl_rules::Registry::from_defs(vec![SlotDef::new("finger")]).unwrap();
            registries.statuses = rl_rules::Registry::from_defs(vec![StatusDef::new("dazed")]).unwrap();
        }
        app.insert_resource(saves).insert_resource(Seed(RunSeed(9))).init_resource::<Stocked>();
        app.add_plugins(SavePlugin::new("run").version(3)).save_kind::<Thing>().save_kind::<Person>().save_kind::<You>().save_state::<Stocked>();
        app.finish();
        app.cleanup();
        (app, start)
    }

    fn play(app: &mut App) {
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
    }

    /// The engine's own kind: a game writes down nothing about its props
    /// and gets them back whole, where they stood, holding what they held,
    /// with a sprung trap still sprung and a found plate still found.
    #[test]
    fn props_are_saved_by_the_engine_with_each_ones_own_history() {
        const PROPS: &str = r#"#![enable(implicit_some)]
            [
                (name: "supply crate", glyph: '&', color: (r: 190, g: 165, b: 115), blocks: true,
                 container: (contents: [("coin", 1, 1)]), offers: [(verb: "open", time: 200)]),
                (name: "pressure plate", glyph: '^', color: (r: 230, g: 140, b: 51),
                 hidden: (spot: 40), triggers: [(on: "entered", fires: 2, effects: [(kind: "Teleport")])]),
            ]"#;

        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game(Saves(backend.clone()));
        rl_bevy::AddEngineEffects::add_engine_effects(&mut app);
        let props = rl_rules::prop::load(PROPS, &rl_rules::Names::new()).expect("the props load");
        app.world_mut().resource_mut::<Registries>().props = props.clone();
        let (crate_id, plate_id) = (props.expect("supply crate"), props.expect("pressure plate"));

        let coin = app.world_mut().spawn((Item, Thing { def: "coin".into(), notches: 0 }, Stack { key: 1, count: 5 })).id();
        let registries = app.world().resource::<Registries>().clone();
        let (chest, plate) = {
            let mut commands = app.world_mut().commands();
            (
                rl_bevy::spawn_prop(&mut commands, &registries, crate_id, start.offset(1, 0), MapId::SURFACE),
                rl_bevy::spawn_prop(&mut commands, &registries, plate_id, start.offset(0, 1), MapId::SURFACE),
            )
        };
        app.world_mut().flush();
        // `Stocked` here is the test's own saved resource, so the prop's is named in full.
        app.world_mut().entity_mut(chest).insert((Inventory { items: vec![coin] }, rl_bevy::Stocked));
        app.world_mut().spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30)));
        play(&mut app);
        // One of the plate's two firings spent, and the player has found it.
        app.world_mut().get_mut::<rl_bevy::Triggers>(plate).expect("the plate is armed").0[0].fires = Some(1);
        app.world_mut().entity_mut(plate).remove::<Hidden>();
        save_run(app.world_mut()).unwrap();

        let (mut back, _) = game(Saves(backend));
        rl_bevy::AddEngineEffects::add_engine_effects(&mut back);
        back.world_mut().resource_mut::<Registries>().props = props;
        let save = load_run(back.world()).unwrap().expect("a save");
        assert_eq!(save.count_of::<PropKind>(), 2, "both props were written down, by the engine");
        save.restore(back.world_mut()).unwrap();
        play(&mut back);

        let w = back.world_mut();
        let mut found: Vec<(String, Point, u32, bool)> = w
            .query::<(&PropKind, &Position, Option<&rl_bevy::Triggers>, Option<&Hidden>)>()
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
        app.world_mut().write_message(DamageEvent::new(ada, rl_rules::Hit::by(me, kind, 99)));
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
        let ring = app.world_mut().spawn((Item, Thing { def: "ring".into(), notches: 2 }, Wearable(EquipShape::in_slot(rl_rules::SlotId::from_raw(0))))).id();
        let coins = app.world_mut().spawn((Item, Thing { def: "coin".into(), notches: 0 }, Stack { key: 1, count: 14 })).id();
        let dropped = app.world_mut().spawn((Item, Thing { def: "coin".into(), notches: 0 }, Stack { key: 1, count: 3 }, Position(start.offset(2, 0)))).id();
        let mut worn = Equipment::with_slot_count(1);
        worn.equip(ring, &EquipShape::in_slot(rl_rules::SlotId::from_raw(0))).unwrap();
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
        app.world_mut().write_message(Afflict { target: me, status: dazed, turns: 6, by: None });
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
        assert_eq!(w.get::<Equipped>(me2).and_then(|e| e.slot_of(ring2)), Some(rl_rules::SlotId::from_raw(0)), "and it is worn again");
        assert_eq!(w.get::<Stack>(coins2).map(|s| s.count), Some(14), "the stack kept its count");
        assert!(w.get::<Position>(coins2).is_none(), "carried, so nowhere");
        assert!(w.get::<Afflicted>(me2).is_some_and(|a| a.has(dazed)), "the status was put back by request");
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
        app.world_mut().write_message(RunOver::died(None));
        let _ = me;
        app.update();
        app.update();
        assert_eq!(*app.world().resource::<State<EngineState>>().get(), EngineState::Over);
        assert_eq!(app.world().resource::<Stash>().pending(), None, "cleared with the save");
        assert!(!backend.exists("run"), "deleted");
    }
}
