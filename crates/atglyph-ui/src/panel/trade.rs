//! What a trader sells and what it would pay, and the keys that deal.
//!
//! A modal the engine runs end to end, like the container screen: it opens
//! itself when the engine opens a trade, the direction keys walk the rows
//! and cross between the two sides, the cursors' confirm key deals one of
//! the row picked out, [`TradeKeys::whole`] deals the whole of it, and the
//! cursors' close key puts it away and ends the trade. A game binds
//! nothing, and rebinds the one key of its own with a `TradeKeys`.
//!
//! The trader's wares are on the left and the customer's pack on the
//! right, so the key that takes from the container screen buys from the
//! left and the same key sells from the right: one key that moves the
//! thing picked out across, whichever side it is on. Dealing does not
//! close the screen, since a deal takes no turn and a shop is usually
//! more than one; it closes itself when the trade ends, as the container
//! screen does when its container goes out of reach.
//!
//! Under the two lists, where the screen is tall enough, the row picked
//! out on either side is described as the bag describes it, under a rule
//! naming it, and weighed against what the customer wears, so a helm is
//! seen to beat the cap worn before it is bought.
//!
//! Every word it draws is the game's or a [`TradeWords`] default the game
//! may replace: the title is the trader's `Name`, the line under it is
//! the game's facet, and the currency is named by the currency's own
//! `Name`.

use atglyph_bevy::prelude::*;
use atglyph_bevy::{EngineSet, PresentSet};
use atglyph_core::{Direction, GridRect};
use atglyph_render::{Cell, Terminal};
use bevy::prelude::*;

use crate::app::UiApp;
use crate::controls::{Chord, ControlInput, key_name};
use crate::modal::{ModalId, Modals};
use crate::panel::inventory::{describe, print_lines};
use crate::panel::{clear, clip, frame, section};
use crate::tone::{Palette, Tones};
use crate::view::trade::{TradeRow, TradeView, TradeViewPlugin};

/// The name the trade screen's modal is declared under.
pub(crate) const TRADE_MODAL: &str = "trade";

/// The keys the trade screen answers to, beside the direction keys that
/// walk the rows and cross the sides and the cursors' keys that deal one
/// and close.
///
/// A default, not a rule: a game that wants another key inserts its own
/// `TradeKeys`, before or after the panel is added, and the screen's
/// bottom border names the key it holds. `a` because it is the container
/// screen's key for all of it, and on this screen all of it is the whole
/// of a stack.
#[derive(Resource, Debug, Clone)]
pub struct TradeKeys {
    /// Deals the whole of the row picked out.
    pub whole: Chord,
}

impl Default for TradeKeys {
    fn default() -> Self {
        Self { whole: Chord::key(KeyCode::KeyA) }
    }
}

/// What the trade screen says that is not a name or the game's line.
///
/// Plain defaults a game replaces with its own, since a shop in one game
/// says "Wares" where another's says "Stock".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeWords {
    /// Over the trader's side.
    pub wares: String,
    /// Over the customer's side.
    pub pack: String,
    /// In place of rows on a side with none.
    pub empty: String,
    /// The confirm key's hint on the trader's side.
    pub buy: String,
    /// The confirm key's hint on the customer's side.
    pub sell: String,
    /// The whole-row key's hint.
    pub all: String,
    /// The close key's hint.
    pub close: String,
    /// The customer carries too little for a ware; drawn before its price.
    pub short: String,
    /// The trader carries too little to pay for what the customer would
    /// sell; drawn before what it would pay. Its own words, since on the
    /// customer's side it is not the customer who is short.
    pub trader_short: String,
    /// The trader does not buy a thing of its kind.
    pub not_bought: String,
    /// It has no worth.
    pub no_worth: String,
    /// It is worn.
    pub worn: String,
    /// After the price on a row of more than one, which is the price of
    /// one, since the confirm key deals one: without it `10 arrows  2`
    /// reads as the ten for two.
    pub each: String,
    /// What the customer's own blow and shot are called where the pane
    /// weighs gear, as the bag's [`own`](crate::panel::InventoryPanel::own).
    pub own: String,
}

impl Default for TradeWords {
    fn default() -> Self {
        Self {
            wares: "For sale".into(),
            pack: "Yours".into(),
            empty: "Nothing.".into(),
            buy: "buy".into(),
            sell: "sell".into(),
            all: "all".into(),
            close: "close".into(),
            short: "cannot pay".into(),
            trader_short: "they lack".into(),
            not_bought: "not wanted".into(),
            no_worth: "no worth".into(),
            worn: "worn".into(),
            each: "each".into(),
            own: "own".into(),
        }
    }
}

