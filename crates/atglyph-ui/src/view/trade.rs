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
//! Things alike and not stacked, three daggers the same in every way a row
//! and its description show, are one row counted, standing for each of
//! them, so a counter reads `3 daggers` once rather than a row apiece.
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
use crate::view::inventory::{ItemRow, ItemRows};

/// One thing on either side of a trade.
#[derive(Debug, Clone, PartialEq)]
pub struct TradeRow {
    /// The thing, named and counted as the bag and the container name it:
    /// the first of [`items`](Self::items).
    pub row: Row,
    /// How many: of a stack, or of the things alike the row stands for.
    pub count: u32,
    /// Every thing the row stands for: the one, or a stack, or each of
    /// several things alike and not stacked, which a screen lists once,
    /// counted, rather than a row apiece. Alike is alike in all the row
    /// and its description show: name, look, price, whether it could
    /// change hands, and everything [`described`](Self::described) says.
    /// What one changes hands for: its [`Worth`] bought from the trader,
    /// the trader's share of it sold. `None` for a thing with no worth or
    /// of a kind the trader does not buy, which has no price to show.
    pub each: Option<u32>,
    /// Why one could not change hands, if it could not.
    pub refused: Option<TradeRefusal>,
    /// The thing described as the customer's bag describes it, the
    /// customer's numbers and what putting it on would change against what
    /// the customer wears, on either side, so a ware is weighed against
    /// what it would replace before it is bought. `None` for a thing that
    /// is not an [`Item`].
    pub described: Option<ItemRow>,
    /// See [`count`](Self::count).
    pub items: Vec<Entity>,
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

    /// Every thing described on both sides, wares first, for a game
    /// annotating what it recognises in
    /// [`ViewSet::Annotate`](crate::ViewSet::Annotate), as it annotates
    /// the bag's rows: a facet pushed here reads in the pane under the
    /// lists.
    pub fn described_mut(&mut self) -> impl Iterator<Item = &mut ItemRow> {
        self.wares.iter_mut().chain(self.pack.iter_mut()).filter_map(|r| r.described.as_mut())
    }
}

/// Keeps [`TradeView`] current.
pub struct TradeViewPlugin;

impl Plugin for TradeViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TradeView>()
            .add_systems(Update, collect_trade.in_set(crate::ViewSet::Collect))
            .add_systems(Update, count_alike.in_set(crate::ViewSet::Settle));
    }

    fn finish(&self, app: &mut App) {
        app.depends_on::<crate::UiPlugin>("TradeViewPlugin");
        app.depends_on::<TradePlugin>("TradeViewPlugin");
    }
}

/// One side of a trade: who, what they carry and what they wear.
type Side = (Option<&'static Name>, Option<&'static Glyph>, &'static Inventory, Option<&'static Equipped>);

