//! Where the mouse is, in cells.
//!
//! The grid is laid out for its window by [`fit`], so which cell a pixel
//! falls in is that arithmetic run backwards, and nothing here asks the
//! camera or a sprite. [`Pointer`] is the answer kept current once a frame:
//! the terminal cell under the cursor, and through a
//! [`MapView`] the map tile drawn there. What a click on a
//! cell means is the game's; the buttons are Bevy's own
//! `ButtonInput<MouseButton>`.

use bevy::prelude::*;
use rl_core::{Point, Rect};

use crate::layout::fit;
use crate::map_view::MapView;
use crate::terminal::Terminal;

/// The terminal cell the mouse is over.
///
/// `None` while the cursor is outside the window, over the margin round
/// the grid, or there is no window at all, as in a headless test, which
/// sets one with [`Pointer::at`].
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Pointer {
    cell: Option<Point>,
}

impl Pointer {
    /// A pointer over `cell`, for a test or a game that drives one itself.
    pub fn at(cell: Point) -> Self {
        Self { cell: Some(cell) }
    }

    /// The terminal cell under the cursor.
    pub fn cell(&self) -> Option<Point> {
        self.cell
    }

    /// The cell under the cursor when it is inside `rect`: how a panel
    /// asks whether the mouse is over it, and where.
    pub fn within(&self, rect: Rect) -> Option<Point> {
        self.cell.filter(|cell| rect.contains(*cell))
    }

    /// The map tile drawn under the cursor, when the cursor is over the
    /// map `view` draws. A tile of the map's void, past its edge, is still
    /// a tile: whether anything is there is the map's to say.
    pub fn tile(&self, view: &MapView) -> Option<Point> {
        self.cell.and_then(|cell| view.to_world(cell))
    }
}

/// Keeps [`Pointer`] on the cell under the cursor.
///
/// After the grid is laid out, so the cell is read off the layout the
/// frame will be drawn with. With no window the pointer is left as it is,
/// which is what lets a headless test place it by hand.
pub(crate) fn track_pointer(windows: Query<&Window, With<bevy::window::PrimaryWindow>>, terminal: Res<Terminal>, mut pointer: ResMut<Pointer>) {
    let Ok(window) = windows.single() else { return };
    let size = UVec2::new(window.physical_width(), window.physical_height());
    let cell = window.physical_cursor_position().filter(|_| size.x > 0 && size.y > 0).and_then(|at| {
        let fit = fit(terminal.width(), terminal.height(), terminal.cell_size(), size, window.scale_factor());
        fit.cell_at(at, terminal.width(), terminal.height())
    });
    let cell = cell.map(|(x, y)| Point::new(x, y));
    if pointer.cell != cell {
        pointer.cell = cell;
    }
}
