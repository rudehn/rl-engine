//! What holds on a body while something about it lasts: a troll's
//! mending halted while it burns, and a Choirmaster's song over his
//! own; and the wraith's shroud, which is the engine's `Concealed`.

use super::*;

/// Marks a kind whose regeneration stops while it burns: a troll's
/// `burns-badly`, the second half of its trait.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct BurnsBadly;

/// Marks a [`BurnsBadly`] body whose mending is halted this moment, so it
/// is lifted exactly once when the burning ends.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct RegenHalted;

/// The tag [`keep_regen_halted`]'s own modifier sits under, so it removes
/// exactly the one it put on the mending stat and nothing else there.
const REGEN_HALT_TAG: u64 = u64::from_be_bytes(*b"burnbad1");

/// A [`BurnsBadly`] body as [`keep_regen_halted`] reads it: what it
/// suffers, its stats, and whether its mending is halted now.
type Burning = (Entity, &'static Afflicted, &'static mut StatBlock, Has<RegenHalted>);

/// Zeroes a [`BurnsBadly`] body's mending while it holds `scorched`, and
/// lifts the zero the moment the burning ends: regeneration, and nothing
/// else that mends it, heals it while it burns, as the trait's words say.
///
/// A blunt tool beside the trait's one line: every heal is scaled by the
/// mending stat, so this halts a mend ally's hand along with a troll's
/// own regeneration while it burns, not only the regeneration by name.
/// Judged the simpler reading over threading a second,
/// regeneration-only scale through every heal in the engine for one
/// trait's one kind; no row of this bestiary both regenerates and is
/// mended by another's ability, so nothing today tells the two apart.
///
/// In `TurnSet::React`.
pub fn keep_regen_halted(registries: Res<Registries>, mut bodies: Query<Burning, (With<BurnsBadly>, With<Actor>)>, mut commands: Commands) {
    let Some(scorched) = registries.statuses.id("scorched") else { return };
    let Some(mending) = registries.stats.id(crate::content::MENDING) else { return };
    for (e, held, mut stats, halted) in &mut bodies {
        let burning = held.0.has(scorched);
        if burning && !halted {
            stats.0.add(Modifier::new(mending, Op::Add(-100), Source::Game(REGEN_HALT_TAG)));
            commands.entity(e).insert(RegenHalted);
        } else if !burning && halted {
            stats.0.remove_source(Source::Game(REGEN_HALT_TAG));
            commands.entity(e).remove::<RegenHalted>();
        }
    }
}

/// Marks a kind whose choir mends whoever of its own side stands near it:
/// the Choirmaster's `singing`. Marked at spawn off [`Tricks::sings`].
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Choir;

/// How far a [`Choir`]'s own song reaches: the content sheet's own four.
const SING_RANGE: i32 = 4;

/// A choir that is singing: a living one, an `Actor`, since a
/// Choirmaster's body keeps `Choir`, its cell and its side, and a choir
/// read off remains sang over its own corpse for good.
type Singing = (With<Choir>, With<Actor>);

/// A body a choir might sing to: where it stands, its side, what it
/// suffers and its stats, which the song's regeneration is kept in, and
/// whether the song already gave it its regeneration.
type SingingTo = (Entity, &'static Position, &'static Faction, &'static mut Afflicted, &'static mut StatBlock, Has<Sung>);

