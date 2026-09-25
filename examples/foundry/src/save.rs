//! Saving a run and continuing it.
//!
//! The engine walks the world and writes the run down: where everything
//! stands, its health, its bag and what it wears, its statuses and stack,
//! a thing's charges, where a lift leads, the props with their history,
//! the dead left as wrecks, every deck built, the clock and what the
//! commando has seen. Foundry says only what each kind of thing it spawns
//! is: the commando and whether its lamp was lit, a droid by its
//! definition, an item by its definition and its heat, a lift by how it is
//! drawn. What Foundry keeps of a run outside its entities, the upgrades
//! taken and the deepest deck reached, goes through [`SaveableState`], and
//! the mission's tracker and ledger are the engine's to save.
//!
//! The run is written on the way out, by the engine's stash when the
//! window closes or the menu quits, and on every deck arrival, so a crash
//! loses at most the deck in hand. There is no save key. Death and a win
//! delete it. The title screen's Continue reads it back through
//! [`run::resume`](crate::run::resume).

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_save::{AddSaveable, SavePlugin, Saveable, SaveableState, Saves, Stash, UnloadPlugin};
use serde::{Deserialize, Serialize};

use crate::climb::Deepest;
use crate::droids::{Kind, Roster};
use crate::gear::ItemKind;
use crate::heat::{Heat, Stowed};
use crate::lifts::{Lift, LiftOut};
use crate::run::Commando;
use crate::upgrades::{Taken, Upgrade};

/// Bump when a kind's shape below changes so an old save would parse
/// wrongly.
// v1: the first shape.
pub const VERSION: u32 = 1;

/// The slot every run saves to.
pub const SLOT: &str = "foundry";

/// What the save is made of: the engine's plugins, the four kinds, and the
/// four resources.
pub fn register(app: &mut App) {
    app.add_plugins((SavePlugin::new(SLOT).version(VERSION).on_arrival(), UnloadPlugin))
        .save_kind::<ItemKind>()
        .save_kind::<Kind>()
        .save_kind::<Commando>()
        .save_kind::<Lift>()
        .save_state::<Taken>()
        .save_state::<Deepest>()
        .save_state::<Quests>()
        .save_state::<Counters>();
}

/// Deletes the saved run, for a New Game that abandons it.
pub fn abandon(world: &mut World) {
    world.resource::<Stash>().clear();
    if let Err(e) = world.resource::<Saves>().delete(SLOT) {
        error!("could not delete the save: {e}");
    }
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

/// The commando, as the save writes it: whether its lamp was lit. The
/// engine gives back where it stood, its health, its bag, its gear and
/// its statuses; its upgrades go back on once all of that is back.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CommandoSave {
    /// Whether the shoulder lamp was on.
    pub lamp: bool,
}

impl Saveable for Commando {
    type Saved = CommandoSave;

    fn capture(world: &World, entity: Entity) -> CommandoSave {
        CommandoSave { lamp: world.get::<LightSource>(entity).is_some() }
    }

    fn restore(world: &mut World, saved: &CommandoSave) -> Entity {
        let registries = world.resource::<Registries>().clone();
        let e = spawned(world, |commands, _| crate::run::spawn_commando(commands, &registries));
        if !saved.lamp {
            world.entity_mut(e).remove::<LightSource>();
        }
        e
    }
}

/// A droid or a critter, as the save writes it: its definition, and
/// whether it had its own glyph taken off, which is what a wreck is: the
/// engine lays it back down as remains dressed as the wreckage prop, and
/// the renderer dresses only what wears no glyph of its own.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroidSave {
    /// The definition's name.
    pub def: String,
    /// Whether it was drawn as something other than itself.
    #[serde(default)]
    pub bare: bool,
}

impl Saveable for Kind {
    type Saved = DroidSave;

    fn capture(world: &World, entity: Entity) -> DroidSave {
        let kind = world.get::<Kind>(entity).expect("a droid's kind");
        DroidSave { def: world.resource::<Roster>().defs.name(kind.0).to_string(), bare: world.get::<Glyph>(entity).is_none() }
    }

