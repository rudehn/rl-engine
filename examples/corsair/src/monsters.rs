//! Monsters: definitions from RON, and spawning as regions stream in. What
//! they do is narrated by the engine, from the events they raise.

use std::collections::BTreeSet;
use std::sync::Arc;

use bevy::prelude::*;
use rand::Rng;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::{DiceRoll, Point, RunSeed, SeedDomain, geometry};
use rl_engine::rl_render::Glyph;
use rl_engine::rl_rules::Brain;
use rl_engine::rl_rules::ai::awareness::NoticeStats;
use rl_engine::rl_rules::ai::tactics::SearchLastKnown;
use rl_engine::rl_rules::ai::tactics::UseAbility;
use rl_engine::rl_rules::ai::tactics::{FleeWhenHurt, Hunt, MeleeAdjacent, Scavenge, ThrowAtRange, Wander};
use rl_engine::rl_rules::damage::{DamageKind, SubtractArmor};
use rl_engine::rl_rules::faction::FactionDef;
use rl_engine::rl_rules::{AbilityDef, Equipment, NameRef, Names, StatusDef, Wits};
use rl_engine::rl_rules::{BandedEntry, BandedTable, Named, Registry};
use serde::Deserialize;

use crate::content::PORT;

const MONSTERS_RON: &str = include_str!("../assets/monsters.ron");

/// One kind of monster, as authored.
#[derive(Debug, Clone, Deserialize)]
pub struct MonsterDef {
    pub name: String,
    pub glyph: char,
    pub color: (f32, f32, f32),
    pub hp: i32,
    pub armor: i32,
    pub attack: DiceRoll,
    pub kind: NameRef<DamageKind>,
    pub faction: NameRef<FactionDef>,
    pub wits: Wits,
    pub perception: i32,
    pub speed: u32,
    pub flee_at: i32,
    pub wander: u32,
    pub spawn: (i32, i32, u32, u32, u32),
    #[serde(default)]
    pub drops: Vec<(NameRef<crate::items::ItemDef>, u32)>,
    #[serde(default)]
    pub inflicts: Option<(NameRef<StatusDef>, u32, u32)>,
    #[serde(default)]
    pub lantern: Option<LightSource>,
    #[serde(default)]
    pub notice: Option<NoticeStats>,
    #[serde(default)]
    pub abilities: Vec<NameRef<AbilityDef>>,
    #[serde(default)]
    pub purse: Option<(u32, u32)>,
    #[serde(default)]
    pub kit: Vec<(NameRef<crate::items::ItemDef>, u32)>,
}

impl Named for MonsterDef {
    fn name(&self) -> &str {
        &self.name
    }
}

impl rl_engine::rl_rules::ThreatSubject for MonsterDef {
    fn hp(&self) -> i32 {
        self.hp
    }
    fn armor(&self) -> i32 {
        self.armor
    }
    fn damage_per_hit(&self) -> f32 {
        self.attack.avg()
    }
    fn speed_pct(&self) -> u32 {
        self.speed
    }
}

/// Marks a monster with the def it came from.
#[derive(Component, Debug, Clone, Copy)]
pub struct MonsterKind(pub rl_engine::rl_core::Id<MonsterDef>);

/// The bestiary and the spawn table built from it.
#[derive(Resource)]
pub struct Bestiary {
    pub defs: Registry<MonsterDef>,
    pub table: BandedTable<rl_engine::rl_core::Id<MonsterDef>>,
    brains: Vec<Arc<Brain<Entity>>>,
    /// How many equipment slots a monster with hands has to fill.
    slots: usize,
    seed: RunSeed,
    home: Point,
    spawned: BTreeSet<Point>,
}

