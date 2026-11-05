//! Field of view over the loaded window, for everyone who has one.
//!
//! The player and every mind carry a [`Viewshed`], cast by the same
//! function, so what a monster sees is worked out the way what the player
//! sees is: a shadowcast to its range, then the light. The frame's pass
//! here recasts whatever is stale once a frame; inside the turn loop the
//! minds recast the one holding the turn before it perceives, since dozens
//! of them move within one frame.

use crate::app::EngineApp;
use atglyph_core::{Grid2D, GridRect, Point};
use atglyph_grid::{BitGrid, fov};
use bevy::prelude::*;

use crate::components::{Position, RevealsMap, Viewshed};
use crate::knowledge::Knowledge;
use crate::lighting::{DarkSight, Lighting, gate};
use crate::minds::Perception;
use crate::world::WorldMap;

/// A viewer: where it stands, what it sees, how far it sees unlit, how far
/// it sees at all when it is a mind, and whether its sight fills in the map.
type ViewerData = (&'static Position, &'static mut Viewshed, Option<&'static DarkSight>, Option<&'static Perception>, Has<RevealsMap>);

/// Whether `viewshed`, of a viewer at `at`, needs casting again: it was
/// marked, what blocks sight changed since it was cast, or the window
/// moved and its sight reaches ground the window did not hold both then
/// and now.
pub(crate) fn is_stale(viewshed: &Viewshed, map: &WorldMap, at: Point) -> bool {
    viewshed.dirty || viewshed.epoch != map.opacity_epoch() || !holds_sight(viewshed, map, at)
}

/// Whether the window holds all that `viewshed`, cast from `at`, reads,
/// as it held it when it was cast: the same window, or one that moved
/// with the square of the viewer's sight, cut to the place, in what it
/// held both before and after. Sight reads nothing past its range, and
/// the tiles a window holds both before and after are the same tiles, so
/// a cast over the new window would come out the same; the light on them
/// is the lighting's to judge, which marks the viewsheds it changes.
/// One that sees everywhere reads the whole window, so any move reaches it.
fn holds_sight(viewshed: &Viewshed, map: &WorldMap, at: Point) -> bool {
    let window = map.window_tiles();
    let over = viewshed.cast_over();
    if over == window {
        return true;
    }
    if viewshed.sees_everywhere() {
        return false;
    }
    let range = viewshed.range.max(0);
    let square = GridRect::from_corners(at.offset(-range, -range), at.offset(range, range));
    let (Some(sight), Some(both)) = (square.intersection(&map.place_bounds()), over.intersection(&window)) else { return false };
    within(sight, both)
}

