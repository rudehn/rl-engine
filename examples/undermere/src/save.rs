//! Saving a run and continuing it.
//!
//! The engine walks the world and writes the run down: where everything
//! stands, its health, its pack and what it wears, its statuses and which
//! worn thing holds each, a stack's count and a thing's charges, where a
//! way through leads, every place built, the clock and what the
//! adventurer has seen. Undermere says only what each kind of thing it
//! spawns is: the adventurer and the strength they have come to, a
//! monster by its definition's name, an item by its definition's name,
//! its level, whether it is still cursed on, how long it has been worn
//! towards knowing its level and how much of it the tally has counted, and
//! a way through by which way it is. What Undermere keeps of a run outside
//! its entities, the [`Tally`] and the [`Lore`] of what is known, goes
//! through [`SaveableState`], and the engine's `Quests` is saved the same
//! way. It keeps no `Counters`, since the tally counts what the score is
//! made of.
//!
//! The run is written on the way out, by the engine's stash when the
//! window closes or the menu quits, by the engine again when the menu
//! leaves the run for the title, and on every arrival, so a crash loses at
//! most the floor in hand. There is no save key. Death and a win delete
//! it. Continue reads it back through [`run::resume`](crate::run::resume).
//!
//! The records of every run that has ended are kept apart, in a slot of
//! their own that no run's end deletes, by the engine's
//! `RecordsSavePlugin`.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_save::{AddSaveable, RecordsSavePlugin, SavePlugin, Saveable, SaveableState, UnloadPlugin};
use serde::{Deserialize, Serialize};

use crate::armory::{Armory, ItemKind};
use crate::bestiary::{Bestiary, Kind};
use crate::bestiary_lore::MonsterLore;
use crate::lore::{CurseShown, Lore, Unfamiliar};
use crate::run::Adventurer;
use crate::stairs::Way;
use crate::tally::{Counted, Tally};
use crate::vaults::VaultKey;

/// Bump when a kind's shape below changes so an old save would parse
/// wrongly.
// v1: the first shape.
// v2: the tally in place of the deepest floor, and how much of an item the
// tally has counted.
// v3: an item's level, curse and familiarity, the adventurer's strength,
// what is known of the run's things, and a finger slot on either hand.
// v4: what is known of the run's monsters' tricks.
pub const VERSION: u32 = 4;

/// The slot every run saves to.
pub const SLOT: &str = "undermere";

// ANCHOR: register
/// What the save is made of: the engine's plugins, the four kinds, and
/// the four resources; and the records, in their own slot.
pub fn register(app: &mut App) {
    app.add_plugins((SavePlugin::new(SLOT).version(VERSION).on_arrival(), UnloadPlugin, RecordsSavePlugin))
        .save_kind::<ItemKind>()
        .save_kind::<Kind>()
        .save_kind::<Adventurer>()
        .save_kind::<Way>()
        .save_state::<Tally>()
        .save_state::<Lore>()
        .save_state::<MonsterLore>()
        .save_state::<crate::vaults::lifts::RisingFloods>()
        .save_state::<Quests>();
}
// ANCHOR_END: register

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

/// The adventurer, as the save writes them: the strength they have come
/// to, and nothing else of their own, since everything else they are,
/// where they stand, their health, their pack, what they wear and the
/// statuses on them, is the engine's to keep, and their light is the
/// lantern's, which is an item.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct AdventurerSave {
    /// Their strength with nothing on it, which a potion of strength
    /// raised for good; `None` in a save written before strength was, read
    /// as the stat's own base. What is worn and the statuses put their own
    /// changes back on top of it.
    #[serde(default)]
    pub strength: Option<i32>,
}

impl Saveable for Adventurer {
    type Saved = AdventurerSave;

    fn capture(world: &World, entity: Entity) -> AdventurerSave {
        let registries = world.resource::<Registries>();
        let stat = registries.stats.id(crate::content::STRENGTH);
        AdventurerSave { strength: stat.zip(world.get::<StatBlock>(entity)).map(|(stat, stats)| stats.0.base(stat, &registries.stats)) }
    }

