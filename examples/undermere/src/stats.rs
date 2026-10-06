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
//! how easily what is hidden is spotted, by [`keep_awareness`].

use bevy::prelude::*;
use rl_engine::prelude::*;

use crate::content::{FOOTFALL, STEALTH};
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
/// with nothing heavy on carries no `Footfall` at all and steps as the
/// house's [`NoiseRules`] say, as it did before footfall was a stat.
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
/// heals for every turn it took. Each point is the engine's [`Heal`], so
/// its holder's mending scales it as it scales every heal, and as a tick of
/// `regenerating`, which the narrator says nothing of, as it says nothing
/// of a troll's: a line every ten turns would bury the log. In
/// `TurnSet::React`.
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
        if points > 0 {
            heals.write(Heal::new(body, points as i32).of_status(tick));
        }
    }
}

/// Keeps every hidden prop as easy to spot as its definition says, and as
/// many points easier a turn as the adventurer's awareness stat: a ring of
/// awareness's fifteen a level, and never below nothing nor above certain.
///
/// Derived every pass from the definition rather than added to, so a ring
/// taken off takes its points with it. In `TurnSet::React`, so the roll
/// the next pass makes for what is in sight is made at the chance the ring
/// gives.
pub fn keep_awareness(registries: Res<Registries>, adventurer: Query<&StatBlock, With<Adventurer>>, mut hidden: Query<(&PropKind, &mut Hidden)>) {
    let Some(stat) = registries.stats.id(crate::content::AWARENESS) else { return };
    let awareness = adventurer.single().map_or(0, |stats| stats.0.value(stat, &registries.stats));
    for (kind, mut secret) in &mut hidden {
        let own = registries.props.get(kind.0).hidden.map_or(0, |h| i32::from(h.spot));
        let spot = (own + awareness).clamp(0, 100) as u8;
        if secret.spot != spot {
            secret.spot = spot;
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
