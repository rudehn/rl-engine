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
use rl_engine::rl_rules::{
    AffixDef, AffixKind, Enchanted, EnhanceRule, EquipShape, Modifier, Op, Scaled, ScaledStrike, SlotDef, StatDef, StatId, Stats, TagDef, TagId, roll_affixes,
};
use rl_engine::rl_ui::{LogCategory, MessageLog};
use serde::Deserialize;

use crate::content::PORT;

const ITEMS_RON: &str = include_str!("../assets/items.ron");
const AFFIXES_RON: &str = include_str!("../assets/affixes.ron");

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
    pub tags: Vec<String>,
    #[serde(default)]
    pub armor: i32,
    #[serde(default)]
    pub attack: Option<DiceRoll>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub ranged: Option<(i32, DiceRoll, String)>,
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

/// An affix as authored.
#[derive(Debug, Clone, Deserialize)]
struct AffixRon {
    name: String,
    #[serde(default)]
    kind: AffixKind,
    applies_to: Vec<String>,
    #[serde(default)]
    grants: Vec<(String, i32, u32)>,
    #[serde(default)]
    strikes: Vec<(String, u32, u32, u32)>,
    #[serde(default = "one")]
    weight: u32,
}

fn one() -> u32 {
    1
}

impl Named for AffixRon {
    fn name(&self) -> &str {
        &self.name
    }
}

/// How likely found gear is to be more than plain, and how much more.
#[derive(Debug, Clone, Copy)]
pub struct Quality {
    /// Chance of an affix, in percent.
    pub affix_pct: u32,
    /// Enchant levels rolled, inclusive.
    pub levels: (i32, i32),
}

impl Quality {
    /// What washes up on the sand.
    pub const FOUND: Quality = Quality { affix_pct: 25, levels: (0, 1) };
    /// What the smugglers keep.
    pub const HOARD: Quality = Quality { affix_pct: 100, levels: (1, 3) };
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
    pub tags: Registry<TagDef>,
    pub affixes: Registry<AffixDef>,
    pub armor_stat: StatId,
    pub attack_stat: StatId,
    shapes: Vec<Option<EquipShape>>,
    item_tags: Vec<Vec<TagId>>,
    table: BandedTable<Id<ItemDef>>,
    seed: RunSeed,
    home: Point,
    spawned: BTreeSet<Point>,
}

