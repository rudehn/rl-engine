//! The screen that asks which of several things you meant.
//!
//! One offer in reach needs no screen: the interact key takes it, and
//! walking into a prop that offers one thing takes that. Two is a
//! question, and this is where it is asked. The direction keys walk the
//! rows, the cursors' confirm key takes the one picked out, and the
//! cursors' close key puts it away.
//!
//! It opens itself two ways, both of them a player asking "what can I do
//! here": the interact key with more than one offer in reach, and a bump
//! into a prop that offers more than one thing, which the engine refuses
//! rather than guessing at and reports as [`Bumped`].
//!
//! Rows that cannot be taken up are still listed, greyed, with the reason
//! in words, the way the ability menu lists what it cannot use: a player
//! who cannot open the cache wants to know it is a cache and that it wants
//! a cutter.

use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_bevy::{Bumped, EngineSet, PresentSet};
use rl_core::{Direction, Rect};
use rl_render::{Cell, Terminal};

use crate::controls::{ControlInput, key_name};
use crate::modal::{AddModal, ModalId, Modals};
use crate::panel::{clear, clip, frame};
use crate::tone::{Palette, Tones};
use crate::view::offers::{OffersView, OffersViewPlugin};

/// The name the offers modal is declared under.
pub const OFFERS_MODAL: &str = "offers";

/// Where the screen is drawn, and what it says.
#[derive(Resource, Debug, Clone)]
pub struct OffersLayout {
    /// The terminal cells it occupies.
    pub rect: Rect,
    /// Drawn in the top border.
    pub title: String,
}

/// Which row the cursor is on. The screen's own state, since a view holds
/// no cursor.
#[derive(Resource, Debug, Default)]
pub struct OffersMenu {
    /// The row picked out, clamped to the rows there are when drawn.
    pub selected: usize,
}

impl OffersMenu {
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

/// Draws [`OffersView`] as a list, and runs the screen.
///
/// Adds [`OffersViewPlugin`] if the game has not, and declares the
/// `offers` modal.
pub struct OffersPanel(OffersLayout);

impl OffersPanel {
    /// The screen in `rect`.
    pub fn new(rect: Rect) -> Self {
        Self(OffersLayout { rect, title: "Here".into() })
    }

    /// Replaces what the top border says.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.0.title = title.into();
        self
    }
}

impl Plugin for OffersPanel {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<OffersViewPlugin>() {
            app.add_plugins(OffersViewPlugin);
        }
        app.add_modal(OFFERS_MODAL);
        app.insert_resource(self.0.clone()).init_resource::<OffersMenu>();
        app.add_systems(Update, offers_keys.in_set(EngineSet::Input))
            // After the pass, not with the keys: a bump is refused inside
            // the turn, which runs after input, so a reader in `Input` would
            // see it a frame late. `Narrate` is where the rest of what a
            // pass reported is read.
            .add_systems(Update, open_on_crowded_bump.in_set(PresentSet::Narrate))
            .add_systems(Update, draw_offers.in_set(PresentSet::Overlay));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::UiPlugin>(app, "OffersPanel");
        rl_bevy::depends_on::<PropsPlugin>(app, "OffersPanel");
    }
}

/// The id of the offers modal, for a game gating its own systems.
///
/// # Panics
/// Panics if [`OffersPanel`] was not added.
pub fn offers_modal(modals: &Modals) -> ModalId {
    modals.get(OFFERS_MODAL).expect("OffersPanel declares the offers modal")
}

/// Opens the screen when a bump came to nothing because the prop walked
/// into offers more than one thing.
///
/// The engine refuses that bump rather than guessing; this is the question
/// it could not ask from inside the turn.
pub fn open_on_crowded_bump(
    mut bumped: MessageReader<Bumped>,
    view: Res<OffersView>,
    mut menu: ResMut<OffersMenu>,
    mut modals: ResMut<Modals>,
    props: Query<(), With<Prop>>,
) {
    let modal = offers_modal(&modals);
    for ev in bumped.read() {
        if !props.contains(ev.into) {
            continue;
        }
        if view.rows.iter().filter(|r| r.prop == ev.into && r.open()).count() < 2 {
            continue;
        }
        menu.selected = 0;
        if !modals.any_open() {
            modals.open(modal);
        }
    }
}

