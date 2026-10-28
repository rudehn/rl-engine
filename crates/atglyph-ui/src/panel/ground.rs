//! What lies underfoot, and how much of a pile to take or leave.
//!
//! Two screens on the modal stack. The ground screen opens from the
//! pick-up key when more than one plain thing lies underfoot: `All` first,
//! then each thing, lettered as the pack is. The count prompt opens over it
//! on a stack of more than one, and over the pack on a drop of one, asking
//! how many, starting at all of it.
//!
//! Taking or dropping closes the screens, since it spent the turn; backing
//! out spends nothing.

use atglyph_bevy::prelude::*;
use atglyph_bevy::{EngineSet, PresentSet};
use atglyph_core::GridRect;
use atglyph_render::{Cell, Terminal};
use bevy::prelude::*;

use crate::app::UiApp;
use crate::controls::{ControlInput, key_name, lettered_row, row_letter};
use crate::modal::{ModalId, Modals};
use crate::panel::{clear, clip, frame};
use crate::tone::{Palette, Tones};
use crate::view::ground::{GroundView, GroundViewPlugin};

/// The name the ground screen's modal is declared under.
pub(crate) const GROUND_MODAL: &str = "ground";
/// The name the count prompt's modal is declared under.
pub(crate) const COUNT_MODAL: &str = "count";

/// Where the ground screen is drawn, and what it is called.
#[derive(Resource, Debug, Clone)]
pub(crate) struct GroundLayout {
    rect: GridRect,
    title: String,
}

/// Which row the ground screen's cursor is on: 0 is `All`.
#[derive(Resource, Debug, Default)]
pub struct GroundMenu {
    /// The row picked out.
    pub selected: usize,
}

/// Draws [`GroundView`] as a list headed by `All`, and runs the screen.
///
/// Adds [`GroundViewPlugin`] and the count prompt if the game has not, and
/// declares the `ground` modal, which the pick-up key opens.
pub struct GroundPanel(GroundLayout);

impl GroundPanel {
    /// The ground screen in `rect`, titled `Here`.
    pub fn new(rect: GridRect) -> Self {
        Self(GroundLayout { rect, title: "Here".into() })
    }

    /// Sets what the screen is titled.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.0.title = title.into();
        self
    }
}

impl Plugin for GroundPanel {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<GroundViewPlugin>() {
            app.add_plugins(GroundViewPlugin);
        }
        add_count_prompt(app);
        app.add_modal(GROUND_MODAL);
        app.insert_resource(self.0.clone()).init_resource::<GroundMenu>();
        app.add_systems(Update, ground_keys.in_set(EngineSet::Input).before(count_keys))
            .add_systems(Update, draw_ground.in_set(PresentSet::Overlay).before(draw_count));
    }

    fn finish(&self, app: &mut App) {
        app.depends_on::<crate::UiPlugin>("GroundPanel");
        app.depends_on::<ItemsPlugin>("GroundPanel");
    }
}

/// The id of the ground screen's modal, if [`GroundPanel`] was added.
pub fn ground_modal(modals: &Modals) -> Option<ModalId> {
    modals.get(GROUND_MODAL)
}

/// What a count is asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountFor {
    /// Picking up part of a stack lying underfoot.
    PickUp,
    /// Dropping part of a carried stack.
    Drop,
}

/// The count prompt's state: what it asks about, the count so far, and the
/// screen it was opened over, which a confirm closes with it.
#[derive(Resource, Debug, Default)]
pub struct Counting {
    /// What is asked, while the prompt is up.
    pub asking: Option<(CountFor, Entity)>,
    /// What the thing is called, for the question.
    pub name: String,
    /// The count so far.
    pub count: u32,
    /// The most there is.
    pub most: u32,
    /// Whether a digit has been typed since it opened, so the first one
    /// replaces the count it opened at rather than adding to it.
    typed: bool,
    /// The screen it was opened over.
    over: Option<(ModalId, GridRect)>,
}

impl Counting {
    /// Asks how many of `item`, called `name`, of the `most` there are, for
    /// `what`, over the screen `over` drawn in `rect`; it opens at all of
    /// them.
    pub(crate) fn ask(&mut self, what: CountFor, item: Entity, name: &str, most: u32, over: ModalId, rect: GridRect) {
        *self = Counting { asking: Some((what, item)), name: name.to_string(), count: most, most, typed: false, over: Some((over, rect)) };
    }