impl Armory {
    /// Loads and validates the items; panics with every problem listed.
    pub fn load(seed: RunSeed, home: Point, kinds: &Registry<DamageKind>) -> Self {
        let slots = Registry::from_defs(vec![SlotDef::new("main hand"), SlotDef::new("off hand"), SlotDef::new("body"), SlotDef::new("head")]).unwrap();
        let stats = Registry::from_defs(vec![StatDef::new("armor", 0).clamp(0, 20), StatDef::new("attack", 0)]).unwrap();
        let tags = Registry::from_defs(["weapon", "blade", "gun", "armor", "shield", "hat"].map(TagDef::new).to_vec()).unwrap();
        let defs: Registry<ItemDef> = Registry::from_ron_str(ITEMS_RON).unwrap_or_else(|e| panic!("assets/items.ron: {e}"));
        defs.validate(|d, _| {
            for t in &d.tags {
                if tags.id(t).is_none() {
                    return Err(format!("unknown tag {t:?}"));
                }
            }
            if let Some((range, _, kind)) = &d.ranged {
                if *range < 2 {
                    return Err("a ranged weapon reaches at least 2".into());
                }
                if kinds.id(kind).is_none() {
                    return Err(format!("unknown damage kind {kind:?}"));
                }
            }
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

        let authored: Registry<AffixRon> = Registry::from_ron_str(AFFIXES_RON).unwrap_or_else(|e| panic!("assets/affixes.ron: {e}"));
        authored
            .validate(|a, _| {
                for t in &a.applies_to {
                    if tags.id(t).is_none() {
                        return Err(format!("{}: unknown tag {t:?}", a.name));
                    }
                }
                for (stat, _, _) in &a.grants {
                    if stats.id(stat).is_none() {
                        return Err(format!("{}: unknown stat {stat:?}", a.name));
                    }
                }
                for (kind, _, _, sides) in &a.strikes {
                    if kinds.id(kind).is_none() {
                        return Err(format!("{}: unknown damage kind {kind:?}", a.name));
                    }
                    if *sides == 0 {
                        return Err(format!("{}: a die needs sides", a.name));
                    }
                }
                Ok(())
            })
            .unwrap_or_else(|e| panic!("assets/affixes.ron: {e}"));
        let affixes = Registry::from_defs(
            authored
                .iter()
                .map(|(_, a)| AffixDef {
                    name: a.name.clone(),
                    kind: a.kind,
                    applies_to: a.applies_to.iter().map(|t| tags.expect(t)).collect(),
                    grants: a.grants.iter().map(|(stat, base, per)| Scaled { stat: stats.expect(stat), base: *base, levels_per_point: *per }).collect(),
                    strikes: a
                        .strikes
                        .iter()
                        .map(|(kind, base_dice, per, sides)| ScaledStrike {
                            kind: kinds.expect(kind),
                            base_dice: *base_dice,
                            levels_per_die: *per,
                            sides: *sides,
                        })
                        .collect(),
                    weight: a.weight,
                })
                .collect(),
        )
        .unwrap();
        let mut shapes = Vec::new();
        let mut item_tags = Vec::new();
        let mut table = BandedTable::default();
        for (id, d) in defs.iter() {
            item_tags.push(d.tags.iter().map(|t| tags.expect(t)).collect::<Vec<_>>());
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
            attack_stat: stats.expect("attack"),
            defs,
            slots,
            stats,
            tags,
            affixes,
            shapes,
            item_tags,
            table,
            seed,
            home,
            spawned: BTreeSet::new(),
        }
    }

    /// The tags of `id`.
    pub fn tags_of(&self, id: Id<ItemDef>) -> &[TagId] {
        &self.item_tags[id.index()]
    }

    /// What a level buys on `id`: damage on a weapon, armor on armor.
    pub fn rule(&self, id: Id<ItemDef>) -> EnhanceRule {
        let tags = self.tags_of(id);
        let mut rule = EnhanceRule::default();
        if tags.contains(&self.tags.expect("weapon")) {
            rule.damage_per_level = 1;
        }
        if tags.contains(&self.tags.expect("armor")) {
            rule.per_level.push((self.armor_stat, 1));
        }
        rule
    }

    /// Rolls what a found `id` is: plain for anything not worn, else by `quality`.
    pub fn roll_quality(&self, id: Id<ItemDef>, quality: Quality, rng: &mut impl Rng) -> Enchanted {
        if self.shape(id).is_none() {
            return Enchanted::plain();
        }
        let level = rng.random_range(quality.levels.0..=quality.levels.1);
        let affixes = if rng.random_range(0..100) < quality.affix_pct { roll_affixes(rng, &self.affixes, self.tags_of(id), 1) } else { Vec::new() };
        Enchanted { level, affixes }
    }

    /// "Sharp cutlass of flame +2", or just "cutlass".
    pub fn display_name(&self, id: Id<ItemDef>, enchant: Option<&Enchant>) -> String {
        let base = &self.defs.get(id).name;
        match enchant {
            Some(e) => e.0.display_name(base, &self.affixes),
            None => base.clone(),
        }
    }

    /// Regions already scattered over.
    pub fn spawned(&self) -> impl Iterator<Item = &Point> {
        self.spawned.iter()
    }

    /// Marks regions as scattered over, when continuing a run.
    pub fn restore_spawned(&mut self, regions: impl IntoIterator<Item = Point>) {
        self.spawned = regions.into_iter().collect();
    }

    /// The shape `id` is worn in, if it is worn at all.
    pub fn shape(&self, id: Id<ItemDef>) -> Option<&EquipShape> {
        self.shapes[id.index()].as_ref()
    }

    /// Spawns one `id`, or a stack of `count`, on the ground at `at` or
    /// nowhere when `at` is `None`.
    pub fn spawn(&self, commands: &mut Commands, id: Id<ItemDef>, count: u32, at: Option<Point>) -> Entity {
        self.spawn_with(commands, id, count, at, Enchanted::plain())
    }

    /// Spawns one `id` with a rolled enchantment.
    pub fn spawn_with(&self, commands: &mut Commands, id: Id<ItemDef>, count: u32, at: Option<Point>, enchant: Enchanted) -> Entity {
        let d = self.defs.get(id);
        let mut e = commands.spawn((Item, ItemKind(id), Glyph::new(d.glyph, Color::srgb(d.color.0, d.color.1, d.color.2)).on_layer(2)));
        if let Some(shape) = self.shape(id) {
            e.insert((Wearable(shape.clone()), Enchant(enchant)));
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
                    let enchant = armory.roll_quality(id, Quality::FOUND, &mut rng);
                    armory.spawn_with(&mut commands, id, n, Some(p), enchant);
                    break;
                }
            }
        }
    }
}

