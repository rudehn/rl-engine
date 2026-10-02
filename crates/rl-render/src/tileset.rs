//! A tileset: pictures drawn in place of glyphs.
//!
//! A tileset is to the terminal what a font is: it says how a character
//! looks. A game draws the same cells it always drew, and for each glyph
//! the tileset names, inside the part of the terminal it covers, the cell
//! shows a picture from a sheet instead of a letter, tinted by the cell's
//! own colour. So everything that already writes cells, the map, the
//! light on it, what is remembered, fire, gas, particles and cursors,
//! goes on working with no second drawing path, and a game that leaves
//! the tileset out is drawn in glyphs exactly as before.
//!
//! The picture is chosen by character, so two things that should look
//! different are given different characters, which a game drawn in glyphs
//! wants anyway. A character with no picture is drawn as its glyph, so a
//! sheet may cover as little of a game as it has pictures for.

use std::collections::BTreeMap;

use bevy::prelude::*;
use rl_core::{Point, Rect};

/// The pictures drawn in place of glyphs, and where.
///
/// Insert one and the terminal draws by it; remove it, or turn it
/// [`off`](Self::off), and the same cells are glyphs again. The sheet is
/// a grid of equal pictures, read by the `layout`; a picture is drawn the
/// size of a cell, so a game with square pictures declares square cells.
#[derive(Resource, Debug, Clone)]
pub struct Tileset {
    image: Handle<Image>,
    layout: Handle<TextureAtlasLayout>,
    /// The cells it covers; `None` is the whole terminal.
    region: Option<Rect>,
    /// The picture each character is drawn as, by its index in the sheet.
    /// Ordered, so a tileset is one thing whatever order it was built in.
    pictures: BTreeMap<char, usize>,
    on: bool,
}

impl Tileset {
    /// A tileset over the sheet `image`, cut up as `layout` says, covering
    /// the whole terminal and naming no character yet.
    pub fn new(image: Handle<Image>, layout: Handle<TextureAtlasLayout>) -> Self {
        Self { image, layout, region: None, pictures: BTreeMap::new(), on: true }
    }

    /// Only within `region`, in terminal cells: the map's rectangle, for a
    /// game whose panels stay text.
    pub fn within(mut self, region: Rect) -> Self {
        self.region = Some(region);
        self
    }

    /// Draws `glyph` as the picture at `index` of the sheet.
    pub fn with(mut self, glyph: char, index: usize) -> Self {
        self.pictures.insert(glyph, index);
        self
    }

    /// Draws each glyph of `pictures` as the picture beside it.
    pub fn with_all(mut self, pictures: impl IntoIterator<Item = (char, usize)>) -> Self {
        self.pictures.extend(pictures);
        self
    }

    /// Draws glyphs everywhere, until [`turned`](Self::turned) on again:
    /// the same game in letters, for a player who prefers them.
    pub fn off(mut self) -> Self {
        self.on = false;
        self
    }

    /// Turns the pictures on or off.
    pub fn turned(&mut self, on: bool) {
        self.on = on;
    }

    /// Whether pictures are being drawn.
    pub fn is_on(&self) -> bool {
        self.on
    }

    /// The sheet.
    pub fn image(&self) -> &Handle<Image> {
        &self.image
    }

    /// How the sheet is cut up.
    pub fn layout(&self) -> &Handle<TextureAtlasLayout> {
        &self.layout
    }

    /// The picture `glyph` is drawn as in terminal cell `cell`, or `None`
    /// when it is drawn as itself: the tileset is off, the cell is outside
    /// what it covers, or it has no picture for the character.
    pub fn picture(&self, glyph: char, cell: Point) -> Option<usize> {
        if !self.on || self.region.is_some_and(|region| !region.contains(cell)) {
            return None;
        }
        self.pictures.get(&glyph).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_glyph_is_a_picture_only_where_the_tileset_covers_and_only_when_it_names_one() {
        let mut tiles = Tileset::new(Handle::default(), Handle::default()).within(Rect::new(0, 1, 4, 3)).with('#', 7).with_all([('.', 2), ('@', 9)]);
        assert_eq!(tiles.picture('#', Point::new(1, 1)), Some(7));
        assert_eq!(tiles.picture('@', Point::new(3, 3)), Some(9));
        assert_eq!(tiles.picture('#', Point::new(1, 0)), None, "above what it covers");
        assert_eq!(tiles.picture('#', Point::new(4, 1)), None, "beside what it covers");
        assert_eq!(tiles.picture('x', Point::new(1, 1)), None, "a character it has no picture for is itself");
        tiles.turned(false);
        assert_eq!(tiles.picture('#', Point::new(1, 1)), None, "off, everything is a glyph");
        assert!(!tiles.is_on());
        let everywhere = Tileset::new(Handle::default(), Handle::default()).with('#', 1);
        assert_eq!(everywhere.picture('#', Point::new(900, 900)), Some(1), "with no region it covers the whole terminal");
    }
}
