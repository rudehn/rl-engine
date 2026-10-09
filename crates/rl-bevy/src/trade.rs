//! Trade between two actors: one sells what it carries at its worth and
//! buys what it names at a share, paid in a currency the game tags.
//!
//! Opt-in by adding [`TradePlugin`]. A trader is an actor carrying
//! [`Trader`] and offering the [`TRADE`] verb through its
//! [`Offers`](crate::props::Offers); taking that offer up is the turn, and
//! opens a trade the player's screen reads from [`Trading`]. Inside it,
//! a [`Buy`] or a [`Sell`] is resolved at once and takes no turn, since a
//! trade is a screen and not a place, and nothing that could act on the
//! time is in it. Each is answered with [`Traded`] or [`TradeRefused`].
//!
//! What is for sale is what the trader carries: its [`Inventory`] is its
//! stock, so the stock is saved with the run the way any actor's pack is,
//! and restocking is the game filling it. The currency is items carrying
//! the tag the game names in [`Currency`], counted by their stacks and
//! handed over as real stacks, split off and merged in the way a pickup
//! merges, so what is paid stays a pile in the pack that can be dropped,
//! stolen and counted as it is everywhere else. Nothing is made and
//! nothing is destroyed: what one side pays, the other now carries.
//!
//! The engine never decides a price. [`Worth`] is the game's, because only
//! the game knows what a level, a kind or a sign of something wrong is
//! worth, and a thing without one is neither sold nor bought.

use bevy::ecs::message::MessageCursor;
use bevy::prelude::*;
use rl_rules::TagId;

use crate::combat::Dead;
use crate::components::{Player, Position};
use crate::items::{Equipped, Inventory, Stack, Tagged};
use crate::places::{MapId, OnMap};
use crate::props::{Interacted, Verbs};

/// The verb a trader offers, which [`TradePlugin`] declares.
///
/// A game puts `Offering::new(verbs.get(TRADE)?, time)` on whoever trades;
/// the offer's time is what opening the trade costs.
pub const TRADE: &str = "trade";

/// What one of a thing is worth, in units of the [`Currency`].
///
/// The game's, kept current on each item, since what a thing is worth
/// turns on what the game knows of it. For a stack it is the worth of one.
/// Not saved by the engine: the game puts it on as it makes the thing and
/// keeps it current as it does [`Bestows`](crate::items::Bestows), so a
/// continued run has it back from the game's own pass rather than from a
/// number that may have gone stale while the run was away.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Worth(pub u32);

/// An actor that trades: it sells whatever it carries that has a
/// [`Worth`], and buys what carries one of the tags it names.
///
/// Not saved by the engine, as [`Offers`](crate::props::Offers) is not:
/// the game puts it back on with whatever it saves the actor as.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct Trader {
    /// The tags of what it buys. Empty, it buys nothing and only sells.
    pub buys: Vec<TagId>,
    /// The share of a thing's worth it pays, in hundredths.
    pub pays_pct: u32,
}

impl Trader {
    /// Whether it buys a thing tagged `tags`.
    pub fn buys(&self, tags: &[TagId]) -> bool {
        tags.iter().any(|t| self.buys.contains(t))
    }

    /// What it pays for one of a thing worth `worth`, rounded down.
    ///
    /// Per one rather than per handful, so selling a stack whole pays what
    /// selling it one at a time would.
    pub fn pays(&self, worth: u32) -> u32 {
        (u64::from(worth) * u64::from(self.pays_pct) / 100).min(u64::from(u32::MAX)) as u32
    }

    /// What `count` of `ware` would come to going the way `deal` says, or
    /// why it would be refused, when whoever pays carries `purse`.
    ///
    /// The rule [`resolve_trades`] strikes every deal by, public so a
    /// screen greys the rows the resolver would refuse and says why
    /// without a second copy of the rule that could drift from this one.
    pub fn quote(&self, deal: Deal, ware: Ware, count: u32, purse: u32) -> Result<u32, TradeRefusal> {
        if ware.worn {
            return Err(TradeRefusal::Worn);
        }
        if deal == Deal::Sold && !self.buys(ware.tags) {
            return Err(TradeRefusal::NotBought);
        }
        let Some(Worth(worth)) = ware.worth else { return Err(TradeRefusal::NoWorth) };
        let each = match deal {
            Deal::Bought => worth,
            Deal::Sold => self.pays(worth),
        };
        let price = each.saturating_mul(count);
        if purse < price { Err(TradeRefusal::Short) } else { Ok(price) }
    }
}