    fn restore(world: &mut World, saved: &DroidSave) -> Entity {
        let Some(id) = world.resource::<Roster>().defs.id(&saved.def) else {
            warn!("a saved droid is a {:?}, which this build has no definition for; it comes back as nothing", saved.def);
            return world.spawn_empty().id();
        };
        let registries = world.resource::<Registries>().clone();
        let e = spawned(world, |commands, world| {
            // Nowhere and on no deck: the engine puts it back where it was.
            crate::droids::spawn_monster(commands, world.resource::<Roster>(), id, Point::ZERO, MapId::SURFACE, &registries)
        });
        if saved.bare {
            world.entity_mut(e).remove::<Glyph>();
        }
        e
    }
}

/// An item, as the save writes it: what it was made from, and how hot it
/// runs if it runs hot at all, which is the one thing about an item the
/// run changes that the engine does not keep.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemSave {
    /// The definition's name.
    pub def: String,
    /// Heat carried, and whether it had locked.
    #[serde(default)]
    pub heat: Option<(u32, bool)>,
}

impl Saveable for ItemKind {
    type Saved = ItemSave;

    fn capture(world: &World, entity: Entity) -> ItemSave {
        let kind = world.get::<ItemKind>(entity).expect("an item's kind");
        let armory = world.resource::<crate::gear::Armory>();
        ItemSave { def: armory.defs.name(kind.0).to_string(), heat: world.get::<Heat>(entity).map(|h| (h.now, h.locked)) }
    }

    fn restore(world: &mut World, saved: &ItemSave) -> Entity {
        let Some(id) = world.resource::<crate::gear::Armory>().defs.id(&saved.def) else {
            warn!("a saved item is a {:?}, which this build has no definition for; it comes back as nothing", saved.def);
            return world.spawn_empty().id();
        };
        let e = spawned(world, |commands, world| {
            let armory = world.resource::<crate::gear::Armory>();
            crate::gear::spawn_item(commands, armory, id, world.resource::<Registries>())
        });
        if let (Some((now, locked)), Some(mut heat)) = (saved.heat, world.get_mut::<Heat>(e)) {
            heat.now = now;
            heat.locked = locked;
        }
        // A locked weapon has its attack put by, as heat puts it by the
        // turn it locks, so the loadout passes over it until it cools.
        if saved.heat.is_some_and(|(_, locked)| locked) {
            let mut item = world.entity_mut(e);
            if let Some(attack) = item.take::<RangedAttack>() {
                item.insert(Stowed::Ranged(attack));
            } else if let Some(attack) = item.take::<MeleeAttack>() {
                item.insert(Stowed::Melee(attack));
            }
        }
        e
    }
}

/// A lift, as the save writes it: how it is drawn, and whether it is the
/// lift out. The engine keeps where a lift leads.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LiftSave {
    /// Its glyph.
    pub glyph: char,
    /// Whether it is the lift out on deck one.
    pub out: bool,
}

impl Saveable for Lift {
    type Saved = LiftSave;

    fn capture(world: &World, entity: Entity) -> LiftSave {
        LiftSave { glyph: world.get::<Glyph>(entity).map_or('<', |g| g.ch), out: world.get::<LiftOut>(entity).is_some() }
    }

    fn restore(world: &mut World, saved: &LiftSave) -> Entity {
        let name = match (saved.out, saved.glyph) {
            (true, _) => "lift out",
            (false, '>') => "lift down",
            (false, _) => "lift up",
        };
        let mut e = world.spawn((Lift, Name::new(name), Glyph::new(saved.glyph, crate::lifts::LIFT).on_layer(1)));
        if saved.out {
            e.insert(LiftOut);
        }
        e.id()
    }
}

impl SaveableState for Taken {
    type Saved = Vec<Upgrade>;

