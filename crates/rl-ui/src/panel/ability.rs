//! The ability menu: what you can do with this turn, what each thing does,
//! and what you cannot.
//!
//! A modal the engine runs end to end: [`AbilityKeys::toggle`] opens and
//! closes it, the direction keys walk the rows, and confirming a row
//! writes [`AimAt`] for it, so the targeting cursor opens on it the same
//! way a game's hotkey would have opened it. A game binds nothing but its
//! hotkeys, and needs none of those.
//!
//! The rows are the list; under them, the row picked out is described from
//! its data: what the game wrote about it, what it is aimed at and how it
//! reaches, what it costs and needs, how long it takes and how long until
//! it is ready again, what each effect does, and, when it cannot be used,
//! why not in words the player can act on.

use crate::modal::AddModal;
use bevy::prelude::*;
use rl_bevy::{EngineSet, PresentSet};
use rl_core::{Direction, Rect};
use rl_grid::TargetMode;
use rl_render::{Cell, Terminal};
use rl_rules::ability::Aim;

use crate::controls::{AddControls, Chord, ControlInput, EngineKey, key_name};
use crate::modal::{ModalId, Modals};
use crate::panel::{clear, clip, frame, wrap};
use crate::tone::{Palette, Tones};
use crate::view::ability::{AbilityRow, AbilityView, AbilityViewPlugin, turns};
use crate::view::target::AimAt;

/// The name the ability menu's modal is declared under.
pub const ABILITY_MODAL: &str = "abilities";

/// The keys the menu answers to. Walking the rows, confirming one and
/// closing use the direction and cursor keys every other screen uses.
#[derive(Resource, Debug, Clone)]
pub struct AbilityKeys {
    /// Opens and closes the menu.
    pub toggle: Chord,
}

impl Default for AbilityKeys {
    fn default() -> Self {
        Self { toggle: Chord::key(KeyCode::KeyA) }
    }
}

/// Where the menu is drawn, and what it is called.
#[derive(Resource, Debug, Clone)]
pub struct AbilityLayout {
    /// The terminal cells it occupies.
    pub rect: Rect,
    /// Drawn in the top border. The engine has no word for a spellbook.
    pub title: String,
    /// The heading over what is being described, on the controls screen
    /// and in the menu.
    pub what: String,
}

/// Which row the cursor is on. The menu's own state, since a view holds
/// no cursor.
#[derive(Resource, Debug, Default)]
pub struct AbilityMenu {
    /// The row picked out, clamped to the rows there are when drawn.
    pub selected: usize,
}

impl AbilityMenu {
    /// Moves the cursor `by` rows over `rows` of them, wrapping at either
    /// end.
    pub fn move_by(&mut self, by: i32, rows: usize) {
        if rows == 0 {
            self.selected = 0;
            return;
        }
        self.selected = (self.selected as i64 + by as i64).rem_euclid(rows as i64) as usize;
    }
}

/// Draws [`AbilityView`] as a list with the row picked out described under
/// it, and runs the menu.
///
/// Adds [`AbilityViewPlugin`] if the game has not, and declares the
/// `abilities` modal.
pub struct AbilityPanel(AbilityLayout);

impl AbilityPanel {
    /// The menu in `rect`.
    pub fn new(rect: Rect) -> Self {
        Self(AbilityLayout { rect, title: "Abilities".into(), what: "abilities".into() })
    }

    /// Sets what the top border says.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.0.title = title.into();
        self
    }

    /// Sets what the abilities are called where the engine names them: on
    /// the controls screen, "list what you can call on" becomes "list
    /// your `what`".
    pub fn called(mut self, what: impl Into<String>) -> Self {
        self.0.what = what.into();
        self
    }
}