    /// Types `digit` onto the count, clamped to the most there is.
    fn type_digit(&mut self, digit: u32) {
        let from = if self.typed { self.count } else { 0 };
        self.count = (from.saturating_mul(10).saturating_add(digit)).min(self.most);
        self.typed = true;
    }
}

/// The count prompt's keys, beside the digits it types and the cursors'
/// keys that confirm and back out.
///
/// A default, not a rule: a game that wants others inserts its own.
#[derive(Resource, Debug, Clone)]
pub struct CountKeys {
    /// One fewer, or ten with Shift.
    pub less: KeyCode,
    /// One more, or ten with Shift.
    pub more: KeyCode,
    /// Just one.
    pub fewest: KeyCode,
    /// All of them.
    pub most: KeyCode,
}

impl Default for CountKeys {
    fn default() -> Self {
        Self { less: KeyCode::ArrowLeft, more: KeyCode::ArrowRight, fewest: KeyCode::Home, most: KeyCode::End }
    }
}

/// Adds the count prompt, once, for whichever screen asks first.
pub(crate) fn add_count_prompt(app: &mut App) {
    if app.world().contains_resource::<Counting>() {
        return;
    }
    app.add_modal(COUNT_MODAL);
    app.init_resource::<Counting>().init_resource::<CountKeys>();
    app.add_message::<Intent<PickUpItem>>().add_message::<Intent<DropItem>>();
    app.add_systems(Update, count_keys.in_set(EngineSet::Input)).add_systems(Update, draw_count.in_set(PresentSet::Overlay));
}

/// The id of the count prompt's modal.
fn count_modal(modals: &Modals) -> ModalId {
    modals.get(COUNT_MODAL).expect("the count prompt declares its modal")
}

/// What the ground screen's keys act on.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct GroundKeys<'w, 's> {
    layout: Res<'w, GroundLayout>,
    view: Res<'w, GroundView>,
    menu: ResMut<'w, GroundMenu>,
    modals: ResMut<'w, Modals>,
    counting: ResMut<'w, Counting>,
    holding: Query<'w, 's, Entity, (With<Player>, With<MyTurn>)>,
    all: MessageWriter<'w, Intent<PickUp>>,
    one: MessageWriter<'w, Intent<PickUpItem>>,
}

/// Walks the rows and takes one, all, or asks how many of a stack.
pub(crate) fn ground_keys(keys: ControlInput, screen: GroundKeys) {
    let GroundKeys { layout, view, mut menu, mut modals, mut counting, holding, mut all, mut one } = screen;
    // Not in the frame it went up, since the key that opened it is a letter
    // this screen reads as a row.
    let Some(modal) = ground_modal(&modals).filter(|m| modals.is_top(*m) && !modals.just_opened()) else { return };
    let (bindings, input) = (keys.bindings(), keys.input());
    // A pile gone from underfoot, picked up or walked away from, leaves the
    // screen nothing to show.
    if input.just_pressed(bindings.cursor.close) || view.rows.is_empty() {
        modals.close_one(modal);
        return;
    }
    let rows = view.rows.len() + 1;
    let confirmed = input.just_pressed(bindings.cursor.confirm) || input.just_pressed(bindings.cursor.also_confirm);
    let Some(picked) = lettered_row(input).filter(|row| *row < rows).or(confirmed.then_some(menu.selected.min(rows - 1))) else {
        menu.selected = crate::panel::inventory::stepped(menu.selected, crate::panel::inventory::walked(&keys), rows);
        return;
    };
    let Ok(player) = holding.single() else { return };
    match picked.checked_sub(1).map(|i| &view.rows[i]) {
        None => drop(all.write(Intent::new(player, PickUp))),
        Some(row) if row.count > 1 => {
            counting.ask(CountFor::PickUp, row.row.entity, &row.name, row.count, modal, layout.rect);
            let prompt = count_modal(&modals);
            modals.open(prompt);
            return;
        }
        Some(row) => drop(one.write(Intent::new(player, PickUpItem { item: row.row.entity, count: None }))),
    }
    modals.close_one(modal);
}

