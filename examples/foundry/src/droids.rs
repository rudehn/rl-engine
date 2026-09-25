//! What walks the decks: line, probe, trooper and heavy droids, and the
//! critters, coolant rats, scrap crabs and lamp moths, loaded from
//! `monsters.ron`, with where each turns up from `monster_spawns.ron`. A
//! kind that names a shot is built with its own `RangedAttack`, rather than
//! handed a weapon to hold, and spawned with a brain that fires it at
//! anything in reach before ever closing to a punch.
//!
//! `alarm` holds a probe's radar reporting to the rest of the deck.
//! `sensors` holds what an ion hit does to that same radar, and owns
//! `DarkSight` as derived state: [`sensors::sync_dark_sight`] is the one
//! system that ever sets or clears it, from a monster's own
//! [`NativeDarkSight`] and whatever it wears, folded together with whether
//! it is currently [`sensors::Jammed`]. `spawns` holds populating a deck
//! the first time it is entered. Kept out of this file so it stays about
//! the roster and the spawn, which is already most of a file's worth on
//! its own.

mod alarm;
mod sensors;
mod spawns;

use std::sync::Arc;

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_rules::ai::hearing::HearingStats;
use rl_engine::rl_rules::ai::tactics::{FleeWhenHurt, Hover, Hunt, Keep, KeepPost, MeleeAdjacent, SearchLastKnown, ShootAtRange, Wander};
use rl_engine::rl_rules::faction::FactionDef;
use rl_engine::rl_rules::{DropRow, DropTable};
use serde::Deserialize;

pub use alarm::{ALARM_LOUDNESS, ALARM_SOUND, Alarm, NOISE, PULSE, shout_alarm, sound_alarm};
pub use sensors::{Jammed, jam_sensors, sync_dark_sight, unjam_sensors};
pub(crate) use spawns::MIN_DISTANCE_FROM_ENTRY;
pub use spawns::populate_deck;

use crate::content::{MeleeDef, Profile, RangedDef, resistances};

/// The roster file, compiled in so the binary runs from anywhere.
const MONSTERS_RON: &str = include_str!("../assets/monsters.ron");
/// Where each kind turns up, compiled in beside it.
const MONSTER_SPAWNS_RON: &str = include_str!("../assets/monster_spawns.ron");

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
    /// The roll and damage kind a blow deals; absent, it never strikes.
    #[serde(default)]
    pub melee: Option<MeleeDef>,
    /// The range, roll and damage kind a shot deals; present, it shoots.
    #[serde(default)]
    pub ranged: Option<RangedDef>,
    /// Its `Speed`, a percentage of normal: 100 is normal, 200 twice as
    /// fast, and 50 half as fast.
    pub speed: u32,
    /// Percent health at or below which it runs; zero never flees.
    pub flee_at: i32,
    /// How it notices a subject that can go unnoticed, the commando among
    /// them; absent, the engine's default, which is sure only of what is
    /// adjacent. A sentry names its own, since a droid that hesitates at a
    /// lit commando in blaster range is not a sentry.
    #[serde(default)]
    pub notice: Option<NoticeStats>,
    /// True when it sounds the deck's alarm for as long as it knows where
    /// an enemy is.
    #[serde(default)]
    pub alarm: bool,
    /// How far from the enemies in sight it keeps; present, it hangs at
    /// that distance rather than closing in.
    #[serde(default)]
    pub shadow: Option<ShadowDef>,
    /// How it hears; absent, it is deaf, and neither the alarm nor a
    /// firefight draws it.
    #[serde(default)]
    pub hearing: Option<HearingStats>,
    /// `(item name, percent chance)`, each rolled on its own death.
    #[serde(default)]
    pub drops: Vec<(String, u32)>,
}

impl Named for MonsterDef {
    fn name(&self) -> &str {
        &self.name
    }
}

/// One row of `monster_spawns.ron`: a kind, the decks it turns up on, how
/// often, and how many at once.
///
/// Its own file rather than a field of [`MonsterDef`], so what a kind is and
/// where it turns up are read and tuned apart, and a kind can have a row per
/// band without its definition growing a list of tuples.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct MonsterSpawn {
    /// Which kind.
    pub monster: NameRef<MonsterDef>,
    /// The first and last deck it applies on, both included.
    pub decks: (i32, i32),
    /// How often, against every other row that applies on a deck.
    pub weight: u32,
    /// The fewest and the most that come together.
    pub group: (u32, u32),
}

