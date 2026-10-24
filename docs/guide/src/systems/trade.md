<!-- documents:
     plugins: TradePlugin, TradeViewPlugin, TradePanel
     files: crates/atglyph-bevy/src/trade.rs
            crates/atglyph-rules/src/trade.rs
            crates/atglyph-bevy/src/props.rs
            crates/atglyph-ui/src/view/trade.rs
            crates/atglyph-ui/src/panel/trade.rs
     fingerprint: f1599438 -->

# Trade

Trade is two actors swapping things for a currency: a trader sells what it carries at its worth and buys what it names at a share of it.
The currency is ordinary items carrying a tag the game names, counted by their stacks and handed over as real stacks, so nothing is made and nothing destroyed: what one side pays, the other now carries.
Taking up a trader's offer is the turn; the deals inside the trade take none, because a trade is a screen and nothing that could act on the time is in it.

## Turning it on

`TradePlugin` is opt-in, declares the `trade` verb, and declares `needs::<Currency>`, the tag every piece of the currency carries, with a hint on how to make one.
It declares `depends_on::<ItemsPlugin>` for the packs and `depends_on::<PropsPlugin>` for the offer a trade opens through.
It resolves deals in `Update` after `EngineSet::Input` and before `EngineSet::Turns`, while playing, so a deal a screen asks for is done in the frame it was asked with no turn dealt in between.
It opens and closes trades in `TurnSet::React`, where every answer to an `Interacted` is, and resets `Trading` when a new run begins.
`TradeViewPlugin` keeps the `TradeView` in `ViewSet::Collect`, and needs `UiPlugin` and `TradePlugin`.
`TradePanel::new(rect)` is the screen: it adds the view plugin if the game has not, declares the `trade` modal, reads its keys in `EngineSet::Input` and draws in `PresentSet::Overlay`, and `TradePanel::words` replaces what it says.
A game binds nothing: the screen opens itself when the engine opens a trade.

## The model

A trader is an actor carrying `Trader` and offering the `TRADE` verb through its `Offers`, as `Offering::new(verb, time)`, whose time is what opening the trade costs.
`Trader` is `buys`, the tags of what it buys, empty for one that only sells, and `pays_pct`, the share of a thing's worth it pays, in hundredths.
What it sells is what it carries: its `Inventory` is its stock, saved with the run as any actor's pack is, and restocking is the game filling it.
`Worth(pub u32)` is what one of a thing is worth in the currency, the game's, kept current on each item and not saved, so a continued run has it back from the game's own pass; a thing without one is neither sold nor bought.
`Currency(pub TagId)` names the tag, `is` says whether a thing is a piece of it, and `count` is how much a bag holds, a stack counting its count.
Taking up the offer opens a trade the player's screen reads from `Trading`, whose `open` gives the `Parties`, the `customer` and the `trader`, and whose `close` ends it.
A trade is the player's only, and is closed in any pass where either side is out of the other's reach, dead or gone, so a screen never deals with someone who has walked off.
A mind that trades writes the deals itself.
`Buy { customer, trader, item, count }` buys from what the trader carries and `Sell` sells from what the customer carries, `count` capped at what a stack holds.
Each is dropped without a word when it could not be meant, and otherwise answered with `Traded { customer, trader, deal, item, count, price }`, `item` as its new holder carries it, or with `TradeRefused { customer, trader, deal, item, why }`, nothing changing hands, for a game's line saying why.
`Trader::each` is what one of a `Ware` changes hands for, its worth bought and the trader's share of it sold, and `Trader::quote` is what a count comes to or the `TradeRefusal` it is refused for: worn, sold to a trader that does not buy its kind, no worth, or a short purse, checked in that order.
`quote` is the one rule the resolver strikes every deal by, public so a screen greys a row by the same rule.
`pay(world, payer, payee, amount, currency)` moves currency from one bag to the other, drawing the payer's pieces down in the order they are carried, and is the one way the engine spends it, a deal's price and a priced offer's alike.
A payee with no pack is given one, a payee that is gone is paid nothing, and `pay` returns what it paid, never more than the payer had.
`TradeView` is the open trade as plain rows: `parties`, the `trader` as a `Row` carrying whatever facet the game pushed onto it, `wares` and `pack` as `TradeRow`s, and `purse`, `trader_purse` and `currency`, the currency's own name.
The wares are what the trader carries with a price and not worn, so its own gear is never offered; the pack is what the customer carries but its currency, priced at what the trader would pay.
A `TradeRow` is a shared `Row`, the `count`, the price of one as `each` and the `refused` reason, with `price` for the whole row and `open` saying whether one could change hands now.
`TradeView::rows_mut` walks every row on both sides, for a game annotating what it recognises.
The screen shows the wares on the left and the pack on the right: the direction keys walk the rows and cross the sides, confirm deals one of the row picked out, `TradeKeys::whole`, `a` by default and rebound by inserting a `TradeKeys`, deals the whole of it, and close puts the screen away and ends the trade.
A deal is written whether or not its row is greyed, so the resolver stays the one judge of it, and dealing never closes the screen, since a shop is usually more than one deal.
`TradeMenu` is where the cursor is, a `TradeSide` and a row on each side, and `trade_modal` is the modal's id for a game gating its own systems.
`TradeWords` is every word the screen draws that is not a name or the game's line, from the side headings to what each refusal is called on the side it is struck from.

