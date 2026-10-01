//! The settings screen: every declared setting under its heading, and the
//! keys that change them.
//!
//! It lists [`Settings`] and nothing else, so
//! what it shows is what the engine and the game read. It does not know
//! what any setting does: whoever declared one watches the registry.
//!
//! Unlike every other screen it is drawn after the whole of
//! `EngineSet::Present` and read in any engine state, because it is the
//! one screen a game opens before there is a world, from its own title
//! screen, where no presenter runs; and because it must cover the menu
//! that opened it.

use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_core::{Direction, Rect};
use rl_render::{Cell, Terminal};

use crate::controls::{AddControls, ControlId, ControlInput, key_name};
use crate::modal::{AddModal, ModalId, Modals};
use crate::panel::{clear, clip, frame};
use crate::tone::{Palette, Tones};

/// The name the settings screen's modal is declared under.
pub const SETTINGS_MODAL: &str = "settings";

/// Where the screen is drawn.
#[derive(Resource, Debug, Clone)]
pub struct SettingsLayout {
    /// The most of the terminal it may occupy; drawn only as far down as
    /// its rows need.
    pub rect: Rect,
}

/// Which row is picked out, counting settings and not headings.
#[derive(Resource, Debug, Default)]
pub struct SettingsScreen {
    /// The row picked out.
    pub selected: usize,
}

/// The control each keyed setting was declared as, so the controls screen
/// lists the key and this screen reads it through the registry.
#[derive(Resource, Debug, Default)]
struct SettingKeys(Vec<(SettingId, ControlId)>);

/// The settings screen.
pub struct SettingsPanel(SettingsLayout);

impl SettingsPanel {
    /// The screen in `rect`, from its top-left corner down as far as its
    /// rows need.
    pub fn new(rect: Rect) -> Self {
        Self(SettingsLayout { rect })
    }
}

impl Plugin for SettingsPanel {
    fn build(&self, app: &mut App) {
        app.add_modal(SETTINGS_MODAL);
        app.init_resource::<Settings>().init_resource::<SettingsScreen>().init_resource::<SettingKeys>().insert_resource(self.0.clone());
        // After the menu's keys, which are this crate's own: the key that
        // closes this screen must not be read by the menu under it in the
        // same frame. Outside the engine's sets, since those do not run
        // before a run exists.
        app.add_systems(Update, settings_keys.after(crate::game_menu::menu_keys).before(EngineSet::Input))
            .add_systems(Update, draw_settings.after(EngineSet::Present));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::UiPlugin>(app, "SettingsPanel");
        let keyed: Vec<_> =
            app.world().resource::<Settings>().iter().filter_map(|(id, s, _)| s.key.map(|key| (id, s.group.clone(), s.label.to_lowercase(), key))).collect();
        let mut keys = Vec::new();
        for (id, group, label, key) in keyed {
            keys.push((id, app.add_control(&group, &format!("switch {label}"), key)));
        }
        app.insert_resource(SettingKeys(keys));
    }
}

/// The id of the settings screen's modal, or `None` when the panel was
/// not added: a menu or a title screen offers the row only when it is.
pub fn settings_modal(modals: &Modals) -> Option<ModalId> {
    modals.get(SETTINGS_MODAL)
}

/// The settings in the order they are drawn: a group stands where its
/// first setting was declared, with every other setting of that group
/// under it. One function for the keys and the drawing, so the row a key
/// picks is the row that is highlighted.
fn ordered(settings: &Settings) -> Vec<SettingId> {
    let mut groups: Vec<&str> = Vec::new();
    for (_, setting, _) in settings.iter() {
        if !groups.contains(&setting.group.as_str()) {
            groups.push(&setting.group);
        }
    }
    groups.into_iter().flat_map(|group| settings.iter().filter(move |(_, s, _)| s.group == group).map(|(id, ..)| id)).collect()
}

/// Reads a setting's own key, and the screen's keys while it is on top.
///
/// A frame that opened the screen reads nothing more: the key that opened
/// it is the confirm key, still down, and it would change the first row.
fn settings_keys(keys: ControlInput, bound: Res<SettingKeys>, mut screen: ResMut<SettingsScreen>, mut modals: ResMut<Modals>, mut settings: ResMut<Settings>) {
    let Some(modal) = settings_modal(&modals) else { return };
    let top = modals.is_top(modal);
    if top && modals.just_opened() {
        screen.selected = 0;
        return;
    }
    if top || !modals.any_open() {
        for (id, control) in &bound.0 {
            if keys.just_pressed(*control) {
                settings.cycle(*id, 1);
            }
        }
    }
    if !top || modals.closing() {
        return;
    }
    let (input, bindings) = (keys.input(), keys.bindings());
    if input.just_pressed(bindings.cursor.close) {
        modals.close_one(modal);
        return;
    }
    let ids = ordered(&settings);
    if ids.is_empty() {
        return;
    }
    let picked = screen.selected.min(ids.len() - 1);
    let confirm = input.just_pressed(bindings.cursor.confirm) || input.just_pressed(bindings.cursor.also_confirm);
    match bindings.directions.just_pressed(input) {
        Some(Direction::North) => screen.selected = (picked + ids.len() - 1) % ids.len(),
        Some(Direction::South) => screen.selected = (picked + 1) % ids.len(),
        Some(Direction::West) => settings.cycle(ids[picked], -1),
        Some(Direction::East) => settings.cycle(ids[picked], 1),
        _ if confirm => settings.cycle(ids[picked], 1),
        _ => {}
    }
}

