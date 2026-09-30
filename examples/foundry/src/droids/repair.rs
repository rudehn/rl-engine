//! The repair drone: what it knows about wrecks, how it picks one, and
//! what rebuilding one means.
//!
//! The engine owns the work: the turns, the breaking off, the row that
//! says `(repairing)`. What Foundry owns is which wrecks are worth
//! rebuilding, how long each takes, and that a finished one stands up as
//! the droid it was.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_bevy::{ReviveCommands, Sight, Thinking, WorkDone, WorkKinds};
use rl_engine::rl_core::Point;
use rl_engine::rl_rules::ai::{Decision, Tactic, TacticCtx};
use rl_engine::rl_rules::work::{Work, WorkKindId, in_reach};

use super::{Kind, Roster};

/// The kind of work a repair drone does, by the word its row shows.
pub const REPAIRING: &str = "repairing";

/// The wrecks of its own side a repair drone can see, with the turns each
/// would take it.
#[derive(Debug, Clone)]
pub struct Wrecks {
    /// The kind of work, so the tactic needs no resource.
    pub kind: WorkKindId,
    /// Each wreck, where it lies, and how long rebuilding it takes.
    pub found: Vec<(Entity, Point, u16)>,
}

/// Every wreck, where it lies, what it was and whose.
type WreckSites<'w, 's> = Query<'w, 's, (Entity, &'static Position, Option<&'static OnMap>, &'static Kind, &'static Faction), With<Remains>>;

// ANCHOR: sense
/// Tells a repair drone holding the turn which wrecks of its own side it
/// can see, and how long each would take: its own `repairs`, plus one
/// turn for every two points of the wrecked kind's health.
pub fn sense_wrecks(
    mut thinking: ResMut<Thinking>,
    sight: Sight,
    roster: Res<Roster>,
    kinds: Res<WorkKinds>,
    me: Query<(&Kind, &Faction)>,
    wrecks: WreckSites,
) {
    let Some(actor) = thinking.actor() else { return };
    let Ok((my_kind, my_side)) = me.get(actor) else { return };
    let (Some(base), Some(kind)) = (roster.defs.get(my_kind.0).repairs, kinds.get(REPAIRING)) else { return };
    let found = wrecks
        .iter()
        .filter(|(.., side)| side.0 == my_side.0)
        .filter(|(_, pos, on, ..)| sight.perceives(&thinking, pos.0, *on))
        .map(|(wreck, pos, _, kind, _)| (wreck, pos.0, base + (roster.defs.get(kind.0).hp.max(0) as u16) / 2))
        .collect();
    if let Some(snapshot) = thinking.snapshot_mut() {
        snapshot.add_sense(Wrecks { kind, found });
    }
}
// ANCHOR_END: sense

// ANCHOR: tactic
/// Rebuild a wreck: set to work on it if it is within reach, and walk
/// toward the nearest otherwise.
pub struct RepairWrecks;

impl Tactic<Entity> for RepairWrecks {
    fn name(&self) -> &'static str {
        "repair_wrecks"
    }

    fn evaluate(&self, ctx: &mut TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
        let me = ctx.snapshot.me.pos;
        let wrecks = ctx.snapshot.sense::<Wrecks>()?.clone();
        if let Some((wreck, _, turns)) = wrecks.found.iter().find(|(_, at, _)| in_reach(me, *at)) {
            return Some(Decision::Work(Work::new(wrecks.kind, *turns).on(*wreck)));
        }
        let cells: Vec<Point> = wrecks.found.iter().map(|(_, at, _)| *at).collect();
        ctx.step_toward(&cells).map(Decision::Step)
    }
}
// ANCHOR_END: tactic

// ANCHOR: rebuild
/// A finished repair stands the wreck up as the droid it was, at half its
/// health, and says so when the commando can see it happen.
pub fn rebuild_wrecks(
    mut commands: Commands,
    mut done: MessageReader<WorkDone>,
    kinds: Res<WorkKinds>,
    roster: Res<Roster>,
    wrecks: Query<(&Kind, &Position), With<Remains>>,
    eyes: Query<&Viewshed, With<Player>>,
    mut tell: MessageWriter<Tell>,
) {
    let Some(repairing) = kinds.get(REPAIRING) else { return };
    for finished in done.read().filter(|d| d.kind == repairing) {
        let Some(wreck) = finished.target else { continue };
        let Ok((kind, pos)) = wrecks.get(wreck) else { continue };
        let def = roster.defs.get(kind.0);
        commands.revive(wreck, (def.hp + 1) / 2);
        if eyes.iter().any(|v| v.can_see(pos.0)) {
            tell.write(Tell::new(format!("The {} whirs back to life.", def.name), Tones::NOTICE));
        }
    }
}
// ANCHOR_END: rebuild

