//! What Undermere's world is made of: its sides, its kinds of harm, its
//! stats, statuses, slots and tags, the props that stand in its places,
//! the abilities its monsters know, the rules a run reads its stats by,
//! and the order every content file is read in.
//!
//! Every name here is content the engine knows nothing about; the engine
//! sees ids. The registries are filled first, since every file names
//! them, and [`Library::load`] reads the files after them in the order
//! their names need: abilities, then the traits that grant them, then the
//! items that lend traits, then the monsters that are written as traits
//! and drop items, and last the faces the things of a kind wear until
//! they are known.
//!
//! The stats are the run's numbers that things change: [`STRENGTH`],
//! which heavy gear asks for, [`ACCURACY`] and [`EVASION`], which the hit
//! model weighs against each other, [`SPEED`], [`STEALTH`], [`MENDING`],
//! [`LIGHT`] and [`ARMOR`]. Each is read where the engine or the game
//! already reads the number it changes: [`combat_rules`] names four of
//! them to the engine's blows and heals, [`turn_rules`] names the speed
//! to the turn loop, and the rest are Undermere's own to read.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_rules::faction::FactionDef;
use serde::Deserialize;

use crate::armory::Armory;
use crate::bestiary::Bestiary;
use crate::traits::TraitDef;

/// The abilities file, compiled in so the binary runs from anywhere.
const ABILITIES_RON: &str = include_str!("../assets/abilities.ron");
/// The props file, compiled in beside it.
const PROPS_RON: &str = include_str!("../assets/props.ron");

/// How bright every light a file writes is at its heart, out of 255.
///
/// One brightness rather than a field, so how far a light reaches is the
/// one number a file gives for how bright it is: a lantern and a glowing
/// cap differ by their radius and their colour, and the lantern dimming
/// as the adventurer descends is its radius shrinking. 200 is what
/// Corsair's lantern, Delve's brand and Foundry's shoulder lamp all shed:
/// the light every example hands whoever carries it.
pub const LIGHT_INTENSITY: u8 = 200;

/// A light as Undermere's files write it: `(radius: 4, color: (0.35, 1.0,
/// 0.8), flicker: 0.3)`, in the same `0..=1` the files write every colour
/// in, rather than the engine's bytes.
///
/// A trait's glow and a lantern are the same thing, so `traits.ron` and
/// `items.ron` write them the same way and [`source`](Self::source) turns
/// both into the engine's [`LightSource`].
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LightDef {
    /// How far it reaches, in tiles.
    pub radius: i32,
    /// Its hue, `(r, g, b)` in `0..=1`.
    pub color: (f32, f32, f32),
    /// How far it flickers when drawn, `0` steady to `1` a flame that can
    /// dip to nothing; absent, steady.
    #[serde(default)]
    pub flicker: f32,
}

impl LightDef {
    /// The engine's light for it, at [`LIGHT_INTENSITY`].
    pub fn source(&self) -> LightSource {
        let byte = |unit: f32| (unit.clamp(0.0, 1.0) * 255.0).round() as u8;
        let (r, g, b) = self.color;
        LightSource::new(LIGHT_INTENSITY, self.radius, Rgb::new(byte(r), byte(g), byte(b))).flickering(byte(self.flicker))
    }
}

/// What heavy gear asks of its wearer: the adventurer's is 12, and a
/// potion of strength raises it by one for good.
pub const STRENGTH: &str = "strength";
/// Out of a hundred, before a target's evasion is taken off: what the hit
/// model reads as the chance a blow or a shot lands.
pub const ACCURACY: &str = "accuracy";
/// Taken off an attacker's accuracy: a shield's, and nothing else's yet.
pub const EVASION: &str = "evasion";
/// A percentage of an actor's own speed added to it wherever the turn
/// loop reads it: haste is a hundred, a chill minus twenty-five.
pub const SPEED: &str = "speed";
/// Percentage points off a watcher's chance of noticing the adventurer.
pub const STEALTH: &str = "stealth";
/// A percentage added to every heal its holder is given.
pub const MENDING: &str = "mending";
/// Tiles added to the reach of the adventurer's lantern.
pub const LIGHT: &str = "light";
/// Added to whatever armor a defender and its gear already have: what
/// gear too heavy to carry well takes away.
pub const ARMOR: &str = "armor";

/// The slots that go in pairs, by name: a thing that fits `either` of a
/// pair is worn in whichever is free, a one-handed blade in either hand
/// and a ring on either hand's finger.
pub const PAIRS: [(&str, &str); 2] = [("main hand", "off hand"), ("left finger", "right finger")];