/// What a deal needs to know of the thing changing hands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ware<'a> {
    /// Its [`Worth`], if it has one.
    pub worth: Option<Worth>,
    /// Its tags, which say whether a trader buys it.
    pub tags: &'a [TagId],
    /// Whether whoever holds it wears it.
    pub worn: bool,
}

/// The tag every piece of the currency carries.
///
/// A tag rather than an item kind, so the engine counts what is paid
/// without knowing what the game's items are.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Currency(pub TagId);

impl Currency {
    /// Whether a thing carrying `tags` is a piece of it.
    pub fn is(&self, tags: Option<&Tagged>) -> bool {
        tags.is_some_and(|t| t.contains(&self.0))
    }

    /// How much of it `bag` holds: a stack counts its count, a single
    /// piece one. Each carried thing is read through `piece`, which gives
    /// its tags and its stack.
    ///
    /// Through a reader rather than a query of its own, so the trade, the
    /// offer gate and a screen each count with whatever they already hold,
    /// the world or their own query, and still count by this one rule.
    pub fn count<'a>(&self, bag: &Inventory, piece: impl Fn(Entity) -> (Option<&'a Tagged>, Option<&'a Stack>)) -> u32 {
        bag.items.iter().map(|i| piece(*i)).filter(|(tags, _)| self.is(*tags)).map(|(_, stack)| stack.map_or(1, |s| s.count)).fold(0u32, u32::saturating_add)
    }
}

/// The two sides of a trade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parties {
    /// Who came to trade: the one who buys from the trader and sells to it.
    pub customer: Entity,
    /// Who carries the [`Trader`].
    pub trader: Entity,
}

/// The trade a screen shows, if one is open.
///
/// Opened when the player takes up a trader's [`TRADE`] offer, and closed
/// when either side is out of the other's reach, dead or gone. The
/// player's only: a trade is a screen, and a mind that trades writes
/// [`Buy`] and [`Sell`] itself. A screen closes it with
/// [`close`](Self::close). Not saved, so a continued run opens with no
/// trade on screen.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Trading(Option<Parties>);

impl Trading {
    /// Who is trading, if a trade is open.
    pub fn open(&self) -> Option<Parties> {
        self.0
    }

    /// Ends the trade, as a screen does when it is put away.
    pub fn close(&mut self) {
        self.0 = None;
    }
}

/// The customer buys `count` of `item` from the trader, at its [`Worth`]
/// each.
///
/// Dropped without a word when it could not be meant: a trader that is
/// not one, either side out of reach, a thing the trader does not carry,
/// or a count of nought. A count past what a stack holds buys the stack.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Buy {
    /// Who buys.
    pub customer: Entity,
    /// Who sells.
    pub trader: Entity,
    /// What, as the trader carries it.
    pub item: Entity,
    /// How many of a stack; one for anything else.
    pub count: u32,
}

/// The customer sells `count` of `item` to the trader, for the trader's
/// share of its [`Worth`] each.
///
/// Dropped without a word when it could not be meant, as a [`Buy`] is.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sell {
    /// Who sells.
    pub customer: Entity,
    /// Who buys.
    pub trader: Entity,
    /// What, as the customer carries it.
    pub item: Entity,
    /// How many of a stack; one for anything else.
    pub count: u32,
}

/// Which way a deal went, from the customer's side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deal {
    /// The customer bought from the trader.
    Bought,
    /// The customer sold to the trader.
    Sold,
}

/// A deal done, for the log and for whatever the game makes of it, such as
/// knowing the kind of a thing bought.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Traded {
    /// Who came to trade.
    pub customer: Entity,
    /// Who carries the [`Trader`].
    pub trader: Entity,
    /// Which way it went.
    pub deal: Deal,
    /// What changed hands, as its new holder now carries it: the stack it
    /// was merged into, the part split off a stack, or the thing itself.
    pub item: Entity,
    /// How many.
    pub count: u32,
    /// What was paid for all of them.
    pub price: u32,
}

