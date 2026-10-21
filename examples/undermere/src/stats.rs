//! The stats the engine does not read itself, read where they count.
//!
//! [`content`](crate::content) registers the stats and names four of them
//! to the engine's blows, heals and turns. The rest are Undermere's: the
//! strength heavy gear asks for is [`strength`](crate::strength)'s, the
//! light stat reaches the lantern in [`light`](crate::light), wisdom and
//! reaping a staff's refill in [`staffs`](crate::staffs), the stealth stat
//! reaches the adventurer's own stealth here, by [`keep_stealth`],
//! footfall how loud every actor's steps are, by [`keep_footfall`],
//! regeneration health back over time, by [`regenerate`], and awareness
//! how easily what is hidden is spotted, by [`keep_keen`].

use bevy::prelude::*;
use rl_engine::prelude::*;

use crate::content::{FOOTFALL, QUIET, STEALTH};
use crate::run::Adventurer;

/// What the adventurer's own stealth is with nothing on it: noticeable,
/// not sneaky, a subject a watcher has to notice rather than one it sees
/// the moment it comes into view.
pub const OWN_STEALTH: StealthStats = StealthStats { quiet: 0, subtlety: 0 };

/// Keeps the adventurer's stealth what their stealth and quiet stats make
/// it: the stealth stat's value is percentage points taken off every
/// watcher's chance of noticing them, added to [`OWN_STEALTH`]'s and never
/// taken below nothing, since there is no chance to give a watcher past
/// its own; the quiet stat is added to [`OWN_STEALTH`]'s own `quiet`
/// straight, which a watcher's own certain notice is taken off, so a value
/// below nought, the Crown of Grubnak's own gleam, widens it rather than
/// narrowing it.
///
/// Every pass in `TurnSet::React`, from the stat as it stands, so a ring
/// put on, a draught drunk or a continued run all read right at once.
pub fn keep_stealth(registries: Res<Registries>, mut adventurer: Query<(&StatBlock, &mut Stealth), With<Adventurer>>) {
    let Some(stat) = registries.stats.id(STEALTH) else { return };
    let quiet_stat = registries.stats.id(QUIET);
    for (stats, mut stealth) in &mut adventurer {
        let subtlety = (OWN_STEALTH.subtlety as i32 + stats.0.value(stat, &registries.stats)).max(0) as u32;
        let quiet = OWN_STEALTH.quiet + quiet_stat.map_or(0, |q| stats.0.value(q, &registries.stats));
        let now = StealthStats { subtlety, quiet };
        if stealth.0 != now {
            stealth.0 = now;
        }
    }
}

/// Keeps every actor's steps as loud as its footfall stat says: the
/// engine's `Footfall`, which its noise reads for a step in place of the
/// house's, so plate is heard further off than leather.
///
/// At the start of every pass, in `TurnSet::Schedule`, from the stat as it
/// stands. What is worn is folded onto its wearer's stats in the pass it
/// goes on, unordered against the game's own reactions there, so reading
/// the stat before anyone steps in the next pass is what makes the first
/// step after plate goes on as loud as every step after it. Written only
/// where it differs from what the actor's steps carry already, so a body
/// that has never had anything heavy on carries no `Footfall` at all and
/// steps as the house's [`NoiseRules`] say, as it did before footfall was
/// a stat; once one has, it keeps its `Footfall`, back at the house's step
/// when the heavy thing comes off, which sounds the same as none.
pub fn keep_footfall(
    mut commands: Commands,
    registries: Res<Registries>,
    rules: Res<NoiseRules>,
    actors: Query<(Entity, &StatBlock, Option<&Footfall>), With<Actor>>,
) {
    let Some(stat) = registries.stats.id(FOOTFALL) else { return };
    for (actor, stats, now) in &actors {
        let loudness = stats.0.value(stat, &registries.stats);
        if now.map_or(rules.step, |f| f.0) != loudness {
            commands.entity(actor).insert(Footfall(loudness));
        }
    }
}

/// Hundredths of a step of regeneration counted towards the next point of
/// health back, for a body whose regeneration stat is above nought.
///
/// What is left over between two points, which a continued run starts
/// again from nothing, a point's worth at most.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Regrowth(pub u32);

/// A body as regeneration reads it: its stats, and what it has counted
/// towards its next point.
type Healing = (Entity, &'static StatBlock, Option<&'static mut Regrowth>);

/// What a point of health back costs in regeneration: a hundred turns in
/// hundredths of a step, so a regeneration of ten is a point every ten
/// turns.
pub const POINT: u32 = 100 * BASE_ACTION_COST;

