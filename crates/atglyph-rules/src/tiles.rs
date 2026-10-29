//! What a tile is and how it looks, read from a file.
//!
//! A game keeps everything about a tile on one line of one file: whether
//! it can be walked and seen through, what it opens into, how it burns,
//! the light it sheds, and its glyph, or the glyphs each cell picks among,
//! and colours. [`load`] reads the file
//! once into a [`TileRegistry`], with no renderer, so a game builds and
//! tests its map headless; the renderer builds its colour table from the
//! registry.
//!
//! ```
//! use atglyph_grid::{Glow, Rgb};
//!
//! let file = r#"[
//!     (tile: "rock", opaque: true, blocks_projectiles: true, glyph: '#', fg: (0.6, 0.6, 0.6)),
//!     (tile: "lava", walkable: true, light: (intensity: 200, radius: 3, color: (r: 255, g: 133, b: 38)), glyph: '~', fg: (1.0, 0.5, 0.1)),
//!     (tile: "grass", walkable: true, glyph: ['"', '"', ','], fg: (0.3, 0.6, 0.2)),
//! ]"#;
//! let tiles = atglyph_rules::tiles::load(file).unwrap();
//! assert!(tiles.get(tiles.expect("rock")).opaque);
//! assert!(tiles.get(tiles.expect("rock")).look.as_ref().is_some_and(|l| l.glyph == '#'));
//! let grass = tiles.get(tiles.expect("grass")).look.as_ref().unwrap();
//! assert_eq!(grass.glyph.as_slice(), ['"', '"', ',']);
//! assert_eq!(tiles.get(tiles.expect("lava")).light, Some(Glow::new(200, 3, Rgb::new(255, 133, 38))));
//! ```

use atglyph_grid::{TileProps, TileRegistry};

use crate::content::ContentError;

/// Reads a tiles file into a registry: one tile a line, in the file's
/// order, with every property and the look the line writes.
///
/// The full option space of a line: `tile`, its name, always; `walkable`,
/// `opaque` and `blocks_projectiles`, false when left out; `passable`, as
/// `walkable` when left out; `move_cost`, hundredths of a step, normal when
/// left out; `opens_to` and `closes_to`, a door's other self by name;
/// `burn`, `(catch_pct:, turns:, leaves:)`; `light`, a
/// [`Glow`](atglyph_grid::Glow); and its look, `glyph` and `fg`, `(r, g,
/// b)` in `0..=1`, with `bg`, `vary: (brightness, hue)` and `shimmer` when
/// wanted. `glyph` is one character, `'.'`, or a list of at least one,
/// `['"', '"', ',']`, each cell drawing the one its position picks and a
/// glyph listed twice picked twice as often
/// ([`TileGlyphs`](atglyph_grid::TileGlyphs)). Refuses the file, naming every problem at once, when it does not
/// parse, writes half a look, defines no tile, defines one twice, or names
/// in `opens_to`, `closes_to` or `burn` a tile it does not define.
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

#[cfg(test)]
mod tests {
    use super::*;
    use atglyph_grid::{Glow, Rgb, TileLook, TileProps};

    /// A line says everything about its tile, its look included, in the
    /// order the file writes them.
    #[test]
    fn a_tiles_file_defines_every_tile_it_lists_with_its_look() {
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
        assert_eq!(tiles.get(tiles.expect("rock")), &TileProps::wall("rock").look(TileLook::new('#', (0.6, 0.6, 0.6))));
        assert_eq!(tiles.get(tiles.expect("moss")), &TileProps::floor("moss").move_cost(150).look(TileLook::new('.', (0.4, 0.7, 0.3)).vary(0.3, 0.1)));
        assert_eq!(tiles.get(tiles.expect("shut")).opens_to.as_deref(), Some("open"));
        assert!(tiles.get(tiles.expect("shut")).opaque && tiles.get(tiles.expect("shut")).passable == Some(true));
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
        let half = load(r#"[(tile: "rock", glyph: '#')]"#);
        assert!(matches!(&half, Err(ContentError::Parse(e)) if e.contains("rock: a look needs a glyph and an fg")), "half a look: {half:?}");
        assert!(matches!(load("not ron"), Err(ContentError::Parse(_))));
    }

    /// `glyph` is one field of two forms: one character as ever, or a list
    /// a cell picks among, and a list of none is refused.
    #[test]
    fn a_glyph_is_written_as_one_character_or_as_a_list_of_them() {
        let tiles = load(
            r#"[
            (tile: "dirt", walkable: true, glyph: '.', fg: (0.6, 0.5, 0.4)),
            (tile: "grass", walkable: true, glyph: ['"', '"', ','], fg: (0.3, 0.6, 0.2)),
            (tile: "forest", opaque: true, glyph: ['♠', '♣'], fg: (0.1, 0.4, 0.1)),
        ]"#,
        )
        .unwrap();
        let glyphs = |name: &str| tiles.get(tiles.expect(name)).look.as_ref().expect("a look").glyph.as_slice().to_vec();
        assert_eq!(tiles.get(tiles.expect("dirt")), &TileProps::floor("dirt").look(TileLook::new('.', (0.6, 0.5, 0.4))));
        assert_eq!(glyphs("grass"), ['"', '"', ',']);
        assert_eq!(tiles.get(tiles.expect("forest")), &TileProps::named("forest").opaque(true).look(TileLook::new(['♠', '♣'], (0.1, 0.4, 0.1))));
        let none = load(r#"[(tile: "bare", glyph: [], fg: (1.0, 1.0, 1.0))]"#);
        assert!(matches!(&none, Err(ContentError::Parse(e)) if e.contains("at least one")), "no glyph at all: {none:?}");
    }
}
