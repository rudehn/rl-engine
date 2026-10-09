//! What Mereholt's keepers may stock, and how one stocking of their shelves
//! is drawn.
//!
//! [`Wares`] is `assets/wares.ron`, read against the [`Armory`]: per
//! keeper, what it buys, what is on its shelf always, and the parts drawn
//! for the rest, each drawn as the engine's [`LootTable`] draws at a band,
//! the band being the [`tier`](tier_of) the adventurer has reached; and one
//! shared [`LevelTable`] for how much better than plain a ware that takes
//! levels is. What a ware costs is its `price` in `items.ron`, so a shelf
//! and a find are priced by the one number.
//!
//! [`Wares::stock`] is the pure half, tested without an `App`: one
//! keeper's [`Stocking`] at a tier from whatever generator it is handed.
//! [`Wares::stocking`] is every keeper's at once from
//! `Seed::stream(b"undermere.wares", number)`, a stream for each stocking
//! of the run, so the same seed meets the same shelves however the run got
//! there.
//!
//! Undermere's own rather than the engine's drop or scatter: a shelf is a
//! fixed number of draws, some of them capped at one, on top of what is
//! always there, and neither of those reads a count like that.

use std::ops::RangeInclusive;

use bevy::prelude::*;
use rand::Rng;
use rand::rngs::StdRng;
use rl_engine::prelude::*;
use rl_engine::rl_rules::loot::{DropRow, DropTable, LevelRow, LevelTable, LootRow, LootTable};
use ron::extensions::Extensions;
use ron::options::Options;
use ron::value::RawValue;
use serde::Deserialize;

use crate::armory::{Armory, ItemDef, RING_LEVELS, STAFF_LEVEL};

const WARES_RON: &str = include_str!("../assets/wares.ron");

/// The stream every stocking draws from, with the stocking's number as its
/// index.
pub const WARES_STREAM: &[u8] = b"undermere.wares";

/// The tiers a stocking is drawn for: one for every two floors, so a run
/// sees at most five stockings.
pub const TIERS: RangeInclusive<u32> = 1..=5;

/// The gold a keeper holds for every tier its stocking was drawn for: what
/// it can pay out for what the adventurer brings up, set afresh with each
/// stocking rather than added to, so selling everything to one keeper at
/// the first tier empties it.
pub const PURSE_PER_TIER: u32 = 200;

/// What no keeper ever stocks, by name: the five artifacts, which are each
/// a mini-boss's alone, and the two scrolls that only ever harm their
/// reader, which keepers buy cheap and never sell. Gold is refused by
/// having no price.
pub const NEVER_STOCKED: [&str; 7] = [
    "the Crown of Grubnak",
    "the Widow's Fang",
    "the Choirmaster's Staff",
    "the Abbess's Chalice",
    "the Forgemaster's Hammer",
    "scroll of aggravate monsters",
    "scroll of summon monsters",
];

/// The tier a stocking drawn after reaching floor `floor` is for: one for
/// floors one and two and before any floor, then one more for every two
/// floors, five for the last two.
pub fn tier_of(floor: u32) -> u32 {
    (floor.saturating_sub(1) / 2 + 1).clamp(*TIERS.start(), *TIERS.end())
}

/// A keeper's purse for a stocking at `tier`: [`PURSE_PER_TIER`] a tier.
pub fn purse(tier: u32) -> u32 {
    PURSE_PER_TIER * tier
}

/// The kinds a keeper buys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Buys {
    /// Anything with a price: the curio.
    Anything,
    /// Things carrying one of these tags and a price; none buys nothing.
    Tags(Vec<TagId>),
}

/// One ware on a shelf: what it is, how many, and its level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ware {
    /// What it is.
    pub def: Id<ItemDef>,
    /// How many lie together on the shelf.
    pub count: u32,
    /// Its level, nought for a thing that takes none and never below.
    pub level: i32,
}

/// One keeper's shelf and purse for one stocking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stocking {
    /// The keeper's name in `wares.ron`.
    pub keeper: String,
    /// What is on the shelf: what is always there first, then what was
    /// drawn, then what turned up on its own chance.
    pub wares: Vec<Ware>,
    /// The gold the keeper can pay out, [`purse`] at the stocking's tier.
    pub purse: u32,
}

