//! What a sword for hire carries and asks: `assets/mercenaries.ron` read
//! against the armory, a kit rolled from it at a band, and its fee.

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_rules::loot::{LootRow, LootTable};
use ron::extensions::Extensions;
use ron::options::Options;
use ron::value::RawValue;
use serde::Deserialize;

use super::OwnKit;
use crate::armory::{Armory, ItemDef, spawn_item_at, spawn_items};
use crate::dungeon::FLOORS;
use crate::shops::{AuthoredRow, Source, Ware};

const MERCENARIES_RON: &str = include_str!("../../assets/mercenaries.ron");

/// What a candidate asks to be hired, `fee` in `mercenaries.ron`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fee {
    /// What a candidate carrying nothing worth anything asks.
    pub base: u32,
    /// The percent of its kit's worth it asks on top.
    pub share_pct: u32,
}

/// One part of a kit: whether a candidate carries one, and which.
#[derive(Debug, Clone)]
struct Part {
    /// What the file calls it.
    name: String,
    /// Percent of candidates carrying one.
    chance: u32,
    /// Which one, drawn at the band.
    rows: LootTable<Source>,
}

/// Everything `mercenaries.ron` holds: the fee, the parts of a kit, and
/// the levels its things are rolled at.
#[derive(Resource, Debug, Clone)]
pub struct Mercenaries {
    fee: Fee,
    parts: Vec<Part>,
    levels: LootTable<i32>,
    /// The armory's table, which a tag row draws from.
    table: LootTable<Id<ItemDef>>,
}

impl Mercenaries {
    /// Reads `assets/mercenaries.ron` against `registries` and `armory`.
    /// Panics naming the file and every problem in it.
    pub fn load(registries: &Registries, armory: &Armory) -> Self {
        Self::read(MERCENARIES_RON, registries, armory).unwrap_or_else(|e| panic!("assets/mercenaries.ron: {e}"))
    }

    /// Reads a mercenaries file: every problem in it at once, each under
    /// the part or the level row it is in, an item `items.ron` has not
    /// among them.
    pub fn read(text: &str, registries: &Registries, armory: &Armory) -> Result<Self, ContentError> {
        let options = Options::default().with_default_extension(Extensions::IMPLICIT_SOME);
        let file: AuthoredFile = options.from_str(text).map_err(|e| ContentError::Parse(e.to_string()))?;
        let names = registries.names().with("item", &armory.defs);
        let mut errors = Vec::new();
        let mut parts: Vec<Part> = Vec::new();
        for a in file.parts {
            let at = a.name.clone();
            if parts.iter().any(|p| p.name == a.name) {
                errors.push(format!("{at}: a second part of the name"));
            }
            if !(1..=100).contains(&a.chance) {
                errors.push(format!("{at}: a chance of {} percent", a.chance));
            }
            let authored: Vec<AuthoredRow> = match names.load_list(a.rows.get_ron()) {
                Ok(authored) => authored,
                Err(ContentError::Invalid(problems)) => {
                    errors.extend(problems.into_iter().map(|e| format!("{at}: {e}")));
                    continue;
                }
                Err(other) => {
                    errors.push(format!("{at}: {other}"));
                    continue;
                }
            };
            let mut rows = Vec::new();
            for (i, row) in authored.iter().enumerate() {
                let source = match row.source(armory) {
                    Ok(source) => source,
                    Err(why) => {
                        errors.push(format!("{at}: entry {}: {why}", i + 1));
                        continue;
                    }
                };
                let name = source.name(armory, registries);
                floors(&format!("{at}: {name}"), row.bands, &mut errors);
                let weight = row.weight.unwrap_or(0);
                if weight == 0 {
                    errors.push(format!("{at}: {name}: a weight of nothing is never drawn; leave the row out instead"));
                }
                if row.chance.is_some() {
                    errors.push(format!("{at}: {name}: a kit's row has a weight, never a chance"));
                }
                let group = row.group.unwrap_or((1, 1));
                if group.0 == 0 || group.0 > group.1 {
                    errors.push(format!("{at}: {name}: a group of {} to {} is no group at all", group.0, group.1));
                }
                rows.push(LootRow::new(source).bands(row.bands.0, row.bands.1).weight(weight).group(group.0, group.1));
            }
            if rows.is_empty() {
                errors.push(format!("{at}: no rows, so never carried"));
            }
            let rows = LootTable::new(rows, |source| source.name(armory, registries));
            parts.push(Part { name: a.name, chance: a.chance, rows });
        }
        let mut levels = Vec::new();
        for (i, a) in file.levels.into_iter().enumerate() {
            let at = format!("levels: row {}", i + 1);
            floors(&at, a.bands, &mut errors);
            if a.weight == 0 {
                errors.push(format!("{at}: a weight of nothing is never drawn"));
            }
            levels.push(LootRow::new(a.level).bands(a.bands.0, a.bands.1).weight(a.weight));
        }
        if !errors.is_empty() {
            return Err(ContentError::Invalid(errors));
        }
        Ok(Self { fee: file.fee, parts, levels: LootTable::new(levels, |level| level.to_string()), table: armory.table.clone() })
    }

