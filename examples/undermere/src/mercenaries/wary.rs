//! A hired mercenary steps round what the adventurer has found underfoot.
//!
//! The engine's minds step onto any cell that is walkable and free, so a
//! follower walked onto a dart plate the adventurer had spotted and
//! stepped round. [`sense_shown_traps`] tells a mercenary where every
//! shown thing that goes off underfoot stands, and [`Wary`] wraps each of
//! its tactics so none steps onto one, save onto the adventurer's own
//! cell, which it follows them to. A thing still hidden is not shown to
//! the mercenary either: it knows what the adventurer knows.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_bevy::props::Hidden;
use rl_engine::rl_rules::ai::brain::Tactic;
use rl_engine::rl_rules::ai::tactics::{Led, Station};

use super::Mercenary;

/// Every cell on the mercenary's map holding a shown prop with a trigger
/// on being entered: a spotted trap, a glyph, a bone circle's bones.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShownUnderfoot(pub Vec<Point>);

/// Senses [`ShownUnderfoot`] for a mercenary holding the turn, and for
/// nobody else, whom [`Wary`] passes through untouched.
///
/// In `PerceiveSet::Annotate`, beside the engine's own senses.
pub fn sense_shown_traps(
    mut thinking: ResMut<Thinking>,
    hired: Query<&OnMap, With<Mercenary>>,
    props: Query<(&PropKind, &Position, &OnMap), Without<Hidden>>,
    registries: Res<Registries>,
) {
    let Some(actor) = thinking.actor() else { return };
    let Ok(on) = hired.get(actor) else { return };
    let cells = props
        .iter()
        .filter(|(kind, _, there)| there.0 == on.0 && registries.props.get(kind.0).triggers.iter().any(|t| t.on == "entered"))
        .map(|(_, at, _)| at.0)
        .collect();
    if let Some(snapshot) = thinking.snapshot_mut() {
        snapshot.add_sense(ShownUnderfoot(cells));
    }
}

/// The tactic it wraps, asked with every cell of [`ShownUnderfoot`] taken
/// for one it cannot step onto, but the one the adventurer stands on; and
/// a step it still chose onto one refused, leaving the turn to the
/// tactics after it.
///
/// Around the tactic rather than in the engine's own step check, which
/// knows nothing of what a game's props do underfoot.
#[derive(Debug, Clone, Copy)]
pub struct Wary<T>(pub T);

impl<T: Tactic<Entity>> Tactic<Entity> for Wary<T> {
    fn name(&self) -> &'static str {
        self.0.name()
    }

    fn evaluate(&self, ctx: &mut TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
        let snapshot = ctx.snapshot;
        let Some(shown) = snapshot.sense::<ShownUnderfoot>() else { return self.0.evaluate(ctx) };
        let leader = snapshot.sense::<Led>().map(|led| led.at);
        let shy = |p: Point| Some(p) != leader && shown.0.contains(&p);
        let can_step = ctx.can_step;
        let guarded = |p: Point| can_step(p) && !shy(p);
        let mut wary = TacticCtx {
            snapshot,
            fields: &mut *ctx.fields,
            can_step: &guarded,
            blocks_shot: ctx.blocks_shot,
            blocks_burst: ctx.blocks_burst,
            bounds: ctx.bounds,
            rng: &mut *ctx.rng,
        };
        match self.0.evaluate(&mut wary)? {
            Decision::Step(to) if shy(to) => None,
            decision => Some(decision),
        }
    }

    fn keeps_with(&self) -> Option<Station> {
        self.0.keeps_with()
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use rl_engine::rl_bevy::props::spawn_prop;

    use super::*;
    use crate::mercenaries::LEASH;
    use crate::mercenaries::fixtures::hired;
    use crate::testing::{arena, at, here, stand_on};

    /// A hired follower walking behind the adventurer down a row of dart
    /// plates they found and walked over never sets foot on one: it goes
    /// round, beside the row, and still keeps within its leash.
    #[test]
    fn a_follower_steps_round_the_traps_the_adventurer_has_found() {
        let (mut app, me, centre) = arena(2);
        let row: Vec<Point> = (-3..=4).map(|dx| centre.offset(dx, 0)).collect();
        let map = here(&app);
        // Found, as the adventurer finds a plate: shown, no longer hidden.
        for cell in row.clone() {
            let plate = app
                .world_mut()
                .run_system_once(move |mut commands: Commands, registries: Res<Registries>| {
                    spawn_prop(&mut commands, &registries, registries.props.expect("dart plate"), cell, map)
                })
                .expect("it runs");
            app.world_mut().entity_mut(plate).remove::<Hidden>();
        }
        stand_on(&mut app, me, centre.offset(-4, 0));
        let brenna = hired(&mut app, me, centre.offset(-5, 0), "Brenna");
        for cell in &row {
            let d = Direction::between(at(&app, me), *cell).expect("the next cell of the row");
            app.world_mut().write_message(Intent::new(me, Bump(d)));
            for _ in 0..2 {
                app.update();
                assert!(!row.contains(&at(&app, brenna)), "it stood on the plate at {:?}", at(&app, brenna));
            }
            assert_eq!(at(&app, me), *cell, "the adventurer walks the row");
        }
        for _ in 0..4 {
            app.world_mut().write_message(Intent::new(me, Wait));
            app.update();
            assert!(!row.contains(&at(&app, brenna)), "it stood on the plate at {:?}", at(&app, brenna));
        }
        assert!(geometry::chebyshev(at(&app, me), at(&app, brenna)) <= LEASH, "and it kept up");
    }
}
