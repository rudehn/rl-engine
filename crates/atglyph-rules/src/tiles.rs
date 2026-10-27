//! What a tile is, read from the file that says how it looks.
//!
//! A game keeps everything about a tile on one line of one file: whether
//! it can be walked and seen through, what it opens into, how it burns,
//! the light it sheds, and its colours. The colours are drawing and the
//! rest is gameplay, so the line is read twice: [`load`] takes what the
//! tile is, here, so a game builds its map without a renderer, and the
//! renderer's `TileAppearance::load` takes the look. Each passes over the
//! other's fields.
//!
//! A game that registers its tiles in code and keeps only their looks and
//! lights in a file reads the lights with [`light_tiles`].
//!
//! ```
//! use atglyph_grid::{Glow, Rgb};
//!
//! let file = r#"[
//!     (tile: "rock", opaque: true, blocks_projectiles: true, glyph: '#', fg: (0.6, 0.6, 0.6)),
//!     (tile: "lava", walkable: true, light: (intensity: 200, radius: 3, color: (r: 255, g: 133, b: 38)), glyph: '~', fg: (1.0, 0.5, 0.1)),
//! ]"#;
//! let tiles = atglyph_rules::tiles::load(file).unwrap();
//! assert!(tiles.get(tiles.expect("rock")).opaque);
//! assert_eq!(tiles.get(tiles.expect("lava")).light, Some(Glow::new(200, 3, Rgb::new(255, 133, 38))));
//! ```

use atglyph_grid::{Glow, TileProps, TileRegistry};
use serde::Deserialize;

use crate::content::ContentError;

/// Reads a tiles file into a registry: one tile a line, in the file's
/// order, with every property the line writes and the rest off.
///
/// The full option space of a line, besides its look: `tile`, its name,
/// always; `walkable`, `opaque` and `blocks_projectiles`, false when left
/// out; `passable`, as `walkable` when left out; `move_cost`, hundredths of
/// a step, normal when left out; `opens_to` and `closes_to`, a door's other
/// self by name; `burn`, `(catch_pct:, turns:, leaves:)`; and `light`, a
/// [`Glow`]. Refuses the file, naming every problem at once, when it does
/// not parse, defines no tile, defines one twice, or names in `opens_to`,
/// `closes_to` or `burn` a tile it does not define. Whether every tile has
/// a look is the look loader's check.
pub fn load(text: &str) -> Result<TileRegistry, ContentError> {
    let options = ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
    let lines: Vec<TileProps> = options.from_str(text).map_err(|e| ContentError::Parse(e.to_string()))?;
    if lines.is_empty() {
        return Err(ContentError::Invalid(vec!["the file defines no tiles".into()]));
    }
    let mut tiles = TileRegistry::new();
    let mut errors = Vec::new();
    for line in lines {
        let name = line.name.clone();
        if tiles.register(line).is_err() {
            errors.push(format!("{name}: defined twice"));
        }
    }
    for (_, tile) in tiles.iter() {
        let named =
            [("opens_to names", tile.opens_to.as_ref()), ("closes_to names", tile.closes_to.as_ref()), ("burn leaves", tile.burn.as_ref().map(|b| &b.leaves))];
        for (what, other) in named {
            if let Some(other) = other.filter(|o| tiles.id(o).is_none()) {
                errors.push(format!("{}: {what} {other:?}, which the file does not define", tile.name));
            }
        }
    }
    if errors.is_empty() { Ok(tiles) } else { Err(ContentError::Invalid(errors)) }
}

/// The part of one line of a tiles file that is its light.
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

    /// A line says everything the engine needs about its tile, in the
    /// order the file writes them, and the look's fields are passed over.
    #[test]
    fn a_tiles_file_defines_every_tile_it_lists_and_skips_the_looks() {
        let text = r#"[
            (tile: "rock", opaque: true, blocks_projectiles: true, glyph: '#', fg: (0.6, 0.6, 0.6)),
            (tile: "moss", walkable: true, move_cost: 150, glyph: '.', fg: (0.4, 0.7, 0.3), vary: (0.3, 0.1)),
            (tile: "shut", passable: true, opaque: true, opens_to: "open", glyph: '+', fg: (0.5, 0.3, 0.1)),
            (tile: "open", walkable: true, closes_to: "shut", glyph: '/', fg: (0.5, 0.3, 0.1)),
            (tile: "brush", walkable: true, burn: (catch_pct: 30, turns: 3, leaves: "moss"), glyph: '"', fg: (0.3, 0.6, 0.2)),
            (tile: "lava", light: (intensity: 200, radius: 3, color: (r: 255, g: 133, b: 38)), glyph: '~', fg: (1.0, 0.5, 0.1)),
        ]"#;
        let tiles = load(text).unwrap();
        let names: Vec<&str> = tiles.iter().map(|(_, t)| t.name.as_str()).collect();
        assert_eq!(names, ["rock", "moss", "shut", "open", "brush", "lava"], "registered in the file's order");
        assert_eq!(tiles.get(tiles.expect("rock")), &TileProps::wall("rock"));
        assert_eq!(tiles.get(tiles.expect("moss")), &TileProps::floor("moss").move_cost(150));
        assert_eq!(tiles.get(tiles.expect("shut")), &TileProps::named("shut").passable(true).opaque(true).opens_to("open"));
        assert_eq!(tiles.get(tiles.expect("brush")).burn.as_ref().map(|b| b.leaves.as_str()), Some("moss"));
        assert_eq!(tiles.get(tiles.expect("lava")).light, Some(Glow::new(200, 3, Rgb::new(255, 133, 38))));
    }

    /// A tile written twice, and a door or a fire naming a tile the file
    /// does not have, are refused by name, every one at once.
    #[test]
    fn a_tiles_file_with_a_doubled_tile_or_a_missing_name_is_refused_whole() {
        let text = r#"[
            (tile: "rock", opaque: true, glyph: '#', fg: (0.6, 0.6, 0.6)),
            (tile: "rock", glyph: '#', fg: (0.6, 0.6, 0.6)),
            (tile: "shut", opens_to: "ajar", glyph: '+', fg: (0.5, 0.3, 0.1)),
            (tile: "open", walkable: true, closes_to: "closed", glyph: '/', fg: (0.5, 0.3, 0.1)),
            (tile: "brush", burn: (catch_pct: 30, turns: 3, leaves: "ash"), glyph: '"', fg: (0.3, 0.6, 0.2)),
        ]"#;
        let Err(ContentError::Invalid(errors)) = load(text) else { panic!("a broken file was taken") };
        assert_eq!(
            errors,
            vec![
                "rock: defined twice",
                "shut: opens_to names \"ajar\", which the file does not define",
                "open: closes_to names \"closed\", which the file does not define",
                "brush: burn leaves \"ash\", which the file does not define",
            ]
        );
        assert!(matches!(load("[]"), Err(ContentError::Invalid(_))), "a file of no tiles is refused");
        assert!(matches!(load("not ron"), Err(ContentError::Parse(_))));
    }
}