    fn restore(world: &mut World, saved: &AdventurerSave) -> Entity {
        let registries = world.resource::<Registries>().clone();
        let adventurer = spawned(world, |commands, _| crate::run::spawn_adventurer(commands, &registries));
        if let (Some(strength), Some(stat)) = (saved.strength, registries.stats.id(crate::content::STRENGTH))
            && let Some(mut stats) = world.get_mut::<StatBlock>(adventurer)
        {
            stats.0.set_base(stat, strength);
        }
        adventurer
    }
}

/// A monster, as the save writes it: its definition, by name, whether it
/// fights for the adventurer, and when a conjured one fades. The engine
/// gives back where it stood, its health, what it carried and wore, its
/// statuses and the post it held.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonsterSave {
    /// The definition's name.
    pub def: String,
    /// Whether it fights for the adventurer, dominated, conjured or made
    /// of one that did, which is for good; false in a save written before
    /// anything could.
    #[serde(default)]
    pub ally: bool,
    /// Whether it follows one who freed it by name rather than whichever
    /// ally is nearest: a freed captive, read off whether it
    /// carries the engine's own `Leader` at the moment it is captured.
    /// `Leader` itself is the generic entity-state machinery's to put
    /// back, once every entity of the save has one; this is only the
    /// word for which mind to give it once that happens, since the mind
    /// a captive follows with is not the mind an ordinary ally does.
    #[serde(default)]
    pub following: bool,
    /// The moment on the turn clock a conjured thing fades at, which the
    /// save keeps with the clock; `None` for anything else.
    #[serde(default)]
    pub fades: Option<u32>,
    /// Where a captive not yet freed is held, which puts its side, its
    /// anchor and its offer to be freed back on it; `None` for anything
    /// else, and in a save written before a continued run kept them, whose
    /// captive came back an ordinary monster of its kind.
    #[serde(default)]
    pub shackled: Option<Point>,
    /// The side it fights for, by name, when that is neither its kind's
    /// own nor the adventurer's, which `ally` says: a goblin conjurer's
    /// blades, a body a necromancer raised. `None` for anything else, and
    /// in a save written before a continued run kept it, whose blades came
    /// back on the side their kind is written with.
    #[serde(default)]
    pub side: Option<String>,
    /// Whether it is a body stood back up, whose second death drops
    /// nothing and counts nothing: see
    /// [`Raised`](crate::effects::bodies::Raised).
    #[serde(default)]
    pub raised: bool,
    /// Whether it has stolen something and is running with it: see
    /// [`Stole`](crate::tricks::Stole).
    #[serde(default)]
    pub stole: bool,
}

/// The side `monster` fights for when that is neither its kind's own nor
/// the adventurer's, nor a captive's, which their own fields say: the side
/// it went back to after a discord, if it is discordant now, since the
/// discord's status is the engine's to keep and turns it again.
fn own_side(world: &World, monster: Entity, kind: Id<crate::bestiary::MonsterDef>) -> Option<String> {
    if world.get::<crate::allies::Ally>(monster).is_some() || crate::captives::held_at(world, monster).is_some() {
        return None;
    }
    let side = match world.get::<crate::conditions::Discordant>(monster) {
        Some(turned) => turned.was,
        None => *world.get::<Faction>(monster)?,
    };
    let factions = &world.resource::<Registries>().factions;
    (side.0 != world.resource::<Bestiary>().defs.get(kind).faction).then(|| factions.name(side.0).to_string())
}

impl Saveable for Kind {
    type Saved = MonsterSave;

    fn capture(world: &World, entity: Entity) -> MonsterSave {
        let kind = world.get::<Kind>(entity).expect("a monster's kind");
        MonsterSave {
            def: world.resource::<Bestiary>().defs.name(kind.0).to_string(),
            // `Leader` alone is a band follower's as readily as a freed
            // captive's; only paired with `Ally` does it mean the
            // follower fights for the adventurer and is a freed captive,
            // since nothing else on the adventurer's own side gets a
            // named leader to begin with.
            ally: world.get::<crate::allies::Ally>(entity).is_some(),
            following: world.get::<crate::allies::Ally>(entity).is_some() && world.get::<Leader>(entity).is_some(),
            fades: world.get::<crate::allies::Fades>(entity).map(|f| f.at),
            shackled: crate::captives::held_at(world, entity),
            side: own_side(world, entity, kind.0),
            raised: world.get::<crate::effects::bodies::Raised>(entity).is_some(),
            stole: world.get::<crate::tricks::Stole>(entity).is_some(),
        }
    }

