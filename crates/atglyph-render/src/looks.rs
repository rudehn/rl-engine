//! How tiles look, read from a file.
//!
//! Every game describes its tiles twice: what each one is, in a
//! [`TileRegistry`], and how it looks, in a [`TileAppearance`]. Both can
//! come from one file, a line a tile: [`load`] reads what each tile is,
//! with no renderer, and this reads how it looks, so a game's tiles are a
//! file beside its monsters rather than the largest block of Rust in its
//! start, and a new tile is a line rather than a recompile.
//!
//! [`load`]: atglyph_rules::tiles::load
//!
//! The file is a list, one entry per tile, named by the tile's registered
//! name. What a tile is, `walkable`, `opaque` and the rest, is passed over
//! here. Every registered tile must be described: a tile with no look
//! would be drawn as a magenta question mark, and a file is checked at
//! load so the gap is reported at startup, by name, rather than found on
//! the map. Every unknown name and every tile left out is listed at once.
//!
//! A line may also say what light the tile sheds, a [`Glow`] written as
//! every light is written, because the file a game keeps a tile's colours
//! in is where its author looks for its glow too. The light is gameplay,
//! not drawing: [`light_tiles`] reads it out of the same file into the
//! [`TileRegistry`], where the engine's lighting finds it, with no
//! renderer, and [`TileAppearance::load`] reads the rest and passes the
//! light over.
//!
//! [`Glow`]: atglyph_grid::Glow
//! [`light_tiles`]: atglyph_rules::tiles::light_tiles
//!
//! ```
//! use atglyph_grid::{TileProps, TileRegistry};
//! use atglyph_render::TileAppearance;
//!
//! let mut tiles = TileRegistry::new();
//! tiles.register(TileProps::wall("rock")).unwrap();
//! tiles.register(TileProps::floor("moss")).unwrap();
//! let look = TileAppearance::load(
//!     r#"[
//!         (tile: "rock", glyph: '#', fg: (0.6, 0.6, 0.6), bg: (0.2, 0.2, 0.2), vary: (0.2, 0.05)),
//!         (tile: "moss", glyph: '.', fg: (0.4, 0.7, 0.3), bg: (0.1, 0.2, 0.1), vary: (0.3, 0.1), shimmer: 0.1),
//!     ]"#,
//!     &tiles,
//! )
//! .unwrap();
//! assert_eq!(look.lit(tiles.expect("rock")).glyph, '#');
//! assert!(TileAppearance::load(r#"[(tile: "rock", glyph: '#', fg: (0.6, 0.6, 0.6))]"#, &tiles).is_err(), "moss was left out");
//! ```

use atglyph_grid::TileRegistry;
use atglyph_rules::ContentError;
use bevy::prelude::Color;
use serde::Deserialize;

use crate::map_view::TileAppearance;
use crate::shade::Vary;
use crate::terminal::Cell;

/// One tile's look, as the file writes it.
///
/// The full option space: `tile` and `glyph` and `fg` always, `bg` black
/// when left out, `vary` none when left out, `shimmer` still when left
/// out, `light` dark when left out. Colours are `(r, g, b)` in `0..=1`,
/// the way the rest of a game's content writes them; a light is a
/// [`Glow`](atglyph_grid::Glow), written the way a prop's or a save's is,
/// colour in bytes, and is passed over here, since
/// [`light_tiles`](atglyph_rules::tiles::light_tiles) reads it.
#[derive(Debug, Clone, Deserialize)]
struct Look {
    /// The tile's registered name.
    tile: String,
    /// The character drawn.
    glyph: char,
    /// The glyph's colour in full light.
    fg: (f32, f32, f32),
    /// The cell's fill in full light.
    #[serde(default)]
    bg: Option<(f32, f32, f32)>,
    /// Cell-to-cell spread in brightness and hue.
    #[serde(default)]
    vary: Option<(f32, f32)>,
    /// Brightness drifting over time.
    #[serde(default)]
    shimmer: f32,
}

