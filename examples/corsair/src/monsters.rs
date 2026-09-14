//! Monsters: definitions from RON, spawning as regions stream in, and the
//! narration of what they do.

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
use rl_engine::rl_rules::ai::tactics::{FleeWhenHurt, Hunt, MeleeAdjacent, Wander};
use rl_engine::rl_rules::damage::{DamageKind, SubtractArmor};
use rl_engine::rl_rules::faction::FactionDef;
use rl_engine::rl_rules::{BandedEntry, BandedTable, Named, Registry};
use rl_engine::rl_rules::{Factions, Relation};
use rl_engine::rl_ui::{MessageLog, Tones};
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
    pub kind: String,
    pub faction: String,
    pub perception: i32,
    pub speed: u32,
    pub flee_at: i32,
    pub wander: u32,
    pub spawn: (i32, i32, u32, u32, u32),
    #[serde(default)]
    pub drops: Vec<(String, u32)>,
    #[serde(default)]
    pub inflicts: Option<(String, u32, u32)>,
    #[serde(default)]
    pub lantern: Option<LightSource>,
    #[serde(default)]
    pub notice: Option<NoticeStats>,
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
    pub kinds: Registry<DamageKind>,
    pub factions: Registry<FactionDef>,
    pub table: BandedTable<rl_engine::rl_core::Id<MonsterDef>>,
    brains: Vec<Arc<Brain<Entity>>>,
    seed: RunSeed,
    home: Point,
    spawned: BTreeSet<Point>,
}

impl Bestiary {
    /// Loads and validates the bestiary; panics with every problem listed.
    pub fn load(seed: RunSeed, home: Point) -> (Self, CombatRules) {
        let kinds = Registry::from_defs(vec![
            DamageKind::new("cutlass"),
            DamageKind::new("pistol"),
            DamageKind::new("bite"),
            DamageKind::new("claw"),
            DamageKind::new("fist"),
            DamageKind::new("fire"),
        ])
        .unwrap();
        let factions = Registry::from_defs(vec![
            FactionDef { name: "player".into() },
            FactionDef { name: "beasts".into() },
            FactionDef { name: "cutthroats".into() },
            FactionDef { name: "navy".into() },
        ])
        .unwrap();
        let defs: Registry<MonsterDef> = Registry::from_ron_str(MONSTERS_RON).unwrap_or_else(|e| panic!("assets/monsters.ron: {e}"));
        defs.validate(|m, _| {
            if kinds.id(&m.kind).is_none() {
                return Err(format!("unknown damage kind {:?}", m.kind));
            }
            if factions.id(&m.faction).is_none() {
                return Err(format!("unknown faction {:?}", m.faction));
            }
            if m.hp <= 0 {
                return Err("hp must be positive".into());
            }
            Ok(())
        })
        .unwrap_or_else(|e| panic!("assets/monsters.ron: {e}"));

        let mut relations = Factions::new(&factions);
        let (player, beasts, cutthroats, navy) = (factions.expect("player"), factions.expect("beasts"), factions.expect("cutthroats"), factions.expect("navy"));
        relations.set_mutual(player, beasts, Relation::Hostile);
        relations.set_mutual(player, cutthroats, Relation::Hostile);
        relations.set_mutual(player, navy, Relation::Hostile);
        relations.set_mutual(beasts, cutthroats, Relation::Hostile);
        relations.set_mutual(beasts, navy, Relation::Hostile);
        // The navy hunts pirates; pirates would rather not meet the navy.
        relations.set(navy, cutthroats, Relation::Hostile);

        let mut table = BandedTable::default();
        let mut brains = Vec::new();
        for (id, m) in defs.iter() {
            let (lo, hi, w, gmin, gmax) = m.spawn;
            table.push(BandedEntry::new(id).bands(lo, hi).weight(w).group(gmin, gmax));
            let mut brain = Brain::new().then(MeleeAdjacent);
            if m.flee_at > 0 {
                brain = brain.then(FleeWhenHurt { at_pct: m.flee_at });
            }
            brains.push(Arc::new(brain.then(Hunt).then(SearchLastKnown).then(Wander { chance_pct: m.wander })));
        }
        let rules = CombatRules { kinds: kinds.clone(), factions: relations };
        (Self { defs, kinds, factions, table, brains, seed, home, spawned: BTreeSet::new() }, rules)
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
        commands
            .spawn((
                Actor,
                Blocks,
                Position(p),
                Health::full(m.hp),
                Armor(m.armor),
                Faction(self.factions.expect(&m.faction)),
                MeleeAttack { kind: self.kinds.expect(&m.kind), dice: m.attack },
                Perception(m.perception),
                Speed(m.speed),
                Mind(self.brains[id.index()].clone()),
                MonsterKind(id),
                // What the panels call it. The engine has no bestiary.
                Name::new(m.name.clone()),
                StatBlock::default(),
                Afflicted::default(),
                Glyph::new(m.glyph, Color::srgb(m.color.0, m.color.1, m.color.2)).on_layer(5),
            ))
            .id()
    }
}

