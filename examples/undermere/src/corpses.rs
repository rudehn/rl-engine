//! What a body looks like.
//!
//! The engine gives remains no look of their own, on purpose: it does not
//! know what a game thinks lies there. `lay_down` keeps the glyph the
//! actor had in life, so left alone a goblin's corpse would be drawn as a
//! goblin. Undermere draws every body as `%` in a dimmed shade of its
//! kind's own colour, so that it reads as dead on sight and what died is
//! still read off the colour.
//!
//! [`dress_the_dead`] runs in `TurnSet::React`, the turn a body is left,
//! and once more when a continued run has laid its bodies back down, since
//! a save keeps what a body was and never how it was drawn. It looks at
//! every body rather than at the new ones, and a body already `%` is left
//! alone, so the dimming is never applied twice: no monster is drawn `%`
//! in life, which a test checks. The engine's twin of the living actor is
//! taken before this runs, so a revived body is drawn as it was.

use bevy::prelude::*;
use rl_engine::prelude::*;

/// The glyph of every body, the feeding ground's prey with them.
pub const REMAINS_GLYPH: char = '%';

/// How much of its colour a body keeps: dimmed enough to be a shade of
/// the kind and not the kind, bright enough to find on a dark floor.
const DIM: f32 = 0.7;

/// Below items (layer 2), so what a body dropped is not hidden under it,
/// and below the living (5), so a body is never drawn over a fight.
const LAYER: i32 = 1;

/// A glyph as a body of whatever it was drawn as in life.
pub fn as_corpse(life: Glyph) -> Glyph {
    let c = life.fg.to_srgba();
    Glyph::new(REMAINS_GLYPH, Color::srgb(c.red * DIM, c.green * DIM, c.blue * DIM)).on_layer(LAYER)
}

/// Draws every body as a corpse. In `TurnSet::React`, and again after a
/// continued run is restored.
pub fn dress_the_dead(mut bodies: Query<&mut Glyph, With<Remains>>) {
    for mut glyph in &mut bodies {
        if glyph.ch != REMAINS_GLYPH {
            *glyph = as_corpse(*glyph);
        }
    }
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_core::RunSeed;

    use super::*;
    use crate::testing::{arena_on, continued, kill, monster, settle};

    fn glyph_of(app: &App, e: Entity) -> Glyph {
        *app.world().get::<Glyph>(e).expect("a body keeps a glyph")
    }

    #[test]
    fn no_monster_is_drawn_as_a_corpse_in_life() {
        let app = crate::testing::headless(RunSeed(1));
        let bestiary = app.world().resource::<crate::bestiary::Bestiary>();
        for (_, def) in bestiary.defs.iter() {
            assert_ne!(def.glyph, REMAINS_GLYPH, "{} would be dressed twice", def.name);
        }
    }

    #[test]
    fn a_body_is_a_dimmed_percent_of_its_kinds_colour_the_turn_it_falls() {
        let (mut app, _me, here) = arena_on(5, 1);
        let goblin = monster(&mut app, "goblin", here.offset(2, 0));
        let life = glyph_of(&app, goblin);
        kill(&mut app, goblin, None);
        settle(&mut app);
        let body = glyph_of(&app, goblin);
        assert_eq!(body.ch, '%');
        assert_eq!(body.layer, LAYER);
        let (a, b) = (life.fg.to_srgba(), body.fg.to_srgba());
        assert!((b.red - a.red * DIM).abs() < 1e-4 && (b.green - a.green * DIM).abs() < 1e-4 && (b.blue - a.blue * DIM).abs() < 1e-4);
        settle(&mut app);
        assert_eq!(glyph_of(&app, goblin).fg, body.fg, "dimmed once and no more");
    }

    #[test]
    fn a_body_is_still_a_corpse_after_a_continued_run() {
        let (mut app, _me, here) = arena_on(5, 1);
        let goblin = monster(&mut app, "goblin", here.offset(2, 0));
        kill(&mut app, goblin, None);
        settle(&mut app);
        rl_engine::rl_save::save_run(app.world_mut()).expect("the run is written down");
        let text = app.world().resource::<Saves>().load(crate::save::SLOT).expect("the slot reads").expect("a save");
        let mut again = continued(&text);
        let world = again.world_mut();
        let glyphs: Vec<Glyph> = world.query_filtered::<&Glyph, With<Remains>>().iter(world).copied().collect();
        assert!(!glyphs.is_empty(), "the body came back");
        assert!(glyphs.iter().all(|g| g.ch == '%'), "and as a corpse: {glyphs:?}");
    }
}
