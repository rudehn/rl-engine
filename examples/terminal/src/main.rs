//! The terminal loop: raw mode, keys in, [`terminal::render`]'s rows out. It decides nothing
//! but which key means which step.

use std::io::{Write, stdout};

use atglyph_core::Direction;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, read};
use crossterm::terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode};
use crossterm::{execute, queue};
use terminal::{direction_of, generate, render, step};

/// Leaves raw mode and the alternate screen when dropped, so a panic or an early return
/// still hands back a usable shell.
struct Restore;

impl Drop for Restore {
    fn drop(&mut self) {
        let _ = execute!(stdout(), Show, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

fn main() -> std::io::Result<()> {
    let seed = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let mut walk = generate(seed);
    enable_raw_mode()?;
    let _restore = Restore;
    execute!(stdout(), EnterAlternateScreen, Hide)?;
    loop {
        let mut out = stdout();
        queue!(out, Clear(ClearType::All))?;
        for (y, row) in render(&walk).iter().enumerate() {
            queue!(out, MoveTo(0, y as u16))?;
            write!(out, "{row}")?;
        }
        out.flush()?;
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
        if let Some(dir) = dir {
            step(&mut walk, dir);
        }
    }
    Ok(())
}
