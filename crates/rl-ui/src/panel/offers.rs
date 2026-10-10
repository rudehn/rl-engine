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
//! A game may put one more question once an offer of its own is taken up,
//! whether the player means it, with [`AskToConfirm`]: the screen opens on
//! the game's words with two answers, No picked out, the way the title
//! asks before giving up a save. Yes writes [`Confirmed`] for the game to
//! act on; No, or putting the screen away, answers nothing. The offer that
//! asked has spent its time before the question is put, so an offer a
//! player should be able to back out of for nothing is offered at none.

use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_bevy::{Bumped, EngineSet, PresentSet};
use rl_core::{Direction, Rect};
use rl_render::{Cell, Terminal};

use crate::controls::{ControlInput, key_name};
use crate::modal::{AddModal, ModalId, Modals};
use crate::panel::{clear, clip, frame, wrap};
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
    /// The answer to an [`AskToConfirm`] that writes [`Confirmed`].
    pub yes: String,
    /// The answer that writes nothing, picked out when the question is
    /// asked, so a press too many backs out rather than goes ahead.
    pub no: String,
}

/// A game's yes-or-no question about an offer of its own just taken up:
/// whether the player means it.
///
/// The sibling of the bag's [`AskForThing`](crate::AskForThing), for a
/// question with two answers rather than a thing to choose. A game writes
/// it when it hears the offer taken up, most often in `TurnSet::React`:
/// the offers screen opens on `prompt` with No picked out, Yes writes
/// [`Confirmed`] and puts the screen away, and No or the close key put it
/// away answering nothing. The engine does nothing on a yes and spends no
/// time on one, since the offer that asked spent its own. Opened only for
/// the player and only with no screen up, as the bag's question is.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct AskToConfirm {
    /// Who answers: the player.
    pub user: Entity,
    /// Who or what asked, for the game to answer: the one the offer was
    /// taken up on.
    pub asker: Entity,
    /// What for: the verb of the offer taken up, so two questions put by
    /// one asker are told apart.
    pub verb: VerbId,
    /// The question, in the game's words.
    pub prompt: String,
}

/// A yes to an [`AskToConfirm`]: the question's own `user`, `asker` and
/// `verb`.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Confirmed {
    /// Who said yes.
    pub user: Entity,
    /// Who or what asked.
    pub asker: Entity,
    /// What for.
    pub verb: VerbId,
}

/// The screen asking a game's [`AskToConfirm`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirming {
    /// The question.
    pub ask: AskToConfirm,
    /// Whether Yes is picked out rather than No.
    pub yes: bool,
}

/// Which row the cursor is on, and the question while one is asked. The
/// screen's own state, since a view holds no cursor.
#[derive(Resource, Debug, Default)]
pub struct OffersMenu {
    /// The row picked out, clamped to the rows there are when drawn.
    pub selected: usize,
    /// A game's question, while the screen asks it in place of the rows.
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
        Self(OffersLayout { rect, title: "Here".into(), yes: "Yes".into(), no: "No".into() })
    }

    /// Replaces the two answers to an [`AskToConfirm`].
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
        app.insert_resource(self.0.clone()).init_resource::<OffersMenu>().add_message::<AskToConfirm>().add_message::<Confirmed>();
        // A game's question opens the screen before it reads its keys, in
        // the frame after the offer that asked was taken up.
        app.add_systems(Update, (confirm_when_asked, offers_keys).chain().in_set(EngineSet::Input))
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
        let free = view.rows.iter().filter(|r| r.target == ev.into && r.open() && r.price.is_none()).count();
        // Nothing to say, or the bump already did the one free thing on
        // offer; a priced one it left for this screen.
        if rows == 0 || (rows == 1 && free == 1) {
            continue;
        }
        menu.selected = 0;
        if !modals.any_open() {
            modals.open(modal);
        }
    }
}