    fn capture(&self) -> Vec<Upgrade> {
        self.0.clone()
    }

    fn restore(&mut self, saved: Vec<Upgrade>) {
        self.0 = saved;
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
    use bevy::prelude::*;
    use rl_engine::prelude::*;
    use rl_engine::rl_bevy::testing::{KeyScriptPlugin, press};
    use rl_engine::rl_core::RunSeed;
    use rl_engine::rl_save::{Saves, save_run};

    use super::*;
    use crate::heat::Heat;
    use crate::upgrades::{Taken, Upgrade};

    /// What a run looks like from outside, for comparing a run with the
    /// same run continued.
    #[derive(Debug, PartialEq)]
    struct Looks {
        deck: u32,
        at: Point,
        health: i32,
        lamp: bool,
        taken: Vec<Upgrade>,
        deepest: u32,
        first_charge_done: bool,
        pack: Vec<(String, u32)>,
        heat: Vec<(u32, bool)>,
        reach: Vec<i32>,
        spent_consoles: usize,
        droids: Vec<(String, i32)>,
        lifts: usize,
        wrecks: Vec<(Point, bool, bool)>,
        lamps: usize,
    }

    fn player(app: &mut App) -> Entity {
        app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap()
    }

    fn looks(app: &mut App) -> Looks {
        let me = player(app);
        let world = app.world();
        let deck = crate::decks::deck_of(world.get::<OnMap>(me).map(|m| m.0).unwrap_or(MapId::SURFACE));
        let bag = world.get::<Inventory>(me).unwrap().items.clone();
        let mut pack: Vec<(String, u32)> =
            bag.iter().map(|i| (world.get::<Name>(*i).unwrap().as_str().to_string(), world.get::<Stack>(*i).map_or(1, |s| s.count))).collect();
        pack.sort();
        let mut heat: Vec<(u32, bool)> = bag.iter().filter_map(|i| world.get::<Heat>(*i)).map(|h| (h.now, h.locked)).collect();
        heat.sort();
        let mut reach: Vec<i32> = bag.iter().filter_map(|i| world.get::<RangedAttack>(*i)).map(|r| r.range).collect();
        reach.sort();
        let spent = world.resource::<Registries>().props.expect("spent reactor console");
        let quests = world.resource::<Quests>();
        let first = quests.defs.expect("first_charge");
        let roster = world.resource::<crate::droids::Roster>();
        let mut droids = Vec::new();
        let mut spent_consoles = 0;
        let mut lifts = 0;
        let mut wrecks = Vec::new();
        let mut lamps = 0;
        for e in world.iter_entities() {
            if let (Some(kind), Some(health), None) = (e.get::<crate::droids::Kind>(), e.get::<Health>(), e.get::<rl_engine::rl_bevy::Remains>()) {
                droids.push((roster.defs.name(kind.0).to_string(), health.current));
            }
            if e.get::<PropKind>().is_some_and(|k| k.0 == spent) {
                spent_consoles += 1;
            }
            if e.contains::<crate::lifts::Lift>() {
                lifts += 1;
            }
            // A wreck: where it lies, whether it still wears the droid's
            // own glyph, and whether it is dressed as the wreckage prop.
            if e.contains::<rl_engine::rl_bevy::Remains>() {
                wrecks.push((e.get::<Position>().map_or(Point::ZERO, |p| p.0), e.contains::<Glyph>(), e.contains::<PropKind>()));
            }
            // A wall lamp: a light nobody carries.
            if e.contains::<LightSource>() && !e.contains::<Player>() && !e.contains::<crate::droids::Kind>() {
                lamps += 1;
            }
        }
        droids.sort();
        wrecks.sort_by_key(|(p, _, _)| (p.x, p.y));
        Looks {
            deck,
            at: world.get::<Position>(me).unwrap().0,
            health: world.get::<Health>(me).unwrap().current,
            lamp: world.get::<LightSource>(me).is_some(),
            taken: world.resource::<Taken>().0.clone(),
            deepest: world.resource::<crate::climb::Deepest>().0,
            first_charge_done: quests.tracker.state(first) == QuestState::Done,
            pack,
            heat,
            reach,
            spent_consoles,
            droids,
            lifts,
            wrecks,
            lamps,
        }
    }

    /// A run on deck three with the console charged through the real
    /// bump and the uplink picked on the real pick screen, servos fitted
    /// beside it, a hand blaster worn and fired until it is half hot,
    /// slugs in the pack, the lamp off and a droid wounded.
    fn a_run_on_deck_three() -> App {
        let mut app = crate::testing::headless(RunSeed(2));
        app.add_plugins(KeyScriptPlugin);
        let me = crate::testing::beside_the_console(&mut app);
        crate::testing::settle(&mut app);
        let into_it = crate::testing::key_toward_the_console(&app, me);
        press(&mut app, into_it);
        crate::testing::settle(&mut app);
        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::Enter);
        crate::testing::settle(&mut app);
        assert!(app.world().get::<crate::upgrades::Uplinked>(me).is_some(), "the uplink was picked");
        crate::upgrades::apply(Upgrade::Servos, me, app.world_mut());
        app.world_mut().resource_mut::<Taken>().0.push(Upgrade::Servos);
        crate::testing::equip_new(&mut app, me, "hand blaster");
        crate::testing::fire_at_a_target(&mut app, me, 2);
        crate::testing::give_slugs(&mut app, me, 7);
        app.world_mut().entity_mut(me).remove::<LightSource>();
        let droids: Vec<Entity> = app.world_mut().query_filtered::<Entity, (With<crate::droids::Kind>, With<Health>)>().iter(app.world()).collect();
        assert!(droids.len() >= 2, "deck three has droids to wound and to wreck");
        app.world_mut().get_mut::<Health>(droids[0]).unwrap().current -= 1;
        // And one killed, which leaves a wreck.
        let kinetic = app.world().resource::<Registries>().damage_kinds.expect("kinetic");
        app.world_mut().write_message(DamageEvent::new(droids[1], rl_engine::rl_rules::Hit::by(me, kinetic, 1_000)));
        crate::testing::settle(&mut app);
        app
    }

