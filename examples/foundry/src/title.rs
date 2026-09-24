//! The title screen: the foundry's floor, seen from the gantry the
//! commando drops in on, and what the player may do about it.
//!
//! Foundry's own screen, not the engine's. The engine owns the menu a run
//! is paused and ended on (`GameMenuPanel`), which is a list over a world
//! that already exists; this is the one before there is a world at all, so
//! it draws the whole terminal itself and holds the run back until the
//! player asks for it.
//!
//! **How it holds the run back.** The engine runs `NewRun` at startup, and
//! Foundry's `run::start` is in it. While [`Title::up`] is true that system
//! returns at once, so no `WorldMap` is inserted, the engine's state stays
//! `Idle` and nothing else runs either. Picking a run lowers the flag and
//! runs `NewRun` again, which is the same door the engine's own restart
//! uses. Nothing in the engine had to learn what a title screen is.
//!
//! **The art.** Hand-drawn, stamped in layers back to front: the far wall
//! and its machine towers, the furnace and its glow, the gantry the scene
//! is watched from, the conveyor and what is on it, then the title and the
//! menu over all of it. `art` holds the stamps and `paint` the order, so a
//! change to the picture is a change to one const. Three things move, and
//! only three, because a title screen that never settles is tiring to look
//! at: the furnace breathes, its sparks rise, and the line carries chassis
//! to the right.

use bevy::prelude::*;
use rl_engine::rl_bevy::EngineState;
use rl_engine::rl_bevy::plugin::{EngineSet, NewRun};
use rl_engine::rl_render::{Cell, Terminal};
use rl_engine::rl_ui::{Palette, Tones};

/// The title screen's state: whether it is up, and the row picked out.
///
/// Up at startup and down for the rest of the process: a run that ends
/// goes to the engine's own end-of-run menu, which offers another run
/// directly, so coming back here would be a second way to say the same
/// thing.
#[derive(Resource, Debug, Clone)]
pub struct Title {
    /// Whether the screen is up and holding the run back.
    pub up: bool,
    /// Which row is picked out.
    pub picked: usize,
    /// What the save slot held when the screen came up.
    pub save: SaveOnDisk,
    /// The question New Game asks over a save, while it is asked: `Some`
    /// with whether Yes is picked out.
    pub confirm: Option<bool>,
}

impl Default for Title {
    /// Up, with no save looked for yet and the first row that can be taken
    /// picked out.
    fn default() -> Self {
        Self { up: true, picked: first_available(SaveOnDisk::None), save: SaveOnDisk::None, confirm: None }
    }
}

/// What the save slot holds, for the title screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveOnDisk {
    /// Nothing: there is no run to continue.
    None,
    /// A run this build can read and continue.
    Readable,
    /// Something this build cannot read, of another version or damaged: it
    /// cannot be continued, and New Game offers to clear it.
    Unreadable,
}

/// The first row that can be taken with `save` in the slot.
fn first_available(save: SaveOnDisk) -> usize {
    Choice::all().iter().position(|c| c.available(save)).unwrap_or(0)
}

/// Looks in the save slot as the screen comes up, and picks Continue out
/// when there is a run to continue.
pub fn look_for_save(world: &mut World) {
    let save = match rl_engine::rl_save::load_run(world) {
        Ok(Some(_)) => SaveOnDisk::Readable,
        Ok(None) => SaveOnDisk::None,
        Err(e) => {
            warn!("the saved run cannot be read: {e}");
            SaveOnDisk::Unreadable
        }
    };
    let mut title = world.resource_mut::<Title>();
    title.save = save;
    title.picked = first_available(save);
}

/// What the title screen offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// Carry on the saved run, when there is one this build can read.
    Continue,
    /// A fresh seed, deck one.
    NewGame,
    /// Leave.
    Exit,
}

impl Choice {
    /// Every row, in the order they are drawn.
    pub fn all() -> [Choice; 3] {
        [Choice::Continue, Choice::NewGame, Choice::Exit]
    }

    /// What the row says.
    fn label(self) -> &'static str {
        match self {
            Choice::Continue => "Continue",
            Choice::NewGame => "New Game",
            Choice::Exit => "Exit",
        }
    }

    /// Whether the row can be picked with `save` in the slot: every row
    /// but `Continue`, which needs a run this build can read.
    fn available(self, save: SaveOnDisk) -> bool {
        self != Choice::Continue || save == SaveOnDisk::Readable
    }
}