impl Bestiary {
    /// Loads the bestiary against `names`, which must hold the damage kinds,
    /// the sides, the items, the statuses and the abilities a monster is
    /// written with, for monsters that wear up to `slots` things; panics
    /// listing every problem.
    pub fn load(seed: RunSeed, home: Point, names: &Names, slots: usize) -> Self {
        let defs: Registry<MonsterDef> = names.load(MONSTERS_RON).unwrap_or_else(|e| panic!("assets/monsters.ron: {e}"));
        defs.validate(|m, _| {
            if m.hp <= 0 {
                return Err("hp must be positive".into());
            }
            if !m.kit.is_empty() && !m.wits.has(Wits::PICKS_UP) && !m.wits.has(Wits::EQUIPS) {
                return Err("a kit needs wits that pick up or equip, or there is no bag to carry it in".into());
            }
            Ok(())
        })
        .unwrap_or_else(|e| panic!("assets/monsters.ron: {e}"));

        let mut table = BandedTable::default();
        let mut brains = Vec::new();
        for (id, m) in defs.iter() {
            let (lo, hi, w, gmin, gmax) = m.spawn;
            table.push(BandedEntry::new(id).bands(lo, hi).weight(w).group(gmin, gmax));
            // One shape of brain for the whole bestiary: a kind that knows an
            // ability leads with it when one is worth firing, and the wits say
            // which of the rest a crab can manage and a cutthroat can.
            let mut brain = if m.abilities.is_empty() { Brain::new() } else { Brain::new().then(UseAbility { chance_pct: 35 }) }.then(MeleeAdjacent);
            if m.flee_at > 0 {
                brain = brain.then(FleeWhenHurt { at_pct: m.flee_at });
            }
            let brain = brain.then(ThrowAtRange { chance_pct: 70 }).then(Scavenge { reach: 4 });
            brains.push(Arc::new(brain.then(Hunt).then(SearchLastKnown).then(Wander { chance_pct: m.wander })));
        }
        Self { defs, table, brains, slots, seed, home, spawned: BTreeSet::new() }
    }
}

impl Bestiary {
    /// Regions already populated.
    pub fn spawned(&self) -> impl Iterator<Item = &Point> {
        self.spawned.iter()
    }

    /// Marks regions as populated, when continuing a run.
    pub fn restore_spawned(&mut self, regions: impl IntoIterator<Item = Point>) {
        self.spawned = regions.into_iter().collect();
    }

    /// Spawns one `id` standing at `p` on the current map, underground, where
    /// whatever carries a lantern has it lit.
    pub fn spawn_underground(&self, commands: &mut Commands, id: rl_engine::rl_core::Id<MonsterDef>, p: Point) -> Entity {
        let e = self.spawn(commands, id, p);
        if let Some(lantern) = self.defs.get(id).lantern {
            commands.entity(e).insert(lantern);
        }
        // Only below: the caves are dark and have somewhere to hide, and
        // the islands in daylight deliberately do not.
        if let Some(notice) = self.defs.get(id).notice {
            commands.entity(e).insert(Notice(notice));
        }
        e
    }

    /// Spawns one `id` standing at `p` on the current map.
    pub fn spawn(&self, commands: &mut Commands, id: rl_engine::rl_core::Id<MonsterDef>, p: Point) -> Entity {
        let m = self.defs.get(id);
        let e = commands
            .spawn((
                Actor,
                Blocks,
                Position(p),
                Health::full(m.hp),
                Armor(m.armor),
                Faction(m.faction.id()),
                MeleeAttack { kind: m.kind.id(), dice: m.attack },
                Perception(m.perception),
                Speed(m.speed),
                Mind(self.brains[id.index()].clone()),
                // One brain shape for the whole bestiary; the wits say how
                // much of it a crab can use.
                Intelligence(m.wits),
                MonsterKind(id),
                // What the panels call it. The engine has no bestiary.
                Name::new(m.name.clone()),
                Glyph::new(m.glyph, Color::srgb(m.color.0, m.color.1, m.color.2)).on_layer(5),
            ))
            .id();
        if !m.abilities.is_empty() {
            commands.entity(e).insert(Grants(m.abilities.iter().map(|a| a.id()).collect()));
        }
        if let Some((min, max)) = m.purse {
            // From the position rather than a stream, so a monster restored
            // from a save carries what it spawned with.
            let span = (max.max(min) - min + 1) as u64;
            let coin = min + (rl_engine::rl_core::seed::position_hash(self.seed.0, p.x, p.y) % span) as u32;
            commands.entity(e).insert(crate::abilities::Purse(coin));
        }
        // Hands: a bag to carry what it picks up and slots for what it puts
        // on. Its own blow and hide above stay what they are; the engine
        // reads worn gear on top of them.
        if m.wits.has(Wits::PICKS_UP) || m.wits.has(Wits::EQUIPS) {
            commands.entity(e).insert((Inventory::default(), Equipped(Equipment::with_slot_count(self.slots))));
        }
        e
    }

