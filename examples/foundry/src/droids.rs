//! What walks the decks: line, probe, trooper and heavy droids, and the
//! critters, coolant rats, scrap crabs and lamp moths, loaded from
//! `monsters.ron`, with where each turns up from `monster_spawns.ron`. A
//! kind that names a shot is built with its own `RangedAttack`, rather than
//! handed a weapon to hold, and spawned with a brain that fires it at
//! anything in reach before ever closing to a punch.
//!
//! `alarm` holds a probe's radar reporting to the rest of the deck.
//! `sensors` holds what an ion hit does to that same radar: a jam that
//! suppresses the engine's dark sight, which the engine folds from a
//! monster's own radar, its parts' `dark_sight`, and whatever it wears.
//! `spawns` holds populating a deck
//! the first time it is entered. Kept out of this file so it stays about
//! the roster and the spawn, which is already most of a file's worth on
//! its own.

mod alarm;
mod repair;
mod sensors;
mod spawns;

use std::sync::Arc;

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_rules::ai::Standard;
use rl_engine::rl_rules::ai::hearing::HearingStats;
use rl_engine::rl_rules::ai::tactics::{Hover, Keep};
use rl_engine::rl_rules::faction::FactionDef;
use rl_engine::rl_rules::parts::{self, KeepAway};
use serde::Deserialize;

pub use alarm::{ALARM_LOUDNESS, ALARM_SOUND, Alarm, NOISE, PULSE, Raised, SIGNALLING, SignalAlarm, lower_alarm, sense_alarm, shout_alarm, sound_alarm};
pub use repair::{REPAIRING, RepairWrecks, Wrecks, rebuild_wrecks, sense_wrecks};
pub use sensors::{Jammed, jam_sensors, unjam_sensors};
pub(crate) use spawns::MIN_DISTANCE_FROM_ENTRY;
pub use spawns::populate_deck;

use rl_engine::rl_rules::parts::{Melee, Ranged};

use crate::content::{Profile, resistances};
use crate::gear::ItemDef;

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
    pub melee: Option<Melee>,
    /// The range, roll and damage kind a shot deals; present, it shoots.
    #[serde(default)]
    pub ranged: Option<Ranged>,
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
    /// The turns it signals before it sounds the deck's alarm, which then
    /// sounds for as long as it knows where an enemy is; absent, it has no
    /// alarm to sound.
    #[serde(default)]
    pub alarm: Option<u16>,
    /// How far from the enemies in sight it keeps; present, it hangs at
    /// that distance rather than closing in.
    #[serde(default)]
    pub keep_away: Option<KeepAway>,
    /// How it hears; absent, it is deaf, and neither the alarm nor a
    /// firefight draws it.
    #[serde(default)]
    pub hearing: Option<HearingStats>,
    /// `(item name, percent chance)`, each rolled on its own death.
    #[serde(default)]
    pub drops: Vec<(NameRef<ItemDef>, u32)>,
    /// The turns it takes to rebuild a wreck of its own side, before the
    /// wreck's own share; present, it rebuilds wrecks rather than hunting.
    #[serde(default)]
    pub repairs: Option<u16>,
}