/// The next row from `from` toward `dir`, one way or the other, wrapping
/// round and passing over any row that cannot be picked.
fn step(from: usize, dir: isize, save: SaveOnDisk) -> usize {
    let rows = Choice::all();
    let n = rows.len() as isize;
    let mut at = from as isize;
    loop {
        at = (at + dir).rem_euclid(n);
        if rows[at as usize].available(save) {
            return at as usize;
        }
    }
}

/// Foundry's title screen: the picture, the keys and the hold on the run.
pub struct TitlePlugin;

impl Plugin for TitlePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Title>()
            // What the slot holds, looked at once as the screen comes up,
            // before the first key is read: in `PreStartup`, so it is done
            // before the engine begins its first run in `Startup`, and after
            // the save's armory is loaded beside it.
            .add_systems(PreStartup, look_for_save.after(crate::save::load_armory))
            // Before the engine's input phase, and both of them before it:
            // that phase holds the exclusive key handlers, which conflict
            // with everything in the schedule they are not ordered against.
            // Outside every play-gated set, because the whole point of this
            // screen is the time before a run exists.
            //
            // The drawing wants a terminal, which a headless run has not
            // got: the keys and the hold on the run are what a test drives,
            // and the picture is what a window shows.
            .add_systems(Update, (read_title_keys, draw_title.run_if(resource_exists::<Terminal>)).chain().before(EngineSet::Input).run_if(title_is_up));
    }
}

/// Whether the title screen is up, for the systems that only run while it
/// is.
pub fn title_is_up(title: Option<Res<Title>>, state: Res<State<EngineState>>) -> bool {
    title.is_some_and(|t| t.up) && *state.get() == EngineState::Idle
}

/// Moves the picked row, and answers Enter.
///
/// An ordinary system, not an exclusive one: starting the run means running
/// the `NewRun` schedule, which wants the whole world, and a system that
/// takes the whole world conflicts with every other system in its schedule.
/// The world work is queued as a command instead, which runs with exclusive
/// access at the next sync point in the same frame.
pub fn read_title_keys(keys: Res<ButtonInput<KeyCode>>, mut title: ResMut<Title>, mut commands: Commands, mut exit: MessageWriter<AppExit>) {
    let up = keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyK);
    let down = keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyJ);
    let across = keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::ArrowRight);
    let take = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) || keys.just_pressed(KeyCode::Space);
    let leave = keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::KeyQ);
    // The question over New Game has the keys while it is asked: any
    // direction moves between its two answers, Escape is No, and No, taken,
    // puts the question away with New Game still picked out.
    if let Some(yes) = title.confirm {
        if up || down || across {
            title.confirm = Some(!yes);
        } else if leave || (take && !yes) {
            title.confirm = None;
        } else if take {
            title.confirm = None;
            title.up = false;
            commands.queue(abandon_and_begin);
        }
        return;
    }
    let rows = Choice::all().len();
    if up {
        title.picked = step(title.picked, -1, title.save);
    }
    if down {
        title.picked = step(title.picked, 1, title.save);
    }
    if leave {
        exit.write(AppExit::Success);
        return;
    }
    if !take {
        return;
    }
    match Choice::all()[title.picked.min(rows - 1)] {
        Choice::Continue => {
            title.up = false;
            commands.insert_resource(crate::run::Resume);
            commands.queue(begin);
        }
        // Over a save, even one this build cannot read, New Game asks
        // first: a run abandoned by a slip of the key is gone for good.
        Choice::NewGame if title.save != SaveOnDisk::None => {
            title.confirm = Some(false);
        }
        Choice::NewGame => {
            title.up = false;
            commands.queue(begin);
        }
        Choice::Exit => {
            exit.write(AppExit::Success);
        }
    }
}

/// Deletes the saved run the player chose to abandon, then starts a new one.
fn abandon_and_begin(world: &mut World) {
    crate::save::abandon(world);
    begin(world);
}

/// Starts the run the player asked for.
///
/// `NewRun` rather than a `Restart`: a restart tears a run down and waits
/// for the state to come round to idle, and there is no run to tear down
/// yet. This is the same schedule the engine runs at startup and for every
/// restart after, so Foundry's own start is written once and runs the same
/// way whoever asked for it.
fn begin(world: &mut World) {
    world.run_schedule(NewRun);
}