    /// Rolls one candidate's kit at `band`, the deepest floor the run has
    /// reached, from `rng`: each part in the file's order, carried on its
    /// own chance, which one by its rows at the band, and a level for a
    /// thing that takes levels by the level rows at the band on top of
    /// its kind's [plain level](Armory::plain_level). A part with no row
    /// at the band is not carried, and a thing that takes no levels draws
    /// none and is at nought.
    pub fn kit(&self, band: u32, armory: &Armory, rng: &mut impl Rng) -> Vec<Ware> {
        let band = band as i32;
        let mut kit = Vec::new();
        for part in &self.parts {
            if rng.random_range(0..100) >= part.chance {
                continue;
            }
            let Some((source, count)) = part.rows.pick(band, rng) else { continue };
            let Some(def) = source.draw(&self.table, band, |_| true, rng) else { continue };
            let level = armory.plain_level(def).map_or(0, |plain| plain + self.levels.pick(band, rng).map_or(0, |(level, _)| level));
            kit.push(Ware { def, count, level });
        }
        kit
    }

    /// What a candidate carrying `kit` asks: the fee's base, plus its share
    /// of what everything in the kit is worth with its kind and its level
    /// told, as [`Armory::told_worth`] prices it. A thing below nought is
    /// worth nothing, so a cursed sword adds nothing to its carrier's
    /// price; the share rounds down.
    pub fn fee(&self, kit: &[Ware], armory: &Armory) -> u32 {
        let worth: u64 = kit.iter().map(|w| u64::from(armory.told_worth(w.def, w.level).unwrap_or(0)) * u64::from(w.count)).sum();
        let share = worth * u64::from(self.fee.share_pct) / 100;
        self.fee.base.saturating_add(u32::try_from(share).unwrap_or(u32::MAX))
    }
}

/// Makes the things of `kit`, placed nowhere, for a candidate to carry:
/// each that takes levels one at a time at its own, so one below nought
/// is cursed and will not come off once worn, as a thing found below is,
/// and the rest as [`spawn_items`] makes them. Made as told rather than
/// found, never unfamiliar, since a hireling says what it is selling; and
/// each its carrier's [`OwnKit`], which it uses knowing what it is.
pub fn make_kit(commands: &mut Commands, armory: &Armory, kit: &[Ware], registries: &Registries) -> Vec<Entity> {
    let mut made = Vec::new();
    for w in kit {
        if armory.takes_levels(w.def) {
            made.extend((0..w.count).map(|_| spawn_item_at(commands, armory, w.def, w.level, registries)));
        } else {
            made.extend(spawn_items(commands, armory, w.def, w.count, registries));
        }
    }
    for thing in &made {
        commands.entity(*thing).insert(OwnKit);
    }
    made
}

/// Pushes onto `errors` when `bands` is not a range of bands from nought,
/// the first evening, to the deepest floor.
fn floors(at: &str, bands: (i32, i32), errors: &mut Vec<String>) {
    let (first, last) = bands;
    let deepest = FLOORS as i32;
    if first > last || first < 0 || last > deepest {
        errors.push(format!("{at}: bands {first} to {last} are not a range within 0 to {deepest}"));
    }
}

/// The file as written: the fee, the parts with their rows left raw so the
/// engine's spawn reader reads them, and the level rows.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredFile {
    fee: Fee,
    parts: Vec<AuthoredPart>,
    levels: Vec<AuthoredLevel>,
}

/// A part, as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredPart {
    name: String,
    chance: u32,
    rows: Box<RawValue>,
}

