//! What a status does that the engine's statuses do not: a confused body's
//! every step turned a random way, a discordant one's side changed for as
//! long as it lasts, and fire immunity's hundred percent kept on whoever
//! holds it; and, by [`note_statuses`], what holds a monster said beside
//! its name on the rail.
//!
//! The engine's statuses change stats, tick, hide and hold; the rest is a
//! game's, keyed by the status. Each here reads the status off its holder
//! every pass and does what it says while it lasts, so a status put back
//! by a continued run does it again with nothing saved for it.

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;

use crate::content::{CONFUSED, DISCORDANT, FIREPROOF};

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

/// A body fighting for the discordant side while its `discordant` lasts,
/// and the side it goes back to after.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Discordant {
    /// The side it fought for before.
    pub was: Faction,
}

/// Keeps every body holding `discordant` on the discordant side, at odds
/// with every side and with every other discordant body, and puts each
/// back on the side it was on the pass its discord ends.
///
/// The faction matrix is read live by every mind and every blow, so the
/// side is the whole of it. In `TurnSet::React`, so a body the scroll
/// turned this pass is turned before anyone decides again. A continued
/// run's discordant body, its status put back and its side its kind's, is
/// turned again here with its kind's side to go back to.
pub fn keep_discord(mut commands: Commands, registries: Res<Registries>, mut bodies: Query<(Entity, &Afflicted, &mut Faction, Option<&Discordant>)>) {
    let (Some(status), Some(side)) = (registries.statuses.id(DISCORDANT), registries.factions.id(DISCORDANT)) else { return };
    for (body, held, mut faction, turned) in &mut bodies {
        match (held.0.has(status), turned) {
            (true, None) => {
                commands.entity(body).insert(Discordant { was: *faction });
                *faction = Faction(side);
            }
            (false, Some(turned)) => {
                *faction = turned.was;
                commands.entity(body).remove::<Discordant>();
            }
            _ => {}
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

/// Says beside each monster on the rail's list of what is in sight what
/// has been done to it: every status on it that has a badge, by name, so
/// a rat in a cloud reads `rat · paralysed (hunting)` and what a thrown
/// potion or a zapped staff did is read off the screen, not only off the
/// log.
///
/// A game's word on a row, in `ViewSet::Annotate`, since the engine's list
/// says what a monster knows of the adventurer and nothing of what holds
/// it. A status with no badge, the king's regenerating, is what a monster
/// is rather than what happened to it, and stays unsaid, as the vitals
/// leave it off the adventurer's own badges.
pub fn note_statuses(mut nearby: ResMut<NearbyView>, mut facets: ResMut<Facets>, registries: Res<Registries>, held: Query<&Afflicted>) {
    for row in &mut nearby.actors {
        let Ok(held) = held.get(row.entity) else { continue };
        for active in held.iter().filter(|active| registries.statuses.get(active.id).badge.is_some()) {
            row.facets.push(facets.facet("status", registries.statuses.name(active.id)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::PARALYSED;
    use crate::testing::{arena, settle, still};

    /// Puts `status` on `who` for a while, as a cloud or a bolt would.
    fn afflict(app: &mut App, who: Entity, status: &str) {
        let status = app.world().resource::<Registries>().statuses.expect(status);
        app.world_mut().write_message(Afflict { target: who, status, turns: 10, by: None, held_by: None });
        settle(app);
    }

    /// What the rail's list says of `who` beside its name, or `None` while
    /// it is not on the list.
    fn said_of(app: &App, who: Entity) -> Option<Vec<String>> {
        let nearby = app.world().resource::<NearbyView>();
        nearby.actors.iter().find(|row| row.entity == who).map(|row| row.facets.iter().map(|f| f.text.clone()).collect())
    }

    /// A goblin in sight that a cloud holds reads as paralysed on the rail,
    /// one nothing holds reads by its name alone, and a status with no
    /// badge, what a monster is rather than what was done to it, stays
    /// unsaid.
    #[test]
    fn a_monster_in_sight_is_said_with_what_holds_it_and_nothing_else() {
        let (mut app, _, here) = arena(1);
        app.add_plugins(NearbyViewPlugin).add_systems(Update, note_statuses.in_set(ViewSet::Annotate));
        let (held, free) = (still(&mut app, "goblin", here + Point::new(2, 0)), still(&mut app, "goblin", here + Point::new(-2, 0)));
        afflict(&mut app, held, PARALYSED);
        afflict(&mut app, free, "regenerating");
        assert_eq!(said_of(&app, held), Some(vec![PARALYSED.to_string()]), "the held goblin says what holds it");
        assert_eq!(said_of(&app, free), Some(vec![]), "and the other, regenerating, says nothing beside its name");
    }
}
