//! The stats the engine does not read itself, read where they count.
//!
//! [`content`](crate::content) registers the stats and names four of them
//! to the engine's blows, heals and turns. The rest are Undermere's: the
//! strength heavy gear asks for is [`strength`](crate::strength)'s, the
//! light stat reaches the lantern in [`light`](crate::light), and the
//! stealth stat reaches the adventurer's own stealth here, by
//! [`keep_stealth`].

use bevy::prelude::*;
use rl_engine::prelude::*;

use crate::content::STEALTH;
use crate::run::Adventurer;

/// What the adventurer's own stealth is with nothing on it: noticeable,
/// not sneaky, a subject a watcher has to notice rather than one it sees
/// the moment it comes into view.
pub const OWN_STEALTH: StealthStats = StealthStats { quiet: 0, subtlety: 0 };

/// Keeps the adventurer's stealth what their stealth stat makes it: the
/// stat's value is percentage points taken off every watcher's chance of
/// noticing them, added to [`OWN_STEALTH`]'s, and never taken below
/// nothing, since there is no chance to give a watcher past its own.
///
/// Every pass in `TurnSet::React`, from the stat as it stands, so a ring
/// put on, a draught drunk or a continued run all read right at once.
pub fn keep_stealth(registries: Res<Registries>, mut adventurer: Query<(&StatBlock, &mut Stealth), With<Adventurer>>) {
    let Some(stat) = registries.stats.id(STEALTH) else { return };
    for (stats, mut stealth) in &mut adventurer {
        let subtlety = (OWN_STEALTH.subtlety as i32 + stats.0.value(stat, &registries.stats)).max(0) as u32;
        let now = StealthStats { subtlety, ..OWN_STEALTH };
        if stealth.0 != now {
            stealth.0 = now;
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use rl_engine::rl_core::RunSeed;
    use rl_engine::rl_rules::stats::Source;

    use super::*;
    use crate::content::{LIGHT, MENDING};
    use crate::testing::{adventurer, at, clear_monsters, equip, headless, lay_floor, make, monster, pass_turns, settle};

    /// Puts `op` on `who`'s stat `name`, as a status or a ring would.
    fn change(app: &mut App, who: Entity, name: &str, op: Op) {
        let stat = app.world().resource::<Registries>().stats.expect(name);
        app.world_mut().get_mut::<StatBlock>(who).expect("stats").0.add(Modifier::new(stat, op, Source::Game(99)));
    }

    /// The stealth stat is taken off every watcher's chance of noticing
    /// the adventurer, and a stealth below nothing gives a watcher no
    /// chance past its own.
    #[test]
    fn the_stealth_stat_is_taken_off_a_watchers_chance_and_never_below_nothing() {
        let mut app = headless(RunSeed(1));
        let me = adventurer(&mut app);
        assert_eq!(app.world().get::<Stealth>(me).map(|s| s.0), Some(OWN_STEALTH));
        change(&mut app, me, STEALTH, Op::Add(20));
        pass_turns(&mut app, 1);
        assert_eq!(app.world().get::<Stealth>(me).map(|s| s.0.subtlety), Some(20));
        change(&mut app, me, STEALTH, Op::Add(-50));
        pass_turns(&mut app, 1);
        assert_eq!(app.world().get::<Stealth>(me).map(|s| s.0.subtlety), Some(0));
    }

    /// The light stat reaches the lantern further, and never puts it out.
    #[test]
    fn the_light_stat_reaches_the_lantern_further_and_never_puts_it_out() {
        let mut app = headless(RunSeed(1));
        let me = adventurer(&mut app);
        let lantern = |app: &mut App| {
            let world = app.world_mut();
            world.query::<(&Name, &LightSource)>().iter(world).find(|(n, _)| n.as_str() == crate::run::LANTERN).map(|(_, l)| l.radius)
        };
        assert_eq!(lantern(&mut app), Some(7));
        change(&mut app, me, LIGHT, Op::Add(2));
        pass_turns(&mut app, 1);
        assert_eq!(lantern(&mut app), Some(9));
        change(&mut app, me, LIGHT, Op::Add(-20));
        pass_turns(&mut app, 1);
        assert_eq!(lantern(&mut app), Some(1));
    }

    /// The speed stat is a percentage of the adventurer's own speed, and a
    /// chill a quarter off it; the mending stat is a percentage on every
    /// heal.
    #[test]
    fn a_chill_takes_a_quarter_off_the_adventurers_speed_and_the_mending_stat_reaches_every_heal() {
        let mut app = headless(RunSeed(1));
        let me = adventurer(&mut app);
        let speed = |app: &mut App| app.world_mut().run_system_once(move |speeds: Speeds| speeds.of(me)).expect("speeds");
        let mending = |app: &mut App| app.world_mut().run_system_once(move |loadout: Loadout| loadout.mending(me)).expect("a loadout");
        assert_eq!((speed(&mut app), mending(&mut app)), (100, 100), "as fast and as mended as anyone, with nothing on");
        let chilled = app.world().resource::<Registries>().statuses.expect("chilled");
        app.world_mut().write_message(Afflict { target: me, status: chilled, turns: 4, by: None, held_by: None });
        settle(&mut app);
        assert_eq!(speed(&mut app), 75, "chilled");
        change(&mut app, me, MENDING, Op::Add(-100));
        assert_eq!(mending(&mut app), 0, "a mending of minus a hundred mends nothing");
    }

    /// Accuracy is a hundred and evasion nothing for everyone, so a blow
    /// lands; a shield's wearer turns one in ten aside.
    #[test]
    fn a_blow_lands_on_anyone_but_a_shields_wearer_turns_one_in_ten() {
        let mut app = headless(RunSeed(2));
        let me = adventurer(&mut app);
        clear_monsters(&mut app);
        let beside = at(&app, me).offset(1, 0);
        lay_floor(&mut app, &[beside]);
        let rat = monster(&mut app, "rat", beside);
        let chance = |app: &mut App, from: Entity, to: Entity| {
            app.world_mut().run_system_once(move |marks: Marksmanship| marks.odds(from, to, Attempt::Blow).map(|o| o.percent())).expect("the odds")
        };
        assert_eq!((chance(&mut app, me, rat), chance(&mut app, rat, me)), (Some(100), Some(100)), "everyone lands a blow");
        let shield = make(&mut app, "shield");
        equip(&mut app, me, shield);
        assert_eq!(chance(&mut app, rat, me), Some(90), "but on the shield's wearer");
    }
}
