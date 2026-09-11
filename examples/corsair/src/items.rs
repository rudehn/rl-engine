//! Items: definitions from RON, what lies about, what the dead drop, what
//! rum does, and what wearing a jerkin is worth.
//!
//! The engine moves items between ground, bag and slots and charges the
//! turns. Everything that gives an item meaning is here: the slot names,
//! the stat it modifies, the effect of using it, the words in the log.

use std::collections::BTreeSet;

use bevy::prelude::*;
use rand::Rng;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_content::{BandedEntry, BandedTable, Named, Registry};
use rl_engine::rl_core::{DiceRoll, Id, Point, RunSeed, SeedDomain, geometry};
use rl_engine::rl_render::Glyph;
use rl_engine::rl_rules::damage::DamageKind;
use rl_engine::rl_rules::{EquipShape, Modifier, Op, SlotDef, StatDef, StatId, Stats};
use rl_engine::rl_ui::{LogCategory, MessageLog};
use serde::Deserialize;

use crate::content::PORT;

const ITEMS_RON: &str = include_str!("../assets/items.ron");

/// One kind of item, as authored.
#[derive(Debug, Clone, Deserialize)]
pub struct ItemDef {
    pub name: String,
    pub glyph: char,
    pub color: (f32, f32, f32),
    #[serde(default)]
    pub slot: Option<String>,
    #[serde(default)]
    pub also: Vec<String>,
    #[serde(default)]
    pub armor: i32,
    #[serde(default)]
    pub attack: Option<DiceRoll>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub heal: i32,
    #[serde(default)]
    pub stack: bool,
    #[serde(default)]
    pub spawn: Option<(i32, i32, u32)>,
}

impl Named for ItemDef {
    fn name(&self) -> &str {
        &self.name
    }
}

impl ItemDef {
    /// Whether using it does anything.
    pub fn usable(&self) -> bool {
        self.heal > 0
    }
}

/// Marks an item with the def it came from.
#[derive(Component, Debug, Clone, Copy)]
pub struct ItemKind(pub Id<ItemDef>);

/// A wearer's stat block, rebuilt from what it wears.
#[derive(Component, Debug, Clone, Default)]
pub struct Sheet(pub Stats);

/// The item definitions and the vocabulary they use.
#[derive(Resource)]
pub struct Armory {
    pub defs: Registry<ItemDef>,
    pub slots: Registry<SlotDef>,
    pub stats: Registry<StatDef>,
    pub armor_stat: StatId,
    shapes: Vec<Option<EquipShape>>,
    table: BandedTable<Id<ItemDef>>,
    seed: RunSeed,
    home: Point,
    spawned: BTreeSet<Point>,
}

impl Armory {
    /// Loads and validates the items; panics with every problem listed.
    pub fn load(seed: RunSeed, home: Point, kinds: &Registry<DamageKind>) -> Self {
        let slots = Registry::from_defs(vec![SlotDef::new("main hand"), SlotDef::new("off hand"), SlotDef::new("body"), SlotDef::new("head")]).unwrap();
        let stats = Registry::from_defs(vec![StatDef::new("armor", 0).clamp(0, 20)]).unwrap();
        let defs: Registry<ItemDef> = Registry::from_ron_str(ITEMS_RON).unwrap_or_else(|e| panic!("assets/items.ron: {e}"));
        defs.validate(|d, _| {
            for s in d.slot.iter().chain(d.also.iter()) {
                if slots.id(s).is_none() {
                    return Err(format!("unknown slot {s:?}"));
                }
            }
            if d.slot.is_none() && !d.also.is_empty() {
                return Err("also without a slot".into());
            }
            if d.attack.is_some() != d.kind.is_some() {
                return Err("attack and kind go together".into());
            }
            if let Some(k) = &d.kind
                && kinds.id(k).is_none()
            {
                return Err(format!("unknown damage kind {k:?}"));
            }
            if d.stack && d.slot.is_some() {
                return Err("a worn item cannot stack".into());
            }
            Ok(())
        })
        .unwrap_or_else(|e| panic!("assets/items.ron: {e}"));

        let mut shapes = Vec::new();
        let mut table = BandedTable::default();
        for (id, d) in defs.iter() {
            shapes.push(d.slot.as_ref().map(|s| {
                let mut shape = EquipShape::in_slot(slots.expect(s));
                for a in &d.also {
                    shape = shape.and_claims(slots.expect(a));
                }
                shape
            }));
            if let Some((lo, hi, w)) = d.spawn {
                table.push(BandedEntry::new(id).bands(lo, hi).weight(w));
            }
        }
        Self {
            armor_stat: stats.expect("armor"),
            defs,
            slots,
            stats,
            shapes,
            table,
            seed,
            home,
            spawned: BTreeSet::new(),
        }
    }

