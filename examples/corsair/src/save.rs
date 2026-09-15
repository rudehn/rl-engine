//! Saving and continuing a run.
//!
//! One versioned RON blob: the engine's state through [`EngineSave`], and
//! everything Corsair spawned, by definition name and save id. The world
//! itself is not saved; it regenerates from the seed, and the engine
//! replays the edits on top. `S` saves, `q` saves and quits, `--continue`
//! loads, and death deletes the save so a run cannot be resumed past it.
//! A window or tab closed on the run saves it too: every turn the run is
//! encoded into the engine's [`Stash`], which the unload bridge writes on
//! the way out.

use bevy::prelude::*;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::Point;
use rl_engine::rl_rules::Tracker;
use rl_engine::rl_rules::{Enchanted, Equipment};
use rl_engine::rl_save::{EngineSave, EntityRemap, SaveBackend, SaveError, SaveId, Saves, Stash, decode, encode};
use rl_engine::rl_ui::{AbilityKeys, Bindings, Controls, MessageLog, ScrollbackKeys, SheetKeys, Tones};
use serde::{Deserialize, Serialize};

use crate::input::Binds;
use crate::items::{Armory, ItemKind};
use crate::monsters::{Bestiary, MonsterKind};
use crate::places::Entrances;

/// Bump when the shape below changes so an old save would parse wrongly.
// v1: the first shape.
// v2: the engine's knowledge keeps its own bucket size and records the
//     surface regions seen, rather than taking the world's region size.
pub const VERSION: u32 = 2;

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
    /// What it carries, empty for a monster without hands.
    #[serde(default)]
    pub bag: Vec<SaveId>,
    /// Which of those it wears.
    #[serde(default)]
    pub worn: Vec<SaveId>,
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
type MonsterData =
    (Entity, &'static MonsterKind, &'static Position, Option<&'static OnMap>, &'static Health, Option<&'static Inventory>, Option<&'static Equipped>);
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
    registries: Res<'w, Registries>,
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
        worn: worn.worn().map(|(slot, item)| (run.registries.slots.name(slot).to_string(), remap.save_id(item))).collect(),
        statuses: afflicted.iter().map(|s| (run.registries.statuses.name(s.id).to_string(), s.turns)).collect(),
    };
    let monsters = run
        .monsters
        .iter()
        .map(|(e, kind, pos, on, hp, carried, worn)| MonsterSave {
            id: remap.save_id(e),
            def: run.bestiary.defs.name(kind.0).to_string(),
            at: pos.0,
            map: map_of(on),
            hp: hp.hp,
            bag: carried.map(|b| b.items.iter().map(|i| remap.save_id(*i)).collect()).unwrap_or_default(),
            worn: worn.map(|w| w.worn().map(|(_, i)| remap.save_id(i)).collect()).unwrap_or_default(),
        })
        .collect();
    // Everything carried, the player's and every monster's, since an item in
    // a bag has no position to be found by.
    let carried: Vec<Entity> = bag.items.iter().copied().chain(run.monsters.iter().filter_map(|m| m.5).flat_map(|b| b.items.iter().copied())).collect();
    let items = run
        .items
        .iter()
        .filter(|(e, _, _, pos, _, _)| pos.is_some() || carried.contains(e))
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

/// Writes the run to the save slot, and stashes it for the bridge that
/// writes on the way out.
pub fn save_run(world: &mut World) -> Result<(), SaveError> {
    let text = encode_run(world)?;
    world.resource::<Saves>().persist(SLOT, &text)?;
    world.resource::<Stash>().stash(SLOT, &text);
    Ok(())
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
        Ok(text) => world.resource::<Stash>().stash(SLOT, &text),
        Err(e) => warn!("the run could not be stashed: {e}"),
    }
}