## Using it

A trader is a component on an actor the game spawns, and Undermere's keepers are each a `Trader` buying what their shop's file says, at a quarter of its worth.

<!-- include: ../../../../examples/undermere/src/shops.rs:keeper -->
```rust,no_run
/// What a keeper is besides one of Mereholt's people: its [`Post`] and the
/// [`Trader`] that buys what `wares.ron` says, which the engine does not
/// save, so a keeper stood on the first arrival and one a continued run
/// puts back are the same.
pub fn keeper(post: Post, wares: &Wares, armory: &Armory) -> impl Bundle {
    let keeper = wares.keeper(post.keeper).expect("every post's keeper is in wares.ron");
    (Shopkeeper(post), Trader { buys: trader_tags(keeper, armory), pays_pct: PAYS_PCT })
}
```

The step turning it on takes beyond the plugins is the currency, a tag of the game's content.

<!-- include: ../../../../examples/undermere/src/plugin.rs:currency -->
```rust,no_run
        // Trading, paid in the one tag every piece of gold carries, so the
        // engine counts a purse and a price without knowing a coin from
        // anything else Undermere calls an item.
        app.insert_resource(Currency(library.registries.tags.expect("gold")));
```

## The line

The engine decides how a deal is struck and paid: the price from the worth and the share, the order of refusals, that the currency is conserved, that a trade takes no time once open and that it closes when the two part.
The game decides every price: `Worth` is its, because only the game knows what a level, a kind or a sign of something wrong is worth, and the engine never puts a number on a thing.
It decides who trades, what each trader buys and what share it pays, what the currency is, and what a trader stocks and when it restocks.
Its words are its own too: the screen's title is the trader's `Name`, the line under it is the game's facet, and the currency is named by its own `Name`.
A service, a rite or a mending, is an offer with a price rather than a trade: `Offering::priced` is paid by the same `pay` before the game hears of it in `Interacted`.
What a deal means beyond the exchange, such as knowing the kind of a thing bought, is the game's, read off `Traded`, and what a refusal is called in the log is the game's, read off `TradeRefused`.

## Where it lives

`atglyph-rules` holds the price and the purse as plain values, `each`, `quote` and `draw_down`, so the share's rounding, the order of refusals and how a purse is drawn down are tested without an `App`.
`atglyph-bevy` gathers a trader's share, a thing's worth, its tags and whether it is worn, applies what the rule returns, and moves real stacks between bags, which only a world can.
`atglyph-ui` reads the trade as a view and runs the screen over it, striking every row by the engine's own `quote`, so the screen and the trade never disagree.
