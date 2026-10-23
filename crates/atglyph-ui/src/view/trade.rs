//! What a trader sells and what it would pay, side by side.
//!
//! The view holds the open trade as plain rows: the trader's wares with
//! what one costs, the customer's pack with what the trader would pay for
//! one, and the currency each side carries. Which trade is open is the
//! engine's, in [`Trading`]; the view only reads it.
//!
//! Rows are the shared [`Row`], the way the container's and the nearby
//! rail's are, so a game that annotates one annotates all of them the same
//! way. The trader is a [`Row`] too, and the line a screen shows under the
//! trader's name is whatever the game pushes onto it as a [`Facet`] in
//! [`ViewSet::Annotate`](crate::ViewSet::Annotate): a greeting is a game's
//! words, and the engine has none to offer.
//!
//! A row that a deal would be refused on says so, by the same rule the
//! engine refuses it by, [`Trader::quote`], priced by [`Trader::each`] and
//! with purses counted by [`Currency::count`], so the screen and the trade
//! never disagree. What the refusal is called is the presenter's.
//!
//! [`Facet`]: crate::facet::Facet

use atglyph_bevy::prelude::*;
use atglyph_render::Glyph;
use bevy::prelude::*;

use crate::view::Row;

/// One thing on either side of a trade.
#[derive(Debug, Clone, PartialEq)]
pub struct TradeRow {
    /// The thing, named and counted as the bag and the container name it.
    pub row: Row,
    /// How many, for a stack; one otherwise.
    pub count: u32,
    /// What one changes hands for: its [`Worth`] bought from the trader,
    /// the trader's share of it sold. `None` for a thing with no worth or
    /// of a kind the trader does not buy, which has no price to show.
    pub each: Option<u32>,
    /// Why one could not change hands, if it could not.
    pub refused: Option<TradeRefusal>,
}

impl TradeRow {
    /// What the whole row changes hands for, when it has a price.
    pub fn price(&self) -> Option<u32> {
        self.each.map(|each| each.saturating_mul(self.count))
    }

    /// Whether one could change hands now.
    pub fn open(&self) -> bool {
        self.refused.is_none()
    }
}

/// The open trade, as a screen reads it.
#[derive(Resource, Debug, Default)]
pub struct TradeView {
    /// Who is trading, while a trade is open.
    pub parties: Option<Parties>,
    /// The trader, named from its `Name`, carrying whatever the game
    /// pushed onto it: the line under its name.
    pub trader: Option<Row>,
    /// What the trader sells: what it carries that has a price and is not
    /// worn, in the order it carries them. Its currency, what it wears and
    /// what it puts no price on are not on the counter, so a trader's own
    /// gear is never offered and never greyed.
    pub wares: Vec<TradeRow>,
    /// What the customer carries but its currency, in the same order,
    /// priced at what the trader would pay.
    pub pack: Vec<TradeRow>,
    /// How much currency the customer carries.
    pub purse: u32,
    /// How much currency the trader carries, which is what it can pay for
    /// what the customer sells.
    pub trader_purse: u32,
    /// What one piece of the currency is called, from the `Name` of a
    /// piece either side carries, else of any piece in play; empty when
    /// no piece has a name. Said with [`sum_of`](crate::view::sum_of).
    pub currency: String,
}

impl TradeView {
    /// Every row on both sides, wares first, for a game annotating what it
    /// recognises.
    pub fn rows_mut(&mut self) -> impl Iterator<Item = &mut Row> {
        self.wares.iter_mut().chain(self.pack.iter_mut()).map(|r| &mut r.row)
    }
}

/// Keeps [`TradeView`] current.
pub struct TradeViewPlugin;

impl Plugin for TradeViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TradeView>().add_systems(Update, collect_trade.in_set(crate::ViewSet::Collect));
    }

    fn finish(&self, app: &mut App) {
        atglyph_bevy::depends_on::<crate::UiPlugin>(app, "TradeViewPlugin");
        atglyph_bevy::depends_on::<TradePlugin>(app, "TradeViewPlugin");
    }
}

/// One side of a trade: who, what they carry and what they wear.
type Side = (Option<&'static Name>, Option<&'static Glyph>, &'static Inventory, Option<&'static Equipped>);