impl Named for MonsterDef {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Marks a monster with the definition it was spawned from, for the drops
/// a later task rolls on its death.
#[derive(Component, Clone, Copy)]
pub struct Kind(pub Id<MonsterDef>);

/// The monster definitions, the deck-banded table they spawn from, and
/// everything of each kind the engine plays.
#[derive(Resource)]
pub struct Roster {
    /// The monster definitions, by id.
    pub defs: Registry<MonsterDef>,
    /// What can spawn on which deck, in what group sizes.
    pub table: BandedTable<Id<MonsterDef>>,
    /// Each kind's parts over [`defaults`], with the brain it decides with
    /// and what it leaves when it dies, indexed the same way `defs` is.
    parts: Vec<ActorParts<ItemDef>>,
}

/// What every kind on the decks gets that its own line does not say: it
/// notices by the engine's plain rule, sure only of what is adjacent, and
/// it leaves something behind, a droid a wreck and a rat a carcass, both
/// named by the engine's remains template and made worth going through by
/// `props::wreck_the_dead`. Foundry's choices, not the engine's, which
/// gives a kind that names neither no notice and no remains.
pub fn defaults() -> ActorParts<ItemDef> {
    ActorParts { notice: Some(NoticeStats::default()), leaves_remains: Some(true), ..ActorParts::default() }
}

impl Roster {
    /// Loads `monsters.ron` and `monster_spawns.ron` against `registries`.
    /// Panics with every problem either file has, since a broken roster is a
    /// game that cannot start.
    ///
    /// The shipped spawn table must have something to draw on every deck,
    /// or a deck is populated by nothing; a test's own roster, through
    /// `from_ron`, is held to nothing of the kind.
    pub fn load(registries: &Registries) -> Self {
        let roster = Self::from_ron(MONSTERS_RON, MONSTER_SPAWNS_RON, registries);
        let gaps = roster.table.gaps(1..=crate::decks::DECKS as i32);
        assert!(gaps.is_empty(), "assets/monster_spawns.ron: nothing to draw on decks {gaps:?}");
        roster
    }

    /// As [`load`](Self::load), but from `ron` and `spawns` rather than the
    /// compiled-in files, so a test can load a tiny roster of its own, such
    /// as one monster with a guaranteed drop.
    ///
    /// Builds the spawn table, and gives every kind its parts, what it drops
    /// named against `items.ron`, and a brain from what it names by
    /// [`DROID`], with Foundry's own tactics where the chain takes them: a
    /// kind with an alarm signals it before anything else, a kind that
    /// repairs rebuilds wrecks in place of the hunt, and a kind that names
    /// `keep_away` keeps its distance with [`Keep::enemies`] and hangs
    /// there with [`Hover`] in place of the hunt, which would close the gap
    /// it keeps.
    pub(crate) fn from_ron(ron: &str, spawns: &str, registries: &Registries) -> Self {
        // What each kind drops is named by the item ids the armory hands
        // out: both read `items.ron` through `gear::load_defs`, so an id
        // here is the same thing there.
        let items = crate::gear::load_defs(registries);
        let defs: Registry<MonsterDef> = registries.names().with("item", &items).load(ron).unwrap_or_else(|e| panic!("monster roster: {e}"));
        let table = load_spawns(spawns, &registries.names().with("monster", &defs), &defs).unwrap_or_else(|e| panic!("assets/monster_spawns.ron: {e}"));
        let mut parts = Vec::new();
        for (_, d) in defs.iter() {
            let made = parts_of(d);
            let wrong = made.check();
            assert!(wrong.is_empty(), "monster roster: {}: {}", d.name, wrong.join("; "));
            // ANCHOR: brains
            let mut brain = DROID.brain(made.repertoire());
            // First, since nothing a probe would otherwise do matters more
            // than raising the alarm it has not raised yet.
            if d.alarm.is_some() {
                brain = brain.first(SignalAlarm);
            }
            brain = match (d.repairs, d.keep_away) {
                // A drone that rebuilds has nothing to hunt with.
                (Some(_), _) => brain.instead_of_hunt(RepairWrecks),
                (None, Some(keep)) => brain.instead_of_hunt(Keep::enemies(keep.within, keep.no_closer)).instead_of_hunt(Hover),
                (None, None) => brain,
            };
            // ANCHOR_END: brains
            parts.push(ActorParts { mind: Some(Arc::new(brain.build())), ..made });
        }
        Self { defs, table, parts }
    }