/// Keeps `regenerating` on every actor of a [`Choir`]'s own side within
/// `SING_RANGE` of it, and takes it off the turn it falls out of range:
/// a Choirmaster's song is heard exactly as far as it carries. Plain
/// rather than a blunt mending-wide change, since a status already
/// scaled by the mending stat is what "regenerate" already means
/// everywhere else in this bestiary.
///
/// Edge-triggered, as `keep_fireproof` is: given only the turn a body
/// comes into range, taken only the turn it leaves it. Only what the song
/// gave is taken back, marked [`Sung`]: a body already regenerating by its
/// own trait or a worn ring keeps it, in range or out.
///
/// Put on and taken off the body's own statuses rather than through
/// `Afflict` and `Cure`, which the narrator says aloud: the song is a
/// trait of the Choirmaster's, known on sight, as a troll's own
/// regeneration is there from its first turn unannounced, and the log of
/// a crypt walked into read "The skeleton is regenerating. (x4)", and
/// again every time one stepped out of range and back.
///
/// In `TurnSet::React`.
pub fn sing_choir(
    mut commands: Commands,
    registries: Res<Registries>,
    choir: Query<(&Position, &Faction), Singing>,
    mut bodies: Query<SingingTo, With<Actor>>,
) {
    let Some(status) = registries.statuses.id("regenerating") else { return };
    for (e, pos, side, mut held, mut stats, sung) in &mut bodies {
        let sung_to = choir.iter().any(|(at, theirs)| theirs.0 == side.0 && geometry::chebyshev(at.0, pos.0) <= SING_RANGE);
        if sung_to && !sung && !held.0.has(status) {
            held.0.apply(status, crate::bestiary::FOR_LIFE, None, &registries.statuses, &mut stats.0);
            commands.entity(e).insert(Sung);
        } else if !sung_to && sung {
            held.0.cure(status, &mut stats.0);
            commands.entity(e).remove::<Sung>();
        }
    }
}

/// Marks a body whose `regenerating` a [`Choir`] gave, so leaving the song
/// takes back only that and never a regeneration of its own.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Sung;

#[cfg(test)]
mod tests {
    use rl_engine::rl_bevy::status::resolve_afflictions;

    use super::*;

    /// A wraith's `shrouded`, played on a real floor: stood on a cell the
    /// adventurer sees but no light reaches, it is not among what they
    /// see, where a skeleton stood in the same dark is; stood in their
    /// lantern's light two cells off, it is.
    #[test]
    fn a_wraith_in_the_dark_is_not_sighted_and_is_once_lit() {
        use crate::testing::{REACH, arena_on, pass_turns, stand_on, still};
        let (mut app, me, here) = arena_on(7, 7);
        let dark: Vec<Point> = {
            let world = app.world();
            let (map, lighting, sight) = (world.resource::<WorldMap>(), world.resource::<Lighting>(), world.get::<Viewshed>(me).expect("sight"));
            geometry::square(here, REACH)
                .filter(|p| map.is_walkable(*p) && sight.can_see(*p) && lighting.band(*p) != LightBand::Lit && geometry::chebyshev(*p, here) > 1)
                .collect()
        };
        assert!(dark.len() >= 2, "two cells in sight that no light reaches: {dark:?}");
        let wraith = still(&mut app, "wraith", dark[0]);
        let skeleton = still(&mut app, "skeleton", dark[1]);
        pass_turns(&mut app, 1);
        let sighted = |app: &App, who: Entity| app.world().resource::<Sighted>().list().iter().any(|s| s.entity == who);
        assert!(sighted(&app, skeleton), "a skeleton in the dark at {:?} is seen", dark[1]);
        assert!(!sighted(&app, wraith), "a wraith in the dark at {:?} is not", dark[0]);
        let lit = here.offset(2, 0);
        assert_eq!(app.world().resource::<Lighting>().band(lit), LightBand::Lit, "the lantern lights two cells off");
        stand_on(&mut app, wraith, lit);
        pass_turns(&mut app, 1);
        assert!(sighted(&app, wraith), "in the lantern's light, the wraith is seen");
    }