/// Paints the whole terminal: the scene, the title, the menu.
pub fn draw_title(mut terminal: ResMut<Terminal>, title: Res<Title>, palette: Res<Palette>, time: Res<Time>) {
    let t = time.elapsed_secs();
    paint(&mut terminal, t);
    paint_title(&mut terminal, t);
    paint_menu(&mut terminal, &title, &palette);
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
    /// The first row of the menu, three below the grating so its rule has a
    /// clear row of its own.
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

/// The tagline and the rows: a word each, and nothing under them, since
/// three words need no gloss and the keys are the ones every menu has.
fn paint_menu(terminal: &mut Terminal, title: &Title, palette: &Palette) {
    let w = terminal.width();
    let centre = |text: &str| (w - text.chars().count() as i32) / 2;

    let tagline = "Ten decks of it, and four charges to set on the way down.";
    terminal.print(centre(tagline), rows::TAGLINE, tagline, palette.get(Tones::MUTED));

    let rule: String = "─".repeat(34);
    terminal.print(centre(&rule), rows::MENU - 2, &rule, palette.get(Tones::MUTED));

    // The question over New Game stands where the menu stood while it is
    // asked: one line, and its two answers under it, the picked one marked
    // and in the title's warm tone as a picked row is.
    if let Some(yes) = title.confirm {
        let question = "Abandon the run in progress?";
        terminal.print(centre(question), rows::MENU, question, palette.get(Tones::TEXT));
        let answers = [("No", !yes), ("Yes", yes)];
        let width = "> No    > Yes".chars().count() as i32;
        let mut x = (w - width) / 2;
        for (word, picked) in answers {
            let mark = if picked { '>' } else { ' ' };
            let tone = if picked { Tones::TITLE } else { Tones::TEXT };
            let text = format!("{mark} {word}");
            terminal.print(x, rows::MENU + 2, &text, palette.get(tone));
            x += text.chars().count() as i32 + 4;
        }
        return;
    }

    // Every row is centred on the same column, the mark's two cells
    // included, so the words line up and do not jump as the cursor moves.
    let x = centre(&format!("> {}", Choice::all().map(Choice::label).iter().max_by_key(|l| l.len()).unwrap()));
    for (i, choice) in Choice::all().into_iter().enumerate() {
        let picked = i == title.picked;
        // The picked row in the title's own warm tone, so it stands out from
        // the plain row and the dim one alike; `SELECT` is a background,
        // and as a foreground it read darker than the row that cannot be
        // picked.
        let tone = match (picked, choice.available(title.save)) {
            (true, _) => Tones::TITLE,
            (false, true) => Tones::TEXT,
            (false, false) => Tones::MUTED,
        };
        let mark = if picked { '>' } else { ' ' };
        terminal.print(x, rows::MENU + 2 * i as i32, &format!("{mark} {}", choice.label()), palette.get(tone));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rl_engine::rl_bevy::{Player, WorldMap};

    /// The title screen holds the run back: the engine ran `NewRun` at
    /// startup and Foundry's start did nothing, so there is no world and
    /// nothing is being dealt turns.
    #[test]
    fn the_title_screen_holds_the_run_back_until_it_is_asked() {
        let mut app = crate::testing::headless(rl_engine::rl_core::RunSeed(3));
        // The harness puts the screen down for every other test; this one is
        // about the screen, so it goes back up before anything runs.
        app.insert_resource(Title::default());
        app.update();
        app.update();
        assert!(app.world().resource::<Title>().up, "the screen is up to begin with");
        assert!(!app.world().contains_resource::<WorldMap>(), "and no deck was built behind it");
        assert_eq!(*app.world().resource::<State<EngineState>>().get(), EngineState::Idle, "so the engine is idle");
        let mut players = app.world_mut().query_filtered::<Entity, With<Player>>();
        assert!(players.iter(app.world()).next().is_none(), "and there is no commando yet");
    }

    /// Taking up the offer starts the run the ordinary way: the same
    /// `NewRun` the engine runs for a restart, so everything Foundry sets up
    /// is set up exactly once.
    #[test]
    fn taking_up_the_offer_starts_the_run() {
        let mut app = crate::testing::headless(rl_engine::rl_core::RunSeed(3));
        app.insert_resource(Title::default());
        app.update();
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Enter);
        app.update();
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
        app.update();

        assert!(!app.world().resource::<Title>().up, "the screen is down");
        assert!(app.world().contains_resource::<WorldMap>(), "the first deck is built");
        let mut players = app.world_mut().query_filtered::<Entity, With<Player>>();
        assert_eq!(players.iter(app.world()).count(), 1, "one commando, dropped in once");
    }

    /// The text of a run saved on arriving on `deck`, played in an app of
    /// its own.
    fn a_save_on(deck: u32) -> String {
        use rl_engine::rl_save::SaveBackend as _;
        let mut app = crate::testing::headless(rl_engine::rl_core::RunSeed(5));
        crate::testing::arrive_on(&mut app, deck);
        crate::testing::settle(&mut app);
        app.world().resource::<rl_engine::rl_save::Saves>().load(crate::save::SLOT).unwrap().expect("arriving wrote it")
    }

    /// The title up, with `save` in the slot before anything ran.
    fn title_over(save: Option<&str>) -> App {
        use rl_engine::rl_save::SaveBackend as _;
        let mut app = crate::testing::headless(rl_engine::rl_core::RunSeed(6));
        app.insert_resource(Title::default());
        if let Some(text) = save {
            app.world().resource::<rl_engine::rl_save::Saves>().persist(crate::save::SLOT, text).unwrap();
        }
        app.update();
        app
    }

    /// Presses `key` for one frame and lets it go, so the same key pressed
    /// again is a new press.
    fn key(app: &mut App, key: KeyCode) {
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
        app.update();
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.release(key);
        input.clear();
        app.update();
    }

    fn slot_holds_a_save(app: &App) -> bool {
        use rl_engine::rl_save::SaveBackend as _;
        app.world().resource::<rl_engine::rl_save::Saves>().exists(crate::save::SLOT)
    }

    fn deck(app: &mut App) -> u32 {
        let me = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).expect("a commando");
        crate::decks::deck_of(app.world().get::<rl_engine::rl_bevy::OnMap>(me).map_or(rl_engine::rl_bevy::MapId::SURFACE, |m| m.0))
    }

    /// Continue is taken only when the slot holds a save this build can
    /// read, and then it is the row picked out to begin with.
    #[test]
    fn continue_is_offered_only_with_a_readable_save() {
        let none = title_over(None);
        assert_eq!(none.world().resource::<Title>().save, SaveOnDisk::None);
        let damaged = title_over(Some("not a save"));
        assert_eq!(damaged.world().resource::<Title>().save, SaveOnDisk::Unreadable);
        for app in [&none, &damaged] {
            let title = app.world().resource::<Title>();
            assert_eq!(Choice::all()[title.picked], Choice::NewGame, "New Game is picked out");
            let mut at = title.picked;
            for _ in 0..6 {
                at = step(at, 1, title.save);
                assert_ne!(Choice::all()[at], Choice::Continue, "and the cursor never lands on Continue");
            }
        }
        let readable = title_over(Some(&a_save_on(2)));
        let title = readable.world().resource::<Title>();
        assert_eq!(title.save, SaveOnDisk::Readable);
        assert_eq!(Choice::all()[title.picked], Choice::Continue, "Continue is picked out");
    }

    /// Continue picks the run up where it was left.
    #[test]
    fn continue_resumes_the_saved_run() {
        let mut app = title_over(Some(&a_save_on(2)));
        key(&mut app, KeyCode::Enter);
        crate::testing::settle(&mut app);
        assert!(!app.world().resource::<Title>().up, "the screen is down");
        assert_eq!(deck(&mut app), 2, "on the deck it was left on");
        let continuing = app.world().resource::<rl_engine::rl_ui::MessageLog>().iter().any(|e| e.text == "Continuing on deck 2.");
        assert!(continuing, "and the log says so");
    }

    /// New Game over a save asks first, and No keeps the save and the
    /// screen as they were.
    #[test]
    fn new_game_over_a_save_asks_and_no_keeps_it() {
        let mut app = title_over(Some(&a_save_on(2)));
        key(&mut app, KeyCode::ArrowDown);
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.world().resource::<Title>().confirm, Some(false), "it asks, with No picked out");
        key(&mut app, KeyCode::Enter);
        let title = app.world().resource::<Title>();
        assert_eq!(title.confirm, None, "the question is gone");
        assert!(title.up, "and the screen is still up");
        assert_eq!(Choice::all()[title.picked], Choice::NewGame, "with New Game picked out");
        assert!(slot_holds_a_save(&app), "and the save is kept");
    }

    /// Yes abandons the saved run and starts a fresh one on deck one.
    #[test]
    fn new_game_over_a_save_and_yes_starts_fresh() {
        let mut app = title_over(Some(&a_save_on(2)));
        key(&mut app, KeyCode::ArrowDown);
        key(&mut app, KeyCode::Enter);
        key(&mut app, KeyCode::ArrowUp);
        assert_eq!(app.world().resource::<Title>().confirm, Some(true), "Yes picked out");
        key(&mut app, KeyCode::Enter);
        crate::testing::settle(&mut app);
        assert!(!app.world().resource::<Title>().up, "the screen is down");
        assert_eq!(deck(&mut app), 1, "a fresh run, on deck one");
    }

    /// A save this build cannot read leaves Continue dim, and New Game
    /// still asks, so it can always be cleared from the menu.
    #[test]
    fn a_damaged_save_leaves_continue_dim_and_new_game_clears_it() {
        let mut app = title_over(Some("not a save"));
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.world().resource::<Title>().confirm, Some(false), "New Game asks");
        key(&mut app, KeyCode::ArrowUp);
        key(&mut app, KeyCode::Enter);
        crate::testing::settle(&mut app);
        assert_eq!(deck(&mut app), 1, "a fresh run");
    }

    /// Every row of the terminal, as text.
    fn screen(terminal: &Terminal) -> Vec<String> {
        (0..terminal.height()).map(|y| (0..terminal.width()).map(|x| terminal.get(x, y).map_or(' ', |c| c.glyph)).collect()).collect()
    }

    /// The menu is three words and nothing else: no line under a row saying
    /// what it does, and no row of keys along the bottom.
    #[test]
    fn the_menu_reads_continue_new_game_and_exit_and_nothing_more() {
        let mut terminal = Terminal::new(100, 40, Vec2::new(10.0, 20.0));
        let palette = Palette::default();
        paint_menu(&mut terminal, &Title::default(), &palette);
        let rows = screen(&terminal);
        let menu: Vec<&str> = rows[rows::MENU as usize..].iter().map(|r| r.trim()).filter(|r| !r.is_empty()).collect();
        assert_eq!(menu, vec!["Continue", "> New Game", "Exit"], "{rows:#?}");
    }

    /// Continue is there to say a run can be continued one day, and until
    /// there is a save it is drawn dim and the cursor steps over it.
    #[test]
    fn continue_is_drawn_dim_and_the_cursor_never_lands_on_it() {
        let mut title = Title::default();
        assert_eq!(Choice::all()[title.picked], Choice::NewGame, "the cursor starts on New Game");
        let mut seen = Vec::new();
        for _ in 0..6 {
            title.picked = step(title.picked, 1, title.save);
            seen.push(Choice::all()[title.picked]);
        }
        for _ in 0..6 {
            title.picked = step(title.picked, -1, title.save);
            seen.push(Choice::all()[title.picked]);
        }
        assert!(!seen.contains(&Choice::Continue), "{seen:?}");

        let mut terminal = Terminal::new(100, 40, Vec2::new(10.0, 20.0));
        let palette = Palette::default();
        paint_menu(&mut terminal, &Title::default(), &palette);
        let rows = screen(&terminal);
        let y = rows.iter().position(|r| r.trim() == "Continue").expect("Continue is drawn") as i32;
        let x = rows[y as usize].find('C').unwrap() as i32;
        assert_eq!(terminal.get(x, y).unwrap().fg, palette.get(Tones::MUTED), "and drawn dim");
        let y = rows.iter().position(|r| r.trim() == "> New Game").expect("New Game is drawn picked") as i32;
        let x = rows[y as usize].find('N').unwrap() as i32;
        assert_eq!(terminal.get(x, y).unwrap().fg, palette.get(Tones::TITLE), "while the picked row stands out");
    }

    /// While New Game's question is asked it stands where the menu stood:
    /// the question, and its two answers under it with the picked one
    /// marked.
    #[test]
    fn the_question_over_new_game_stands_where_the_menu_stood() {
        let mut terminal = Terminal::new(100, 40, Vec2::new(10.0, 20.0));
        let palette = Palette::default();
        let title = Title { confirm: Some(false), save: SaveOnDisk::Readable, ..Title::default() };
        paint_menu(&mut terminal, &title, &palette);
        let rows = screen(&terminal);
        let menu: Vec<&str> = rows[rows::MENU as usize..].iter().map(|r| r.trim()).filter(|r| !r.is_empty()).collect();
        assert_eq!(menu, vec!["Abandon the run in progress?", "> No      Yes"], "{rows:#?}");
    }

    /// Exit leaves, the same as it always did.
    #[test]
    fn exit_leaves() {
        let mut app = crate::testing::headless(rl_engine::rl_core::RunSeed(3));
        app.insert_resource(Title::default());
        app.update();
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ArrowDown);
        app.update();
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Enter);
        app.update();
        assert!(app.should_exit().is_some(), "it asked to exit");
    }

    /// Prints the title screen as the player sees it, for eyeballing the
    /// art: `cargo test -p foundry title_screen -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn title_screen() {
        let mut terminal = Terminal::new(100, 40, Vec2::new(10.0, 20.0));
        let palette = Palette::default();
        paint(&mut terminal, 0.4);
        paint_title(&mut terminal, 0.4);
        paint_menu(&mut terminal, &Title::default(), &palette);
        for y in 0..terminal.height() {
            let row: String = (0..terminal.width()).map(|x| terminal.get(x, y).map(|c| c.glyph).unwrap_or(' ')).collect();
            println!("{row}");
        }
    }
}
