//! What you opened, and what you take out of it.
//!
//! A modal the engine runs end to end, like the bag: it opens itself when
//! the engine says a container was opened, the direction keys walk the
//! rows, one key takes the row picked out, one takes everything, and the
//! cursors' close key puts it away. A game binds nothing.
//!
//! Unlike the bag, taking does not close the screen. Opening the
//! container already cost what its definition said, and a crate emptied a
//! piece at a time would otherwise cost a screen a time; each take spends
//! a turn, the screen stays up, and the player closes it when done. It
//! closes itself when the container goes out of reach, since whatever
//! moved the player away has already happened.
//!
//! Take-only: putting things back is a stash mechanic, and the engine has
//! no opinion about stashes.

use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_bevy::{EngineSet, PresentSet};
use rl_core::{Direction, Rect};
use rl_render::{Cell, Terminal};

use crate::controls::{Chord, ControlInput, key_name};
use crate::modal::{AddModal, ModalId, Modals};
use crate::panel::{clear, clip, frame};
use crate::tone::{Palette, Tones};
use crate::view::container::{ContainerView, ContainerViewPlugin, OpenContainer};

/// The name the container's modal is declared under.
pub const CONTAINER_MODAL: &str = "container";

/// The keys the container screen answers to, beside the direction keys
/// that walk the rows and the cursors' keys that take and close.
///
/// A default, not a rule: a game that wants another key inserts its own
/// `ContainerKeys` and the screen reads that, the way every other panel's
/// keys work. `g` because it is what the genre has always meant by
/// getting something, and because the engine's other screens already
/// spoke for `a`, `c`, `i` and `p`.
#[derive(Resource, Debug, Clone)]
pub struct ContainerKeys {
    /// Takes everything.
    pub take_all: Chord,
}

impl Default for ContainerKeys {
    fn default() -> Self {
        Self { take_all: Chord::key(KeyCode::KeyA) }
    }
}

/// Where the container screen is drawn, and what it says.
#[derive(Resource, Debug, Clone)]
pub struct ContainerLayout {
    /// The terminal cells it occupies.
    pub rect: Rect,
    /// Shown in place of rows when there are none.
    pub empty: String,
}

/// Which row the cursor is on. The screen's own state, since a view holds
/// no cursor.
#[derive(Resource, Debug, Default)]
pub struct ContainerMenu {
    /// The row picked out, clamped to the rows there are when drawn.
    pub selected: usize,
}

impl ContainerMenu {
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

/// Draws [`ContainerView`] as a list, and runs the screen.
///
/// Adds [`ContainerViewPlugin`] if the game has not, and declares the
/// `container` modal.
pub struct ContainerPanel(ContainerLayout);

impl ContainerPanel {
    /// The container screen in `rect`. Its title is whatever the container
    /// is called, so the engine never names a crate.
    pub fn new(rect: Rect) -> Self {
        Self(ContainerLayout { rect, empty: "Empty.".into() })
    }

    /// Sets what an emptied container says.
    pub fn empty(mut self, text: impl Into<String>) -> Self {
        self.0.empty = text.into();
        self
    }
}

impl Plugin for ContainerPanel {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<ContainerViewPlugin>() {
            app.add_plugins(ContainerViewPlugin);
        }
        app.add_modal(CONTAINER_MODAL);
        app.insert_resource(self.0.clone()).init_resource::<ContainerMenu>().init_resource::<ContainerKeys>();
        app.add_systems(Update, (open_on_interact, container_keys).chain().in_set(EngineSet::Input))
            .add_systems(Update, draw_container.in_set(PresentSet::Overlay));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::UiPlugin>(app, "ContainerPanel");
        rl_bevy::depends_on::<PropsPlugin>(app, "ContainerPanel");
        // Nothing on the controls screen: the screen opens itself when a
        // container is opened, so there is no root-level key to list, and
        // what it answers to is written along its own bottom border.
    }
}

/// The id of the container's modal, for a game gating its own systems.
///
/// # Panics
/// Panics if [`ContainerPanel`] was not added.
pub fn container_modal(modals: &Modals) -> ModalId {
    modals.get(CONTAINER_MODAL).expect("ContainerPanel declares the container modal")
}

/// Opens the screen when the engine says a container was opened.
///
/// The engine decides what `open` means and what it costs; this only
/// shows what is inside once it has happened.
pub fn open_on_interact(
    mut opened: MessageReader<Interacted>,
    mut open: ResMut<OpenContainer>,
    mut menu: ResMut<ContainerMenu>,
    mut modals: ResMut<Modals>,
    containers: Query<(), With<Container>>,
    player: Query<Entity, With<Player>>,
) {
    let modal = container_modal(&modals);
    for event in opened.read() {
        if event.verb != Verbs::OPEN || !containers.contains(event.prop) || !player.contains(event.actor) {
            continue;
        }
        open.0 = Some(event.prop);
        menu.selected = 0;
        if !modals.is_open(modal) {
            modals.open(modal);
        }
    }
}