/// Undermere's stats, each at the value an actor has with nothing on it.
///
/// One base for every actor, the adventurer's: a monster's strength is
/// never read, since only the adventurer pays for what is too heavy, and
/// every other stat starts where theirs does, accuracy at a hundred and
/// the rest at nought. Speed's base is nought, not a hundred, because the
/// turn loop adds it to an actor's own speed as a percentage of it: a base
/// of a hundred would have everyone move twice as fast. It is held between
/// a tenth of an actor's speed and four times it, so no stack of chills
/// stops a body dead.
pub fn stats() -> Registry<StatDef> {
    Registry::from_defs(vec![
        StatDef::new(STRENGTH, 12),
        StatDef::new(ACCURACY, 100),
        StatDef::new(EVASION, 0),
        StatDef::new(SPEED, 0).clamp(-90, 300),
        StatDef::new(STEALTH, 0),
        StatDef::new(MENDING, 0),
        StatDef::new(LIGHT, 0),
        StatDef::new(ARMOR, 0),
    ])
    .unwrap()
}

/// Every registry Undermere's content names, filled before anything is
/// loaded against them.
///
/// Assembled the way Foundry's is: one `Registry::from_defs` per table,
/// then the struct literal, the statuses after the kinds they deal and the
/// stats they change, and the props last, since a chest asks for what it
/// holds by tag. Panics naming `assets/props.ron` and every problem in
/// it, since a prop that does not load is a place that cannot be filled.
pub fn registries() -> Registries {
    // Armor turns a blow and nothing else. The elements go through plate,
    // since a tick of `poisoned` or `scorched` that armor could stop would
    // never tick at all, and a resistance is what answers them.
    let damage_kinds = Registry::from_defs(vec![
        DamageKind::new("slash"),
        DamageKind::new("pierce"),
        DamageKind::new("crush"),
        DamageKind::new("fire").unarmored(),
        DamageKind::new("cold").unarmored(),
        DamageKind::new("poison").unarmored(),
    ])
    .unwrap();
    let (fire, poison) = (damage_kinds.expect("fire"), damage_kinds.expect("poison"));
    let stats = stats();
    // `poisoned` is a point of poison a turn. `scorched` is what standing
    // in fire leaves, a point a turn for as long as the run's `FireRules`
    // says. `mending` is a potion's, two a turn. `regenerating` is what
    // the trait that names it holds for life, a point a turn, with no
    // badge, since it is what the monster is rather than something that
    // happened to it. The last two are boons, so the log tells them as
    // good news. `chilled` is the drowning grasp's, a quarter off its
    // holder's speed, which the turn loop reads through the speed stat.
    let statuses = Registry::from_defs(vec![
        StatusDef { badge: Some('p'), ..StatusDef::new("poisoned").ticks(poison, 1) },
        StatusDef { badge: Some('*'), ..StatusDef::new("scorched").ticks(fire, 1) },
        StatusDef { badge: Some('+'), ..StatusDef::new("mending").mends(2).boon() },
        StatusDef::new("regenerating").mends(1).boon(),
        StatusDef { badge: Some('c'), ..StatusDef::new("chilled").modifies(stats.expect(SPEED), Op::Add(-25)) },
    ])
    .unwrap();
    let mut registries = Registries {
        damage_kinds,
        stats,
        statuses,
        factions: Registry::from_defs(["adventurer", "beasts", "goblins", "undead"].map(FactionDef::new).to_vec()).unwrap(),
        tags: Registry::from_defs(["weapon", "armor", "potion", "scroll", "staff", "wand", "ring", "gold", "arrow", "light"].map(TagDef::new).to_vec())
            .unwrap(),
        slots: Registry::from_defs(["main hand", "off hand", "head", "body", "left finger", "right finger", "light"].map(SlotDef::new).to_vec()).unwrap(),
        ..Registries::default()
    };
    registries.props = rl_engine::rl_rules::prop::load(PROPS_RON, &registries.names()).unwrap_or_else(|e| panic!("assets/props.ron: {e}"));
    registries
}