/// Why a deal was refused, for a screen to say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeRefusal {
    /// Whoever pays carries less currency than the price: the customer
    /// buying, the trader buying back.
    Short,
    /// The trader does not buy a thing of this kind.
    NotBought,
    /// It has no [`Worth`], so it is neither sold nor bought.
    NoWorth,
    /// It is worn, and comes off before it changes hands.
    Worn,
}

/// A deal refused, with the reason, and nothing changed hands.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradeRefused {
    /// Who came to trade.
    pub customer: Entity,
    /// Who carries the [`Trader`].
    pub trader: Entity,
    /// Which way it would have gone.
    pub deal: Deal,
    /// What was offered or asked for.
    pub item: Entity,
    /// Why not.
    pub why: TradeRefusal,
}

/// Where an actor stands and on which map, if it stands anywhere.
fn whereabouts(world: &World, who: Entity) -> Option<(rl_core::Point, MapId)> {
    let at = world.get::<Position>(who)?.0;
    Some((at, world.get::<OnMap>(who).map(|m| m.0).unwrap_or(MapId::SURFACE)))
}

/// Whether two actors can reach each other to trade: on one map, on one
/// cell or beside each other, the reach an offer is made within.
fn within_reach(a: Option<(rl_core::Point, MapId)>, b: Option<(rl_core::Point, MapId)>) -> bool {
    match (a, b) {
        (Some((a, on)), Some((b, also))) => on == also && (a == b || rl_core::geometry::is_adjacent(a, b)),
        _ => false,
    }
}

/// Whether a trade between `parties` can still go on: two sides, the
/// trader still one and alive, and both within reach.
fn still_open(world: &World, parties: Parties) -> bool {
    let alive = |who: Entity| world.get_entity(who).is_ok_and(|e| !e.contains::<Dead>());
    parties.customer != parties.trader
        && alive(parties.customer)
        && alive(parties.trader)
        && world.get::<Trader>(parties.trader).is_some()
        && within_reach(whereabouts(world, parties.customer), whereabouts(world, parties.trader))
}

/// How much currency `who` carries.
fn purse(world: &World, who: Entity, currency: Currency) -> u32 {
    world.get::<Inventory>(who).map_or(0, |bag| currency.count(bag, |i| (world.get::<Tagged>(i), world.get::<Stack>(i))))
}

/// Moves `count` of `item` out of `from`'s bag into `to`'s, and returns
/// what `to` now carries it as.
///
/// Part of a stack is split off as a thing of its own, as a throw splits
/// one off; a stack lands in a carried stack of the same key, as a pickup
/// merges one. Done on the world at once, so the next deal in the same
/// frame counts what this one left.
fn hand_over(world: &mut World, from: Entity, to: Entity, item: Entity, count: u32) -> Entity {
    let stack = world.get::<Stack>(item).copied();
    let moving = match stack {
        Some(s) if count < s.count => {
            if let Some(mut left) = world.get_mut::<Stack>(item) {
                left.count -= count;
            }
            let part = world.entity_mut(item).clone_and_spawn();
            world.entity_mut(part).insert(Stack { key: s.key, count });
            part
        }
        _ => {
            if let Some(mut bag) = world.get_mut::<Inventory>(from) {
                bag.remove(item);
            }
            item
        }
    };
    let joins = stack.and_then(|s| {
        let bag = world.get::<Inventory>(to)?;
        bag.items.iter().copied().find(|c| world.get::<Stack>(*c).is_some_and(|t| t.key == s.key))
    });
    match joins {
        Some(into) => {
            if let Some(mut s) = world.get_mut::<Stack>(into) {
                s.count += count;
            }
            world.despawn(moving);
            into
        }
        None => {
            if let Some(mut bag) = world.get_mut::<Inventory>(to) {
                bag.items.push(moving);
            }
            moving
        }
    }
}

