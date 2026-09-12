//! Field of view over the loaded window.

use bevy::prelude::*;
use rl_core::Grid2D;
use rl_grid::{BitGrid, fov};

use crate::components::{Position, RevealsMap, Viewshed};
use crate::knowledge::Knowledge;
use crate::lighting::{DarkSight, Lighting, gate};
use crate::world::{WorldMap, WorldRes};

/// A viewer: where it stands, what it sees, how far it sees unlit, and
/// whether its sight fills in the map.
type ViewerData = (&'static Position, &'static mut Viewshed, Option<&'static DarkSight>, Has<RevealsMap>);

/// Recomputes every stale viewshed and fills in what a revealer saw.
///
/// The only system that clears `Viewshed::dirty`. Every viewshed goes
/// stale together when an edit changes what blocks sight.
pub fn update_viewsheds(
    map: Res<WorldMap>,
    world: Option<Res<WorldRes>>,
    lighting: Option<Res<Lighting>>,
    mut knowledge: ResMut<Knowledge>,
    mut viewers: Query<ViewerData>,
    mut seen_epoch: Local<Option<u64>>,
) {
    let view = map.view();
    let (w, h) = (view.width(), view.height());
    if w == 0 || h == 0 {
        return;
    }
    let all_stale = *seen_epoch != Some(map.opacity_epoch());
    *seen_epoch = Some(map.opacity_epoch());
    let origin = map.window_tiles().origin();
    for (pos, mut viewshed, dark, reveals) in &mut viewers {
        if !viewshed.dirty && !all_stale {
            continue;
        }
        if viewshed.line.width() != w || viewshed.line.height() != h {
            viewshed.line = BitGrid::new(w, h);
            viewshed.visible = BitGrid::new(w, h);
        }
        viewshed.origin = origin;
        let local = pos.0 - origin;
        let range = viewshed.range;
        fov::compute(&view, local, range, &mut viewshed.line);
        match &lighting {
            Some(lighting) => gate(lighting, pos.0, dark.map(|d| d.0).unwrap_or(0), &mut viewshed),
            None => {
                let Viewshed { visible, line, .. } = &mut *viewshed;
                visible.copy_from(line);
            }
        }
        viewshed.dirty = false;
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