    /// The Choirmaster's `singing`: an ally of its own side within
    /// `SING_RANGE` regenerates, and loses it the pass it steps past
    /// that range, as a body walking out of it would, and nothing of it
    /// is a status the narrator would say aloud.
    #[test]
    fn a_choirs_own_side_within_its_range_regenerates_quietly_and_loses_it_once_out_of_range() {
        let mut app = App::new();
        app.add_message::<Afflict>().add_message::<Cure>().add_message::<rl_engine::rl_bevy::status::StatusEvent>();
        let registries = crate::content::registries();
        let undead = registries.factions.expect("undead");
        app.insert_resource(registries);
        app.add_systems(Update, (sing_choir, resolve_afflictions).chain());
        app.world_mut().spawn((Actor, Position(Point::ZERO), Faction(undead), Choir));
        let near = app.world_mut().spawn((Actor, Position(Point::new(3, 0)), Faction(undead), Afflicted(Statuses::new()), StatBlock(Stats::new()))).id();
        let far = app.world_mut().spawn((Actor, Position(Point::new(10, 0)), Faction(undead), Afflicted(Statuses::new()), StatBlock(Stats::new()))).id();
        app.update();
        let regenerating = app.world().resource::<Registries>().statuses.expect("regenerating");
        assert!(app.world().get::<Afflicted>(near).unwrap().0.has(regenerating), "within the choir's range, it regenerates");
        assert!(!app.world().get::<Afflicted>(far).unwrap().0.has(regenerating), "past it, nothing");
        let said = app.world().resource::<Messages<rl_engine::rl_bevy::status::StatusEvent>>().iter_current_update_messages().count();
        assert_eq!(said, 0, "given quietly, as a trait of the choir's known on sight");
        app.world_mut().entity_mut(near).insert(Position(Point::new(10, 0)));
        app.update();
        assert!(!app.world().get::<Afflicted>(near).unwrap().0.has(regenerating), "stepped past the range, the choir's own gift is lifted");
    }

    /// The song ends with the singer: a skeleton beside the Choirmaster
    /// regenerates while he lives, and the pass he dies it no longer does,
    /// though his body keeps lying within range of it.
    #[test]
    fn a_choirs_song_stops_the_moment_its_singer_dies() {
        use crate::testing::{arena, kill, monster, pass_turns, settle};
        let (mut app, _, here) = arena(33);
        let singer = monster(&mut app, "the Choirmaster", here.offset(4, 0));
        let skeleton = monster(&mut app, "skeleton", here.offset(4, 2));
        app.world_mut().entity_mut(skeleton).remove::<Mind>();
        settle(&mut app);
        let regenerating = app.world().resource::<Registries>().statuses.expect("regenerating");
        let sung = |app: &App| app.world().get::<Afflicted>(skeleton).is_some_and(|a| a.0.has(regenerating));
        assert!(sung(&app), "sung to while the Choirmaster lives");
        kill(&mut app, singer, None);
        pass_turns(&mut app, 1);
        assert!(app.world().get::<Remains>(singer).is_some(), "his body lies where he fell, within range");
        assert!(!sung(&app), "the song stopped when he died");
    }

    /// A `BurnsBadly` body's mending is zeroed while `resolve_afflictions`
    /// holds `scorched` on it, and lifted the moment it is cured.
    #[test]
    fn a_burns_badly_kinds_mending_is_halted_while_it_burns_and_lifted_once_it_is_not() {
        let mut app = App::new();
        app.add_message::<Afflict>().add_message::<Cure>().add_message::<rl_engine::rl_bevy::status::StatusEvent>();
        let registries = crate::content::registries();
        let (scorched, mending) = (registries.statuses.expect("scorched"), registries.stats.expect(crate::content::MENDING));
        app.insert_resource(registries);
        app.add_systems(Update, (resolve_afflictions, keep_regen_halted).chain());
        let e = app.world_mut().spawn((Actor, BurnsBadly, Afflicted(Statuses::new()), StatBlock(Stats::new()))).id();
        // The same formula `Loadout::mending` reads the stat by: a
        // hundred and the stat, never below nothing.
        let mending_of = |app: &App| (100 + app.world().get::<StatBlock>(e).unwrap().0.value(mending, &app.world().resource::<Registries>().stats)).max(0);
        app.update();
        assert_eq!(mending_of(&app), 100, "not burning yet");
        app.world_mut().write_message(Afflict { target: e, status: scorched, turns: 3, by: None, held_by: None });
        app.update();
        assert_eq!(mending_of(&app), 0, "burning, so no mending reaches it");
        app.world_mut().write_message(Cure { target: e, status: scorched });
        app.update();
        assert_eq!(mending_of(&app), 100, "the burning is over, and so is the halt");
    }
}
