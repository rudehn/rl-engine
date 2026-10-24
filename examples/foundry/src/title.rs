//! The title screen's picture: the foundry's floor, seen from the gantry
//! the commando drops in on.
//!
//! The menu over it is the engine's. `main.rs` adds `TitlePanel` with
//! [`menu`]'s rectangle beside the other panels, and [`paint_backdrop`] in
//! `TitleSet::Backdrop` with it. The panel holds the run back until the
//! player picks, and offers Continue, New Game, Settings and Exit in the
//! rows the picture leaves dark under the gantry; the picture is painted
//! under them every frame the title is up. What is Foundry's here is the
//! picture, and where it leaves room for the menu.
//!
//! **The art.** Hand-drawn, stamped in layers back to front: the far wall
//! and its machine towers, the furnace and its glow, the gantry the scene
//! is watched from, the conveyor and what is on it, then the title, its
//! tagline and the rule over the menu. `art` holds the stamps and `paint`
//! the order, so a change to the picture is a change to one const. Three
//! things move, and only three, because a title screen that never settles
//! is tiring to look at: the furnace breathes, its sparks rise, and the
//! line carries chassis to the right.

use atglyph::primitives::GridRect;
use atglyph::render::{Cell, Terminal};
use atglyph::ui::{Palette, Tones};
use bevy::prelude::*;

/// Where the engine's menu is drawn on a terminal `width` by `height`: the
/// rows under the gantry's rule, the whole width across, which the picture
/// leaves dark so the menu reads over nothing. The panel centres its rows
/// across it, where Foundry's own menu stood.
pub fn menu(width: i32, height: i32) -> GridRect {
    GridRect::new(0, rows::MENU, width, height - rows::MENU)
}

/// Paints the picture: the scene, the title, its tagline and the rule
/// over the menu.
///
/// In `TitleSet::Backdrop`, which runs only while the title is up, and
/// before the engine draws its menu over the picture. Paints nothing
/// without a terminal, which a headless test may leave out.
pub fn paint_backdrop(terminal: Option<ResMut<Terminal>>, palette: Res<Palette>, time: Res<Time>) {
    let Some(mut terminal) = terminal else { return };
    let t = time.elapsed_secs();
    paint(&mut terminal, t);
    paint_title(&mut terminal, t);
    paint_words(&mut terminal, &palette);
}

/// The colours the picture is drawn in.
///
/// Its own palette rather than the interned tones, because these are the
/// scene's own: ember, steel, the cyan of a sensor. The menu over it reads
/// the game's `Palette` like every other panel, so a game that repaints
/// its tones repaints the words and not the picture.
mod ink {
    use bevy::prelude::Color;

    /// The far wall, behind everything.
    pub const DARK: Color = Color::srgb(0.10, 0.10, 0.13);
    /// Machine towers and the gantry: cold steel in shadow.
    pub const STEEL: Color = Color::srgb(0.28, 0.30, 0.35);
    /// What the furnace lights: the near edges of the works.
    pub const LIT_STEEL: Color = Color::srgb(0.46, 0.42, 0.40);
    /// Pipes and rails, a shade warmer than the steel.
    pub const PIPE: Color = Color::srgb(0.36, 0.33, 0.30);
    /// The furnace's own mouth.
    pub const EMBER: Color = Color::srgb(1.00, 0.55, 0.15);
    /// Deeper in the furnace, and the coals under it.
    pub const EMBER_DEEP: Color = Color::srgb(0.85, 0.28, 0.08);
    /// A spark off the line, and the welder's flash.
    pub const SPARK: Color = Color::srgb(1.00, 0.85, 0.45);
    /// Ceiling lamps.
    pub const LAMP: Color = Color::srgb(0.95, 0.88, 0.60);
    /// A droid's sensor, and the light of a live panel.
    pub const SENSOR: Color = Color::srgb(0.45, 0.80, 0.95);
    /// The chassis on the line.
    pub const CHASSIS: Color = Color::srgb(0.62, 0.58, 0.48);
    /// The commando on the gantry.
    pub const COMMANDO: Color = Color::srgb(0.85, 0.92, 0.85);
}

/// The stamps the scene is drawn from, back to front.
mod art {
    /// A machine tower along the far wall: ten wide, nine tall.
    pub const TOWER: [&str; 9] = [" ┌──────┐ ", " │▄▄  ▄▄│ ", " │      │ ", "┌┴──────┴┐", "│ ░░  ░░ │", "│ ░░  ░░ │", "│ ░░  ░░ │", "└─┬────┬─┘", "  ┴    ┴  "];