/// What the count prompt's keys act on.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct CountScreen<'w, 's> {
    binds: Res<'w, CountKeys>,
    counting: ResMut<'w, Counting>,
    modals: ResMut<'w, Modals>,
    holding: Query<'w, 's, Entity, (With<Player>, With<MyTurn>)>,
    pick_ups: MessageWriter<'w, Intent<PickUpItem>>,
    drops: MessageWriter<'w, Intent<DropItem>>,
}

/// The digits, as the count prompt types them.
const DIGITS: [KeyCode; 10] = [
    KeyCode::Digit0,
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

/// Steps, types and clamps the count, and confirms it or backs out.
///
/// Runs after the screens that open it, which do nothing while it is on
/// top, so the key that closes it is not read again by the screen under
/// it; not in the frame it went up, so the press that opened it is not
/// read as its answer.
pub(crate) fn count_keys(keys: ControlInput, screen: CountScreen) {
    let CountScreen { binds, mut counting, mut modals, holding, mut pick_ups, mut drops } = screen;
    let modal = count_modal(&modals);
    if !modals.is_top(modal) || modals.just_opened() {
        return;
    }
    let (bindings, input) = (keys.bindings(), keys.input());
    if input.just_pressed(bindings.cursor.close) {
        counting.asking = None;
        modals.close_one(modal);
        return;
    }
    let step = if input.pressed(KeyCode::ShiftLeft) || input.pressed(KeyCode::ShiftRight) { 10 } else { 1 };
    let most = counting.most;
    if input.just_pressed(binds.less) {
        counting.count = counting.count.saturating_sub(step).max(1);
    } else if input.just_pressed(binds.more) {
        counting.count = (counting.count + step).min(most);
    } else if input.just_pressed(binds.fewest) {
        counting.count = 1;
    } else if input.just_pressed(binds.most) {
        counting.count = most;
    } else if input.just_pressed(KeyCode::Backspace) {
        counting.count /= 10;
        counting.typed = true;
    } else if let Some(digit) = DIGITS.iter().position(|key| input.just_pressed(*key)) {
        counting.type_digit(digit as u32);
    }
    let confirmed = input.just_pressed(bindings.cursor.confirm) || input.just_pressed(bindings.cursor.also_confirm);
    // A count of none is no answer: the prompt stays up until it is one.
    let (Some((what, item)), true, Ok(player)) = (counting.asking, confirmed && counting.count > 0, holding.single()) else { return };
    let count = Some(counting.count);
    match what {
        CountFor::PickUp => drop(pick_ups.write(Intent::new(player, PickUpItem { item, count }))),
        CountFor::Drop => drop(drops.write(Intent::new(player, DropItem { item, count }))),
    }
    counting.asking = None;
    modals.close_one(modal);
    if let Some((over, _)) = counting.over.take() {
        modals.close_one(over);
    }
}

/// Everything the ground screen draws from.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct GroundDrawn<'w> {
    layout: Res<'w, GroundLayout>,
    view: Res<'w, GroundView>,
    menu: Res<'w, GroundMenu>,
    keys: ControlInput<'w>,
    modals: Res<'w, Modals>,
    palette: Res<'w, Palette>,
}