/// One carried thing, as a trade reads it.
type Piece =
    (Option<&'static Name>, Option<&'static Glyph>, Option<&'static Stack>, Option<&'static Worth>, Option<&'static Tagged>, Has<crate::narrate::MassNoun>);

/// A thing's name, and whether it is a stuff or a pair, counted as
/// written.
type Noun = (String, bool);

/// `count` of a thing called `name` as a row lists it: the name alone for
/// one, and the count before it for more, the name made plural unless it
/// is a stuff or a pair, `2 leggings`, as the log counts them.
fn listed_as((name, mass): &Noun, count: u32) -> String {
    match (mass, count) {
        (true, 2..) => format!("{count} {name}"),
        _ => atglyph_core::noun::listed(name, count),
    }
}

/// What the things on a counter are read from: each as a trade prices it,
/// the currency's name, and each as the customer's bag describes it.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Goods<'w, 's> {
    items: Query<'w, 's, Piece>,
    named: crate::view::NamedPieces<'w, 's>,
    described: ItemRows<'w, 's>,
}

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
    goods: Goods,
) {
    let Goods { items, named, described } = &goods;
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
    view.currency = crate::view::currency_name(currency.as_ref(), customer.2.items.iter().chain(&trader.2.items).copied(), named);
    view.trader = Some(Row::new(
        parties.trader,
        trader.0.map(|n| n.as_str().to_string()).unwrap_or_default(),
        trader.1.copied().unwrap_or(Glyph::new('?', Color::WHITE)),
    ));
    // Each side's rows, with the one who pays for a deal on it: the
    // customer pays for wares, the trader for what the customer sells.
    let rows = |bag: &Inventory, worn: Option<&Equipped>, deal: Deal, payer_purse: u32| -> Vec<TradeRow> {
        let listed: Vec<(Noun, TradeRow)> = bag
            .items
            .iter()
            .filter(|item| !is_currency(**item))
            .filter_map(|item| {
                let (name, glyph, stack, worth, tags, mass) = items.get(*item).ok()?;
                let count = stack.map_or(1, |s| s.count);
                let noun: Noun = (name.map(|n| n.as_str().to_string()).unwrap_or_default(), mass);
                let label = if name.is_some() { listed_as(&noun, count) } else { String::new() };
                let ware = Ware { worth: worth.copied(), tags: tags.map_or(&[], |t| t.0.as_slice()), worn: worn.is_some_and(|w| w.contains(*item)) };
                let each = terms.each(deal, ware);
                // Only what is for sale is on the trader's side; the
                // customer's side lists the whole pack, greyed where the
                // trader would refuse it, so a player learns what it takes.
                if deal == Deal::Bought && (each.is_none() || ware.worn) {
                    return None;
                }
                let row = TradeRow {
                    row: Row::new(*item, label, glyph.copied().unwrap_or(Glyph::new('?', Color::WHITE))),
                    count,
                    each,
                    refused: terms.quote(deal, ware, 1, payer_purse).err(),
                    described: described.row(parties.customer, customer.2, customer.3, *item),
                    items: vec![*item],
                };
                Some((noun, row))
            })
            .collect();
        alike_together(listed).into_iter().map(|(_, row)| row).collect()
    };
    view.wares = rows(trader.2, trader.3, Deal::Bought, customer_purse);
    view.pack = rows(customer.2, customer.3, Deal::Sold, trader_purse);
}

/// `listed`'s rows with every thing of one name together, where the first
/// of that name stands, and otherwise in the order they came: a dagger
/// sold to a trader who has two goes in beside them rather than to the
/// foot of the list, while the order a bag keeps is kept.
fn alike_together(mut listed: Vec<(Noun, TradeRow)>) -> Vec<(Noun, TradeRow)> {
    let mut names: Vec<Noun> = Vec::new();
    for (name, _) in &listed {
        if !names.contains(name) {
            names.push(name.clone());
        }
    }
    // A stable sort, so rows of one name keep their own order.
    listed.sort_by_key(|(name, _)| names.iter().position(|n| n == name));
    listed
}

/// Makes each set of things alike on either side, not stacked, one row
/// counting them, so a counter of three daggers reads `3 daggers` once
/// rather than `dagger` three times.
///
/// After the game has annotated the rows, in
/// [`ViewSet::Settle`](crate::ViewSet::Settle), so a thing the game notes,
/// the other weapon among daggers, is told apart by its note and stays a
/// row of its own, its note on it alone.
pub(crate) fn count_alike(mut view: ResMut<TradeView>, nouns: Query<(Option<&Name>, Has<crate::narrate::MassNoun>)>) {
    if view.parties.is_none() {
        return;
    }
    let noun = |row: &TradeRow| -> Noun {
        let (name, mass) = nouns.get(row.row.entity).unwrap_or((None, false));
        (name.map(|n| n.as_str().to_string()).unwrap_or_default(), mass)
    };
    let view = &mut *view;
    for side in [&mut view.wares, &mut view.pack] {
        let mut rows: Vec<(Noun, TradeRow)> = Vec::new();
        for row in side.drain(..) {
            let name = noun(&row);
            match rows.iter_mut().find(|(n, group)| *n == name && alike(group, &row)) {
                Some((name, group)) => {
                    group.items.push(row.row.entity);
                    group.count += 1;
                    if !name.0.is_empty() {
                        group.row.label = listed_as(name, group.count);
                    }
                }
                None => rows.push((name, row)),
            }
        }
        side.extend(rows.into_iter().map(|(_, row)| row));
    }
}