/// A level row, as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredLevel {
    bands: (i32, i32),
    level: i32,
    weight: u32,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use bevy::ecs::world::CommandQueue;
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use rl_engine::rl_core::RunSeed;
    use rl_engine::rl_rules::prefab::{Pick, Slot};

    use super::*;
    use crate::content::Library;
    use crate::mercenaries::MERCENARY;
    use crate::townsfolk::NAMES_STREAM;

    fn library() -> Library {
        Library::load(&crate::content::effect_kinds(), &Moments::default())
    }

    /// The kits of a span of seeds at `band`, each from a generator of its
    /// own seed.
    fn kits(library: &Library, band: u32) -> Vec<Vec<Ware>> {
        (0..2000).map(|s| library.mercenaries.kit(band, &library.armory, &mut StdRng::seed_from_u64(s))).collect()
    }

    fn named(library: &Library, w: &Ware) -> String {
        library.armory.defs.get(w.def).name.clone()
    }

    fn tagged(library: &Library, w: &Ware, tag: &str) -> bool {
        let tag = library.registries.tags.expect(tag);
        library.armory.defs.get(w.def).tags.iter().any(|t| t.id() == tag)
    }

    fn refused(library: &Library, text: &str) -> String {
        match Mercenaries::read(text, &library.registries, &library.armory) {
            Err(e) => e.to_string(),
            Ok(_) => panic!("expected the fixture refused"),
        }
    }

    /// A part that names a tag carries only items carrying it, from the
    /// band's rows of the armory's table, over a span of seeds; and a row
    /// naming both an item and a tag is refused.
    #[test]
    fn a_tag_row_draws_only_items_carrying_the_tag_over_a_span_of_seeds() {
        let library = library();
        let text =
            r#"(fee: (base: 80, share_pct: 100), parts: [(name: "weapon", chance: 100, rows: [(tag: "weapon", bands: (0, 10), weight: 1)])], levels: [])"#;
        let merc = Mercenaries::read(text, &library.registries, &library.armory).expect("a tag row reads");
        let weapon = library.registries.tags.expect("weapon");
        let mut seen = std::collections::BTreeSet::new();
        for s in 0..400 {
            for band in [0, 5, 10] {
                let kit = merc.kit(band, &library.armory, &mut StdRng::seed_from_u64(s));
                assert_eq!(kit.len(), 1, "seed {s}: the one weapon");
                assert!(tagged(&library, &kit[0], "weapon"), "seed {s}: {} is no weapon", named(&library, &kit[0]));
                assert!(library.armory.table.rows().iter().any(|r| r.item == kit[0].def && r.tags.contains(&weapon)), "laid by the table");
                seen.insert((band, named(&library, &kit[0])));
            }
        }
        assert!(seen.contains(&(10, "war hammer".to_string())) && !seen.contains(&(0, "war hammer".to_string())), "deeper bands draw deeper rows: {seen:?}");
        let error = refused(
            &library,
            r#"(fee: (base: 80, share_pct: 100), parts: [(name: "weapon", chance: 100, rows: [(item: "sword", tag: "weapon", bands: (0, 10), weight: 1)])], levels: [])"#,
        );
        assert!(error.contains("weapon: entry 1: an item or a tag, not both"), "{error}");
    }

    /// The mercenaries' set never runs dry: on every seed of a span it
    /// draws a hundred names, each distinct from every one before it, each
    /// one the set itself accepts, every word in title case; and about one
    /// in four carries a byname.
    #[test]
    fn the_mercenary_set_draws_a_hundred_distinct_names_on_every_seed_of_a_span() {
        let names = library().names;
        let set = names.mercenary();
        let mut bynamed = 0;
        for s in 0..200 {
            let mut rng = Seed(RunSeed(s)).stream(NAMES_STREAM, 1);
            let mut drawn: BTreeSet<String> = BTreeSet::new();
            for n in 0..100 {
                let name = set.draw_distinct(&mut rng, |taken| drawn.contains(taken)).unwrap_or_else(|| panic!("seed {s}: dry at name {n}"));
                assert!(set.accepts(&name), "seed {s}: {name}");
                for word in name.split(' ') {
                    assert!(word.starts_with(char::is_uppercase) && word.chars().skip(1).all(char::is_lowercase), "seed {s}: {name} in title case");
                }
                bynamed += u32::from(name.contains(' '));
                drawn.insert(name);
            }
        }
        assert!((4000..=6000).contains(&bynamed), "{bynamed} of 20000 with a byname");
    }

    /// Every kit has a weapon; about four in five have armor and about one
    /// in two something to drink or throw, each within a few in a hundred,
    /// at the first evening's band and the deepest alike.
    #[test]
    fn a_kit_always_has_a_weapon_armor_about_four_times_in_five_and_a_consumable_about_one_in_two_over_a_span_of_seeds() {
        let library = library();
        for band in [0, FLOORS] {
            let (mut armor, mut consumable) = (0, 0);
            for kit in kits(&library, band) {
                assert_eq!(kit.iter().filter(|w| tagged(&library, w, "weapon")).count(), 1, "band {band}: one weapon in {kit:?}");
                armor += kit.iter().filter(|w| tagged(&library, w, "armor")).count();
                consumable += kit.iter().filter(|w| !tagged(&library, w, "weapon") && !tagged(&library, w, "armor")).count();
            }
            assert!((1520..=1680).contains(&armor), "band {band}: {armor} of 2000 with armor");
            assert!((900..=1100).contains(&consumable), "band {band}: {consumable} of 2000 with a consumable");
        }
    }

    /// A broadsword, chain mail and a level of two wait for the third
    /// floor: never in a kit on the first evening, and each in some kit of
    /// a span once the run has been down to the third.
    #[test]
    fn a_later_band_reaches_a_broadsword_chain_mail_and_a_level_two_the_first_evening_never_does_over_a_span_of_seeds() {
        let library = library();
        let reached = |band: u32| {
            let things: Vec<Ware> = kits(&library, band).into_iter().flatten().collect();
            let has = |name: &str| things.iter().any(|w| named(&library, w) == name);
            (has("broadsword"), has("chain mail"), things.iter().any(|w| w.level == 2))
        };
        assert_eq!(reached(0), (false, false, false), "the first evening");
        assert_eq!(reached(3), (true, true, true), "once floor three is reached");
    }

    /// A fee is its base, eighty, plus all of what its kit is worth told:
    /// a sword one better is its price and a hundred, armor and a potion at
    /// plain their prices, darts their price each; a cursed sword adds
    /// nothing, and an empty kit costs the base. Over a span of seeds no
    /// candidate costs less than the base or more than its kit's whole
    /// worth and the base together.
    #[test]
    fn a_fee_is_its_base_plus_its_share_of_its_kits_worth() {
        let library = library();
        let (merc, armory) = (&library.mercenaries, &library.armory);
        let ware = |name: &str, count: u32, level: i32| Ware { def: armory.defs.expect(name), count, level };
        let kit = [ware("sword", 1, 1), ware("leather armor", 1, 0), ware("dart", 6, 0), ware("potion of healing", 1, 0)];
        assert_eq!(merc.fee(&kit, armory), 80 + 60 + 100 + 40 + 6 * 3 + 40);
        assert_eq!(merc.fee(&[ware("sword", 1, -1)], armory), 80, "a cursed sword is worth nothing");
        assert_eq!(merc.fee(&[], armory), 80, "nothing worth anything");
        for kit in kits(&library, 5) {
            let worth: u32 = kit.iter().map(|w| armory.told_worth(w.def, w.level).unwrap_or(0) * w.count).sum();
            assert_eq!(merc.fee(&kit, armory), 80 + worth, "{kit:?}");
        }
    }

    /// On the first evening the cheapest candidate asks about what the
    /// smithy's best weapon of the first tier does, the axe or the bow, and
    /// the dearest asks several times that: a hireling is a purchase to
    /// save for.
    #[test]
    fn the_cheapest_first_evening_candidate_costs_a_smithys_best_first_tier_weapon_and_the_dearest_several_times_that() {
        let library = library();
        let armory = &library.armory;
        let best = ["axe", "bow"].map(|n| armory.defs.get(armory.defs.expect(n)).price.expect("priced")).into_iter().max().expect("two");
        let fees: Vec<u32> = kits(&library, 0).iter().map(|kit| library.mercenaries.fee(kit, armory)).collect();
        let (cheapest, dearest) = (*fees.iter().min().expect("kits"), *fees.iter().max().expect("kits"));
        assert!((best * 9 / 10..=best * 11 / 10).contains(&cheapest), "cheapest {cheapest} against a weapon at {best}");
        assert!(dearest >= 3 * best, "dearest {dearest} against a weapon at {best}");
    }

    /// A thing rolled below nought is made cursed as a thing found below
    /// is: at its level and stuck once worn; one at nought or better is
    /// not. Over a span of seeds about one weapon in five is rolled below.
    #[test]
    fn a_level_below_nought_is_cursed_as_one_found_below_is_over_a_span_of_seeds() {
        let library = library();
        let mut world = World::new();
        let mut cursed = 0;
        for kit in kits(&library, 0) {
            let mut queue = CommandQueue::default();
            let made = make_kit(&mut Commands::new(&mut queue, &world), &library.armory, &kit, &library.registries);
            queue.apply(&mut world);
            let levelled: Vec<&Ware> = kit.iter().filter(|w| library.armory.takes_levels(w.def)).collect();
            let things: Vec<Entity> = made.into_iter().filter(|&e| world.get::<Enchant>(e).is_some()).collect();
            assert_eq!(things.len(), levelled.len(), "one thing a levelled ware in {kit:?}");
            for (w, thing) in levelled.iter().zip(things) {
                let level = world.get::<Enchant>(thing).map(|e| e.level);
                assert_eq!(level, Some(w.level), "{}", named(&library, w));
                assert_eq!(world.get::<Stuck>(thing).is_some(), w.level < 0, "{} at {}", named(&library, w), w.level);
                cursed += u32::from(w.level < 0 && tagged(&library, w, "weapon"));
            }
        }
        assert!((330..=470).contains(&cursed), "{cursed} of 2000 weapons below nought");
    }

    /// The mercenary is one kind on the adventurer's footing: their health,
    /// sapient wits that use and wear what they carry, and running at a
    /// quarter of its health.
    #[test]
    fn the_mercenary_is_one_kind_on_the_adventurers_health_that_thinks_and_runs_at_a_quarter() {
        let library = library();
        let def = library.bestiary.defs.get(library.bestiary.defs.expect(MERCENARY));
        assert_eq!(def.hp, crate::run::HEALTH);
        assert!(def.folded.wits.has(Wits::SAPIENT) && def.folded.wits.has(Wits::USES) && def.folded.wits.has(Wits::EQUIPS), "{:?}", def.folded.wits);
        assert_eq!(def.folded.flee_at, Some(25));
    }

    /// The mercenary is never drawn as a floor's monster: no row of the
    /// spawn table names it at any band, no role a piece's slot draws from
    /// does, which draws by that table anyway, and no piece stands one by
    /// its kind; it is only ever met in town.
    #[test]
    fn the_mercenary_is_never_drawn_as_a_floors_monster() {
        let library = library();
        let merc = library.bestiary.defs.expect(MERCENARY);
        for band in -1..=FLOORS as i32 + 1 {
            assert!(library.bestiary.spawns.at(band).all(|row| row.item != merc), "drawn at band {band}");
        }
        let undermere = crate::dungeon::Undermere::new(RunSeed(0));
        let prefabs = undermere.prefabs();
        assert!(prefabs.roles().iter().all(|(_, role)| !role.members.contains(&merc)), "a member of a role");
        for (_, piece) in prefabs.defs().iter() {
            for (glyph, slot) in piece.slots() {
                assert!(!matches!(slot, Slot::Monster { pick: Pick::Kind(kind), .. } if *kind == merc), "{} stands one at {glyph:?}", piece.name);
            }
        }
    }

    /// An item `items.ron` has not is a load error naming it and its part.
    #[test]
    fn every_item_mercenaries_ron_names_exists_or_the_load_names_it() {
        let library = library();
        let error = refused(
            &library,
            r#"(fee: (base: 80, share_pct: 100), parts: [(name: "weapon", chance: 100, rows: [(item: "sword of legend", bands: (0, 10), weight: 1)])], levels: [])"#,
        );
        assert!(error.contains("weapon") && error.contains("sword of legend"), "{error}");
    }

    /// A chance outside one to a hundred, a second part of one name, a
    /// band past the deepest floor or before the first evening, and a
    /// weight of nothing are each refused, naming where.
    #[test]
    fn a_bad_chance_a_second_part_of_a_name_a_band_past_the_floors_and_a_weight_of_nothing_are_refused_naming_each() {
        let library = library();
        let error = refused(
            &library,
            r#"(fee: (base: 80, share_pct: 100), parts: [
                (name: "weapon", chance: 0, rows: [(item: "sword", bands: (0, 11), weight: 1)]),
                (name: "weapon", chance: 50, rows: [(item: "dagger", bands: (0, 10), weight: 0)]),
            ], levels: [(bands: (-1, 10), level: 0, weight: 0)])"#,
        );
        for said in
            ["weapon: a chance of 0 percent", "weapon: a second part", "weapon: sword: bands 0 to 11", "weight of nothing", "levels: row 1: bands -1 to 10"]
        {
            assert!(error.contains(said), "{said:?} not in:\n{error}");
        }
    }
}
