//! What walks the decks: line droids, probe droids, heavy droids and
//! coolant rats, loaded from `monsters.ron` and spawned with a brain that
//! shoots what it holds a weapon for, once it is in reach, before ever
//! closing to a punch.
//!
//! [`alarm`] holds a probe's radar reporting to the rest of the deck.
//! [`sensors`] holds what an ion hit does to that same radar, and owns
//! `DarkSight` as derived state: [`sensors::sync_dark_sight`] is the one
//! system that ever sets or clears it, from a monster's own
//! [`NativeDarkSight`] and whatever it wears, folded together with whether
//! it is currently [`sensors::Jammed`]. [`spawns`] holds populating a deck
//! the first time it is entered. Kept out of this file so it stays about
//! the roster and the spawn, which is already most of a file's worth on
//! its own.

mod alarm;
mod sensors;
mod spawns;

use std::sync::Arc;

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_rules::ai::tactics::{FleeWhenHurt, Hunt, MeleeAdjacent, SearchLastKnown, ShootAtRange, Wander};
use rl_engine::rl_rules::faction::FactionDef;
use serde::Deserialize;

pub use alarm::{Alarm, sound_alarm};
pub use sensors::{Jammed, jam_sensors, sync_dark_sight, unjam_sensors};
pub use spawns::populate_deck;

use crate::content::{Profile, resistances};

/// The roster file, compiled in so the binary runs from anywhere.
const MONSTERS_RON: &str = include_str!("../assets/monsters.ron");

/// One kind of monster, as authored in `monsters.ron`.
#[derive(Debug, Clone, Deserialize)]
pub struct MonsterDef {
    /// Unique; the spawn table and drops refer to it.
    pub name: String,
    /// One character.
    pub glyph: char,
    /// `(r, g, b)` in `0..=1`.
    pub color: (f32, f32, f32),
    /// Maximum health.
    pub hp: i32,
    /// Flat armor.
    pub armor: i32,
    /// How it takes a hit: chassis or organic.
    pub profile: Profile,
    /// Droids or vermin.
    pub faction: NameRef<FactionDef>,
    /// What its brain is allowed to manage, on top of what it does
    /// unconditionally.
    pub wits: Wits,
    /// How far it sees, in tiles.
    pub perception: i32,
    /// Tiles it sees without light: radar, for the droids that have it.
    #[serde(default)]
    pub dark_sight: Option<i32>,
    /// The roll and damage kind a blow deals.
    pub melee: (DiceRoll, NameRef<DamageKind>),
    /// The range, roll and damage kind a shot deals; present, it shoots.
    #[serde(default)]
    pub ranged: Option<(i32, DiceRoll, NameRef<DamageKind>)>,
    /// Percent of a normal step's time one of its own takes; 100 is normal.
    pub speed: u32,
    /// Percent health at or below which it runs; zero never flees.
    pub flee_at: i32,
    /// True when noticing an enemy wakes every droid on the deck.
    #[serde(default)]
    pub alarm: bool,
    /// One or more `(min deck, max deck, weight, min group, max group)`
    /// rows; several let a group grow with depth.
    pub spawn: Vec<(i32, i32, u32, u32, u32)>,
    /// `(item name, percent chance)`, each rolled on its own death.
    #[serde(default)]
    pub drops: Vec<(String, u32)>,
}

impl Named for MonsterDef {
    fn name(&self) -> &str {
        &self.name
    }
}

/// A monster's own radar, from `monsters.ron`'s `dark_sight`: the probe's
/// is 4, and nothing else in this roster names one.
///
/// Never read as `DarkSight` itself and never removed once given:
/// [`sensors::sync_dark_sight`] is the one place that folds it together
/// with whatever an actor wears and whether it is jammed into the
/// `DarkSight` everything else reads, so a monster's own radar and a
/// helmet's sensor suite can never fight over which one last wrote the
/// component.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeDarkSight(i32);

/// Marks a monster with the definition it was spawned from, for the drops
/// a later task rolls on its death.
#[derive(Component, Clone, Copy)]
pub struct Kind(pub Id<MonsterDef>);

/// The monster definitions, the deck-banded table they spawn from, and the
/// brain each kind decides with.
#[derive(Resource)]
pub struct Roster {
    /// The monster definitions, by id.
    pub defs: Registry<MonsterDef>,
    /// What can spawn on which deck, in what group sizes.
    pub table: BandedTable<Id<MonsterDef>>,
    /// One brain per definition, indexed the same way `defs` is.
    brains: Vec<Arc<Brain<Entity>>>,
}

impl Roster {
    /// Loads `monsters.ron` against `registries`. Panics with every
    /// problem the file has, since a broken roster is a game that cannot
    /// start.
    pub fn load(registries: &Registries) -> Self {
        Self::from_ron(MONSTERS_RON, registries)
    }