/// Moves `amount` of the currency from `payer`'s bag to `payee`'s,
/// drawing the payer's pieces down in the order they are carried.
///
/// The crate's one way to spend it, a deal's price and an offer's alike,
/// so what is paid is always a real pile that lands in someone's pack.
pub(crate) fn pay(world: &mut World, payer: Entity, payee: Entity, amount: u32, currency: Currency) {
    let pieces: Vec<Entity> =
        world.get::<Inventory>(payer).map(|b| b.items.iter().copied().filter(|i| currency.is(world.get::<Tagged>(*i))).collect()).unwrap_or_default();
    let mut left = amount;
    for piece in pieces {
        if left == 0 {
            break;
        }
        let take = left.min(world.get::<Stack>(piece).map_or(1, |s| s.count));
        hand_over(world, payer, payee, piece, take);
        left -= take;
    }
}

/// One deal asked for, a [`Buy`] or a [`Sell`] in one shape.
#[derive(Debug, Clone, Copy)]
struct Asked {
    parties: Parties,
    deal: Deal,
    item: Entity,
    count: u32,
}

/// Works one deal through: `None` for one that could not be meant, a
/// refusal with its reason, or the deal done.
fn strike(world: &mut World, asked: Asked, currency: Currency) -> Option<Result<Traded, TradeRefused>> {
    let Asked { parties, deal, item, count } = asked;
    let Parties { customer, trader } = parties;
    if !still_open(world, parties) || world.get::<Inventory>(customer).is_none() || world.get::<Inventory>(trader).is_none() {
        return None;
    }
    // The thing goes one way and the currency the other.
    let (from, to) = match deal {
        Deal::Bought => (trader, customer),
        Deal::Sold => (customer, trader),
    };
    if !world.get::<Inventory>(from).is_some_and(|b| b.contains(item)) {
        return None;
    }
    let count = count.min(world.get::<Stack>(item).map_or(1, |s| s.count));
    if count == 0 {
        return None;
    }
    let terms = world.get::<Trader>(trader)?.clone();
    let tags = world.get::<Tagged>(item).map(|t| t.0.clone()).unwrap_or_default();
    let ware = Ware { worth: world.get::<Worth>(item).copied(), tags: &tags, worn: world.get::<Equipped>(from).is_some_and(|w| w.contains(item)) };
    let price = match terms.quote(deal, ware, count, purse(world, to, currency)) {
        Ok(price) => price,
        Err(why) => return Some(Err(TradeRefused { customer, trader, deal, item, why })),
    };
    pay(world, to, from, price, currency);
    let item = hand_over(world, from, to, item, count);
    Some(Ok(Traded { customer, trader, deal, item, count, price }))
}

/// Resolves every [`Buy`] and [`Sell`] asked for this frame, at once and
/// for no time, answering each with [`Traded`] or [`TradeRefused`].
///
/// On the world rather than through queries, so each deal's moves are made
/// before the next is counted: two buys in one frame pay out of what the
/// first left, and a stack split off by one is there to merge into for the
/// next. Buys before sells, each in the order asked.
pub fn resolve_trades(world: &mut World, mut buys: Local<MessageCursor<Buy>>, mut sells: Local<MessageCursor<Sell>>) {
    let mut asked: Vec<Asked> = Vec::new();
    if let Some(messages) = world.get_resource::<Messages<Buy>>() {
        asked.extend(buys.read(messages).map(|b| Asked {
            parties: Parties { customer: b.customer, trader: b.trader },
            deal: Deal::Bought,
            item: b.item,
            count: b.count,
        }));
    }
    if let Some(messages) = world.get_resource::<Messages<Sell>>() {
        asked.extend(sells.read(messages).map(|s| Asked {
            parties: Parties { customer: s.customer, trader: s.trader },
            deal: Deal::Sold,
            item: s.item,
            count: s.count,
        }));
    }
    let Some(currency) = world.get_resource::<Currency>().copied() else { return };
    for one in asked {
        match strike(world, one, currency) {
            Some(Ok(done)) => drop(world.write_message(done)),
            Some(Err(refused)) => drop(world.write_message(refused)),
            None => debug!("a deal that could not be meant was dropped: {one:?}"),
        }
    }
}

