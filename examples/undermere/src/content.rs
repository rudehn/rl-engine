//! What Undermere's world is made of: its sides, its kinds of harm, its
//! statuses, slots and tags, the props that stand in its places, the
//! abilities its monsters know, and the order every content file is read
//! in.
//!
//! Every name here is content the engine knows nothing about; the engine
//! sees ids. The registries are filled first, since every file names
//! them, and [`Library::load`] reads the files after them in the order
//! their names need: abilities, then the traits that grant them, then the
//! items that lend traits, then the monsters that are written as traits
//! and drop items.

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

/// Every registry Undermere's content names, filled before anything is
/// loaded against them.
///
/// Assembled the way Foundry's is: one `Registry::from_defs` per table,
/// then the struct literal, the statuses after the kinds they deal, and
/// the props last, since a chest asks for what it holds by tag. Panics
/// naming `assets/props.ron` and every problem in it, since a prop that
/// does not load is a place that cannot be filled.
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
    // `poisoned` is a point of poison a turn. `scorched` is what standing
    // in fire leaves, a point a turn for as long as the run's `FireRules`
    // says. `mending` is a potion's, two a turn. `regenerating` is what
    // the trait that names it holds for life, a point a turn, with no
    // badge, since it is what the monster is rather than something that
    // happened to it. The last two are boons, so the log tells them as
    // good news.
    //
    // No `chilled`: slowing a body needs a speed read from a stat, and the
    // engine's `Speed` is a plain number no status is folded into, so the
    // drowning grasp's cold is all it does.
    let statuses = Registry::from_defs(vec![
        StatusDef { badge: Some('p'), ..StatusDef::new("poisoned").ticks(poison, 1) },
        StatusDef { badge: Some('*'), ..StatusDef::new("scorched").ticks(fire, 1) },
        StatusDef { badge: Some('+'), ..StatusDef::new("mending").mends(2).boon() },
        StatusDef::new("regenerating").mends(1).boon(),
    ])
    .unwrap();
    let mut registries = Registries {
        damage_kinds,
        statuses,
        factions: Registry::from_defs(["adventurer", "beasts", "goblins", "undead"].map(FactionDef::new).to_vec()).unwrap(),
        tags: Registry::from_defs(["weapon", "armor", "potion", "gold", "arrow", "light"].map(TagDef::new).to_vec()).unwrap(),
        slots: Registry::from_defs(["main hand", "off hand", "head", "body", "light"].map(SlotDef::new).to_vec()).unwrap(),
        ..Registries::default()
    };
    registries.props = rl_engine::rl_rules::prop::load(PROPS_RON, &registries.names()).unwrap_or_else(|e| panic!("assets/props.ron: {e}"));
    registries
}

/// Who fights whom: the adventurer against every side, beasts and goblins
/// against each other, and the dead against everything living, so a
/// crypt's dead and a goblin band fight and the dungeon fights itself.
pub fn combat_rules(registries: &Registries) -> CombatRules {
    let [adventurer, beasts, goblins, undead] = ["adventurer", "beasts", "goblins", "undead"].map(|name| registries.factions.expect(name));
    CombatRules::new(&registries.factions)
        .hostile(adventurer, beasts)
        .hostile(adventurer, goblins)
        .hostile(adventurer, undead)
        .hostile(beasts, goblins)
        .hostile(undead, beasts)
        .hostile(undead, goblins)
}

/// The effect kinds an Undermere app has, for reading the content where no
/// app is running, as a test does: whatever `add_engine_effects` declares,
/// and the `Ignite` that `FirePlugin` declares for the fire breath and the
/// fire flask. The throwaway `App` is there so the list is the engine's
/// own and never one kept beside it.
pub fn effect_kinds() -> EffectKinds {
    let mut app = App::new();
    app.add_engine_effects().add_effect::<rl_engine::rl_bevy::Ignite>();
    std::mem::take(&mut app.world_mut().resource_mut::<EffectKinds>())
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
        Self { registries, abilities, traits, armory, bestiary }
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
        for status in ["poisoned", "scorched", "mending", "regenerating"] {
            assert!(r.statuses.id(status).is_some(), "{status}");
        }
        for slot in ["main hand", "off hand", "head", "body", "light"] {
            assert!(r.slots.id(slot).is_some(), "{slot}");
        }
        for tag in ["weapon", "armor", "potion", "gold", "arrow", "light"] {
            assert!(r.tags.id(tag).is_some(), "{tag}");
        }
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
