//! What the mercenaries' tests share: a run in Mereholt, its candidates,
//! taking up an offer, and a mercenary hired on the spot.

use atglyph::prelude::*;
use atglyph::primitives::RunSeed;
use bevy::prelude::*;

use super::*;
use crate::testing::{adventurer, at, headless, monster, settle, stand_on};

/// A run begun in Mereholt, its first visit's candidates seated.
pub(super) fn in_mereholt(seed: u64) -> (App, Entity) {
    let mut app = headless(RunSeed(seed));
    let me = adventurer(&mut app);
    (app, me)
}

/// The candidates waiting, by where they stand.
pub(super) fn waiting(app: &mut App) -> Vec<Entity> {
    let world = app.world_mut();
    let mut found: Vec<(Point, Entity)> = world.query_filtered::<(&Position, Entity), With<Candidate>>().iter(world).map(|(p, e)| (p.0, e)).collect();
    found.sort();
    found.into_iter().map(|(_, e)| e).collect()
}

/// A free cell beside `who` the adventurer could stand on.
pub(super) fn beside(app: &mut App, who: Entity) -> Point {
    let (there, here) = (at(app, who), crate::testing::here(app));
    let world = app.world_mut();
    let taken: Vec<Point> = world.query_filtered::<&Position, With<Blocks>>().iter(world).map(|p| p.0).collect();
    let map = app.world().resource::<WorldMap>();
    (0..8).map(|i| there + Direction::from_index(i).offset()).find(|p| map.is_walkable_on(here, *p) && !taken.contains(p)).expect("ground beside them")
}

/// Stands the adventurer beside `who` and takes up `verb` on it, as the
/// offers screen's choice does.
pub(super) fn take(app: &mut App, me: Entity, who: Entity, verb: &str) {
    take_with(app, me, who, verb, None);
}

/// [`take`], with the thing the bag chose for an offer that chooses
/// first, as a pick in the bag takes it up.
pub(super) fn take_with(app: &mut App, me: Entity, who: Entity, verb: &str, thing: Option<Entity>) {
    let cell = beside(app, who);
    stand_on(app, me, cell);
    let verb = app.world().resource::<Verbs>().get(verb).expect("a verb");
    app.world_mut().write_message(Intent::new(me, Interact { target: who, verb, thing }));
    settle(app);
}

/// The verbs `who` offers, each with its price.
pub(super) fn offered(app: &App, who: Entity) -> Vec<(String, Option<u32>)> {
    let verbs = app.world().resource::<Verbs>();
    let offers = app.world().get::<Offers>(who).expect("offers");
    offers.0.iter().map(|o| (verbs.name(o.verb).to_string(), o.price)).collect()
}

/// A mercenary called `name` hired where it stands at `at` on the place
/// being read, as a candidate taken up is, with nothing in its pack: for a
/// test about how a hired one thinks, on a floor no candidate stands on.
pub(super) fn hired(app: &mut App, me: Entity, at: Point, name: &str) -> Entity {
    let merc = monster(app, MERCENARY, at);
    app.world_mut().entity_mut(merc).insert((Name::new(name.to_string()), Candidate { fee: 0 }));
    hire(app.world_mut(), merc, me);
    settle(app);
    merc
}