impl Plugin for AbilityPanel {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<AbilityViewPlugin>() {
            app.add_plugins(AbilityViewPlugin);
        }
        app.add_modal(ABILITY_MODAL);
        app.add_message::<AimAt>();
        app.insert_resource(self.0.clone()).init_resource::<AbilityMenu>().init_resource::<AbilityKeys>();
        app.add_systems(Update, ability_keys.in_set(EngineSet::Input)).add_systems(Update, draw_abilities.in_set(PresentSet::Overlay));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::UiPlugin>(app, "AbilityPanel");
        // After the game's own, so its groups are listed first.
        app.add_control(crate::focus::SCREENS_GROUP, &format!("list your {}", self.0.what), EngineKey::ListAbilities);
    }
}

/// The id of the ability menu's modal, for a game gating its own systems.
///
/// # Panics
/// Panics if [`AbilityPanel`] was not added.
pub fn ability_modal(modals: &Modals) -> ModalId {
    modals.get(ABILITY_MODAL).expect("AbilityPanel declares the abilities modal")
}

/// Opens, closes and walks the menu, and aims the row confirmed.
pub fn ability_keys(
    keys: ControlInput,
    binds: Res<AbilityKeys>,
    view: Res<AbilityView>,
    mut menu: ResMut<AbilityMenu>,
    mut modals: ResMut<Modals>,
    mut aims: MessageWriter<AimAt>,
) {
    let modal = ability_modal(&modals);
    if binds.toggle.just_pressed(keys.input()) && (modals.is_top(modal) || !modals.any_open()) {
        modals.toggle(modal);
        return;
    }
    if !modals.is_top(modal) {
        return;
    }
    let bindings = keys.bindings();
    if keys.input().just_pressed(bindings.cursor.close) {
        modals.close_one(modal);
        return;
    }
    match bindings.directions.just_pressed(keys.input()) {
        Some(Direction::North) => menu.move_by(-1, view.rows.len()),
        Some(Direction::South) => menu.move_by(1, view.rows.len()),
        _ => {}
    }
    if keys.input().just_pressed(bindings.cursor.confirm) || keys.input().just_pressed(bindings.cursor.also_confirm) {
        // Aimed whether or not the gate allows it: the cursor says why not
        // in its banner, and a player who insists gets the refusal in the
        // log, which beats a key that does nothing.
        if let (Some(user), Some(row)) = (view.entity, view.row(menu.selected)) {
            aims.write(AimAt { user, ability: row.ability });
            modals.close_one(modal);
        }
    }
}

/// What an aim and a shape come to, in one phrase: `at a foe, a burst of
/// 2 around a point up to 8 away`.
pub fn reach(aim: Aim, mode: TargetMode) -> String {
    let at = match aim {
        Aim::SelfOnly => "on yourself",
        Aim::Foe => "at a foe",
        Aim::Ally => "at an ally",
        Aim::Ground => "at the ground",
        Aim::Anyone => "at anyone",
    };
    let shape = match mode {
        TargetMode::Own => String::new(),
        TargetMode::Adjacent => "beside you".to_string(),
        TargetMode::Bolt { range } => format!("a bolt up to {range} away"),
        TargetMode::Ball { range, radius } => format!("a burst of {radius} around a point up to {range} away"),
        TargetMode::Beam { range } => format!("a line out to {range}"),
        TargetMode::Cone { length } => format!("a cone {length} long"),
    };
    if shape.is_empty() { at.to_string() } else { format!("{at}, {shape}") }
}