    /// A taller, narrower tower, to break the skyline up.
    pub const STACK: [&str; 11] = [
        "  ╔════╗  ",
        "  ║ ▄▄ ║  ",
        "  ║    ║  ",
        "┌─╫────╫─┐",
        "│ ║ ░░ ║ │",
        "│ ║ ░░ ║ │",
        "│ ╚════╝ │",
        "│  ░░░░  │",
        "│  ░░░░  │",
        "└──┬──┬──┘",
        "   ┴  ┴   ",
    ];

    /// The blast furnace: twenty-one wide, nine tall, its mouth left blank
    /// for the glow to fill.
    pub const FURNACE: [&str; 9] = [
        "   ╔═════════════╗   ",
        "   ║ ▀▀▀▀▀▀▀▀▀▀▀ ║   ",
        "  ╔╝             ╚╗  ",
        "  ║  ▄▄▄▄▄▄▄▄▄▄▄  ║  ",
        "  ║ │           │ ║  ",
        "  ║ │           │ ║  ",
        "  ║ │           │ ║  ",
        "  ╚═╧═══════════╧═╝  ",
        "                     ",
    ];

    /// The furnace's chimney, which breaks the crane's rail and vents its
    /// sparks past the gantry.
    pub const CHIMNEY: [&str; 3] = ["╔════╗", "║    ║", "║    ║"];

    /// The crane's trolley, hung off the rail above the works.
    pub const TROLLEY: [&str; 5] = ["┌───┐", "│ ▓ │", "└─┬─┘", "  ╎  ", "  ∪  "];

    /// A welding arm over the line.
    pub const ARM: [&str; 4] = ["┌──┐", "│╱ │", "╰╮ │", " ╰╯ "];

    /// A chassis riding the line: half a droid, and not yet awake.
    pub const CHASSIS: [&str; 2] = ["┌──┐", "╘══╛"];
}

/// Stamps `lines` at `(x, y)`, leaving spaces transparent so a later stamp
/// shows what is behind it.
fn stamp(terminal: &mut Terminal, x: i32, y: i32, lines: &[&str], fg: Color) {
    for (row, line) in lines.iter().enumerate() {
        for (col, glyph) in line.chars().enumerate() {
            if glyph == ' ' {
                continue;
            }
            terminal.set(x + col as i32, y + row as i32, Cell::new(glyph, fg));
        }
    }
}

/// The same, but opaque: a space paints the dark rather than letting what
/// is behind show through.
///
/// What every solid thing in the scene wants. A furnace drawn transparently
/// has a machine tower standing inside it, which is how the first draft of
/// this screen looked.
fn stamp_over(terminal: &mut Terminal, x: i32, y: i32, lines: &[&str], fg: Color) {
    for (row, line) in lines.iter().enumerate() {
        for (col, glyph) in line.chars().enumerate() {
            let cell = if glyph == ' ' { Cell::new(' ', ink::DARK) } else { Cell::new(glyph, fg) };
            terminal.set(x + col as i32, y + row as i32, cell);
        }
    }
}

/// Fills a band of rows with one cell, which is how the far wall and the
/// dark under the floor are laid in.
fn wash(terminal: &mut Terminal, from: i32, to: i32, glyph: char, fg: Color) {
    for y in from..=to {
        for x in 0..terminal.width() {
            terminal.set(x, y, Cell::new(glyph, fg));
        }
    }
}

/// A number from 0 to 1 that depends on nothing but its inputs.
///
/// The scene's dice: which cell has a spark, how hot this corner of the
/// mouth is. A hash rather than a stream, because the title screen is
/// outside a run and has no seed to draw from, and a picture that flickers
/// the same way every time the game is opened is the point.
fn hashed(a: i32, b: i32) -> f32 {
    let mut h = (a as u32).wrapping_mul(0x9E37_79B9) ^ (b as u32).wrapping_mul(0x85EB_CA6B);
    h ^= h >> 13;
    h = h.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 16;
    (h % 1000) as f32 / 1000.0
}