    fn restore(world: &mut World, saved: &MonsterSave) -> Entity {
        let Some(id) = world.resource::<Bestiary>().defs.id(&saved.def) else {
            warn!("a saved monster is a {:?}, which this build has no definition for; it comes back as nothing", saved.def);
            return world.spawn_empty().id();
        };
        let registries = world.resource::<Registries>().clone();
        let monster = spawned(world, |commands, world| {
            // Nowhere and on no place: the engine puts it back where it was.
            crate::bestiary::spawn_monster(commands, world.resource::<Bestiary>().defs.get(id), Point::ZERO, MapId::SURFACE, &registries, false)
        });
        if saved.ally {
            crate::allies::befriend(world, monster);
        }
        if saved.following {
            // Overrides `befriend`'s own ally mind with the one that
            // follows by name, and gives back the wit to travel with its
            // leader: the generic entity state the save already restores
            // puts `Leader` back on this same entity once every entity of
            // the save exists to point it at, so the follower only has to
            // be shaped to read it, not to know yet who it names.
            crate::captives::follow(world, monster);
        }
        if let Some(at) = saved.fades {
            world.entity_mut(monster).insert(crate::allies::Fades { at });
        }
        if let Some(at) = saved.shackled {
            let shackles = crate::captives::shackled(&registries, world.resource::<Verbs>(), at);
            world.entity_mut(monster).insert(shackles);
        }
        if let Some(side) = saved.side.as_deref().and_then(|name| registries.factions.id(name)) {
            crate::allies::fight_for(world, monster, side);
        }
        if saved.raised {
            crate::effects::bodies::forget_its_life(world, monster);
        }
        if saved.stole {
            world.entity_mut(monster).insert(crate::tricks::Stole);
        }
        monster
    }
}

/// An item, as the save writes it: its definition, by name, its level,
/// whether it is still cursed on, how long the adventurer has worn it
/// towards knowing its level, and how much of it the tally has counted as
/// found. Its stack and its charges are the engine's to keep, and what it
/// lends is its definition's, made again with it at its level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemSave {
    /// The definition's name.
    pub def: String,
    /// How many of it the tally has counted, nought for what the
    /// adventurer has never held: a pile of gold they carry or dropped is
    /// not counted again in the run carried on.
    #[serde(default)]
    pub counted: u32,
    /// Its level, nought for a thing that takes none.
    #[serde(default)]
    pub level: i32,
    /// Whether it will not come off once worn: a thing below nought is
    /// made stuck, and one a scroll freed is not, at the same level.
    #[serde(default)]
    pub stuck: bool,
    /// The turns the adventurer has worn it towards knowing its level, for
    /// a thing whose level they do not know yet; `None` once they do.
    #[serde(default)]
    pub unfamiliar: Option<u32>,
    /// Whether its curse has shown itself, clinging to the adventurer, so
    /// they know its level is below nought before they know by how much.
    #[serde(default)]
    pub curse_shown: bool,
    /// For a vault's key, the floor whose vault it opens, which the tag its
    /// lock asks for is put back on from; `None` for anything else, and
    /// in a save written before a vault had a key.
    #[serde(default)]
    pub opens: Option<u32>,
}

impl Saveable for ItemKind {
    type Saved = ItemSave;

    fn capture(world: &World, entity: Entity) -> ItemSave {
        let kind = world.get::<ItemKind>(entity).expect("an item's kind");
        ItemSave {
            def: world.resource::<Armory>().defs.name(kind.0).to_string(),
            counted: world.get::<Counted>(entity).map_or(0, |c| c.0),
            level: world.get::<Enchant>(entity).map_or(0, |e| e.level),
            stuck: world.get::<Stuck>(entity).is_some(),
            unfamiliar: world.get::<Unfamiliar>(entity).map(|u| u.worn),
            curse_shown: world.get::<CurseShown>(entity).is_some(),
            opens: world.get::<VaultKey>(entity).map(|k| k.floor),
        }
    }

