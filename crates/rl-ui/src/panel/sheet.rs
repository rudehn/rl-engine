//! The character sheet, on a screen of its own.
//!
//! A modal, since a player reading their numbers is not walking. Opened
//! and closed by [`SheetKeys`], which the controls screen lists, and drawn
//! from [`SheetView`] a section at a time: who, the stats with what moved
//! them, what is resisted, the blows, the statuses, what is worn, and
//! whatever the game added. A section with nothing in it is left out, and
//! a screen too short for the rest stops rather than overflows.

use crate::modal::AddModal;
use bevy::prelude::*;
use rl_bevy::{EngineSet, PresentSet};
use rl_core::Rect;
use rl_render::Terminal;
use rl_rules::stats::Op;

use crate::controls::{AddControls, Chord, ControlInput, EngineKey, key_name};
use crate::modal::{ModalId, Modals};
use crate::panel::{clear, clip, frame, section};
use crate::tone::{Palette, Tones};
use crate::view::sheet::{SheetView, SheetViewPlugin, StatLine};

/// The name the sheet's modal is declared under.
pub const SHEET_MODAL: &str = "sheet";

/// The keys the sheet answers to.
#[derive(Resource, Debug, Clone)]
pub struct SheetKeys {
    /// Opens and closes the sheet.
    pub toggle: Chord,
    /// Closes it.
    pub close: KeyCode,
}

impl Default for SheetKeys {
    fn default() -> Self {
        Self { toggle: Chord::key(KeyCode::KeyC), close: KeyCode::Escape }
    }
}

/// Where the sheet is drawn and what its borders say.
#[derive(Resource, Debug, Clone)]
pub struct SheetLayout {
    /// The terminal cells it occupies, border included.
    pub rect: Rect,
    /// The title in the top border.
    pub title: String,
    /// The headings, in the order the sections are drawn: stats,
    /// resists, attacks, statuses, worn.
    pub headings: [String; 5],
}

/// Draws [`SheetView`] as a screen.
///
/// Adds [`SheetViewPlugin`] if the game has not, declares the `sheet`
/// modal, and opens and closes it on [`SheetKeys`].
pub struct SheetPanel(SheetLayout);

impl SheetPanel {
    /// A sheet in `rect`.
    pub fn new(rect: Rect) -> Self {
        Self(SheetLayout { rect, title: "Character".into(), headings: ["Stats".into(), "Resists".into(), "Attacks".into(), "Statuses".into(), "Worn".into()] })
    }

    /// Sets the title in the top border.
    pub fn titled(mut self, title: impl Into<String>) -> Self {
        self.0.title = title.into();
        self
    }

    /// Replaces the five headings: stats, resists, attacks, statuses, worn.
    pub fn headings(mut self, headings: [&str; 5]) -> Self {
        self.0.headings = headings.map(String::from);
        self
    }
}

impl Plugin for SheetPanel {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<SheetViewPlugin>() {
            app.add_plugins(SheetViewPlugin);
        }
        app.init_resource::<SheetKeys>().insert_resource(self.0.clone());
        app.add_modal(SHEET_MODAL);
        app.add_systems(Update, sheet_keys.in_set(EngineSet::Input)).add_systems(Update, draw_sheet.in_set(PresentSet::Overlay));
    }

    fn finish(&self, app: &mut App) {
        // After the game's own, so its groups are listed first.
        app.add_control(crate::focus::SCREENS_GROUP, "read the character sheet", EngineKey::OpenSheet);
    }
}

/// The id of the sheet's modal, for a game gating its own systems.
///
/// # Panics
/// Panics if [`SheetPanel`] was not added.
pub fn sheet_modal(modals: &Modals) -> ModalId {
    modals.get(SHEET_MODAL).expect("SheetPanel declares the sheet modal")
}

/// Opens and closes the sheet.
pub fn sheet_keys(keys: ControlInput, binds: Res<SheetKeys>, mut modals: ResMut<Modals>) {
    let modal = sheet_modal(&modals);
    if binds.toggle.just_pressed(keys.input()) && (modals.is_top(modal) || !modals.any_open()) {
        modals.toggle(modal);
        return;
    }
    if modals.is_top(modal) && keys.input().just_pressed(binds.close) {
        modals.close_one(modal);
    }
}

/// An operation as a player reads it: `+2`, `x110%`, `at least 3`.
pub fn plain_op(op: Op) -> String {
    match op {
        Op::Add(n) => format!("{n:+}"),
        Op::MulPct(pct) => format!("x{pct}%"),
        Op::AtLeast(n) => format!("at least {n}"),
        Op::AtMost(n) => format!("at most {n}"),
    }
}