/// The run as one versioned blob.
fn encode_run(world: &mut World) -> Result<String, SaveError> {
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
    let regions = (world.resource::<WorldRes>().width(), world.resource::<WorldRes>().height());
    let engine = EngineSave::capture(world, &mut remap);
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
    encode(VERSION, &save)
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
            let e = if m.map.is_surface() { bestiary.spawn(&mut commands, id, m.at) } else { bestiary.spawn_underground(&mut commands, id, m.at) };
            commands.entity(e).insert((OnMap(m.map), Health { hp: m.hp, max: bestiary.defs.get(id).hp }));
            e
        };
        world.flush();
        remap.bind(m.id, e);
        // What it carried and wore, rather than the kit a fresh one gets.
        if !m.bag.is_empty() {
            let items: Vec<Entity> = m.bag.iter().filter_map(|id| remap.entity(*id)).collect();
            let mut worn = Equipment::for_slots(&world.resource::<Registries>().slots);
            for item in m.worn.iter().filter_map(|id| remap.entity(*id)) {
                if let Some(shape) = world.get::<ItemKind>(item).and_then(|k| armory.shape(k.0)) {
                    let _ = worn.equip(item, shape);
                }
            }
            world.entity_mut(e).insert((Inventory { items }, Equipped(worn)));
        }
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
    let mut worn = Equipment::for_slots(&world.resource::<Registries>().slots);
    for (slot, id) in &p.worn {
        let item = remap.entity(*id).expect("worn item restored");
        let kind = world.get::<ItemKind>(item).expect("a restored item has a kind").0;
        let _ = world.resource::<Registries>().slots.expect(slot);
        if let Some(shape) = armory.shape(kind) {
            worn.equip(item, shape).expect("the slots exist");
        }
    }
    let bag: Vec<Entity> = p.bag.iter().filter_map(|id| remap.entity(*id)).collect();
    let (faction, unarmed, max_hp) = (world.resource::<Registries>().factions.expect("player"), crate::items::unarmed(&armory), 30);
    let grants = crate::abilities::player_grants(world.resource::<rl_engine::rl_bevy::Abilities>());
    let player = world
        .spawn((
            (Actor, Player, Blocks, Position(p.at), OnMap(p.map), Viewshed::new(12), RevealsMap),
            (
                Health { hp: p.hp, max: max_hp },
                Armor(0),
                Faction(faction),
                unarmed,
                Inventory { items: bag },
                Equipped(worn),
                Strikes::default(),
                rl_engine::rl_render::Glyph::new('@', Color::WHITE).on_layer(10),
            ),
            // What a fresh player has and a save does not record: what it
            // knows, its name on the panels, and that it can hide.
            (grants, Name::new("you"), rl_engine::rl_bevy::Stealth(rl_engine::rl_rules::ai::awareness::StealthStats { quiet: 1, subtlety: 10 })),
        ))
        .id();
    remap.bind(p.id, player);
    // Statuses go back on by request, so their modifiers are installed
    // the same way they were the first time.
    for (name, turns) in &p.statuses {
        let status = world.resource::<Registries>().statuses.expect(name);
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

/// Death ends the run: the save goes with it, and so does the stash, or
/// closing the window afterwards would write the run back.
pub fn delete_on_death(mut deaths: MessageReader<DeathEvent>, saves: Res<Saves>, stash: Res<Stash>) {
    for d in deaths.read() {
        if !d.was_player {
            continue;
        }
        stash.clear();
        if let Err(e) = saves.delete(SLOT) {
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
    use crate::testing::headless;
    use rl_engine::rl_core::{Direction, RunSeed};

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

    /// The stash follows the run a turn at a time, and death clears it, so
    /// a window closed after dying writes nothing back.
    #[test]
    fn the_stash_follows_the_run_and_death_clears_it() {
        let dir = std::env::temp_dir().join(format!("corsair-stash-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = headless(RunSeed(7), false, &dir);
        app.add_systems(Update, delete_on_death.in_set(PresentSet::Narrate));
        app.update();
        app.update();
        let stash = app.world().resource::<Stash>().clone();
        assert_eq!(stash.pending().as_deref(), Some(SLOT), "stashed by the first turn");

        let me = player(&mut app);
        let at = app.world().get::<Position>(me).unwrap().0;
        app.world_mut().write_message(DeathEvent { entity: me, at, credit: None, was_player: true });
        app.update();
        assert_eq!(stash.pending(), None, "cleared with the save");
        app.world_mut().write_message(AppExit::Success);
        app.update();
        assert!(!app.world().resource::<Saves>().exists(SLOT), "closing the window after dying wrote nothing back");
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

    /// A monster saved carrying knives is continued carrying the same knives,
    /// not the kit a fresh one would be handed.
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