    /// Everything a run is, saved on deck three and continued in a fresh
    /// app, comes back as it was, and the continued run plays on.
    #[test]
    fn a_run_saved_on_deck_three_comes_back_as_it_was() {
        let mut app = a_run_on_deck_three();
        let before = looks(&mut app);
        assert!(before.heat.iter().any(|(now, _)| *now > 0), "the blaster is warm: {before:?}");
        assert_eq!(before.spent_consoles, 1, "the console is spent");
        assert!(!before.wrecks.is_empty(), "a droid lies wrecked");
        assert!(before.lamps > 0, "and the decks seen have their wall lamps");
        save_run(app.world_mut()).expect("the run saves");
        let text = app.world().resource::<Saves>().load(SLOT).unwrap().expect("a save");

        let mut continued = crate::testing::continued(&text);
        assert_eq!(looks(&mut continued), before);
        crate::testing::pass_turns(&mut continued, 5);
        assert_eq!(*continued.world().resource::<State<EngineState>>().get(), EngineState::Playing, "and it plays on");
    }

    /// The uplink's extra tile of reach, on a gun worn when the run was
    /// saved, comes back once: given again by the upgrade, not saved on
    /// the gun and then given a second time.
    #[test]
    fn the_uplinks_reach_comes_back_once_on_a_gun_worn_when_saved() {
        let mut app = a_run_on_deck_three();
        let before = looks(&mut app).reach;
        save_run(app.world_mut()).expect("the run saves");
        let text = app.world().resource::<Saves>().load(SLOT).unwrap().expect("a save");
        let mut continued = crate::testing::continued(&text);
        assert_eq!(looks(&mut continued).reach, before);
    }