    /// What `id` leaves when it dies, as the engine's loot rolls it.
    pub fn drops(&self, id: Id<MonsterDef>) -> Drops<ItemDef> {
        Drops(self.parts[id.index()].drops.clone())
    }
}

/// The chain every kind on the decks decides with, the engine's standard
/// chain at Foundry's numbers: a blow for a kind with one, then a shot for
/// a kind with a shot, the one fired only at an adjacent enemy and the
/// other only at one two or more tiles off, so they never compete for the
/// same target; then running, after the attacks, for a kind that names
/// `flee_at` above zero, so a droid trades the blow it has in hand before
/// it goes; then the hunt.
///
/// Every kind ends the same way: search where the commando was last seen,
/// walk back to its post, and drift. A guard the engine stood at a
/// prefab's slot that loses the commando searches before it walks home,
/// and walks home before it drifts off; a monster with no post passes
/// straight over `KeepPost`, so one brain serves a kind whether it was
/// posted or not. The span a kind keeps is not the chain's: Foundry holds
/// it in place of the hunt, beside `Hover`.
const DROID: Standard = Standard {
    melee: true,
    shoot: true,
    flee: true,
    flee_after_attacks: true,
    hunt: true,
    search: true,
    keep_post: true,
    wander_chance: Some(30),
    ..Standard::NONE
};

/// `d`'s parts over [`defaults`], its brain still to be built from them:
/// everything of it the engine plays, and nothing of what is Foundry's own. Its radar is
/// the engine's dark sight, its own beside whatever it wears, its
/// resistances are its profile's, and its alarm and its
/// repairs are Foundry's systems, so `spawn_monster` puts each on beside
/// these.
fn parts_of(d: &MonsterDef) -> ActorParts<ItemDef> {
    ActorParts {
        hp: Some(d.hp),
        armor: Some(d.armor),
        faction: Some(d.faction.id()),
        speed: Some(d.speed),
        perception: Some(d.perception),
        flee_at: Some(d.flee_at),
        wits: Some(d.wits),
        notice: d.notice,
        hearing: d.hearing,
        dark_sight: d.dark_sight,
        attacks: Attacks { melee: d.melee, ranged: d.ranged, ..Attacks::default() },
        keep_away: d.keep_away,
        drops: parts::drops(&d.drops),
        ..ActorParts::default()
    }
    .over(&defaults())
}

// ANCHOR: maker
/// The engine's guards are made here: a monster at a prefab's slot is
/// made exactly as one a deck's population places, and a deck's band is
/// its number.
impl ActorMaker for Roster {
    type Def = MonsterDef;

    fn make_at(&self, commands: &mut Commands, registries: &Registries, def: Id<MonsterDef>, at: Point, map: MapId) -> Entity {
        spawn_monster(commands, self, def, at, map, registries)
    }

    fn id_of(&self, name: &str) -> Option<Id<MonsterDef>> {
        self.defs.id(name)
    }

    fn name_of(&self, def: Id<MonsterDef>) -> &str {
        self.defs.name(def)
    }

    fn table(&self) -> &BandedTable<Id<MonsterDef>> {
        &self.table
    }

    fn band(&self, map: MapId) -> i32 {
        crate::decks::deck_of(map) as i32
    }
}
// ANCHOR_END: maker

/// Spawns `id` on `map` at `at`: an actor with its name and its look, the
/// resistances its profile gives it, and everything of it its parts say,
/// health, armor, perception, a mind, the notice every monster carries,
/// remains, and its blow, its shot, its hearing, its radar and its drops
/// for a kind that names them. `Alarm` is added only for a kind that names
/// one.
pub fn spawn_monster(commands: &mut Commands, roster: &Roster, id: Id<MonsterDef>, at: Point, map: MapId, registries: &Registries) -> Entity {
    // ANCHOR: spawn
    let d = roster.defs.get(id);
    let mut e = commands.spawn((
        (Actor, Blocks, Position(at), OnMap(map), Kind(id), Resists(resistances(d.profile, registries))),
        (Name::new(d.name.clone()), Glyph::new(d.glyph, Color::srgb(d.color.0, d.color.1, d.color.2)).on_layer(5)),
    ));
    // Health, a mind, the notice every kind carries by `defaults`, and
    // remains: everything on a deck leaves something behind.
    roster.parts[id.index()].insert(&mut e, registries);
    // ANCHOR_END: spawn
    if let Some(signal) = d.alarm {
        e.insert(Alarm { signal });
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
