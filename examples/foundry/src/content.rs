//! What Foundry's world is made of: its damage kinds, slots, sides and
//! the two ways a body takes a hit.
//!
//! Every name here is content the engine knows nothing about; the engine
//! sees ids. The two resistance profiles are the design's damage table,
//! kept in one place so a droid and a commando cannot drift from it.

use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_rules::faction::FactionDef;
use rl_engine::rl_rules::gas::GasDef;
use rl_engine::rl_rules::{DamageKind, Registry, Resistances, SlotDef, StatusDef, TagDef};

/// What one turn of `mending` gives back.
///
/// The medkit's half of the medical pair: a stim closes a wound now, and
/// a medkit gives twice as much at this a turn, which the commando has to
/// stay alive to collect. Here rather than in `statuses.ron`, which
/// Foundry has none of, and read by `assets/abilities.ron` only through
/// how long it inflicts the status for, which
/// `a_medkit_is_worth_twice_a_stim_spread_over_ten_turns` holds to.
pub const MEND_PER_TURN: i32 = 2;

/// How a body takes a hit.
///
/// `Deserialize` by hand, from a plain string, so a monster's file writes
/// `profile: "chassis" | "organic"` the way every other name in content is
/// written; the derived enum deserializer RON gives a fieldless enum reads
/// a bare, unquoted identifier instead, which is not this file's style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// Plated: shrugs off half a slug, takes a bolt in full, and is undone by ion.
    Chassis,
    /// Flesh in plate: a bolt is blunted, ion barely registers.
    Organic,
}

impl<'de> serde::Deserialize<'de> for Profile {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ProfileVisitor;
        impl serde::de::Visitor<'_> for ProfileVisitor {
            type Value = Profile;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "\"chassis\" or \"organic\"")
            }

            fn visit_str<E: serde::de::Error>(self, name: &str) -> Result<Profile, E> {
                match name {
                    "chassis" => Ok(Profile::Chassis),
                    "organic" => Ok(Profile::Organic),
                    other => Err(E::custom(format!("no profile called {other:?}; expected \"chassis\" or \"organic\""))),
                }
            }
        }
        deserializer.deserialize_str(ProfileVisitor)
    }
}

/// The resistance table for `profile`, as percentages removed.
pub fn resistances(profile: Profile, registries: &Registries) -> Resistances {
    let k = &registries.damage_kinds;
    let (kinetic, energy, ion, electricity) = (k.expect("kinetic"), k.expect("energy"), k.expect("ion"), k.expect("electricity"));
    let mut r = Resistances::new();
    let (a, b, c, d) = match profile {
        Profile::Chassis => (50, 0, -100, 0),
        Profile::Organic => (0, 25, 75, 0),
    };
    r.set(kinetic, a);
    r.set(energy, b);
    r.set(ion, c);
    r.set(electricity, d);
    r
}