/// Opens the screen, walks it, takes the row picked out, and closes it.
pub fn offers_keys(
    keys: ControlInput,
    view: Res<OffersView>,
    mut menu: ResMut<OffersMenu>,
    mut modals: ResMut<Modals>,
    mut intents: MessageWriter<Intent<Interact>>,
    holding: Query<Entity, (With<Player>, With<MyTurn>)>,
) {
    let modal = offers_modal(&modals);
    if !modals.is_top(modal) {
        return;
    }
    let bindings = keys.bindings();
    let input = keys.input();
    // Nothing to choose between any more: whatever moved the player away
    // took the question with it.
    if input.just_pressed(bindings.cursor.close) || view.rows.is_empty() {
        modals.close_one(modal);
        return;
    }
    match bindings.directions.just_pressed(input) {
        Some(Direction::North) => menu.move_by(-1, view.rows.len()),
        Some(Direction::South) => menu.move_by(1, view.rows.len()),
        _ => {}
    }
    if !input.just_pressed(bindings.cursor.confirm) && !input.just_pressed(bindings.cursor.also_confirm) {
        return;
    }
    let (Ok(player), Some(row)) = (holding.single(), view.row(menu.selected)) else { return };
    // A refused row stays on screen and says why: taking it up would
    // spend nothing and teach nothing.
    if row.open() {
        intents.write(Intent::new(player, Interact { prop: row.prop, verb: row.verb }));
        modals.close_one(modal);
    }
}

/// What the offers screen draws from.
#[derive(bevy::ecs::system::SystemParam)]
pub struct HereScreen<'w> {
    layout: Res<'w, OffersLayout>,
    view: Res<'w, OffersView>,
    keys: ControlInput<'w>,
    modals: Res<'w, Modals>,
    palette: Res<'w, Palette>,
}

/// What a turn's worth of time reads as: `2 turns`, or hundredths when it
/// is not a whole number of them.
fn cost(time: u32) -> String {
    match (time / 100, time % 100) {
        (1, 0) => "1 turn".to_string(),
        (turns, 0) => format!("{turns} turns"),
        (turns, rest) => format!("{turns}.{rest:02} turns"),
    }
}