/// One keeper as `wares.ron` writes it.
#[derive(Debug, Clone)]
pub struct Keeper {
    /// Who stands behind the counter, the name the shop is found by.
    pub name: String,
    /// The kinds it buys.
    pub buys: Buys,
    /// What every stocking holds, with how many and the level a thing of
    /// it is at plain, when it takes levels.
    always: Vec<(Id<ItemDef>, u32, Option<i32>)>,
    /// How many parts are drawn by weight a stocking.
    draws: u32,
    /// The parts, each item at most once.
    parts: Vec<Part>,
}

/// One part of a keeper's shelf, drawn or on its own chance.
#[derive(Debug, Clone)]
struct Part {
    /// The item's name, which the table is sorted by.
    name: String,
    /// The tiers, weight and count, as a loot table's row.
    row: LootRow<Id<ItemDef>>,
    /// Percent of stockings it is on the shelf, instead of being drawn.
    chance: Option<u32>,
    /// The most times it is drawn in one stocking.
    most: Option<u32>,
    /// The level a thing of it is at plain, when it takes levels.
    plain: Option<i32>,
}

impl Keeper {
    /// Whether it buys a thing of `def`: one with a price, carrying a tag
    /// it names unless it buys anything.
    pub fn buys(&self, def: &ItemDef) -> bool {
        def.price.is_some()
            && match &self.buys {
                Buys::Anything => true,
                Buys::Tags(tags) => def.tags.iter().any(|t| tags.contains(&t.id())),
            }
    }
}

/// Everything `wares.ron` holds: every keeper, and the one level table
/// their wares are rolled from.
#[derive(Resource, Debug, Clone)]
pub struct Wares {
    levels: LevelTable,
    keepers: Vec<Keeper>,
}

impl Wares {
    /// Reads `assets/wares.ron` against `registries` and `armory`. Panics
    /// naming the file and every problem in it.
    pub fn load(registries: &Registries, armory: &Armory) -> Self {
        Self::read(WARES_RON, registries, armory).unwrap_or_else(|e| panic!("assets/wares.ron: {e}"))
    }

    /// Reads a wares file: every name that resolves to nothing first, and
    /// once every name resolves, every other problem at once, each under
    /// its keeper.
    pub fn read(text: &str, registries: &Registries, armory: &Armory) -> Result<Self, ContentError> {
        let options = Options::default().with_default_extension(Extensions::IMPLICIT_SOME);
        let file: AuthoredFile = options.from_str(text).map_err(|e| ContentError::Parse(e.to_string()))?;
        let levels = LevelTable::new(file.levels).map_err(|e| ContentError::Invalid(e.into_iter().map(|e| format!("levels: {e}")).collect()))?;
        let authored: Vec<AuthoredKeeper> = registries.names().with("item", &armory.defs).load_list(file.keepers.get_ron())?;
        let mut errors = Vec::new();
        let mut keepers: Vec<Keeper> = Vec::new();
        for a in authored {
            if keepers.iter().any(|k| k.name == a.name) {
                errors.push(format!("{}: a second keeper of the name", a.name));
            }
            keepers.push(a.build(registries, armory, &mut errors));
        }
        if !errors.is_empty() {
            return Err(ContentError::Invalid(errors));
        }
        Ok(Self { levels, keepers })
    }

    /// Every keeper, in the file's order.
    pub fn keepers(&self) -> &[Keeper] {
        &self.keepers
    }

    /// The keeper called `name`, or `None`.
    pub fn keeper(&self, name: &str) -> Option<&Keeper> {
        self.keepers.iter().find(|k| k.name == name)
    }