impl TradeWords {
    /// What `why` is called on the side a `deal` is struck from: a short
    /// purse is the customer's buying and the trader's selling to it.
    pub fn refusal(&self, why: TradeRefusal, deal: Deal) -> &str {
        match why {
            TradeRefusal::Short if deal == Deal::Sold => &self.trader_short,
            TradeRefusal::Short => &self.short,
            TradeRefusal::NotBought => &self.not_bought,
            TradeRefusal::NoWorth => &self.no_worth,
            TradeRefusal::Worn => &self.worn,
        }
    }
}

/// Where the trade screen is drawn, and what it says.
#[derive(Resource, Debug, Clone)]
pub(crate) struct TradeLayout {
    /// The terminal cells it occupies.
    pub rect: GridRect,
    /// Its words.
    pub words: TradeWords,
}

/// Which side of a trade the cursor is on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TradeSide {
    /// The trader's wares, which the confirm key buys from.
    #[default]
    Wares,
    /// The customer's pack, which the confirm key sells from.
    Pack,
}

/// Where the cursor is. The screen's own state, since a view holds no
/// cursor; each side keeps its own row, so crossing over and back finds
/// the row left.
#[derive(Resource, Debug, Default)]
pub struct TradeMenu {
    /// The side picked out.
    pub side: TradeSide,
    /// The row picked out among the wares.
    pub wares: usize,
    /// The row picked out in the pack.
    pub pack: usize,
}

impl TradeMenu {
    /// The row picked out on the side picked out.
    pub(crate) fn selected(&self) -> usize {
        match self.side {
            TradeSide::Wares => self.wares,
            TradeSide::Pack => self.pack,
        }
    }

    /// Moves the cursor `by` rows over `rows` of them on the side picked
    /// out, wrapping at either end.
    pub(crate) fn move_by(&mut self, by: i32, rows: usize) {
        let at = match self.side {
            TradeSide::Wares => &mut self.wares,
            TradeSide::Pack => &mut self.pack,
        };
        *at = if rows == 0 { 0 } else { (*at as i64 + by as i64).rem_euclid(rows as i64) as usize };
    }
}

/// Draws [`TradeView`] as two lists side by side, and runs the screen.
///
/// Adds [`TradeViewPlugin`] if the game has not, and declares the `trade`
/// modal.
pub struct TradePanel(TradeLayout);

impl TradePanel {
    /// The trade screen in `rect`, titled by whatever the trader is
    /// called, so the engine never names a shop.
    pub fn new(rect: GridRect) -> Self {
        Self(TradeLayout { rect, words: TradeWords::default() })
    }

    /// Replaces the words the screen draws.
    pub fn words(mut self, words: TradeWords) -> Self {
        self.0.words = words;
        self
    }
}

impl Plugin for TradePanel {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<TradeViewPlugin>() {
            app.add_plugins(TradeViewPlugin);
        }
        app.add_modal(TRADE_MODAL);
        app.insert_resource(self.0.clone()).init_resource::<TradeMenu>().init_resource::<TradeKeys>();
        app.add_systems(Update, (follow_trade, trade_keys).chain().in_set(EngineSet::Input)).add_systems(Update, draw_trade.in_set(PresentSet::Overlay));
    }

    fn finish(&self, app: &mut App) {
        app.depends_on::<crate::UiPlugin>("TradePanel");
        app.depends_on::<TradePlugin>("TradePanel");
        // Nothing on the controls screen: the screen opens itself when a
        // trade is opened, and what it answers to is written along its own
        // bottom border, as the container screen's is.
    }
}

/// The id of the trade screen's modal, for a game gating its own systems.
///
/// # Panics
/// Panics if [`TradePanel`] was not added.
pub fn trade_modal(modals: &Modals) -> ModalId {
    modals.get(TRADE_MODAL).expect("TradePanel declares the trade modal")
}

/// Keeps the screen and the engine's [`Trading`] in step.
///
/// A trade the engine opens puts the screen up; a trade the engine ends,
/// the trader out of reach or gone, takes it down; and a screen something
/// else put away, such as a bag that closes every screen on a use, ends
/// the trade, so no trade stays open with nothing showing it.
pub(crate) fn follow_trade(mut trading: ResMut<Trading>, mut menu: ResMut<TradeMenu>, mut modals: ResMut<Modals>) {
    let modal = trade_modal(&modals);
    let up = modals.is_open(modal);
    match trading.open() {
        Some(_) if trading.is_changed() && !up => {
            *menu = TradeMenu::default();
            modals.open(modal);
        }
        Some(_) if !up => trading.close(),
        None if up => modals.close_one(modal),
        _ => {}
    }
}

/// What the trade screen's keys act on.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Counter<'w> {
    binds: Res<'w, TradeKeys>,
    view: Res<'w, TradeView>,
    menu: ResMut<'w, TradeMenu>,
    modals: ResMut<'w, Modals>,
    trading: ResMut<'w, Trading>,
}

