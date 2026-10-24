//! The play screen most roguelikes draw, cut from a few numbers.
//!
//! A map on the left with the log along its bottom, and a rail down the
//! right: vitals at its head, what is worn under them, what is in sight
//! filling the middle, the company at its foot, and the one row that says
//! how to see the controls. Every game the engine has carried cut that
//! screen by hand from the same half-dozen numbers. [`RailLayout`] takes
//! the numbers and hands back every rectangle as a [`RailScreen`].
//!
//! A plain value, not a plugin and not a resource: a game keeps the cut in
//! a struct of its own, as it always did, and anything it floats over the
//! map it places itself from [`RailScreen::map`]. A section of no rows is
//! no section, so a game with no equipment asks for no gear rows.
//!
//! ```
//! # use bevy::prelude::*;
//! # use atglyph_ui::layout::RailLayout;
//! # use atglyph_ui::{UiPlugin, VitalsPanel};
//! let screen = RailLayout::new(100, 40, 30).gutter(1).log(4).vitals(8).gear(11).hint().target(10).cut();
//! let mut app = App::new();
//! app.add_plugins(UiPlugin);
//! screen.clear_blanks(&mut app);
//! app.add_plugins(screen.panels().set(VitalsPanel::new(screen.vitals).heading("Vitals")));
//! ```
//!
//! [`RailScreen::panels`] is the panels always on screen in the rail's
//! cuts, as a [`PluginGroup`] of the engine's own panels and nothing else:
//! a game sets any of them to its own configuration with
//! [`set`](bevy::app::PluginGroupBuilder::set), leaves one out with
//! [`disable`](bevy::app::PluginGroupBuilder::disable), and adds what is
//! drawn only while it is up beside it: the targeting box in
//! [`RailScreen::target`], the controls screen at [`RailScreen::controls`]
//! with its hint, and the screens over the map. Those stay the game's to
//! add because the order a game adds the panels with keys in is the order
//! the controls screen lists their keys in.

use atglyph_core::GridRect;
use atglyph_render::Terminal;
use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;

use crate::panel::{self, GearPanel, LogPanel, NearbyPanel, PartyPanel, VitalsPanel};
use crate::tone::Palette;

/// The numbers a play screen with a rail is cut from.
///
/// Every count is in cells and starts at nought but the rail's width, so a
/// game names only what its screen has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RailLayout {
    width: i32,
    height: i32,
    rail: i32,
    edge: i32,
    gutter: i32,
    across: i32,
    log: i32,
    vitals: i32,
    gear: i32,
    hint: bool,
    party: i32,
    party_gap: i32,
    target: i32,
}

impl RailLayout {
    /// A screen `width` by `height` cells with a rail `rail` cells wide
    /// down its right, and nothing cut from either side yet.
    pub fn new(width: i32, height: i32, rail: i32) -> Self {
        Self { width, height, rail, edge: 0, gutter: 0, across: 1, log: 0, vitals: 0, gear: 0, hint: false, party: 0, party_gap: 0, target: 0 }
    }

    /// Columns of nothing between the rail and the window's edge, taken
    /// from the rail, so a heading's count or a bar ends short of the glass.
    pub fn edge(mut self, columns: i32) -> Self {
        self.edge = columns;
        self
    }

    /// Columns of nothing between the map and the rail, taken from the
    /// left, so a wall on the map's edge does not run into the rail's words.
    pub fn gutter(mut self, columns: i32) -> Self {
        self.gutter = columns;
        self
    }

    /// The map drawn `cells` cells to a tile: the gutter widens by whatever
    /// the left would leave of a tile, since half a tile is no tile.
    pub fn tiles_across(mut self, cells: i32) -> Self {
        self.across = cells.max(1);
        self
    }

    /// Rows of log along the bottom of the left.
    pub fn log(mut self, rows: i32) -> Self {
        self.log = rows;
        self
    }

    /// Rows at the head of the rail for the vitals.
    pub fn vitals(mut self, rows: i32) -> Self {
        self.vitals = rows;
        self
    }

    /// Rows under the vitals for what is worn.
    pub fn gear(mut self, rows: i32) -> Self {
        self.gear = rows;
        self
    }