#[cfg(test)]
mod tests {
    use crate::droids::Roster;
    use crate::testing::{clear_droids, droid_down_a_lane, headless, settle};
    use bevy::ecs::world::CommandQueue;
    use bevy::prelude::*;
    use rl_engine::prelude::*;

    /// A line droid wrecked five cells east of the commando, and a repair
    /// drone two cells past it, on the cleared lane.
    fn a_wreck_and_a_drone() -> (App, Entity, Entity, Entity) {
        let mut app = headless(RunSeed(2));
        let (droid, player) = droid_down_a_lane(&mut app, "line droid", 5, 8);
        clear_droids(&mut app, &[droid]);
        strike(&mut app, droid, 1_000);
        // Damage lands in a turn pass, and the commando holds the turn.
        wait(&mut app, player);
        settle(&mut app);
        assert!(app.world().get::<Remains>(droid).is_some(), "a wreck");
        let registries = app.world().resource::<Registries>().clone();
        let roster = Roster::load(&registries);
        let at = app.world().get::<Position>(player).unwrap().0.offset(7, 0);
        let map = app.world().resource::<WorldMap>().current();
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, app.world_mut());
        let drone = crate::droids::spawn_monster(&mut commands, &roster, roster.defs.expect("repair drone"), at, map, &registries);
        queue.apply(app.world_mut());
        (app, player, droid, drone)
    }

    /// Real damage, through combat, so health is lost and a death is a
    /// death; `testing::hit` writes only what a reaction reads.
    fn strike(app: &mut App, target: Entity, amount: i32) {
        let kinetic = app.world().resource::<Registries>().damage_kinds.expect("kinetic");
        app.world_mut().write_message(DamageEvent::new(target, rl_engine::rl_rules::Hit::from_source(None, kinetic, amount)));
    }

    fn wait(app: &mut App, player: Entity) {
        if app.world().get::<MyTurn>(player).is_some() {
            app.world_mut().write_message(Intent::new(player, Wait));
        }
        app.update();
    }

    #[test]
    fn a_repair_drone_walks_to_a_wreck_of_its_own_side_and_stands_it_back_up() {
        let (mut app, player, droid, drone) = a_wreck_and_a_drone();
        let mut seen_working = false;
        for _ in 0..40 {
            wait(&mut app, player);
            seen_working |= app.world().get::<Working>(drone).is_some();
            if app.world().get::<Actor>(droid).is_some() {
                break;
            }
        }
        assert!(seen_working, "the drone set to work on it");
        let world = app.world();
        assert!(world.get::<Remains>(droid).is_none(), "the wreck is a droid again");
        assert_eq!(world.get::<Health>(droid).map(|h| (h.current, h.max)), Some((4, 8)), "at half its health, rounded up");
        assert_eq!(world.get::<Name>(droid).map(|n| n.as_str().to_string()), Some("line droid".into()));
    }

    #[test]
    fn a_repair_drone_shot_at_work_breaks_off_and_runs() {
        let (mut app, player, droid, drone) = a_wreck_and_a_drone();
        for _ in 0..20 {
            wait(&mut app, player);
            if app.world().get::<Working>(drone).is_some() {
                break;
            }
        }
        assert!(app.world().get::<Working>(drone).is_some(), "at work");
        let before = app.world().get::<Position>(drone).unwrap().0;
        // Already hurt, so whatever of the shot its chassis lets through
        // leaves it at or under the half it runs at; a drone shot while
        // still above that half breaks off and goes straight back to work.
        // And it knows where the commando is, since a drone runs from what
        // it knows of; shot from the dark by nobody it has noticed, it has
        // nothing to run from and goes back to work too.
        app.world_mut().get_mut::<Health>(drone).unwrap().current = 4;
        crate::testing::alert(&mut app, drone, player);
        strike(&mut app, drone, 3);
        wait(&mut app, player);
        assert!(app.world().get::<Working>(drone).is_none(), "shot, it stops");
        // It runs along the deck's flee map, round walls rather than in a
        // straight line, so what is checked is that it keeps running: off
        // the cell it worked from and never back at work.
        for turn in 1..=3 {
            wait(&mut app, player);
            assert!(app.world().get::<Working>(drone).is_none(), "turn {turn}: it runs rather than going back to work at half health");
        }
        assert_ne!(app.world().get::<Position>(drone).unwrap().0, before, "it left the cell it worked from");
        assert!(app.world().get::<Remains>(droid).is_some(), "the wreck stays a wreck");
    }
}