/// Opens a trade when the player takes up a trader's [`TRADE`] offer.
///
/// In [`TurnSet::React`](crate::plugin::TurnSet::React), where every
/// answer to an [`Interacted`] is: the offer's time is already spent. An
/// offer of `trade` from something that is no [`Trader`] is left to the
/// game, which may answer it itself.
pub fn open_trades(
    mut done: MessageReader<Interacted>,
    verbs: Res<Verbs>,
    traders: Query<(), With<Trader>>,
    players: Query<(), With<Player>>,
    mut trading: ResMut<Trading>,
) {
    let Some(trade) = verbs.get(TRADE) else { return };
    for i in done.read() {
        if i.verb == trade && traders.contains(i.target) && players.contains(i.actor) {
            trading.0 = Some(Parties { customer: i.actor, trader: i.target });
        }
    }
}

/// Closes the open trade once either side is out of the other's reach,
/// dead or gone, so a screen never deals with someone who has walked off.
///
/// In [`TurnSet::React`](crate::plugin::TurnSet::React) after
/// [`open_trades`], every pass, so a shove, a warp or a death in any pass
/// closes it before the player is next shown it.
pub fn close_trades(mut trading: ResMut<Trading>, places: Query<(&Position, Option<&OnMap>), Without<Dead>>, traders: Query<(), With<Trader>>) {
    let Some(Parties { customer, trader }) = trading.open() else { return };
    let at = |who: Entity| places.get(who).ok().map(|(p, on)| (p.0, on.map(|m| m.0).unwrap_or(MapId::SURFACE)));
    if !traders.contains(trader) || !within_reach(at(customer), at(trader)) {
        trading.close();
    }
}

/// Trade between two actors: buying and selling at once, for no time,
/// paid in the game's currency.
///
/// Opt-in, and needs [`ItemsPlugin`](crate::items::ItemsPlugin) for the
/// packs and [`PropsPlugin`](crate::props::PropsPlugin) for the offer a
/// trade opens through, and a [`Currency`] before play begins.
pub struct TradePlugin;

impl Plugin for TradePlugin {
    fn build(&self, app: &mut App) {
        use crate::plugin::{EngineSet, Needs, Reads, ResetsOnNewRun, Turn, TurnSet};
        use crate::props::AddVerb;
        app.needs::<Currency>("TradePlugin", "`Currency(registries.tags.expect(\"...\"))`, the tag every piece of the currency carries")
            .add_verb(TRADE)
            .init_resource::<Trading>()
            .reset_on_new_run::<Trading>()
            .add_message::<Buy>()
            .add_message::<Sell>()
            .add_message::<Traded>()
            .add_message::<TradeRefused>()
            .reads::<Interacted>()
            // After the keys and before the turns, so a deal a screen asks
            // for is done in the frame it was asked, with no turn dealt in
            // between, and a game's answer in the turn's `React` sees it.
            .add_systems(Update, resolve_trades.after(EngineSet::Input).before(EngineSet::Turns).run_if(in_state(crate::state::EngineState::Playing)))
            .add_systems(Turn, (open_trades, close_trades).chain().in_set(TurnSet::React));
    }

