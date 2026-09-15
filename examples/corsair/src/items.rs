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
use rl_engine::rl_core::{DiceRoll, Id, Point, RunSeed, SeedDomain, geometry};
use rl_engine::rl_render::Glyph;
use rl_engine::rl_rules::damage::DamageKind;
use rl_engine::rl_rules::damage::DamageKindId;
use rl_engine::rl_rules::{AffixDef, Enchanted, EnhanceRule, EquipShape, Modifier, NameRef, Op, SlotDef, SlotId, StatId, TagDef, TagId, affix, roll_affixes};
use rl_engine::rl_rules::{BandedEntry, BandedTable, Named, Registry};
use rl_engine::rl_ui::{MessageLog, Tones};
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
    pub slot: Option<NameRef<SlotDef>>,
    #[serde(default)]
    pub also: Vec<NameRef<SlotDef>>,
    #[serde(default)]
    pub tags: Vec<NameRef<TagDef>>,
    #[serde(default)]
    pub armor: i32,
    #[serde(default)]
    pub attack: Option<DiceRoll>,
    #[serde(default)]
    pub kind: Option<NameRef<DamageKind>>,
    #[serde(default)]
    pub ranged: Option<(i32, DiceRoll, NameRef<DamageKind>)>,
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

/// The item definitions and the vocabulary they use.
#[derive(Resource)]
pub struct Armory {
    pub defs: Registry<ItemDef>,
    pub affixes: Registry<AffixDef>,
    pub armor_stat: StatId,
    pub attack_stat: StatId,
    pub weapon_tag: TagId,
    pub armor_tag: TagId,
    pub main_hand: SlotId,
    /// What a bare fist deals.
    pub fist: DamageKindId,
    shapes: Vec<Option<EquipShape>>,
    item_tags: Vec<Vec<TagId>>,
    table: BandedTable<Id<ItemDef>>,
    seed: RunSeed,
    home: Point,
    spawned: BTreeSet<Point>,
}

