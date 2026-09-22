//! What the player can see, in the order a list reads it, and which one of
//! it the player has picked out.
//!
//! Three things point at something in sight: the nearby list, the look
//! cursor and the targeting cursor. A player who presses Tab in one expects
//! the order it steps in and the thing it lands on to be the order and the
//! thing in the others, so both are written once. [`InSight`] is the list,
//! actors nearest first and then things nearest first, which is the order
//! [`NearbyView`](crate::NearbyView) prints; [`Focus`] is the one entity
//! picked out of it, which the nearby panel highlights, the look cursor
//! opens on, and an aim opens on when the aim can take it.
//!
//! Focus is held by entity rather than by cell, so two things on one tile
//! are two stops. A focus on something that has since left sight is not an
//! error: every reader checks it against the list with [`Focus::within`]
//! and treats a stale one as none.

use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_core::{Point, geometry};
use rl_render::Glyph;

use crate::controls::{AddControls, EngineKey, Keys};
use crate::cursor::CursorInput;
use crate::modal::Modals;

/// The heading the cursors' keys are listed under on the controls screen.
pub const CURSOR_GROUP: &str = "Look and aim";
/// The heading the keys that open and close screens are listed under.
pub const SCREENS_GROUP: &str = "Screens";

/// Declares the keys that step the [`Focus`], in the words every plugin
/// that reads them uses, so the controls screen lists them once however
/// many plugins declare them.
pub(crate) fn declare_focus_controls(app: &mut App) {
    app.add_control(CURSOR_GROUP, "next thing in sight", EngineKey::Next);
    app.add_control(CURSOR_GROUP, "previous thing in sight", EngineKey::Previous);
    app.add_control(SCREENS_GROUP, "close a cursor or screen", EngineKey::Close);
}

/// [`declare_focus_controls`], and the keys that step a cursor.
pub(crate) fn declare_cursor_controls(app: &mut App) {
    declare_focus_controls(app);
    app.add_control(CURSOR_GROUP, "move the cursor", Keys::Directions { shift: false });
}

/// One entity the player can see.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sighting {
    /// Which entity.
    pub entity: Entity,
    /// Where it stands.
    pub at: Point,
    /// Whether it is an [`Actor`], the line between the nearby list's two
    /// headings.
    pub actor: bool,
    /// Chebyshev tiles from the player.
    pub distance: i32,
}

/// What a sighting is read off.
type Seen = (Entity, &'static Position, &'static Name, Option<&'static OnMap>, Has<Actor>);

/// What may be listed: something drawn, still alive, and not a prop
/// nobody has spotted.
type Sightable = (Without<Dead>, With<Glyph>, Without<Hidden>);

/// The list of what the player can see, borrowed as a system parameter.
///
/// Something needs a [`Name`] and a [`Glyph`] to be on it, the same rule
/// every row in a view follows, so the list never stops on a thing no
/// panel could name. A hidden prop is left out until it is spotted, which
/// is the whole of what hidden means.
#[derive(bevy::ecs::system::SystemParam)]
pub struct InSight<'w, 's> {
    map: Res<'w, WorldMap>,
    player: Query<'w, 's, (Entity, &'static Position, &'static Viewshed), With<Player>>,
    seen: Query<'w, 's, Seen, Sightable>,
}

impl InSight<'_, '_> {
    /// The player and the cell it stands on, when there is exactly one.
    pub fn viewer(&self) -> Option<(Entity, Point)> {
        self.player.single().ok().map(|(entity, pos, _)| (entity, pos.0))
    }

    /// Everything the player can see but itself, in list order: actors
    /// nearest first, then things nearest first.
    ///
    /// Ties go by name and then by entity, so two things at one distance do
    /// not trade places from one frame to the next, and Tab pressed twice
    /// from one place lands on the same second thing in two runs of a seed.
    pub fn list(&self) -> Vec<Sighting> {
        let Ok((me, origin, viewshed)) = self.player.single() else { return Vec::new() };
        let here = self.map.current();
        let mut found: Vec<(Sighting, &str)> = self
            .seen
            .iter()
            .filter(|(entity, pos, _, on, _)| *entity != me && on.map(|m| m.0).unwrap_or(MapId::SURFACE) == here && viewshed.can_see(pos.0))
            .map(|(entity, pos, name, _, actor)| (Sighting { entity, at: pos.0, actor, distance: geometry::chebyshev(origin.0, pos.0) }, name.as_str()))
            .collect();
        found.sort_by(|(a, a_name), (b, b_name)| (!a.actor, a.distance, *a_name, a.entity).cmp(&(!b.actor, b.distance, *b_name, b.entity)));
        found.into_iter().map(|(sighting, _)| sighting).collect()
    }
}

/// What the player can see, in list order, collected once a phase.
///
/// [`InSight::list`] is a full scan, two allocations and a sort, and five
/// systems used to ask for it: `browse` and each cursor while the keys are
/// read, and each view's collector while the frame is drawn. Nothing moves
/// within either phase, so one collection at the head of each serves every
/// reader in it and the answers cannot disagree.
///
/// Refilled twice a frame rather than once, because the two phases are two
/// different moments: the turns run between them, and a list collected
/// before the player's key was resolved is not the list the panels draw.
#[derive(Resource, Debug, Default)]
pub struct Sighted(Vec<Sighting>);

impl Sighted {
    /// What is in sight, actors nearest first and then things.
    pub fn list(&self) -> &[Sighting] {
        &self.0
    }
}

/// Refills [`Sighted`]. Runs at the head of the input phase and again at
/// the head of [`ViewSet::Collect`](crate::ViewSet::Collect).
pub fn collect_sighted(mut sighted: ResMut<Sighted>, sight: InSight) {
    sighted.0 = sight.list();
}

/// The one entity the player has picked out of what is in sight.
///
/// Stepped by Tab in the nearby list and by either cursor, and read by all
/// three, so a thing picked out in one is the thing the next one opens on.
/// It may name something that has since left sight: read it through
/// [`Focus::within`] rather than bare.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Focus(Option<Entity>);

impl Focus {
    /// What is picked out, whether or not it is still in sight.
    pub fn get(&self) -> Option<Entity> {
        self.0
    }

    /// Picks out `entity`, or nothing.
    pub fn set(&mut self, entity: Option<Entity>) {
        self.0 = entity;
    }

    /// Lets go of whatever was picked out.
    pub fn clear(&mut self) {
        self.0 = None;
    }

    /// The picked-out sighting, if it is one of `list`.
    pub fn within<'a>(&self, list: &'a [Sighting]) -> Option<&'a Sighting> {
        self.0.and_then(|entity| list.iter().find(|s| s.entity == entity))
    }

    /// Picks out the next of `list`, or the previous when `back`, and
    /// returns it. See [`cycle`] for where a step from nothing lands.
    pub fn step(&mut self, list: &[Sighting], back: bool) -> Option<Sighting> {
        let next = cycle(list, self.0, back).copied();
        self.0 = next.map(|s| s.entity);
        next
    }
}