/// Every row the scene is laid out from, top to bottom, so nothing is
/// placed by counting on the screen.
mod rows {
    /// The ceiling run of pipe.
    pub const PIPE: i32 = 0;
    /// Where the lamps hang off it.
    pub const LAMP: i32 = 1;
    /// The top row of the title, which is six tall.
    pub const TITLE: i32 = 4;
    /// The line under the title.
    pub const TAGLINE: i32 = 11;
    /// The furnace's chimney, which the crane's rail passes behind.
    pub const CHIMNEY: i32 = 12;
    /// The crane's rail, over the works.
    pub const CRANE: i32 = 13;
    /// The top of the machine towers and of the furnace.
    pub const TOWER: i32 = 15;
    /// The top of a welding arm.
    pub const ARM: i32 = 20;
    /// The chassis riding the line.
    pub const CHASSIS: i32 = 24;
    /// The belt's rollers, with its frame a row under.
    pub const BELT: i32 = 26;
    /// The gantry's rail, its posts, and its grating, a clear row below the
    /// belt so the two do not read as one machine.
    pub const RAIL: i32 = 29;
    /// The top of the engine's menu, three below the grating so the rule
    /// over it has a clear row of its own. Its four rows, one to a line,
    /// leave two clear under them.
    pub const MENU: i32 = 34;
}

/// Where the furnace stands, and how wide it is.
const FURNACE_X: i32 = 4;
/// Where the towers stand, and which of the two they are: clear of the
/// furnace, and of the arms between them. `true` is the tall stack, which
/// starts two rows higher.
const TOWERS: [(i32, bool); 3] = [(30, false), (56, true), (82, false)];
/// Where the welding arms hang: in the spans the towers leave, and clear of
/// the crane's trolley.
const ARMS: [i32; 2] = [46, 76];
/// Where the crane's trolley sits on its rail.
const TROLLEY_X: i32 = 68;