/// Opens the screen on a game's [`AskToConfirm`], for the player and only
/// with no screen up.
pub fn confirm_when_asked(mut asked: MessageReader<AskToConfirm>, players: Query<(), With<Player>>, mut menu: ResMut<OffersMenu>, mut modals: ResMut<Modals>) {
    let modal = offers_modal(&modals);
    for ask in asked.read() {
        if players.contains(ask.user) && !modals.any_open() {
            menu.confirming = Some(Confirming { ask: ask.clone(), yes: false });
            modals.open(modal);
        }
    }
}

/// Opens the screen, walks it, takes the row picked out, and closes it;
/// or, while a game's question is up, moves between its answers and
/// answers it.
pub fn offers_keys(
    keys: ControlInput,
    view: Res<OffersView>,
    mut menu: ResMut<OffersMenu>,
    mut modals: ResMut<Modals>,
    mut intents: MessageWriter<Intent<Interact>>,
    mut confirmed: MessageWriter<Confirmed>,
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
    if let Some(confirming) = menu.confirming.as_mut() {
        if modals.just_opened() {
            return;
        }
        let take = input.just_pressed(bindings.cursor.confirm) || input.just_pressed(bindings.cursor.also_confirm);
        if bindings.directions.just_pressed(input).is_some() {
            confirming.yes = !confirming.yes;
        }
        let answered = input.just_pressed(bindings.cursor.close) || (take && !confirming.yes);
        // A yes is an act, taken only while the player holds the turn; the
        // screen stays up until they do.
        let agreed = take && confirming.yes && holding.contains(confirming.ask.user);
        if agreed {
            let ask = &confirming.ask;
            confirmed.write(Confirmed { user: ask.user, asker: ask.asker, verb: ask.verb });
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
    if !input.just_pressed(bindings.cursor.confirm) && !input.just_pressed(bindings.cursor.also_confirm) {
        return;
    }
    let (Ok(player), Some(row)) = (holding.single(), view.row(menu.selected)) else { return };
    // A refused row stays on screen and says why: taking it up would
    // spend nothing and teach nothing.
    if row.open() {
        intents.write(Intent::new(player, Interact { target: row.target, verb: row.verb }));
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
    if let Some(confirming) = &menu.confirming {
        draw_question(&mut terminal, &screen, confirming);
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

/// A game's question in place of the rows: its words wrapped from the
/// top, a line left empty, and its two answers a row each, No first, the
/// one picked out drawn as a picked row is. The answers are kept to the
/// bottom of the screen when the words run long, which are cut short
/// rather than the answers.
fn draw_question(terminal: &mut Terminal, screen: &HereScreen, confirming: &Confirming) {
    let HereScreen { layout, keys, palette, .. } = screen;
    let (rect, bindings) = (layout.rect, keys.bindings());
    let hints = format!("{} answer \u{2022} {} never mind", key_name(bindings.cursor.confirm), key_name(bindings.cursor.close));
    clear(terminal, rect, palette);
    frame(terminal, rect, &layout.title, &hints, palette);
    let inner = Rect::new(rect.x + 2, rect.y + 1, rect.width - 4, rect.height - 2);
    let (text, surface, select) = (palette.get(Tones::TEXT), palette.get(Tones::SURFACE), palette.get(Tones::SELECT));
    let room = (inner.height - 3).max(0) as usize;
    for (i, line) in wrap(&confirming.ask.prompt, inner.width.max(0) as usize).iter().take(room).enumerate() {
        terminal.print(inner.x, inner.y + i as i32, line, text);
    }
    let answers = [(&layout.no, !confirming.yes), (&layout.yes, confirming.yes)];
    for (i, (word, picked)) in answers.into_iter().enumerate() {
        let y = inner.bottom() - 2 + i as i32;
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
        assert_eq!(stage.app.world().resource::<Seen>().0, vec![Interacted { actor: player, target: bench, verb: tip }], "the second row, not the first");
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

    /// Puts a game's question to the player about `bench`'s `tip`, and lets
    /// the screen open on it.
    fn ask(stage: &mut Stage, bench: Entity) -> AskToConfirm {
        let tip = stage.app.world().resource::<Verbs>().get("tip").expect("the game's verb");
        let ask = AskToConfirm { user: stage.player, asker: bench, verb: tip, prompt: "Tip the workbench over for good?".into() };
        stage.app.world_mut().write_message(ask.clone());
        stage.tick();
        ask
    }

    /// Every yes the screen has written since the last look.
    fn yeses(stage: &mut Stage) -> Vec<Confirmed> {
        stage.app.world_mut().resource_mut::<Messages<Confirmed>>().drain().collect()
    }

    /// A game's question opens the screen on its words with No picked out,
    /// and taking No, or putting the screen away, answers nothing.
    #[test]
    fn a_games_question_opens_on_no_and_backing_out_of_it_answers_nothing() {
        let (mut stage, bench) = stage();
        let ask = ask(&mut stage, bench);
        let modals = stage.app.world().resource::<Modals>();
        assert!(modals.is_open(offers_modal(modals)), "the question is asked");
        assert_eq!(stage.app.world().resource::<OffersMenu>().confirming, Some(Confirming { ask: ask.clone(), yes: false }), "No picked out");
        let rows = stage.rows();
        let text = rows.join("\n");
        assert!(text.contains("Tip the workbench over for good?"), "in the game's words:\n{text}");
        let (no, yes) = (rows.iter().position(|r| r.contains(" No ")), rows.iter().position(|r| r.contains(" Yes ")));
        assert!(matches!((no, yes), (Some(no), Some(yes)) if no + 1 == yes), "No above Yes:\n{text}");

        for key in [KeyCode::Enter, KeyCode::Escape] {
            if key == KeyCode::Escape {
                ask_again(&mut stage, &ask);
            }
            stage.press(key);
            stage.tick();
            let modals = stage.app.world().resource::<Modals>();
            assert!(!modals.is_open(offers_modal(modals)), "{key:?} puts it away");
            assert_eq!(stage.app.world().resource::<OffersMenu>().confirming, None, "{key:?}");
            assert!(yeses(&mut stage).is_empty(), "{key:?} answers nothing");
        }
    }

    /// Puts `ask` to the player again.
    fn ask_again(stage: &mut Stage, ask: &AskToConfirm) {
        stage.app.world_mut().write_message(ask.clone());
        stage.tick();
    }

    /// A direction moves to Yes, and taking it writes the question's own
    /// answer and puts the screen away; nothing is done for it but that.
    #[test]
    fn yes_to_a_games_question_writes_confirmed_and_puts_the_screen_away() {
        let (mut stage, bench) = stage();
        let ask = ask(&mut stage, bench);
        stage.press(KeyCode::ArrowDown);
        stage.tick();
        assert!(stage.app.world().resource::<OffersMenu>().confirming.as_ref().is_some_and(|c| c.yes), "Yes picked out");
        stage.press(KeyCode::Enter);
        stage.tick();
        assert_eq!(yeses(&mut stage), vec![Confirmed { user: ask.user, asker: ask.asker, verb: ask.verb }]);
        let modals = stage.app.world().resource::<Modals>();
        assert!(!modals.is_open(offers_modal(modals)), "and the screen is gone");
    }

    /// A question is never put over another screen, nor to anyone but the
    /// player.
    #[test]
    fn a_games_question_waits_for_no_screen_and_is_put_only_to_the_player() {
        let (mut stage, bench) = stage();
        let mut ask = ask(&mut stage, bench);
        stage.press(KeyCode::Escape);
        stage.tick();
        ask.user = bench;
        ask_again(&mut stage, &ask);
        assert!(!stage.app.world().resource::<Modals>().any_open(), "asked of someone else");
        let other = stage.app.world_mut().resource_mut::<Modals>().declare("other");
        stage.app.world_mut().resource_mut::<Modals>().open(other);
        ask.user = stage.player;
        ask_again(&mut stage, &ask);
        let modals = stage.app.world().resource::<Modals>();
        assert!(!modals.is_open(offers_modal(modals)), "over another screen");
    }

    /// A cost reads as turns, since that is what a player spends.
    #[test]
    fn a_cost_reads_as_turns() {
        assert_eq!(cost(100), "1 turn");
        assert_eq!(cost(300), "3 turns");
        assert_eq!(cost(150), "1.50 turns");
    }
}