    /// As [`load`](Self::load), but from `ron` rather than the compiled-in
    /// roster: `pub(crate)` for a test's own tiny roster, such as one
    /// monster with a guaranteed drop `load` alone could never build.
    /// Builds the spawn table and gives every kind a brain built from what
    /// it names: a shot in reach before a melee kind will ever reach for
    /// it does not apply, since [`MeleeAdjacent`] only ever fires on an
    /// adjacent enemy and so never competes with [`ShootAtRange`] for the
    /// same one; a mind that flees does so only if it names `flee_at`
    /// above zero.
    pub(crate) fn from_ron(ron: &str, registries: &Registries) -> Self {
        let defs: Registry<MonsterDef> = registries.names().load(ron).unwrap_or_else(|e| panic!("monster roster: {e}"));
        let mut table = BandedTable::default();
        let mut brains = Vec::new();
        for (id, d) in defs.iter() {
            for &(lo, hi, weight, gmin, gmax) in &d.spawn {
                table.push(BandedEntry::new(id).bands(lo, hi).weight(weight).group(gmin, gmax));
            }
            let mut brain = Brain::new().then(MeleeAdjacent);
            if d.ranged.is_some() {
                brain = brain.then(ShootAtRange::default());
            }
            if d.flee_at > 0 {
                brain = brain.then(FleeWhenHurt { at_pct: d.flee_at });
            }
            brains.push(Arc::new(brain.then(Hunt).then(SearchLastKnown).then(Wander { chance_pct: 30 })));
        }
        Self { defs, table, brains }
    }
}

/// Spawns `id` on `map` at `at`: an actor with health, armor, resistances,
/// perception, a mind and the notice every monster carries so a probe's
/// alarm and an ion hit both have something to act on. `RangedAttack` and
/// `Alarm` are added only for a kind that names them; a kind naming
/// `dark_sight` gets [`NativeDarkSight`] rather than `DarkSight` itself,
/// which [`sensors::sync_dark_sight`] sets from it the moment this pass's
/// `Turn` schedule runs.
pub fn spawn_monster(commands: &mut Commands, roster: &Roster, id: Id<MonsterDef>, at: Point, map: MapId, registries: &Registries) -> Entity {
    let d = roster.defs.get(id);
    let (dice, kind) = d.melee;
    let mut e = commands.spawn((
        (Actor, Blocks, Position(at), OnMap(map)),
        (Health::full(d.hp), Armor(d.armor), Faction(d.faction.id()), Resists(resistances(d.profile, registries))),
        (Perception(d.perception), Speed(d.speed), Mind(roster.brains[id.index()].clone()), Intelligence(d.wits)),
        (Notice(NoticeStats::default()), MeleeAttack { kind: kind.id(), dice, cost: None }, Kind(id)),
        (Name::new(d.name.clone()), Glyph::new(d.glyph, Color::srgb(d.color.0, d.color.1, d.color.2)).on_layer(5)),
    ));
    if let Some((range, dice, kind)) = d.ranged {
        e.insert(RangedAttack { kind: kind.id(), dice, range, cost: None });
    }
    if let Some(n) = d.dark_sight {
        e.insert(NativeDarkSight(n));
    }
    if d.alarm {
        e.insert(Alarm);
    }
    e.id()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use rand::SeedableRng;
    use rl_engine::rl_core::RunSeed;

    use super::*;

    #[test]
    fn every_deck_in_the_slice_has_something_to_spawn() {
        let roster = Roster::load(&crate::content::registries());
        assert!(roster.table.gaps(1..=3).is_empty());
    }

    #[test]
    fn line_droids_come_alone_or_in_pairs_on_deck_one_and_never_more() {
        // Spec section 8.5: lone sentries first, bigger groups deeper.
        let roster = Roster::load(&crate::content::registries());
        let line = roster.defs.expect("line droid");
        let mut rng = rand::rngs::StdRng::seed_from_u64(9);
        let sizes: BTreeSet<u32> = (0..2000).filter_map(|_| roster.table.pick_group(1, &mut rng)).filter(|(id, _)| **id == line).map(|(_, n)| n).collect();
        assert_eq!(sizes, [1, 2].into_iter().collect());
    }

    #[test]
    fn a_droid_with_a_blaster_shoots_rather_than_walking_up_to_punch() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (droid, player) = crate::testing::droid_facing_player(&mut app, "line droid", 4);
        let struck = crate::testing::run_until_struck(&mut app, droid, 10);
        assert!(struck.ranged, "four tiles off with a clear line: it shoots");
        assert_eq!(struck.target, player);
    }

    #[test]
    fn a_probe_that_notices_the_player_wakes_every_droid_on_its_deck() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (probe, player, others) = crate::testing::probe_and_sleepers(&mut app);
        app.world_mut().write_message(Noticed { observer: probe, subject: player, at: Point::ZERO });
        app.update();
        for d in others {
            assert!(app.world().get::<Aware>(d).is_some_and(|a| a.of(player).is_alert()), "a sleeper slept through the alarm");
        }
    }

    #[test]
    fn an_ion_hit_blinds_a_probes_radar_for_three_turns_and_a_slug_does_not() {
        let mut app = crate::testing::headless(RunSeed(1));
        let probe = crate::testing::lone_monster(&mut app, "probe droid");
        crate::testing::hit(&mut app, probe, "kinetic", 1);
        assert_eq!(app.world().get::<DarkSight>(probe).map(|d| d.0), Some(4), "a slug does nothing to its sensors");
        crate::testing::hit(&mut app, probe, "ion", 1);
        assert_eq!(app.world().get::<DarkSight>(probe), None, "blinded");
        crate::testing::pass_turns(&mut app, 3);
        assert_eq!(app.world().get::<DarkSight>(probe).map(|d| d.0), Some(4), "and back after three turns");
    }
}