/// Draws what is on offer here, the row picked out, and why a row cannot
/// be taken up.
pub fn draw_offers(mut terminal: ResMut<Terminal>, mut menu: ResMut<OffersMenu>, screen: HereScreen) {
    let HereScreen { layout, view, keys, modals, palette } = &screen;
    if !modals.is_open(offers_modal(modals)) {
        return;
    }
    let rect = layout.rect;
    if rect.width < 12 || rect.height < 4 {
        return;
    }
    let bindings = keys.bindings();
    menu.selected = menu.selected.min(view.rows.len().saturating_sub(1));
    let hints = format!("{} do it \u{2022} {} never mind", key_name(bindings.cursor.confirm), key_name(bindings.cursor.close));
    clear(&mut terminal, rect, palette);
    frame(&mut terminal, rect, &layout.title, &hints, palette);
    let inner = Rect::new(rect.x + 2, rect.y + 1, rect.width - 4, rect.height - 2);
    let surface = palette.get(Tones::SURFACE);
    let rows = (inner.height.max(0) as usize).min(view.rows.len());
    let first = menu.selected.saturating_sub(rows.saturating_sub(1)).min(view.rows.len().saturating_sub(rows));
    for (i, row) in view.rows.iter().enumerate().skip(first).take(rows) {
        let y = inner.y + (i - first) as i32;
        let selected = i == menu.selected;
        let bg = if selected { palette.get(Tones::SELECT) } else { surface };
        let fg = palette.get(if row.open() { Tones::TEXT } else { Tones::MUTED });
        terminal.fill(Rect::new(inner.x, y, inner.width, 1), Cell::new(' ', fg).on(bg));
        let said = match &row.refused {
            Some(why) => format!("{} \u{2014} {why}", row.label),
            None => format!("{} ({})", row.label, cost(row.time)),
        };
        terminal.print_on(inner.x, y, &clip(&said, inner.width.max(0) as usize), fg, bg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use rl_bevy::props::Stocked;
    use rl_bevy::testing::TEST_SEED;
    use rl_core::Point;

    /// A bench beside the player offering two things, and a crate beside
    /// it offering one.
    fn stage() -> (Stage, Entity) {
        let mut stage = Stage::new_with(OffersPanel::new(Rect::new(8, 4, 40, 8)), |app| {
            app.add_plugins(PropsPlugin);
            app.insert_resource(Seed(TEST_SEED));
            app.add_verb("strip");
            app.add_verb("tip");
            let props = rl_rules::prop::load(
                r#"#![enable(implicit_some)]
                [
                    (name: "workbench", glyph: 'T', color: (r: 1, g: 2, b: 3), blocks: true,
                     offers: [(verb: "strip", time: 100), (verb: "tip", time: 200)]),
                ]"#,
                &rl_rules::Names::new(),
            )
            .expect("the props load");
            app.world_mut().resource_mut::<Registries>().props = props;
        });
        let registries = stage.app.world().resource::<Registries>().clone();
        let bench = {
            let id = registries.props.expect("workbench");
            let at = stage.at.offset(1, 0);
            let mut commands = stage.app.world_mut().commands();
            spawn_prop(&mut commands, &registries, id, at, MapId::SURFACE)
        };
        stage.app.world_mut().flush();
        stage.app.world_mut().entity_mut(bench).insert(Stocked);
        stage.tick();
        stage.tick();
        (stage, bench)
    }

    /// What the engine refused to guess at, the screen asks.
    #[test]
    fn walking_into_a_prop_that_offers_two_things_asks_which() {
        let (mut stage, _bench) = stage();
        let modals = stage.app.world().resource::<Modals>();
        assert!(!modals.any_open(), "nothing is up until the player walks into it");

        let player = stage.player;
        stage.app.world_mut().write_message(Intent::new(player, Bump(Direction::East)));
        stage.tick();
        let modals = stage.app.world().resource::<Modals>();
        assert!(modals.is_open(offers_modal(modals)), "the question is asked");
        let view = stage.app.world().resource::<OffersView>();
        assert_eq!(view.rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), vec!["strip workbench", "tip workbench"]);
    }

    /// Down and Enter take the second of them, and nothing is guessed at.
    #[test]
    fn the_row_picked_out_is_what_is_done() {
        let (mut stage, bench) = stage();
        let player = stage.player;
        #[derive(Resource, Default)]
        struct Seen(Vec<Interacted>);
        stage.app.init_resource::<Seen>().add_systems(Turn, |mut seen: ResMut<Seen>, mut done: MessageReader<Interacted>| {
            seen.0.extend(done.read().copied());
        });

        stage.app.world_mut().write_message(Intent::new(player, Bump(Direction::East)));
        stage.tick();
        stage.press(KeyCode::ArrowDown);
        stage.tick();
        stage.press(KeyCode::Enter);
        stage.tick();

        let tip = stage.app.world().resource::<Verbs>().get("tip").expect("the game's verb");
        assert_eq!(stage.app.world().resource::<Seen>().0, vec![Interacted { actor: player, prop: bench, verb: tip }], "the second row, not the first");
        let modals = stage.app.world().resource::<Modals>();
        assert!(!modals.is_open(offers_modal(modals)), "and the question is answered, so the screen is gone");
    }

    /// Whatever took the player away from it takes the question with it.
    #[test]
    fn walking_away_closes_the_question() {
        let (mut stage, _bench) = stage();
        let player = stage.player;
        stage.app.world_mut().write_message(Intent::new(player, Bump(Direction::East)));
        stage.tick();
        stage.app.world_mut().entity_mut(player).insert(Position(stage.at.offset(-6, 0)));
        stage.tick();
        stage.tick();
        let modals = stage.app.world().resource::<Modals>();
        assert!(!modals.is_open(offers_modal(modals)), "nothing to choose between any more");
        let _ = Point::ZERO;
    }

    /// A cost reads as turns, since that is what a player spends.
    #[test]
    fn a_cost_reads_as_turns() {
        assert_eq!(cost(100), "1 turn");
        assert_eq!(cost(300), "3 turns");
        assert_eq!(cost(150), "1.50 turns");
    }
}