/// Paints the scene: the wall, the towers, the furnace, the crane, the
/// line, the gantry, the lamps.
fn paint(terminal: &mut Terminal, t: f32) {
    let w = terminal.width();
    wash(terminal, 0, terminal.height() - 1, ' ', ink::DARK);

    // The crane's rail first, edge to edge, so the towers and the furnace
    // are drawn over it and it reads as passing behind them.
    for x in 0..w {
        terminal.set(x, rows::CRANE, Cell::new('═', ink::PIPE));
    }

    // The far wall's towers, opaque, with a few lit windows each.
    for (x, tall) in TOWERS {
        match tall {
            true => stamp_over(terminal, x, rows::TOWER - 2, &art::STACK, ink::STEEL),
            false => stamp_over(terminal, x, rows::TOWER, &art::TOWER, ink::STEEL),
        }
    }
    for (x, tall) in TOWERS {
        let top = if tall { rows::TOWER - 2 } else { rows::TOWER };
        for dy in 4..9 {
            for dx in [2, 3, 6, 7] {
                if terminal.get(x + dx, top + dy).is_some_and(|c| c.glyph == '░') && hashed(x + dx, dy) > 0.5 {
                    terminal.set(x + dx, top + dy, Cell::new('░', ink::SENSOR));
                }
            }
        }
    }
    terminal.set(TROLLEY_X + 2, rows::CRANE, Cell::new('╤', ink::PIPE));
    stamp_over(terminal, TROLLEY_X, rows::CRANE + 1, &art::TROLLEY, ink::PIPE);

    // The furnace, and the mouth breathing inside it.
    stamp_over(terminal, FURNACE_X, rows::TOWER, &art::FURNACE, ink::LIT_STEEL);
    let breath = (t * 0.7).sin() * 0.5 + 0.5;
    for row in 0..3 {
        for col in 0..11 {
            let (x, y) = (FURNACE_X + 5 + col, rows::TOWER + 4 + row);
            let heat = hashed(x, y) * 0.55 + breath * 0.45;
            let glyph = match heat {
                h if h > 0.74 => '▓',
                h if h > 0.42 => '▒',
                _ => '░',
            };
            terminal.set(x, y, Cell::new(glyph, if heat > 0.62 { ink::EMBER } else { ink::EMBER_DEEP }));
        }
    }
    // Coals spilling from under the mouth, steady, and the light they throw
    // on the floor of the works.
    for col in 0..15 {
        let x = FURNACE_X + 3 + col;
        if hashed(x, 91) > 0.35 {
            terminal.set(x, rows::TOWER + 8, Cell::new('░', ink::EMBER_DEEP));
        }
    }
    // The chimney, over the crown and through the crane's rail, and the
    // sparks going up its bore: they start where the brick ends and go out
    // above the gantry, so none of them is ever drawn inside the furnace.
    let chimney = FURNACE_X + 8;
    stamp_over(terminal, chimney, rows::CHIMNEY, &art::CHIMNEY, ink::LIT_STEEL);
    for i in 0..8 {
        let drift = ((t * 4.0 + i as f32 * 1.9) % 7.0).floor() as i32;
        let x = chimney + 1 + (hashed(i, 7) * 4.0) as i32;
        let y = rows::TOWER - 1 - drift;
        // Inside the bore only: a spark drawn on the stack's own rim reads
        // as a chip out of the brick.
        if y > rows::CHIMNEY {
            let glyph = match drift {
                0..=1 => '*',
                2..=4 => '.',
                _ => '\u{00b7}',
            };
            terminal.set(x, y, Cell::new(glyph, ink::SPARK));
        }
    }

    // The line: the arms over it, the chassis riding right, the belt under.
    for x in ARMS {
        // A short hanger, so the arm is held by something without a wire
        // drawn through every tower between it and the ceiling.
        for y in rows::ARM - 2..rows::ARM {
            terminal.set(x + 1, y, Cell::new('╎', ink::PIPE));
        }
        stamp_over(terminal, x, rows::ARM, &art::ARM, ink::LIT_STEEL);
    }
    let riding: Vec<i32> = (0..4).map(|i| ((t * 5.0) as i32 + i * 26) % (w + 8) - 4).collect();
    for (i, x) in riding.iter().copied().enumerate() {
        stamp_over(terminal, x, rows::CHASSIS, &art::CHASSIS, ink::CHASSIS);
        if i % 2 == 0 {
            terminal.set(x + 1, rows::CHASSIS, Cell::new('o', ink::SENSOR));
        }
    }
    for x in 0..w {
        let roller = if (x + (t * 4.0) as i32) % 4 == 0 { '·' } else { '─' };
        terminal.set(x, rows::BELT, Cell::new(roller, ink::PIPE));
        terminal.set(x, rows::BELT + 1, Cell::new('═', ink::STEEL));
    }
    // The weld, which fires on what is under it rather than on air: an arm
    // strikes while a chassis is passing beneath it, and the spark lands on
    // the chassis.
    for x in ARMS {
        let under = riding.iter().any(|c| (*c..*c + 4).contains(&(x + 1)));
        if under && hashed(x, (t * 4.0) as i32) > 0.35 {
            terminal.set(x + 1, rows::ARM + 3, Cell::new('▼', ink::SPARK));
            terminal.set(x + 1, rows::CHASSIS, Cell::new('*', ink::SPARK));
            terminal.set(x + 2, rows::CHASSIS + 1, Cell::new('·', ink::SPARK));
        }
    }

    // The gantry the commando stands on, and the dark under it.
    for x in 0..w {
        terminal.set(x, rows::RAIL, Cell::new('─', ink::LIT_STEEL));
        // Grating, not a wall: a bar every fourth cell over a lighter weave,
        // so the works show through the walkway the way they would.
        let grate = if x % 4 == 0 { '╫' } else { '▒' };
        terminal.set(x, rows::RAIL + 2, Cell::new(grate, ink::STEEL));
    }
    let me = w / 2 - 17;
    for x in (0..w).step_by(6) {
        if x != me {
            terminal.set(x, rows::RAIL + 1, Cell::new('│', ink::STEEL));
        }
    }
    wash(terminal, rows::RAIL + 3, terminal.height() - 1, ' ', ink::DARK);
    // The commando, leaning on the rail, looking down into the works.
    terminal.set(me, rows::RAIL + 1, Cell::new('@', ink::COMMANDO));

    // The ceiling: one run of pipe, lamps off it, and what they throw.
    for x in 0..w {
        terminal.set(x, rows::PIPE, Cell::new('═', ink::PIPE));
    }
    for x in (8..w - 8).step_by(17) {
        terminal.set(x, rows::LAMP, Cell::new('╤', ink::PIPE));
        terminal.set(x, rows::LAMP + 1, Cell::new('▄', ink::LAMP));
        // What the lamp throws: a short cone, brightest under the housing.
        for dx in -2..=2 {
            let glyph = if dx == 0 { '░' } else { '·' };
            terminal.set(x + dx, rows::LAMP + 2, Cell::new(glyph, ink::LAMP.with_alpha(0.3)));
        }
    }
}