/// What the dead leave behind, from the bestiary's drop lists.
pub fn drop_loot(
    mut commands: Commands,
    mut deaths: MessageReader<DeathEvent>,
    armory: Res<Armory>,
    bestiary: Res<crate::monsters::Bestiary>,
    mut rng: ResMut<CombatRng>,
    kinds: Query<&crate::monsters::MonsterKind>,
) {
    for d in deaths.read() {
        let Ok(kind) = kinds.get(d.entity) else { continue };
        for (name, pct) in &bestiary.defs.get(kind.0).drops {
            if rng.random_range(0..100) < *pct {
                let id = armory.defs.expect(name);
                let n = armory.stack_size(id, &mut **rng);
                let enchant = armory.roll_quality(id, Quality::FOUND, &mut **rng);
                armory.spawn_with(&mut commands, id, n, Some(d.at), enchant);
            }
        }
    }
}

/// Applies what using an item does, and consumes it.
pub fn use_items(
    mut commands: Commands,
    mut events: MessageReader<ItemEvent>,
    armory: Res<Armory>,
    turns: Res<Turns>,
    mut log: ResMut<MessageLog>,
    mut users: Query<&mut Health>,
    mut items: Query<(&ItemKind, Option<&mut Stack>)>,
) {
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

/// A wearer as the gear refresh sees it.
type WearerData = (Entity, &'static Equipped, &'static mut Sheet, &'static mut Armor, &'static mut MeleeAttack, &'static mut Strikes);

/// Rebuilds a wearer's stats, armor, attack, extra strikes and shot from
/// what it wears: the items' own numbers, their affixes and their levels.
pub fn refresh_gear(
    mut commands: Commands,
    armory: Res<Armory>,
    bestiary: Res<crate::monsters::Bestiary>,
    mut wearers: Query<WearerData, Changed<Equipped>>,
    items: Query<(&ItemKind, Option<&Enchant>)>,
) {
    let main_hand = armory.slots.expect("main hand");
    for (wearer, worn, mut sheet, mut armor, mut attack, mut strikes) in &mut wearers {
        let mut stats = Stats::new();
        let mut shot = None;
        for (_, item) in worn.worn() {
            let Ok((kind, enchant)) = items.get(item) else { continue };
            let d = armory.defs.get(kind.0);
            if d.armor != 0 {
                stats.add(Modifier::new(armory.armor_stat, Op::Add(d.armor), item.to_bits()));
            }
            if let Some(e) = enchant {
                for m in e.0.modifiers(&armory.affixes, &armory.rule(kind.0), item.to_bits()) {
                    stats.add(m);
                }
            }
            if let Some((range, dice, kind_name)) = &d.ranged {
                let dice = enchant.map(|e| e.0.strike(*dice, &armory.rule(kind.0))).unwrap_or(*dice);
                shot = Some(RangedAttack { kind: bestiary.kinds.expect(kind_name), dice, range: *range });
            }
        }
        armor.0 = stats.value(armory.armor_stat, &armory.stats);
        let bonus = stats.value(armory.attack_stat, &armory.stats);
        let wielded = worn.in_slot(main_hand).and_then(|e| items.get(e).ok());
        (*attack, strikes.0) = match wielded {
            Some((kind, enchant)) if armory.defs.get(kind.0).attack.is_some() => {
                let d = armory.defs.get(kind.0);
                let base = d.attack.expect("checked");
                let dice = enchant.map(|e| e.0.strike(base, &armory.rule(kind.0))).unwrap_or(base);
                let extra = enchant.map(|e| e.0.strikes(&armory.affixes)).unwrap_or_default();
                (MeleeAttack { kind: bestiary.kinds.expect(d.kind.as_deref().expect("checked")), dice: DiceRoll { bonus: dice.bonus + bonus, ..dice } }, extra)
            }
            _ => {
                let bare = unarmed(&bestiary);
                (MeleeAttack { dice: DiceRoll { bonus: bare.dice.bonus + bonus, ..bare.dice }, ..bare }, Vec::new())
            }
        };
        match shot {
            Some(s) => {
                commands.entity(wearer).insert(s);
            }
            None => {
                commands.entity(wearer).remove::<RangedAttack>();
            }
        }
        sheet.0 = stats;
    }
}

/// A bare-knuckle strike.
pub fn unarmed(bestiary: &crate::monsters::Bestiary) -> MeleeAttack {
    MeleeAttack { kind: bestiary.kinds.expect("fist"), dice: DiceRoll::new(1, 3) }
}

/// Turns item events into log lines.
pub fn narrate_items(
    mut events: MessageReader<ItemEvent>,
    armory: Res<Armory>,
    turns: Res<Turns>,
    mut log: ResMut<MessageLog>,
    items: Query<(&ItemKind, Option<&Stack>, Option<&Enchant>)>,
    players: Query<(), With<Player>>,
) {
    let turn = turns.turn_number();
    let describe = |item: Entity| -> String {
        match items.get(item) {
            Ok((k, Some(s), _)) if s.count > 1 => format!("{} {}", s.count, armory.defs.get(k.0).name),
            Ok((k, _, e)) => format!("the {}", armory.display_name(k.0, e)),
            Err(_) => "something".into(),
        }
    };
    for ev in events.read() {
        let (actor, text, cat) = match *ev {
            ItemEvent::PickedUp { actor, item, merged_into } => (actor, format!("You pick up {}.", describe(merged_into.unwrap_or(item))), LogCategory::Info),
            ItemEvent::Dropped { actor, item, .. } => (actor, format!("You drop {}.", describe(item)), LogCategory::Info),
            ItemEvent::Equipped { actor, item } => {
                let verb = if items.get(item).is_ok_and(|(k, _, _)| armory.defs.get(k.0).attack.is_some() || armory.defs.get(k.0).ranged.is_some()) {
                    "wield"
                } else {
                    "put on"
                };
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

/// An item on the ground as the status line sees it.
pub type GroundData = (&'static Position, &'static ItemKind, Option<&'static Stack>, Option<&'static Enchant>);

/// Names what lies at `p`, for the status line.
pub fn whats_here(p: Point, armory: &Armory, ground: &Query<GroundData, With<Item>>) -> Option<String> {
    let names: Vec<String> = ground
        .iter()
        .filter(|(pos, _, _, _)| pos.0 == p)
        .map(|(_, k, s, e)| match s {
            Some(s) if s.count > 1 => format!("{} {}", s.count, armory.defs.get(k.0).name),
            _ => armory.display_name(k.0, e),
        })
        .collect();
    (!names.is_empty()).then(|| names.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn the_armory_loads_its_affixes_and_rolls_the_hoard_rich() {
        let (bestiary, _) = crate::monsters::Bestiary::load(RunSeed(1), Point::ZERO);
        let armory = Armory::load(RunSeed(1), Point::ZERO, &bestiary.kinds);
        assert_eq!(armory.affixes.len(), 4);
        let cutlass = armory.defs.expect("cutlass");
        let pistol = armory.defs.expect("pistol");
        assert!(armory.tags_of(cutlass).contains(&armory.tags.expect("blade")));
        assert_eq!(armory.rule(cutlass).damage_per_level, 1);
        assert_eq!(armory.rule(armory.defs.expect("tricorne")).per_level, vec![(armory.armor_stat, 1)]);
        assert!(armory.defs.get(pistol).ranged.is_some());
        let mut rng = rand::rngs::StdRng::seed_from_u64(3);
        for _ in 0..20 {
            let e = armory.roll_quality(cutlass, Quality::HOARD, &mut rng);
            assert!((1..=3).contains(&e.level) && e.affixes.len() == 1, "{e:?}");
            let name = e.display_name("cutlass", &armory.affixes);
            assert!(name.contains("cutlass") && name.contains('+'), "{name}");
        }
        assert_eq!(armory.roll_quality(armory.defs.expect("rum"), Quality::HOARD, &mut rng), Enchanted::plain(), "drink is never enchanted");
    }
}
