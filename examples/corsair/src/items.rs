//! Items: definitions from RON, what lies about, what the dead drop, and
//! what wearing a jerkin is worth.
//!
//! The engine moves items between ground, bag and slots, charges the turns,
//! and reads what an item does off the item itself: a spawned cutlass
//! carries the `MeleeAttack` it is swung with, a jerkin its `Armor`, an
//! affix what it `Bestows` on a stat, each with the rolled enchant already
//! in it, and a bottle of rum the `swig` it `Grants`, so what a drink does
//! is a line of `abilities.ron` and nothing here. Everything that gives an
//! item meaning is here or in the files: the slot names, the numbers, the
//! words in the log.

use std::collections::BTreeSet;

use bevy::prelude::*;
use rand::Rng;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::{DiceRoll, Id, Point, RunSeed, SeedDomain, geometry};
use rl_engine::rl_render::Glyph;
use rl_engine::rl_rules::ability::AbilityDef;
use rl_engine::rl_rules::damage::DamageKind;
use rl_engine::rl_rules::damage::DamageKindId;
use rl_engine::rl_rules::{AffixDef, Enchanted, EnhanceRule, EquipShape, NameRef, SlotDef, SlotId, StatId, TagDef, TagId, affix, roll_affixes};
use rl_engine::rl_rules::{BandedEntry, BandedTable, Named, Registry};
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
    pub thrown: Option<(i32, DiceRoll, NameRef<DamageKind>)>,
    #[serde(default)]
    pub grants: Vec<NameRef<AbilityDef>>,
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
    /// Loads the items and their affixes against `registries` and the
    /// abilities a bottle lends; panics with every problem listed.
    pub fn load(seed: RunSeed, home: Point, registries: &Registries, abilities: &Abilities) -> Self {
        let defs: Registry<ItemDef> = registries.names().with("ability", abilities.defs()).load(ITEMS_RON).unwrap_or_else(|e| panic!("assets/items.ron: {e}"));
        defs.validate(|d, _| {
            if d.ranged.as_ref().is_some_and(|(range, _, _)| *range < 2) {
                return Err("a ranged weapon reaches at least 2".into());
            }
            if d.thrown.as_ref().is_some_and(|(range, _, _)| *range < 2) {
                return Err("a thrown item reaches at least 2".into());
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

    /// What wearing `id` enchanted as `enchant` is worth, for a monster
    /// choosing between two things to wear: its armor twice over, its
    /// average blow, half its average shot, and a point for each level and
    /// affix on top.
    pub fn gear_score(&self, id: Id<ItemDef>, enchant: &Enchanted) -> i32 {
        let d = self.defs.get(id);
        let blow = d.attack.map_or(0.0, |a| a.avg()) + d.ranged.as_ref().map_or(0.0, |(_, roll, _)| roll.avg() / 2.0);
        d.armor * 2 + blow.round() as i32 + enchant.level + enchant.affixes.len() as i32
    }

    /// Spawns one `id`, or a stack of `count`, on the ground at `at` or
    /// nowhere when `at` is `None`.
    pub fn spawn(&self, commands: &mut Commands, id: Id<ItemDef>, count: u32, at: Option<Point>) -> Entity {
        self.spawn_with(commands, id, count, at, Enchanted::plain())
    }

    /// Spawns one `id` with a rolled enchantment.
    ///
    /// What the item does when worn goes on the item, enchant folded in:
    /// the engine's `Loadout` reads a worn blade's blow and a worn coat's
    /// armor straight off them, and folds what the affixes bestow into the
    /// wearer's stats. Nothing here is copied onto a wearer.
    pub fn spawn_with(&self, commands: &mut Commands, id: Id<ItemDef>, count: u32, at: Option<Point>, enchant: Enchanted) -> Entity {
        let d = self.defs.get(id);
        // The label carries the rolled name, so a panel shows "fine
        // cutlass" without ever seeing the armory.
        let name = self.display_name(id, Some(&Enchant(enchant.clone())));
        let mut e = commands.spawn((Item, ItemKind(id), Name::new(name), Glyph::new(d.glyph, Color::srgb(d.color.0, d.color.1, d.color.2)).on_layer(2)));
        // Tagged, so an ability that spends powder can find it in a bag.
        let tags = self.tags_of(id);
        if !tags.is_empty() {
            e.insert(Tagged(tags.to_vec()));
        }
        // What using it does: the ability it lends, spent from the stack.
        if !d.grants.is_empty() {
            e.insert(Grants(d.grants.iter().map(|g| g.id()).collect()));
        }
        if let Some(shape) = self.shape(id) {
            let rule = self.rule(id);
            if d.armor != 0 {
                e.insert(Armor(d.armor));
            }
            if let (Some(dice), Some(kind)) = (d.attack, &d.kind) {
                e.insert(MeleeAttack { kind: kind.id(), dice: enchant.strike(dice, &rule), cost: None });
            }
            if let Some((range, dice, kind)) = &d.ranged {
                e.insert(RangedAttack { kind: kind.id(), dice: enchant.strike(*dice, &rule), range: *range, cost: None });
            }
            let extra = enchant.strikes(&self.affixes);
            if !extra.is_empty() {
                e.insert(Strikes(extra));
            }
            let bestows = enchant.grants(&self.affixes, &rule);
            if !bestows.is_empty() {
                e.insert(Bestows(bestows));
            }
            e.insert((Wearable(shape.clone()), GearScore(self.gear_score(id, &enchant)), Enchant(enchant)));
        }
        if let Some((range, dice, kind)) = &d.thrown {
            e.insert(Throwable { range: *range, strike: Some((kind.id(), *dice)) });
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

/// A bare-knuckle strike: what the player fights with when nothing worn
/// carries a blow of its own.
pub fn unarmed(armory: &Armory) -> MeleeAttack {
    MeleeAttack { kind: armory.fist, dice: DiceRoll::new(1, 3), cost: None }
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
        assert_eq!(armory.roll_quality(armory.defs.expect("bottle of rum"), Quality::HOARD, &mut rng), Enchanted::plain(), "drink is never enchanted");
    }

    /// A knife can be thrown and gear is scored, better gear higher, so a
    /// monster can tell which of two things to wear without an armory.
    #[test]
    fn a_knife_is_made_to_be_thrown_and_gear_is_scored_better_for_better() {
        let loaded = crate::rules::load(RunSeed(1), Point::ZERO, &crate::rules::effect_kinds());
        let armory = &loaded.armory;
        let score = |name: &str, level: i32| armory.gear_score(armory.defs.expect(name), &Enchanted { level, affixes: Vec::new() });
        assert!(score("buckler", 0) > 0, "a buckler is worth something");
        assert!(score("buckler", 2) > score("buckler", 0), "and more enchanted");
        assert!(score("boarding axe", 0) > score("cutlass", 0), "an axe hits harder than a cutlass");
        let (range, _, _) = armory.defs.get(armory.defs.expect("throwing knife")).thrown.expect("a knife is thrown");
        assert!(range >= 2);
    }

    /// What `who` fights with, as the engine sums it at the moment of a blow.
    fn loadout_of(app: &mut App, who: Entity) -> (i32, Option<DiceRoll>, Vec<(DamageKindId, DiceRoll)>) {
        let mut state: bevy::ecs::system::SystemState<Loadout> = bevy::ecs::system::SystemState::new(app.world_mut());
        let loadout = state.get(app.world()).expect("the loadout's inputs are all optional");
        (loadout.armor(who), loadout.melee(who).map(|m| m.dice), loadout.strikes(who))
    }

    /// A marine that puts on a buckler keeps its own pistol-whip and hide,
    /// and wears the buckler's armor on top of them, rather than dropping to
    /// a bare fist the moment it wears anything. Nothing on the marine
    /// changes when it dresses: the engine reads the buckler.
    #[test]
    fn a_monster_wears_gear_on_top_of_what_its_kind_fights_with() {
        let dir = std::env::temp_dir().join(format!("corsair-marine-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = crate::testing::headless(RunSeed(7), false, &dir);
        app.update();
        app.update();
        let me = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        let at = app.world().get::<Position>(me).unwrap().0.offset(0, 6);
        let kind = app.world().resource::<crate::monsters::Bestiary>().defs.expect("marine");
        let (marine, buckler) = app.world_mut().resource_scope(|world: &mut World, bestiary: Mut<crate::monsters::Bestiary>| {
            world.resource_scope(|world: &mut World, armory: Mut<Armory>| {
                let mut queue = bevy::ecs::world::CommandQueue::default();
                let mut commands = Commands::new(&mut queue, world);
                let marine = bestiary.spawn(&mut commands, kind, at);
                let buckler = armory.spawn(&mut commands, armory.defs.expect("buckler"), 1, None);
                queue.apply(world);
                (marine, buckler)
            })
        });
        app.update();
        let def = app.world().resource::<crate::monsters::Bestiary>().defs.get(kind).clone();
        assert_eq!(loadout_of(&mut app, marine).0, def.armor, "its own hide before it wears anything");

        let shape = app.world().resource::<Armory>().shape(app.world().resource::<Armory>().defs.expect("buckler")).unwrap().clone();
        app.world_mut().get_mut::<Inventory>(marine).unwrap().items.push(buckler);
        app.world_mut().get_mut::<Equipped>(marine).unwrap().equip(buckler, &shape).unwrap();
        app.world_mut().write_message(Intent::new(me, Wait));
        app.update();
        let (armor, blow, _) = loadout_of(&mut app, marine);
        assert_eq!(armor, def.armor + 1, "the buckler's armor on top");
        assert_eq!(blow, Some(def.attack), "and its own blow, not a fist");
        assert_eq!(app.world().get::<Armor>(marine).map(|a| a.0), Some(def.armor), "with nothing copied onto the marine");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A Sharp cutlass of flame is a blade whose blow the engine swings, a
    /// fire strike it adds, and an attack bonus it folds into the wielder's
    /// stats and back into the blow. Taken off, all three go with it.
    #[test]
    fn an_enchanted_blade_is_swung_and_its_affixes_folded_only_while_it_is_worn() {
        let dir = std::env::temp_dir().join(format!("corsair-blade-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = crate::testing::headless(RunSeed(7), false, &dir);
        app.update();
        app.update();
        let me = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        let (fist, main_hand) = {
            let armory = app.world().resource::<Armory>();
            (unarmed(armory).dice, armory.main_hand)
        };
        let (blade, base) = app.world_mut().resource_scope(|world: &mut World, armory: Mut<Armory>| {
            let cutlass = armory.defs.expect("cutlass");
            let enchant = Enchanted { level: 2, affixes: vec![armory.affixes.expect("Sharp"), armory.affixes.expect("flame")] };
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let mut commands = Commands::new(&mut queue, world);
            let blade = armory.spawn_with(&mut commands, cutlass, 1, None, enchant);
            queue.apply(world);
            (blade, armory.defs.get(cutlass).attack.expect("a cutlass strikes"))
        });
        // Put the starting cutlass away and wield the enchanted one.
        let worn_before: Vec<Entity> = app.world().get::<Equipped>(me).unwrap().worn().map(|(_, e)| e).collect();
        for item in worn_before {
            app.world_mut().get_mut::<Equipped>(me).unwrap().unequip(item);
        }
        app.world_mut().get_mut::<Inventory>(me).unwrap().items.push(blade);
        app.world_mut().get_mut::<Equipped>(me).unwrap().equip(blade, &EquipShape::in_slot(main_hand)).unwrap();
        app.world_mut().write_message(Intent::new(me, Wait));
        app.update();

        let (_, blow, strikes) = loadout_of(&mut app, me);
        // 1d6, +2 for two levels on a weapon, and Sharp's attack bonus: +1
        // at level zero and one more every two levels.
        assert_eq!(blow, Some(DiceRoll { bonus: base.bonus + 2 + 2, ..base }), "the blade's roll, its level, and the bonus Sharp bestows");
        assert_eq!(strikes.len(), 1, "flame's strike: {strikes:?}");
        let stats = app.world().resource::<Registries>().stats.clone();
        let attack_stat = stats.expect("attack");
        assert_eq!(app.world().get::<StatBlock>(me).unwrap().0.value(attack_stat, &stats), 2, "Sharp is folded into the stat");

        app.world_mut().get_mut::<Equipped>(me).unwrap().unequip(blade);
        app.world_mut().write_message(Intent::new(me, Wait));
        app.update();
        let (_, blow, strikes) = loadout_of(&mut app, me);
        assert_eq!(blow, Some(fist), "bare fists again");
        assert!(strikes.is_empty());
        assert_eq!(app.world().get::<StatBlock>(me).unwrap().0.value(attack_stat, &stats), 0, "and the bonus left with the blade");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Drinking the bottle is using the swig it grants: the engine resolves
    /// it inside the pass, so the healing lands before the next monster due
    /// this frame strikes, and the bottle is one lighter. At one hit point
    /// that is the difference between a close call and a death.
    #[test]
    fn a_drink_at_deaths_door_lands_before_the_next_blow_and_costs_the_bottle() {
        let dir = std::env::temp_dir().join(format!("corsair-drink-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = crate::testing::headless(RunSeed(7), false, &dir);
        app.update();
        app.update();

        let me = {
            let w = app.world_mut();
            let mut q = w.query_filtered::<Entity, With<Player>>();
            q.single(w).unwrap()
        };
        let rum_kind = app.world().resource::<Armory>().defs.expect("bottle of rum");
        let rum = {
            let w = app.world();
            let bag = w.get::<Inventory>(me).expect("a bag");
            bag.items.iter().copied().find(|i| w.get::<ItemKind>(*i).is_some_and(|k| k.0 == rum_kind)).expect("a bottle of rum")
        };
        app.world_mut().get_mut::<Health>(me).expect("health").current = 1;
        let bottles = app.world().get::<Stack>(rum).map(|s| s.count).expect("the bottles stack");

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
        // What landed on the player this frame, in order: the mend first,
        // then whatever the cutthroat's turn did.
        let dealt: Vec<i32> = app.world_mut().resource_mut::<Messages<DamageDealt>>().drain().filter(|d| d.target == me).map(|d| d.dealt).collect();

        let died = app.world_mut().resource_mut::<Messages<DeathEvent>>().drain().any(|d| d.was_player);
        assert!(!died, "the drink landed after the blow: {dealt:?}");
        assert!(dealt.first().is_some_and(|d| *d < 0), "the mend landed first: {dealt:?}");
        assert!(app.world().get::<Health>(me).is_some_and(|h| h.current >= 1), "and the player stands");
        assert_eq!(app.world().get::<Stack>(rum).map(|s| s.count), Some(bottles - 1), "and cost a bottle");
        let hearty = app.world().resource::<Registries>().statuses.expect("hearty");
        assert!(app.world().get::<Afflicted>(me).is_some_and(|a| a.has(hearty)), "and put heart in you");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
