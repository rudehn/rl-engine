//! What a status does that the engine's statuses do not: a confused body's
//! every step turned a random way, and fire immunity's hundred percent kept
//! on whoever holds it.
//!
//! The engine's statuses change stats, tick, hide and hold; the rest is a
//! game's, keyed by the status. Each here reads the status off its holder
//! every pass and does what it says while it lasts, so a status put back
//! by a continued run does it again with nothing saved for it.

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;

use crate::content::{CONFUSED, FIREPROOF};

/// The draws a confused body's steps are turned by: a stream of the run's
/// own, so a stumble never shifts a blow's roll or a cloud's spread.
#[derive(Resource)]
pub struct Stumbles(pub rand::rngs::StdRng);

impl rl_engine::rl_bevy::Stream for Stumbles {
    fn for_run(seed: rl_engine::rl_core::RunSeed) -> Self {
        Self(seed.rng(SeedDomain::new(b"undermere.confusion"), 0))
    }
}

/// Turns every step a confused body means to take a random way among those
/// open to it, the adventurer's walk and a mind's step alike: a walk into
/// someone it did not mean to strike strikes them.
///
/// Among the open ways, cells that can be walked on whoever stands there,
/// as Brogue turns a confused step: a way into a wall would be refused,
/// which for the adventurer costs nothing and would only ask again. A
/// body walled in on every side steps as it meant to. In
/// `DecideSet::Game`, the game's answer to what was chosen, after every
/// mind has decided and with the adventurer's walk already written, and
/// before anything resolves it, so the intent the resolver reads is the
/// one turned about; and by rewriting the intent where it stands rather
/// than replacing it, so nothing else that reads it reads the old way.
pub fn stumble(
    mut steps: MessageMutator<Intent<Step>>,
    mut walks: MessageMutator<Intent<Bump>>,
    bodies: Query<(&Afflicted, &Position)>,
    registries: Res<Registries>,
    map: Res<WorldMap>,
    mut rng: ResMut<Stumbles>,
) {
    let Some(status) = registries.statuses.id(CONFUSED) else { return };
    let mut way = |who: Entity| {
        let (held, at) = bodies.get(who).ok()?;
        if !held.0.has(status) {
            return None;
        }
        let open: Vec<Direction> = Direction::ALL.into_iter().filter(|d| map.is_walkable(at.0 + d.offset())).collect();
        (!open.is_empty()).then(|| open[rng.0.random_range(0..open.len())])
    };
    for step in steps.read() {
        if let Some(turned) = way(step.actor) {
            step.action.0 = turned;
        }
    }
    for walk in walks.read() {
        if let Some(turned) = way(walk.actor) {
            walk.action.0 = turned;
        }
    }
}

/// Marks a body whose resistances carry fire immunity's hundred percent,
/// so it is taken off again exactly once when the status ends.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Fireproofed;

/// What fire immunity resists of fire: all of it.
pub const FIREPROOF_PCT: i32 = 100;

/// A body as fire immunity reads it: its statuses, its own resistances,
/// and whether they carry fire immunity's share now.
type Proofing = (Entity, Option<&'static Afflicted>, Option<&'static mut Resists>, Has<Fireproofed>);

/// Keeps fire immunity on whoever holds `fireproof`: [`FIREPROOF_PCT`] of
/// fire added to their own resistances when it comes on, which the
/// engine's damage reads as any resistance, and taken off when it ends.
///
/// Every pass, in `TurnSet::React`. The engine does not save a body's own
/// resistances, and does put its statuses back, so a continued run's
/// fireproof drinker is made fireproof again here, once.
pub fn keep_fireproof(mut commands: Commands, registries: Res<Registries>, mut bodies: Query<Proofing, With<Actor>>) {
    let (Some(status), Some(fire)) = (registries.statuses.id(FIREPROOF), registries.damage_kinds.id("fire")) else { return };
    for (body, held, resists, marked) in &mut bodies {
        let holds = held.is_some_and(|a| a.0.has(status));
        let by = match (holds, marked) {
            (true, false) => FIREPROOF_PCT,
            (false, true) => -FIREPROOF_PCT,
            _ => continue,
        };
        match resists {
            Some(mut resists) => resists.0.add(fire, by),
            None => {
                let mut fresh = Resistances::new();
                fresh.add(fire, by);
                commands.entity(body).insert(Resists(fresh));
            }
        }
        if holds {
            commands.entity(body).insert(Fireproofed);
        } else {
            commands.entity(body).remove::<Fireproofed>();
        }
    }
}
