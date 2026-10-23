//! What Undermere says of what holds a monster, in its own words: the
//! facet key the rail's list says each status under, and the word for a
//! sleeper.
//!
//! What a status does is the engine's, written on each status in
//! `content.rs`: `confused` stumbles every step, `discordant` names the
//! side its holder fights for, `fireproof` resists all of fire, and
//! `weakened` stacks to three. Each is kept on its holder every pass from
//! what it holds, so a status put back by a continued run does it again
//! with nothing saved for it but, for a discordant body, the side it goes
//! back to, which the engine saves too. The rail's saying of them is the
//! engine's as well, asked for with `NearbyPanel::statuses` in `main.rs`.

/// The facet key each status a monster holds is said under on the rail,
/// beside its name: `rat · paralysed (hunting)`.
pub const STATUS_FACET: &str = "status";

/// What a sleeper is doing, as the rail's activity words name it: not one
/// of the engine's tactics, so `main.rs` gives it a word of its own beside
/// theirs, `goblin archer (asleep)`.
pub const SLEEPING: &str = "asleep";

#[cfg(test)]
mod tests {
    use atglyph::prelude::*;
    use atglyph::ui::view::Alert;
    use bevy::prelude::*;

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
        app.add_plugins(NearbyViewPlugin).insert_resource(NearbyStatuses { key: STATUS_FACET, asleep: SLEEPING });
        let (held, free) = (still(&mut app, "goblin", here + Point::new(2, 0)), still(&mut app, "goblin", here + Point::new(-2, 0)));
        afflict(&mut app, held, PARALYSED);
        afflict(&mut app, free, "regenerating");
        assert_eq!(said_of(&app, held), Some(vec![PARALYSED.to_string()]), "the held goblin says what holds it");
        assert_eq!(said_of(&app, free), Some(vec![]), "and the other, regenerating, says nothing beside its name");
    }

    /// A goblin asleep is doing nothing but sleeping on the rail, muted as
    /// one that knows of nothing, with no facet saying so twice; one awake
    /// beside it is doing nothing of the kind.
    #[test]
    fn a_sleeper_in_sight_is_said_asleep_once_and_muted() {
        let (mut app, _, here) = arena(1);
        app.add_plugins(NearbyViewPlugin).insert_resource(NearbyStatuses { key: STATUS_FACET, asleep: SLEEPING });
        let (sleeper, waking) = (still(&mut app, "goblin", here + Point::new(2, 0)), still(&mut app, "goblin", here + Point::new(-2, 0)));
        afflict(&mut app, sleeper, crate::content::ASLEEP);
        let row = |app: &App, who: Entity| app.world().resource::<NearbyView>().actors.iter().find(|row| row.entity == who).cloned().expect("on the list");
        assert_eq!(said_of(&app, sleeper), Some(vec![]), "said once, as what it is doing, not beside its name too");
        assert_eq!((row(&app, sleeper).doing, row(&app, sleeper).alert), (Some(SLEEPING), Some(Alert::Unaware)));
        assert_eq!(row(&app, waking).doing, None, "awake, it is doing nothing of the kind");
    }
}