    /// Hands a freshly spawned `id` what its kind carries when it first
    /// appears. Not for a monster restored from a save, which carries what it
    /// had when it was saved.
    pub fn arm(&self, commands: &mut Commands, armory: &crate::items::Armory, monster: Entity, id: rl_engine::rl_core::Id<MonsterDef>) {
        let kit = &self.defs.get(id).kit;
        if kit.is_empty() {
            return;
        }
        let items = kit.iter().map(|(item, count)| armory.spawn(commands, item.id(), *count, None)).collect();
        commands.entity(monster).insert(Inventory { items });
    }
}

/// Where a region's monsters are placed: the world, the map, who already
/// stands where, and the player they must not appear beside.
#[derive(bevy::ecs::system::SystemParam)]
pub struct SpawnSite<'w, 's> {
    world: Res<'w, WorldRes>,
    map: Res<'w, WorldMap>,
    occupancy: Res<'w, Occupancy>,
    player: Query<'w, 's, &'static Position, With<Player>>,
}

/// Populates each region the first time it streams in, by its distance
/// from the starting town.
pub fn spawn_on_load(
    mut commands: Commands,
    mut loaded: MessageReader<ChunkLoaded>,
    mut bestiary: ResMut<Bestiary>,
    armory: Res<crate::items::Armory>,
    site: SpawnSite,
) {
    let SpawnSite { world, map, occupancy, player } = site;
    let player_pos = player.single().map(|p| p.0).unwrap_or(Point::ZERO);
    for ev in loaded.read() {
        let region = ev.region;
        if !bestiary.spawned.insert(region) || world.site_at(region).is_some_and(|s| s.kind == PORT) {
            continue;
        }
        let band = geometry::chebyshev(region, bestiary.home);
        let index = ((region.x as u32 as u64) << 32) | (region.y as u32 as u64);
        let mut rng = bestiary.seed.rng(SeedDomain::new(b"corsair.spawns"), index);
        let groups = rng.random_range(0..=2);
        let tiles = world.region_tiles(region);
        for _ in 0..groups {
            let Some((id, count)) = bestiary.table.pick_group(band, &mut rng) else { continue };
            let id = *id;
            let anchor = Point::new(rng.random_range(tiles.x..tiles.right()), rng.random_range(tiles.y..tiles.bottom()));
            let mut placed = 0;
            for p in geometry::square(anchor, 3) {
                if placed >= count {
                    break;
                }
                if !map.is_walkable(p) || occupancy.is_occupied(p) || geometry::chebyshev(p, player_pos) < 6 {
                    continue;
                }
                let monster = bestiary.spawn(&mut commands, id, p);
                bestiary.arm(&mut commands, &armory, monster, id);
                placed += 1;
            }
        }
    }
}

/// Damage stages: Corsair uses armor only, for now.
pub fn stages() -> DamageStages {
    DamageStages(vec![Box::new(SubtractArmor)])
}

#[cfg(test)]
mod tests {
    use super::*;

    use rl_engine::rl_core::{Direction, RunSeed};
    use rl_engine::rl_ui::{VitalsView, VitalsViewPlugin};

    /// The bestiary's wits load as written: a crab has none to speak of, a
    /// dog runs but cannot work a latch, and a cutthroat can.
    #[test]
    fn the_bestiary_says_who_can_work_a_door() {
        let loaded = crate::rules::load(RunSeed(1), Point::ZERO, &crate::rules::effect_kinds());
        let defs = &loaded.bestiary.defs;
        let wits = |name: &str| defs.get(defs.expect(name)).wits;
        assert_eq!(wits("crab"), Wits::MINDLESS);
        assert!(wits("wild dog").has(Wits::FLEES) && !wits("wild dog").has(Wits::OPENS_DOORS));
        assert!(wits("cutthroat").has(Wits::OPENS_DOORS));
    }