/// Who fights whom: the adventurer against every side, beasts and goblins
/// against each other, and the dead against everything living, so a
/// crypt's dead and a goblin band fight and the dungeon fights itself.
///
/// And which stats a blow and a heal read: accuracy against evasion for
/// whether it lands, the armor stat beside whatever armor is worn, and
/// mending on every heal.
pub fn combat_rules(registries: &Registries) -> CombatRules {
    let [adventurer, beasts, goblins, undead] = ["adventurer", "beasts", "goblins", "undead"].map(|name| registries.factions.expect(name));
    let stat = |name: &str| registries.stats.expect(name);
    CombatRules::new(&registries.factions)
        .hostile(adventurer, beasts)
        .hostile(adventurer, goblins)
        .hostile(adventurer, undead)
        .hostile(beasts, goblins)
        .hostile(undead, beasts)
        .hostile(undead, goblins)
        .accuracy_stat(stat(ACCURACY))
        .evasion_stat(stat(EVASION))
        .armor_stat(stat(ARMOR))
        .mending_stat(stat(MENDING))
}

/// How fast everyone is: their own speed, with the speed stat added as a
/// percentage of it, so a haste or a chill is a status changing a stat.
pub fn turn_rules(registries: &Registries) -> TurnRules {
    TurnRules::new().speed_stat(registries.stats.expect(SPEED))
}

/// Whether a blow or a shot lands: the engine's percent roll, accuracy
/// less evasion, so a blow that meets nothing in the way lands, a shield
/// turns one in ten, and every point of strength a weapon wants and its
/// wielder lacks is four in a hundred more misses.
///
/// A shot loses ten a tile past its weapon's effective reach, and thirty
/// at a target in the dark, which only dark sight could see it in. Nothing
/// for a dim one: every floor is dim beyond the lantern, so a cost for dim
/// light would be a cost on nearly every shot, and true dark is rare.
pub fn hit_rules() -> HitRules {
    HitRules(Box::new(rl_engine::rl_rules::accuracy::Percent::new(10, 0, 30)))
}

/// The effect kinds an Undermere app has, for reading the content where no
/// app is running, as a test does: whatever `add_engine_effects` declares,
/// the `Ignite` that `FirePlugin` declares for the fire breath and the
/// fire flask, and Undermere's own, by [`add_game_effects`]. The throwaway
/// `App` is there so the list is the engine's own and never one kept
/// beside it.
pub fn effect_kinds() -> EffectKinds {
    let mut app = App::new();
    app.add_engine_effects().add_effect::<rl_engine::rl_bevy::Ignite>();
    add_game_effects(&mut app);
    std::mem::take(&mut app.world_mut().resource_mut::<EffectKinds>())
}

/// Declares the effects Undermere writes itself, the ones that reach what
/// the engine knows nothing of: a thing's level, what is known of it, a
/// curse, and the adventurer's strength. The one list, for the app and for
/// [`effect_kinds`] alike.
pub fn add_game_effects(app: &mut App) {
    app.add_effect::<crate::enchant::Enchanting>()
        .add_effect::<crate::enchant::RemoveCurse>()
        .add_effect::<crate::lore::Identify>()
        .add_effect::<crate::strength::Strengthen>();
}

/// Reads `abilities.ron` against `registries`, its effects built through
/// `kinds`. Panics naming the file and every problem in it, since a
/// broken ability is a game that cannot start.
pub fn abilities(registries: &Registries, kinds: &EffectKinds) -> Abilities {
    Abilities::load(ABILITIES_RON, kinds, &registries.names()).unwrap_or_else(|e| panic!("assets/abilities.ron: {e}"))
}

/// Everything Undermere's content files hold, read in the order their
/// names need.
///
/// The one place that order is written, so whatever wants the content
/// never has to know it: traits read before the abilities they grant
/// would find every grant an unknown name.
pub struct Library {
    /// The registries every file names.
    pub registries: Registries,
    /// The abilities, their effects built.
    pub abilities: Abilities,
    /// The traits monsters are written as and items lend.
    pub traits: Registry<TraitDef>,
    /// The items, and what each floor holds.
    pub armory: Armory,
    /// The monsters, and where each turns up.
    pub bestiary: Bestiary,
    /// The faces the things of a kind wear until the kind is known.
    pub faces: crate::lore::Faces,
}

impl Library {
    /// Reads every file, with `kinds` and `moments` the ones the app
    /// registered, so an effect or a moment nobody registered fails here.
    /// Panics naming the file at the first one that is broken.
    pub fn load(kinds: &EffectKinds, moments: &Moments) -> Self {
        let registries = registries();
        let abilities = abilities(&registries, kinds);
        let traits = crate::traits::load_defs(&registries, abilities.defs());
        let armory = Armory::load(&registries, &traits, kinds, moments);
        let bestiary = Bestiary::load(&registries, &traits, &armory.defs);
        let faces = crate::lore::Faces::load(&registries, &armory.defs);
        Self { registries, abilities, traits, armory, bestiary, faces }
    }
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_rules::Relation;