    /// Draws one stocking of `keeper`'s shelf at `tier` from `rng`: what is
    /// always there, then its draws by weight among the parts at the tier
    /// that have not reached their most, then each part with a chance at
    /// the tier rolled on its own. Each thing that takes levels rolls one
    /// from the level table at the tier on top of its plain level, so it is
    /// never below nought; a thing that takes none draws nothing for it.
    /// Draws stop early when nothing is left to draw. Panics on a keeper
    /// `wares.ron` has not, which is a typo in the caller.
    pub fn stock(&self, keeper: &str, tier: u32, rng: &mut impl Rng) -> Stocking {
        let keeper = self.keeper(keeper).unwrap_or_else(|| panic!("no keeper called {keeper:?} in wares.ron"));
        let band = tier as i32;
        let mut wares: Vec<Ware> = Vec::new();
        for &(def, count, plain) in &keeper.always {
            wares.push(Ware { def, count, level: self.level(plain, band, rng) });
        }
        let drawn: Vec<&Part> = keeper.parts.iter().filter(|p| p.chance.is_none()).collect();
        let mut taken = vec![0; drawn.len()];
        for _ in 0..keeper.draws {
            let open = drawn.iter().zip(&taken).filter(|(p, n)| p.most.is_none_or(|most| **n < most)).map(|(p, _)| p.row.clone()).collect();
            let table = LootTable::new(open, |id| drawn.iter().find(|p| p.row.item == id).map(|p| p.name.clone()).unwrap_or_default());
            let Some((def, count)) = table.pick(band, rng) else { break };
            let i = drawn.iter().position(|p| p.row.item == def).expect("drawn from these parts");
            taken[i] += 1;
            wares.push(Ware { def, count, level: self.level(drawn[i].plain, band, rng) });
        }
        let chanced: Vec<&Part> = keeper.parts.iter().filter(|p| p.chance.is_some() && p.row.applies(band)).collect();
        let rows = chanced.iter().map(|p| DropRow::new(p.row.item, p.chance.unwrap_or(0)).count(p.row.group.0, p.row.group.1)).collect();
        for (def, count) in DropTable(rows).roll(rng) {
            let plain = chanced.iter().find(|p| p.row.item == def).and_then(|p| p.plain);
            wares.push(Ware { def, count, level: self.level(plain, band, rng) });
        }
        Stocking { keeper: keeper.name.clone(), wares, purse: purse(tier) }
    }

    /// The level of a ware at plain level `plain` stocked at `band`: the
    /// level table's roll on top of it, or nought for a thing that takes no
    /// levels, which draws nothing.
    fn level(&self, plain: Option<i32>, band: i32, rng: &mut impl Rng) -> i32 {
        plain.map_or(0, |plain| plain + self.levels.roll(band, rng))
    }

    /// Stocking number `number` of every keeper at `tier`, in the file's
    /// order, all from `Seed::stream(WARES_STREAM, number)`: the same seed
    /// and number meet the same shelves, and no stocking's draws move
    /// another's.
    pub fn stocking(&self, seed: &Seed, number: u64, tier: u32) -> Vec<Stocking> {
        let mut rng: StdRng = seed.stream(WARES_STREAM, number);
        self.keepers.iter().map(|k| self.stock(&k.name, tier, &mut rng)).collect()
    }
}

/// The level a thing of `def` is at plain, or `None` for a thing that
/// takes no levels: the least a found one of its kind is at uncursed, so a
/// ring sold does something and a staff sold has charges. Never below
/// nought.
fn plain_level(def: Id<ItemDef>, registries: &Registries, armory: &Armory) -> Option<i32> {
    if !armory.takes_levels(def) {
        return None;
    }
    let d = armory.defs.get(def);
    let tagged = |name: &str| d.tags.iter().any(|t| registries.tags.name(t.id()) == name);
    let plain = match d.found_at {
        Some((lowest, _)) => lowest,
        None if tagged("staff") => STAFF_LEVEL,
        None if tagged("ring") => *RING_LEVELS.start(),
        None => 0,
    };
    Some(plain.max(0))
}

/// The file as written: the level rows, and the keepers left raw so their
/// names are read with the registries in scope.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredFile {
    levels: Vec<LevelRow>,
    keepers: Box<RawValue>,
}

/// A keeper as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredKeeper {
    name: String,
    buys: AuthoredBuys,
    #[serde(default)]
    always: Vec<AuthoredAlways>,
    #[serde(default)]
    draws: u32,
    #[serde(default)]
    parts: Vec<AuthoredPart>,
}

/// What a keeper buys, as written.
#[derive(Deserialize)]
enum AuthoredBuys {
    Anything,
    Tags(Vec<NameRef<TagDef>>),
}

/// Something always on a shelf, as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredAlways {
    item: NameRef<ItemDef>,
    count: u32,
}