impl Armory {
    /// Loads the items and their affixes against `registries`; panics with
    /// every problem listed.
    pub fn load(seed: RunSeed, home: Point, registries: &Registries) -> Self {
        let defs: Registry<ItemDef> = registries.names().load(ITEMS_RON).unwrap_or_else(|e| panic!("assets/items.ron: {e}"));
        defs.validate(|d, _| {
            if d.ranged.as_ref().is_some_and(|(range, _, _)| *range < 2) {
                return Err("a ranged weapon reaches at least 2".into());
            }
            if d.slot.is_none() && !d.also.is_empty() {
                return Err("also without a slot".into());
            }
            if d.attack.is_some() != d.kind.is_some() {
                return Err("attack and kind go together".into());
            }
            if d.stack && d.slot.is_some() {
                return Err("a worn item cannot stack".into());
            }
            Ok(())
        })
        .unwrap_or_else(|e| panic!("assets/items.ron: {e}"));
        let affixes = affix::load(AFFIXES_RON, &registries.names()).unwrap_or_else(|e| panic!("assets/affixes.ron: {e}"));
        let mut shapes = Vec::new();
        let mut item_tags = Vec::new();
        let mut table = BandedTable::default();
        for (id, d) in defs.iter() {
            item_tags.push(d.tags.iter().map(|t| t.id()).collect::<Vec<_>>());
            shapes.push(d.slot.map(|s| {
                let mut shape = EquipShape::in_slot(s.id());
                for a in &d.also {
                    shape = shape.and_claims(a.id());
                }
                shape
            }));
            if let Some((lo, hi, w)) = d.spawn {
                table.push(BandedEntry::new(id).bands(lo, hi).weight(w));
            }
        }
        Self {
            armor_stat: registries.stats.expect("armor"),
            attack_stat: registries.stats.expect("attack"),
            weapon_tag: registries.tags.expect("weapon"),
            armor_tag: registries.tags.expect("armor"),
            main_hand: registries.slots.expect("main hand"),
            fist: registries.damage_kinds.expect("fist"),
            defs,
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
        if tags.contains(&self.weapon_tag) {
            rule.damage_per_level = 1;
        }
        if tags.contains(&self.armor_tag) {
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
        // The label carries the rolled name, so a panel shows "fine
        // cutlass" without ever seeing the armory.
        let name = self.display_name(id, Some(&Enchant(enchant.clone())));
        let mut e = commands.spawn((Item, ItemKind(id), Name::new(name), Glyph::new(d.glyph, Color::srgb(d.color.0, d.color.1, d.color.2)).on_layer(2)));
        // Tagged, so an ability that spends powder or rum can find it in a bag.
        let tags = self.tags_of(id);
        if !tags.is_empty() {
            e.insert(Tagged(tags.to_vec()));
        }
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
        for (item, pct) in &bestiary.defs.get(kind.0).drops {
            if rng.random_range(0..100) < *pct {
                let id = item.id();
                let n = armory.stack_size(id, &mut **rng);
                let enchant = armory.roll_quality(id, Quality::FOUND, &mut **rng);
                armory.spawn_with(&mut commands, id, n, Some(d.at), enchant);
            }
        }
    }
}

/// Applies what using an item does, and consumes it.
/// What using an item reaches.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Using<'w, 's> {
    armory: Res<'w, Armory>,
    registries: Res<'w, Registries>,
    turns: Res<'w, Turns>,
    log: ResMut<'w, MessageLog>,
    afflict: MessageWriter<'w, Afflict>,
    cure: MessageWriter<'w, Cure>,
    users: Query<'w, 's, &'static mut Health>,
    items: Query<'w, 's, (&'static ItemKind, Option<&'static mut Stack>)>,
}

/// Applies what using an item does, and consumes it. Drink heals, cures
/// what a bite left and makes you hearty for a while.
pub fn use_items(mut commands: Commands, mut events: MessageReader<ItemEvent>, mut using: Using) {
    let Using { armory, registries, turns, log, afflict, cure, users, items } = &mut using;
    for ev in events.read() {
        let ItemEvent::Used { actor, item } = *ev else { continue };
        let Ok((kind, stack)) = items.get_mut(item) else { continue };
        let d = armory.defs.get(kind.0);
        if !d.usable() {
            log.push(format!("You cannot think what to do with the {}.", d.name), Tones::MUTED, turns.turn_number());
            continue;
        }
        if let Ok(mut hp) = users.get_mut(actor) {
            let before = hp.hp;
            hp.hp = (hp.hp + d.heal).min(hp.max);
            log.push(format!("You drink the {}. It restores {} health.", d.name, hp.hp - before), Tones::GOOD, turns.turn_number());
            for name in ["venom", "bleeding"] {
                cure.write(Cure { target: actor, status: registries.statuses.expect(name) });
            }
            afflict.write(Afflict { target: actor, status: registries.statuses.expect("hearty"), turns: 10, by: None });
        }
        match stack {
            Some(mut s) if s.count > 1 => s.count -= 1,
            _ => commands.entity(item).despawn(),
        }
    }
}

/// A wearer as the gear refresh sees it.
type WearerData = (Entity, &'static Equipped, &'static mut StatBlock, &'static mut Armor, &'static mut MeleeAttack, &'static mut Strikes);

/// Rebuilds a wearer's stats, armor, attack, extra strikes and shot from
/// what it wears: the items' own numbers, their affixes and their levels.
pub fn refresh_gear(
    mut commands: Commands,
    armory: Res<Armory>,
    registries: Res<Registries>,
    mut wearers: Query<WearerData, Changed<Equipped>>,
    items: Query<(&ItemKind, Option<&Enchant>)>,
) {
    let main_hand = armory.main_hand;
    for (wearer, worn, mut sheet, mut armor, mut attack, mut strikes) in &mut wearers {
        // Gear is rebuilt from scratch; what statuses put there stays.
        let mut stats = std::mem::take(&mut sheet.0);
        stats.retain_sources(rl_engine::rl_rules::is_status_source);
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
            if let Some((range, dice, shot_kind)) = &d.ranged {
                let dice = enchant.map(|e| e.0.strike(*dice, &armory.rule(kind.0))).unwrap_or(*dice);
                shot = Some(RangedAttack { kind: shot_kind.id(), dice, range: *range });
            }
        }
        armor.0 = stats.value(armory.armor_stat, &registries.stats);
        let bonus = stats.value(armory.attack_stat, &registries.stats);
        let wielded = worn.in_slot(main_hand).and_then(|e| items.get(e).ok());
        (*attack, strikes.0) = match wielded {
            Some((kind, enchant)) if armory.defs.get(kind.0).attack.is_some() => {
                let d = armory.defs.get(kind.0);
                let base = d.attack.expect("checked");
                let dice = enchant.map(|e| e.0.strike(base, &armory.rule(kind.0))).unwrap_or(base);
                let extra = enchant.map(|e| e.0.strikes(&armory.affixes)).unwrap_or_default();
                (MeleeAttack { kind: d.kind.expect("checked").id(), dice: DiceRoll { bonus: dice.bonus + bonus, ..dice } }, extra)
            }
            _ => {
                let bare = unarmed(&armory);
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
pub fn unarmed(armory: &Armory) -> MeleeAttack {
    MeleeAttack { kind: armory.fist, dice: DiceRoll::new(1, 3) }
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
            ItemEvent::PickedUp { actor, item, merged_into } => (actor, format!("You pick up {}.", describe(merged_into.unwrap_or(item))), Tones::TEXT),
            ItemEvent::Dropped { actor, item, .. } => (actor, format!("You drop {}.", describe(item)), Tones::TEXT),
            ItemEvent::Equipped { actor, item } => {
                let verb = if items.get(item).is_ok_and(|(k, _, _)| armory.defs.get(k.0).attack.is_some() || armory.defs.get(k.0).ranged.is_some()) {
                    "wield"
                } else {
                    "put on"
                };
                (actor, format!("You {verb} {}.", describe(item)), Tones::TEXT)
            }
            ItemEvent::Unequipped { actor, item } => (actor, format!("You take off {}.", describe(item)), Tones::MUTED),
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
        let loaded = crate::rules::load(RunSeed(1), Point::ZERO, &crate::rules::effect_kinds());
        let armory = &loaded.armory;
        assert_eq!(armory.affixes.len(), 4);
        let cutlass = armory.defs.expect("cutlass");
        let pistol = armory.defs.expect("pistol");
        assert!(armory.tags_of(cutlass).contains(&loaded.registries.tags.expect("blade")));
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

    /// A drink is a reaction to an item event, and reactions run after the
    /// whole turn loop, so the healing lands after every monster due this
    /// frame has already struck. At one hit point that is the difference
    /// between a close call and a death.
    #[test]
    fn a_drink_at_deaths_door_lands_before_the_next_blow() {
        let dir = std::env::temp_dir().join(format!("corsair-drink-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = crate::testing::headless(RunSeed(7), false, &dir);
        app.add_systems(Turn, use_items.in_set(TurnSet::React));
        app.update();
        app.update();

        let me = {
            let w = app.world_mut();
            let mut q = w.query_filtered::<Entity, With<Player>>();
            q.single(w).unwrap()
        };
        let rum_kind = app.world().resource::<Armory>().defs.expect("rum");
        let rum = {
            let w = app.world();
            let bag = w.get::<Inventory>(me).expect("a bag");
            bag.items.iter().copied().find(|i| w.get::<ItemKind>(*i).is_some_and(|k| k.0 == rum_kind)).expect("a bottle of rum")
        };
        app.world_mut().get_mut::<Health>(me).expect("health").hp = 1;

        // A cutthroat at the player's elbow, due the moment the player's
        // turn is spent.
        let at = app.world().get::<Position>(me).expect("a position").0.offset(1, 0);
        let kind = app.world().resource::<crate::monsters::Bestiary>().defs.expect("cutthroat");
        app.world_mut().resource_scope(|world: &mut World, bestiary: Mut<crate::monsters::Bestiary>| {
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let mut commands = Commands::new(&mut queue, world);
            bestiary.spawn(&mut commands, kind, at);
            queue.apply(world);
        });
        app.update();

        app.world_mut().write_message(Intent::new(me, UseItem(rum)));
        app.update();

        let died = app.world_mut().resource_mut::<Messages<DeathEvent>>().drain().any(|d| d.was_player);
        let hp = app.world().get::<Health>(me).map(|h| h.hp);
        assert!(!died, "the drink landed after the blow: {hp:?}");
        assert!(hp.is_some_and(|hp| hp > 1), "the drink healed: {hp:?}");
    }
}