    use super::*;

    /// Every file in `assets/`, read the way the game reads it: if any
    /// name in any of them were unknown, or any entry malformed, loading
    /// would panic naming the file and the entry.
    #[test]
    fn every_content_file_loads_and_every_name_in_it_resolves() {
        let library = Library::load(&effect_kinds(), &Moments::default());
        assert_eq!(library.abilities.defs().len(), 2, "fire breath and drowning grasp");
        assert_eq!(library.traits.len(), 19, "the skeleton's nineteen traits");
        assert!(library.armory.defs.len() >= 18, "about twenty items: {}", library.armory.defs.len());
        assert!(library.bestiary.defs.len() >= 12, "a dozen monsters or so: {}", library.bestiary.defs.len());
        assert!(!library.armory.table.rows().is_empty(), "item_spawns.ron has rows");
        assert!(!library.bestiary.spawns.entries().is_empty(), "monster_spawns.ron has rows");
    }

    /// The registries hold every side, kind, status, slot and tag the
    /// files write, so a file naming one of them reads.
    #[test]
    fn the_registries_hold_every_side_kind_status_slot_and_tag_the_content_names() {
        let r = registries();
        for side in ["adventurer", "beasts", "goblins", "undead"] {
            assert!(r.factions.id(side).is_some(), "{side}");
        }
        for kind in ["slash", "pierce", "crush", "fire", "cold", "poison"] {
            assert!(r.damage_kinds.id(kind).is_some(), "{kind}");
        }
        for status in ["poisoned", "scorched", "mending", "regenerating", "chilled"] {
            assert!(r.statuses.id(status).is_some(), "{status}");
        }
        for slot in ["main hand", "off hand", "head", "body", "left finger", "right finger", "light"] {
            assert!(r.slots.id(slot).is_some(), "{slot}");
        }
        for tag in ["weapon", "armor", "potion", "scroll", "staff", "wand", "ring", "gold", "arrow", "light"] {
            assert!(r.tags.id(tag).is_some(), "{tag}");
        }
        for (one, other) in PAIRS {
            assert!(r.slots.id(one).is_some() && r.slots.id(other).is_some(), "{one} and {other} are slots");
        }
    }

    /// Every stat starts where the content sheet puts it, the adventurer's
    /// strength at twelve and accuracy at a hundred, and speed at nought,
    /// since the turn loop adds it to an actor's own: at a hundred,
    /// everyone would move twice as fast.
    #[test]
    fn every_stat_starts_where_the_sheet_says_and_speed_at_nought_so_nobody_moves_faster() {
        let r = registries();
        let base = |name: &str| r.stats.get(r.stats.expect(name)).base;
        let bases: Vec<(&str, i32)> = [STRENGTH, ACCURACY, EVASION, SPEED, STEALTH, MENDING, LIGHT, ARMOR].map(|s| (s, base(s))).to_vec();
        assert_eq!(bases, [(STRENGTH, 12), (ACCURACY, 100), (EVASION, 0), (SPEED, 0), (STEALTH, 0), (MENDING, 0), (LIGHT, 0), (ARMOR, 0)], "the sheet's bases");
        assert_eq!(r.stats.len(), 8, "a stat this test does not name");
        let speed = r.stats.get(r.stats.expect(SPEED));
        assert_eq!((speed.min, speed.max), (Some(-90), Some(300)), "no chill stops a body dead");
    }

    /// The run's rules name the stats they read: accuracy against evasion
    /// for a blow, the armor stat beside what is worn, mending on a heal,
    /// and speed in the turn loop.
    #[test]
    fn a_blow_a_heal_and_the_turn_loop_read_the_stats_that_change_them() {
        let r = registries();
        let stat = |name: &str| Some(r.stats.expect(name));
        let rules = combat_rules(&r);
        assert_eq!((rules.accuracy, rules.evasion, rules.armor, rules.mending), (stat(ACCURACY), stat(EVASION), stat(ARMOR), stat(MENDING)));
        assert_eq!(rules.attack, None, "a blow's roll is the weapon's own");
        assert_eq!(turn_rules(&r).speed, stat(SPEED));
    }

