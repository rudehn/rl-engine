//! The ability menu: what you can do with this turn, and what you cannot.

use bevy::prelude::*;
use rl_bevy::PresentSet;
use rl_core::Rect;
use rl_render::Terminal;

use crate::menu::{ListMenu, MenuRow, draw_menu};
use crate::modal::{ModalId, Modals};
use crate::tone::{Palette, Tones};
use crate::view::ability::{AbilityView, AbilityViewPlugin, plain};

/// The name the ability menu's modal is declared under.
pub const ABILITY_MODAL: &str = "abilities";

/// Where the menu is drawn, and what it is called.
#[derive(Resource, Debug, Clone)]
pub struct AbilityLayout {
    /// The terminal cells it occupies.
    pub rect: Rect,
    /// Drawn in the top border. The engine has no word for a spellbook.
    pub title: String,
    /// Key hints in the bottom border.
    pub hints: String,
}

/// Which row the cursor is on. The menu's own state, since a view holds
/// no cursor.
#[derive(Resource, Debug, Default)]
pub struct AbilityMenu {
    /// The widget, rebuilt from the view every frame it is open.
    pub list: ListMenu,
}

/// Draws [`AbilityView`] as a list.
///
/// Adds [`AbilityViewPlugin`] if the game has not, and declares the
/// `abilities` modal. Opening and closing it is the game's, since the key
/// that opens a menu is the game's to bind.
pub struct AbilityPanel(AbilityLayout);

impl AbilityPanel {
    /// The menu in `rect`.
    pub fn new(rect: Rect) -> Self {
        Self(AbilityLayout { rect, title: "Abilities".into(), hints: String::new() })
    }

    /// Sets what the top border says.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.0.title = title.into();
        self
    }

    /// Sets the key hints in the bottom border.
    pub fn hints(mut self, hints: impl Into<String>) -> Self {
        self.0.hints = hints.into();
        self
    }
}

impl Plugin for AbilityPanel {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<AbilityViewPlugin>() {
            app.add_plugins(AbilityViewPlugin);
        }
        app.world_mut().resource_mut::<Modals>().declare(ABILITY_MODAL);
        app.insert_resource(self.0.clone()).init_resource::<AbilityMenu>().add_systems(Update, draw_abilities.in_set(PresentSet::Overlay));
    }
}

/// The id of the ability menu's modal, for a game gating its own systems.
///
/// # Panics
/// Panics if [`AbilityPanel`] was not added.
pub fn ability_modal(modals: &Modals) -> ModalId {
    modals.get(ABILITY_MODAL).expect("AbilityPanel declares the abilities modal")
}

/// Paints the menu while its modal is open.
pub fn draw_abilities(
    mut terminal: ResMut<Terminal>,
    mut menu: ResMut<AbilityMenu>,
    layout: Res<AbilityLayout>,
    view: Res<AbilityView>,
    modals: Res<Modals>,
    palette: Res<Palette>,
) {
    if !modals.is_open(ability_modal(&modals)) {
        return;
    }
    let rows: Vec<MenuRow> = view
        .rows
        .iter()
        .map(|row| {
            // The game's own words first, then the engine's plain ones.
            let note = match (row.facets.first(), row.blocked.first()) {
                (Some(facet), _) => facet.text.clone(),
                (None, Some(reason)) => plain(reason).to_string(),
                (None, None) => String::new(),
            };
            MenuRow::new(row.label.clone()).tag(note).toned(if row.ready() { Tones::TEXT } else { Tones::MUTED })
        })
        .collect();
    menu.list.title = layout.title.clone();
    menu.list.hints = layout.hints.clone();
    menu.list.empty = "Nothing to call on.".into();
    menu.list.set_rows(rows);
    draw_menu(&mut terminal, layout.rect, &menu.list, &palette);
}
