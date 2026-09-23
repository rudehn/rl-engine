//! Saving and continuing a run.
//!
//! The engine walks the world and writes the run down; Corsair says only
//! what each kind of thing it spawns is. A captain is spawned fresh and
//! given back its bag, its gear, its health and its statuses by the
//! engine; a monster is its definition, where it was spawned and what is
//! in its purse; an item its definition and the enchant rolled for it; a
//! stairway the glyph it is drawn with, since the engine knows where it
//! leads. What Corsair keeps of a run outside its entities, which regions
//! were stocked, where the cave mouths are, the ledger's state and the
//! world's size, goes through [`SaveableState`].
//!
//! The world itself is not saved; it regenerates from the seed, and the
//! engine replays the edits on top. `S` saves, `q` saves and quits,
//! `--continue` loads, and the engine deletes the save when the run ends,
//! so a run cannot be resumed past it. A window or tab closed on the run
//! saves it too: the engine keeps the run encoded in its stash, which
//! the unload bridge writes on the way out.

use bevy::prelude::*;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::Point;
use rl_engine::rl_rules::Enchanted;
use rl_engine::rl_save::{AddSaveable, RunSave, SavePlugin, Saveable, SaveableState, save_run};
use rl_engine::rl_ui::{AbilityKeys, Bindings, Controls, InventoryKeys, MenuKeys, MessageLog, ScrollbackKeys, SheetKeys, Tones};
use serde::{Deserialize, Serialize};

use crate::StartOptions;
use crate::input::Binds;
use crate::items::{Armory, ItemKind};
use crate::monsters::{Bestiary, MonsterKind};
use crate::places::Entrances;

/// Bump when a kind's shape below changes so an old save would parse
/// wrongly.
// v1: the first shape.
// v2: the engine's knowledge keeps its own bucket size and records the
//     surface regions seen, rather than taking the world's region size.
// v3: the engine walks the world by kind; Corsair writes only these.
pub const VERSION: u32 = 3;

/// The slot every run saves to.
pub const SLOT: &str = "corsair";

// ANCHOR: register
/// What the save is made of: the plugin that keeps it, the four kinds, and
/// the four resources.
pub fn register(app: &mut App) {
    app.add_plugins(SavePlugin::new(SLOT).version(VERSION))
        // Items before those who carry them is not required, since every
        // kind is spawned before any bag is filled, but it reads better.
        .save_kind::<ItemKind>()
        .save_kind::<MonsterKind>()
        .save_kind::<Captain>()
        .save_kind::<Stairway>()
        .save_state::<StartOptions>()
        .save_state::<Armory>()
        .save_state::<Bestiary>()
        .save_state::<Entrances>()
        .save_state::<Quests>();
}
// ANCHOR_END: register

/// The player, as a kind the save can name. The engine gives it back its
/// place, its health, its bag, its gear and its statuses; what a fresh
/// captain is, its name, its fist, what it knows and how quiet it is, is
/// spawned again the same way the first time.
#[derive(Component, Debug, Clone, Copy)]
pub struct Captain;

impl Saveable for Captain {
    type Saved = ();

    fn capture(_: &World, _: Entity) {}

    fn restore(world: &mut World, _: &()) -> Entity {
        crate::spawn_player(world, Point::ZERO)
    }
}

/// An item, as the save writes it: what it is and what was rolled for it,
/// since the enchant is already in the components the game spawns it with.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemSave {
    pub def: String,
    #[serde(default)]
    pub enchant: Enchanted,
}

impl Saveable for ItemKind {
    type Saved = ItemSave;

    fn capture(world: &World, entity: Entity) -> ItemSave {
        let armory = world.resource::<Armory>();
        let kind = world.get::<ItemKind>(entity).expect("an item kind");
        ItemSave { def: armory.defs.name(kind.0).to_string(), enchant: world.get::<Enchant>(entity).map(|e| e.0.clone()).unwrap_or_default() }
    }