/// A part, as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredPart {
    item: NameRef<ItemDef>,
    bands: (i32, i32),
    #[serde(default)]
    weight: Option<u32>,
    #[serde(default)]
    chance: Option<u32>,
    #[serde(default)]
    group: Option<(u32, u32)>,
    #[serde(default)]
    most: Option<u32>,
}

impl AuthoredKeeper {
    /// The keeper, every problem with it pushed onto `errors`.
    fn build(self, registries: &Registries, armory: &Armory, errors: &mut Vec<String>) -> Keeper {
        let buys = match self.buys {
            AuthoredBuys::Anything => Buys::Anything,
            AuthoredBuys::Tags(tags) => Buys::Tags(tags.into_iter().map(NameRef::id).collect()),
        };
        let mut keeper = Keeper { name: self.name, buys, always: Vec::new(), draws: self.draws, parts: Vec::new() };
        let check = |def: Id<ItemDef>, keeper: &Keeper, errors: &mut Vec<String>| {
            let d = armory.defs.get(def);
            let at = format!("{}: {}", keeper.name, d.name);
            if d.price.is_none() {
                errors.push(format!("{at} has no price, so is never for sale"));
            } else if !keeper.buys(d) {
                errors.push(format!("{at} is nothing the keeper buys"));
            }
            if NEVER_STOCKED.contains(&d.name.as_str()) {
                errors.push(format!("{at} is never stocked"));
            }
            at
        };
        for a in self.always {
            let def = a.item.id();
            let at = check(def, &keeper, errors);
            if a.count == 0 {
                errors.push(format!("{at}: a count of nothing is never on the shelf"));
            }
            keeper.always.push((def, a.count, plain_level(def, registries, armory)));
        }
        for a in self.parts {
            let def = a.item.id();
            let at = check(def, &keeper, errors);
            if keeper.parts.iter().any(|p| p.row.item == def) {
                errors.push(format!("{at}: a second part of the same thing"));
            }
            let (first, last) = a.bands;
            if first > last || !TIERS.contains(&(first.max(0) as u32)) || !TIERS.contains(&(last.max(0) as u32)) {
                errors.push(format!("{at}: tiers {first} to {last} are not a range within {} to {}", TIERS.start(), TIERS.end()));
            }
            match (a.weight, a.chance) {
                (Some(0), _) => errors.push(format!("{at}: a weight of nothing is never drawn")),
                (Some(_), Some(_)) => errors.push(format!("{at}: drawn by weight or on a chance, not both")),
                (None, None) => errors.push(format!("{at}: neither a weight nor a chance")),
                (None, Some(chance)) if !(1..=100).contains(&chance) => errors.push(format!("{at}: a chance of {chance} percent")),
                _ => {}
            }
            let group = a.group.unwrap_or((1, 1));
            if group.0 == 0 || group.0 > group.1 {
                errors.push(format!("{at}: a group of {} to {} is no group at all", group.0, group.1));
            }
            if a.most == Some(0) {
                errors.push(format!("{at}: drawn at most no times is never drawn"));
            }
            let row = LootRow::new(def).bands(first, last).weight(a.weight.unwrap_or(0)).group(group.0, group.1);
            keeper.parts.push(Part {
                name: armory.defs.get(def).name.clone(),
                row,
                chance: a.chance,
                most: a.most,
                plain: plain_level(def, registries, armory),
            });
        }
        keeper
    }
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rl_engine::rl_core::RunSeed;

    use super::*;
    use crate::content::Library;

    fn library() -> Library {
        Library::load(&crate::content::effect_kinds(), &Moments::default())
    }

