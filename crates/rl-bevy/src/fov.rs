//! Field of view over the loaded window.

use bevy::prelude::*;
use rl_core::Grid2D;
use rl_grid::{BitGrid, fov};

use crate::components::{Position, RevealsMap, Viewshed};
use crate::knowledge::Knowledge;
use crate::world::{WorldMap, WorldRes};

/// Recomputes every stale viewshed and fills in what a revealer saw.
///
/// The only system that clears `Viewshed::dirty`.
pub fn update_viewsheds(
    map: Res<WorldMap>,
    world: Option<Res<WorldRes>>,
    mut knowledge: ResMut<Knowledge>,
    mut viewers: Query<(&Position, &mut Viewshed, Has<RevealsMap>)>,
) {
    let view = map.view();
    let (w, h) = (view.width(), view.height());
    if w == 0 || h == 0 {
        return;
    }
    let origin = map.window_tiles().origin();
    for (pos, mut viewshed, reveals) in &mut viewers {
        if !viewshed.dirty {
            continue;
        }
        if viewshed.visible.width() != w || viewshed.visible.height() != h {
            viewshed.visible = BitGrid::new(w, h);
        }
        viewshed.origin = origin;
        let local = pos.0 - origin;
        let range = viewshed.range;
        fov::compute(&view, local, range, &mut viewshed.visible);
        viewshed.dirty = false;
        if reveals {
            for p in viewshed.iter() {
                knowledge.mark(p);
                if let Some(world) = &world
                    && map.current().is_surface()
                    && let Some(site) = world.site_index_at(world.region_of_tile(p))
                {
                    knowledge.discover_site(site);
                }
            }
        }
    }
}