    /// A cutthroat comes with knives, throws one at a player out of arm's
    /// reach, and the knife lies on the ground where it fell.
    #[test]
    fn a_cutthroat_throws_one_of_its_knives_at_a_player_out_of_arms_reach() {
        #[derive(Resource, Default)]
        struct Throws(Vec<(Entity, Entity)>);
        fn record(mut events: MessageReader<ItemEvent>, mut throws: ResMut<Throws>) {
            throws.0.extend(events.read().filter_map(|e| match *e {
                ItemEvent::Thrown { actor, item, .. } => Some((actor, item)),
                _ => None,
            }));
        }
        let dir = std::env::temp_dir().join(format!("corsair-knives-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = crate::testing::headless(RunSeed(7), false, &dir);
        app.init_resource::<Throws>().add_systems(PostUpdate, record);
        app.update();
        app.update();
        let me = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        let at = app.world().get::<Position>(me).unwrap().0;
        // Four steps off down a clear line, on whichever side has one.
        let along = |dir: Direction, n: i32| {
            let (dx, dy) = dir.delta();
            at.offset(dx * n, dy * n)
        };
        let clear = |app: &App, dir: Direction| {
            (1..=4).all(|n| app.world().resource::<WorldMap>().is_walkable(along(dir, n)) && !app.world().resource::<Occupancy>().is_occupied(along(dir, n)))
        };
        let side = Direction::ALL.into_iter().find(|d| !d.is_diagonal() && clear(&app, *d)).expect("a clear line from the player");
        let kind = app.world().resource::<Bestiary>().defs.expect("cutthroat");
        let cutthroat = app.world_mut().resource_scope(|world: &mut World, bestiary: Mut<Bestiary>| {
            world.resource_scope(|world: &mut World, armory: Mut<crate::items::Armory>| {
                let mut queue = bevy::ecs::world::CommandQueue::default();
                let mut commands = Commands::new(&mut queue, world);
                let e = bestiary.spawn(&mut commands, kind, along(side, 4));
                bestiary.arm(&mut commands, &armory, e, kind);
                // Throwing and nothing else, so the test is about the knives
                // rather than about which tactic the dice favoured.
                commands.entity(e).insert(Mind(Arc::new(Brain::new().then(ThrowAtRange::default()))));
                queue.apply(world);
                e
            })
        });
        app.update();
        let carried = app.world().get::<Inventory>(cutthroat).map(|b| b.items.len());
        assert_eq!(carried, Some(1), "it came with a stack of knives");

        app.world_mut().write_message(Intent::new(me, Wait));
        app.update();
        let throws = &app.world().resource::<Throws>().0;
        let [(by, knife)] = throws.as_slice() else { panic!("one throw: {throws:?}") };
        assert_eq!(*by, cutthroat);
        assert!(app.world().get::<Health>(me).is_some_and(|h| h.hp < h.max), "the knife struck");
        assert_eq!(app.world().get::<Position>(*knife).map(|p| p.0), Some(at), "and lies at the player's feet");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Reported from play: on the surface, a cutthroat was cutting the
    /// player down while the vitals strip still read "hidden". Surface
    /// monsters carry no `Notice`, so they see on sight and never keep an
    /// `Aware`, and "seen" asked only the observers that did.
    #[test]
    fn a_player_under_attack_on_the_surface_is_never_reported_hidden() {
        let dir = std::env::temp_dir().join(format!("corsair-seen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = crate::testing::headless(RunSeed(7), false, &dir);
        // The real game's vitals and stealth, which the shared harness
        // leaves out.
        app.add_plugins((StealthPlugin, VitalsViewPlugin));
        app.update();
        app.update();

        let me = {
            let w = app.world_mut();
            let mut q = w.query_filtered::<Entity, With<Player>>();
            q.single(w).unwrap()
        };
        assert!(app.world().get::<Stealth>(me).is_some(), "Corsair's player can hide");
        let at = app.world().get::<Position>(me).expect("a position").0.offset(1, 0);
        let kind = app.world().resource::<Bestiary>().defs.expect("cutthroat");
        app.world_mut().resource_scope(|world: &mut World, bestiary: Mut<Bestiary>| {
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let mut commands = Commands::new(&mut queue, world);
            // `spawn`, not `spawn_underground`: a surface cutthroat, no Notice.
            bestiary.spawn(&mut commands, kind, at);
            queue.apply(world);
        });
        app.update();

        let full = app.world().get::<Health>(me).expect("health").hp;
        for _ in 0..6 {
            app.world_mut().write_message(Intent::new(me, Wait));
            app.update();
        }
        let hp = app.world().get::<Health>(me).map(|h| h.hp).unwrap_or(0);
        assert!(hp < full, "the cutthroat at the player's elbow attacked: {hp} of {full}");
        assert_eq!(app.world().resource::<VitalsView>().seen, Some(true), "a player being cut down has been seen");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