    fn finish(&self, app: &mut App) {
        crate::plugin::depends_on::<crate::items::ItemsPlugin>(app, "TradePlugin");
        crate::plugin::depends_on::<crate::props::PropsPlugin>(app, "TradePlugin");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{Actor, Blocks, MyTurn, Viewshed};
    use crate::items::{Item, ItemsPlugin};
    use crate::plugin::headless_app;
    use crate::props::{Interact, Offering, Offers, PropsPlugin};
    use crate::registries::Registries;
    use crate::state::EngineState;
    use crate::turn::{Intent, Step, Turns};
    use rl_core::{Direction, Point};
    use rl_rules::{Registry, TagDef};

    /// The stack keys of the three kinds of thing in play.
    const SCRIP: u64 = 1;
    const TOOL: u64 = 2;

    struct Rig {
        app: App,
        start: Point,
        player: Entity,
        trader: Entity,
        currency: TagId,
        tool: TagId,
        trinket: TagId,
    }

    impl Rig {
        /// The player with 50 in currency, and beside it a trader with 100,
        /// three tools worth 10 each and a trinket worth 30, who buys tools
        /// at a quarter of their worth and offers `trade`.
        fn new() -> Self {
            let mut app = headless_app();
            app.add_plugins((crate::fov::FovPlugin, crate::world::StreamingPlugin, ItemsPlugin, PropsPlugin, TradePlugin));
            let start = crate::testing::surface(&mut app);
            let tags = Registry::from_defs(vec![TagDef::new("scrip"), TagDef::new("tool"), TagDef::new("trinket")]).expect("three tags");
            let (currency, tool, trinket) = (tags.expect("scrip"), tags.expect("tool"), tags.expect("trinket"));
            app.insert_resource(Registries { tags, ..Default::default() })
                .insert_resource(Currency(currency))
                .insert_resource(crate::seed::Seed(crate::testing::TEST_SEED));
            let trade = app.world().resource::<Verbs>().get(TRADE).expect("the plugin declares the verb");
            let purse = app.world_mut().spawn((Item, Tagged(vec![currency]), Stack { key: SCRIP, count: 50 })).id();
            let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(6), Inventory { items: vec![purse] })).id();
            let till = app.world_mut().spawn((Item, Tagged(vec![currency]), Stack { key: SCRIP, count: 100 })).id();
            let tools = app.world_mut().spawn((Item, Tagged(vec![tool]), Stack { key: TOOL, count: 3 }, Worth(10))).id();
            let charm = app.world_mut().spawn((Item, Tagged(vec![trinket]), Worth(30))).id();
            let trader = app
                .world_mut()
                .spawn((
                    Actor,
                    Blocks,
                    Position(start.offset(1, 0)),
                    Inventory { items: vec![till, tools, charm] },
                    Trader { buys: vec![tool], pays_pct: 25 },
                    Offers(vec![Offering::new(trade, 100)]),
                ))
                .id();
            app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
            app.update();
            app.update();
            assert!(app.world().get::<MyTurn>(player).is_some(), "the player holds the turn");
            Self { app, start, player, trader, currency, tool, trinket }
        }

        fn bag(&self, who: Entity) -> Vec<Entity> {
            self.app.world().get::<Inventory>(who).map(|b| b.items.clone()).unwrap_or_default()
        }

        /// Each carried thing of `who`'s as (stack key or none, count).
        fn holds(&self, who: Entity) -> Vec<(Option<u64>, u32)> {
            let w = self.app.world();
            self.bag(who).iter().map(|i| w.get::<Stack>(*i).map_or((None, 1), |s| (Some(s.key), s.count))).collect()
        }

        fn purse(&self, who: Entity) -> u32 {
            purse(self.app.world(), who, Currency(self.currency))
        }

        fn carried(&mut self, who: Entity, thing: impl Bundle) -> Entity {
            let item = self.app.world_mut().spawn((Item, thing)).id();
            self.app.world_mut().get_mut::<Inventory>(who).unwrap().items.push(item);
            item
        }

        /// Writes `ask` and runs a frame, returning what was answered.
        fn ask<M: Message>(&mut self, ask: M) -> (Vec<Traded>, Vec<TradeRefused>) {
            self.app.world_mut().write_message(ask);
            self.app.update();
            let w = self.app.world_mut();
            let done = w.resource_mut::<Messages<Traded>>().drain().collect();
            let refused = w.resource_mut::<Messages<TradeRefused>>().drain().collect();
            (done, refused)
        }

        fn buy(&mut self, item: Entity, count: u32) -> (Vec<Traded>, Vec<TradeRefused>) {
            self.ask(Buy { customer: self.player, trader: self.trader, item, count })
        }

        fn sell(&mut self, item: Entity, count: u32) -> (Vec<Traded>, Vec<TradeRefused>) {
            self.ask(Sell { customer: self.player, trader: self.trader, item, count })
        }