    /// Made again at its level by the function that made it, so the level
    /// is applied once and never on top of itself, then held on or let go
    /// as it was when it was saved.
    fn restore(world: &mut World, saved: &ItemSave) -> Entity {
        let Some(id) = world.resource::<Armory>().defs.id(&saved.def) else {
            warn!("a saved item is a {:?}, which this build has no definition for; it comes back as nothing", saved.def);
            return world.spawn_empty().id();
        };
        let registries = world.resource::<Registries>().clone();
        let item = spawned(world, |commands, world| crate::armory::spawn_item_at(commands, world.resource::<Armory>(), id, saved.level, &registries));
        let mut e = world.entity_mut(item);
        if saved.counted > 0 {
            e.insert(Counted(saved.counted));
        }
        match saved.stuck {
            true => e.insert(Stuck),
            false => e.remove::<Stuck>(),
        };
        if let Some(worn) = saved.unfamiliar {
            e.insert(Unfamiliar::worn_for(worn));
        }
        if saved.curse_shown {
            e.insert(CurseShown);
        }
        if let Some(floor) = saved.opens {
            e.insert((VaultKey { floor }, crate::vaults::key_tags(&registries, floor)));
        }
        item
    }
}

/// What is known of the run's things, as the save writes it: the face each
/// kind wears and which kinds are known, both by name.
impl SaveableState for Lore {
    type Saved = Lore;

    fn capture(&self) -> Lore {
        self.clone()
    }

    fn restore(&mut self, saved: Lore) {
        *self = saved;
    }
}

/// What is known of the run's monsters, as the save writes it: the whole
/// of [`MonsterLore`], by the kind's name and the trick's, as `Lore` is.
impl SaveableState for MonsterLore {
    type Saved = MonsterLore;

    fn capture(&self) -> MonsterLore {
        self.clone()
    }