/// What the container screen's keys act on.
#[derive(bevy::ecs::system::SystemParam)]
pub struct ChestKeys<'w> {
    binds: Res<'w, ContainerKeys>,
    view: Res<'w, ContainerView>,
    menu: ResMut<'w, ContainerMenu>,
    modals: ResMut<'w, Modals>,
    open: ResMut<'w, OpenContainer>,
}

/// Who is looking into it, and where everything stands.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Reaching<'w, 's> {
    holding: Query<'w, 's, Entity, (With<Player>, With<MyTurn>)>,
    reach: Query<'w, 's, &'static Position>,
}

/// Walks the rows, takes, and closes.
///
/// Taking is only written while the player holds the turn, since an
/// intent written for anyone else is dropped unread; the screen stays up
/// either way.
pub fn container_keys(keys: ControlInput, mut screen: ChestKeys, mut takes: MessageWriter<Intent<Take>>, who: Reaching) {
    let ChestKeys { binds, view, menu, modals, open } = &mut screen;
    let Reaching { holding, reach } = &who;
    let modal = container_modal(modals);
    if !modals.is_top(modal) {
        return;
    }
    // Whatever took the player away from it closes it: a screen onto a
    // crate on the other side of the room is a screen showing a lie.
    let out_of_reach = match (open.0.and_then(|p| reach.get(p).ok()), holding.single().ok().and_then(|p| reach.get(p).ok())) {
        (Some(prop), Some(player)) => prop.0 != player.0 && !rl_core::geometry::is_adjacent(prop.0, player.0),
        _ => false,
    };
    let bindings = keys.bindings();
    let input = keys.input();
    if out_of_reach || input.just_pressed(bindings.cursor.close) {
        modals.close_one(modal);
        open.0 = None;
        return;
    }
    match bindings.directions.just_pressed(input) {
        Some(Direction::North) => menu.move_by(-1, view.rows.len()),
        Some(Direction::South) => menu.move_by(1, view.rows.len()),
        _ => {}
    }
    let (Ok(player), Some(prop)) = (holding.single(), view.entity) else { return };
    if binds.take_all.just_pressed(input) {
        takes.write(Intent::new(player, Take { from: prop, item: None }));
    } else if input.just_pressed(bindings.cursor.confirm) || input.just_pressed(bindings.cursor.also_confirm) {
        let taking = view.row(menu.selected).map(|row| row.entity);
        if let Some(item) = taking {
            takes.write(Intent::new(player, Take { from: prop, item: Some(item) }));
        }
    }
}

/// Everything the container screen draws from.
#[derive(bevy::ecs::system::SystemParam)]
pub struct ChestScreen<'w> {
    layout: Res<'w, ContainerLayout>,
    view: Res<'w, ContainerView>,
    binds: Res<'w, ContainerKeys>,
    keys: ControlInput<'w>,
    modals: Res<'w, Modals>,
    palette: Res<'w, Palette>,
}

