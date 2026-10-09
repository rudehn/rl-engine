//! Field of view over the loaded window, for everyone who has one.
//!
//! The player and every mind carry a [`Viewshed`], cast by the same
//! function, so what a monster sees is worked out the way what the player
//! sees is: a shadowcast to its range, then the light. The frame's pass
//! here recasts whatever is stale once a frame; inside the turn loop the
//! minds recast the one holding the turn before it perceives, since dozens
//! of them move within one frame.

use bevy::prelude::*;
use rl_core::{Grid2D, Point};
use rl_grid::{BitGrid, fov};

use crate::components::{Position, RevealsMap, Viewshed};
use crate::knowledge::Knowledge;
use crate::lighting::{DarkSight, Lighting, gate};
use crate::minds::Perception;
use crate::world::{WorldMap, WorldRes};

/// A viewer: where it stands, what it sees, how far it sees unlit, how far
/// it sees at all when it is a mind, and whether its sight fills in the map.
type ViewerData = (&'static Position, &'static mut Viewshed, Option<&'static DarkSight>, Option<&'static Perception>, Has<RevealsMap>);

/// Whether `viewshed` needs casting again: it was marked, or what blocks
/// sight changed since it was cast.
pub fn is_stale(viewshed: &Viewshed, map: &WorldMap) -> bool {
    viewshed.dirty || viewshed.epoch != map.opacity_epoch()
}

/// Casts `viewshed` from `at` over the loaded window: the line to its
/// range, then what the light lets through. A mind's range is its
/// `Perception`. The one function that clears `dirty`.
pub fn cast(map: &WorldMap, lighting: Option<&Lighting>, at: Point, dark_sight: i32, perception: Option<i32>, viewshed: &mut Viewshed) {
    let view = map.view();
    let (w, h) = (view.width(), view.height());
    if w == 0 || h == 0 {
        return;
    }
    if viewshed.line.width() != w || viewshed.line.height() != h {
        viewshed.line = BitGrid::new(w, h);
        viewshed.visible = BitGrid::new(w, h);
    }
    if let Some(range) = perception {
        viewshed.range = range;
    }
    viewshed.origin = map.window_tiles().origin();
    if viewshed.sees_everywhere() {
        // Neither the shadowcast nor the light is asked: both bits of every
        // tile are set, so nothing that reads either can be told no.
        viewshed.line.fill();
        viewshed.visible.fill();
        viewshed.dirty = false;
        viewshed.epoch = map.opacity_epoch();
        return;
    }
    let local = at - viewshed.origin;
    let range = viewshed.range;
    fov::compute(&view, local, range, &mut viewshed.line);
    match lighting {
        Some(lighting) => gate(lighting, at, dark_sight, viewshed),
        None => {
            let Viewshed { visible, line, .. } = &mut *viewshed;
            visible.copy_from(line);
        }
    }
    viewshed.dirty = false;
    viewshed.epoch = map.opacity_epoch();
}

/// Recomputes every stale viewshed and fills in what a revealer saw.
pub fn update_viewsheds(
    map: Res<WorldMap>,
    world: Option<Res<WorldRes>>,
    lighting: Option<Res<Lighting>>,
    mut knowledge: ResMut<Knowledge>,
    mut viewers: Query<ViewerData>,
) {
    for (pos, mut viewshed, dark, perception, reveals) in &mut viewers {
        if !is_stale(&viewshed, &map) {
            continue;
        }
        cast(&map, lighting.as_deref(), pos.0, dark.map(|d| d.0).unwrap_or(0), perception.map(|p| p.0), &mut viewshed);
        if reveals {
            for p in viewshed.iter() {
                knowledge.mark(p);
                // Regions and sites are the surface's, and the world
                // graph is the only thing that knows either.
                if let Some(world) = &world
                    && map.current().is_surface()
                {
                    let region = world.region_of_tile(p);
                    knowledge.touch_region(region);
                    if let Some(site) = world.site_index_at(region) {
                        knowledge.discover_site(site);
                    }
                }
            }
        }
    }
}

/// Sight: every stale viewshed recomputed, and what a revealer saw
/// written into [`Knowledge`].
pub struct FovPlugin;

impl Plugin for FovPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_viewsheds.in_set(crate::plugin::EngineSet::Fov));
    }

    fn finish(&self, app: &mut App) {
        crate::plugin::depends_on::<crate::plugin::CorePlugin>(app, "FovPlugin");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::Player;
    use crate::plugin::headless_app;
    use crate::state::EngineState;

    /// An onlooker's viewshed is every tile of the window, the walls and
    /// what is behind them included, and what it sees is remembered.
    #[test]
    fn a_viewshed_that_sees_everywhere_sees_the_whole_window_through_every_wall() {
        let mut app = headless_app();
        app.add_plugins((FovPlugin, crate::world::StreamingPlugin));
        let start = crate::testing::surface(&mut app);
        let wall = rl_grid::TileRegistry::standard().expect("wall");
        let onlooker = app.world_mut().spawn((Player, Position(start), Viewshed::everywhere(), RevealsMap)).id();
        let walker = app.world_mut().spawn((Position(start), Viewshed::new(6))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        // A wall right beside both of them, with open ground behind it.
        app.world_mut().resource_mut::<WorldMap>().set_tile(start.offset(1, 0), wall);
        app.update();

        let world = app.world();
        let window = world.resource::<WorldMap>().window_tiles();
        let all = world.get::<Viewshed>(onlooker).expect("the onlooker");
        assert_eq!(all.visible.count(), (window.width * window.height) as usize, "every tile of the window");
        assert_eq!(all.line.count(), all.visible.count(), "in line as well as seen");
        let behind = start.offset(3, 0);
        assert!(all.can_see(behind), "what is behind the wall");
        assert!(!world.get::<Viewshed>(walker).expect("the walker").can_see(behind), "which an ordinary viewshed from the same cell does not see");
        assert!(world.resource::<Knowledge>().is_explored(Point::new(window.x, window.y)), "and the far corner is remembered");
    }
}