    /// The shape `id` is worn in, if it is worn at all.
    pub fn shape(&self, id: Id<ItemDef>) -> Option<&EquipShape> {
        self.shapes[id.index()].as_ref()
    }

    /// Spawns one `id`, or a stack of `count`, on the ground at `at` or
    /// nowhere when `at` is `None`.
    pub fn spawn(&self, commands: &mut Commands, id: Id<ItemDef>, count: u32, at: Option<Point>) -> Entity {
        let d = self.defs.get(id);
        let mut e = commands.spawn((Item, ItemKind(id), Glyph::new(d.glyph, Color::srgb(d.color.0, d.color.1, d.color.2)).on_layer(2)));
        if let Some(shape) = self.shape(id) {
            e.insert(Wearable(shape.clone()));
        }
        if d.stack {
            e.insert(Stack { key: id.raw() as u64, count });
        }
        if let Some(p) = at {
            e.insert(Position(p));
        }
        e.id()
    }

    /// How many a fresh stack of `id` holds.
    fn stack_size(&self, id: Id<ItemDef>, rng: &mut impl Rng) -> u32 {
        if self.defs.get(id).stack { rng.random_range(1..=12) } else { 1 }
    }
}

/// Scatters a few items over each region the first time it streams in.
pub fn scatter_on_load(mut commands: Commands, mut loaded: MessageReader<ChunkLoaded>, mut armory: ResMut<Armory>, world: Res<WorldRes>, map: Res<WorldMap>) {
    for ev in loaded.read() {
        let region = ev.region;
        if !armory.spawned.insert(region) {
            continue;
        }
        let band = geometry::chebyshev(region, armory.home);
        let index = ((region.x as u32 as u64) << 32) | (region.y as u32 as u64);
        let mut rng = armory.seed.rng(SeedDomain::new(b"corsair.loot"), index);
        let count = if world.site_at(region).is_some_and(|s| s.kind == PORT) { 3 } else { rng.random_range(0..=2) };
        let tiles = world.region_tiles(region);
        for _ in 0..count {
            let Some(entry) = armory.table.pick(band, &mut rng) else { continue };
            let id = entry.item;
            for _ in 0..8 {
                let p = Point::new(rng.random_range(tiles.x..tiles.right()), rng.random_range(tiles.y..tiles.bottom()));
                if map.is_walkable(p) {
                    let n = armory.stack_size(id, &mut rng);
                    armory.spawn(&mut commands, id, n, Some(p));
                    break;
                }
            }
        }
    }
}

/// What the dead leave behind, from the bestiary's drop lists.
pub fn drop_loot(mut commands: Commands, mut deaths: MessageReader<DeathEvent>, armory: Res<Armory>, bestiary: Res<crate::monsters::Bestiary>, mut rng: ResMut<CombatRng>, kinds: Query<&crate::monsters::MonsterKind>) {
    for d in deaths.read() {
        let Ok(kind) = kinds.get(d.entity) else { continue };
        for (name, pct) in &bestiary.defs.get(kind.0).drops {
            if rng.random_range(0..100) < *pct {
                let id = armory.defs.expect(name);
                let n = armory.stack_size(id, &mut **rng);
                armory.spawn(&mut commands, id, n, Some(d.at));
            }
        }
    }
}

/// Applies what using an item does, and consumes it.
pub fn use_items(mut commands: Commands, mut events: MessageReader<ItemEvent>, armory: Res<Armory>, turns: Res<Turns>, mut log: ResMut<MessageLog>, mut users: Query<&mut Health>, mut items: Query<(&ItemKind, Option<&mut Stack>)>) {
    for ev in events.read() {
        let ItemEvent::Used { actor, item } = *ev else { continue };
        let Ok((kind, stack)) = items.get_mut(item) else { continue };
        let d = armory.defs.get(kind.0);
        if !d.usable() {
            log.push(format!("You cannot think what to do with the {}.", d.name), LogCategory::Muted, turns.turn_number());
            continue;
        }
        if let Ok(mut hp) = users.get_mut(actor) {
            let before = hp.hp;
            hp.hp = (hp.hp + d.heal).min(hp.max);
            log.push(format!("You drink the {}. It restores {} health.", d.name, hp.hp - before), LogCategory::Good, turns.turn_number());
        }
        match stack {
            Some(mut s) if s.count > 1 => s.count -= 1,
            _ => commands.entity(item).despawn(),
        }
    }
}