    /// A charge set and its pick not yet taken when the run was saved: the
    /// pick is offered again on the way back, rather than lost with the
    /// frame it was waiting in.
    #[test]
    fn a_pick_pending_when_the_run_was_saved_is_offered_again() {
        let mut app = crate::testing::headless(RunSeed(2));
        app.add_plugins(KeyScriptPlugin);
        let me = crate::testing::beside_the_console(&mut app);
        crate::testing::settle(&mut app);
        let into_it = crate::testing::key_toward_the_console(&app, me);
        press(&mut app, into_it);
        crate::testing::settle(&mut app);
        assert!(app.world().resource::<crate::upgrades::Choosing>().0.is_some(), "the pick is up");
        save_run(app.world_mut()).expect("the run saves");
        let text = app.world().resource::<Saves>().load(SLOT).unwrap().expect("a save");

        let continued = crate::testing::continued(&text);
        assert!(continued.world().resource::<crate::upgrades::Choosing>().0.is_some(), "and up again on the way back");
        let pick = continued.world().resource::<Modals>().get(crate::upgrades::MODAL).unwrap();
        assert!(continued.world().resource::<Modals>().is_top(pick), "on top");
    }

    /// A save that names something the content no longer has, an item
    /// renamed since, continues without it rather than failing to start.
    #[test]
    fn a_save_naming_what_the_content_no_longer_has_continues_without_it() {
        let mut app = a_run_on_deck_three();
        save_run(app.world_mut()).expect("the run saves");
        let text = app.world().resource::<Saves>().load(SLOT).unwrap().expect("a save").replace("\"hand blaster\"", "\"nonesuch\"");
        assert!(text.contains("nonesuch"), "the save named the blaster");
        let mut continued = crate::testing::continued(&text);
        assert_eq!(*continued.world().resource::<State<EngineState>>().get(), EngineState::Playing, "the run goes on");
        crate::testing::pass_turns(&mut continued, 2);
    }

    /// Arriving on a deck writes the save, so each deck is a save point.
    #[test]
    fn a_deck_arrival_writes_the_save() {
        let mut app = crate::testing::headless(RunSeed(3));
        crate::testing::settle(&mut app);
        app.world().resource::<Saves>().delete(SLOT).unwrap();
        crate::testing::arrive_on(&mut app, 2);
        assert!(app.world().resource::<Saves>().exists(SLOT), "written on arrival");
    }

    /// A death deletes the save: nothing is continued past its end.
    #[test]
    fn a_death_deletes_the_save() {
        let mut app = crate::testing::headless(RunSeed(3));
        crate::testing::settle(&mut app);
        assert!(app.world().resource::<Saves>().exists(SLOT), "the first arrival wrote it");
        let me = player(&mut app);
        let kinetic = app.world().resource::<Registries>().damage_kinds.expect("kinetic");
        app.world_mut().write_message(DamageEvent::new(me, rl_engine::rl_rules::Hit::from_source(None, kinetic, 1_000)));
        crate::testing::settle(&mut app);
        assert_eq!(*app.world().resource::<State<EngineState>>().get(), EngineState::Over);
        assert!(!app.world().resource::<Saves>().exists(SLOT), "deleted");
    }

    /// A win deletes the save too.
    #[test]
    fn a_win_deletes_the_save() {
        let mut app = crate::testing::headless(RunSeed(3));
        crate::testing::settle(&mut app);
        let kind = app.world().resource::<crate::mission::Facts>().charge_set;
        for deck in [3u64, 6, 9, 10] {
            app.world_mut().write_message(Happened(Fact::new(kind).about(deck)));
            app.update();
        }
        let me = player(&mut app);
        app.world_mut().write_message(Intent::new(me, GoThrough));
        crate::testing::settle(&mut app);
        crate::testing::settle(&mut app);
        assert_eq!(*app.world().resource::<State<EngineState>>().get(), EngineState::Over);
        assert!(!app.world().resource::<Saves>().exists(SLOT), "deleted");
    }
}