    fn restore(world: &mut World, saved: &ItemSave) -> Entity {
        // One, nowhere: the engine puts back the count and the place.
        world.resource_scope(|world: &mut World, armory: Mut<Armory>| {
            let id = armory.defs.expect(&saved.def);
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let mut commands = Commands::new(&mut queue, world);
            let e = armory.spawn_with(&mut commands, id, 1, None, saved.enchant.clone());
            queue.apply(world);
            e
        })
    }
}

/// A monster, as the save writes it: what it is, whether it stood
/// underground when spawned, which is what gave it a lantern, and what is
/// in its purse, which plunder may have emptied.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonsterSave {
    pub def: String,
    pub underground: bool,
    #[serde(default)]
    pub purse: Option<u32>,
}

impl Saveable for MonsterKind {
    type Saved = MonsterSave;

    fn capture(world: &World, entity: Entity) -> MonsterSave {
        let bestiary = world.resource::<Bestiary>();
        let kind = world.get::<MonsterKind>(entity).expect("a monster kind");
        MonsterSave {
            def: bestiary.defs.name(kind.0).to_string(),
            underground: world.get::<OnMap>(entity).is_some_and(|m| !m.0.is_surface()),
            purse: world.get::<crate::abilities::Purse>(entity).map(|p| p.0),
        }
    }

    fn restore(world: &mut World, saved: &MonsterSave) -> Entity {
        let e = world.resource_scope(|world: &mut World, bestiary: Mut<Bestiary>| {
            let id = bestiary.defs.expect(&saved.def);
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let mut commands = Commands::new(&mut queue, world);
            let e = if saved.underground { bestiary.spawn_underground(&mut commands, id, Point::ZERO) } else { bestiary.spawn(&mut commands, id, Point::ZERO) };
            queue.apply(world);
            e
        });
        // What it carried and wore comes from the save, not from the kit a
        // fresh one is handed; and its purse is what was left in it.
        let mut monster = world.entity_mut(e);
        match saved.purse {
            Some(coin) => {
                monster.insert(crate::abilities::Purse(coin));
            }
            None => {
                monster.remove::<crate::abilities::Purse>();
            }
        }
        e
    }
}

// ANCHOR: kind
/// A stairway or a cave mouth: the engine knows where it leads, Corsair
/// only how it is drawn.
#[derive(Component, Debug, Clone, Copy)]
pub struct Stairway;

impl Saveable for Stairway {
    type Saved = char;

    fn capture(world: &World, entity: Entity) -> char {
        world.get::<rl_engine::rl_render::Glyph>(entity).map_or('>', |g| g.ch)
    }

    fn restore(world: &mut World, glyph: &char) -> Entity {
        world.spawn((Stairway, crate::places::stair_glyph(*glyph))).id()
    }
}
// ANCHOR_END: kind

impl SaveableState for Entrances {
    type Saved = Vec<Point>;

    fn capture(&self) -> Vec<Point> {
        self.0.iter().copied().collect()
    }

    fn restore(&mut self, saved: Vec<Point>) {
        self.0 = saved.into_iter().collect();
    }
}

impl SaveableState for Armory {
    type Saved = Vec<Point>;

    fn capture(&self) -> Vec<Point> {
        self.spawned().copied().collect()
    }

    fn restore(&mut self, saved: Vec<Point>) {
        self.restore_spawned(saved);
    }
}

impl SaveableState for Bestiary {
    type Saved = Vec<Point>;

    fn capture(&self) -> Vec<Point> {
        self.spawned().copied().collect()
    }

    fn restore(&mut self, saved: Vec<Point>) {
        self.restore_spawned(saved);
    }
}