    /// Every keeper's stocking at `tier` for each of a span of seeds, drawn
    /// as the run draws its first.
    fn stockings(library: &Library, tier: u32) -> impl Iterator<Item = Stocking> + '_ {
        (0..200).flat_map(move |s| library.wares.stocking(&Seed(RunSeed(s)), 0, tier))
    }

    fn names(library: &Library, stocking: &Stocking) -> Vec<String> {
        stocking.wares.iter().map(|w| library.armory.defs.get(w.def).name.clone()).collect()
    }

    /// A fixture's problems, read against the game's own items.
    fn refused(library: &Library, keepers: &str) -> Vec<String> {
        let text = format!("(levels: [(bands: (1, 5), levels: [(0, 1)])], keepers: [{keepers}])");
        match Wares::read(&text, &library.registries, &library.armory) {
            Err(ContentError::Invalid(errors)) => errors,
            other => panic!("expected the fixture refused, got {other:?}"),
        }
    }

    #[test]
    fn the_tiers_run_two_floors_each_from_the_town_to_the_throne() {
        assert_eq!((0..=10).map(tier_of).collect::<Vec<_>>(), [1, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5]);
    }

    #[test]
    fn the_same_seed_and_stocking_number_stock_the_same_shelves_and_another_number_others_over_a_span_of_seeds() {
        let library = library();
        let mut differ = 0;
        for s in 0..50 {
            let seed = Seed(RunSeed(s));
            for tier in TIERS {
                assert_eq!(library.wares.stocking(&seed, tier as u64, tier), library.wares.stocking(&seed, tier as u64, tier), "seed {s} tier {tier}");
            }
            differ += usize::from(library.wares.stocking(&seed, 0, 1) != library.wares.stocking(&seed, 1, 1));
        }
        assert!(differ >= 45, "a second stocking is a fresh draw: {differ} of 50 differed");
    }

    #[test]
    fn no_ware_is_below_nought_an_artifact_gold_or_a_scroll_that_only_harms_over_a_span_of_seeds() {
        let library = library();
        for tier in TIERS {
            for stocking in stockings(&library, tier) {
                for ware in &stocking.wares {
                    let def = library.armory.defs.get(ware.def);
                    assert!(ware.level >= 0, "{} at {} in tier {tier}", def.name, ware.level);
                    assert!(def.price.is_some(), "{} has no price", def.name);
                    assert!(!NEVER_STOCKED.contains(&def.name.as_str()), "{} on {}'s shelf", def.name, stocking.keeper);
                }
            }
        }
    }

    #[test]
    fn plate_armor_waits_for_tier_four_and_a_potion_of_life_for_tier_five_over_a_span_of_seeds() {
        let library = library();
        let seen = |tier: u32, name: &str| stockings(&library, tier).any(|s| names(&library, &s).iter().any(|n| n == name));
        for tier in 1..=3 {
            assert!(!seen(tier, "plate armor"), "plate armor at tier {tier}");
        }
        for tier in 1..=4 {
            assert!(!seen(tier, "potion of life"), "a potion of life at tier {tier}");
        }
        assert!(seen(4, "plate armor") && seen(5, "potion of life"));
    }

    #[test]
    fn a_potion_of_life_is_on_the_curios_shelf_in_about_a_quarter_of_its_deepest_stockings_over_a_span_of_seeds() {
        let library = library();
        let curio: Vec<Stocking> = stockings(&library, 5).filter(|s| s.keeper == "curio").collect();
        let life = curio.iter().filter(|s| names(&library, s).iter().any(|n| n == "potion of life")).count();
        assert!((30..=70).contains(&life), "{life} of {}", curio.len());
    }

    #[test]
    fn a_scroll_of_enchanting_and_a_potion_of_strength_are_stocked_at_most_once_a_stocking_over_a_span_of_seeds() {
        let library = library();
        for tier in TIERS {
            for stocking in stockings(&library, tier) {
                let names = names(&library, &stocking);
                for capped in ["scroll of enchanting", "potion of strength"] {
                    assert!(names.iter().filter(|n| *n == capped).count() <= 1, "{capped} twice in {names:?}");
                }
            }
        }
    }

    #[test]
    fn arrows_and_three_potions_of_healing_are_always_there_and_each_keeper_draws_its_count_over_a_span_of_seeds() {
        let library = library();
        let healing = library.armory.defs.expect("potion of healing");
        let arrow = library.armory.defs.expect("arrow");
        for tier in TIERS {
            for stocking in stockings(&library, tier) {
                let first = stocking.wares.first().map(|w| (w.def, w.count));
                match stocking.keeper.as_str() {
                    "smith" => assert_eq!((first, stocking.wares.len()), (Some((arrow, 10)), 7)),
                    "apothecary" => assert_eq!((first, stocking.wares.len()), (Some((healing, 3)), 5)),
                    "curio" => assert!((4..=5).contains(&stocking.wares.len()), "four drawn, and a potion of life by chance"),
                    "temple" => assert!(stocking.wares.is_empty(), "its rites are not wares"),
                    other => panic!("a keeper called {other}"),
                }
            }
        }
    }

    #[test]
    fn a_weapon_on_the_shelf_is_plain_at_tier_one_one_better_from_tier_two_and_two_from_tier_four_over_a_span_of_seeds() {
        let library = library();
        let weapon = library.registries.tags.expect("weapon");
        let levels = |tier: u32| -> Vec<i32> {
            let weapons = stockings(&library, tier).flat_map(|s| s.wares).filter(|w| library.armory.defs.get(w.def).tags.iter().any(|t| t.id() == weapon));
            let mut levels: Vec<i32> = weapons.map(|w| w.level).collect();
            levels.sort_unstable();
            levels.dedup();
            levels
        };
        assert_eq!(levels(1), [0]);
        assert_eq!(levels(2), [0, 1]);
        assert_eq!(levels(4), [0, 1, 2]);
    }

    #[test]
    fn a_ring_or_a_staff_on_the_shelf_is_at_least_as_good_as_one_found_plain_over_a_span_of_seeds() {
        let library = library();
        let r = &library.registries;
        let (ring, staff) = (r.tags.expect("ring"), r.tags.expect("staff"));
        for ware in stockings(&library, 2).flat_map(|s| s.wares) {
            let tags: Vec<TagId> = library.armory.defs.get(ware.def).tags.iter().map(|t| t.id()).collect();
            if tags.contains(&ring) {
                assert!(ware.level >= *RING_LEVELS.start());
            }
            if tags.contains(&staff) {
                assert!(ware.level >= STAFF_LEVEL);
            }
        }
    }

    #[test]
    fn a_keepers_purse_is_two_hundred_gold_a_tier() {
        let library = library();
        let mut rng = StdRng::seed_from_u64(1);
        for tier in TIERS {
            assert_eq!(library.wares.stock("smith", tier, &mut rng).purse, 200 * tier);
        }
    }

    #[test]
    fn every_keeper_of_the_town_is_in_the_file_and_stocks_only_what_it_buys() {
        let library = library();
        let wares = &library.wares;
        assert_eq!(wares.keepers().iter().map(|k| k.name.as_str()).collect::<Vec<_>>(), ["smith", "apothecary", "curio", "temple"]);
        for tier in TIERS {
            for stocking in stockings(&library, tier) {
                let keeper = wares.keeper(&stocking.keeper).expect("a keeper of the file");
                for ware in &stocking.wares {
                    assert!(keeper.buys(library.armory.defs.get(ware.def)), "{} sells what it would not buy", keeper.name);
                }
            }
        }
    }

    #[test]
    fn a_ware_not_in_items_ron_is_a_load_error_naming_it() {
        let library = library();
        let text = r#"(levels: [], keepers: [(name: "smith", buys: Anything, draws: 1, parts: [(item: "sword of legend", bands: (1, 5), weight: 1)])])"#;
        let error = Wares::read(text, &library.registries, &library.armory).expect_err("no such item").to_string();
        assert!(error.contains("sword of legend"), "{error}");
    }

    #[test]
    fn gold_an_artifact_a_bad_scroll_or_a_kind_the_keeper_does_not_buy_is_refused_naming_each() {
        let library = library();
        let errors = refused(
            &library,
            r#"(name: "smith", buys: Tags(["weapon"]), always: [(item: "gold coin", count: 1)], draws: 1, parts: [
                (item: "the Widow's Fang", bands: (1, 5), weight: 1),
                (item: "scroll of summon monsters", bands: (1, 5), weight: 1),
                (item: "sword", bands: (0, 6), weight: 1, chance: 10),
            ])"#,
        );
        let joined = errors.join("\n");
        for said in
            ["gold coin has no price", "the Widow's Fang is never stocked", "scroll of summon monsters is nothing the keeper buys", "tiers 0 to 6", "not both"]
        {
            assert!(joined.contains(said), "{said:?} not in:\n{joined}");
        }
    }
}
