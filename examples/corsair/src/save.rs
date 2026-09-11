//! Saving and continuing a run.
//!
//! One versioned RON blob: the engine's state through [`EngineSave`], and
//! everything Corsair spawned, by definition name and save id. The world
//! itself is not saved; it regenerates from the seed, and the engine
//! replays the edits on top. `S` saves, `q` saves and quits, `--continue`
//! loads, and death deletes the save so a run cannot be resumed past it.

use bevy::prelude::*;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::Point;
use rl_engine::rl_events::Tracker;
use rl_engine::rl_rules::{Enchanted, Equipment};
use rl_engine::rl_save::{EngineSave, EntityRemap, SaveBackend, SaveError, SaveId, Saves, decode, encode};
use rl_engine::rl_ui::{LogCategory, MessageLog};
use serde::{Deserialize, Serialize};

use crate::items::{Armory, ItemKind};
use crate::monsters::{Bestiary, MonsterKind};
use crate::places::Entrances;

/// Bump when the shape below changes so an old save would parse wrongly.
// v1: the first shape.
pub const VERSION: u32 = 1;

/// The slot every run saves to.
pub const SLOT: &str = "corsair";

/// The whole run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSave {
    pub regions: (i32, i32),
    pub engine: EngineSave,
    pub player: PlayerSave,
    pub monsters: Vec<MonsterSave>,
    pub items: Vec<ItemSave>,
    pub transitions: Vec<TransitionSave>,
    pub spawned_monsters: Vec<Point>,
    pub spawned_loot: Vec<Point>,
    pub entrances: Vec<Point>,
    pub quests: Tracker,
    pub turn: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSave {
    pub id: SaveId,
    pub at: Point,
    pub map: MapId,
    pub hp: i32,
    pub bag: Vec<SaveId>,
    pub worn: Vec<(String, SaveId)>,
    #[serde(default)]
    pub statuses: Vec<(String, u32)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonsterSave {
    pub id: SaveId,
    pub def: String,
    pub at: Point,
    pub map: MapId,
    pub hp: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemSave {
    pub id: SaveId,
    pub def: String,
    pub count: Option<u32>,
    #[serde(default)]
    pub enchant: Enchanted,
    /// On the ground here, or `None` in the player's bag.
    pub at: Option<(Point, MapId)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionSave {
    pub at: Point,
    pub map: MapId,
    pub to: Destination,
}

/// The player as a capture sees it.
type PlayerData = (Entity, &'static Position, Option<&'static OnMap>, &'static Health, &'static Inventory, &'static Equipped, &'static Afflicted);
/// A monster as a capture sees it.
type MonsterData = (Entity, &'static MonsterKind, &'static Position, Option<&'static OnMap>, &'static Health);
/// An item as a capture sees it.
type ItemData = (Entity, &'static ItemKind, Option<&'static Stack>, Option<&'static Position>, Option<&'static OnMap>, Option<&'static Enchant>);

/// The parts of the world a capture reads.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Run<'w, 's> {
    player: Query<'w, 's, PlayerData, With<Player>>,
    monsters: Query<'w, 's, MonsterData, Without<Dead>>,
    items: Query<'w, 's, ItemData, With<Item>>,
    transitions: Query<'w, 's, (&'static Position, Option<&'static OnMap>, &'static Transition)>,
    bestiary: Res<'w, Bestiary>,
    armory: Res<'w, Armory>,
    entrances: Res<'w, Entrances>,
    quests: Res<'w, Quests>,
    turns: Res<'w, Turns>,
    statuses: Res<'w, StatusRules>,
}

/// The game's entities, captured.
struct Captured {
    player: PlayerSave,
    monsters: Vec<MonsterSave>,
    items: Vec<ItemSave>,
    transitions: Vec<TransitionSave>,
}

/// Captures the game's entities; the engine's state is added by the caller.
fn capture_game(run: &Run, remap: &mut EntityRemap) -> Option<Captured> {
    let map_of = |on: Option<&OnMap>| on.map(|m| m.0).unwrap_or(MapId::SURFACE);
    let (entity, pos, on, hp, bag, worn, afflicted) = run.player.single().ok()?;
    let player = PlayerSave {
        id: remap.save_id(entity),
        at: pos.0,
        map: map_of(on),
        hp: hp.hp,
        bag: bag.items.iter().map(|i| remap.save_id(*i)).collect(),
        worn: worn.worn().map(|(slot, item)| (run.armory.slots.name(slot).to_string(), remap.save_id(item))).collect(),
        statuses: afflicted.iter().map(|s| (run.statuses.defs.name(s.id).to_string(), s.turns)).collect(),
    };
    let monsters = run
        .monsters
        .iter()
        .map(|(e, kind, pos, on, hp)| MonsterSave {
            id: remap.save_id(e),
            def: run.bestiary.defs.name(kind.0).to_string(),
            at: pos.0,
            map: map_of(on),
            hp: hp.hp,
        })
        .collect();
    let items = run
        .items
        .iter()
        .filter(|(e, _, _, pos, _, _)| pos.is_some() || bag.contains(*e))
        .map(|(e, kind, stack, pos, on, enchant)| ItemSave {
            id: remap.save_id(e),
            def: run.armory.defs.name(kind.0).to_string(),
            count: stack.map(|s| s.count),
            enchant: enchant.map(|e| e.0.clone()).unwrap_or_default(),
            at: pos.map(|p| (p.0, map_of(on))),
        })
        .collect();
    let transitions = run.transitions.iter().map(|(pos, on, t)| TransitionSave { at: pos.0, map: map_of(on), to: t.to }).collect();
    Some(Captured { player, monsters, items, transitions })
}

/// Writes the run to the save slot.
pub fn save_run(world: &mut World) -> Result<(), SaveError> {
    let mut remap = EntityRemap::new();
    let mut state: bevy::ecs::system::SystemState<Run> = bevy::ecs::system::SystemState::new(world);
    let (player, monsters, items, transitions, rest) = {
        let run = state.get(world).expect("the run's resources exist while playing");
        let Some(Captured { player, monsters, items, transitions }) = capture_game(&run, &mut remap) else {
            return Err(SaveError::Encode("no player to save".into()));
        };
        let rest = (
            run.bestiary.spawned().copied().collect::<Vec<_>>(),
            run.armory.spawned().copied().collect::<Vec<_>>(),
            run.entrances.0.iter().copied().collect::<Vec<_>>(),
            run.quests.tracker.clone(),
            run.turns.turn_number(),
        );
        (player, monsters, items, transitions, rest)
    };
    let seed = world.resource::<WorldRes>().seed();
    let regions = (world.resource::<WorldRes>().width(), world.resource::<WorldRes>().height());
    let engine = EngineSave::capture(world, seed, &mut remap);
    let save = RunSave {
        regions,
        engine,
        player,
        monsters,
        items,
        transitions,
        spawned_monsters: rest.0,
        spawned_loot: rest.1,
        entrances: rest.2,
        quests: rest.3,
        turn: rest.4,
    };
    let text = encode(VERSION, &save)?;
    world.resource::<Saves>().persist(SLOT, &text)
}

/// Reads the save slot, if there is one this build can read.
pub fn load_run(saves: &Saves) -> Result<Option<RunSave>, SaveError> {
    match saves.load(SLOT)? {
        Some(text) => decode::<RunSave>(VERSION, &text).map(Some),
        None => Ok(None),
    }
}

/// Spawns the run's entities and restores the engine's state. The world
/// graph and the content resources must already be inserted; the player
/// is spawned here with its bag and gear.
pub fn restore_run(world: &mut World, save: &RunSave) {
    let mut remap = EntityRemap::new();
    // The registries come out while their spawners borrow commands.
    let armory = world.remove_resource::<Armory>().expect("the armory is inserted before restoring");
    let bestiary = world.remove_resource::<Bestiary>().expect("the bestiary is inserted before restoring");
    // Items first, so bags and slots can point at them.
    for item in &save.items {
        let id = armory.defs.expect(&item.def);
        let count = item.count.unwrap_or(1);
        let at = item.at.map(|(p, _)| p);
        let e = {
            let mut commands = world.commands();
            let e = armory.spawn_with(&mut commands, id, count, at, item.enchant.clone());
            if let Some((_, map)) = item.at {
                commands.entity(e).insert(OnMap(map));
            }
            e
        };
        world.flush();
        remap.bind(item.id, e);
    }
    for m in &save.monsters {
        let id = bestiary.defs.expect(&m.def);
        let e = {
            let mut commands = world.commands();
            let e = bestiary.spawn(&mut commands, id, m.at);
            commands.entity(e).insert((OnMap(m.map), Health { hp: m.hp, max: bestiary.defs.get(id).hp }));
            e
        };
        world.flush();
        remap.bind(m.id, e);
    }
    for t in &save.transitions {
        world.spawn((
            Position(t.at),
            OnMap(t.map),
            Transition { to: t.to },
            rl_engine::rl_render::Glyph::new(if matches!(t.to, Destination::Surface(_)) { '<' } else { '>' }, Color::srgb(0.9, 0.9, 0.6)).on_layer(1),
        ));
    }
    let p = &save.player;
    let mut worn = Equipment::for_slots(&armory.slots);
    for (slot, id) in &p.worn {
        let item = remap.entity(*id).expect("worn item restored");
        let kind = world.get::<ItemKind>(item).expect("a restored item has a kind").0;
        let _ = armory.slots.expect(slot);
        if let Some(shape) = armory.shape(kind) {
            worn.equip(item, shape).expect("the slots exist");
        }
    }
    let bag: Vec<Entity> = p.bag.iter().filter_map(|id| remap.entity(*id)).collect();
    let (faction, unarmed, max_hp) = (bestiary.factions.expect("player"), crate::items::unarmed(&bestiary), 30);
    let player = world
        .spawn((
            (Actor, Player, Blocks, Position(p.at), OnMap(p.map), Viewshed::new(12), RevealsMap, Speed(100)),
            (
                Health { hp: p.hp, max: max_hp },
                Armor(0),
                Faction(faction),
                unarmed,
                Inventory { items: bag },
                Equipped(worn),
                StatBlock::default(),
                Afflicted::default(),
                Strikes::default(),
                rl_engine::rl_render::Glyph::new('@', Color::WHITE).on_layer(10),
            ),
        ))
        .id();
    remap.bind(p.id, player);
    // Statuses go back on by request, so their modifiers are installed
    // the same way they were the first time.
    for (name, turns) in &p.statuses {
        let status = world.resource::<StatusRules>().defs.expect(name);
        world.write_message(Afflict { target: player, status, turns: *turns, by: None });
    }
    save.engine.restore(world, &remap);
    let mut bestiary = bestiary;
    let mut armory = armory;
    bestiary.restore_spawned(save.spawned_monsters.iter().copied());
    armory.restore_spawned(save.spawned_loot.iter().copied());
    world.insert_resource(armory);
    world.insert_resource(bestiary);
    world.resource_mut::<Entrances>().0 = save.entrances.iter().copied().collect();
    world.resource_mut::<Quests>().tracker = save.quests.clone();
}

/// `S` saves; `q` saves then quits. Both only while playing.
pub fn save_keys(world: &mut World) {
    let keys = world.resource::<ButtonInput<KeyCode>>();
    let wants_save = keys.just_pressed(KeyCode::KeyS) && keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let wants_quit = keys.just_pressed(KeyCode::KeyQ);
    if !wants_save && !wants_quit {
        return;
    }
    let playing = *world.resource::<State<EngineState>>().get() == EngineState::Playing;
    if playing {
        let turn = world.resource::<Turns>().turn_number();
        match save_run(world) {
            Ok(()) => world.resource_mut::<MessageLog>().push("The run is written in the log book.", LogCategory::Notice, turn),
            Err(e) => {
                error!("save failed: {e}");
                world.resource_mut::<MessageLog>().push(format!("The save failed: {e}"), LogCategory::Bad, turn);
            }
        }
    }
    if wants_quit {
        world.write_message(AppExit::Success);
    }
}

/// Death ends the run: the save goes with it.
pub fn delete_on_death(mut deaths: MessageReader<DeathEvent>, saves: Res<Saves>) {
    for d in deaths.read() {
        if d.was_player
            && let Err(e) = saves.delete(SLOT)
        {
            error!("could not delete the save: {e}");
        }
    }
}

/// Where continuing lands, for the log.
pub fn describe(save: &RunSave) -> String {
    format!("Continuing from turn {} with {} things in the bag.", save.turn, save.player.bag.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rl_engine::rl_core::{Direction, RunSeed};
    use rl_engine::rl_save::FileBackend;

    /// A headless Corsair: the engine plugins and the resources the
    /// startup system reads, without a window.
    fn headless(seed: RunSeed, resume: bool, dir: &std::path::Path) -> App {
        let mut app = rl_engine::rl_bevy::plugin::headless_app();
        app.insert_resource(crate::StartSeed { seed, regions: (24, 24), resume })
            .insert_resource(Saves(Box::new(FileBackend::new(dir))))
            .init_resource::<MessageLog>()
            .init_resource::<crate::inventory::InventoryScreen>()
            .init_resource::<crate::places::Entrances>()
            .init_resource::<crate::quests::LedgerScreen>()
            .add_systems(Startup, crate::start_world)
            .add_systems(Update, (crate::monsters::spawn_on_load, crate::items::scatter_on_load, crate::places::mark_entrances).in_set(EngineSet::Stream))
            .add_systems(Update, crate::items::refresh_gear.in_set(EngineSet::Present));
        app
    }

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

    #[test]
    fn a_run_saved_and_continued_is_the_same_run() {
        let dir = std::env::temp_dir().join(format!("corsair-save-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = headless(RunSeed(7), false, &dir);
        app.update();
        app.update();
        let me = player(&mut app);
        for _ in 0..3 {
            app.world_mut().write_message(Intent { actor: me, action: Action::Move(Direction::East) });
            app.update();
        }
        let bottle = app.world().get::<Inventory>(me).unwrap().items[1];
        app.world_mut().write_message(Intent { actor: me, action: Action::Drop(bottle) });
        app.update();
        let (pos, hp, bag, turn) = {
            let w = app.world();
            (w.get::<Position>(me).unwrap().0, w.get::<Health>(me).unwrap().hp, w.get::<Inventory>(me).unwrap().items.len(), w.resource::<Turns>().now())
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
        assert_eq!(w.get::<Health>(me2).unwrap().hp, hp);
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

    #[test]
    fn the_shape_round_trips_through_the_versioned_envelope() {
        let text =
            encode(VERSION, &TransitionSave { at: Point::new(1, 2), map: MapId(3), to: Destination::Place { map: MapId(4), arrive: Arrive::Exit } }).unwrap();
        let back: TransitionSave = decode(VERSION, &text).unwrap();
        assert_eq!(back.map, MapId(3));
        assert!(matches!(decode::<TransitionSave>(VERSION + 1, &text), Err(SaveError::Version { .. })));
    }
}