/// What the screen is drawn from.
#[derive(bevy::ecs::system::SystemParam)]
struct Screen<'w> {
    layout: Res<'w, SettingsLayout>,
    screen: Res<'w, SettingsScreen>,
    modals: Res<'w, Modals>,
    settings: Res<'w, Settings>,
    keys: ControlInput<'w>,
    palette: Res<'w, Palette>,
}

/// Paints the screen while it is open: each group's heading, and under it
/// each setting with its choice at the right edge.
fn draw_settings(terminal: Option<ResMut<Terminal>>, screen: Screen) {
    let Screen { layout, screen, modals, settings, keys, palette } = &screen;
    let Some(mut terminal) = terminal else { return };
    if !settings_modal(modals).is_some_and(|modal| modals.is_open(modal)) {
        return;
    }
    let rect = layout.rect;
    if rect.width < 16 || rect.height < 4 {
        return;
    }
    let ids = ordered(settings);
    let mut headings = 0;
    for (i, id) in ids.iter().enumerate() {
        if i == 0 || settings.get(ids[i - 1]).group != settings.get(*id).group {
            headings += 1;
        }
    }
    let height = ((ids.len() + headings) as i32 + 2).min(rect.height);
    let rect = Rect::new(rect.x, rect.y, rect.width, height);
    let hints = format!("\u{2191}\u{2193} pick \u{2022} \u{2190}\u{2192} change \u{2022} {} back", key_name(keys.bindings().cursor.close));
    clear(&mut terminal, rect, palette);
    frame(&mut terminal, rect, "Settings", &hints, palette);
    let inner = Rect::new(rect.x + 2, rect.y + 1, rect.width - 4, rect.height - 2);
    let (surface, text) = (palette.get(Tones::SURFACE), palette.get(Tones::TEXT));
    let picked = screen.selected.min(ids.len().saturating_sub(1));
    let width = inner.width as usize;
    let mut y = inner.y;
    for (row, id) in ids.iter().enumerate() {
        let setting = settings.get(*id);
        if row == 0 || settings.get(ids[row - 1]).group != setting.group {
            if y >= inner.bottom() {
                break;
            }
            terminal.print_on(inner.x, y, &clip(&setting.group, width), palette.get(Tones::TITLE), surface);
            y += 1;
        }
        if y >= inner.bottom() {
            break;
        }
        let bg = if row == picked { palette.get(Tones::SELECT) } else { surface };
        terminal.fill(Rect::new(inner.x, y, inner.width, 1), Cell::new(' ', text).on(bg));
        let choice = clip(settings.choice(*id), width.saturating_sub(4));
        let room = width.saturating_sub(choice.chars().count() + 3);
        terminal.print_on(inner.x + 1, y, &clip(&setting.label, room), text, bg);
        terminal.print_on(inner.right() - 1 - choice.chars().count() as i32, y, &choice, text, bg);
        y += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_menu::GameMenuPanel;
    use crate::harness::Stage;

    fn staged() -> Stage {
        let rect = Rect::new(0, 0, 40, 12);
        let mut stage = Stage::new_with((GameMenuPanel::new(rect), SettingsPanel::new(rect), crate::InventoryPanel::new(rect)), |app| {
            app.add_setting(Setting::new("glow", "Display", "Glow", ["Off", "On"]).key(KeyCode::F11));
            app.add_setting(Setting::new("pace", "Play", "Pace", ["Slow", "Even", "Fast"]).default_choice(1));
        })
        .screen(40, 12);
        stage.tick();
        stage
    }

    fn choice(stage: &Stage, name: &str) -> String {
        let settings = stage.app.world().resource::<Settings>();
        settings.choice(settings.find(name).unwrap()).to_string()
    }

    /// Opens the menu and takes its Settings row.
    fn open(stage: &mut Stage) {
        stage.press(KeyCode::Escape);
        for _ in 0..3 {
            stage.press(KeyCode::ArrowDown);
        }
        stage.press(KeyCode::Enter);
    }

    #[test]
    fn the_menu_row_opens_the_screen_with_every_setting_under_its_heading() {
        let mut stage = staged();
        open(&mut stage);
        assert!(stage.row(0).contains(" Settings "), "{:?}", stage.row(0));
        let rows = stage.rows().join("\n");
        assert!(rows.contains("Display") && rows.contains("Glow") && rows.contains("Off"), "{rows}");
        assert!(rows.contains("Play") && rows.contains("Pace") && rows.contains("Even"), "{rows}");
    }

    /// The menu is as tall as its rows and so is this screen, and the two
    /// are seldom the same height: a menu left drawn under it shows its
    /// last rows below the frame.
    #[test]
    fn the_menu_is_not_drawn_under_the_screen_and_is_back_when_it_closes() {
        // The two apart on the terminal, so a menu still drawn is seen.
        let mut stage = Stage::new_with((GameMenuPanel::new(Rect::new(0, 0, 40, 8)), SettingsPanel::new(Rect::new(0, 8, 40, 4))), |app| {
            app.add_setting(Setting::new("glow", "Display", "Glow", ["Off", "On"]));
        })
        .screen(40, 12);
        open(&mut stage);
        // A game's map repaints the terminal every frame; this harness has
        // no map, so wipe what earlier frames left and draw one more.
        stage.app.world_mut().resource_mut::<Terminal>().clear(Color::BLACK);
        stage.tick();
        let rows = stage.rows().join("\n");
        assert!(rows.contains("Glow"), "{rows}");
        assert!(!rows.contains("Quit") && !rows.contains("Back to the run"), "{rows}");
        stage.press(KeyCode::Escape);
        assert!(stage.rows().join("\n").contains("Quit"));
    }

    #[test]
    fn the_key_that_opened_the_screen_does_not_also_change_the_first_setting() {
        let mut stage = staged();
        open(&mut stage);
        assert_eq!(choice(&stage, "glow"), "Off");
    }

    #[test]
    fn left_and_right_change_the_picked_row_and_down_picks_the_next() {
        let mut stage = staged();
        open(&mut stage);
        stage.press(KeyCode::ArrowRight);
        assert_eq!(choice(&stage, "glow"), "On");
        stage.press(KeyCode::ArrowDown);
        stage.press(KeyCode::ArrowLeft);
        assert_eq!(choice(&stage, "pace"), "Slow");
        stage.press(KeyCode::Enter);
        assert_eq!(choice(&stage, "pace"), "Even", "confirm cycles forward");
        assert_eq!(choice(&stage, "glow"), "On", "the other row is untouched");
    }

    #[test]
    fn the_close_key_goes_back_to_the_menu_and_not_past_it() {
        let mut stage = staged();
        open(&mut stage);
        stage.press(KeyCode::Escape);
        assert!(stage.row(0).contains(" Menu "), "{:?}", stage.row(0));
        assert!(stage.app.world().resource::<Modals>().any_open());
    }

    #[test]
    fn a_settings_key_changes_it_with_nothing_open_and_is_listed_as_a_control() {
        let mut stage = staged();
        stage.press(KeyCode::F11);
        assert_eq!(choice(&stage, "glow"), "On");
        let controls = stage.app.world().resource::<crate::Controls>();
        assert!(controls.find("Display", "switch glow").is_some());
    }

    #[test]
    fn a_settings_key_does_nothing_while_another_screen_is_on_top() {
        let mut stage = staged();
        let bag = stage.app.world().resource::<Modals>().get(crate::INVENTORY_MODAL).unwrap();
        stage.app.world_mut().resource_mut::<Modals>().open(bag);
        stage.tick();
        stage.press(KeyCode::F11);
        assert_eq!(choice(&stage, "glow"), "Off");
    }

    #[test]
    fn the_menu_offers_no_settings_row_when_nothing_is_declared() {
        let rect = Rect::new(0, 0, 40, 12);
        let mut stage = Stage::new((GameMenuPanel::new(rect), SettingsPanel::new(rect))).screen(40, 12);
        stage.press(KeyCode::Escape);
        assert!(!stage.rows().join("\n").contains("Settings"));
    }

    #[test]
    fn rows_are_picked_in_the_order_they_are_drawn_when_a_group_is_declared_in_two_parts() {
        let rect = Rect::new(0, 0, 40, 12);
        let mut stage = Stage::new_with((GameMenuPanel::new(rect), SettingsPanel::new(rect)), |app| {
            app.add_setting(Setting::new("a", "Display", "A", ["Off", "On"]));
            app.add_setting(Setting::new("b", "Play", "B", ["Off", "On"]));
            app.add_setting(Setting::new("c", "Display", "C", ["Off", "On"]));
        })
        .screen(40, 12);
        open(&mut stage);
        stage.press(KeyCode::ArrowDown);
        stage.press(KeyCode::ArrowRight);
        assert_eq!(choice(&stage, "c"), "On", "the second row drawn is C, under Display");
        assert_eq!(choice(&stage, "b"), "Off");
    }
}
