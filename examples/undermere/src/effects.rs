//! Undermere's own effects: what its staffs, wands, potions and scrolls do
//! that the engine's `Harm`, `Mend`, `Inflict` and the rest do not.
//!
//! Each is a type registered with `add_effect`, by
//! [`add_game_effects`](crate::content::add_game_effects), that reads its
//! own arguments and refuses any it does not name, and says what it does
//! in its own words, which is what the bag shows under a thing once it is
//! known. They sit beside the engine's, and nothing that lands them can
//! tell the difference.
//!
//! [`moving`] is where someone stands: a blink, a bolt that sends its
//! target far off or pulls it close, a draught that sinks its drinker to
//! the floor below. [`bodies`] is who someone is: a foe made into another,
//! made into two, blades called up out of nothing, and a drinker given
//! more life. [`minds`] is whose side someone is on, and [`senses`] what is
//! seen: a dark to hide in.
//!
//! An effect that needs more of the world than [`EffectWorld`] holds, a
//! monster's kind or its health, queues a closure on its commands and does
//! its work there, as the scrolls of enchanting and identify do. One that
//! did nothing anybody could see says so with [`showed_nothing`], so the
//! log says nothing seemed to happen and what was used is not learned by
//! it: [`lore`](crate::lore) reads that before it makes a kind known.

pub mod bodies;
pub mod minds;
pub mod moving;
pub mod senses;

use bevy::prelude::*;
use rl_engine::prelude::*;

/// The things whose use, this pass, showed nothing anybody could see: a
/// potion of descent drunk on the deepest floor, a wand zapped at nobody.
///
/// Filled by the effects as they land and by the zaps weighed before they
/// do, and emptied by [`learn_by_use`](crate::lore::learn_by_use), which
/// says so in the log rather than making the thing's kind known.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct ShowedNothing(pub Vec<Entity>);

/// The thing a landing came from, when a thing's trigger landed it.
pub fn carrier(landing: &Landing) -> Option<Entity> {
    match landing.source {
        rl_engine::rl_bevy::Source::Trigger { on, .. } => Some(on),
        _ => None,
    }
}

/// Marks the thing `landing` came from as having shown nothing this pass.
/// Nothing for a landing no thing made.
pub fn showed_nothing(world: &mut World, landing: &Landing) {
    let Some(thing) = carrier(landing) else { return };
    let mut shown = world.get_resource_or_insert_with(ShowedNothing::default);
    if !shown.0.contains(&thing) {
        shown.0.push(thing);
    }
}

/// `name` as the subject of a line: `The goblin`, and a name that is said
/// as written, `the Drowned King`, with its first letter raised.
pub fn the(name: &str) -> String {
    let said = if name.starts_with(char::is_uppercase) || name.starts_with("the ") { name.to_string() } else { format!("the {name}") };
    let mut letters = said.chars();
    letters.next().map(|first| first.to_uppercase().chain(letters).collect()).unwrap_or_default()
}

/// Up to `count` cells nearest `at` that someone could step onto now, in a
/// fixed order: by distance, then row, then column, so where a conjured
/// blade appears never depends on how a query happened to run.
pub fn free_cells_near(map: &WorldMap, occupancy: &Occupancy, at: Point, count: usize) -> Vec<Point> {
    let mut found = Vec::new();
    for ring in 1..=FAR {
        let mut cells: Vec<Point> = geometry::square(at, ring).filter(|p| geometry::chebyshev(*p, at) == ring).collect();
        cells.sort_by_key(|p| (p.y, p.x));
        found.extend(cells.into_iter().filter(|p| map.is_walkable(*p) && !occupancy.is_occupied(*p)));
        if found.len() >= count {
            break;
        }
    }
    found.truncate(count);
    found
}

/// How far, in steps, a place counts as far off: what a scroll of
/// teleportation sends its reader at least, and the furthest a cell is
/// looked for beside someone.
pub const FAR: i32 = 20;
