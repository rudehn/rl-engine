//! Saving a run and continuing it.
//!
//! The engine walks the world and writes the run down: where everything
//! stands, its health, its pack and what it wears, its statuses and which
//! worn thing holds each, a stack's count and a thing's charges, where a
//! way through leads, every place built, the clock and what the
//! adventurer has seen. Undermere says only what each kind of thing it
//! spawns is: the adventurer, a monster by its definition's name, an item
//! by its definition's name, and a way through by which way it is. What
//! Undermere keeps of a run outside its entities, the deepest floor
//! reached, goes through [`SaveableState`], and the engine's `Quests` and
//! `Counters` are saved the same way wherever a run keeps them.
//!
//! The run is written on the way out, by the engine's stash when the
//! window closes or the menu quits, by the engine again when the menu
//! leaves the run for the title, and on every arrival, so a crash loses at
//! most the floor in hand. There is no save key. Death and a win delete
//! it. Continue reads it back through [`run::resume`](crate::run::resume).

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_save::{AddSaveable, SavePlugin, Saveable, SaveableState, UnloadPlugin};
use serde::{Deserialize, Serialize};

use crate::armory::{Armory, ItemKind};
use crate::bestiary::{Bestiary, Kind};
use crate::run::{Adventurer, Deepest};
use crate::stairs::Way;

/// Bump when a kind's shape below changes so an old save would parse
/// wrongly.
// v1: the first shape.
pub const VERSION: u32 = 1;

/// The slot every run saves to.
pub const SLOT: &str = "undermere";

/// What the save is made of: the engine's plugins, the four kinds, and
/// the three resources.
pub fn register(app: &mut App) {
    app.add_plugins((SavePlugin::new(SLOT).version(VERSION).on_arrival(), UnloadPlugin))
        .save_kind::<ItemKind>()
        .save_kind::<Kind>()
        .save_kind::<Adventurer>()
        .save_kind::<Way>()
        .save_state::<Deepest>()
        .save_state::<Quests>()
        .save_state::<Counters>();
}

/// Runs `spawn` against the world through a command queue, the way a
/// restore makes what a system would have made.
fn spawned(world: &mut World, spawn: impl FnOnce(&mut Commands, &World) -> Entity) -> Entity {
    let mut queue = CommandQueue::default();
    let e = {
        let world: &World = world;
        let mut commands = Commands::new(&mut queue, world);
        spawn(&mut commands, world)
    };
    queue.apply(world);
    e
}

/// The adventurer, as the save writes them: nothing of their own, since
/// everything they are, where they stand, their health, their pack, what
/// they wear and the statuses on them, is the engine's to keep, and their
/// light is the lantern's, which is an item.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct AdventurerSave {}

impl Saveable for Adventurer {
    type Saved = AdventurerSave;

    fn capture(_: &World, _: Entity) -> AdventurerSave {
        AdventurerSave {}
    }

    fn restore(world: &mut World, _: &AdventurerSave) -> Entity {
        let registries = world.resource::<Registries>().clone();
        spawned(world, |commands, _| crate::run::spawn_adventurer(commands, &registries))
    }
}

/// A monster, as the save writes it: its definition, by name. The engine
/// gives back where it stood, its health, what it carried and wore, its
/// statuses and the post it held.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonsterSave {
    /// The definition's name.
    pub def: String,
}

impl Saveable for Kind {
    type Saved = MonsterSave;

    fn capture(world: &World, entity: Entity) -> MonsterSave {
        let kind = world.get::<Kind>(entity).expect("a monster's kind");
        MonsterSave { def: world.resource::<Bestiary>().defs.name(kind.0).to_string() }
    }

    fn restore(world: &mut World, saved: &MonsterSave) -> Entity {
        let Some(id) = world.resource::<Bestiary>().defs.id(&saved.def) else {
            warn!("a saved monster is a {:?}, which this build has no definition for; it comes back as nothing", saved.def);
            return world.spawn_empty().id();
        };
        let registries = world.resource::<Registries>().clone();
        spawned(world, |commands, world| {
            // Nowhere and on no place: the engine puts it back where it was.
            crate::bestiary::spawn_monster(commands, world.resource::<Bestiary>().defs.get(id), Point::ZERO, MapId::SURFACE, &registries)
        })
    }
}

/// An item, as the save writes it: its definition, by name. Its stack and
/// its charges are the engine's to keep, and what it lends is its
/// definition's, made again with it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemSave {
    /// The definition's name.
    pub def: String,
}

impl Saveable for ItemKind {
    type Saved = ItemSave;

    fn capture(world: &World, entity: Entity) -> ItemSave {
        let kind = world.get::<ItemKind>(entity).expect("an item's kind");
        ItemSave { def: world.resource::<Armory>().defs.name(kind.0).to_string() }
    }

    fn restore(world: &mut World, saved: &ItemSave) -> Entity {
        let Some(id) = world.resource::<Armory>().defs.id(&saved.def) else {
            warn!("a saved item is a {:?}, which this build has no definition for; it comes back as nothing", saved.def);
            return world.spawn_empty().id();
        };
        let registries = world.resource::<Registries>().clone();
        spawned(world, |commands, world| crate::armory::spawn_item(commands, world.resource::<Armory>(), id, &registries))
    }
}

