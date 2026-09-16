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