/// `S` saves; `q` saves then quits. Both only while playing.
///
/// An exclusive system, since saving reads the whole world, so the keys
/// are read through the registry by hand rather than as a `ControlInput`.
pub fn save_keys(world: &mut World) {
    let (wants_save, wants_quit) = {
        let keys = world.resource::<ButtonInput<KeyCode>>();
        let controls = world.resource::<Controls>();
        let binds = world.resource::<Binds>();
        let bindings = Bindings {
            directions: world.resource(),
            cursor: world.resource(),
            help: world.resource(),
            log: world.get_resource::<ScrollbackKeys>(),
            sheet: world.get_resource::<SheetKeys>(),
            abilities: world.get_resource::<AbilityKeys>(),
            inventory: world.get_resource::<InventoryKeys>(),
            menu: world.get_resource::<MenuKeys>(),
        };
        (controls.which(binds.save, keys, &bindings).is_some(), controls.which(binds.quit, keys, &bindings).is_some())
    };
    if !wants_save && !wants_quit {
        return;
    }
    let playing = *world.resource::<State<EngineState>>().get() == EngineState::Playing;
    if playing {
        let turn = world.resource::<Turns>().turn_number();
        match save_run(world) {
            Ok(()) => world.resource_mut::<MessageLog>().push("The run is written in the log book.", Tones::NOTICE, turn),
            Err(e) => {
                error!("save failed: {e}");
                world.resource_mut::<MessageLog>().push(format!("The save failed: {e}"), Tones::BAD, turn);
            }
        }
    }
    if wants_quit {
        world.write_message(AppExit::Success);
    }
}