        fn now(&self) -> u32 {
            self.app.world().resource::<Turns>().now()
        }
    }

    /// Taking up the offer is the turn, and opens a trade with the one who
    /// made it; walking out of its reach closes the trade again.
    #[test]
    fn taking_up_the_trade_offer_opens_a_trade_and_walking_out_of_reach_closes_it() {
        let mut rig = Rig::new();
        let trade = rig.app.world().resource::<Verbs>().get(TRADE).unwrap();
        rig.app.world_mut().write_message(Intent::new(rig.player, Interact { target: rig.trader, verb: trade }));
        rig.app.update();
        assert_eq!(rig.app.world().resource::<Trading>().open(), Some(Parties { customer: rig.player, trader: rig.trader }), "a trade is open");
        assert_eq!(rig.now(), 100, "for the offer's time");

        rig.app.world_mut().write_message(Intent::new(rig.player, Step(Direction::South)));
        rig.app.update();
        assert!(rig.app.world().resource::<Trading>().open().is_some(), "a step that keeps beside the trader keeps the trade");
        rig.app.world_mut().write_message(Intent::new(rig.player, Step(Direction::West)));
        rig.app.update();
        assert_eq!(rig.app.world().get::<Position>(rig.player).map(|p| p.0), Some(rig.start.offset(-1, 1)));
        assert_eq!(rig.app.world().resource::<Trading>().open(), None, "out of reach, the trade is closed");
    }

    /// One bought off a stack is split off it into the buyer's pack, the
    /// next joins it there, and the price leaves the buyer's currency for
    /// the trader's, merged into the trader's own pile; none of it is a turn.
    #[test]
    fn a_buy_moves_the_ware_and_its_worth_in_currency_splitting_and_merging_stacks_and_takes_no_turn() {
        let mut rig = Rig::new();
        let tools = rig.bag(rig.trader)[1];
        let (done, refused) = rig.buy(tools, 1);
        assert!(refused.is_empty());
        let bought = rig.bag(rig.player)[1];
        assert_ne!(bought, tools, "one split off the stack is a thing of its own");
        assert_eq!(done, vec![Traded { customer: rig.player, trader: rig.trader, deal: Deal::Bought, item: bought, count: 1, price: 10 }]);
        assert_eq!(rig.holds(rig.player), vec![(Some(SCRIP), 40), (Some(TOOL), 1)], "the buyer paid 10 and carries one");
        assert_eq!(rig.holds(rig.trader), vec![(Some(SCRIP), 110), (Some(TOOL), 2), (None, 1)], "the trader's pile took the 10, and two are left");
        assert_eq!(rig.app.world().get::<Worth>(bought), Some(&Worth(10)), "the part split off is what the stack was");

        let (done, _) = rig.buy(tools, 5);
        assert_eq!(done[0].item, bought, "the rest of the stack joined the one already carried");
        assert_eq!(done[0].price, 20, "a count past the stack buys the stack");
        assert_eq!(rig.holds(rig.player), vec![(Some(SCRIP), 20), (Some(TOOL), 3)]);
        assert_eq!(rig.holds(rig.trader), vec![(Some(SCRIP), 130), (None, 1)]);
        assert!(rig.app.world().get_entity(tools).is_err(), "the emptied stack is gone");

        assert_eq!(rig.now(), 0, "no time passed");
        assert!(rig.app.world().get::<MyTurn>(rig.player).is_some(), "and the player still holds the turn");
    }

    /// A trader pays its share of each one's worth rounded down, so a
    /// handful sold at once fetches what it would one at a time.
    #[test]
    fn a_sell_pays_the_traders_share_of_each_ones_worth_rounded_down() {
        let mut rig = Rig::new();
        let tool = rig.tool;
        let mine = rig.carried(rig.player, (Tagged(vec![tool]), Stack { key: TOOL, count: 3 }, Worth(7)));
        let (done, refused) = rig.sell(mine, 3);
        assert!(refused.is_empty());
        let tools = rig.bag(rig.trader)[1];
        assert_eq!(
            done,
            vec![Traded { customer: rig.player, trader: rig.trader, deal: Deal::Sold, item: tools, count: 3, price: 3 }],
            "a quarter of 7 is 1, three times"
        );
        assert_eq!(rig.purse(rig.player), 53);
        assert_eq!(rig.purse(rig.trader), 97);
        assert_eq!(rig.holds(rig.trader)[1], (Some(TOOL), 6), "sold into the trader's own stack");
        assert_eq!(rig.holds(rig.player), vec![(Some(SCRIP), 53)], "and gone from the seller's pack");
    }

    /// Each refusal is a refusal, says why, and moves nothing: the buyer
    /// short of the price, the trader short of its share, a kind the
    /// trader does not buy, a thing of no worth, and a thing worn.
    #[test]
    fn short_currency_a_kind_not_bought_no_worth_and_a_worn_thing_are_each_refused_and_say_which() {
        let mut rig = Rig::new();
        let (tool, trinket) = (rig.tool, rig.trinket);
        let refusal = |(done, refused): (Vec<Traded>, Vec<TradeRefused>)| {
            assert!(done.is_empty(), "nothing traded");
            refused.iter().map(|r| r.why).collect::<Vec<_>>()
        };
        let charm = rig.bag(rig.trader)[2];
        rig.app.world_mut().get_mut::<Worth>(charm).unwrap().0 = 60;
        assert_eq!(refusal(rig.buy(charm, 1)), vec![TradeRefusal::Short], "60 asked of 50");

        let trinket_of_mine = rig.carried(rig.player, (Tagged(vec![trinket]), Worth(30)));
        assert_eq!(refusal(rig.sell(trinket_of_mine, 1)), vec![TradeRefusal::NotBought]);
        let worthless = rig.carried(rig.player, Tagged(vec![tool]));
        assert_eq!(refusal(rig.sell(worthless, 1)), vec![TradeRefusal::NoWorth]);
        let dear = rig.carried(rig.player, (Tagged(vec![tool]), Worth(1000)));
        assert_eq!(refusal(rig.sell(dear, 1)), vec![TradeRefusal::Short], "250 asked of a trader with 100");

        let slots = Registry::from_defs(vec![rl_rules::SlotDef::new("hand")]).unwrap();
        let hand = slots.expect("hand");
        let mut worn = rl_rules::Equipment::for_slots(&slots);
        let held = rig.carried(rig.player, (Tagged(vec![tool]), Worth(8)));
        worn.equip(held, &rl_rules::EquipShape::in_slot(hand)).unwrap();
        rig.app.world_mut().entity_mut(rig.player).insert(Equipped(worn));
        assert_eq!(refusal(rig.sell(held, 1)), vec![TradeRefusal::Worn]);

        assert_eq!((rig.purse(rig.player), rig.purse(rig.trader)), (50, 100), "no currency moved");
        assert_eq!(rig.bag(rig.player).len(), 5, "and every thing stayed where it was");
        assert_eq!(rig.bag(rig.trader).len(), 3);
    }

    /// Currency is counted by its stacks, a single piece as one, and
    /// nothing else in the pack counts toward it.
    #[test]
    fn the_currency_counts_a_stack_by_its_count_a_single_piece_as_one_and_nothing_else() {
        let mut rig = Rig::new();
        let (currency, tool) = (rig.currency, rig.tool);
        rig.carried(rig.player, Tagged(vec![currency]));
        rig.carried(rig.player, (Tagged(vec![tool]), Stack { key: TOOL, count: 9 }));
        let w = rig.app.world();
        let bag = w.get::<Inventory>(rig.player).unwrap();
        assert_eq!(Currency(currency).count(bag, |i| (w.get::<Tagged>(i), w.get::<Stack>(i))), 51, "50 in a stack and one alone");
    }

    /// A deal with a trader out of reach could not have been asked from a
    /// screen, so it is dropped rather than refused.
    #[test]
    fn a_deal_with_a_trader_out_of_reach_is_dropped() {
        let mut rig = Rig::new();
        let far = rig.start.offset(3, 0);
        rig.app.world_mut().get_mut::<Position>(rig.trader).unwrap().0 = far;
        let tools = rig.bag(rig.trader)[1];
        assert_eq!(rig.buy(tools, 1), (vec![], vec![]));
        assert_eq!(rig.purse(rig.player), 50);
    }
}