/// The distance a kind keeps from what it has in sight, as `monsters.ron`
/// writes it: the engine's [`Keep::enemies`] by its two fields.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ShadowDef {
    /// How far it lets the nearest enemy get before it closes.
    pub keep_within: i32,
    /// How near it lets the nearest enemy come before it backs off.
    pub no_closer_than: i32,
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
    /// What each kind leaves when it dies, its names resolved against
    /// `items.ron` once, indexed the same way `defs` is.
    drops: Vec<DropTable<Id<crate::gear::ItemDef>>>,
}

impl Roster {
    /// Loads `monsters.ron` and `monster_spawns.ron` against `registries`.
    /// Panics with every problem either file has, since a broken roster is a
    /// game that cannot start.
    pub fn load(registries: &Registries) -> Self {
        Self::from_ron(MONSTERS_RON, MONSTER_SPAWNS_RON, registries)
    }

    /// As [`load`](Self::load), but from `ron` and `spawns` rather than the
    /// compiled-in files, so a test can load a tiny roster of its own, such
    /// as one monster with a guaranteed drop.
    ///
    /// Builds the spawn table, and gives every kind a brain from what it
    /// names. [`MeleeAdjacent`] comes first for a kind with a blow, then
    /// [`ShootAtRange`] for a kind with a shot: the one fires only on an
    /// adjacent enemy and the other only on one two or more tiles off, so
    /// they never compete for the same target. A kind flees only if it
    /// names `flee_at` above zero. A kind that names `shadow` keeps its
    /// distance with [`Shadow`] and hangs there with [`Hover`] in place of
    /// [`Hunt`], which would close the gap it keeps.
    ///
    /// Every kind ends the same way: [`SearchLastKnown`], then
    /// [`KeepPost`], then [`Wander`]. A guard the engine stood at a
    /// prefab's slot that loses the commando searches where it was last
    /// seen before it walks home, and walks home before it drifts off; a
    /// monster with no post passes straight over `KeepPost`, so one brain
    /// serves a kind whether it was posted or not.
    pub(crate) fn from_ron(ron: &str, spawns: &str, registries: &Registries) -> Self {
        let defs: Registry<MonsterDef> = registries.names().load(ron).unwrap_or_else(|e| panic!("monster roster: {e}"));
        let rows: Vec<MonsterSpawn> = registries.names().with("monster", &defs).load_list(spawns).unwrap_or_else(|e| panic!("assets/monster_spawns.ron: {e}"));
        let mut table = BandedTable::default();
        for row in rows {
            table.push(BandedEntry::new(row.monster.id()).bands(row.decks.0, row.decks.1).weight(row.weight).group(row.group.0, row.group.1));
        }
        // What each kind drops, by the item ids the armory hands out: both
        // read `items.ron` through `gear::load_defs`, so an id here is the
        // same thing there.
        let items = crate::gear::load_defs(registries);
        let mut unknown = Vec::new();
        let drops: Vec<DropTable<Id<crate::gear::ItemDef>>> = defs
            .iter()
            .map(|(_, d)| {
                DropTable(
                    d.drops
                        .iter()
                        .filter_map(|(name, pct)| match items.id(name) {
                            Some(id) => Some(DropRow::new(id, *pct)),
                            None => {
                                unknown.push(format!("{} drops {name:?}, which is no item", d.name));
                                None
                            }
                        })
                        .collect(),
                )
            })
            .collect();
        assert!(unknown.is_empty(), "monster roster: {}", unknown.join("; "));
        let mut brains = Vec::new();
        for (_, d) in defs.iter() {
            let mut brain = Brain::new();
            if d.melee.is_some() {
                brain = brain.then(MeleeAdjacent);
            }
            if d.ranged.is_some() {
                brain = brain.then(ShootAtRange::default());
            }
            if d.flee_at > 0 {
                brain = brain.then(FleeWhenHurt { at_pct: d.flee_at });
            }
            brain = match d.shadow {
                Some(s) => brain.then(Keep::enemies(s.keep_within, s.no_closer_than)).then(Hover),
                None => brain.then(Hunt),
            };
            brains.push(Arc::new(brain.then(SearchLastKnown).then(KeepPost).then(Wander { chance_pct: 30 })));
        }
        Self { defs, table, brains, drops }
    }

    /// What `id` leaves when it dies, as the engine's loot rolls it.
    pub fn drops(&self, id: Id<MonsterDef>) -> Drops<crate::gear::ItemDef> {
        Drops(self.drops[id.index()].clone())
    }
}

// ANCHOR: maker
/// The engine's guards are made here: a monster at a prefab's slot is
/// made exactly as one a deck's population places, and a deck's band is
/// its number.
impl ActorMaker for Roster {
    type Def = MonsterDef;