/// Whether every cell of `inner` lies in `outer`.
pub(crate) fn within(inner: GridRect, outer: GridRect) -> bool {
    inner.x >= outer.x && inner.y >= outer.y && inner.right() <= outer.right() && inner.bottom() <= outer.bottom()
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
    viewshed.set_cast_over(map.window_tiles());
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
pub(crate) fn update_viewsheds(map: Res<WorldMap>, lighting: Option<Res<Lighting>>, mut knowledge: ResMut<Knowledge>, mut viewers: Query<ViewerData>) {
    for (pos, mut viewshed, dark, perception, reveals) in &mut viewers {
        if !is_stale(&viewshed, &map, pos.0) {
            continue;
        }
        cast(&map, lighting.as_deref(), pos.0, dark.map(|d| d.0).unwrap_or(0), perception.map(|p| p.0), &mut viewshed);
        if reveals {
            for p in viewshed.iter() {
                knowledge.mark(p);
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
        app.depends_on::<crate::plugin::CorePlugin>("FovPlugin");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::Player;
    use crate::state::EngineState;
    use crate::testing::headless_app;

    /// An onlooker's viewshed is every tile of the window, the walls and
    /// what is behind them included, and what it sees is remembered.
    #[test]
    fn a_viewshed_that_sees_everywhere_sees_the_whole_window_through_every_wall() {
        let mut app = headless_app();
        app.add_plugins(FovPlugin);
        let start = crate::testing::ground(&mut app);
        let wall = atglyph_grid::TileRegistry::standard().expect("wall");
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
    /// What a viewer at `at` seeing `range` sees and has a line to, in
    /// world tiles round it.
    fn sight(v: &Viewshed, at: Point, range: i32) -> (Vec<Point>, Vec<Point>) {
        let square = atglyph_core::GridRect::from_corners(at.offset(-range, -range), at.offset(range, range));
        (square.cells().filter(|p| v.can_see(*p)).collect(), square.cells().filter(|p| v.in_line(*p)).collect())
    }

    /// The player on the streamed test ground, with three viewers of six
    /// tiles: one in the middle of what stays loaded when the window moves
    /// a chunk east, one whose sight reaches the column that leaves, and
    /// one whose sight reaches the column that comes in; then the window
    /// moved. With `lit`, under the lighting.
    fn moved_window(lit: bool) -> (App, [(Entity, Point); 3]) {
        let mut app = headless_app();
        app.add_plugins(FovPlugin);
        if lit {
            app.add_plugins(crate::lighting::LightingPlugin);
        }
        let start = crate::testing::streamed_ground(&mut app);
        let player = app.world_mut().spawn((Player, crate::components::Actor, Position(start), Viewshed::new(6))).id();
        let spots = [start.offset(8, 0), start.offset(-6, 0), start.offset(26, 0)];
        let viewers = spots.map(|at| (app.world_mut().spawn((Position(at), Viewshed::new(6))).id(), at));
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        let window = app.world().resource::<WorldMap>().window_tiles();
        for (e, _) in viewers {
            assert_eq!(app.world().get::<Viewshed>(e).unwrap().origin, window.origin(), "each cast over the first window");
        }
        let slack = crate::testing::StreamedGround::layout().slack;
        app.world_mut().get_mut::<Position>(player).unwrap().0 = start.offset(crate::testing::STREAMED_CHUNK + slack + 1, 0);
        app.update();
        app.update();
        let moved = app.world().resource::<WorldMap>().window_tiles();
        assert_eq!(moved.x, window.x + crate::testing::STREAMED_CHUNK, "the window moved a chunk east");
        (app, viewers)
    }

    /// A window move recasts only the viewsheds it reaches: one whose
    /// sight lies in ground loaded before and after is kept as it was
    /// cast, over the window it was cast over, and sees exactly what a
    /// cast over the new window sees; one whose sight reaches ground that
    /// left or came in is cast again, and sees into what came in.
    #[test]
    fn a_window_move_recasts_only_the_viewsheds_whose_sight_it_reaches() {
        for lit in [false, true] {
            let (mut app, [(kept, kept_at), (left, _), (came, came_at)]) = moved_window(lit);
            let map = app.world().resource::<WorldMap>();
            let window = map.window_tiles();
            let viewshed = |e: Entity| app.world().get::<Viewshed>(e).unwrap();
            assert_ne!(viewshed(kept).origin, window.origin(), "lit {lit}: kept, cast over the window before");
            assert_eq!(viewshed(left).origin, window.origin(), "lit {lit}: the one reaching ground that left is cast again");
            assert_eq!(viewshed(came).origin, window.origin(), "lit {lit}: the one reaching ground that came in is cast again");
            assert!(viewshed(came).in_line(came_at.offset(6, 0)), "lit {lit}: and has a line into it");
            let mut fresh = Viewshed::new(6);
            cast(map, app.world().get_resource::<Lighting>(), kept_at, 0, None, &mut fresh);
            assert_eq!(sight(viewshed(kept), kept_at, 6), sight(&fresh, kept_at, 6), "lit {lit}: the kept one sees what a fresh cast sees");
            app.update();
            assert_ne!(app.world().get::<Viewshed>(kept).unwrap().origin, window.origin(), "lit {lit}: and stays kept");
        }
    }
}