/// Whether `row`, one thing not stacked, is alike `group`, a row of such
/// things: in look, price, whether it could change hands, what the game
/// noted on it, and its description, the thing itself aside.
fn alike(group: &TradeRow, row: &TradeRow) -> bool {
    let unstacked = |r: &TradeRow| r.count as usize == r.items.len();
    let described = |r: &TradeRow| r.described.clone().map(|d| ItemRow { entity: Entity::PLACEHOLDER, ..d });
    let listed = |r: &TradeRow| Row { entity: Entity::PLACEHOLDER, label: String::new(), ..r.row.clone() };
    unstacked(group)
        && row.count == 1
        && listed(group) == listed(row)
        && (group.each, group.refused) == (row.each, row.refused)
        && described(group) == described(row)
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

    /// A trader wearing a jerkin and holding another in its bag lists the
    /// one for sale, not the one on its back, while the customer's worn
    /// jerkin is still listed and refused as worn.
    #[test]
    fn a_traders_worn_gear_is_not_listed() {
        let (mut stage, trader) = stage();
        let slots = atglyph_rules::Registry::from_defs(vec![atglyph_rules::SlotDef::new("body")]).expect("one slot");
        let body = atglyph_rules::EquipShape::in_slot(slots.expect("body"));
        stage.app.world_mut().resource_mut::<Registries>().slots = slots.clone();
        let w = stage.app.world_mut();
        let jerkins: Vec<Entity> = (0..4).map(|_| w.spawn((Item, Name::new("jerkin"), Worth(40))).id()).collect();
        let (worn_by_trader, spare, worn_by_customer) = (jerkins[0], jerkins[1], jerkins[2]);
        let mut theirs = atglyph_rules::Equipment::for_slots(&slots);
        theirs.equip(worn_by_trader, &body).expect("it fits");
        w.get_mut::<Inventory>(trader).expect("a bag").items.extend([worn_by_trader, spare]);
        w.entity_mut(trader).insert(Equipped(theirs));
        let mut mine = atglyph_rules::Equipment::for_slots(&slots);
        mine.equip(worn_by_customer, &body).expect("it fits");
        let player = stage.player;
        stage.app.world_mut().get_mut::<Inventory>(player).expect("a bag").items.push(worn_by_customer);
        stage.app.world_mut().entity_mut(player).insert(Equipped(mine));
        open(&mut stage, trader);
        let view = stage.app.world().resource::<TradeView>();
        let jerkins: Vec<&TradeRow> = view.wares.iter().filter(|r| r.row.label.contains("jerkin")).collect();
        assert_eq!(jerkins.len(), 1, "one jerkin for sale: {:?}", view.wares.iter().map(|r| &r.row.label).collect::<Vec<_>>());
        assert_eq!((jerkins[0].count, jerkins[0].items.as_slice()), (1, [spare].as_slice()), "the spare, not the one worn");
        let mine = view.pack.iter().find(|r| r.row.label == "jerkin").expect("the customer's worn jerkin is still listed");
        assert_eq!(mine.refused, Some(TradeRefusal::Worn), "and refused");
    }

    /// A thing sold to a trader who already has one of its name is listed
    /// beside it, not at the foot of the list, and the rest keep the order
    /// the bag keeps.
    #[test]
    fn a_thing_sold_goes_in_beside_the_trader_s_own_of_its_name() {
        let (mut stage, trader) = stage();
        let w = stage.app.world_mut();
        let first = w.spawn((Item, Name::new("awl"), Worth(6))).id();
        let comb = w.spawn((Item, Name::new("comb"), Worth(2))).id();
        let sold = w.spawn((Item, Name::new("awl"), Worth(8))).id();
        w.get_mut::<Inventory>(trader).expect("a bag").items.extend([first, comb, sold]);
        open(&mut stage, trader);
        let view = stage.app.world().resource::<TradeView>();
        let wares: Vec<&str> = view.wares.iter().map(|r| r.row.label.as_str()).collect();
        assert_eq!(wares, vec!["2 files", "lamp", "awl", "awl", "comb"]);
    }

    /// Things alike in every way the row and its description show are one
    /// row, counted, standing for each of them; a thing worn, of another
    /// worth or a stack stays a row of its own.
    #[test]
    fn things_alike_are_one_row_standing_for_each_and_things_unlike_stay_apart() {
        let (mut stage, trader) = stage();
        let w = stage.app.world_mut();
        let combs: Vec<Entity> = (0..2).map(|_| w.spawn((Item, Name::new("comb"), Worth(2))).id()).collect();
        let dear = w.spawn((Item, Name::new("comb"), Worth(3))).id();
        w.get_mut::<Inventory>(trader).expect("a bag").items.extend(combs.iter().copied().chain([dear]));
        open(&mut stage, trader);
        let view = stage.app.world().resource::<TradeView>();
        let rows: Vec<(&str, u32, usize)> = view.wares.iter().map(|r| (r.row.label.as_str(), r.count, r.items.len())).collect();
        assert_eq!(rows, vec![("2 files", 2, 1), ("lamp", 1, 1), ("2 combs", 2, 2), ("comb", 1, 1)]);
        assert_eq!(view.wares[2].items, combs, "the row stands for both");
        assert_eq!(view.wares[2].row.entity, combs[0], "and is the first of them");
    }

    /// A thing the game notes, in its row or in what describes it, is
    /// never counted in with things it does not note: two combs, one of
    /// them noted, are two rows.
    #[test]
    fn a_thing_the_game_notes_is_not_counted_in_with_things_alike_it_does_not() {
        let (mut stage, trader) = stage();
        let w = stage.app.world_mut();
        let combs: Vec<Entity> = (0..3).map(|_| w.spawn((Item, Name::new("comb"), Worth(2))).id()).collect();
        w.get_mut::<Inventory>(trader).expect("a bag").items.extend(combs.iter().copied());
        let noted = combs[1];
        stage.app.add_systems(
            Update,
            (move |mut view: ResMut<TradeView>| {
                for item in view.described_mut().filter(|d| d.entity == noted) {
                    item.facets.push(crate::Facet::new("note", "the good one"));
                }
            })
            .in_set(crate::ViewSet::Annotate),
        );
        open(&mut stage, trader);
        let view = stage.app.world().resource::<TradeView>();
        let rows: Vec<(&str, Vec<Entity>)> = view.wares.iter().map(|r| (r.row.label.as_str(), r.items.clone())).collect();
        assert_eq!(rows[2..], [("2 combs", vec![combs[0], combs[2]]), ("comb", vec![noted])], "the noted comb on its own");
    }

    /// A stuff or a pair counted is counted as written, `2 leggings`, as
    /// the log says it, never with a second plural on it.
    #[test]
    fn things_alike_named_as_a_stuff_are_counted_as_written() {
        let (mut stage, trader) = stage();
        let w = stage.app.world_mut();
        let pairs: Vec<Entity> = (0..2).map(|_| w.spawn((Item, Name::new("leggings"), crate::narrate::MassNoun, Worth(2))).id()).collect();
        w.get_mut::<Inventory>(trader).expect("a bag").items.extend(pairs);
        open(&mut stage, trader);
        let view = stage.app.world().resource::<TradeView>();
        assert_eq!(view.wares.iter().map(|r| r.row.label.as_str()).collect::<Vec<_>>(), vec!["2 files", "lamp", "2 leggings"]);
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