/// Walks the rows, crosses the sides, deals, and closes.
///
/// A deal is written whether or not the row says it would be refused, so
/// the engine's resolver stays the one judge of it: a refused one is
/// answered with [`TradeRefused`](atglyph_bevy::TradeRefused), for a game that wants a line saying
/// why, over the row the screen already greys.
pub(crate) fn trade_keys(keys: ControlInput, mut screen: Counter, mut buys: MessageWriter<Buy>, mut sells: MessageWriter<Sell>) {
    let Counter { binds, view, menu, modals, trading } = &mut screen;
    let modal = trade_modal(modals);
    if !modals.is_top(modal) {
        return;
    }
    let bindings = keys.bindings();
    let input = keys.input();
    if input.just_pressed(bindings.cursor.close) {
        modals.close_one(modal);
        trading.close();
        return;
    }
    let rows = |side| match side {
        TradeSide::Wares => &view.wares,
        TradeSide::Pack => &view.pack,
    };
    let here = rows(menu.side).len();
    match bindings.directions.just_pressed(input) {
        Some(Direction::North) => menu.move_by(-1, here),
        Some(Direction::South) => menu.move_by(1, here),
        Some(Direction::West) => menu.side = TradeSide::Wares,
        Some(Direction::East) => menu.side = TradeSide::Pack,
        _ => {}
    }
    let whole = binds.whole.just_pressed(input);
    let one = input.just_pressed(bindings.cursor.confirm) || input.just_pressed(bindings.cursor.also_confirm);
    let (Some(parties), Some(row)) = (view.parties, rows(menu.side).get(menu.selected())) else { return };
    if !whole && !one {
        return;
    }
    // One of the row, or all of it: the whole of a stack, or each of the
    // things alike the row stands for, one deal apiece.
    let deals: Vec<(Entity, u32)> = match (whole, row.items.len() > 1) {
        (true, true) => row.items.iter().map(|item| (*item, 1)).collect(),
        (true, false) => vec![(row.row.entity, row.count)],
        (false, _) => vec![(row.row.entity, 1)],
    };
    let (customer, trader) = (parties.customer, parties.trader);
    for (item, count) in deals {
        match menu.side {
            TradeSide::Wares => {
                buys.write(Buy { customer, trader, item, count });
            }
            TradeSide::Pack => {
                sells.write(Sell { customer, trader, item, count });
            }
        }
    }
}

/// Everything the trade screen draws from.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct ShopScreen<'w> {
    layout: Res<'w, TradeLayout>,
    view: Res<'w, TradeView>,
    binds: Res<'w, TradeKeys>,
    keys: ControlInput<'w>,
    modals: Res<'w, Modals>,
    palette: Res<'w, Palette>,
}

/// Draws the open trade: the trader's name and the game's line under it,
/// the two sides with a price on each row, each side's currency at its
/// foot, and the keys that do something.
pub(crate) fn draw_trade(mut terminal: ResMut<Terminal>, mut menu: ResMut<TradeMenu>, screen: ShopScreen) {
    let ShopScreen { layout, view, binds, keys, modals, palette } = &screen;
    if !modals.is_open(trade_modal(modals)) {
        return;
    }
    let (rect, words) = (layout.rect, &layout.words);
    if rect.width < 24 || rect.height < 8 {
        return;
    }
    menu.wares = menu.wares.min(view.wares.len().saturating_sub(1));
    menu.pack = menu.pack.min(view.pack.len().saturating_sub(1));
    let bindings = keys.bindings();
    let deal = match menu.side {
        TradeSide::Wares => &words.buy,
        TradeSide::Pack => &words.sell,
    };
    let hints = [
        format!("{} {deal}", key_name(bindings.cursor.confirm)),
        format!("{} {}", binds.whole.label(), words.all),
        format!("{} {}", key_name(bindings.cursor.close), words.close),
    ];
    let title = view.trader.as_ref().map_or("", |t| t.label.as_str());
    clear(&mut terminal, rect, palette);
    frame(&mut terminal, rect, title, &hints.join(" \u{2022} "), palette);
    let inner = GridRect::new(rect.x + 2, rect.y + 1, rect.width - 4, rect.height - 2);
    let surface = palette.get(Tones::SURFACE);

    // The game's line, in its own tones, one facet after another.
    let mut x = inner.x;
    for facet in view.trader.iter().flat_map(|t| &t.facets) {
        let room = (inner.right() - x).max(0) as usize;
        let text = clip(&facet.text, room);
        terminal.print_on(x, inner.y, &text, palette.get(facet.tone), surface);
        x += text.chars().count() as i32 + 1;
    }

    // The pane under the lists, its rule and a blank row parting it from
    // the purses, where the screen is tall enough to keep a useful list
    // over it: as tall as the longest description of a row on either
    // side, so it holds still as the cursor moves and spends no row on
    // nothing, and at most a third or so of the height and ten rows; none
    // at all where that leaves fewer than three, so a short screen keeps
    // its lists whole.
    let room = ((inner.height - 10) / 2).min(10);
    let width = inner.width.max(0) as usize;
    let longest = view.wares.iter().chain(&view.pack).filter_map(|r| r.described.as_ref()).map(|d| describe(d, width, &words.own).len()).max().unwrap_or(0);
    let pane = if room >= 3 { (longest as i32).clamp(1, room) } else { 0 };
    let under = if pane > 0 { pane + 2 } else { 0 };
    let gap = 2;
    let half = (inner.width - gap) / 2;
    let lists = GridRect::new(inner.x, inner.y + 2, half, inner.height - 3 - under);
    let wares = lists;
    let pack = GridRect::new(inner.x + half + gap, lists.y, inner.width - half - gap, lists.height);
    let selected = |side| (menu.side == side).then_some(menu.selected());
    draw_side(&mut terminal, wares, (&words.wares, Deal::Bought), &view.wares, selected(TradeSide::Wares), words, palette);
    draw_side(&mut terminal, pack, (&words.pack, Deal::Sold), &view.pack, selected(TradeSide::Pack), words, palette);

    // Each purse under its own side, so whose is short reads off the
    // screen as well as off the row.
    let purse = |amount: u32| crate::view::sum_of(&view.currency, amount);
    for (side, amount) in [(wares, view.trader_purse), (pack, view.purse)] {
        terminal.print_on(side.x, inner.bottom() - 1 - under, &clip(&purse(amount), side.width.max(0) as usize), palette.get(Tones::NOTICE), surface);
    }
    if pane > 0 {
        let picked = match menu.side {
            TradeSide::Wares => view.wares.get(menu.wares),
            TradeSide::Pack => view.pack.get(menu.pack),
        };
        draw_pane(&mut terminal, GridRect::new(inner.x, inner.bottom() - pane - 1, inner.width, pane + 1), picked, &words.own, palette);
    }
}

