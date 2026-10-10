//! What a trader sells and what it would pay, and the keys that deal.
//!
//! A modal the engine runs end to end, like the container screen: it opens
//! itself when the engine opens a trade, the direction keys walk the rows
//! and cross between the two sides, the cursors' confirm key deals one of
//! the row picked out, one key deals the whole of it, and the cursors'
//! close key puts it away and ends the trade. A game binds nothing.
//!
//! The trader's wares are on the left and the customer's pack on the
//! right, so the key that takes from the container screen buys from the
//! left and the same key sells from the right: one key that moves the
//! thing picked out across, whichever side it is on. Dealing does not
//! close the screen, since a deal takes no turn and a shop is usually
//! more than one; it closes itself when the trade ends, as the container
//! screen does when its container goes out of reach.
//!
//! Every word it draws is the game's or a [`TradeWords`] default the game
//! may replace: the title is the trader's `Name`, the line under it is
//! the game's facet, and the currency is named by the currency's own
//! `Name`.

use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_bevy::{EngineSet, PresentSet};
use rl_core::{Direction, Rect};
use rl_render::{Cell, Terminal};

use crate::controls::{Chord, ControlInput, key_name};
use crate::modal::{AddModal, ModalId, Modals};
use crate::panel::{clear, clip, frame, section};
use crate::tone::{Palette, Tones};
use crate::view::trade::{TradeRow, TradeView, TradeViewPlugin};

/// The name the trade screen's modal is declared under.
pub const TRADE_MODAL: &str = "trade";

/// The keys the trade screen answers to, beside the direction keys that
/// walk the rows and cross the sides and the cursors' keys that deal one
/// and close.
///
/// A default, not a rule: a game that wants another key inserts its own
/// `TradeKeys`. `a` because it is the container screen's key for all of
/// it, and on this screen all of it is the whole of a stack.
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
pub struct TradeLayout {
    /// The terminal cells it occupies.
    pub rect: Rect,
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
    pub fn selected(&self) -> usize {
        match self.side {
            TradeSide::Wares => self.wares,
            TradeSide::Pack => self.pack,
        }
    }

