//! The screen that asks which of several things you meant.
//!
//! One offer in reach needs no screen: the interact key takes it, and
//! walking into a prop or a person that offers one thing takes that. Two is a
//! question, and this is where it is asked. The direction keys walk the
//! rows, the cursors' confirm key takes the one picked out, and the
//! cursors' close key puts it away.
//!
//! It opens itself two ways, both of them a player asking "what can I do
//! here": the interact key with more than one offer in reach, and a bump
//! into something that offers more than one thing, a prop or someone who
//! is neither foe nor ally, which the engine refuses rather than guessing
//! at and reports as a [`Bumped`] with `offers` set.
//!
//! Rows that cannot be taken up are still listed, greyed, with the reason
//! in words, the way the ability menu lists what it cannot use: a player
//! who cannot open the cache wants to know it is a cache and that it wants
//! a cutter. A row with a price shows it beside the time, and one the
//! player cannot pay says how much it wants.
//!
//! An offer that asks first, [`Offering::asking`], is asked about when it
//! is taken up here: the screen shows the game's question with two
//! answers, No picked out, the way the title asks before giving up a save.
//! Yes is the offer taken up, the same [`Interact`] as any row, so the
//! engine checks its reach and charges its time; No, or putting the screen
//! away, is nothing and costs nothing. The question lasts only while its
//! offer is still on offer: whatever took it out of reach takes the
//! question with it, unanswered.
//!
//! An offer that chooses first, [`Offering::choosing`], is chosen for when
//! it is taken up here: the screen is put away and the bag opens in its
//! place, under the offer's prompt among what the game's system names, as
//! [`InventoryPanel`](crate::InventoryPanel) says. A pick is the offer
//! taken up with the thing picked out, the same [`Interact`]; backing out
//! is nothing and costs nothing.
//!
//! The screen sits over the map, so it moves off what it is about, as the
//! look panel moves off its cursor: when the one choosing or anyone or
//! anything offering a row stands under its rectangle, it is drawn in its
//! other place, the game's [`OffersPanel::else_at`] or its rectangle
//! mirrored top for bottom in the map. A screen that covered the person
//! it asks about asks the player to choose blind.

use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_bevy::{Bumped, EngineSet, PresentSet};
use rl_core::{Direction, Rect};
use rl_render::{Cell, MapView, Terminal};

use crate::controls::{ControlInput, key_name};
use crate::modal::{AddModal, ModalId, Modals};
use crate::panel::{clear, clip, frame, hides, other_place, wrap};
use crate::tone::{Palette, Tones};
use crate::view::offers::{OffersView, OffersViewPlugin};

/// The name the offers modal is declared under.
pub const OFFERS_MODAL: &str = "offers";

/// Where the screen is drawn, and what it says.
#[derive(Resource, Debug, Clone)]
pub struct OffersLayout {
    /// The terminal cells it occupies.
    pub rect: Rect,
    /// Where it goes while the one choosing, or what offers a row, stands
    /// under `rect`. `None` mirrors `rect` top for bottom within the map,
    /// which suits a screen along either edge; a game names a place when
    /// its screen is not there.
    pub elsewhere: Option<Rect>,
    /// Drawn in the top border.
    pub title: String,
    /// The answer to an asking offer's question that takes it up.
    pub yes: String,
    /// The answer that takes nothing up, picked out when the question is
    /// asked, so a press too many backs out rather than goes ahead.
    pub no: String,
}

/// The screen asking an offer's question: which offer, and the answer
/// picked out. The question's words are read off the offer each frame, so
/// the screen never asks about an offer no longer there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Confirming {
    /// Who or what offers it.
    pub target: Entity,
    /// Which of its offers.
    pub verb: VerbId,
    /// Whether Yes is picked out rather than No.
    pub yes: bool,
}