    fn make(&self, commands: &mut Commands, registries: &Registries, def: Id<MonsterDef>, at: Point, map: MapId, _: &mut rand::rngs::StdRng) -> Entity {
        spawn_monster(commands, self, def, at, map, registries)
    }

    fn table(&self) -> &BandedTable<Id<MonsterDef>> {
        &self.table
    }

    fn band(&self, map: MapId) -> i32 {
        crate::decks::deck_of(map) as i32
    }
}
// ANCHOR_END: maker

/// Spawns `id` on `map` at `at`: an actor with health, armor, resistances,
/// perception, a mind and the notice every monster carries. `MeleeAttack`,
/// `RangedAttack`, `Alarm` and `Hearing` are added only for a kind that names them; a kind naming
/// `dark_sight` gets [`NativeDarkSight`] rather than `DarkSight` itself,
/// which [`sensors::sync_dark_sight`] sets from it the moment this pass's
/// `Turn` schedule runs.
pub fn spawn_monster(commands: &mut Commands, roster: &Roster, id: Id<MonsterDef>, at: Point, map: MapId, registries: &Registries) -> Entity {
    let d = roster.defs.get(id);
    let mut e = commands.spawn((
        (Actor, Blocks, Position(at), OnMap(map)),
        (Health::full(d.hp), Armor(d.armor), Faction(d.faction.id()), Resists(resistances(d.profile, registries))),
        (Perception(d.perception), Speed(d.speed), Mind(roster.brains[id.index()].clone()), Intelligence(d.wits)),
        // Everything on a deck leaves something behind: a droid a wreck,
        // a rat a carcass, both named by the engine's remains template and
        // made worth going through by `props::wreck_the_dead`.
        (Notice(d.notice.unwrap_or_default()), Kind(id), LeavesRemains),
        (Name::new(d.name.clone()), Glyph::new(d.glyph, Color::srgb(d.color.0, d.color.1, d.color.2)).on_layer(5)),
    ));
    if let Some(melee) = d.melee {
        e.insert(melee.attack());
    }
    if let Some(ranged) = d.ranged {
        e.insert(ranged.attack());
    }
    if let Some(n) = d.dark_sight {
        e.insert(NativeDarkSight(n));
    }
    if d.alarm {
        e.insert(Alarm);
    }
    if let Some(hearing) = d.hearing {
        e.insert(Hearing(hearing));
    }
    // What it leaves is the engine's to roll and put down when it dies.
    if !d.drops.is_empty() {
        e.insert(roster.drops(id));
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
    fn every_deck_of_the_run_has_something_to_spawn() {
        let roster = Roster::load(&crate::content::registries());
        assert!(roster.table.gaps(1..=crate::decks::DECKS as i32).is_empty());
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
        let (droid, player) = crate::testing::droid_facing_player(&mut app, "heavy droid", 4);
        let struck = crate::testing::run_until_struck(&mut app, droid, 10);
        assert!(struck.ranged, "four tiles off with a clear line: it shoots");
        assert_eq!(struck.target, player);
    }

    /// The droids' half of the ranged ramp in `DESIGN.md`: nothing on deck
    /// one shoots, something on deck two does, and it does so from inside
    /// the six tiles the lamp shows, so the first bolt of a run comes out
    /// of a droid the commando could already see.
    #[test]
    fn nothing_on_deck_one_shoots_and_the_first_shooter_on_deck_two_stands_inside_the_lamp() {
        let roster = Roster::load(&crate::content::registries());
        let reach_on = |deck: i32| -> Vec<(String, Option<i32>)> {
            roster.table.at(deck).map(|row| roster.defs.get(row.item)).map(|d| (d.name.clone(), d.ranged.map(|r| r.range))).collect()
        };
        assert!(reach_on(1).iter().all(|(_, r)| r.is_none()), "a shooter on deck one: {:?}", reach_on(1));
        let shooters: Vec<i32> = reach_on(2).iter().filter_map(|(_, r)| *r).collect();
        assert!(!shooters.is_empty(), "nothing shoots on deck two: {:?}", reach_on(2));
        assert!(shooters.iter().all(|r| *r < crate::light::SHOULDER_LAMP.radius), "a deck two shot from outside the lamp: {shooters:?}");
    }

    #[test]
    fn a_trooper_four_tiles_off_shoots_rather_than_walking_up_to_strike() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (trooper, player) = crate::testing::droid_facing_player(&mut app, "trooper droid", 4);
        let struck = crate::testing::run_until_struck(&mut app, trooper, 10);
        assert!(struck.ranged, "four tiles is its reach: it shoots");
        assert_eq!(struck.target, player);
    }

    /// The other half of the ranged ramp: assembly's own droids have no
    /// gun to shoot with, so the deck the commando arrives on unarmed is
    /// one it can back away from.
    #[test]
    fn a_droid_with_no_gun_walks_the_distance_and_strikes_instead_of_shooting() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (droid, player) = crate::testing::droid_facing_player(&mut app, "line droid", 4);
        let struck = crate::testing::run_until_struck(&mut app, droid, 12);
        assert!(!struck.ranged, "four tiles off with nothing to fire: it closes and strikes");
        assert_eq!(struck.target, player);
    }

    /// Where `listener` last heard something, if it still remembers.
    fn heard(app: &App, listener: Entity) -> Option<Point> {
        app.world().get::<Heard>(listener).and_then(|h| h.last_known())
    }

    fn at(app: &App, e: Entity) -> Point {
        app.world().get::<Position>(e).expect("it stands somewhere").0
    }

    /// A shot is heard ten steps off by a droid that cannot see it, which
    /// is what draws a deck's droids into a firefight.
    #[test]
    fn a_droid_hears_a_shot_it_cannot_see() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (far, _) = crate::testing::out_of_sight(&mut app, "line droid", 500, 900);
        let target = crate::testing::lone_monster(&mut app, "coolant rat");
        let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        let shooter = at(&app, player);
        let kind = app.world().resource::<Registries>().damage_kinds.expect("energy");
        app.world_mut().write_message(DamageEvent::new(target, rl_engine::rl_rules::Hit::by(player, kind, 1)));
        app.update();
        assert_eq!(heard(&app, far), Some(shooter), "it heard the shot, where it was fired from");
    }

    /// The damage table is played, not only written down: the same four
    /// points of ion, down the same pipeline a shot goes down, take eight
    /// off a chassis and one off the commando's flesh.
    #[test]
    fn four_points_of_ion_take_eight_off_a_chassis_and_one_off_the_commando() {
        let mut app = crate::testing::headless(RunSeed(1));
        let droid = crate::testing::lone_monster(&mut app, "heavy droid");
        let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        let ion = app.world().resource::<Registries>().damage_kinds.expect("ion");
        let hp = |app: &App, e: Entity| app.world().get::<Health>(e).expect("it has health").current;
        let (droid_before, player_before) = (hp(&app, droid), hp(&app, player));
        app.world_mut().write_message(DamageEvent::new(droid, rl_engine::rl_rules::Hit::by(player, ion, 4)));
        app.world_mut().write_message(DamageEvent::new(player, rl_engine::rl_rules::Hit::by(droid, ion, 4)));
        app.update();
        assert_eq!(droid_before - hp(&app, droid), 8, "ion undoes a chassis");
        assert_eq!(player_before - hp(&app, player), 1, "and barely registers on flesh");
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

    /// With the lamp on, a droid is sure of the commando well past any
    /// blaster's reach, so a lit fight opens as it always did; unlit, a
    /// droid is sure only up close, and past that noticing is a roll a
    /// careful commando can slip.
    #[test]
    fn a_lit_commando_is_noticed_at_once_within_eight_and_an_unlit_one_past_two_is_a_roll() {
        use rl_engine::rl_rules::ai::awareness::{certain_radius, notice_chance};
        let roster = Roster::load(&crate::content::registries());
        let commando = StealthStats::default();
        for name in ["line droid", "probe droid", "trooper droid", "heavy droid"] {
            let notice = roster.defs.get(roster.defs.expect(name)).notice.expect("every droid names how it notices");
            assert_eq!(certain_radius(&notice, &commando, true), 8, "{name}, lit");
            assert_eq!(certain_radius(&notice, &commando, false), 2, "{name}, unlit");
            assert!((1..100).contains(&notice_chance(&notice, &commando)), "{name}: past that, a roll, never certain and never hopeless");
        }
    }

    /// Droids have the wits to work a door and critters do not, so a shut
    /// door stops the vermin and never a droid on the commando's trail. The
    /// engine's minds open a door for any mind with the wit; this is only
    /// which kinds have it.
    #[test]
    fn every_droid_opens_doors_and_no_critter_does() {
        let roster = Roster::load(&crate::content::registries());
        let doors = [
            ("line droid", true),
            ("probe droid", true),
            ("trooper droid", true),
            ("heavy droid", true),
            ("coolant rat", false),
            ("scrap crab", false),
            ("lamp moth", false),
        ];
        for (name, opens) in doors {
            let wits = roster.defs.get(roster.defs.expect(name)).wits;
            assert_eq!(wits.has(Wits::OPENS_DOORS), opens, "{name}");
        }
    }
}