/// Draws the ground screen: `All`, then each thing lettered after it.
pub(crate) fn draw_ground(mut terminal: ResMut<Terminal>, screen: GroundDrawn) {
    let GroundDrawn { layout, view, menu, keys, modals, palette } = &screen;
    if !ground_modal(modals).is_some_and(|m| modals.is_open(m)) {
        return;
    }
    let rect = layout.rect;
    if rect.width < 12 || rect.height < 5 {
        return;
    }
    let bindings = keys.bindings();
    let rows = view.rows.len() + 1;
    let last = row_letter(rows - 1).map_or(String::new(), |c| format!("-{c}"));
    let hints = format!("a{last} take \u{2022} {} close", key_name(bindings.cursor.close));
    clear(&mut terminal, rect, palette);
    frame(&mut terminal, rect, &layout.title, &hints, palette);
    let inner = GridRect::new(rect.x + 2, rect.y + 1, rect.width - 4, rect.height - 2);
    let surface = palette.get(Tones::SURFACE);
    let shown = (inner.height.max(0) as usize).min(rows);
    let selected = menu.selected.min(rows - 1);
    let first = selected.saturating_sub(shown.saturating_sub(1)).min(rows - shown);
    for i in first..first + shown {
        let y = inner.y + (i - first) as i32;
        let bg = if i == selected { palette.get(Tones::SELECT) } else { surface };
        let fg = palette.get(Tones::TEXT);
        terminal.fill(GridRect::new(inner.x, y, inner.width, 1), Cell::new(' ', fg).on(bg));
        let letter = row_letter(i).unwrap_or(' ');
        terminal.print_on(inner.x, y, &format!("{letter})"), palette.get(Tones::MUTED), bg);
        let mut x = inner.x + 3;
        let label = match i.checked_sub(1).map(|r| &view.rows[r].row) {
            None => "All".to_string(),
            Some(row) => {
                terminal.set(x, y, Cell::new(row.glyph.ch, row.glyph.fg).on(bg));
                x += 2;
                row.label.clone()
            }
        };
        let room = (inner.right() - x).max(0) as usize;
        terminal.print_on(x, y, &clip(&label, room), fg, bg);
    }
}