    /// The rail's last row, for the hint that opens the controls screen.
    pub fn hint(mut self) -> Self {
        self.hint = true;
        self
    }

    /// Rows at the foot of the rail, over the hint, for the company.
    pub fn party(mut self, rows: i32) -> Self {
        self.party = rows;
        self
    }

    /// Blank rows between what is in sight and the company, so the last
    /// row in sight never runs into the company's heading.
    pub fn party_gap(mut self, rows: i32) -> Self {
        self.party_gap = rows;
        self
    }

    /// Rows the targeting box takes over the bottom of what is in sight
    /// while the cursor is up: where the eye already is when choosing what
    /// to aim at, and never over the map, where it would hide the aim.
    pub fn target(mut self, rows: i32) -> Self {
        self.target = rows;
        self
    }

    /// Every rectangle of the screen.
    ///
    /// The rail first, its edge from it, then the gutter from what is left,
    /// the log from the bottom of that, and the map the rest. Down the
    /// rail, the vitals and the gear from its head; the hint, the company
    /// and the gap above it from its foot; what is in sight between; and
    /// the targeting box over the bottom of that.
    pub fn cut(self) -> RailScreen {
        let (left, rail) = panel::split_right(GridRect::new(0, 0, self.width, self.height), self.rail);
        let (rail, edge) = panel::split_right(rail, self.edge);
        let gutter = self.gutter + (left.width - self.gutter).rem_euclid(self.across);
        let (left, gutter) = panel::split_right(left, gutter);
        let (map, log) = panel::split_bottom(left, self.log);
        let (vitals, below) = panel::split_top(rail, self.vitals);
        let (gear, nearby) = panel::split_top(below, self.gear);
        let (nearby, hint) = panel::split_bottom(nearby, i32::from(self.hint));
        let (nearby, party) = panel::split_bottom(nearby, self.party);
        let (nearby, party_gap) = panel::split_bottom(nearby, self.party_gap);
        let target = over_bottom(nearby, self.target);
        RailScreen { map, log, gutter, rail, edge, vitals, gear, nearby, party_gap, party, hint, target }
    }
}

/// The bottom `rows` of `rect`, for a box drawn over the foot of another
/// while it is up.
pub(crate) fn over_bottom(rect: GridRect, rows: i32) -> GridRect {
    panel::split_bottom(rect, rows).1
}

/// A play screen cut by [`RailLayout`]. A section the layout gave no rows
/// is an empty rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RailScreen {
    /// The map: the left, less the gutter and the log.
    pub map: GridRect,
    /// The log along the bottom of the left.
    pub log: GridRect,
    /// The blank columns between the map and the rail.
    pub gutter: GridRect,
    /// The whole rail, less its edge.
    pub rail: GridRect,
    /// The blank columns between the rail and the window's edge.
    pub edge: GridRect,
    /// The vitals, at the rail's head.
    pub vitals: GridRect,
    /// What is worn, under the vitals.
    pub gear: GridRect,
    /// What is in sight: the rest of the rail.
    pub nearby: GridRect,
    /// The blank rows between what is in sight and the company.
    pub party_gap: GridRect,
    /// The company, over the hint.
    pub party: GridRect,
    /// The rail's last row.
    pub hint: GridRect,
    /// The targeting box, over the bottom of what is in sight.
    pub target: GridRect,
}

impl RailScreen {
    /// A box `width` by `height` centred across the map, `y` rows below
    /// its top: where a screen over the map usually stands.
    pub fn centred(&self, width: i32, y: i32, height: i32) -> GridRect {
        GridRect::new(self.map.x + (self.map.width - width) / 2, self.map.y + y, width, height)
    }

    /// Where the controls screen stands: over the map, two cells in from
    /// each of its edges.
    pub fn controls(&self) -> GridRect {
        self.map.inflate(-2)
    }

    /// The cuts that belong to no panel: the gutter, the gap above the
    /// company and the edge.
    pub(crate) fn blanks(&self) -> [GridRect; 3] {
        [self.gutter, self.party_gap, self.edge]
    }