    /// Moves the cursor `by` rows over `rows` of them on the side picked
    /// out, wrapping at either end.
    pub fn move_by(&mut self, by: i32, rows: usize) {
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
    pub fn new(rect: Rect) -> Self {
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
        rl_bevy::depends_on::<crate::UiPlugin>(app, "TradePanel");
        rl_bevy::depends_on::<TradePlugin>(app, "TradePanel");
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
pub fn follow_trade(mut trading: ResMut<Trading>, mut menu: ResMut<TradeMenu>, mut modals: ResMut<Modals>) {
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
/// answered with [`TradeRefused`], for a game that wants a line saying
/// why, over the row the screen already greys.
pub fn trade_keys(keys: ControlInput, mut screen: Counter, mut buys: MessageWriter<Buy>, mut sells: MessageWriter<Sell>) {
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
    let count = if whole { row.count } else { 1 };
    let (customer, trader, item) = (parties.customer, parties.trader, row.row.entity);
    match menu.side {
        TradeSide::Wares => {
            buys.write(Buy { customer, trader, item, count });
        }
        TradeSide::Pack => {
            sells.write(Sell { customer, trader, item, count });
        }
    }
}

/// Everything the trade screen draws from.
#[derive(bevy::ecs::system::SystemParam)]
pub struct ShopScreen<'w> {
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
pub fn draw_trade(mut terminal: ResMut<Terminal>, mut menu: ResMut<TradeMenu>, screen: ShopScreen) {
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
    let inner = Rect::new(rect.x + 2, rect.y + 1, rect.width - 4, rect.height - 2);
    let surface = palette.get(Tones::SURFACE);

    // The game's line, in its own tones, one facet after another.
    let mut x = inner.x;
    for facet in view.trader.iter().flat_map(|t| &t.facets) {
        let room = (inner.right() - x).max(0) as usize;
        let text = clip(&facet.text, room);
        terminal.print_on(x, inner.y, &text, palette.get(facet.tone), surface);
        x += text.chars().count() as i32 + 1;
    }

    let gap = 2;
    let half = (inner.width - gap) / 2;
    let lists = Rect::new(inner.x, inner.y + 2, half, inner.height - 3);
    let wares = lists;
    let pack = Rect::new(inner.x + half + gap, lists.y, inner.width - half - gap, lists.height);
    let selected = |side| (menu.side == side).then_some(menu.selected());
    draw_side(&mut terminal, wares, (&words.wares, Deal::Bought), &view.wares, selected(TradeSide::Wares), words, palette);
    draw_side(&mut terminal, pack, (&words.pack, Deal::Sold), &view.pack, selected(TradeSide::Pack), words, palette);

    // Each purse under its own side, so whose is short reads off the
    // screen as well as off the row.
    let purse = |amount: u32| crate::view::sum_of(&view.currency, amount);
    for (side, amount) in [(wares, view.trader_purse), (pack, view.purse)] {
        terminal.print_on(side.x, inner.bottom() - 1, &clip(&purse(amount), side.width.max(0) as usize), palette.get(Tones::NOTICE), surface);
    }
}

/// One side of the trade under its heading, and the deal its rows are
/// struck by, the row picked out lit when the cursor is on this side.
fn draw_side(
    terminal: &mut Terminal,
    rect: Rect,
    (heading, deal): (&str, Deal),
    rows: &[TradeRow],
    selected: Option<usize>,
    words: &TradeWords,
    palette: &Palette,
) {
    section(terminal, rect, rect.y, heading, None, palette);
    let list = Rect::new(rect.x, rect.y + 2, rect.width, rect.height - 2);
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
        terminal.fill(Rect::new(list.x, y, list.width, 1), Cell::new(' ', fg).on(bg));
        // What one changes hands for, or why it cannot: a short purse
        // still shows the price, since how short is the useful part. A
        // row of more than one says the price is for one, where it fits.
        let price = match (row.refused, row.each) {
            (None, Some(each)) => each.to_string(),
            (Some(TradeRefusal::Short), Some(each)) => format!("{} {each}", words.refusal(TradeRefusal::Short, deal)),
            (Some(why), _) => words.refusal(why, deal).to_string(),
            (None, None) => String::new(),
        };
        let room = (list.width / 2).max(0) as usize;
        let per_one = format!("{price} {}", words.each);
        let price = match row.count > 1 && row.each.is_some() && matches!(row.refused, None | Some(TradeRefusal::Short)) && per_one.chars().count() <= room {
            true => per_one,
            false => clip(&price, room),
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
    use rl_bevy::testing::TEST_SEED;
    use rl_rules::{Registry, TagDef};

    /// A trader beside the player with two files worth 10, buying tools at
    /// half from a till of 9; the player with 25 in scrip and an awl worth
    /// 6. The screen is added and the trade opened.
    fn stage() -> (Stage, Entity) {
        let mut stage = Stage::new_with(TradePanel::new(Rect::new(10, 5, 60, 12)), |app| {
            app.add_plugins((PropsPlugin, TradePlugin));
            app.insert_resource(Seed(TEST_SEED));
            let tags = Registry::from_defs(vec![TagDef::new("scrip"), TagDef::new("tool")]).expect("two tags");
            app.insert_resource(Currency(tags.expect("scrip")));
            app.world_mut().resource_mut::<Registries>().tags = tags;
        });
        let tags = stage.app.world().resource::<Registries>().tags.clone();
        let (scrip, tool) = (tags.expect("scrip"), tags.expect("tool"));
        let glyph = rl_render::Glyph::new('-', Color::WHITE);
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
        stage.app.world_mut().write_message(Interacted { actor: player, target: trader, verb: trade });
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

    /// The trader's name heads the screen with the game's line under it,
    /// each side lists its rows with a price or the reason there is none,
    /// a short purse worded by whose it is, and each side's currency is at
    /// its foot, named by its own name.
    #[test]
    fn the_screen_draws_the_trader_the_games_line_both_sides_with_prices_and_both_purses() {
        let (mut stage, trader) = stage();
        let lamp = stage.app.world_mut().spawn((Item, Name::new("lamp"), rl_render::Glyph::new('!', Color::WHITE), Worth(30))).id();
        stage.app.world_mut().get_mut::<Inventory>(trader).expect("a bag").items.push(lamp);
        let tool = stage.app.world().resource::<Registries>().tags.expect("tool");
        let saw = stage.app.world_mut().spawn((Item, Name::new("saw"), rl_render::Glyph::new('-', Color::WHITE), Tagged(vec![tool]), Worth(40))).id();
        let player = stage.player;
        stage.app.world_mut().get_mut::<Inventory>(player).expect("a bag").items.push(saw);
        stage.app.add_systems(
            Update,
            (|mut view: ResMut<TradeView>, mut facets: ResMut<crate::Facets>| {
                let line = facets.facet("greeting", "Look around.");
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