/// Gives every body its regeneration stat in health a hundred turns, a
/// point at a time, on the turn clock: a ring of regeneration's ten a
/// level, and nothing for a stat at or below nought, since nothing heals
/// by itself under the lake.
///
/// Read off the clock rather than counted in passes, so a long action
/// heals for every turn it took. Each point is the engine's [`Heal`] of
/// its own, so its holder's mending scales it as it scales every heal, and
/// a span of turns heals what the same turns one at a time would: one heal
/// of the span's several points would be scaled whole, and a quarter more
/// mending would round ten points up to twelve where ten single points
/// each stay one. As a tick of `regenerating`, which the narrator says
/// nothing of, as it says nothing of a troll's: a line every ten turns
/// would bury the log. In `TurnSet::React`.
pub fn regenerate(
    mut commands: Commands,
    turns: Res<Turns>,
    mut last: Local<Option<u32>>,
    registries: Res<Registries>,
    mut bodies: Query<Healing, (With<Actor>, With<Health>)>,
    mut heals: MessageWriter<Heal>,
) {
    let now = turns.now();
    let passed = now.saturating_sub(last.unwrap_or(now));
    *last = Some(now);
    let (Some(stat), Some(tick)) = (registries.stats.id(crate::content::REGENERATION), registries.statuses.id("regenerating")) else { return };
    if passed == 0 {
        return;
    }
    for (body, stats, grown) in &mut bodies {
        let rate = stats.0.value(stat, &registries.stats);
        if rate <= 0 {
            continue;
        }
        let counted = grown.as_deref().map_or(0, |g| g.0) + passed * rate as u32;
        let points = counted / POINT;
        match grown {
            Some(mut grown) => grown.0 = counted % POINT,
            None => {
                commands.entity(body).insert(Regrowth(counted % POINT));
            }
        }
        for _ in 0..points {
            heals.write(Heal::new(body, 1).of_status(tick));
        }
    }
}

/// Keeps the adventurer's eye for what is hidden, the engine's [`Keen`],
/// what their awareness stat makes it: a ring of awareness's fifteen a
/// level, added to every roll to spot a hidden thing or a secret, and
/// never below nothing.
///
/// On the finder rather than rewritten into every hidden thing's chance,
/// so a secret, which is a cell and not a prop, is found as much faster as
/// a trap is. Derived every pass from the stat rather than added to, so a
/// ring taken off takes its points with it. In `TurnSet::React`, so the
/// roll the next turn makes is made with the eye the ring gives.
pub fn keep_keen(mut commands: Commands, registries: Res<Registries>, adventurer: Query<(Entity, &StatBlock, Option<&Keen>), With<Adventurer>>) {
    let Some(stat) = registries.stats.id(crate::content::AWARENESS) else { return };
    for (who, stats, keen) in &adventurer {
        let points = stats.0.value(stat, &registries.stats).clamp(0, i32::from(u8::MAX)) as u8;
        if keen.map(|k| k.0) != Some(points) {
            commands.entity(who).insert(Keen(points));
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

    /// The quiet stat is added to the watcher's own certain notice
    /// straight, so a value below nought, the Crown of Grubnak's own
    /// gleam, widens it rather than narrowing it as a positive one would.
    #[test]
    fn the_quiet_stat_widens_a_watchers_certain_notice_when_it_is_below_nought() {
        let mut app = headless(RunSeed(1));
        let me = adventurer(&mut app);
        assert_eq!(app.world().get::<Stealth>(me).map(|s| s.0.quiet), Some(OWN_STEALTH.quiet));
        change(&mut app, me, crate::content::QUIET, Op::Add(-2));
        pass_turns(&mut app, 1);
        assert_eq!(app.world().get::<Stealth>(me).map(|s| s.0.quiet), Some(-2));
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

    /// Regeneration heals the same however the clock is chunked: a hundred
    /// turns let pass one at a time and the same hundred in one span, as a
    /// long action takes them, give a wearer mended a quarter better the
    /// ten points a regeneration of ten is worth, each point its own heal
    /// that the mending scales, rather than the one span's ten scaled whole
    /// into twelve.
    #[test]
    fn regeneration_heals_alike_whether_a_hundred_turns_pass_one_at_a_time_or_in_one_span() {
        let mut app = headless(RunSeed(1));
        let me = adventurer(&mut app);
        clear_monsters(&mut app);
        change(&mut app, me, crate::content::REGENERATION, Op::Add(10));
        change(&mut app, me, MENDING, Op::Add(25));
        let mut healed = |spans: &[u32]| {
            // A turn in play first, so the pass has read the clock, then
            // nothing counted towards a point, so each case starts level.
            pass_turns(&mut app, 1);
            app.world_mut().entity_mut(me).insert((Health { current: 10, max: 100 }, Regrowth(0)));
            for span in spans {
                rl_engine::testing::let_time_pass(&mut app, *span);
            }
            // The heal the last pass wrote lands on the frame after it.
            app.update();
            app.world().get::<Health>(me).expect("health").current - 10
        };
        let one_at_a_time = healed(&[1; 100]);
        let at_once = healed(&[100]);
        assert_eq!((one_at_a_time, at_once), (10, 10), "a hundred turns of regeneration ten, a turn at a time and in one span");
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
            app.world_mut().run_system_once(move |marks: Marksmanship| marks.odds(from, to, Attempt::Blow, 0).map(|o| o.percent())).expect("the odds")
        };
        assert_eq!((chance(&mut app, me, rat), chance(&mut app, rat, me)), (Some(100), Some(100)), "everyone lands a blow");
        let shield = make(&mut app, "shield");
        equip(&mut app, me, shield);
        assert_eq!(chance(&mut app, rat, me), Some(90), "but on the shield's wearer");
    }
}