/// A way through, as the save writes it: which way it is, which is how it
/// is named and drawn. The engine keeps where it stands and where it
/// leads.
impl Saveable for Way {
    type Saved = Way;

    fn capture(world: &World, entity: Entity) -> Way {
        *world.get::<Way>(entity).expect("a way's kind")
    }

    fn restore(world: &mut World, saved: &Way) -> Entity {
        let mut e = world.spawn((*saved, Name::new(saved.name())));
        if let Some(glyph) = saved.glyph() {
            e.insert(glyph);
        }
        e.id()
    }
}

impl SaveableState for Deepest {
    type Saved = u32;

    fn capture(&self) -> u32 {
        self.0
    }

    fn restore(&mut self, saved: u32) {
        self.0 = saved;
    }
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_core::RunSeed;

    use super::*;
    use crate::dungeon::map_of;
    use crate::testing::{adventurer, arrive, at, continued, headless, here, pack, pass_turns, worn};

    /// A run as it looks from outside, for comparing a run with the same
    /// run continued.
    #[derive(Debug, PartialEq)]
    struct Looks {
        place: MapId,
        at: Point,
        health: (i32, i32),
        pack: Vec<(String, u32)>,
        worn: Vec<Option<String>>,
        lantern: Option<i32>,
        deepest: u32,
        monsters: Vec<(String, (i32, i32), i32)>,
        ways: Vec<((i32, i32), Way)>,
    }

    fn looks(app: &mut App) -> Looks {
        let me = adventurer(app);
        let place = here(app);
        let mut monsters: Vec<(String, (i32, i32), i32)> = {
            let world = app.world_mut();
            let mut q = world.query::<(&Name, &Position, &OnMap, &Health, &Kind)>();
            q.iter(world).filter(|(_, _, on, _, _)| on.0 == place).map(|(n, p, _, h, _)| (n.as_str().to_string(), (p.0.x, p.0.y), h.current)).collect()
        };
        monsters.sort();
        let mut ways: Vec<((i32, i32), Way)> = {
            let world = app.world_mut();
            let mut q = world.query::<(&Position, &OnMap, &Way)>();
            q.iter(world).filter(|(_, on, _)| on.0 == place).map(|(p, _, w)| ((p.0.x, p.0.y), *w)).collect()
        };
        ways.sort_by_key(|(p, _)| *p);
        let world = app.world();
        let bag = &world.get::<Inventory>(me).expect("a pack").items;
        let lantern = bag
            .iter()
            .find(|i| world.get::<Name>(**i).is_some_and(|n| n.as_str() == crate::run::LANTERN))
            .and_then(|l| world.get::<LightSource>(*l))
            .map(|l| l.radius);
        let mut carried = pack(app, me);
        carried.sort();
        Looks {
            place,
            at: at(app, me),
            health: world.get::<Health>(me).map(|h| (h.current, h.max)).expect("health"),
            pack: carried,
            worn: worn(app, me),
            lantern,
            deepest: world.resource::<Deepest>().0,
            monsters,
            ways,
        }
    }

    /// Saved on the way down to floor four, by the arrival itself, and
    /// continued in a fresh app: the same floor, the adventurer where they
    /// stood with what they carried and wore and the lantern as dim as the
    /// floor makes it, the floor's people where they stood, and the ways.
    #[test]
    fn a_run_saved_on_arrival_continues_on_the_same_floor_with_the_same_kit() {
        let mut app = headless(RunSeed(3));
        adventurer(&mut app);
        app.world().resource::<Saves>().delete(SLOT).expect("the slot can be emptied");
        arrive(&mut app, map_of(4));
        let before = looks(&mut app);
        assert_eq!((before.place, before.deepest), (map_of(4), 4), "{before:?}");
        assert_eq!(before.pack.len(), 4, "the kit: {before:?}");
        assert_eq!(before.lantern, Some(crate::light::lantern_radius(Some(4))), "the lantern dimmed for floor four");
        assert!(!before.monsters.is_empty() && !before.ways.is_empty(), "{before:?}");
        let text = app.world().resource::<Saves>().load(SLOT).expect("the slot reads").expect("the arrival wrote the run down");

        let mut continued = continued(&text);
        assert_eq!(looks(&mut continued), before);
        pass_turns(&mut continued, 5);
        assert_eq!(*continued.world().resource::<State<EngineState>>().get(), EngineState::Playing, "and it plays on");
    }

    /// A save naming something the content no longer has, a thing renamed
    /// since, continues without it rather than failing to start.
    #[test]
    fn a_save_naming_what_the_content_no_longer_has_continues_without_it() {
        let mut app = headless(RunSeed(3));
        arrive(&mut app, map_of(1));
        let text = app.world().resource::<Saves>().load(SLOT).expect("the slot reads").expect("a save");
        assert!(text.contains("\"sword\""), "the save names the sword");
        let mut continued = continued(&text.replace("\"sword\"", "\"nonesuch\""));
        assert_eq!(here(&continued), map_of(1), "the run goes on where it was");
        let world = continued.world_mut();
        let swords = world.query_filtered::<&Name, With<Item>>().iter(world).filter(|n| n.as_str() == "sword").count();
        assert_eq!(swords, 0, "without the sword");
        pass_turns(&mut continued, 2);
        assert_eq!(*continued.world().resource::<State<EngineState>>().get(), EngineState::Playing, "and plays on");
    }
}