    /// Paints `blanks` clear with the chrome each frame,
    /// since nothing else would ever paint them again.
    pub fn clear_blanks(&self, app: &mut App) {
        let blank = self.blanks();
        app.add_systems(
            Update,
            (move |mut terminal: ResMut<Terminal>, palette: Res<Palette>| blank.into_iter().for_each(|r| panel::clear(&mut terminal, r, &palette)))
                .in_set(atglyph_bevy::PresentSet::Chrome),
        );
    }

    /// The engine's panels always on screen, each in its own cut: vitals,
    /// gear, what is in sight, the company and the log. A section with no
    /// rows has no panel.
    pub fn panels(&self) -> RailPanels {
        RailPanels(*self)
    }
}

/// The panels always on screen in a [`RailScreen`], as a group of the
/// engine's own, in the order they stand: vitals, gear, what is in sight,
/// the company, the log.
///
/// Each is added as its constructor makes it, in its cut; a game replaces
/// one with its own configuration by
/// [`set`](bevy::app::PluginGroupBuilder::set), which panics on a panel
/// the group does not hold, and leaves one out by
/// [`disable`](bevy::app::PluginGroupBuilder::disable).
#[derive(Debug, Clone, Copy)]
pub struct RailPanels(RailScreen);

impl PluginGroup for RailPanels {
    fn build(self) -> PluginGroupBuilder {
        let s = self.0;
        let has = |r: GridRect| r.width > 0 && r.height > 0;
        let mut group = PluginGroupBuilder::start::<Self>().add(VitalsPanel::new(s.vitals));
        if has(s.gear) {
            group = group.add(GearPanel::new(s.gear));
        }
        group = group.add(NearbyPanel::new(s.nearby));
        if has(s.party) {
            group = group.add(PartyPanel::new(s.party));
        }
        group.add(LogPanel::new(s.log))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A rail with every section, in a screen whose left is an odd number
    /// of cells for a map two cells to a tile.
    #[test]
    fn every_section_is_cut_where_the_rail_says_and_the_map_is_whole_tiles() {
        let s = RailLayout::new(160, 45, 34).edge(1).gutter(1).tiles_across(2).log(5).vitals(9).gear(12).hint().party(10).party_gap(1).target(10).cut();
        assert_eq!(s.map, GridRect::new(0, 0, 124, 40));
        assert_eq!(s.map.width % 2, 0, "whole tiles");
        assert_eq!(s.gutter, GridRect::new(124, 0, 2, 45));
        assert_eq!(s.log, GridRect::new(0, 40, 124, 5));
        assert_eq!(s.edge, GridRect::new(159, 0, 1, 45));
        assert_eq!(s.vitals, GridRect::new(126, 0, 33, 9));
        assert_eq!(s.gear, GridRect::new(126, 9, 33, 12));
        assert_eq!(s.hint, GridRect::new(126, 44, 33, 1));
        assert_eq!(s.party, GridRect::new(126, 34, 33, 10));
        assert_eq!(s.party_gap, GridRect::new(126, 33, 33, 1));
        assert_eq!(s.nearby, GridRect::new(126, 21, 33, 12));
        assert_eq!(s.target, GridRect::new(126, 23, 33, 10));
    }

    /// What a layout leaves out is an empty rectangle, and the rest close
    /// up round it.
    #[test]
    fn a_section_given_no_rows_is_empty_and_the_rest_close_up() {
        let s = RailLayout::new(80, 40, 24).log(5).vitals(9).hint().cut();
        assert_eq!(s.map, GridRect::new(0, 0, 56, 35));
        assert_eq!((s.gear.height, s.party.height, s.target.height, s.gutter.width, s.edge.width), (0, 0, 0, 0, 0));
        assert_eq!(s.nearby, GridRect::new(56, 9, 24, 30));
        assert_eq!(s.hint, GridRect::new(56, 39, 24, 1));
    }

    #[test]
    fn a_box_centred_on_the_map_stands_at_its_middle() {
        let s = RailLayout::new(100, 40, 30).gutter(1).log(4).cut();
        assert_eq!(s.centred(40, 10, 6), GridRect::new(s.map.x + (s.map.width - 40) / 2, 10, 40, 6));
    }
}