/// A stat's line: the value, and how it got there when anything moved it.
fn stat_line(line: &StatLine, width: usize) -> String {
    let mut text = format!("{:<width$} {:>4}", line.name, line.value);
    if !line.changes.is_empty() {
        let changes: Vec<String> =
            line.changes.iter().map(|c| if c.from.is_empty() { plain_op(c.op) } else { format!("{} {}", plain_op(c.op), c.from) }).collect();
        text.push_str(&format!("   {} {}", line.base, changes.join(", ")));
    }
    text
}

/// Paints the sheet, while it is open.
pub fn draw_sheet(
    mut terminal: ResMut<Terminal>,
    layout: Res<SheetLayout>,
    view: Res<SheetView>,
    keys: Res<SheetKeys>,
    palette: Res<Palette>,
    modals: Res<Modals>,
) {
    if !modals.is_open(sheet_modal(&modals)) {
        return;
    }
    let rect = layout.rect;
    if rect.width < 16 || rect.height < 4 {
        return;
    }
    clear(&mut terminal, rect, &palette);
    frame(&mut terminal, rect, &layout.title, &format!("{} close", key_name(keys.close)), &palette);
    let inner = Rect::new(rect.x + 2, rect.y + 1, rect.width - 4, rect.height - 2);
    let bg = palette.get(Tones::SURFACE);
    let width = inner.width.max(0) as usize;
    let bottom = inner.bottom();
    let mut y = inner.y;
    if view.entity.is_none() {
        terminal.print_on(inner.x, y, "Nobody.", palette.get(Tones::MUTED), bg);
        return;
    }

    // Who, and the three numbers every roguelike shows first.
    let mut x = inner.x;
    if let Some(glyph) = view.glyph {
        terminal.print_on(x, y, &glyph.ch.to_string(), glyph.fg, bg);
        x += 2;
    }
    terminal.print_on(x, y, &clip(&view.label, width.saturating_sub(2)), palette.get(Tones::TITLE), bg);
    let mut vitals = Vec::new();
    if let Some((hp, max)) = view.health {
        vitals.push(format!("health {hp}/{max}"));
    }
    if let Some(armor) = view.armor {
        vitals.push(format!("armor {armor}"));
    }
    if let Some(speed) = view.speed {
        vitals.push(format!("speed {speed}"));
    }
    let vitals = vitals.join("   ");
    let at = inner.right() - vitals.chars().count() as i32;
    if at > x + view.label.chars().count() as i32 + 1 {
        terminal.print_on(at, y, &vitals, palette.get(Tones::TEXT), bg);
    }
    y += 2;

    let name_width = |names: &[&str]| names.iter().map(|n| n.chars().count()).max().unwrap_or(0);
    let [stats, resists, attacks, statuses, worn] = &layout.headings;
    let mut sections: Vec<(&str, Vec<String>)> = Vec::new();
    if !view.stats.is_empty() {
        let w = name_width(&view.stats.iter().map(|s| s.name.as_str()).collect::<Vec<_>>());
        sections.push((stats, view.stats.iter().map(|s| stat_line(s, w)).collect()));
    }
    if !view.resists.is_empty() {
        let w = name_width(&view.resists.iter().map(|r| r.name.as_str()).collect::<Vec<_>>());
        sections.push((resists, view.resists.iter().map(|r| format!("{:<w$} {:>4}%", r.name, r.pct)).collect()));
    }
    if !view.strikes.is_empty() {
        let lines = view
            .strikes
            .iter()
            .enumerate()
            .map(|(i, s)| match (s.range, i) {
                (Some(range), _) => format!("{} {}, range {range}", s.dice, s.kind),
                (None, 0) => format!("{} {}", s.dice, s.kind),
                (None, _) => format!("and {} {}", s.dice, s.kind),
            })
            .collect();
        sections.push((attacks, lines));
    }
    if !view.statuses.is_empty() {
        let lines = view
            .statuses
            .iter()
            .map(|s| {
                let mut does: Vec<String> = s.modifies.iter().map(|(stat, op)| format!("{} {stat}", plain_op(*op))).collect();
                if let Some((kind, amount)) = &s.ticks {
                    does.push(format!("{amount} {kind} a turn"));
                }
                let turns = if s.turns == 1 { "1 turn".to_string() } else { format!("{} turns", s.turns) };
                if does.is_empty() { format!("{} {turns}", s.name) } else { format!("{} {turns}   {}", s.name, does.join(", ")) }
            })
            .collect();
        sections.push((statuses, lines));
    }
    if !view.worn.is_empty() {
        let w = name_width(&view.worn.iter().map(|s| s.name.as_str()).collect::<Vec<_>>());
        sections.push((worn, view.worn.iter().map(|s| format!("{:<w$} {}", s.name, s.item.as_deref().unwrap_or("\u{2014}"))).collect()));
    }

    for (heading, lines) in sections {
        if y + 3 > bottom {
            break;
        }
        section(&mut terminal, inner, y, heading, None, &palette);
        y += 2;
        for line in lines {
            if y >= bottom {
                break;
            }
            terminal.print_on(inner.x, y, &clip(&line, width), palette.get(Tones::TEXT), bg);
            y += 1;
        }
        y += 1;
    }
    for facet in &view.facets {
        if y >= bottom {
            break;
        }
        terminal.print_on(inner.x, y, &clip(&facet.text, width), palette.get(facet.tone), bg);
        y += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use rl_bevy::{Afflict, Afflicted, Registries, Resists, StatBlock};
    use rl_rules::content::Registry;
    use rl_rules::{StatDef, StatusDef, stats::Modifier};

    fn stage() -> Stage {
        let mut stage = Stage::new_with(SheetPanel::new(Rect::new(0, 0, 50, 20)), |app| {
            let mut registries = app.world_mut().resource_mut::<Registries>();
            registries.stats = Registry::from_defs(vec![StatDef::new("might", 10)]).unwrap();
            let might = registries.stats.expect("might");
            registries.statuses = Registry::from_defs(vec![StatusDef::new("weak").modifies(might, Op::Add(-3))]).unwrap();
        })
        .screen(50, 20);
        let (player, kind) = (stage.player, stage.kind);
        let might = stage.app.world().resource::<Registries>().stats.expect("might");
        let mut stats = rl_rules::Stats::new();
        stats.add(Modifier::new(might, Op::Add(2), 77));
        let mut resists = rl_rules::Resistances::new();
        resists.set(kind, 25);
        stage.app.world_mut().entity_mut(player).insert((StatBlock(stats), Resists(resists), Afflicted::default()));
        stage.app.add_systems(Update, (|mut view: ResMut<SheetView>| view.name_source(77, "a ring")).in_set(crate::ViewSet::Annotate));
        stage.tick();
        stage
    }

    #[test]
    fn the_sheet_opens_on_its_key_and_says_where_every_number_came_from() {
        let mut stage = stage();
        let player = stage.player;
        let weak = stage.app.world().resource::<Registries>().statuses.expect("weak");
        stage.app.world_mut().write_message(Afflict { target: player, status: weak, turns: 4, by: None });
        stage.tick();
        assert!(stage.rows().iter().all(|r| r.is_empty()), "nothing drawn until the key");

        stage.press(KeyCode::KeyC);
        let rows = stage.rows();
        assert!(rows[0].contains(" Character "), "{:?}", rows[0]);
        assert_eq!(rows[1], "\u{2502} @ you       health 30/30   armor 0   speed 100 \u{2502}", "the numbers right-aligned: {:?}", rows[1]);
        // The text inside the frame, its border and padding cut off.
        let inside = |needle: &str| {
            let row = rows.iter().find(|r| r.contains(needle)).unwrap_or_else(|| panic!("no {needle:?} in {rows:#?}"));
            row.trim_start_matches("\u{2502} ").trim_end_matches('\u{2502}').trim_end().to_string()
        };
        assert_eq!(inside("Stats"), "Stats");
        assert_eq!(inside("might"), "might    9   10 +2 a ring, -3 weak", "the value, then the base and what moved it");
        assert_eq!(inside("kinetic   25"), "kinetic   25%");
        assert_eq!(inside("1d6"), "1d6 kinetic");
        assert_eq!(inside("weak 4"), "weak 4 turns   -3 might");
        assert!(rows[19].contains("esc close"), "{:?}", rows[19]);

        stage.press(KeyCode::Escape);
        assert!(!stage.app.world().resource::<Modals>().any_open());
    }

    #[test]
    fn the_sheet_is_on_the_controls_screen_under_the_key_it_answers_to() {
        let mut stage = Stage::new((SheetPanel::new(Rect::new(0, 0, 50, 20)), crate::ControlsPanel::new(Rect::new(0, 0, 60, 30)))).screen(60, 30);
        stage.app.world_mut().resource_mut::<SheetKeys>().toggle = Chord::shift(KeyCode::Digit2);
        stage.chord(&[KeyCode::ShiftLeft, KeyCode::Slash]);
        let rows = stage.rows();
        let row = rows.iter().find(|r| r.contains("read the character sheet")).unwrap_or_else(|| panic!("{rows:#?}"));
        assert!(row.starts_with("\u{2502} @ "), "listed under the key as rebound: {row:?}");
    }
}