    fn restore(&mut self, saved: MonsterLore) {
        *self = saved;
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

/// The tally, as the save writes it: the whole of it, every field
/// defaulted, so a field added later reads as nought in a run saved
/// before it.
impl SaveableState for Tally {
    type Saved = Tally;

    fn capture(&self) -> Tally {
        self.clone()
    }

    fn restore(&mut self, saved: Tally) {
        *self = saved;
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
            deepest: world.resource::<Tally>().deepest,
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

    /// What `who` wears on the slot called `slot`.
    fn on(app: &App, who: Entity, slot: &str) -> Entity {
        let slot = app.world().resource::<Registries>().slots.expect(slot);
        app.world().get::<Equipped>(who).and_then(|w| w.in_slot(slot)).expect("something worn there")
    }

    /// A continued run keeps what is known and what holds on: the faces
    /// the run's things wear and which kinds are known, the strength a
    /// potion gave for good, a curse still on, one lifted at the level it
    /// was, both still known to be below nought since they clung, and the
    /// turns each has been worn towards knowing by how much.
    #[test]
    fn a_continued_run_keeps_the_faces_the_strength_the_curses_and_the_turns_worn() {
        use crate::testing::{equip, find, give, use_thing};
        let mut app = headless(RunSeed(3));
        let me = adventurer(&mut app);
        let strength = find(&mut app, "potion of strength", 0);
        give(&mut app, me, strength);
        use_thing(&mut app, me, strength);
        let freed = find(&mut app, "ring of light", -2);
        equip(&mut app, me, freed);
        let scroll = find(&mut app, "scroll of remove curse", 0);
        give(&mut app, me, scroll);
        use_thing(&mut app, me, scroll);
        let held = find(&mut app, "ring of stealth", -1);
        equip(&mut app, me, held);
        let draught = find(&mut app, "potion of extra healing", 0);
        give(&mut app, me, draught);
        pass_turns(&mut app, 10);
        let face = app.world().get::<Name>(draught).map(|n| n.as_str().to_string());
        let worn_for = |app: &App, ring: Entity| app.world().get::<Unfamiliar>(ring).map(|u| u.worn);
        let before = (worn_for(&app, freed), worn_for(&app, held));
        assert!(before.0.is_some_and(|w| w >= 10) && before.1.is_some_and(|w| w >= 10), "{before:?}");
        app.world().resource::<Saves>().delete(SLOT).expect("the slot can be emptied");
        arrive(&mut app, map_of(1));
        let text = app.world().resource::<Saves>().load(SLOT).expect("the slot reads").expect("the arrival wrote the run down");
        let lore = app.world().resource::<Lore>().clone();
        let before = (worn_for(&app, freed), worn_for(&app, held));

        let mut carried = continued(&text);
        let me = adventurer(&mut carried);
        assert_eq!(*carried.world().resource::<Lore>(), lore, "the faces and what is known");
        let registries = carried.world().resource::<Registries>().clone();
        let stats = &carried.world().get::<StatBlock>(me).expect("stats").0;
        assert_eq!(stats.base(registries.stats.expect(crate::content::STRENGTH), &registries.stats), 13, "the strength a potion gave");
        let (freed, held) = (on(&carried, me, "left finger"), on(&carried, me, "right finger"));
        let level = |ring: Entity| carried.world().get::<Enchant>(ring).map(|e| e.level);
        assert_eq!((level(freed), carried.world().get::<Stuck>(freed).is_some()), (Some(-2), false), "lifted, at the level it was");
        assert_eq!((level(held), carried.world().get::<Stuck>(held).is_some()), (Some(-1), true), "still held on");
        assert_eq!((worn_for(&carried, freed), worn_for(&carried, held)), before, "and the turns each has been worn");
        let named = |ring: Entity| carried.world().get::<Name>(ring).map(|n| n.as_str().to_string());
        assert_eq!((named(freed), named(held)), (Some("unidentified ring of light".into()), Some("unidentified ring of stealth".into())), "both unidentified");
        let shown = |ring: Entity| carried.world().get::<CurseShown>(ring).is_some();
        assert!(shown(freed) && shown(held), "and both still known to be below nought");
        let bag = &carried.world().get::<Inventory>(me).expect("a pack").items;
        let names: Vec<Option<String>> = bag.iter().map(|i| carried.world().get::<Name>(*i).map(|n| n.as_str().to_string())).collect();
        assert!(names.contains(&face), "the draught under its face: {names:?}");
    }

    /// What a continued run gives back of who fights for whom and what they
    /// are about: a goblin conjurer's blade still fights for the goblins,
    /// not the adventurer its kind is written with; a body a necromancer
    /// raised still fights for the dead, still raised, with nothing to drop
    /// again; and a monkey that stole something still runs with it.
    #[test]
    fn a_continued_run_keeps_a_blades_side_a_raised_bodys_side_and_a_thiefs_flight() {
        use crate::testing::monster;
        let mut app = headless(RunSeed(3));
        let me = adventurer(&mut app);
        arrive(&mut app, map_of(1));
        let here_at = at(&app, me);
        let registries = app.world().resource::<Registries>().clone();
        let (goblins, undead) = (registries.factions.expect("goblins"), registries.factions.expect("undead"));
        let blade = monster(&mut app, "spectral blade", here_at.offset(30, 30));
        crate::allies::fight_for(app.world_mut(), blade, goblins);
        let raised = monster(&mut app, "goblin", here_at.offset(31, 30));
        crate::allies::fight_for(app.world_mut(), raised, undead);
        crate::effects::bodies::forget_its_life(app.world_mut(), raised);
        let thief = monster(&mut app, "monkey", here_at.offset(32, 30));
        app.world_mut().entity_mut(thief).insert(crate::tricks::Stole);
        rl_engine::rl_save::save_run(app.world_mut()).expect("the run is written down");
        let text = app.world().resource::<Saves>().load(SLOT).expect("the slot reads").expect("a save");

        let mut again = continued(&text);
        let world = again.world_mut();
        let found: Vec<(String, FactionId, bool, bool, bool)> = world
            .query::<(&Name, &Faction, Has<crate::effects::bodies::Raised>, Has<crate::tricks::Stole>, Has<rl_engine::rl_bevy::Drops<crate::armory::ItemDef>>)>(
            )
            .iter(world)
            .map(|(n, f, r, s, d)| (n.as_str().to_string(), f.0, r, s, d))
            .collect();
        let of = |name: &str| found.iter().filter(|f| f.0 == name).cloned().collect::<Vec<_>>();
        assert!(of("spectral blade").iter().any(|f| f.1 == goblins), "the blade still fights for the goblins: {found:?}");
        assert!(of("goblin").iter().any(|f| f.1 == undead && f.2 && !f.4), "the raised goblin still fights for the dead, raised, dropping nothing: {found:?}");
        assert!(of("monkey").iter().any(|f| f.3), "the monkey still runs with what it stole: {found:?}");
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