/// Stamps FOUNDRY across the top, over the wall and under the pipes.
fn paint_title(terminal: &mut Terminal, t: f32) {
    const WORD: [&str; 7] = ["F", "O", "U", "N", "D", "R", "Y"];
    let letters: Vec<[&str; 6]> = WORD.iter().map(|l| letter(l)).collect();
    let width: i32 = letters.iter().map(|l| l[0].chars().count() as i32 + 1).sum::<i32>() - 1;
    let mut x = (terminal.width() - width) / 2;
    let y = rows::TITLE;
    for letter in &letters {
        // The word is lit from the furnace below it: the lower rows warmer
        // than the upper, and the whole of it breathing with the mouth.
        let breath = (t * 0.7).sin() * 0.06;
        for (row, line) in letter.iter().enumerate() {
            let warmth = row as f32 / 5.0;
            let fg = Color::srgb(0.72 + warmth * 0.28 + breath, 0.70 + warmth * 0.12 + breath, 0.68 - warmth * 0.42);
            stamp(terminal, x, y + row as i32, &[line], fg);
        }
        x += letter[0].chars().count() as i32 + 1;
    }
}

/// One letter of the title, six rows tall.
fn letter(which: &str) -> [&'static str; 6] {
    match which {
        "F" => ["██████", "██    ", "█████ ", "██    ", "██    ", "██    "],
        "O" => [" █████ ", "██   ██", "██   ██", "██   ██", "██   ██", " █████ "],
        "U" => ["██   ██", "██   ██", "██   ██", "██   ██", "██   ██", " █████ "],
        "N" => ["██   ██", "███  ██", "██ █ ██", "██  ███", "██   ██", "██   ██"],
        "D" => ["██████ ", "██   ██", "██   ██", "██   ██", "██   ██", "██████ "],
        "R" => ["██████ ", "██   ██", "██████ ", "██  ██ ", "██   ██", "██   ██"],
        "Y" => ["██   ██", " ██ ██ ", "  ███  ", "   ██  ", "   ██  ", "   ██  "],
        _ => ["", "", "", "", "", ""],
    }
}

/// The words over the picture: the tagline under the title, and the rule
/// over the menu. In the game's muted tone rather than the picture's ink,
/// so a game that repaints its tones repaints them with the menu's rows.
fn paint_words(terminal: &mut Terminal, palette: &Palette) {
    let w = terminal.width();
    let centre = |text: &str| (w - text.chars().count() as i32) / 2;

    let tagline = "Ten decks of it, and four charges to set on the way down.";
    terminal.print(centre(tagline), rows::TAGLINE, tagline, palette.get(Tones::MUTED));

    let rule: String = "─".repeat(34);
    terminal.print(centre(&rule), rows::MENU - 2, &rule, palette.get(Tones::MUTED));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The picture `t` seconds in, on Foundry's screen.
    fn painted(t: f32) -> Terminal {
        let mut terminal = Terminal::new(100, 40, Vec2::new(10.0, 20.0));
        paint(&mut terminal, t);
        paint_title(&mut terminal, t);
        paint_words(&mut terminal, &Palette::default());
        terminal
    }

    /// The engine draws its menu over the picture, and the rows read
    /// because what is under them is the dark: over a minute of the
    /// picture's motion, nothing it draws lands in the menu's rectangle.
    #[test]
    fn the_picture_never_draws_in_the_menus_rectangle_at_any_moment_of_its_motion() {
        let menu = menu(100, 40);
        for tenth in 0..600 {
            let terminal = painted(tenth as f32 / 10.0);
            let drawn: Vec<_> = menu.cells().filter(|at| terminal.get(at.x, at.y).is_some_and(|c| c.glyph != ' ' || c.bg != Color::BLACK)).collect();
            assert!(drawn.is_empty(), "{} seconds in, the picture draws in the menu at {drawn:?}", tenth as f32 / 10.0);
        }
    }

    /// Prints the picture as the player sees it under the menu, for
    /// eyeballing the art: `cargo test -p foundry title_screen -- --ignored
    /// --nocapture`. The menu is the engine's and is drawn over the dark
    /// rows at the foot; running the game shows the two together.
    #[test]
    #[ignore]
    fn title_screen() {
        let terminal = painted(0.4);
        for y in 0..terminal.height() {
            let row: String = (0..terminal.width()).map(|x| terminal.get(x, y).map(|c| c.glyph).unwrap_or(' ')).collect();
            println!("{row}");
        }
    }
}