/// Every registry Foundry's content names, filled before anything is
/// loaded against them.
///
/// Assembled as one `Registry::from_defs` per table, then the struct
/// literal.
pub fn registries() -> Registries {
    let damage_kinds = Registry::from_defs(vec![
        DamageKind::new("kinetic"),
        DamageKind::new("energy"),
        // Plate does not stop a charge.
        DamageKind::new("ion").unarmored(),
        // Raw current: a live cable, a capacitor let go. Plate conducts
        // it, so only a resist takes any off, and it is not ion: flesh and
        // chassis take it alike, and it jams nothing.
        DamageKind::new("electricity").unarmored(),
        // Fire, and what an incendiary burns with. Unarmored for the reason
        // electricity is: plate heats through, and a tick of `scorched`
        // that armor could stop would never tick at all.
        DamageKind::new("thermal").unarmored(),
    ])
    .unwrap();
    // Mending is what a medkit's gel does: two a turn, as a mend and not
    // as damage of some kind, so plate and resistances have nothing to say
    // about it. `Stacking::Refresh` is the default and is the rule that
    // matters here: a second medkit puts the clock back to ten turns
    // rather than mending four a turn, so a pack of them is a longer
    // recovery and never a faster one.
    // Scorched is what standing in fire leaves on whoever stood there, a
    // point a turn for as long as `run::start`'s `FireRules` says. Cloaked,
    // last, ticks nothing: it hides whoever holds it and does no more.
    // Mending and cloaked are boons, so the log tells them as good news.
    // ANCHOR: statuses
    let thermal = damage_kinds.expect("thermal");
    let statuses = Registry::from_defs(vec![
        StatusDef { badge: Some('~'), ..StatusDef::new("sensors down") },
        StatusDef { badge: Some('+'), ..StatusDef::new("mending").mends(MEND_PER_TURN).boon() },
        StatusDef { badge: Some('^'), ..StatusDef::new("scorched").ticks(thermal, 1) },
        // What a cloak plate's charge puts on its wearer: nothing sees them
        // until it runs out or they strike.
        StatusDef { badge: Some('%'), ..StatusDef::new("cloaked").unseen().boon() },
    ])
    .unwrap();
    // ANCHOR_END: statuses
    // Smoke is the one gas: what a smoke grenade throws and what anything
    // burning gives off, thick enough to hide in while it hangs.
    let gases = Registry::from_defs(vec![GasDef::new("smoke").spread(55).fade(9).veils_at(70)]).unwrap();
    let mut registries = Registries {
        damage_kinds,
        statuses,
        gases,
        factions: Registry::from_defs(vec![FactionDef::new("commando"), FactionDef::new("droids"), FactionDef::new("vermin")]).unwrap(),
        tags: Registry::from_defs(vec![TagDef::new("weapon"), TagDef::new("armor"), TagDef::new("slug"), TagDef::new("med"), TagDef::new("keycard")]).unwrap(),
        slots: Registry::from_defs(vec![
            SlotDef::new("main hand").paired_with("off hand"),
            SlotDef::new("off hand"),
            SlotDef::new("head"),
            SlotDef::new("torso"),
            SlotDef::new("arms"),
            SlotDef::new("legs"),
        ])
        .unwrap(),
        ..Registries::default()
    };
    // Props name the tags above, so they are loaded once those exist, the
    // way the statuses and the armory are.
    registries.props = crate::props::load(&registries);
    registries
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every quoted string in Foundry's own files and code but the tests,
    /// the pieces, and `read`, the files the check reads for itself: what a
    /// name is looked for in when no table or piece reaches it, so a kind a
    /// system stands by name, a wreck or a console, is not taken for one
    /// nothing names, and a row of a table on no deck, or a slot of a piece
    /// no deck lays, is not taken for one something does.
    fn quoted_elsewhere(read: &[&str]) -> std::collections::BTreeSet<String> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut files: Vec<std::path::PathBuf> = Vec::new();
        let mut dirs = vec![root.join("assets"), root.join("src")];
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(&dir).expect("a directory of the game's own").flatten() {
                let path = entry.path();
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
                if path.is_dir() && name != "testing" && name != "prefabs" {
                    dirs.push(path);
                } else if path.extension().is_some_and(|e| e == "ron" || e == "rs") && name != "tests.rs" && !read.contains(&name.as_str()) {
                    files.push(path);
                }
            }
        }
        let mut quoted = std::collections::BTreeSet::new();
        for path in files {
            let text = std::fs::read_to_string(&path).expect("readable");
            // A file's own tests name content to test it, not to lay it:
            // the module of them inline, never a `mod tests;` naming a
            // file of its own, which the walk above leaves out by name.
            let text = text.split("#[cfg(test)]\nmod tests {").next().unwrap_or_default();
            let mut parts = text.split('"');
            parts.next();
            while let Some(inside) = parts.next() {
                quoted.insert(inside.to_string());
                parts.next();
            }
        }
        quoted
    }

    /// Nothing in the content is orphaned: every monster, item and prop is
    /// reached by something a run draws, on a deck that something can
    /// actually draw it on, read from the files with no deck grown. A
    /// monster by a row of the spawn table on a deck of the ten, a piece a
    /// deck lays, or a role such a piece draws for; an item by a row of the
    /// item table on a deck of the ten, which a deck's scatter draws from
    /// whole, or one a container, a slot or a drop names; a prop by a piece
    /// a deck lays. What none of those reaches is looked for, by its quoted
    /// name, in every other file and line of code but the tests, for a
    /// system that stands it itself; what is named nowhere at all is content
    /// no run ever meets.
    #[test]
    fn nothing_in_the_content_is_orphaned() {
        use rl_engine::rl_rules::prefab::Reached;
        let registries = registries();
        let roster = crate::droids::Roster::load(&registries);
        let armory = crate::testing::armory(&registries);
        let foundry = crate::decks::Foundry::new(rl_engine::rl_core::RunSeed(0));
        let prefabs = foundry.prefabs();
        let laid = crate::decks::laid_pieces();
        let reached: Reached<crate::droids::MonsterDef> =
            Reached::by(laid.iter().filter_map(|p| prefabs.defs().id(p)).map(|id| prefabs.defs().get(id)), &registries.props);
        let decks = 1..=crate::decks::DECKS as i32;
        let on_a_deck = |(lo, hi): (i32, i32)| lo <= *decks.end() && hi >= *decks.start();

        let mut monsters: std::collections::BTreeSet<String> = roster
            .table
            .entries()
            .iter()
            .filter(|r| r.weight > 0 && on_a_deck((r.min_band, r.max_band)))
            .map(|r| roster.defs.get(r.item).name.clone())
            .collect();
        monsters.extend(reached.monsters.iter().map(|id| roster.defs.get(*id).name.clone()));
        monsters.extend(reached.roles.iter().flat_map(|r| prefabs.roles().get(*r).members.iter().map(|m| roster.defs.get(*m).name.clone())));

        let mut items: std::collections::BTreeSet<String> =
            armory.table.rows().iter().filter(|r| r.weight > 0 && on_a_deck(r.bands)).map(|r| armory.defs.get(r.item).name.clone()).collect();
        items.extend(reached.items.iter().cloned());
        for (_, def) in roster.defs.iter().filter(|(_, d)| monsters.contains(&d.name)) {
            items.extend(def.drops.iter().map(|(item, _)| armory.defs.get(item.id()).name.clone()));
        }

        let props: std::collections::BTreeSet<String> = reached.props.iter().map(|id| registries.props.get(*id).name.clone()).collect();

        let read = ["monsters.ron", "monster_spawns.ron", "roles.ron", "items.ron", "item_spawns.ron", "props.ron"];
        let quoted = quoted_elsewhere(&read);
        let mut orphans = Vec::new();
        for (unreached, what) in [
            (roster.defs.iter().map(|(_, d)| d.name.clone()).filter(|n| !monsters.contains(n)).collect::<Vec<_>>(), "monster"),
            (armory.defs.iter().map(|(_, d)| d.name.clone()).filter(|n| !items.contains(n)).collect(), "item"),
            (registries.props.iter().map(|(_, d)| d.name.clone()).filter(|n| !props.contains(n)).collect(), "prop"),
        ] {
            orphans.extend(unreached.into_iter().filter(|n| !quoted.contains(n)).map(|n| format!("{what} {n:?}")));
        }
        let pieces: Vec<&str> = prefabs.defs().iter().map(|(_, d)| d.name.as_str()).filter(|n| !laid.contains(n)).collect();
        orphans.extend(pieces.into_iter().map(|n| format!("piece {n:?}")));
        assert!(orphans.is_empty(), "reached by nothing a run draws and named nowhere else: {orphans:#?}");
    }

    #[test]
    fn the_damage_table_is_the_designs_to_the_percent() {
        // Spec section 5: a droid shrugs off half of a slug and takes double
        // from ion; flesh takes a quarter less from a bolt and nearly nothing
        // from ion; electricity hurts both alike, which is what keeps it a
        // different idea from ion. A profile that drifts from this
        // rebalances every fight.
        let r = registries();
        let k = &r.damage_kinds;
        let (kinetic, energy, ion, electricity) = (k.expect("kinetic"), k.expect("energy"), k.expect("ion"), k.expect("electricity"));
        let table = |p| {
            let t = resistances(p, &r);
            (t.get(kinetic), t.get(energy), t.get(ion), t.get(electricity))
        };
        assert_eq!(table(Profile::Chassis), (50, 0, -100, 0));
        assert_eq!(table(Profile::Organic), (0, 25, 75, 0));
    }

    /// The design's table of what each attack flies as, to the byte: a
    /// blaster's bolt, an ion pistol's charge, a slug. Walks every item
    /// and every monster in the files, so a new gun without a row here, or
    /// a blade or a bite given a look, fails it.
    #[test]
    fn every_attack_in_the_files_flies_the_look_the_design_gives_it_and_no_blow_bursts() {
        use bevy::ecs::world::CommandQueue;
        use bevy::prelude::*;
        use rl_engine::rl_core::{Point, RunSeed};
        use rl_engine::rl_grid::Rgb;
        let bolt = Some(('*', Rgb::new(255, 77, 38)));
        let slug = Some(('o', Rgb::new(242, 204, 115)));
        let table = [
            ("hand blaster", bolt),
            ("blaster carbine", bolt),
            ("ion pistol", Some(('~', Rgb::new(89, 166, 255)))),
            ("slug pistol", slug),
            ("slug rifle", slug),
            ("heavy repeater", bolt),
            ("line droid", None),
            ("probe droid", None),
            ("trooper droid", bolt),
            ("heavy droid", bolt),
        ];
        let flies = |name: &str| table.iter().find(|(n, _)| *n == name).and_then(|(_, look)| *look);
        let mut app = crate::testing::headless(RunSeed(1));
        let registries = app.world().resource::<Registries>().clone();
        let armory = crate::testing::armory_of(&app);
        let roster = crate::droids::Roster::load(&registries);
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, app.world_mut());
        let mut spawned: Vec<(String, Entity)> =
            armory.defs.iter().map(|(id, d)| (d.name.clone(), crate::gear::spawn_item(&mut commands, &armory, id))).collect();
        let map = MapId::ZERO;
        spawned.extend(
            roster.defs.iter().map(|(id, d)| (d.name.clone(), crate::droids::spawn_monster(&mut commands, &roster, id, Point::ZERO, map, &registries))),
        );
        queue.apply(app.world_mut());
        for (name, e) in spawned {
            let fired = app.world().get::<RangedAttack>(e).and_then(|r| r.look).map(|l| (l.glyph, l.color));
            assert_eq!(fired, flies(&name), "{name}");
            assert_eq!(app.world().get::<MeleeAttack>(e).and_then(|m| m.look), None, "{name} bursts on nothing yet");
        }
    }

    /// Every gun's reach and the edge of its no-penalty range are the
    /// design's: short guns reach further than they aim well, and a long
    /// gun aims well most of the way.
    #[test]
    fn every_weapon_reaches_and_aims_as_far_as_the_design_says() {
        use bevy::ecs::world::CommandQueue;
        use bevy::prelude::*;
        use rl_engine::rl_core::RunSeed;
        let table =
            [("hand blaster", 3, 8), ("ion pistol", 3, 8), ("slug pistol", 3, 9), ("heavy repeater", 4, 10), ("blaster carbine", 6, 12), ("slug rifle", 7, 14)];
        let mut app = crate::testing::headless(RunSeed(1));
        let armory = crate::testing::armory_of(&app);
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, app.world_mut());
        let spawned: Vec<(String, Entity)> = armory.defs.iter().map(|(id, d)| (d.name.clone(), crate::gear::spawn_item(&mut commands, &armory, id))).collect();
        queue.apply(app.world_mut());
        let found = |name: &str| spawned.iter().find(|(n, _)| n == name).map(|(_, e)| *e).unwrap_or_else(|| panic!("{name} is in the armory"));
        for (name, effective, reach) in table {
            let gun = app.world().get::<RangedAttack>(found(name)).unwrap_or_else(|| panic!("{name} shoots"));
            assert_eq!((gun.effective_range(), gun.range), (effective, reach), "{name}");
        }
        let blade = app.world().get::<Throwable>(found("monoblade")).expect("a monoblade can be thrown");
        assert_eq!((blade.effective_range(), blade.range), (2, 5), "the monoblade thrown");
    }

    #[test]
    fn every_slot_the_armor_names_is_registered() {
        let r = registries();
        for slot in ["main hand", "off hand", "head", "torso", "arms", "legs"] {
            assert!(r.slots.id(slot).is_some(), "{slot}");
        }
    }
}