/// The row picked out, on whichever side, described as the bag describes
/// it, under a rule that names it: what it is, and against what the
/// customer wears, what putting it on would change.
fn draw_pane(terminal: &mut Terminal, rect: GridRect, picked: Option<&TradeRow>, own: &str, palette: &Palette) {
    for x in rect.x..rect.right() {
        terminal.put(x, rect.y, '\u{2500}', palette.get(Tones::FRAME));
    }
    let Some(row) = picked else { return };
    let width = rect.width.max(0) as usize;
    let name = clip(&row.row.label, width.saturating_sub(6));
    if !name.is_empty() {
        terminal.print_on(rect.x + 2, rect.y, &format!(" {name} "), palette.get(Tones::TITLE), palette.get(Tones::SURFACE));
    }
    let lines = row.described.as_ref().map(|item| describe(item, width, own)).unwrap_or_default();
    print_lines(terminal, (rect.x, rect.y + 1, rect.bottom()), width, &lines, palette);
}

/// One side of the trade under its heading, and the deal its rows are
/// struck by, the row picked out lit when the cursor is on this side.
fn draw_side(
    terminal: &mut Terminal,
    rect: GridRect,
    (heading, deal): (&str, Deal),
    rows: &[TradeRow],
    selected: Option<usize>,
    words: &TradeWords,
    palette: &Palette,
) {
    section(terminal, rect, rect.y, heading, None, palette);
    let list = GridRect::new(rect.x, rect.y + 2, rect.width, rect.height - 2);
    let surface = palette.get(Tones::SURFACE);
    if rows.is_empty() {
        terminal.print_on(list.x, list.y, &clip(&words.empty, list.width.max(0) as usize), palette.get(Tones::MUTED), surface);
        return;
    }
    let shown = (list.height.max(0) as usize).min(rows.len());
    let at = selected.unwrap_or(0);
    let first = at.saturating_sub(shown.saturating_sub(1)).min(rows.len().saturating_sub(shown));
    for (i, row) in rows.iter().enumerate().skip(first).take(shown) {
        let y = list.y + (i - first) as i32;
        let bg = if selected == Some(i) { palette.get(Tones::SELECT) } else { surface };
        let fg = palette.get(if row.open() { Tones::TEXT } else { Tones::MUTED });
        terminal.fill(GridRect::new(list.x, y, list.width, 1), Cell::new(' ', fg).on(bg));
        // What one changes hands for, or why it cannot: a short purse
        // still shows the price, since how short is the useful part.
        let price = match (row.refused, row.each) {
            (None, Some(each)) => each.to_string(),
            (Some(TradeRefusal::Short), Some(each)) => format!("{} {each}", words.refusal(TradeRefusal::Short, deal)),
            (Some(why), _) => words.refusal(why, deal).to_string(),
            (None, None) => String::new(),
        };
        let room = (list.width / 2).max(0) as usize;
        // The glyph and its space, the name, and the space before the price.
        let beside = (list.width.max(0) as usize).saturating_sub(row.row.label.chars().count() + 3);
        let price = match (row.count > 1, row.each) {
            // A row of more than one always says its price is for one,
            // since a bare `60` beside `2 leggings` reads as the two for
            // sixty: the whole of it where it fits beside the name, then
            // without the refusal, which the row's muted tone says too,
            // and else the name is cut short to make room, never the price.
            (true, Some(each)) if matches!(row.refused, None | Some(TradeRefusal::Short)) => {
                let bare = format!("{each} {}", words.each);
                [format!("{price} {}", words.each), bare.clone()].into_iter().find(|said| said.chars().count() <= room.min(beside)).unwrap_or(bare)
            }
            _ => clip(&price, room),
        };
        let price_x = list.right() - price.chars().count() as i32;
        terminal.print_on(price_x, y, &price, fg, bg);
        terminal.set(list.x, y, Cell::new(row.row.glyph.ch, row.row.glyph.fg).on(bg));
        let mut x = list.x + 2;
        let end = price_x - 1;
        let label = clip(&row.row.label, (end - x).max(0) as usize);
        terminal.print_on(x, y, &label, fg, bg);
        x += label.chars().count() as i32 + 1;
        // What the game added to the row, after its name, as the bag
        // draws it, for as much of it as fits.
        for facet in &row.row.facets {
            let text = clip(&facet.text, (end - x).max(0) as usize);
            if text.is_empty() {
                break;
            }
            terminal.print_on(x, y, &text, palette.get(facet.tone), bg);
            x += text.chars().count() as i32 + 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use atglyph_bevy::testing::TEST_SEED;
    use atglyph_rules::{Registry, TagDef};

    /// A trader beside the player with two files worth 10, buying tools at
    /// half from a till of 9; the player with 25 in scrip and an awl worth
    /// 6. The screen is added and the trade opened.
    fn stage() -> (Stage, Entity) {
        stage_in(GridRect::new(10, 5, 60, 12))
    }

    /// [`stage`] with the screen in `rect`.
    fn stage_in(rect: GridRect) -> (Stage, Entity) {
        let mut stage = Stage::new_with(TradePanel::new(rect), |app| {
            app.add_plugins((PropsPlugin, TradePlugin));
            app.insert_resource(Seed(TEST_SEED));
            let tags = Registry::from_defs(vec![TagDef::new("scrip"), TagDef::new("tool")]).expect("two tags");
            app.insert_resource(Currency(tags.expect("scrip")));
            app.world_mut().resource_mut::<Registries>().tags = tags;
        });
        let tags = stage.app.world().resource::<Registries>().tags.clone();
        let (scrip, tool) = (tags.expect("scrip"), tags.expect("tool"));
        let glyph = atglyph_render::Glyph::new('-', Color::WHITE);
        let w = stage.app.world_mut();
        let till = w.spawn((Item, Name::new("scrip"), Tagged(vec![scrip]), Stack { key: 1, count: 9 })).id();
        let files = w.spawn((Item, Name::new("file"), glyph, Tagged(vec![tool]), Stack { key: 2, count: 2 }, Worth(10))).id();
        let purse = w.spawn((Item, Name::new("scrip"), Tagged(vec![scrip]), Stack { key: 1, count: 25 })).id();
        let awl = w.spawn((Item, Name::new("awl"), glyph, Tagged(vec![tool]), Worth(6))).id();
        let at = stage.at.offset(1, 0);
        let trader = w.spawn((Actor, Name::new("Odda"), Position(at), Inventory { items: vec![till, files] }, Trader { buys: vec![tool], pays_pct: 50 })).id();
        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![purse, awl] });
        let trade = stage.app.world().resource::<Verbs>().get(TRADE).expect("declared");
        stage.app.world_mut().write_message(Interacted { actor: player, target: trader, verb: trade, thing: None });
        stage.tick();
        stage.tick();
        (stage, trader)
    }

    fn is_up(stage: &Stage) -> bool {
        let modals = stage.app.world().resource::<Modals>();
        modals.is_open(trade_modal(modals))
    }

    /// The screen goes up when the engine opens the trade the offer asked
    /// for, and comes down with the trade when the trader is out of reach.
    #[test]
    fn the_screen_opens_on_the_trade_offer_and_closes_when_the_trader_is_out_of_reach() {
        let (mut stage, _trader) = stage();
        assert!(is_up(&stage), "the trade opened the screen");

        let player = stage.player;
        let far = stage.at.offset(-6, 0);
        stage.app.world_mut().entity_mut(player).insert(Position(far));
        stage.tick();
        stage.tick();
        assert!(stage.app.world().resource::<Trading>().open().is_none(), "the engine ended the trade");
        assert!(!is_up(&stage), "and the screen closed with it");
    }

    /// The confirm key buys one of the ware picked out, the whole-row key
    /// sells all of the thing picked out on the other side, and neither
    /// closes the screen; the close key ends the trade.
    #[test]
    fn confirm_buys_one_across_the_sides_the_whole_key_sells_a_row_and_close_ends_the_trade() {
        let (mut stage, trader) = stage();
        stage.press(KeyCode::Enter);
        stage.tick();
        let view = stage.app.world().resource::<TradeView>();
        assert_eq!(view.wares.iter().map(|r| r.row.label.as_str()).collect::<Vec<_>>(), vec!["file"], "one of the two files was bought");
        assert_eq!(view.purse, 15, "for its worth");
        assert!(view.pack.iter().any(|r| r.row.label == "file"), "and the file is in the pack");

        stage.press(KeyCode::ArrowRight);
        stage.tick();
        stage.press(KeyCode::KeyA);
        stage.tick();
        let view = stage.app.world().resource::<TradeView>();
        assert_eq!(view.pack.iter().map(|r| r.row.label.as_str()).collect::<Vec<_>>(), vec!["file"], "the awl was sold");
        assert_eq!(view.purse, 18, "for half its worth");
        assert!(is_up(&stage), "dealing leaves the screen up");
        assert!(stage.app.world().get::<Inventory>(trader).is_some_and(|b| b.items.len() == 3), "the trader carries its till, a file and the awl");

        stage.press(KeyCode::Escape);
        stage.tick();
        assert!(!is_up(&stage), "the close key puts it away");
        assert!(stage.app.world().resource::<Trading>().open().is_none(), "and ends the trade");
    }

    /// A game's own `TradeKeys` moves the whole-row key: its key deals the
    /// whole of a row, `a` no longer does, and the screen's border names
    /// the key it holds.
    #[test]
    fn a_games_own_trade_keys_move_the_whole_row_key_and_the_border_names_it() {
        let (mut stage, trader) = stage();
        stage.app.insert_resource(TradeKeys { whole: Chord::key(KeyCode::KeyW) });
        stage.press(KeyCode::ArrowRight);
        stage.tick();
        stage.press(KeyCode::KeyA);
        stage.tick();
        assert_eq!(stage_pack(&stage), vec!["awl"], "the old key deals nothing");
        stage.press(KeyCode::KeyW);
        stage.tick();
        assert_eq!(stage_pack(&stage), Vec::<String>::new(), "the game's key sold the awl");
        assert!(stage.app.world().get::<Inventory>(trader).is_some_and(|b| b.items.len() == 3), "to the trader");
        let rows = stage.rows();
        assert!(rows[16].contains(" w all "), "the border names the game's key:\n{}", rows.join("\n"));
    }

    /// A counted row always says its price is for one: where its name and
    /// `10 each` will not both fit, the name is cut short, never the word,
    /// since a bare `10` beside `3 flagons` reads as the three for ten.
    #[test]
    fn a_counted_rows_price_always_says_each_and_its_name_gives_way() {
        let (mut stage, trader) = stage();
        let glyph = atglyph_render::Glyph::new('!', Color::WHITE);
        let cider = stage.app.world_mut().spawn((Item, Name::new("flagon of cider"), glyph, Stack { key: 3, count: 3 }, Worth(10))).id();
        stage.app.world_mut().get_mut::<Inventory>(trader).expect("a bag").items.push(cider);
        stage.tick();
        let rows = stage.rows();
        let text = rows.join("\n");
        let row = |name: &str| rows.iter().find(|r| r.contains(name)).unwrap_or_else(|| panic!("a row of {name}:\n{text}")).clone();
        let wares = |row: String| row.chars().skip(12).take(27).collect::<String>().trim_end().to_string();
        assert!(wares(row("3 flagons of ci")).ends_with("\u{2026} 10 each"), "the name cut short, the price for one whole:\n{text}");
        assert!(wares(row("2 files")).ends_with("10 each"), "a row with the room says each too:\n{text}");
    }

    /// A counted row the purse cannot meet says so and that its price is
    /// for one where both fit, and gives up saying it cannot be paid, which
    /// its muted tone says too, before it gives up `each`.
    #[test]
    fn a_counted_row_past_the_purse_gives_up_cannot_pay_before_each() {
        let (mut stage, trader) = stage_in(GridRect::new(0, 0, 80, 14));
        let glyph = atglyph_render::Glyph::new('[', Color::WHITE);
        let w = stage.app.world_mut();
        let pairs: Vec<Entity> = (0..2).map(|_| w.spawn((Item, Name::new("leather leggings"), glyph, crate::narrate::MassNoun, Worth(60))).id()).collect();
        let pans: Vec<Entity> = (0..2).map(|_| w.spawn((Item, Name::new("pan"), glyph, Worth(60))).id()).collect();
        w.get_mut::<Inventory>(trader).expect("a bag").items.extend(pairs.into_iter().chain(pans));
        stage.tick();
        let rows = stage.rows();
        let text = rows.join("\n");
        let wares = |name: &str| -> String {
            let row = rows.iter().find(|r| r.contains(name)).unwrap_or_else(|| panic!("a row of {name}:\n{text}"));
            row.chars().skip(2).take(37).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
        };
        assert_eq!(wares("2 pans"), "[ 2 pans cannot pay 60 each", "{text}");
        assert_eq!(wares("2 leather leggings"), "[ 2 leather leggings 60 each", "the name whole, and each:\n{text}");
    }

    /// A trade screen tall enough for a pane under its lists, the trader
    /// selling a helm and the player wearing a cap: the pane describes the
    /// row picked out on either side as the bag does, under a rule that
    /// names it, and a ware that would go on in place of what is worn says
    /// what it replaces and what changes.
    #[test]
    fn the_pane_under_the_lists_describes_the_row_picked_out_and_compares_a_ware_with_what_is_worn() {
        let mut stage = Stage::new_with(TradePanel::new(GridRect::new(0, 0, 60, 24)), |app| {
            app.add_plugins((PropsPlugin, TradePlugin));
            app.insert_resource(Seed(TEST_SEED));
            let tags = Registry::from_defs(vec![TagDef::new("scrip"), TagDef::new("armor")]).expect("two tags");
            app.insert_resource(Currency(tags.expect("scrip")));
            let mut registries = app.world_mut().resource_mut::<Registries>();
            registries.tags = tags;
            registries.slots = Registry::from_defs(vec![atglyph_rules::SlotDef::new("head")]).expect("one slot");
        })
        .screen(60, 24);
        let (armor, head) = {
            let r = stage.app.world().resource::<Registries>();
            (r.tags.expect("armor"), r.slots.expect("head"))
        };
        let glyph = atglyph_render::Glyph::new('[', Color::WHITE);
        let shape = atglyph_rules::EquipShape::in_slot(head);
        let w = stage.app.world_mut();
        let helm = w.spawn((Item, Name::new("helm"), glyph, Tagged(vec![armor]), Wearable(shape.clone()), Armor(2), Worth(20))).id();
        let cap = w.spawn((Item, Name::new("cap"), glyph, Tagged(vec![armor]), Wearable(shape.clone()), Armor(1), Worth(4))).id();
        let at = stage.at.offset(1, 0);
        let trader = w.spawn((Actor, Name::new("Odda"), Position(at), Inventory { items: vec![helm] }, Trader { buys: vec![armor], pays_pct: 50 })).id();
        let mut worn = Equipped(atglyph_rules::Equipment::with_slot_count(1));
        worn.equip(cap, &shape).expect("it fits");
        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert((Inventory { items: vec![cap] }, worn));
        let trade = stage.app.world().resource::<Verbs>().get(TRADE).expect("declared");
        stage.app.world_mut().write_message(Interacted { actor: player, target: trader, verb: trade, thing: None });
        stage.tick();
        stage.tick();

        let rows = stage.rows();
        let text = rows.join("\n");
        let pane = |rows: &[String]| -> Vec<String> {
            let rule = rows.iter().rposition(|r| r.starts_with("\u{2502} \u{2500}")).unwrap_or_else(|| panic!("a rule over the pane:\n{}", rows.join("\n")));
            rows[rule..rows.len() - 1].iter().map(|r| r.trim_start_matches("\u{2502} ").trim_end_matches('\u{2502}').trim_end().to_string()).collect()
        };
        let shown = pane(&rows);
        let rule = rows.iter().rposition(|r| r.starts_with("\u{2502} \u{2500}")).expect("a rule");
        assert_eq!(rows[rule - 1].trim_matches(|c| c == '\u{2502}' || c == ' '), "", "a blank row parts the purses from the pane:\n{text}");
        assert!(
            shown[0].starts_with("\u{2500}\u{2500} helm \u{2500}") && shown[0].chars().count() == 56,
            "a rule the width of the screen naming the ware:\n{text}"
        );
        assert_eq!(
            shown[1..],
            ["armor +2".to_string(), "goes on the head".into(), "replaces: cap".into(), "  armor 1 -> 2 (+1)".into()],
            "the pane as tall as the longest description on the screen, and no taller:\n{text}"
        );

        stage.press(KeyCode::ArrowRight);
        let rows = stage.rows();
        let pane = pane(&rows);
        assert!(pane[0].contains(" cap "), "{}", rows.join("\n"));
        assert_eq!(pane[1..3], ["armor +1".to_string(), "worn on the head".to_string()], "{}", rows.join("\n"));
        assert_eq!(pane.len(), 5, "the pane keeps its height across the sides:\n{}", rows.join("\n"));

        // A game annotates what the pane describes, as it annotates a bag's
        // row.
        stage.app.add_systems(
            Update,
            (|mut view: ResMut<TradeView>| {
                for item in view.described_mut() {
                    item.facets.push(crate::Facet::new("note", "a fine cap"));
                }
            })
            .in_set(crate::ViewSet::Annotate),
        );
        stage.tick();
        let rows = stage.rows();
        assert!(rows.iter().any(|r| r.contains("a fine cap")), "{}", rows.join("\n"));
    }

    /// Two things alike and not stacked are one counted row, priced for
    /// one: confirm deals one of them and the whole-row key the rest.
    #[test]
    fn things_alike_are_one_counted_row_and_the_keys_deal_one_or_all_of_them() {
        let (mut stage, trader) = stage();
        let glyph = atglyph_render::Glyph::new('!', Color::WHITE);
        let w = stage.app.world_mut();
        let lamps: Vec<Entity> = (0..3).map(|_| w.spawn((Item, Name::new("lamp"), glyph, Worth(2))).id()).collect();
        let dear = w.spawn((Item, Name::new("lamp"), glyph, Worth(5))).id();
        w.get_mut::<Inventory>(trader).expect("a bag").items.extend(lamps.iter().copied().chain([dear]));
        stage.tick();
        let wares = |stage: &Stage| stage.app.world().resource::<TradeView>().wares.iter().map(|r| (r.row.label.clone(), r.count, r.each)).collect::<Vec<_>>();
        assert_eq!(wares(&stage)[1..], [("3 lamps".to_string(), 3, Some(2)), ("lamp".to_string(), 1, Some(5))], "a dearer lamp is not alike");
        assert!(stage.rows().iter().any(|r| r.contains("3 lamps") && r.contains("2 each")), "{}", stage.rows().join("\n"));
        stage.press(KeyCode::ArrowDown);
        stage.press(KeyCode::Enter);
        stage.tick();
        assert_eq!(wares(&stage)[1], ("2 lamps".to_string(), 2, Some(2)), "one bought");
        stage.press(KeyCode::KeyA);
        stage.tick();
        assert_eq!(wares(&stage)[1..], [("lamp".to_string(), 1, Some(5))], "and the rest");
    }

    /// The labels of the customer's side, in order.
    fn stage_pack(stage: &Stage) -> Vec<String> {
        stage.app.world().resource::<TradeView>().pack.iter().map(|r| r.row.label.clone()).collect()
    }

    /// The trader's name heads the screen with the game's line under it,
    /// each side lists its rows with a price or the reason there is none,
    /// a short purse worded by whose it is, and each side's currency is at
    /// its foot, named by its own name.
    #[test]
    fn the_screen_draws_the_trader_the_games_line_both_sides_with_prices_and_both_purses() {
        let (mut stage, trader) = stage();
        let lamp = stage.app.world_mut().spawn((Item, Name::new("lamp"), atglyph_render::Glyph::new('!', Color::WHITE), Worth(30))).id();
        stage.app.world_mut().get_mut::<Inventory>(trader).expect("a bag").items.push(lamp);
        let tool = stage.app.world().resource::<Registries>().tags.expect("tool");
        let saw = stage.app.world_mut().spawn((Item, Name::new("saw"), atglyph_render::Glyph::new('-', Color::WHITE), Tagged(vec![tool]), Worth(40))).id();
        let player = stage.player;
        stage.app.world_mut().get_mut::<Inventory>(player).expect("a bag").items.push(saw);
        stage.app.add_systems(
            Update,
            (|mut view: ResMut<TradeView>| {
                let line = crate::Facet::new("greeting", "Look around.");
                if let Some(row) = view.trader.as_mut() {
                    row.facets.push(line);
                }
            })
            .in_set(crate::ViewSet::Annotate),
        );
        stage.tick();
        let rows = stage.rows();
        let text = rows.join("\n");
        assert_eq!(rows[5], "          ┌─ Odda ───────────────────────────────────────────────────┐", "the trader's name is the title:\n{text}");
        assert_eq!(rows[8], "          │ For sale                     Yours                       │", "{text}");
        assert_eq!(rows[6], "          │ Look around.                                             │", "the game's line under the name:\n{text}");
        assert_eq!(rows[10], "          │ - 2 files           10 each  - awl                     3 │", "a price on each row, for one of a stack:\n{text}");
        assert_eq!(rows[11], "          │ ! lamp        cannot pay 30  - saw          they lack 20 │", "a row a purse cannot meet says whose:\n{text}");
        assert_eq!(rows[15], "          │ 9 scrips                     25 scrips                   │", "each purse at its side's foot:\n{text}");
        assert!(rows[16].ends_with(" enter buy • a all • esc close ─┘"), "{text}");
    }
}