/// Draws the open container: what it is called, what is in it, and the
/// keys that do something.
pub fn draw_container(mut terminal: ResMut<Terminal>, mut menu: ResMut<ContainerMenu>, screen: ChestScreen) {
    let ChestScreen { layout, view, binds, keys, modals, palette } = &screen;
    if !modals.is_open(container_modal(modals)) {
        return;
    }
    let rect = layout.rect;
    if rect.width < 12 || rect.height < 5 {
        return;
    }
    let bindings = keys.bindings();
    menu.selected = menu.selected.min(view.rows.len().saturating_sub(1));
    let mut hints = Vec::new();
    if !view.rows.is_empty() {
        hints.push(format!("{} take", key_name(bindings.cursor.confirm)));
        hints.push(format!("{} take all", binds.take_all.label()));
    }
    hints.push(format!("{} close", key_name(bindings.cursor.close)));
    clear(&mut terminal, rect, palette);
    frame(&mut terminal, rect, &view.title, &hints.join(" \u{2022} "), palette);
    let inner = Rect::new(rect.x + 2, rect.y + 1, rect.width - 4, rect.height - 2);
    let surface = palette.get(Tones::SURFACE);
    let width = inner.width.max(0) as usize;

    if view.rows.is_empty() {
        terminal.print_on(inner.x, inner.y, &clip(&layout.empty, width), palette.get(Tones::MUTED), surface);
        return;
    }
    let rows = (inner.height.max(0) as usize).min(view.rows.len());
    let first = menu.selected.saturating_sub(rows.saturating_sub(1)).min(view.rows.len().saturating_sub(rows));
    for (i, row) in view.rows.iter().enumerate().skip(first).take(rows) {
        let y = inner.y + (i - first) as i32;
        let selected = i == menu.selected;
        let bg = if selected { palette.get(Tones::SELECT) } else { surface };
        let fg = palette.get(Tones::TEXT);
        terminal.fill(Rect::new(inner.x, y, inner.width, 1), Cell::new(' ', fg).on(bg));
        let mut x = inner.x;
        terminal.set(x, y, Cell::new(row.glyph.ch, row.glyph.fg).on(bg));
        x += 2;
        let room = (inner.right() - x).max(0) as usize;
        terminal.print_on(x, y, &clip(&row.label, room), fg, bg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use crate::view::container::ContainerView;
    use rl_bevy::props::Stocked;
    use rl_bevy::testing::TEST_SEED;

    /// A crate beside the player, holding two things, with the screen
    /// added and the engine's props running.
    fn stage() -> (Stage, Entity) {
        let mut stage = Stage::new_with(ContainerPanel::new(Rect::new(10, 5, 40, 12)), |app| {
            app.add_plugins(PropsPlugin);
            app.insert_resource(Seed(TEST_SEED));
        });
        let at = stage.at.offset(1, 0);
        let slugs = stage.app.world_mut().spawn((Item, Name::new("slug"), Stack { key: 1, count: 3 }, rl_render::Glyph::new(',', Color::WHITE))).id();
        let kit = stage.app.world_mut().spawn((Item, Name::new("medkit"), rl_render::Glyph::new('!', Color::WHITE))).id();
        let chest =
            stage.app.world_mut().spawn((Prop, Container, Name::new("supply crate"), Position(at), Inventory { items: vec![slugs, kit] }, Stocked)).id();
        stage.tick();
        (stage, chest)
    }

    /// Opening is the engine's to say; the screen only shows what is in
    /// there once it has happened.
    #[test]
    fn the_screen_opens_when_the_engine_says_a_container_was_opened() {
        let (mut stage, chest) = stage();
        let player = stage.player;
        assert!(!stage.app.world().resource::<Modals>().any_open(), "nothing is up before anything is opened");

        stage.app.world_mut().write_message(Interacted { actor: player, prop: chest, verb: Verbs::OPEN });
        stage.tick();
        let modals = stage.app.world().resource::<Modals>();
        assert!(modals.is_open(container_modal(modals)), "the screen is up");
        let view = stage.app.world().resource::<ContainerView>();
        assert_eq!(view.title, "supply crate", "titled by what the game called it");
        assert_eq!(view.rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), vec!["3 slugs", "medkit"]);
    }

    /// Taking everything empties the crate and leaves the screen up, so
    /// the player sees what they did and closes it themselves.
    #[test]
    fn taking_all_empties_it_and_the_screen_stays_up_until_it_is_closed() {
        let (mut stage, chest) = stage();
        let player = stage.player;
        stage.app.world_mut().write_message(Interacted { actor: player, prop: chest, verb: Verbs::OPEN });
        stage.tick();

        stage.press(KeyCode::KeyA);
        stage.tick();
        assert!(stage.app.world().get::<Inventory>(chest).expect("it still holds a bag").items.is_empty(), "the crate is empty");
        assert_eq!(stage.app.world().get::<Inventory>(player).expect("a bag").items.len(), 2, "and the player carries what was in it");
        let modals = stage.app.world().resource::<Modals>();
        assert!(modals.is_open(container_modal(modals)), "the screen is still up, showing an empty crate");
        assert!(stage.app.world().resource::<ContainerView>().rows.is_empty());
    }

    /// A screen onto a crate the player is no longer beside would be a
    /// screen showing a lie.
    #[test]
    fn walking_away_closes_the_screen() {
        let (mut stage, chest) = stage();
        let player = stage.player;
        stage.app.world_mut().write_message(Interacted { actor: player, prop: chest, verb: Verbs::OPEN });
        stage.tick();

        let far = stage.at.offset(-6, 0);
        stage.app.world_mut().entity_mut(player).insert(Position(far));
        stage.tick();
        stage.tick();
        let modals = stage.app.world().resource::<Modals>();
        assert!(!modals.is_open(container_modal(modals)), "it closed itself");
        assert!(stage.app.world().resource::<OpenContainer>().0.is_none(), "and forgot what it was showing");
    }
}
