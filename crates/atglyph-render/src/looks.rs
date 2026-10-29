//! How tiles look, from the tiles themselves.
//!
//! A tile carries its look on its own record, a
//! [`TileLook`] of plain data written beside the
//! rest of the tile, in a file read by
//! [`atglyph_rules::tiles::load`] or with a builder in code. This turns
//! those looks into the renderer's colour table, so a tile is defined in
//! one place and read once.
//!
//! A look that names several glyphs keeps them all, and each cell draws
//! the one [`TileGlyphs::at`](atglyph_grid::TileGlyphs::at) picks for its
//! world position; under a [`Tileset`](crate::Tileset) each is its own
//! character, so it is its own picture where the sheet names one.
//!
//! Every tile must have a look: a tile with none would be drawn as a
//! magenta question mark, so the table is refused at startup, naming every
//! tile left without one, rather than the gap being found on the map.
//!
//! ```
//! use atglyph_render::TileAppearance;
//!
//! let tiles = atglyph_rules::tiles::load(
//!     r#"[
//!         (tile: "rock", opaque: true, glyph: '#', fg: (0.6, 0.6, 0.6), bg: (0.2, 0.2, 0.2), vary: (0.2, 0.05)),
//!         (tile: "moss", walkable: true, glyph: '.', fg: (0.4, 0.7, 0.3), shimmer: 0.1),
//!     ]"#,
//! )
//! .unwrap();
//! let look = TileAppearance::from_tiles(&tiles).unwrap();
//! assert_eq!(look.lit(tiles.expect("rock")).glyph, '#');
//! ```

use atglyph_grid::{TileLook, TileRegistry};
use atglyph_rules::ContentError;
use bevy::prelude::Color;

use crate::map_view::{Look, TileAppearance};
use crate::shade::Vary;
use crate::terminal::Cell;

/// A tile's look as the renderer draws it: the cell in full light, the
/// glyphs a cell picks among, and how it varies from cell to cell.
pub(crate) fn drawn(look: &TileLook) -> Look {
    let color = |(r, g, b): (f32, f32, f32)| Color::srgb(r, g, b);
    let mut cell = Cell::new(look.glyph.first(), color(look.fg));
    if let Some(bg) = look.bg {
        cell = cell.on(color(bg));
    }
    let vary = look.vary.map_or(Vary::NONE, |(brightness, hue)| Vary::new(brightness, hue)).shimmering(look.shimmer);
    Look { cell, glyphs: look.glyph.clone(), vary }
}

impl TileAppearance {
    /// The colour table for every tile in `tiles`, from each tile's look.
    ///
    /// Refuses, naming every tile at once, when any tile has no look.
    /// Memory and shading stay at their defaults, since they are the
    /// renderer's manner rather than any tile's look; a game that wants
    /// them changed sets them on what comes back.
    pub fn from_tiles(tiles: &TileRegistry) -> Result<Self, ContentError> {
        let mut out = TileAppearance::new();
        let mut errors = Vec::new();
        for (id, props) in tiles.iter() {
            match &props.look {
                Some(look) => out.put(id, drawn(look)),
                None => errors.push(format!("{}: has no look", props.name)),
            }
        }
        if errors.is_empty() { Ok(out) } else { Err(ContentError::Invalid(errors)) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atglyph_grid::TileProps;

    /// A tile's colours and spread come through to the table, and what its
    /// look leaves out takes the renderer's plain defaults.
    #[test]
    fn a_tiles_looks_fill_the_table_with_what_they_say_and_defaults_for_what_they_leave_out() {
        let mut tiles = TileRegistry::new();
        tiles.register(TileProps::wall("rock").look(TileLook::new('#', (0.6, 0.6, 0.6)).on((0.2, 0.2, 0.2)).vary(0.2, 0.05).shimmer(0.3))).unwrap();
        tiles.register(TileProps::floor("moss").look(TileLook::new('.', (0.4, 0.7, 0.3)))).unwrap();
        let look = TileAppearance::from_tiles(&tiles).unwrap();
        let rock = look.lit(tiles.expect("rock"));
        assert_eq!((rock.glyph, rock.fg, rock.bg), ('#', Color::srgb(0.6, 0.6, 0.6), Color::srgb(0.2, 0.2, 0.2)));
        let moss = look.lit(tiles.expect("moss"));
        assert_eq!((moss.glyph, moss.bg), ('.', Color::BLACK), "no fill given: black");
        // Shimmer shows as a changing colour over time; none was given for moss.
        let p = atglyph_core::Point::new(3, 4);
        assert_eq!(look.seen(tiles.expect("moss"), p, 0.0), look.seen(tiles.expect("moss"), p, 1.0), "moss sits still");
        assert_ne!(look.seen(tiles.expect("rock"), p, 0.0), look.seen(tiles.expect("rock"), p, 1.0), "rock shimmers");
    }

    /// A tile of several glyphs draws in each cell the one its world
    /// position picks, the same in every frame and in memory as in sight,
    /// with every glyph showing somewhere; its authored cell is the first.
    #[test]
    fn a_tile_of_several_glyphs_draws_each_cell_by_its_position_alike_in_sight_and_memory() {
        let tiles = atglyph_rules::tiles::load(r#"[(tile: "grass", walkable: true, glyph: ['"', '"', ','], fg: (0.3, 0.6, 0.2), shimmer: 0.4)]"#).unwrap();
        let grass = tiles.expect("grass");
        let look = TileAppearance::from_tiles(&tiles).unwrap();
        assert_eq!(look.lit(grass).glyph, '"');
        let patch: Vec<_> = (0..12).flat_map(|y| (0..12).map(move |x| atglyph_core::Point::new(x, y))).collect();
        for &p in &patch {
            let glyph = look.seen(grass, p, 0.0).glyph;
            assert_eq!(look.seen(grass, p, 7.5).glyph, glyph, "{p:?} keeps its glyph as time passes");
            assert_eq!(look.remembered(grass, p).glyph, glyph, "{p:?} is remembered as it was seen");
        }
        for glyph in ['"', ','] {
            assert!(patch.iter().any(|&p| look.seen(grass, p, 0.0).glyph == glyph), "{glyph:?} shows somewhere");
        }
    }

    /// Every tile left without a look is named at once.
    #[test]
    fn every_tile_without_a_look_is_reported_together() {
        let mut tiles = TileRegistry::new();
        tiles.register(TileProps::wall("rock")).unwrap();
        tiles.register(TileProps::floor("moss").look(TileLook::new('.', (0.4, 0.7, 0.3)))).unwrap();
        tiles.register(TileProps::floor("lava")).unwrap();
        let Err(ContentError::Invalid(errors)) = TileAppearance::from_tiles(&tiles) else { panic!("tiles without looks were drawn") };
        assert_eq!(errors, vec!["rock: has no look", "lava: has no look"]);
    }

    /// The standard tiles carry the engine's placeholder looks.
    #[test]
    fn the_standard_tiles_have_looks() {
        assert!(TileAppearance::from_tiles(&TileRegistry::standard()).is_ok());
    }
}