/// The sighting after `from` in `list`, or before it when `back`, wrapping
/// round either end.
///
/// The first, or the last going back, when `from` is none of them: where a
/// fresh press should land, and where a cursor nudged off its target should
/// snap back to. `None` only when there is nothing to step through.
pub fn cycle(list: &[Sighting], from: Option<Entity>, back: bool) -> Option<&Sighting> {
    let n = list.len();
    if n == 0 {
        return None;
    }
    let i = match from.and_then(|entity| list.iter().position(|s| s.entity == entity)) {
        Some(i) if back => (i + n - 1) % n,
        Some(i) => (i + 1) % n,
        None if back => n - 1,
        None => 0,
    };
    list.get(i)
}

/// With nothing open, the cursor's "next" key steps [`Focus`] through what
/// is in sight, Shift steps it back, and the close key lets go of it.
///
/// Only with nothing open: inside a cursor the same keys move the cursor,
/// which moves the focus with it.
pub fn browse(input: CursorInput, modals: Res<Modals>, sighted: Res<Sighted>, mut focus: ResMut<Focus>) {
    if modals.any_open() {
        return;
    }
    if input.just_pressed(input.keys().next) {
        focus.step(sighted.list(), input.back());
    } else if input.just_pressed(input.keys().close) && focus.get().is_some() {
        focus.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use bevy::ecs::system::RunSystemOnce;

    /// Three sightings on distinct entities, one tile apart.
    fn three() -> Vec<Sighting> {
        let mut world = World::new();
        (1..=3).map(|x| Sighting { entity: world.spawn_empty().id(), at: Point::new(x, 0), actor: true, distance: x }).collect()
    }

    #[test]
    fn stepping_wraps_both_ways_and_a_step_from_nothing_lands_on_an_end() {
        let list = three();
        let [a, b, c] = [list[0].entity, list[1].entity, list[2].entity];
        let at = |s: Option<&Sighting>| s.map(|s| s.entity);
        assert_eq!(at(cycle(&list, Some(a), false)), Some(b));
        assert_eq!(at(cycle(&list, Some(c), false)), Some(a), "round the end");
        assert_eq!(at(cycle(&list, Some(a), true)), Some(c), "and back round the start");
        assert_eq!(at(cycle(&list, None, false)), Some(a), "a fresh press lands on the first");
        assert_eq!(at(cycle(&list, None, true)), Some(c), "and a fresh press going back on the last");
        let gone = World::new().spawn_empty().id();
        assert_eq!(at(cycle(&list[1..], Some(gone), false)), Some(b), "something no longer listed is nothing");
        assert_eq!(cycle(&[], Some(a), false), None);
    }

    #[test]
    fn a_focus_on_something_out_of_the_list_reads_as_none() {
        let list = three();
        let mut focus = Focus::default();
        assert_eq!(focus.step(&list, false).map(|s| s.entity), Some(list[0].entity));
        assert_eq!(focus.within(&list).map(|s| s.entity), Some(list[0].entity));
        assert_eq!(focus.within(&list[1..]), None, "left sight, so nothing is picked out");
    }

    #[test]
    fn the_list_is_actors_nearest_first_then_things_and_never_the_player() {
        let mut stage = Stage::new(crate::NearbyViewPlugin);
        let far = stage.actor("far one", 'f', 6, 0);
        let coin = stage.thing("a coin", '$', 1, 0);
        let near = stage.actor("near one", 'n', 2, 0);
        stage.actor("over the hill", 'o', 40, 40);
        stage.tick();
        let list = stage.app.world_mut().run_system_once(|sight: InSight| sight.list()).expect("the list ran");
        assert_eq!(list.iter().map(|s| s.entity).collect::<Vec<_>>(), vec![near, far, coin], "the coin is nearer but a thing");
        assert_eq!(list[0].distance, 2);
        assert!(list[0].actor && !list[2].actor);
    }
}