    /// A chill takes a quarter off its holder's speed, and the drowning
    /// grasp chills whatever it catches for four turns, as part 1's sheet
    /// first wanted before the turn loop read a speed from a stat.
    #[test]
    fn the_drowning_grasp_chills_what_it_catches_for_four_turns_and_a_chill_takes_a_quarter_off_its_speed() {
        let r = registries();
        let chilled = r.statuses.get(r.statuses.expect("chilled"));
        assert_eq!(chilled.modifiers.iter().map(|m| (m.stat, m.op)).collect::<Vec<_>>(), [(r.stats.expect(SPEED), Op::Add(-25))]);
        let library = Library::load(&effect_kinds(), &Moments::default());
        let grasp = library.abilities.expect("drowning grasp");
        let said = library.abilities.describe(grasp, &library.registries);
        assert!(said.iter().any(|s| s == "chilled for 4 turns"), "{said:?}");
    }

    /// A tripwire over what stands in the places, by name, so a prop is
    /// added or dropped on purpose. Everything but the chest is scenery,
    /// which the nearby rail leaves out; the chest holds something and is
    /// opened, and what blocks the way is what the places are laid round.
    #[test]
    fn every_prop_of_the_town_and_the_floors_is_in_the_file_and_only_the_chest_is_more_than_scenery() {
        let r = registries();
        let props = [
            ("lamp post", true),
            ("well", true),
            ("forge", true),
            ("anvil", true),
            ("shelf", true),
            ("barrel", true),
            ("table", true),
            ("counter", true),
            ("hearth", true),
            ("bed", true),
            ("altar", true),
            ("pew", true),
            ("candelabra", false),
            ("brazier", true),
            ("throne", true),
            ("chest", true),
        ];
        assert_eq!(r.props.len(), props.len(), "a prop this list does not name");
        for (name, blocks) in props {
            let def = r.props.get(r.props.id(name).unwrap_or_else(|| panic!("no prop called {name:?}")));
            assert_eq!(def.blocks, blocks, "{name}: whether it stands in the way");
            assert_eq!(def.is_scenery(), name != "chest", "{name}");
        }
        let chest = r.props.get(r.props.expect("chest"));
        assert!(chest.container.as_ref().is_some_and(|c| c.contents.len() == 3), "a weapon, armor and a potion, by tag");
    }

    /// Plate turns a blade and a club, and nothing else: a tick of poison
    /// or of standing in fire that armor could stop would never tick at
    /// all, so the elements are what a resistance answers and armor never
    /// does.
    #[test]
    fn armor_turns_blows_and_never_the_elements() {
        let r = registries();
        let armored = |name: &str| r.damage_kinds.get(r.damage_kinds.expect(name)).armored;
        for kind in ["slash", "pierce", "crush"] {
            assert!(armored(kind), "{kind} is a blow");
        }
        for kind in ["fire", "cold", "poison"] {
            assert!(!armored(kind), "{kind} goes through plate");
        }
    }

    /// The design's war: the adventurer against every side, beasts and
    /// goblins against each other, and the dead against everything
    /// living, so the dungeon fights itself.
    #[test]
    fn the_adventurer_fights_every_side_and_the_sides_fight_each_other_as_the_design_says() {
        let r = registries();
        let rules = combat_rules(&r);
        let at = |a: &str, b: &str| rules.factions.relation(r.factions.expect(a), r.factions.expect(b));
        for other in ["beasts", "goblins", "undead"] {
            assert_eq!(at("adventurer", other), Relation::Hostile, "the adventurer and the {other}");
            assert_eq!(at(other, "adventurer"), Relation::Hostile, "the {other} and the adventurer");
        }
        assert_eq!(at("beasts", "goblins"), Relation::Hostile);
        assert_eq!(at("undead", "beasts"), Relation::Hostile);
        assert_eq!(at("undead", "goblins"), Relation::Hostile);
        for own in ["beasts", "goblins", "undead"] {
            assert_eq!(at(own, own), Relation::Allied, "{own} stand together");
        }
    }

    /// A file's colour and flicker in `0..=1` become the engine's bytes,
    /// rounded, at the one brightness every light has.
    #[test]
    fn a_light_in_the_files_units_becomes_the_engines_light() {
        let glow = LightDef { radius: 4, color: (0.35, 1.0, 0.8), flicker: 0.3 }.source();
        assert_eq!((glow.intensity, glow.radius), (LIGHT_INTENSITY, 4));
        assert_eq!(glow.color, Rgb::new(89, 255, 204));
        assert_eq!(glow.flicker, 77);
    }
}