/// The lines describing `row`, the game's words first.
fn describe(row: &AbilityRow, width: usize) -> Vec<(String, crate::tone::ToneId)> {
    let mut lines = Vec::new();
    for line in wrap(&row.description, width) {
        lines.push((line, Tones::TEXT));
    }
    if !row.description.is_empty() {
        lines.push((String::new(), Tones::TEXT));
    }
    // Every line wrapped rather than clipped: a reach cut short reads as a
    // shorter reach.
    let mut muted = |text: String| lines.extend(wrap(&text, width).into_iter().map(|l| (l, Tones::MUTED)));
    muted(reach(row.aim, row.mode));
    let mut spends = Vec::new();
    if !row.costs.is_empty() {
        spends.push(format!("costs {}", row.costs.join(", ")));
    }
    if row.time != 100 {
        spends.push(format!("takes {}", turns(row.time)));
    }
    if row.cooldown > 0 {
        spends.push(format!("again after {}", turns(row.cooldown)));
    }
    if !spends.is_empty() {
        muted(spends.join("; "));
    }
    if !row.requires.is_empty() {
        muted(format!("needs {}", row.requires.join(", ")));
    }
    for effect in &row.effects {
        lines.extend(wrap(effect, width).into_iter().map(|l| (l, Tones::TEXT)));
    }
    for facet in &row.facets {
        lines.extend(wrap(&facet.text, width).into_iter().map(|l| (l, facet.tone)));
    }
    for why in &row.why {
        lines.extend(wrap(why, width).into_iter().map(|l| (l, Tones::BAD)));
    }
    lines
}

