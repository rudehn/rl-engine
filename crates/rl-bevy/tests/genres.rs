//! Five genres of ability in one registry: the claim that abilities are
//! data, kept as a test now that no example game carries all five.
//!
//! A fantasy caster, a pirate, a marine, a man-at-arms and a thief, each a
//! RON file in `genres/` that differs from the others in nothing but its
//! words. They load through one loader into one table and build against one
//! set of effect kinds, so a fireball and a smoke bomb end up as two rows of
//! the same thing. The five effects no engine ships are reduced to their
//! names here, because what they do is a game's business; that each name is
//! found, and that a missing one is refused at build rather than at the
//! first key press, is the engine's.

use bevy::prelude::*;
use rl_bevy::ability::{Abilities, AddEffect, AddEngineEffects, Effect, EffectKinds, EffectWorld, FromArgs, Landing};
use rl_rules::ability::{Lookup, RawValue, load};
use rl_rules::damage::{DamageKind, DamageKindId};
use rl_rules::{AbilityDef, Registry, SlotDef, SlotId, StatDef, StatId, StatusDef, StatusId, TagDef, TagId};

const SETS: [(&str, &str); 5] = [
    ("fantasy", include_str!("genres/fantasy.ron")),
    ("pirates", include_str!("genres/pirates.ron")),
    ("scifi", include_str!("genres/scifi.ron")),
    ("medieval", include_str!("genres/medieval.ron")),
    ("crime", include_str!("genres/crime.ron")),
];

/// Every registry the five files name between them.
struct Content {
    stats: Registry<StatDef>,
    statuses: Registry<StatusDef>,
    tags: Registry<TagDef>,
    slots: Registry<SlotDef>,
    kinds: Registry<DamageKind>,
}

impl Lookup for Content {
    fn stat(&self, n: &str) -> Option<StatId> {
        self.stats.id(n)
    }
    fn status(&self, n: &str) -> Option<StatusId> {
        self.statuses.id(n)
    }
    fn tag(&self, n: &str) -> Option<TagId> {
        self.tags.id(n)
    }
    fn slot(&self, n: &str) -> Option<SlotId> {
        self.slots.id(n)
    }
    fn damage(&self, n: &str) -> Option<DamageKindId> {
        self.kinds.id(n)
    }
}

impl Content {
    fn new() -> Self {
        Self {
            // One pool per genre, and the engine cannot tell them apart.
            stats: Registry::from_defs(vec![StatDef::new("mana", 40), StatDef::new("power", 40), StatDef::new("stamina", 30), StatDef::new("nerve", 20)])
                .unwrap(),
            statuses: Registry::from_defs(
                ["scorched", "deafened", "stunned", "cloaked", "dazed", "inspired", "blinded", "bleeding", "spotted"].map(StatusDef::new).to_vec(),
            )
            .unwrap(),
            tags: Registry::from_defs(["powder", "rum", "cash", "shield"].map(TagDef::new).to_vec()).unwrap(),
            slots: Registry::from_defs(vec![SlotDef::new("main hand"), SlotDef::new("off hand")]).unwrap(),
            kinds: Registry::from_defs(vec![
                DamageKind::new("fire").unarmored(),
                DamageKind::new("shot"),
                DamageKind::new("shock").unarmored(),
                DamageKind::new("blunt"),
                DamageKind::new("pierce"),
                DamageKind::new("care").unarmored(),
            ])
            .unwrap(),
        }
    }

    /// All five files, in one table.
    fn every_set(&self) -> Registry<AbilityDef> {
        let mut all = Vec::new();
        for (name, text) in SETS {
            let loaded = load(text, self).unwrap_or_else(|e| panic!("genres/{name}.ron: {e}"));
            all.extend(loaded.iter().map(|(_, d)| d.clone()));
        }
        Registry::from_defs(all).expect("no two sets name the same ability")
    }
}

/// An effect a game would write, reduced to its name.
macro_rules! named_effect {
    ($name:ident) => {
        struct $name;
        impl Effect for $name {
            fn apply(&self, _: &Landing, _: &mut EffectWorld<'_, '_>) {}
        }
        impl FromArgs for $name {
            const KIND: &'static str = stringify!($name);
            fn from_args(_: &RawValue, _: &dyn Lookup) -> Result<Self, String> {
                Ok($name)
            }
        }
    };
}

named_effect!(Drain);
named_effect!(Plunder);
named_effect!(Hack);
named_effect!(Banner);
named_effect!(Smoke);

/// The effect kinds a game registers: the engine's seven, and the five
/// genre effects when `with_the_games_own` is set.
fn effect_kinds(with_the_games_own: bool) -> App {
    let mut app = App::new();
    app.add_engine_effects();
    if with_the_games_own {
        app.add_effect::<Drain>().add_effect::<Plunder>().add_effect::<Hack>().add_effect::<Banner>().add_effect::<Smoke>();
    }
    app
}

#[test]
fn five_genres_of_ability_load_into_one_table_and_build_against_one_set_of_effects() {
    let content = Content::new();
    let all = content.every_set();
    assert_eq!(all.len(), 18, "eighteen abilities across five genres");
    for name in ["fireball", "broadside", "overload", "shield bash", "smoke bomb"] {
        assert!(all.id(name).is_some(), "{name} is missing");
    }

    let app = effect_kinds(true);
    let built = Abilities::build(all, app.world().resource::<EffectKinds>(), &content).unwrap_or_else(|e| panic!("{e}"));
    // One table means one namespace: a fireball and a broadside are rows of
    // the same thing, addressed the same way.
    assert_ne!(built.expect("fireball"), built.expect("broadside"));
}

#[test]
fn an_effect_no_game_registered_is_refused_at_build_by_name() {
    let content = Content::new();
    let app = effect_kinds(false);
    let Err(e) = Abilities::build(content.every_set(), app.world().resource::<EffectKinds>(), &content) else {
        panic!("built with none of the genres' own effects registered");
    };
    let why = e.to_string();
    assert!(why.contains("Drain") && why.contains("Smoke"), "names every missing effect: {why}");
}