/// Draws the count prompt across the middle of the screen it was opened
/// over.
pub(crate) fn draw_count(
    mut terminal: ResMut<Terminal>,
    counting: Res<Counting>,
    binds: Res<CountKeys>,
    keys: ControlInput,
    modals: Res<Modals>,
    palette: Res<Palette>,
) {
    if !modals.is_open(count_modal(&modals)) {
        return;
    }
    let (Some((what, _)), Some((_, over))) = (counting.asking, counting.over) else { return };
    let rect = GridRect::new(over.x, over.y + over.height / 2 - 2, over.width, 4);
    let verb = match what {
        CountFor::PickUp => "Pick up",
        CountFor::Drop => "Drop",
    };
    let cursor = &keys.bindings().cursor;
    let hints = format!(
        "{}/{} fewer or more \u{2022} {} ok \u{2022} {} back",
        key_name(binds.less),
        key_name(binds.more),
        key_name(cursor.confirm),
        key_name(cursor.close)
    );
    clear(&mut terminal, rect, &palette);
    frame(&mut terminal, rect, "", &hints, &palette);
    let width = (rect.width - 4).max(0) as usize;
    let question = format!("{verb} how many {}?", atglyph_core::noun::plural(&counting.name));
    let surface = palette.get(Tones::SURFACE);
    terminal.print_on(rect.x + 2, rect.y + 1, &clip(&question, width), palette.get(Tones::TEXT), surface);
    terminal.print_on(rect.x + 2, rect.y + 2, &clip(&format!("[{}] of {}", counting.count, counting.most), width), palette.get(Tones::TITLE), surface);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;

    /// The ground screen and the pick-up key, a pile of 20 pellets and a
    /// medkit under the player.
    fn stage() -> (Stage, Entity, Entity) {
        let mut stage = Stage::new((
            GroundPanel::new(GridRect::new(10, 5, 40, 12)),
            crate::PlayerKeysPlugin::new().pick_up("Act", "pick up", crate::Otherwise::say("Nothing here.")),
        ));
        let at = stage.at;
        let pile = stage.app.world_mut().spawn((Item, Name::new("pellet"), Stack { key: 1, count: 20 }, Position(at))).id();
        let kit = stage.app.world_mut().spawn((Item, Name::new("medkit"), Position(at))).id();
        stage.app.world_mut().entity_mut(stage.player).insert(Inventory::default());
        stage.tick();
        (stage, pile, kit)
    }

    fn up(stage: &Stage, name: &str) -> bool {
        let modals = stage.app.world().resource::<Modals>();
        modals.get(name).is_some_and(|m| modals.is_open(m))
    }

    fn carried(stage: &Stage) -> Vec<Entity> {
        stage.app.world().get::<Inventory>(stage.player).unwrap().items.clone()
    }

    #[test]
    fn g_on_two_things_opens_the_ground_screen_and_a_takes_all() {
        let (mut stage, _, _) = stage();
        stage.press(KeyCode::KeyG);
        assert!(up(&stage, GROUND_MODAL), "two things: the screen asks which");
        stage.press(KeyCode::KeyA);
        assert_eq!(carried(&stage).len(), 2, "all of it");
        assert!(!up(&stage, GROUND_MODAL), "and the screen closed with the turn spent");
    }

    #[test]
    fn a_letter_takes_one_plain_thing() {
        let (mut stage, _, kit) = stage();
        stage.press(KeyCode::KeyG);
        stage.press(KeyCode::KeyC);
        assert_eq!(carried(&stage), vec![kit]);
    }

    #[test]
    fn g_on_a_stack_asks_how_many_and_takes_that_many() {
        let (mut stage, pile, _) = stage();
        stage.press(KeyCode::KeyG);
        stage.press(KeyCode::KeyB);
        assert!(up(&stage, COUNT_MODAL), "a stack's row asks how many");
        assert_eq!(stage.app.world().resource::<Counting>().count, 20, "starting at all of it");
        stage.press(KeyCode::Digit7);
        stage.press(KeyCode::Enter);
        let held = carried(&stage);
        assert_eq!(held.len(), 1);
        assert_eq!(stage.app.world().get::<Stack>(held[0]).unwrap().count, 7);
        assert_eq!(stage.app.world().get::<Stack>(pile).unwrap().count, 13, "the rest lies there");
        assert!(!up(&stage, COUNT_MODAL) && !up(&stage, GROUND_MODAL), "both screens closed");
    }

    #[test]
    fn the_count_prompt_steps_types_and_clamps() {
        let (mut stage, _, _) = stage();
        stage.press(KeyCode::KeyG);
        stage.press(KeyCode::KeyB);
        let count = |stage: &Stage| stage.app.world().resource::<Counting>().count;
        stage.press(KeyCode::ArrowRight);
        assert_eq!(count(&stage), 20, "no more than there are");
        stage.press(KeyCode::ArrowLeft);
        assert_eq!(count(&stage), 19);
        stage.press(KeyCode::Home);
        assert_eq!(count(&stage), 1);
        stage.press(KeyCode::End);
        assert_eq!(count(&stage), 20);
        stage.press(KeyCode::Digit9);
        stage.press(KeyCode::Digit9);
        assert_eq!(count(&stage), 20, "typed past the most is the most");
        stage.press(KeyCode::Backspace);
        stage.press(KeyCode::Backspace);
        stage.press(KeyCode::Digit0);
        assert_eq!(count(&stage), 0);
        stage.press(KeyCode::Enter);
        assert!(up(&stage, COUNT_MODAL), "none is no answer");
        assert!(carried(&stage).is_empty());
    }

    #[test]
    fn escape_from_the_count_prompt_goes_back_and_spends_nothing() {
        let (mut stage, _, _) = stage();
        let before = stage.app.world().resource::<atglyph_bevy::Turns>().now();
        stage.press(KeyCode::KeyG);
        stage.press(KeyCode::KeyB);
        stage.press(KeyCode::Escape);
        assert!(!up(&stage, COUNT_MODAL) && up(&stage, GROUND_MODAL), "back on the ground screen");
        assert_eq!(stage.app.world().resource::<atglyph_bevy::Turns>().now(), before);
    }

    #[test]
    fn g_on_one_plain_thing_picks_it_up_with_no_screen() {
        let (mut stage, pile, kit) = stage();
        stage.app.world_mut().despawn(pile);
        stage.tick();
        stage.press(KeyCode::KeyG);
        assert!(!up(&stage, GROUND_MODAL));
        assert_eq!(carried(&stage), vec![kit]);
    }

    #[test]
    fn nothing_here_says_so_and_spends_nothing() {
        let (mut stage, pile, kit) = stage();
        stage.app.world_mut().despawn(pile);
        stage.app.world_mut().despawn(kit);
        stage.tick();
        let before = stage.app.world().resource::<atglyph_bevy::Turns>().now();
        stage.press(KeyCode::KeyG);
        let said = stage.app.world().resource::<crate::MessageLog>().iter().last().map(|e| e.text.clone());
        assert_eq!(said.as_deref(), Some("Nothing here."));
        assert_eq!(stage.app.world().resource::<atglyph_bevy::Turns>().now(), before);
    }
}