/// Paints the menu while its modal is open: the rows, a rule, and the row
/// picked out described under it.
pub fn draw_abilities(
    mut terminal: ResMut<Terminal>,
    mut menu: ResMut<AbilityMenu>,
    layout: Res<AbilityLayout>,
    view: Res<AbilityView>,
    keys: ControlInput,
    modals: Res<Modals>,
    palette: Res<Palette>,
) {
    if !modals.is_open(ability_modal(&modals)) {
        return;
    }
    let rect = layout.rect;
    if rect.width < 12 || rect.height < 6 {
        return;
    }
    let bindings = keys.bindings();
    let hints = format!("\u{2191}\u{2193} pick \u{2022} {} aim \u{2022} {} close", key_name(bindings.cursor.confirm), key_name(bindings.cursor.close));
    clear(&mut terminal, rect, &palette);
    frame(&mut terminal, rect, &layout.title, &hints, &palette);
    let inner = Rect::new(rect.x + 2, rect.y + 1, rect.width - 4, rect.height - 2);
    let surface = palette.get(Tones::SURFACE);
    let width = inner.width.max(0) as usize;

    // The cursor survives the rebuild, clamped to the rows there are now.
    menu.selected = menu.selected.min(view.rows.len().saturating_sub(1));
    if view.rows.is_empty() {
        terminal.print_on(inner.x, inner.y, &clip("Nothing to call on.", width), palette.get(Tones::MUTED), surface);
        return;
    }
    // Half the height to the rows, the rest to the words about the one
    // picked out, and never fewer than three rows.
    let list_rows = ((inner.height / 2) as usize).max(3).min(view.rows.len());
    let first = menu.selected.saturating_sub(list_rows - 1).min(view.rows.len().saturating_sub(list_rows));
    for (i, row) in view.rows.iter().enumerate().skip(first).take(list_rows) {
        let y = inner.y + (i - first) as i32;
        let selected = i == menu.selected;
        let bg = if selected { palette.get(Tones::SELECT) } else { surface };
        let fg = palette.get(if row.ready() { Tones::TEXT } else { Tones::MUTED });
        terminal.fill(Rect::new(inner.x, y, inner.width, 1), Cell::new(' ', fg).on(bg));
        let tag = row.why.first().cloned().unwrap_or_default();
        terminal.print_on(inner.x, y, &clip(&row.label, width.saturating_sub(tag.chars().count() + 1)), fg, bg);
        if !tag.is_empty() {
            terminal.print_on(inner.right() - tag.chars().count() as i32, y, &tag, palette.get(Tones::BAD), bg);
        }
    }
    let rule_y = inner.y + list_rows as i32;
    for x in inner.x..inner.right() {
        terminal.put(x, rule_y, '\u{2500}', palette.get(Tones::FRAME));
    }
    let Some(row) = view.row(menu.selected) else { return };
    for (y, (line, tone)) in (rule_y + 1..inner.bottom()).zip(describe(row, width)) {
        terminal.print_on(inner.x, y, &clip(&line, width), palette.get(tone), surface);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use crate::view::target::harness::{abilities, arm};
    use rl_bevy::{AbilitiesPlugin, AddEngineEffects, Intent, Use};

    fn staged() -> Stage {
        let mut stage = Stage::new_with((AbilitiesPlugin, crate::TargetViewPlugin, AbilityPanel::new(Rect::new(0, 0, 44, 16)).title("Knacks")), |app| {
            app.add_engine_effects();
            abilities(app);
        })
        .screen(44, 16);
        stage.tick();
        arm(&mut stage);
        stage.tick();
        stage
    }

    /// The text inside the frame on row `y`, border and padding cut off.
    fn inside(stage: &Stage, y: i32) -> String {
        stage.row(y).trim_start_matches("\u{2502} ").trim_end_matches('\u{2502}').trim_end().to_string()
    }

    #[test]
    fn the_menu_opens_walks_and_describes_the_row_picked_out() {
        let mut stage = staged();
        assert!(stage.rows().iter().all(|r| r.is_empty()), "nothing until the key");
        stage.press(KeyCode::KeyA);
        assert!(stage.row(0).contains(" Knacks "), "{:?}", stage.row(0));
        assert_eq!(inside(&stage, 1), "bolt", "the first row, picked out");
        assert!(stage.row(15).contains("pick \u{2022} enter aim \u{2022} esc close"), "{:?}", stage.row(15));
        let body: Vec<String> = (6..15).map(|y| inside(&stage, y)).collect();
        assert_eq!(body[0], "a bolt of force", "the game's words first");
        assert_eq!(body[2], "at a foe, a bolt up to 6 away");
        assert_eq!(body[3], "costs 5 focus");
        assert_eq!(body[4], "3 kinetic");

        stage.press(KeyCode::ArrowDown);
        stage.press(KeyCode::ArrowDown);
        stage.press(KeyCode::ArrowDown);
        let body: Vec<String> = (6..15).map(|y| inside(&stage, y)).collect();
        assert!(inside(&stage, 4).starts_with("dear"), "walked to the fourth row: {:?}", inside(&stage, 4));
        assert!(inside(&stage, 4).ends_with("needs 99 focus"), "with why beside it: {:?}", inside(&stage, 4));
        assert!(body.contains(&"needs 99 focus".to_string()), "and under it: {body:?}");

        stage.press(KeyCode::Escape);
        assert!(!stage.app.world().resource::<Modals>().any_open());
    }

    #[test]
    fn confirming_a_row_aims_it_and_the_cursor_opens_on_the_nearest_foe() {
        let mut stage = staged();
        stage.actor("them", 't', 2, 0);
        stage.tick();
        let at = stage.at;
        stage.press(KeyCode::KeyA);
        stage.press(KeyCode::Enter);
        let modals = stage.app.world().resource::<Modals>();
        assert!(!modals.is_open(ability_modal(modals)), "the menu closed");
        assert!(modals.is_open(crate::target_modal(modals)), "and the cursor opened in its place");
        assert_eq!(stage.app.world().resource::<crate::TargetView>().cursor, at.offset(2, 0));

        let _ = stage.app.world_mut().resource_mut::<Messages<Intent<Use>>>().drain().count();
        stage.press(KeyCode::Enter);
        let uses: Vec<Use> = stage.app.world_mut().resource_mut::<Messages<Intent<Use>>>().drain().map(|i| i.action).collect();
        assert_eq!(uses.len(), 1, "one use, from the cursor: {uses:?}");
    }

    #[test]
    fn an_aim_and_a_shape_read_as_one_phrase() {
        assert_eq!(reach(Aim::Foe, TargetMode::Ball { range: 8, radius: 2 }), "at a foe, a burst of 2 around a point up to 8 away");
        assert_eq!(reach(Aim::SelfOnly, TargetMode::Own), "on yourself");
        assert_eq!(reach(Aim::Ground, TargetMode::Bolt { range: 7 }), "at the ground, a bolt up to 7 away");
        assert_eq!(reach(Aim::Foe, TargetMode::Adjacent), "at a foe, beside you");
    }
}
