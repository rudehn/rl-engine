//! The typeface Undermere is drawn in: DejaVu Sans Mono, the unmodified
//! file from the DejaVu fonts' 2.37 release, kept in `assets/fonts/` beside
//! its licence and compiled in as the content is.
//!
//! Without a face of its own the terminal draws in the system's monospace
//! family: Courier on macOS, another face on Linux, and only ASCII in a
//! browser. The game would look different on each, and a glyph a face
//! lacked, a rule, a shade or a symbol, would be drawn as an empty box.
//! DejaVu Sans Mono may be bundled with a game, has every box rule, block
//! shade, arrow and symbol the engine's panels and the game draw with, and
//! stays clear at the size a cell allows.
//!
//! The tests hold every glyph the game draws to the face's own character
//! map: the content's here, the title's picture in [`title`](crate::title),
//! and every screen a run passes through in `main.rs`.

/// DejaVu Sans Mono's file, which `RoguelikePlugins::typeface` draws every
/// glyph in.
pub const DEJAVU_SANS_MONO: &[u8] = include_bytes!("../assets/fonts/DejaVuSansMono.ttf");

/// The typeface as a font, read for its character map.
#[cfg(test)]
pub(crate) fn face() -> ttf_parser::Face<'static> {
    ttf_parser::Face::parse(DEJAVU_SANS_MONO, 0).expect("the typeface is a font")
}

#[cfg(test)]
mod tests {
    use atglyph::prelude::all::*;
    use atglyph::primitives::RunSeed;
    use atglyph::render::map_view::FieldAppearance;
    use bevy::prelude::*;

    use crate::armory::Armory;
    use crate::bestiary::Bestiary;
    use crate::dungeon::Undermere;

    /// Every glyph Undermere's content may be drawn with, each beside what
    /// it is the glyph of: every tile, prop, monster and item as its file
    /// writes it, the look of every blow, shot, use and ability, each
    /// status's badge, the stairs, and the fire, gas and bursts the engine
    /// draws for the game.
    fn content(app: &App) -> Vec<(char, String)> {
        let world = app.world();
        let mut glyphs = Vec::new();
        let undermere = Undermere::new(RunSeed(7));
        let looks = undermere.appearance();
        glyphs.extend(undermere.tiles().iter().map(|(id, tile)| (looks.lit(id).glyph, format!("the tile {}", tile.name))));
        let registries = world.resource::<Registries>();
        for (_, prop) in registries.props.iter() {
            glyphs.push((prop.look.glyph, format!("the prop {}", prop.name)));
            glyphs.extend(prop.triggers.iter().filter_map(|t| t.look).map(|l| (l.glyph, format!("what the prop {} sets off", prop.name))));
        }
        glyphs.extend(registries.statuses.iter().filter_map(|(_, s)| s.badge.map(|b| (b, format!("the status {}", s.name)))));
        for (_, monster) in world.resource::<Bestiary>().defs.iter() {
            glyphs.push((monster.glyph, format!("the monster {}", monster.name)));
            let blows = [monster.attack.look, monster.ranged.and_then(|r| r.look)];
            glyphs.extend(blows.into_iter().flatten().map(|l| (l.glyph, format!("{}'s blow", atglyph::primitives::noun::with_the(&monster.name)))));
        }
        for (_, item) in world.resource::<Armory>().defs.iter() {
            glyphs.push((item.glyph, format!("the item {}", item.name)));
            let blows = [item.melee.as_ref().and_then(|m| m.look), item.ranged.as_ref().and_then(|r| r.look)];
            glyphs.extend(blows.into_iter().flatten().map(|l| (l.glyph, format!("{}'s blow", atglyph::primitives::noun::with_the(&item.name)))));
            glyphs.extend(item.triggers.iter().filter_map(|t| t.look).map(|l| (l.glyph, format!("what the {} sets off", item.name))));
        }
        for (_, ability) in world.resource::<Abilities>().defs().iter() {
            glyphs.extend(ability.look.map(|l| (l.glyph, format!("the ability {}", ability.name))));
        }
        let ladder = crate::stairs::ladder();
        glyphs.extend([ladder.up, ladder.down].into_iter().map(|l| (l.glyph, l.name)));
        // The engine's own looks, as the game takes them: its own where it
        // sets one, the engine's otherwise.
        let fields = world.get_resource::<FieldAppearance>().cloned().unwrap_or_default();
        glyphs.extend([fields.flame.glyph, fields.ember].into_iter().chain(fields.haze.iter().copied()).map(|g| (g, "fire and gas".to_string())));
        let bursts = world.get_resource::<ParticleStyle>().cloned().unwrap_or_default();
        glyphs.extend(std::iter::once(bursts.plain.0).chain(bursts.burst_glyphs).chain(bursts.blast_glyphs).map(|g| (g, "a flight or a burst".to_string())));
        glyphs
    }

    /// Every glyph the content is drawn with is one the typeface has, read
    /// from the face's own character map. A glyph it has not is drawn from
    /// whatever face the system falls back on, or as an empty box, and in a
    /// browser always as the box.
    #[test]
    fn every_glyph_the_content_is_drawn_with_is_one_the_typeface_has() {
        let app = crate::testing::headless(RunSeed(7));
        let (face, content) = (super::face(), content(&app));
        assert!(content.iter().any(|(_, of)| of == "the tile lake") && content.iter().any(|(_, of)| of.starts_with("the monster")), "the content is read");
        let lacking: Vec<_> = content.iter().filter(|(glyph, _)| face.glyph_index(*glyph).is_none()).collect();
        assert!(lacking.is_empty(), "the typeface has no glyph for {lacking:?}");
    }
}