impl TileAppearance {
    /// Reads a list of looks from RON, one per registered tile.
    ///
    /// Refuses the file, naming every problem at once, when an entry names
    /// a tile that is not registered, two entries name the same tile, or a
    /// registered tile has no entry. Memory and shading stay at their
    /// defaults, since they are the renderer's manner rather than any
    /// tile's look; a game that wants them changed sets them on what comes
    /// back.
    pub fn load(text: &str, tiles: &TileRegistry) -> Result<Self, ContentError> {
        let options = ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
        let looks: Vec<Look> = options.from_str(text).map_err(|e| ContentError::Parse(e.to_string()))?;
        let mut out = TileAppearance::new();
        let mut errors = Vec::new();
        let mut described = vec![false; tiles.len()];
        for look in &looks {
            let Some(id) = tiles.id(&look.tile) else {
                errors.push(format!("{}: no tile registered under that name", look.tile));
                continue;
            };
            if std::mem::replace(&mut described[id.index()], true) {
                errors.push(format!("{}: described twice", look.tile));
                continue;
            }
            let color = |(r, g, b): (f32, f32, f32)| Color::srgb(r, g, b);
            let mut cell = Cell::new(look.glyph, color(look.fg));
            if let Some(bg) = look.bg {
                cell = cell.on(color(bg));
            }
            let vary = look.vary.map_or(Vary::NONE, |(brightness, hue)| Vary::new(brightness, hue)).shimmering(look.shimmer);
            out.set_varied(id, cell, vary);
        }
        for (id, props) in tiles.iter() {
            if !described[id.index()] {
                errors.push(format!("{}: registered, but the file gives it no look", props.name));
            }
        }
        if errors.is_empty() { Ok(out) } else { Err(ContentError::Invalid(errors)) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atglyph_grid::TileProps;

    fn tiles() -> TileRegistry {
        let mut tiles = TileRegistry::new();
        tiles.register(TileProps::wall("rock")).unwrap();
        tiles.register(TileProps::floor("moss")).unwrap();
        tiles
    }

    /// The file's colours and spread come through to the table, and what
    /// the file leaves out takes the renderer's plain defaults.
    #[test]
    fn a_look_file_fills_the_table_with_what_it_says_and_defaults_for_what_it_leaves_out() {
        let tiles = tiles();
        let look = TileAppearance::load(
            r#"[
                (tile: "rock", glyph: '#', fg: (0.6, 0.6, 0.6), bg: (0.2, 0.2, 0.2), vary: (0.2, 0.05), shimmer: 0.3),
                (tile: "moss", glyph: '.', fg: (0.4, 0.7, 0.3)),
            ]"#,
            &tiles,
        )
        .unwrap();
        let rock = look.lit(tiles.expect("rock"));
        assert_eq!((rock.glyph, rock.fg, rock.bg), ('#', Color::srgb(0.6, 0.6, 0.6), Color::srgb(0.2, 0.2, 0.2)));
        let moss = look.lit(tiles.expect("moss"));
        assert_eq!((moss.glyph, moss.bg), ('.', Color::BLACK), "no fill given: black");
        // Shimmer shows as a changing colour over time; none was given for moss.
        let p = atglyph_core::Point::new(3, 4);
        assert_eq!(look.seen(tiles.expect("moss"), p, 0.0), look.seen(tiles.expect("moss"), p, 1.0), "moss sits still");
        assert_ne!(look.seen(tiles.expect("rock"), p, 0.0), look.seen(tiles.expect("rock"), p, 1.0), "rock shimmers");
    }

    /// Every problem in the file is named at once: the unknown, the
    /// doubled and the missing.
    #[test]
    fn every_unknown_doubled_and_missing_tile_is_reported_together() {
        let tiles = tiles();
        let text = r#"[
            (tile: "rock", glyph: '#', fg: (0.6, 0.6, 0.6)),
            (tile: "rock", glyph: '%', fg: (0.6, 0.6, 0.6)),
            (tile: "lava", glyph: '~', fg: (1.0, 0.3, 0.0)),
        ]"#;
        let Err(ContentError::Invalid(errors)) = TileAppearance::load(text, &tiles) else { panic!("a bad file loaded") };
        assert_eq!(errors, vec!["rock: described twice", "lava: no tile registered under that name", "moss: registered, but the file gives it no look"]);
        assert!(matches!(TileAppearance::load("not ron", &tiles), Err(ContentError::Parse(_))));
    }

    /// A line that writes a light loads as a line that writes none: the
    /// light is the rules' to read, and the look is unchanged by it.
    #[test]
    fn a_look_file_with_lights_loads_and_its_looks_are_unchanged() {
        let tiles = tiles();
        let text = r#"[
            (tile: "rock", glyph: '#', fg: (0.6, 0.6, 0.6)),
            (tile: "moss", glyph: '.', fg: (0.4, 0.7, 0.3), light: (intensity: 200, radius: 2, color: (r: 102, g: 255, b: 217))),
        ]"#;
        let look = TileAppearance::load(text, &tiles).unwrap();
        assert_eq!(look.lit(tiles.expect("moss")).glyph, '.');
        assert_eq!(tiles.get(tiles.expect("moss")).light, None, "loading a look lights nothing");
    }
}
