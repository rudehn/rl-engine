//! What a tile sheds, read from the file that says how it looks.
//!
//! A game keeps a tile's colours and its glow on one line, because the
//! file its author looks in for a tile's colours is where they look for its
//! glow too. The colours are drawing and the glow is gameplay, which
//! lighting and stealth read, so the two are read apart: the renderer's
//! loader takes the look and [`light_tiles`] takes the light, here, so a
//! game reads its tiles' light without a renderer.
//!
//! ```
//! use atglyph_grid::{Glow, Rgb, TileProps, TileRegistry};
//!
//! let mut tiles = TileRegistry::new();
//! let lava = tiles.register(TileProps::floor("lava")).unwrap();
//! let file = r#"[(tile: "lava", glyph: '~', fg: (1.0, 0.5, 0.1), light: (intensity: 200, radius: 3, color: (r: 255, g: 133, b: 38)))]"#;
//! atglyph_rules::tiles::light_tiles(file, &mut tiles).unwrap();
//! assert_eq!(tiles.get(lava).light, Some(Glow::new(200, 3, Rgb::new(255, 133, 38))));
//! ```

use atglyph_grid::{Glow, TileRegistry};
use serde::Deserialize;

use crate::content::ContentError;

/// The part of one line of a tiles file that is gameplay.
///
/// The full option space of the line is the renderer's: `tile`, `glyph`
/// and `fg` always, `bg`, `vary`, `shimmer` and `light` when wanted. Only
/// `tile` and `light`, dark when left out, are read here; every other field
/// is the look's and is passed over, so the file stays one file.
#[derive(Debug, Clone, Deserialize)]
struct TileLight {
    /// The tile's registered name.
    tile: String,
    /// The light the tile sheds, a [`Glow`] written as every light is.
    #[serde(default)]
    light: Option<Glow>,
}

/// Reads a tiles file's lights into `tiles`: every line that writes a
/// `light` gives its tile that light, and every line that writes none
/// leaves its tile as it was, so a light registered in code stands unless
/// the file says otherwise.
///
/// Called where the game builds its registry, before the tables are taken
/// from it, since the tables are what the map and its lighting read. A
/// separate read rather than a part of the renderer's look loader, because
/// the look is wanted wherever something draws and the light only once, on
/// a registry still being filled, and a headless game has no renderer.
/// Refuses the file, naming every problem at once, when it does not parse
/// or names a tile that is not registered; whether every tile has a line is
/// the look's check.
pub fn light_tiles(text: &str, tiles: &mut TileRegistry) -> Result<(), ContentError> {
    let options = ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
    let lines: Vec<TileLight> = options.from_str(text).map_err(|e| ContentError::Parse(e.to_string()))?;
    let mut errors = Vec::new();
    for line in &lines {
        match tiles.id(&line.tile) {
            Some(id) if line.light.is_some() => tiles.set_light(id, line.light),
            Some(_) => {}
            None => errors.push(format!("{}: no tile registered under that name", line.tile)),
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(ContentError::Invalid(errors)) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atglyph_grid::{Rgb, TileProps};

    fn tiles() -> TileRegistry {
        let mut tiles = TileRegistry::new();
        tiles.register(TileProps::wall("rock")).unwrap();
        tiles.register(TileProps::floor("moss")).unwrap();
        tiles
    }

    /// A line's light lands on its tile, a line without one leaves its
    /// tile dark, and the look's fields are passed over.
    #[test]
    fn a_tiles_files_lights_land_on_the_registry() {
        let mut tiles = tiles();
        let text = r#"[
            (tile: "rock", glyph: '#', fg: (0.6, 0.6, 0.6), bg: (0.2, 0.2, 0.2), vary: (0.2, 0.05), shimmer: 0.1),
            (tile: "moss", glyph: '.', fg: (0.4, 0.7, 0.3), light: (intensity: 200, radius: 2, color: (r: 102, g: 255, b: 217))),
        ]"#;
        light_tiles(text, &mut tiles).unwrap();
        assert_eq!(tiles.get(tiles.expect("moss")).light, Some(Glow::new(200, 2, Rgb::new(102, 255, 217))));
        assert_eq!(tiles.get(tiles.expect("rock")).light, None, "a line with no light sheds none");
    }

    /// A light on a name the registry does not hold is refused by that
    /// name, every one at once, and a file that does not parse is a parse
    /// error.
    #[test]
    fn a_light_on_a_tile_never_registered_is_reported_by_name() {
        let mut tiles = tiles();
        let text = r#"[
            (tile: "lava", glyph: '~', fg: (1.0, 0.3, 0.0), light: (intensity: 200, radius: 3, color: (r: 255, g: 133, b: 38))),
            (tile: "rock", glyph: '#', fg: (0.6, 0.6, 0.6)),
            (tile: "ember", glyph: ',', fg: (1.0, 0.3, 0.0)),
        ]"#;
        let Err(ContentError::Invalid(errors)) = light_tiles(text, &mut tiles) else { panic!("an unknown tile was lit") };
        assert_eq!(errors, vec!["lava: no tile registered under that name", "ember: no tile registered under that name"]);
        assert!(matches!(light_tiles("not ron", &mut tiles), Err(ContentError::Parse(_))));
    }
}