/// Rebuilds a wearer's stats, armor and attack from what it wears.
pub fn refresh_gear(armory: Res<Armory>, bestiary: Res<crate::monsters::Bestiary>, mut wearers: Query<(&Equipped, &mut Sheet, &mut Armor, &mut MeleeAttack), Changed<Equipped>>, kinds: Query<&ItemKind>) {
    let main_hand = armory.slots.expect("main hand");
    for (worn, mut sheet, mut armor, mut attack) in &mut wearers {
        let mut stats = Stats::new();
        for (_, item) in worn.worn() {
            let Ok(kind) = kinds.get(item) else { continue };
            let d = armory.defs.get(kind.0);
            if d.armor != 0 {
                stats.add(Modifier::new(armory.armor_stat, Op::Add(d.armor), item.to_bits()));
            }
        }
        armor.0 = stats.value(armory.armor_stat, &armory.stats);
        *attack = match worn.in_slot(main_hand).and_then(|e| kinds.get(e).ok()).map(|k| armory.defs.get(k.0)) {
            Some(ItemDef { attack: Some(dice), kind: Some(kind), .. }) => MeleeAttack { kind: bestiary.kinds.expect(kind), dice: *dice },
            _ => unarmed(&bestiary),
        };
        sheet.0 = stats;
    }
}

/// A bare-knuckle strike.
pub fn unarmed(bestiary: &crate::monsters::Bestiary) -> MeleeAttack {
    MeleeAttack { kind: bestiary.kinds.expect("fist"), dice: DiceRoll::new(1, 3) }
}

/// Turns item events into log lines.
pub fn narrate_items(mut events: MessageReader<ItemEvent>, armory: Res<Armory>, turns: Res<Turns>, mut log: ResMut<MessageLog>, items: Query<(&ItemKind, Option<&Stack>)>, players: Query<(), With<Player>>) {
    let turn = turns.turn_number();
    let describe = |item: Entity| -> String {
        match items.get(item) {
            Ok((k, Some(s))) if s.count > 1 => format!("{} {}", s.count, armory.defs.get(k.0).name),
            Ok((k, _)) => format!("the {}", armory.defs.get(k.0).name),
            Err(_) => "something".into(),
        }
    };
    for ev in events.read() {
        let (actor, text, cat) = match *ev {
            ItemEvent::PickedUp { actor, item, merged_into } => (actor, format!("You pick up {}.", describe(merged_into.unwrap_or(item))), LogCategory::Info),
            ItemEvent::Dropped { actor, item, .. } => (actor, format!("You drop {}.", describe(item)), LogCategory::Info),
            ItemEvent::Equipped { actor, item } => {
                let verb = if items.get(item).is_ok_and(|(k, _)| armory.defs.get(k.0).attack.is_some()) { "wield" } else { "put on" };
                (actor, format!("You {verb} {}.", describe(item)), LogCategory::Info)
            }
            ItemEvent::Unequipped { actor, item } => (actor, format!("You take off {}.", describe(item)), LogCategory::Muted),
            ItemEvent::Used { .. } => continue,
        };
        if players.get(actor).is_ok() {
            log.push(text, cat, turn);
        }
    }
}

/// Names what lies at `p`, for the status line.
pub fn whats_here(p: Point, armory: &Armory, ground: &Query<(&Position, &ItemKind, Option<&Stack>), With<Item>>) -> Option<String> {
    let names: Vec<String> = ground
        .iter()
        .filter(|(pos, _, _)| pos.0 == p)
        .map(|(_, k, s)| match s {
            Some(s) if s.count > 1 => format!("{} {}", s.count, armory.defs.get(k.0).name),
            _ => armory.defs.get(k.0).name.clone(),
        })
        .collect();
    (!names.is_empty()).then(|| names.join(", "))
}