/// Where continuing lands, for the log.
pub fn describe(save: &RunSave) -> String {
    format!("Continuing from turn {} with {} things about.", save.turn(), save.count_of::<ItemKind>())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::headless;
    use rl_engine::rl_core::{Direction, RunSeed};
    use rl_engine::rl_save::{SaveBackend, Saves, Stash};

    fn player(app: &mut App) -> Entity {
        let w = app.world_mut();
        let mut q = w.query_filtered::<Entity, With<Player>>();
        q.single(w).unwrap()
    }

    fn monsters(app: &mut App) -> usize {
        let w = app.world_mut();
        let mut q = w.query::<&MonsterKind>();
        q.iter(w).count()
    }

    fn on_ground(app: &mut App) -> usize {
        let w = app.world_mut();
        let mut q = w.query_filtered::<&Position, With<Item>>();
        q.iter(w).count()
    }

    /// The stash follows the run a turn at a time, and the run's end
    /// deletes the save and clears the stash, so a window closed after
    /// dying writes nothing back.
    #[test]
    fn the_stash_follows_the_run_and_death_forgets_the_save() {
        let dir = std::env::temp_dir().join(format!("corsair-stash-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = headless(RunSeed(7), false, &dir);
        app.update();
        app.update();
        let stash = app.world().resource::<Stash>().clone();
        assert_eq!(stash.pending().as_deref(), Some(SLOT), "stashed by the first turn");
        save_run(app.world_mut()).unwrap();
        assert!(app.world().resource::<Saves>().exists(SLOT));

        let me = player(&mut app);
        let at = app.world().get::<Position>(me).unwrap().0;
        app.world_mut().write_message(DeathEvent { entity: me, at, credit: None, was_player: true });
        app.update();
        app.update();
        assert_eq!(stash.pending(), None, "cleared with the save");
        assert!(!app.world().resource::<Saves>().exists(SLOT), "the save went with the run");
        app.world_mut().write_message(AppExit::Success);
        app.update();
        assert!(!app.world().resource::<Saves>().exists(SLOT), "and closing the window afterwards wrote nothing back");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_run_saved_and_continued_is_the_same_run() {
        let dir = std::env::temp_dir().join(format!("corsair-save-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = headless(RunSeed(7), false, &dir);
        app.update();
        app.update();
        let me = player(&mut app);
        for _ in 0..3 {
            app.world_mut().write_message(Intent::new(me, Step(Direction::East)));
            app.update();
        }
        let bottle = app.world().get::<Inventory>(me).unwrap().items[1];
        app.world_mut().write_message(Intent::new(me, DropItem(bottle)));
        app.update();
        let (pos, hp, bag, turn) = {
            let w = app.world();
            (w.get::<Position>(me).unwrap().0, w.get::<Health>(me).unwrap().current, w.get::<Inventory>(me).unwrap().items.len(), w.resource::<Turns>().now())
        };
        let (monster_count, ground) = (monsters(&mut app), on_ground(&mut app));
        save_run(app.world_mut()).unwrap();

        let mut back = headless(RunSeed(1), true, &dir);
        back.update();
        back.update();
        let me2 = player(&mut back);
        let w = back.world();
        assert_eq!(w.resource::<WorldRes>().seed(), RunSeed(7), "the saved seed wins over the flag");
        assert_eq!(w.get::<Position>(me2).unwrap().0, pos);
        assert_eq!(w.get::<Health>(me2).unwrap().current, hp);
        assert_eq!(w.get::<Inventory>(me2).unwrap().items.len(), bag);
        assert_eq!(w.resource::<Turns>().now(), turn);
        assert!(w.get::<MyTurn>(me2).is_some(), "the player is waiting for input again");
        assert!(w.get::<Armor>(me2).is_some());
        let cutlass = w.resource::<Armory>().defs.expect("cutlass");
        let wielding = w.get::<Equipped>(me2).unwrap().worn().any(|(_, e)| w.get::<ItemKind>(e).is_some_and(|k| k.0 == cutlass));
        assert!(wielding, "the cutlass is back in hand");
        assert_eq!(monsters(&mut back), monster_count, "every monster came back, none were respawned");
        assert_eq!(on_ground(&mut back), ground, "the dropped bottle is still on the ground");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A monster saved carrying knives is continued carrying the same knives,
    /// not the kit a fresh one would be handed, and with the purse it had.
    #[test]
    fn a_monster_keeps_what_it_carries_across_a_save() {
        let dir = std::env::temp_dir().join(format!("corsair-save-bag-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = headless(RunSeed(7), false, &dir);
        app.update();
        app.update();
        let me = player(&mut app);
        let at = app.world().get::<Position>(me).unwrap().0.offset(0, 5);
        let kind = app.world().resource::<Bestiary>().defs.expect("cutthroat");
        let cutthroat = app.world_mut().resource_scope(|world: &mut World, bestiary: Mut<Bestiary>| {
            world.resource_scope(|world: &mut World, armory: Mut<Armory>| {
                let mut queue = bevy::ecs::world::CommandQueue::default();
                let mut commands = Commands::new(&mut queue, world);
                let e = bestiary.spawn(&mut commands, kind, at);
                bestiary.arm(&mut commands, &armory, e, kind);
                queue.apply(world);
                e
            })
        });
        app.world_mut().entity_mut(cutthroat).insert(crate::abilities::Purse(7));
        app.update();
        let knives = |app: &App, who: Entity| -> Vec<u32> {
            let w = app.world();
            w.get::<Inventory>(who).map(|b| b.items.iter().filter_map(|i| w.get::<Stack>(*i).map(|s| s.count)).collect()).unwrap_or_default()
        };
        let carried = knives(&app, cutthroat);
        assert!(!carried.is_empty(), "it came armed");
        save_run(app.world_mut()).unwrap();

        let mut back = headless(RunSeed(7), true, &dir);
        back.update();
        back.update();
        let restored = {
            let w = back.world_mut();
            let mut q = w.query::<(Entity, &MonsterKind, &Position)>();
            q.iter(w).find(|(_, k, p)| k.0 == kind && p.0 == at).map(|(e, _, _)| e).expect("the cutthroat came back where it stood")
        };
        assert_eq!(knives(&back, restored), carried, "with the knives it had");
        assert_eq!(back.world().get::<crate::abilities::Purse>(restored).map(|p| p.0), Some(7), "and the purse it had");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