/// One carried thing, as a trade reads it.
type Piece = (Option<&'static Name>, Option<&'static Glyph>, Option<&'static Stack>, Option<&'static Worth>, Option<&'static Tagged>);

/// Fills [`TradeView`] from the open trade, if there is one.
///
/// A row's refusal is what a deal for one would meet, since one is what a
/// screen most often asks for; a deal for a whole stack the customer can
/// afford only part of is refused by the engine when it is asked.
pub(crate) fn collect_trade(
    mut view: ResMut<TradeView>,
    trading: Res<Trading>,
    currency: Option<Res<Currency>>,
    sides: Query<Side>,
    traders: Query<&Trader>,
    items: Query<Piece>,
    named: crate::view::NamedPieces,
) {
    view.parties = None;
    view.trader = None;
    view.wares.clear();
    view.pack.clear();
    view.purse = 0;
    view.trader_purse = 0;
    view.currency.clear();
    let Some(parties) = trading.open() else { return };
    let (Ok(customer), Ok(trader), Ok(terms)) = (sides.get(parties.customer), sides.get(parties.trader), traders.get(parties.trader)) else { return };
    // Counted and told apart by the trade slice's own rule, not a copy of it.
    let currency = currency.as_deref().copied();
    let piece = |item: Entity| items.get(item).map_or((None, None), |p| (p.4, p.2));
    let is_currency = |item: Entity| currency.is_some_and(|c| c.is(piece(item).0));
    let purse = |bag: &Inventory| currency.map_or(0, |c| c.count(bag, piece));
    let (customer_purse, trader_purse) = (purse(customer.2), purse(trader.2));
    view.parties = Some(parties);
    view.purse = customer_purse;
    view.trader_purse = trader_purse;
    view.currency = crate::view::currency_name(currency.as_ref(), customer.2.items.iter().chain(&trader.2.items).copied(), &named);
    view.trader = Some(Row::new(
        parties.trader,
        trader.0.map(|n| n.as_str().to_string()).unwrap_or_default(),
        trader.1.copied().unwrap_or(Glyph::new('?', Color::WHITE)),
    ));
    // Each side's rows, with the one who pays for a deal on it: the
    // customer pays for wares, the trader for what the customer sells.
    let rows = |bag: &Inventory, worn: Option<&Equipped>, deal: Deal, payer_purse: u32| -> Vec<TradeRow> {
        bag.items
            .iter()
            .filter(|item| !is_currency(**item))
            .filter_map(|item| {
                let (name, glyph, stack, worth, tags) = items.get(*item).ok()?;
                let count = stack.map_or(1, |s| s.count);
                let label = name.map(|n| atglyph_core::noun::listed(n.as_str(), count)).unwrap_or_default();
                let ware = Ware { worth: worth.copied(), tags: tags.map_or(&[], |t| t.0.as_slice()), worn: worn.is_some_and(|w| w.contains(*item)) };
                let each = terms.each(deal, ware);
                // Only what is for sale is on the trader's side; the
                // customer's side lists the whole pack, greyed where the
                // trader would refuse it, so a player learns what it takes.
                if deal == Deal::Bought && (each.is_none() || ware.worn) {
                    return None;
                }
                Some(TradeRow {
                    row: Row::new(*item, label, glyph.copied().unwrap_or(Glyph::new('?', Color::WHITE))),
                    count,
                    each,
                    refused: terms.quote(deal, ware, 1, payer_purse).err(),
                })
            })
            .collect()
    };
    view.wares = rows(trader.2, trader.3, Deal::Bought, customer_purse);
    view.pack = rows(customer.2, customer.3, Deal::Sold, trader_purse);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use atglyph_bevy::testing::TEST_SEED;
    use atglyph_rules::{Registry, TagDef};

    /// A trader beside the player with a till of 4, two tools worth 10 and
    /// a trinket worth 30, buying tools at half; the player with 12 in
    /// scrip, a tool worth 6 and a pebble worth nothing.
    fn stage() -> (Stage, Entity) {
        let mut stage = Stage::new_with(TradeViewPlugin, |app| {
            app.add_plugins((PropsPlugin, TradePlugin));
            app.insert_resource(Seed(TEST_SEED));
            let tags = Registry::from_defs(vec![TagDef::new("scrip"), TagDef::new("tool"), TagDef::new("trinket")]).expect("three tags");
            app.insert_resource(Currency(tags.expect("scrip")));
            app.world_mut().resource_mut::<Registries>().tags = tags;
        });
        let tags = stage.app.world().resource::<Registries>().tags.clone();
        let (scrip, tool, trinket) = (tags.expect("scrip"), tags.expect("tool"), tags.expect("trinket"));
        let w = stage.app.world_mut();
        let till = w.spawn((Item, Name::new("scrip"), Tagged(vec![scrip]), Stack { key: 1, count: 4 })).id();
        let tools = w.spawn((Item, Name::new("file"), Tagged(vec![tool]), Stack { key: 2, count: 2 }, Worth(10))).id();
        let lamp = w.spawn((Item, Name::new("lamp"), Tagged(vec![trinket]), Worth(30))).id();
        let purse = w.spawn((Item, Name::new("scrip"), Tagged(vec![scrip]), Stack { key: 1, count: 12 })).id();
        let mine = w.spawn((Item, Name::new("awl"), Tagged(vec![tool]), Worth(6))).id();
        let pebble = w.spawn((Item, Name::new("pebble"))).id();
        let at = stage.at.offset(1, 0);
        let trader =
            w.spawn((Actor, Name::new("Odda"), Position(at), Inventory { items: vec![till, tools, lamp] }, Trader { buys: vec![tool], pays_pct: 50 })).id();
        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![purse, mine, pebble] });
        (stage, trader)
    }

    fn open(stage: &mut Stage, trader: Entity) {
        let player = stage.player;
        let trade = stage.app.world().resource::<Verbs>().get(TRADE).expect("declared");
        stage.app.world_mut().write_message(Interacted { actor: player, target: trader, verb: trade, thing: None });
        stage.tick();
    }

    /// Each side lists what it carries but its currency, priced per one, and a
    /// row the engine would refuse says which refusal it would meet.
    #[test]
    fn the_view_lists_both_sides_with_their_prices_and_greys_a_refused_row_with_its_reason() {
        let (mut stage, trader) = stage();
        stage.tick();
        assert!(stage.app.world().resource::<TradeView>().parties.is_none(), "no trade is open, so nothing is listed");

        open(&mut stage, trader);
        let view = stage.app.world().resource::<TradeView>();
        assert_eq!(view.trader.as_ref().map(|r| r.label.as_str()), Some("Odda"));
        let side = |rows: &[TradeRow]| rows.iter().map(|r| (r.row.label.clone(), r.each, r.refused)).collect::<Vec<_>>();
        assert_eq!(
            side(&view.wares),
            vec![("2 files".to_string(), Some(10), None), ("lamp".to_string(), Some(30), Some(TradeRefusal::Short))],
            "the currency is not for sale, a file is within the purse and the lamp is not"
        );
        assert_eq!(view.wares[0].price(), Some(20), "the row's price is each times the count");
        assert_eq!(
            side(&view.pack),
            vec![("awl".to_string(), Some(3), None), ("pebble".to_string(), None, Some(TradeRefusal::NotBought))],
            "an awl fetches half its worth, and a pebble is of a kind not bought"
        );
        assert_eq!((view.purse, view.currency.as_str()), (12, "scrip"), "the customer's currency, named by its own name");
        assert_eq!(view.trader_purse, 4, "and the trader's beside it, what it can pay with");
    }

    /// The trader's side is only what is for sale: a thing it puts no price
    /// on and a thing it wears stay off the counter, while the customer's
    /// side still lists the whole pack.
    #[test]
    fn the_wares_are_only_what_the_trader_prices_and_does_not_wear() {
        let (mut stage, trader) = stage();
        let w = stage.app.world_mut();
        let rag = w.spawn((Item, Name::new("rag"))).id();
        let coat = w.spawn((Item, Name::new("coat"), Worth(40))).id();
        let slots = atglyph_rules::Registry::from_defs(vec![atglyph_rules::SlotDef::new("body")]).expect("one slot");
        let mut worn = atglyph_rules::Equipment::for_slots(&slots);
        worn.equip(coat, &atglyph_rules::EquipShape::in_slot(slots.expect("body"))).expect("it fits");
        w.get_mut::<Inventory>(trader).expect("a bag").items.extend([rag, coat]);
        w.entity_mut(trader).insert(Equipped(worn));
        open(&mut stage, trader);
        let view = stage.app.world().resource::<TradeView>();
        assert_eq!(view.wares.iter().map(|r| r.row.label.as_str()).collect::<Vec<_>>(), vec!["2 files", "lamp"], "no rag, no coat");
        assert_eq!(view.pack.iter().map(|r| r.row.label.as_str()).collect::<Vec<_>>(), vec!["awl", "pebble"], "the pack is listed whole");
    }

    /// The line under the trader's name is whatever the game pushes onto
    /// the trader's row, in the phase every annotation is pushed in.
    #[test]
    fn the_games_line_comes_through_a_facet_on_the_traders_row() {
        let (mut stage, trader) = stage();
        stage.app.add_systems(
            Update,
            (|mut view: ResMut<TradeView>| {
                let line = crate::Facet::new("greeting", "What will it be?");
                if let Some(row) = view.trader.as_mut() {
                    row.facets.push(line);
                }
            })
            .in_set(crate::ViewSet::Annotate),
        );
        open(&mut stage, trader);
        let view = stage.app.world().resource::<TradeView>();
        let row = view.trader.as_ref().expect("a trade is open");
        assert_eq!(row.facets.iter().map(|f| f.text.as_str()).collect::<Vec<_>>(), vec!["What will it be?"]);
    }
}