/// Populates each region the first time it streams in, by its distance
/// from the starting town.
pub fn spawn_on_load(
    mut commands: Commands,
    mut loaded: MessageReader<ChunkLoaded>,
    mut bestiary: ResMut<Bestiary>,
    world: Res<WorldRes>,
    map: Res<WorldMap>,
    occupancy: Res<Occupancy>,
    player: Query<&Position, With<Player>>,
) {
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
                bestiary.spawn(&mut commands, id, p);
                placed += 1;
            }
        }
    }
}

/// What narration writes.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Voice<'w> {
    log: ResMut<'w, MessageLog>,
    next: ResMut<'w, NextState<EngineState>>,
}

/// Turns hits and deaths into log lines.
pub fn narrate(
    mut dealt: MessageReader<DamageDealt>,
    mut deaths: MessageReader<DeathEvent>,
    mut voice: Voice,
    turns: Res<Turns>,
    bestiary: Res<Bestiary>,
    names: Query<&MonsterKind>,
    players: Query<(), With<Player>>,
) {
    let Voice { log, next } = &mut voice;
    let name = |e: Entity| -> String {
        if players.get(e).is_ok() {
            "you".to_string()
        } else {
            names.get(e).map(|k| format!("the {}", bestiary.defs.get(k.0).name)).unwrap_or_else(|_| "something".into())
        }
    };
    let turn = turns.turn_number();
    for d in dealt.read() {
        let attacker = d.hit.attacker.map(name).unwrap_or_else(|| "something".into());
        let target = name(d.target);
        let you_hit = attacker == "you";
        let verb = if you_hit { "hit" } else { "hits" };
        let (text, cat) = if d.dealt <= 0 {
            (format!("{} {} {} but {} nothing.", cap(&attacker), verb, target, if you_hit { "do" } else { "does" }), Tones::MUTED)
        } else {
            (format!("{} {} {} for {}.", cap(&attacker), verb, target, d.dealt), if target == "you" { Tones::BAD } else { Tones::TEXT })
        };
        log.push(text, cat, turn);
    }
    for d in deaths.read() {
        if d.was_player {
            log.push("You die. Press q to quit.", Tones::BAD, turn);
            next.set(EngineState::Idle);
        } else {
            log.push(format!("{} dies.", cap(&name(d.entity))), Tones::GOOD, turn);
        }
    }
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Damage stages: Corsair uses armor only, for now.
pub fn stages() -> DamageStages {
    DamageStages(vec![Box::new(SubtractArmor)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use rl_engine::rl_core::RunSeed;
    use rl_engine::rl_ui::{UiPlugin, VitalsView, VitalsViewPlugin};

    /// Reported from play: on the surface, a cutthroat was cutting the
    /// player down while the vitals strip still read "hidden". Surface
    /// monsters carry no `Notice`, so they see on sight and never keep an
    /// `Aware`, and "seen" asked only the observers that did.
    #[test]
    fn a_player_under_attack_on_the_surface_is_never_reported_hidden() {
        let dir = std::env::temp_dir().join(format!("corsair-seen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = crate::testing::headless(RunSeed(7), false, &dir);
        // The real game's panels and stealth, which the shared harness
        // leaves out.
        app.add_plugins((StealthPlugin, UiPlugin, VitalsViewPlugin));
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