/// Which row the cursor is on, and the question while one is asked. The
/// screen's own state, since a view holds no cursor.
#[derive(Resource, Debug, Default)]
pub struct OffersMenu {
    /// The row picked out, clamped to the rows there are when drawn.
    pub selected: usize,
    /// An asking offer's question, while the screen asks it in place of
    /// the rows.
    pub confirming: Option<Confirming>,
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
        Self(OffersLayout { rect, elsewhere: None, title: "Here".into(), yes: "Yes".into(), no: "No".into() })
    }

    /// Sets where the screen goes while what it is about stands under its
    /// rectangle, in place of the mirror of it.
    pub fn else_at(mut self, rect: Rect) -> Self {
        self.0.elsewhere = Some(rect);
        self
    }

    /// Replaces the two answers to an asking offer's question.
    pub fn answers(mut self, yes: impl Into<String>, no: impl Into<String>) -> Self {
        self.0.yes = yes.into();
        self.0.no = no.into();
        self
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

/// Opens the screen when a bump came to nothing against something that
/// offers, a prop or a person.
///
/// Two cases, and the screen answers both: it offers more than one thing,
/// which is a question the walk key could not ask, or it offers one it
/// refuses, which is an explanation the walk key could not give. A lone
/// offer with a price is asked about too, since the walk key never pays.
/// One that offered one thing and did it is not here at all, because the bump
/// became the interaction, and nor is an ally offering something, which
/// the engine bumps as an ally and reports without `offers`.
pub fn open_on_crowded_bump(mut bumped: MessageReader<Bumped>, view: Res<OffersView>, mut menu: ResMut<OffersMenu>, mut modals: ResMut<Modals>) {
    let modal = offers_modal(&modals);
    for ev in bumped.read() {
        if !ev.offers {
            continue;
        }
        let rows = view.rows.iter().filter(|r| r.target == ev.into).count();
        let free = view.rows.iter().filter(|r| r.target == ev.into && r.open() && r.price.is_none() && r.asks.is_none()).count();
        // Nothing to say, or the bump already did the one free thing on
        // offer; a priced one, or one that asks first, it left for this
        // screen.
        if rows == 0 || (rows == 1 && free == 1) {
            continue;
        }
        menu.selected = 0;
        if !modals.any_open() {
            modals.open(modal);
        }
    }
}

/// Opens the screen, walks it, takes the row picked out, and closes it;
/// or, for a row that asks first, asks, and takes it up only on a yes; or,
/// for one that chooses first, puts itself away and opens the bag
/// choosing.
pub fn offers_keys(
    mut commands: Commands,
    keys: ControlInput,
    view: Res<OffersView>,
    mut menu: ResMut<OffersMenu>,
    mut modals: ResMut<Modals>,
    mut intents: MessageWriter<Intent<Interact>>,
    holding: Query<Entity, (With<Player>, With<MyTurn>)>,
) {
    let modal = offers_modal(&modals);
    // A question lasts only while the screen is up, however it was put
    // away, and the cursor starts on the first row however it is opened
    // again: the interact key opens it with no word to this screen, and a
    // cursor left on the row taken last time reads as a choice made.
    if !modals.is_open(modal) {
        menu.confirming = None;
        menu.selected = 0;
    }
    if !modals.is_top(modal) {
        return;
    }
    let bindings = keys.bindings();
    let input = keys.input();
    let take = input.just_pressed(bindings.cursor.confirm) || input.just_pressed(bindings.cursor.also_confirm);
    if let Some(confirming) = menu.confirming.as_mut() {
        // Whatever took the offer out of reach, or took it off whoever
        // offered it, takes the question with it, unanswered.
        let Some(row) = view.rows.iter().find(|r| r.target == confirming.target && r.verb == confirming.verb && question(r).is_some()) else {
            menu.confirming = None;
            modals.close_one(modal);
            return;
        };
        if modals.just_opened() {
            return;
        }
        if bindings.directions.just_pressed(input).is_some() {
            confirming.yes = !confirming.yes;
        }
        let answered = input.just_pressed(bindings.cursor.close) || (take && !confirming.yes);
        // A yes is the offer taken up, written only while the player holds
        // the turn; the screen stays up until they do.
        let agreed = take && confirming.yes && row.open() && holding.single().is_ok();
        if let (true, Ok(player)) = (agreed, holding.single()) {
            intents.write(Intent::new(player, Interact { target: row.target, verb: row.verb, thing: None }));
        }
        if answered || agreed {
            menu.confirming = None;
            modals.close_one(modal);
        }
        return;
    }
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
    if !take {
        return;
    }
    let (Ok(player), Some(row)) = (holding.single(), view.row(menu.selected)) else { return };
    // A refused row stays on screen and says why: taking it up would
    // spend nothing and teach nothing. One that asks first asks, and one
    // that chooses first chooses, in the bag.
    if !row.open() {
        return;
    }
    match &row.asks {
        Some(AsksFirst::YesOrNo(_)) => {
            menu.confirming = Some(Confirming { target: row.target, verb: row.verb, yes: false });
            return;
        }
        Some(AsksFirst::Thing { prompt, among }) => {
            crate::panel::inventory::choose_for_offer(&mut commands, player, row.target, row.verb, prompt.clone(), *among)
        }
        None => {
            intents.write(Intent::new(player, Interact { target: row.target, verb: row.verb, thing: None }));
        }
    }
    modals.close_one(modal);
}

/// A row's yes-or-no question, when it asks one.
fn question(row: &crate::view::offers::OfferRow) -> Option<&str> {
    row.asks.as_ref().and_then(AsksFirst::question)
}

/// What the offers screen draws from.
#[derive(bevy::ecs::system::SystemParam)]
pub struct HereScreen<'w, 's> {
    layout: Res<'w, OffersLayout>,
    view: Res<'w, OffersView>,
    keys: ControlInput<'w>,
    modals: Res<'w, Modals>,
    palette: Res<'w, Palette>,
    map: Option<Res<'w, MapView>>,
    places: Query<'w, 's, &'static Position>,
}

impl HereScreen<'_, '_> {
    /// Where the screen is drawn this frame: its rectangle, or its other
    /// place when the rectangle covers the one choosing or anything a row
    /// is offered by and the other place covers none of them. Worked out
    /// afresh each frame rather than kept, since nothing walks while the
    /// screen is up and a map resized under it takes it to the new mirror.
    /// Without a [`MapView`] nothing is drawn under it to keep clear.
    fn rect(&self) -> Rect {
        let here = self.layout.rect;
        let Some(map) = &self.map else { return here };
        let about = self.view.actor.into_iter().chain(self.view.rows.iter().map(|r| r.target));
        let cells: Vec<rl_core::Point> = about.filter_map(|e| self.places.get(e).ok()).flat_map(|p| map.cells(p.0)).collect();
        let there = other_place(here, self.layout.elsewhere, map.viewport);
        if hides(here, &cells) && !hides(there, &cells) { there } else { here }
    }
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
    let HereScreen { layout, view, keys, modals, palette, .. } = &screen;
    if !modals.is_open(offers_modal(modals)) {
        return;
    }
    let rect = screen.rect();
    if rect.width < 12 || rect.height < 4 {
        return;
    }
    let asked = menu.confirming.and_then(|c| view.rows.iter().find(|r| r.target == c.target && r.verb == c.verb).and_then(question).map(|q| (q, c.yes)));
    if let Some((question, yes)) = asked {
        draw_question(&mut terminal, &screen, rect, question, yes);
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
        let said = match (&row.refused, row.price) {
            (Some(why), _) => format!("{} - {why}", row.label),
            (None, Some(price)) => format!("{} ({}, {})", row.label, cost(row.time), crate::view::sum_of(&view.currency, price)),
            (None, None) => format!("{} ({})", row.label, cost(row.time)),
        };
        terminal.print_on(inner.x, y, &clip(&said, inner.width.max(0) as usize), fg, bg);
    }
}

/// An offer's question in place of the rows: its words wrapped from the
/// top, a line left empty, and its two answers a row each, No first, the
/// one picked out drawn as a picked row is. The answers follow the words
/// after the one empty line, so a short question in a tall screen does not
/// stand over a gap; when the words run long they are cut short rather
/// than the answers, which keep to the bottom, and the empty line goes
/// before the words do, so a screen too short for both still asks
/// something.
fn draw_question(terminal: &mut Terminal, screen: &HereScreen, rect: Rect, question: &str, yes: bool) {
    let HereScreen { layout, keys, palette, .. } = screen;
    let bindings = keys.bindings();
    let hints = format!("{} answer \u{2022} {} never mind", key_name(bindings.cursor.confirm), key_name(bindings.cursor.close));
    clear(terminal, rect, palette);
    frame(terminal, rect, &layout.title, &hints, palette);
    let inner = Rect::new(rect.x + 2, rect.y + 1, rect.width - 4, rect.height - 2);
    let (text, surface, select) = (palette.get(Tones::TEXT), palette.get(Tones::SURFACE), palette.get(Tones::SELECT));
    let room = (inner.height - 3).max((inner.height - 2).min(1)).max(0) as usize;
    let lines = wrap(question, inner.width.max(0) as usize);
    let shown = lines.len().min(room);
    for (i, line) in lines.iter().take(shown).enumerate() {
        terminal.print(inner.x, inner.y + i as i32, line, text);
    }
    let first = (inner.y + shown as i32 + 1).min(inner.bottom() - 2);
    let answers = [(&layout.no, !yes), (&layout.yes, yes)];
    for (i, (word, picked)) in answers.into_iter().enumerate() {
        let y = first + i as i32;
        let bg = if picked { select } else { surface };
        terminal.fill(Rect::new(inner.x, y, inner.width, 1), Cell::new(' ', text).on(bg));
        terminal.print_on(inner.x, y, &clip(word, inner.width.max(0) as usize), text, bg);
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

    /// Someone who is neither foe nor ally and offers two things is asked
    /// about as a bench is, by name.
    #[test]
    fn walking_into_someone_who_offers_two_things_asks_which() {
        let (mut stage, bench) = stage();
        stage.app.world_mut().despawn(bench);
        let verbs = stage.app.world().resource::<Verbs>();
        let offers = Offers(vec![Offering::new(verbs.get("strip").expect("declared"), 100), Offering::new(verbs.get("tip").expect("declared"), 200)]);
        let at = stage.at.offset(1, 0);
        stage.app.world_mut().spawn((Actor, Blocks, Position(at), OnMap(MapId::SURFACE), Name::new("Mirela"), offers));
        stage.tick();

        let player = stage.player;
        stage.app.world_mut().write_message(Intent::new(player, Bump(Direction::East)));
        stage.tick();
        let modals = stage.app.world().resource::<Modals>();
        assert!(modals.is_open(offers_modal(modals)), "the question is asked");
        let view = stage.app.world().resource::<OffersView>();
        assert_eq!(view.rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), vec!["strip Mirela", "tip Mirela"]);
    }

    /// Walking into something locked used to do nothing at all: the one
    /// offer was refused, so the bump came to nothing and said nothing,
    /// and the player was left pressing a direction at a crate. Now the
    /// screen opens and says what it wants.
    #[test]
    fn walking_into_something_locked_says_what_it_wants() {
        let mut stage = Stage::new_with(OffersPanel::new(Rect::new(8, 4, 40, 8)), |app| {
            app.add_plugins(PropsPlugin);
            app.insert_resource(Seed(TEST_SEED));
            let tags = rl_rules::Registry::from_defs(vec![rl_rules::TagDef::new("cutter")]).expect("one tag");
            let props = rl_rules::prop::load(
                r#"#![enable(implicit_some)]
                [
                    (name: "armory locker", glyph: '&', color: (r: 1, g: 2, b: 3), blocks: true,
                     container: (contents: [], locked: "cutter"), offers: [(verb: "open", time: 300)]),
                ]"#,
                &rl_rules::Names::new().tags(&tags),
            )
            .expect("the props load");
            let mut registries = app.world_mut().resource_mut::<Registries>();
            registries.props = props;
            registries.tags = tags;
        });
        let registries = stage.app.world().resource::<Registries>().clone();
        let locker = {
            let id = registries.props.expect("armory locker");
            let at = stage.at.offset(1, 0);
            let mut commands = stage.app.world_mut().commands();
            spawn_prop(&mut commands, &registries, id, at, MapId::SURFACE)
        };
        stage.app.world_mut().flush();
        stage.app.world_mut().entity_mut(locker).insert(Stocked);
        stage.tick();
        stage.tick();

        let player = stage.player;
        stage.app.world_mut().write_message(Intent::new(player, Bump(Direction::East)));
        stage.tick();
        let modals = stage.app.world().resource::<Modals>();
        assert!(modals.is_open(offers_modal(modals)), "walking into it says something rather than nothing");
        let view = stage.app.world().resource::<OffersView>();
        let row = view.rows.iter().find(|r| r.target == locker).expect("the locker is listed");
        assert_eq!(row.refused.as_deref(), Some("needs a cutter"), "and says what it wants");
    }

    /// A row with a price shows it beside the time, and one the purse
    /// cannot meet says how much it wants.
    #[test]
    fn a_priced_row_shows_its_price_and_one_the_player_cannot_pay_says_how_much() {
        let (mut stage, bench) = stage();
        stage.app.world_mut().despawn(bench);
        let tags = rl_rules::Registry::from_defs(vec![rl_rules::TagDef::new("scrip")]).expect("one tag");
        let scrip = tags.expect("scrip");
        stage.app.insert_resource(Currency(scrip));
        stage.app.world_mut().resource_mut::<Registries>().tags = tags;
        let verbs = stage.app.world().resource::<Verbs>();
        let offers = Offers(vec![
            Offering::new(verbs.get("strip").expect("declared"), 100).priced(20),
            Offering::new(verbs.get("tip").expect("declared"), 200).priced(50),
        ]);
        let (at, player) = (stage.at.offset(1, 0), stage.player);
        let coins = stage.app.world_mut().spawn((Item, Name::new("scrip"), Tagged(vec![scrip]), Stack { key: 1, count: 30 })).id();
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![coins] });
        stage.app.world_mut().spawn((Actor, Blocks, Position(at), OnMap(MapId::SURFACE), Name::new("Mirela"), offers));
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, Bump(Direction::East)));
        stage.tick();

        let rows = stage.rows();
        let text = rows.join("\n");
        assert!(rows[5].contains("strip Mirela (1 turn, 20 scrips)"), "the price beside the time:\n{text}");
        assert!(rows[6].contains("tip Mirela - needs 50 scrips"), "and how much a row past the purse wants:\n{text}");
    }

    /// Walking into someone whose one offer has a price the player can
    /// pay asks first, since the walk key never pays.
    #[test]
    fn walking_into_someone_whose_one_offer_has_a_price_asks_before_paying() {
        let (mut stage, bench) = stage();
        stage.app.world_mut().despawn(bench);
        let tags = rl_rules::Registry::from_defs(vec![rl_rules::TagDef::new("scrip")]).expect("one tag");
        let scrip = tags.expect("scrip");
        stage.app.insert_resource(Currency(scrip));
        let strip = stage.app.world().resource::<Verbs>().get("strip").expect("declared");
        let (at, player) = (stage.at.offset(1, 0), stage.player);
        let coins = stage.app.world_mut().spawn((Item, Name::new("scrip"), Tagged(vec![scrip]), Stack { key: 1, count: 80 })).id();
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![coins] });
        stage.app.world_mut().spawn((
            Actor,
            Blocks,
            Position(at),
            OnMap(MapId::SURFACE),
            Name::new("Mirela"),
            Offers(vec![Offering::new(strip, 100).priced(20)]),
        ));
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, Bump(Direction::East)));
        stage.tick();
        let modals = stage.app.world().resource::<Modals>();
        assert!(modals.is_open(offers_modal(modals)), "the screen asks");
        assert_eq!(stage.app.world().get::<Stack>(coins).map(|s| s.count), Some(80), "and nothing was paid");
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
        assert_eq!(
            stage.app.world().resource::<Seen>().0,
            vec![Interacted { actor: player, target: bench, verb: tip, thing: None }],
            "the second row, not the first"
        );
        let modals = stage.app.world().resource::<Modals>();
        assert!(!modals.is_open(offers_modal(modals)), "and the question is answered, so the screen is gone");
    }

    /// Opened again, by a key that tells the screen nothing, the cursor
    /// starts on the first row, not on the row last taken.
    #[test]
    fn the_screen_opens_again_on_its_first_row() {
        let (mut stage, _bench) = stage();
        let player = stage.player;
        stage.app.world_mut().write_message(Intent::new(player, Bump(Direction::East)));
        stage.tick();
        stage.press(KeyCode::ArrowDown);
        stage.tick();
        assert_eq!(stage.app.world().resource::<OffersMenu>().selected, 1);
        stage.press(KeyCode::Escape);
        stage.tick();
        let offers = offers_modal(stage.app.world().resource::<Modals>());
        stage.app.world_mut().resource_mut::<Modals>().open(offers);
        stage.tick();
        assert_eq!(stage.app.world().resource::<OffersMenu>().selected, 0, "on the first row");
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

    /// A stage with Mirela beside the player offering one thing, `tip`,
    /// for two turns, that asks first; every `Interacted` heard kept.
    fn asking() -> (Stage, Entity) {
        let mut stage = Stage::new_with(OffersPanel::new(Rect::new(8, 4, 40, 8)), |app| {
            app.add_plugins(PropsPlugin);
            app.insert_resource(Seed(TEST_SEED));
            app.add_verb("tip");
            app.init_resource::<Heard>().add_systems(Turn, |mut heard: ResMut<Heard>, mut done: MessageReader<Interacted>| {
                heard.0.extend(done.read().copied());
            });
        });
        let tip = stage.app.world().resource::<Verbs>().get("tip").expect("declared");
        let at = stage.at.offset(1, 0);
        let offers = Offers(vec![Offering::new(tip, 200).asking("Tip it over for good?")]);
        let mirela = stage.app.world_mut().spawn((Actor, Blocks, Position(at), OnMap(MapId::SURFACE), Name::new("Mirela"), offers)).id();
        stage.tick();
        stage.tick();
        (stage, mirela)
    }

    /// Every offer taken up, as the game hears it.
    #[derive(Resource, Default)]
    struct Heard(Vec<Interacted>);

    /// Mirela beside the player, on the map's middle row, with the screen
    /// in `panel` opened by the interact key on her asking offer: the row
    /// the screen's title is drawn on.
    fn opened_over_the_map(panel: OffersPanel) -> Option<usize> {
        let mut stage = Stage::new_with(panel, |app| {
            app.add_plugins((PropsPlugin, rl_render::MapViewPlugin::new(Rect::new(0, 0, 40, 20))));
            app.insert_resource(Seed(TEST_SEED));
            app.add_verb("tip");
        })
        .screen(40, 20);
        let tip = stage.app.world().resource::<Verbs>().get("tip").expect("declared");
        let at = stage.at.offset(1, 0);
        let offers = Offers(vec![Offering::new(tip, 200).asking("Tip it over?")]);
        stage.app.world_mut().spawn((Actor, Blocks, Position(at), OnMap(MapId::SURFACE), Name::new("Mirela"), offers));
        stage.tick();
        stage.tick();
        let map = stage.app.world().resource::<MapView>();
        assert_eq!((map.to_screen(stage.at), map.to_screen(at)), (Some(Point::new(20, 10)), Some(Point::new(21, 10))), "both on row ten");
        stage.press(KeyCode::Enter);
        stage.tick();
        stage.rows().iter().position(|r| r.contains("Here"))
    }

    /// The screen over the map moves off the one choosing and whoever it
    /// offers for, as the look panel moves off its cursor: to its mirror
    /// top for bottom, or where the game says; and stays put when neither
    /// stands under it.
    #[test]
    fn the_screen_moves_off_the_player_and_its_offerer_to_its_mirror_or_where_the_game_says() {
        assert_eq!(opened_over_the_map(OffersPanel::new(Rect::new(1, 12, 38, 7))), Some(12), "clear of them, where the game put it");
        assert_eq!(opened_over_the_map(OffersPanel::new(Rect::new(1, 10, 38, 7))), Some(3), "over them, so mirrored top for bottom");
        let named = OffersPanel::new(Rect::new(1, 10, 38, 7)).else_at(Rect::new(1, 1, 38, 7));
        assert_eq!(opened_over_the_map(named), Some(1), "or where the game names");
    }

    /// A short question in a tall screen is followed by one empty row and
    /// then its answers, not by a gap down to the screen's foot.
    #[test]
    fn a_short_question_is_followed_by_one_empty_row_and_then_its_answers() {
        let (mut stage, _) = asking();
        stage.press(KeyCode::Enter);
        stage.tick();
        let rows = stage.rows();
        let inner = |y: usize| rows[y].chars().skip(10).take(36).collect::<String>();
        assert_eq!(inner(5).trim(), "Tip it over for good?", "the question on the first row:\n{}", rows.join("\n"));
        assert_eq!(inner(6).trim(), "", "one empty row");
        assert_eq!((inner(7).trim(), inner(8).trim()), ("No", "Yes"), "then the answers:\n{}", rows.join("\n"));
    }

    /// What the stage has heard taken up, and the turn clock.
    fn done(stage: &Stage) -> (Vec<Interacted>, u32) {
        (stage.app.world().resource::<Heard>().0.clone(), stage.app.world().resource::<Turns>().now())
    }

    /// Walking into Mirela opens the screen rather than taking the offer
    /// up; taking its row up shows its question, No picked out above Yes;
    /// and No, or the close key, takes nothing up and spends no time.
    #[test]
    fn an_offer_that_asks_shows_its_question_and_no_takes_nothing_up_and_costs_nothing() {
        let (mut stage, mirela) = asking();
        let before = done(&stage);
        for key in [KeyCode::Enter, KeyCode::Escape] {
            let player = stage.player;
            stage.app.world_mut().write_message(Intent::new(player, Bump(Direction::East)));
            stage.tick();
            let modals = stage.app.world().resource::<Modals>();
            assert!(modals.is_open(offers_modal(modals)), "a bump opens the screen");
            stage.press(KeyCode::Enter);
            stage.tick();
            let tip = stage.app.world().resource::<Verbs>().get("tip").expect("declared");
            assert_eq!(stage.app.world().resource::<OffersMenu>().confirming, Some(Confirming { target: mirela, verb: tip, yes: false }), "No picked out");
            let rows = stage.rows();
            let text = rows.join("\n");
            assert!(text.contains("Tip it over for good?"), "the question:\n{text}");
            let (no, yes) = (rows.iter().position(|r| r.contains(" No ")), rows.iter().position(|r| r.contains(" Yes ")));
            assert!(matches!((no, yes), (Some(no), Some(yes)) if no + 1 == yes), "No above Yes:\n{text}");
            stage.press(key);
            stage.tick();
            let modals = stage.app.world().resource::<Modals>();
            assert!(!modals.is_open(offers_modal(modals)), "{key:?} puts it away");
            assert_eq!(done(&stage), before, "{key:?} takes nothing up and costs nothing");
        }
    }

    /// The interact key on a lone offer that asks opens the screen on its
    /// question at once, and Yes takes it up, charged its own two turns.
    #[test]
    fn yes_to_an_offers_question_takes_it_up_and_charges_its_time() {
        let (mut stage, mirela) = asking();
        let (_, clock) = done(&stage);
        stage.press(KeyCode::Enter);
        stage.tick();
        assert!(stage.rows().join("\n").contains("Tip it over for good?"), "asked at once:\n{}", stage.rows().join("\n"));
        assert!(done(&stage).0.is_empty(), "nothing taken up yet");
        stage.press(KeyCode::ArrowDown);
        stage.press(KeyCode::Enter);
        stage.tick();
        let (heard, now) = done(&stage);
        let tip = stage.app.world().resource::<Verbs>().get("tip").expect("declared");
        assert_eq!(heard, vec![Interacted { actor: stage.player, target: mirela, verb: tip, thing: None }], "taken up");
        assert_eq!(now - clock, 200, "for its own time");
    }

    /// Mirela stepping out of reach while the question is up takes it
    /// away unanswered; and a yes pressed in the very frame she went, on a
    /// screen still showing her, is refused by the engine as any offer out
    /// of reach is, so nothing is taken up and no time spent.
    #[test]
    fn an_offers_question_whose_offerer_went_out_of_reach_takes_nothing_up() {
        let (mut stage, mirela) = asking();
        let before = done(&stage);
        stage.press(KeyCode::Enter);
        stage.tick();
        stage.app.world_mut().entity_mut(mirela).insert(Position(stage.at.offset(6, 0)));
        stage.tick();
        stage.tick();
        let modals = stage.app.world().resource::<Modals>();
        assert!(!modals.is_open(offers_modal(modals)), "the question goes with her");
        assert_eq!(done(&stage), before, "unanswered");

        stage.app.world_mut().entity_mut(mirela).insert(Position(stage.at.offset(1, 0)));
        stage.tick();
        stage.tick();
        stage.press(KeyCode::Enter);
        stage.tick();
        stage.press(KeyCode::ArrowDown);
        stage.app.world_mut().entity_mut(mirela).insert(Position(stage.at.offset(6, 0)));
        stage.press(KeyCode::Enter);
        stage.tick();
        assert_eq!(done(&stage), before, "a yes to an offer out of reach is refused, for nothing");
    }

    /// A cost reads as turns, since that is what a player spends.
    #[test]
    fn a_cost_reads_as_turns() {
        assert_eq!(cost(100), "1 turn");
        assert_eq!(cost(300), "3 turns");
        assert_eq!(cost(150), "1.50 turns");
    }
}
