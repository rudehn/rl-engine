//! The terminal loop: raw mode, keys in, [`terminal::render`]'s rows out in colour. It decides
//! nothing but which key means which turn, and which colour a cell is drawn in.

use std::io::{Stdout, Write, stdout};

use atglyph_core::{Direction, Point};
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, read};
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode};
use crossterm::{execute, queue};
use terminal::{HEIGHT, Walk, direction_of, generate, render, status, step, wait};

/// The keys, shown after the status line, which is padded so they never shift as health falls.
const KEYS: &str = "hjklyubn or arrows move, . waits, q quits";

/// Leaves raw mode and the alternate screen when dropped, so a panic or an early return
/// still hands back a usable shell.
struct Restore;

impl Drop for Restore {
    fn drop(&mut self) {
        let _ = execute!(stdout(), ResetColor, Show, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

/// The colour a glyph is drawn in: what is in sight bright, what is only remembered dim.
fn colour(glyph: char, in_sight: bool) -> Color {
    match glyph {
        '@' => Color::White,
        'r' => Color::Red,
        _ if in_sight => Color::Grey,
        _ => Color::DarkGrey,
    }
}

/// Draws the map, the status line and the last two messages, those of the turn just played
/// bright and older ones dim. Each line is overwritten in place rather than the screen cleared,
/// so nothing flickers between turns.
fn draw(out: &mut Stdout, walk: &Walk, fresh: usize) -> std::io::Result<()> {
    for (y, row) in render(walk).iter().enumerate() {
        queue!(out, MoveTo(0, y as u16))?;
        let mut current = None;
        for (x, glyph) in row.chars().enumerate() {
            let c = colour(glyph, walk.seen.contains(Point::new(x as i32, y as i32)));
            if current != Some(c) {
                queue!(out, SetForegroundColor(c))?;
                current = Some(c);
            }
            queue!(out, Print(glyph))?;
        }
    }
    let line = HEIGHT as u16;
    let (health, keys) = if walk.is_alive() { (Color::White, KEYS) } else { (Color::Red, "q quits") };
    queue!(out, MoveTo(0, line), Clear(ClearType::CurrentLine), SetForegroundColor(health), SetAttribute(Attribute::Bold))?;
    queue!(out, Print(format!("{:<24}", status(walk))), SetAttribute(Attribute::Reset), SetForegroundColor(Color::DarkGrey), Print(keys))?;
    let shown = walk.log.len().saturating_sub(2);
    for (n, row) in (line + 1..line + 3).enumerate() {
        queue!(out, MoveTo(0, row), Clear(ClearType::CurrentLine))?;
        if let Some(message) = walk.log.get(shown + n) {
            let c = if shown + n >= fresh { Color::White } else { Color::DarkGrey };
            queue!(out, SetForegroundColor(c), Print(message))?;
        }
    }
    queue!(out, ResetColor)?;
    out.flush()
}

fn main() -> std::io::Result<()> {
    let seed = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let mut walk = generate(seed);
    walk.log.push(format!("Seed {seed}. Rats are about."));
    enable_raw_mode()?;
    let _restore = Restore;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, Hide, Clear(ClearType::All))?;
    let mut fresh = 0;
    loop {
        draw(&mut out, &walk, fresh)?;
        let Event::Key(key) = read()? else { continue };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        let dir = match key.code {
            KeyCode::Char('q') | KeyCode::Esc => break,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
            KeyCode::Up => Some(Direction::North),
            KeyCode::Down => Some(Direction::South),
            KeyCode::Left => Some(Direction::West),
            KeyCode::Right => Some(Direction::East),
            KeyCode::Char(c) => direction_of(c),
            _ => None,
        };
        let before = walk.log.len();
        let spent = match (key.code, dir) {
            (_, Some(dir)) => step(&mut walk, dir),
            (KeyCode::Char('.'), None) => wait(&mut walk),
            _ => false,
        };
        if spent {
            fresh = before;
        }
    }
    Ok(())
}
