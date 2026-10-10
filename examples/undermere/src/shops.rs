//! What Mereholt's keepers may stock, and how one stocking of their shelves
//! is drawn.
//!
//! [`Wares`] is `assets/wares.ron`, read against the [`Armory`]: per
//! keeper, what it buys, what is on its shelf always, and the parts drawn
//! for the rest, each drawn as the engine's [`LootTable`] draws at a band,
//! the band being the [`tier`](tier_of) the adventurer has reached; and one
//! shared [`LevelTable`] for how much better than plain a ware that takes
//! levels is. What a ware costs is its `price` in `items.ron`, so a shelf
//! and a find are priced by the one number.
//!
//! [`Wares::stock`] is the pure half, tested without an `App`: one
//! keeper's [`Stocking`] at a tier from whatever generator it is handed.
//! [`Wares::stocking`] is every keeper's at once from
//! `Seed::stream(b"undermere.wares", number)`, a stream for each stocking
//! of the run, so the same seed meets the same shelves however the run got
//! there.
//!
//! Undermere's own rather than the engine's drop or scatter: a shelf is a
//! fixed number of draws, never the same item twice, on top of what is
//! always there, and neither of those reads a count like that.
//!
//! The keepers are the engine's traders, and four of Mereholt's
//! [`townsfolk`](crate::townsfolk), who stand them at their marks; a
//! [`Post`] is which shelf of `wares.ron` a role keeps. [`keep_shops`]
//! fills their shelves on Mereholt's first arrival, with the run's first
//! stocking as the `Inventory` each sells from and its purse in gold
//! beside it, and on a later arrival from a deeper tier than the shelves
//! were drawn for, it clears them and draws the next stocking, which
//! [`Shelves`] counts. The engine runs the screen, the prices and the
//! deals; what is Undermere's is what a keeper knows. Keepers name what
//! they sell, so a thing on a shelf is worth its kind's price known or not
//! ([`keep_worth`](crate::armory::keep_worth)), [`shelf_names`] lists
//! every ware by its true name in the shop's own colour rather than by the
//! face it wears in the pack, and [`learn_by_buying`] makes a bought
//! thing's kind known. [`count_gold`] says on the rail what the adventurer
//! has to spend, counted by the trade's own rule.

use std::ops::RangeInclusive;

use bevy::prelude::*;
use rand::Rng;
use rand::rngs::StdRng;
use rl_engine::prelude::*;
use rl_engine::rl_core::noun::listed;
use rl_engine::rl_rules::loot::{DropRow, DropTable, LevelRow, LevelTable, LootRow, LootTable};
use rl_engine::rl_ui::TradeView;
use rl_engine::rl_ui::view::offers::OffersView;
use ron::extensions::Extensions;
use ron::options::Options;
use ron::value::RawValue;
use serde::{Deserialize, Serialize};

use crate::armory::{Armory, ItemDef, ItemKind, spawn_found, spawn_items};
use crate::dungeon::TOWN;
use crate::lore::Lore;
use crate::run::Adventurer;
use crate::tally::Tally;

const WARES_RON: &str = include_str!("../assets/wares.ron");

/// The stream every stocking draws from, with the stocking's number as its
/// index.
pub const WARES_STREAM: &[u8] = b"undermere.wares";

/// The tiers a stocking is drawn for: one for every two floors, so a run
/// sees at most five stockings.
pub const TIERS: RangeInclusive<u32> = 1..=5;

/// What no keeper ever stocks by name: the two scrolls that only ever
/// harm their reader, which keepers buy cheap and never sell. An artifact
/// is never stocked either, by its `artifact` tag, since each is a
/// mini-boss's alone; gold is refused by having no price.
pub const NEVER_STOCKED: [&str; 2] = ["scroll of aggravate monsters", "scroll of summon monsters"];

/// Whether `def` is never on a shelf: an artifact, or one of
/// [`NEVER_STOCKED`].
fn never_stocked(def: Id<ItemDef>, armory: &Armory) -> bool {
    NEVER_STOCKED.contains(&armory.defs.get(def).name.as_str()) || armory.is_artifact(def)
}

/// The tier a stocking drawn after reaching floor `floor` is for: one for
/// floors one and two and before any floor, then one more for every two
/// floors, five for the last two.
pub fn tier_of(floor: u32) -> u32 {
    (floor.saturating_sub(1) / 2 + 1).clamp(*TIERS.start(), *TIERS.end())
}

/// The kinds a keeper buys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Buys {
    /// Anything with a price: the curio.
    Anything,
    /// Things carrying one of these tags and a price; none buys nothing.
    Tags(Vec<TagId>),
}

/// One ware on a shelf: what it is, how many, and its level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ware {
    /// What it is.
    pub def: Id<ItemDef>,
    /// How many lie together on the shelf.
    pub count: u32,
    /// Its level, nought for a thing that takes none and never below.
    pub level: i32,
}

/// One keeper's shelf and purse for one stocking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stocking {
    /// The keeper's name in `wares.ron`.
    pub keeper: String,
    /// What is on the shelf: what is always there first, then what was
    /// drawn, then what turned up on its own chance.
    pub wares: Vec<Ware>,
    /// The gold the keeper can pay out, its [`purse`](Keeper::purse) at
    /// the stocking's tier.
    pub purse: u32,
}

/// One keeper as `wares.ron` writes it.
#[derive(Debug, Clone)]
pub struct Keeper {
    /// Who stands behind the counter, the name the shop is found by.
    pub name: String,
    /// The kinds it buys.
    pub buys: Buys,
    /// The gold its purse holds for each tier, `purse` in `wares.ron`.
    purse_per_tier: u32,
    /// What every stocking holds, with how many and the level a thing of
    /// it is at plain, when it takes levels.
    always: Vec<(Id<ItemDef>, u32, Option<i32>)>,
    /// How many parts are drawn by weight a stocking.
    draws: u32,
    /// The parts, each item at most once and none of them an `always` one.
    parts: Vec<Part>,
}

/// One part of a keeper's shelf, drawn or on its own chance.
#[derive(Debug, Clone)]
struct Part {
    /// The item's name, which the table is sorted by.
    name: String,
    /// The tiers, weight and count, as a loot table's row.
    row: LootRow<Id<ItemDef>>,
    /// Percent of stockings it is on the shelf, instead of being drawn.
    chance: Option<u32>,
    /// The level a thing of it is at plain, when it takes levels.
    plain: Option<i32>,
}

impl Keeper {
    /// Whether it buys a thing of `def`: one with a price, carrying a tag
    /// it names unless it buys anything.
    pub fn buys(&self, def: &ItemDef) -> bool {
        def.price.is_some()
            && match &self.buys {
                Buys::Anything => true,
                Buys::Tags(tags) => def.tags.iter().any(|t| tags.contains(&t.id())),
            }
    }

    /// The gold it holds for a stocking at `tier`: its `purse` from
    /// `wares.ron` for every tier, what it can pay out for what the
    /// adventurer brings up. Set afresh with each stocking rather than
    /// added to, so selling everything to one keeper at the first tier
    /// empties it, and what the adventurer paid a keeper is gone at the
    /// next restock. A deal conserves coins; a restock does not, on
    /// purpose, so no purse grows past what its tier allows. A keeper's own
    /// in the file rather than one rule for all, so the curio, the shop
    /// that buys the rare, can carry twice the others' and pay for an
    /// artifact from the first tier.
    pub fn purse(&self, tier: u32) -> u32 {
        self.purse_per_tier * tier
    }
}

/// Everything `wares.ron` holds: every keeper, and the one level table
/// their wares are rolled from.
#[derive(Resource, Debug, Clone)]
pub struct Wares {
    levels: LevelTable,
    keepers: Vec<Keeper>,
}

impl Wares {
    /// Reads `assets/wares.ron` against `registries` and `armory`. Panics
    /// naming the file and every problem in it.
    pub fn load(registries: &Registries, armory: &Armory) -> Self {
        Self::read(WARES_RON, registries, armory).unwrap_or_else(|e| panic!("assets/wares.ron: {e}"))
    }

    /// Reads a wares file: every name that resolves to nothing first, and
    /// once every name resolves, every other problem at once, each under
    /// its keeper.
    pub fn read(text: &str, registries: &Registries, armory: &Armory) -> Result<Self, ContentError> {
        let options = Options::default().with_default_extension(Extensions::IMPLICIT_SOME);
        let file: AuthoredFile = options.from_str(text).map_err(|e| ContentError::Parse(e.to_string()))?;
        let levels = LevelTable::new(file.levels).map_err(|e| ContentError::Invalid(e.into_iter().map(|e| format!("levels: {e}")).collect()))?;
        let authored: Vec<AuthoredKeeper> = registries.names().with("item", &armory.defs).load_list(file.keepers.get_ron())?;
        let mut errors = Vec::new();
        let mut keepers: Vec<Keeper> = Vec::new();
        for a in authored {
            if keepers.iter().any(|k| k.name == a.name) {
                errors.push(format!("{}: a second keeper of the name", a.name));
            }
            keepers.push(a.build(armory, &mut errors));
        }
        if !errors.is_empty() {
            return Err(ContentError::Invalid(errors));
        }
        Ok(Self { levels, keepers })
    }

    /// Every keeper, in the file's order.
    pub fn keepers(&self) -> &[Keeper] {
        &self.keepers
    }

    /// The keeper called `name`, or `None`.
    pub fn keeper(&self, name: &str) -> Option<&Keeper> {
        self.keepers.iter().find(|k| k.name == name)
    }

    /// Draws one stocking of `keeper`'s shelf at `tier` from `rng`: what is
    /// always there, then its draws by weight among the parts at the tier
    /// not on the shelf yet, then each part with a chance at the tier
    /// rolled on its own. No item is on one shelf twice: a second dagger
    /// would be a row the browsing reads as another kind, and a shelf of
    /// six draws that came up three daggers is a poorer smithy than six
    /// kinds. Each thing that takes levels rolls one
    /// from the level table at the tier on top of its plain level, so it is
    /// never below nought; a thing that takes none draws nothing for it.
    /// Draws stop early when nothing is left to draw. Panics on a keeper
    /// `wares.ron` has not, which is a typo in the caller.
    pub fn stock(&self, keeper: &str, tier: u32, rng: &mut impl Rng) -> Stocking {
        let keeper = self.keeper(keeper).unwrap_or_else(|| panic!("no keeper called {keeper:?} in wares.ron"));
        let band = tier as i32;
        let mut wares: Vec<Ware> = Vec::new();
        for &(def, count, plain) in &keeper.always {
            wares.push(Ware { def, count, level: self.level(plain, band, rng) });
        }
        let drawn: Vec<&Part> = keeper.parts.iter().filter(|p| p.chance.is_none()).collect();
        for _ in 0..keeper.draws {
            let open = drawn.iter().filter(|p| wares.iter().all(|w| w.def != p.row.item)).map(|p| p.row.clone()).collect();
            let table = LootTable::new(open, |id| drawn.iter().find(|p| p.row.item == id).map(|p| p.name.clone()).unwrap_or_default());
            let Some((def, count)) = table.pick(band, rng) else { break };
            let plain = drawn.iter().find(|p| p.row.item == def).and_then(|p| p.plain);
            wares.push(Ware { def, count, level: self.level(plain, band, rng) });
        }
        let chanced: Vec<&Part> = keeper.parts.iter().filter(|p| p.chance.is_some() && p.row.applies(band)).collect();
        let rows = chanced.iter().map(|p| DropRow::new(p.row.item, p.chance.unwrap_or(0)).count(p.row.group.0, p.row.group.1)).collect();
        for (def, count) in DropTable(rows).roll(rng) {
            let plain = chanced.iter().find(|p| p.row.item == def).and_then(|p| p.plain);
            wares.push(Ware { def, count, level: self.level(plain, band, rng) });
        }
        Stocking { keeper: keeper.name.clone(), wares, purse: keeper.purse(tier) }
    }

    /// The level of a ware at plain level `plain` stocked at `band`: the
    /// level table's roll on top of it, or nought for a thing that takes no
    /// levels, which draws nothing.
    fn level(&self, plain: Option<i32>, band: i32, rng: &mut impl Rng) -> i32 {
        plain.map_or(0, |plain| plain + self.levels.roll(band, rng))
    }

    /// Stocking number `number` of every keeper at `tier`, in the file's
    /// order, all from `Seed::stream(WARES_STREAM, number)`: the same seed
    /// and number meet the same shelves, and no stocking's draws move
    /// another's.
    pub fn stocking(&self, seed: &Seed, number: u64, tier: u32) -> Vec<Stocking> {
        let mut rng: StdRng = seed.stream(WARES_STREAM, number);
        self.keepers.iter().map(|k| self.stock(&k.name, tier, &mut rng)).collect()
    }
}

/// What taking up a keeper's `trade` offer costs, in hundredths of a step:
/// one step, the turn the walk into them would have been. The deals on the
/// screen it opens take none.
pub const TRADE_TIME: u32 = 100;

/// The share of a thing's worth every keeper pays for it, in hundredths:
/// a quarter, rounded down by the engine's own `Trader::pays`.
pub const PAYS_PCT: u32 = 25;

/// Which keeper of `wares.ron` a role of `townsfolk.ron` keeps shop as.
///
/// Everything else a keeper is, their name, glyph and colour and the mark
/// they stand at, is their role's, so a keeper is one of Mereholt's people
/// first and a shop besides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Post {
    /// The keeper's name in `wares.ron`, which its shelf is drawn by.
    pub keeper: &'static str,
    /// The role in `townsfolk.ron` that keeps it.
    pub role: &'static str,
}

/// Mereholt's four shops, in `wares.ron`'s order.
pub const POSTS: [Post; 4] = [
    Post { keeper: "smith", role: "smith" },
    Post { keeper: "apothecary", role: "apothecary" },
    Post { keeper: "curio", role: "curio" },
    Post { keeper: "temple", role: "priest" },
];

/// The tags a keeper's [`Trader`] names: its own, or for one that buys
/// anything, every tag a priced thing carries, so the engine's rule, a
/// tag it names, buys exactly what [`Keeper::buys`] says.
pub fn trader_tags(keeper: &Keeper, armory: &Armory) -> Vec<TagId> {
    let mut tags: Vec<TagId> = match &keeper.buys {
        Buys::Tags(tags) => tags.clone(),
        Buys::Anything => armory.defs.iter().filter(|(_, d)| d.price.is_some()).flat_map(|(_, d)| d.tags.iter().map(|t| t.id())).collect(),
    };
    tags.sort();
    tags.dedup();
    tags
}

/// Which stocking Mereholt's shelves were last drawn from, and the tier it
/// was drawn for. The run's own: saved with it, and begun again with the
/// next run, so the same seed meets the same shelves however often the
/// adventurer goes home.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shelves {
    /// The stocking's number, its index in [`WARES_STREAM`]: nought for the
    /// run's first, one more for each restock.
    pub stocking: u64,
    /// The tier it was drawn for, which a restock waits to be passed.
    pub tier: u32,
}

impl Default for Shelves {
    fn default() -> Self {
        Self { stocking: 0, tier: *TIERS.start() }
    }
}

/// One of Mereholt's keepers, by its post: what a restock finds each
/// shelf by.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Shopkeeper(pub Post);

/// Fills each of Mereholt's keepers' shelves the first time the adventurer
/// arrives there, with the run's first stocking: stocking nought, at the
/// tier reached, which is the first for any run begun in Mereholt. On
/// every later arrival from deeper than the shelves were drawn for,
/// restocks them: every keeper's old wares, what it was sold among them,
/// and its purse are taken off and gone, and the next stocking is made in
/// their place with a purse for the new tier, set rather than added to.
/// Home again from the same tier, nothing is drawn, so the shelves are as
/// they were left, less what was bought.
///
/// A keeper is one of the [`townsfolk`](crate::townsfolk), stood at its
/// mark by them; its shelf is its `Inventory`, each ware made by the
/// armory at the level the stocking rolled for it, as a thing found is,
/// and its purse is gold coins in the same bag, which is what the engine
/// pays out of.
///
/// In the arrival chain, after the people are stood and the tally has the
/// floor just left. Shelves are made in [`POSTS`] order, so the run's
/// entities come out the same every time.
pub fn keep_shops(
    mut commands: Commands,
    mut entered: MessageReader<PlaceEntered>,
    shops: Shops,
    keepers: Query<(Entity, &Shopkeeper, Option<&Inventory>)>,
    tally: Res<Tally>,
    mut shelves: ResMut<Shelves>,
) {
    let Some(first) = entered.read().filter(|ev| ev.map == TOWN).map(|ev| ev.first).last() else { return };
    let tier = tier_of(tally.deepest);
    if first {
        shelves.tier = tier;
    } else if tier > shelves.tier {
        *shelves = Shelves { stocking: shelves.stocking + 1, tier };
    } else {
        return;
    }
    let stocking = shops.wares.stocking(&shops.seed, shelves.stocking, tier);
    for post in POSTS {
        let Some((keeper, _, bag)) = keepers.iter().find(|(_, k, _)| k.0 == post) else { continue };
        for item in bag.into_iter().flat_map(|b| &b.items) {
            commands.entity(*item).despawn();
        }
        let bag = shops.shelve(&mut commands, shelf_of(&stocking, post));
        commands.entity(keeper).insert(Inventory { items: bag });
    }
}

/// The stocking's shelf for `post`'s keeper.
fn shelf_of(stocking: &[Stocking], post: Post) -> &Stocking {
    stocking.iter().find(|s| s.keeper == post.keeper).unwrap_or_else(|| panic!("wares.ron has no keeper {:?}", post.keeper))
}

/// What a keeper is besides one of Mereholt's people: its [`Post`] and the
/// [`Trader`] that buys what `wares.ron` says, which the engine does not
/// save, so a keeper stood on the first arrival and one a continued run
/// puts back are the same.
pub fn keeper(post: Post, wares: &Wares, armory: &Armory) -> impl Bundle {
    let keeper = wares.keeper(post.keeper).expect("every post's keeper is in wares.ron");
    (Shopkeeper(post), Trader { buys: trader_tags(keeper, armory), pays_pct: PAYS_PCT })
}

/// What [`keep_shops`] reads.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Shops<'w> {
    wares: Res<'w, Wares>,
    seed: Res<'w, Seed>,
    armory: Res<'w, Armory>,
    registries: Res<'w, Registries>,
}

impl Shops<'_> {
    /// Makes `shelf`'s wares and purse, in that order, for a keeper's bag.
    fn shelve(&self, commands: &mut Commands, shelf: &Stocking) -> Vec<Entity> {
        let (armory, registries) = (&*self.armory, &*self.registries);
        let mut bag: Vec<Entity> = Vec::new();
        for ware in &shelf.wares {
            if armory.takes_levels(ware.def) {
                bag.extend((0..ware.count).map(|_| spawn_found(commands, armory, ware.def, ware.level, registries)));
            } else {
                bag.extend(spawn_items(commands, armory, ware.def, ware.count, registries));
            }
        }
        bag.extend(spawn_items(commands, armory, armory.defs.expect("gold coin"), shelf.purse, registries));
        bag
    }
}

/// Makes known the kind of each thing the adventurer buys: the shelf named
/// it, and now it is in the pack, so the face it wears there is no longer
/// a secret worth keeping. Said nothing of in the log, since the trade
/// screen already called it by its name; browsing teaches nothing, since
/// only a [`Traded`] for a buy does.
///
/// In `Update`, after the turns, where the engine strikes a deal and
/// writes its `Traded`, and before the frame's names and worths are given.
pub fn learn_by_buying(
    mut traded: MessageReader<Traded>,
    things: Query<&ItemKind>,
    adventurer: Query<(), With<Adventurer>>,
    armory: Res<Armory>,
    mut lore: ResMut<Lore>,
) {
    for ev in traded.read().filter(|ev| ev.deal == Deal::Bought && adventurer.contains(ev.customer)) {
        let Ok(kind) = things.get(ev.item) else { continue };
        let name = &armory.defs.get(kind.0).name;
        if !lore.knows(name) {
            lore.learn(name);
        }
    }
}

/// Lists every ware on a keeper's side of the trade screen by its kind's
/// true name, drawn in the shop's own colour, the keeper's: never by the
/// face a thing not yet known wears in the pack, and never in that face's
/// colour, which would tell the same. So browsing says which kinds a shelf
/// has and not which bottle in the pack is which. A level is not told
/// either, `+?` or known, since a shop never tells one; what the adventurer
/// carries keeps the name the pack gives it.
///
/// In `ViewSet::Annotate`, over the engine's rows.
pub fn shelf_names(mut view: ResMut<TradeView>, things: Query<&ItemKind>, armory: Res<Armory>) {
    let Some(color) = view.trader.as_ref().map(|t| t.glyph.fg) else { return };
    for ware in &mut view.wares {
        let Ok(kind) = things.get(ware.row.entity) else { continue };
        ware.row.label = listed(&armory.defs.get(kind.0).name, ware.count);
        ware.row.glyph.fg = color;
    }
}

/// Lists the trade on the offers screen as `trade with the priest`, over
/// the engine's verb and the keeper's name, `trade the priest`, which reads
/// as the keeper being what is traded. One wording for every keeper, as
/// the rites have theirs in [`temple::rite_rows`](crate::temple::rite_rows);
/// the time and any refusal stay the engine's.
///
/// In `ViewSet::Annotate`, over the engine's rows.
pub fn trade_rows(mut view: ResMut<OffersView>, verbs: Res<Verbs>, named: Query<&Name>) {
    for row in view.rows.iter_mut().filter(|row| verbs.name(row.verb) == TRADE) {
        if let Ok(name) = named.get(row.target) {
            row.label = format!("{TRADE} with {}", listed(name.as_str(), 1));
        }
    }
}

/// The tone the gold carried is said in on the rail, declared in the gold
/// coin's own colour by [`UndermerePlugin`](crate::plugin::UndermerePlugin).
pub const GOLD_TONE: &str = "gold";

/// Says on the rail the gold the adventurer carries, `gold 50`, under the
/// gauges and in the coin's colour, so what a trip home could buy is always
/// on the screen. Nought is said as well: a purse that vanished when it
/// emptied would read as a rail that forgot it.
///
/// Counted by [`Currency::count`], the rule a trade pays by, so the rail
/// and the shop never disagree. In `ViewSet::Annotate`, after the engine's
/// collector; absent vitals, as in a test that draws none, says nothing.
pub fn count_gold(
    vitals: Option<ResMut<VitalsView>>,
    currency: Res<Currency>,
    bags: Query<&Inventory>,
    pieces: Query<(Option<&Tagged>, Option<&Stack>)>,
    tones: Res<Tones>,
    mut facets: ResMut<Facets>,
) {
    let Some(mut vitals) = vitals else { return };
    let Some(bag) = vitals.entity.and_then(|me| bags.get(me).ok()) else { return };
    let gold = currency.count(bag, |item| pieces.get(item).unwrap_or((None, None)));
    let tone = tones.get(GOLD_TONE).unwrap_or(Tones::NOTICE);
    vitals.facets.push(facets.facet(GOLD_TONE, format!("gold {gold}")).toned(tone));
}

/// The file as written: the level rows, and the keepers left raw so their
/// names are read with the registries in scope.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredFile {
    levels: Vec<LevelRow>,
    keepers: Box<RawValue>,
}

/// A keeper as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredKeeper {
    name: String,
    buys: AuthoredBuys,
    purse: u32,
    #[serde(default)]
    always: Vec<AuthoredAlways>,
    #[serde(default)]
    draws: u32,
    #[serde(default)]
    parts: Vec<AuthoredPart>,
}

/// What a keeper buys, as written.
#[derive(Deserialize)]
enum AuthoredBuys {
    Anything,
    Tags(Vec<NameRef<TagDef>>),
}

/// Something always on a shelf, as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredAlways {
    item: NameRef<ItemDef>,
    count: u32,
}

/// A part, as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredPart {
    item: NameRef<ItemDef>,
    bands: (i32, i32),
    #[serde(default)]
    weight: Option<u32>,
    #[serde(default)]
    chance: Option<u32>,
    #[serde(default)]
    group: Option<(u32, u32)>,
}

impl AuthoredKeeper {
    /// The keeper, every problem with it pushed onto `errors`.
    fn build(self, armory: &Armory, errors: &mut Vec<String>) -> Keeper {
        let buys = match self.buys {
            AuthoredBuys::Anything => Buys::Anything,
            AuthoredBuys::Tags(tags) => Buys::Tags(tags.into_iter().map(NameRef::id).collect()),
        };
        let mut keeper = Keeper { name: self.name, buys, purse_per_tier: self.purse, always: Vec::new(), draws: self.draws, parts: Vec::new() };
        if keeper.purse_per_tier == 0 && keeper.buys != Buys::Tags(Vec::new()) {
            errors.push(format!("{}: a purse of nothing pays for nothing it buys", keeper.name));
        }
        let check = |def: Id<ItemDef>, keeper: &Keeper, errors: &mut Vec<String>| {
            let d = armory.defs.get(def);
            let at = format!("{}: {}", keeper.name, d.name);
            if d.price.is_none() {
                errors.push(format!("{at} has no price, so is never for sale"));
            } else if !keeper.buys(d) {
                errors.push(format!("{at} is nothing the keeper buys"));
            }
            if never_stocked(def, armory) {
                errors.push(format!("{at} is never stocked"));
            }
            at
        };
        for a in self.always {
            let def = a.item.id();
            let at = check(def, &keeper, errors);
            if a.count == 0 {
                errors.push(format!("{at}: a count of nothing is never on the shelf"));
            }
            keeper.always.push((def, a.count, armory.plain_level(def)));
        }
        for a in self.parts {
            let def = a.item.id();
            let at = check(def, &keeper, errors);
            if keeper.parts.iter().any(|p| p.row.item == def) {
                errors.push(format!("{at}: a second part of the same thing"));
            }
            if keeper.always.iter().any(|(always, ..)| *always == def) {
                errors.push(format!("{at}: a part of something always on the shelf"));
            }
            let (first, last) = a.bands;
            if first > last || !TIERS.contains(&(first.max(0) as u32)) || !TIERS.contains(&(last.max(0) as u32)) {
                errors.push(format!("{at}: tiers {first} to {last} are not a range within {} to {}", TIERS.start(), TIERS.end()));
            }
            match (a.weight, a.chance) {
                (Some(0), _) => errors.push(format!("{at}: a weight of nothing is never drawn")),
                (Some(_), Some(_)) => errors.push(format!("{at}: drawn by weight or on a chance, not both")),
                (None, None) => errors.push(format!("{at}: neither a weight nor a chance")),
                (None, Some(chance)) if !(1..=100).contains(&chance) => errors.push(format!("{at}: a chance of {chance} percent")),
                _ => {}
            }
            let group = a.group.unwrap_or((1, 1));
            if group.0 == 0 || group.0 > group.1 {
                errors.push(format!("{at}: a group of {} to {} is no group at all", group.0, group.1));
            }
            let row = LootRow::new(def).bands(first, last).weight(a.weight.unwrap_or(0)).group(group.0, group.1);
            keeper.parts.push(Part { name: armory.defs.get(def).name.clone(), row, chance: a.chance, plain: armory.plain_level(def) });
        }
        keeper
    }
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rl_engine::rl_core::RunSeed;

    use super::*;
    use crate::armory::{RING_LEVELS, STAFF_LEVEL, WORTH_PER_LEVEL};
    use crate::content::Library;
    use crate::testing::{adventurer, at, give, headless, lay, make, name_of, pack, settle, stand_on, take_up};
    use rl_engine::rl_ui::{OffersViewPlugin, TradeViewPlugin};

    use crate::content::TOWNSFOLK;
    use crate::temple::{Rites, TEMPLE};
    use crate::townsfolk::{Townsfolk, Townsperson};

    fn library() -> Library {
        Library::load(&crate::content::effect_kinds(), &Moments::default())
    }

    /// Every keeper's stocking at `tier` for each of a span of seeds, drawn
    /// as the run draws its first.
    fn stockings(library: &Library, tier: u32) -> impl Iterator<Item = Stocking> + '_ {
        (0..200).flat_map(move |s| library.wares.stocking(&Seed(RunSeed(s)), 0, tier))
    }

    fn names(library: &Library, stocking: &Stocking) -> Vec<String> {
        stocking.wares.iter().map(|w| library.armory.defs.get(w.def).name.clone()).collect()
    }

    /// A fixture's problems, read against the game's own items.
    fn refused(library: &Library, keepers: &str) -> Vec<String> {
        let text = format!("(levels: [(bands: (1, 5), levels: [(0, 1)])], keepers: [{keepers}])");
        match Wares::read(&text, &library.registries, &library.armory) {
            Err(ContentError::Invalid(errors)) => errors,
            other => panic!("expected the fixture refused, got {other:?}"),
        }
    }

    #[test]
    fn the_tiers_run_two_floors_each_from_the_town_to_the_throne() {
        assert_eq!((0..=10).map(tier_of).collect::<Vec<_>>(), [1, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5]);
    }

    #[test]
    fn the_same_seed_and_stocking_number_stock_the_same_shelves_and_another_number_others_over_a_span_of_seeds() {
        let library = library();
        let mut differ = 0;
        for s in 0..50 {
            let seed = Seed(RunSeed(s));
            for tier in TIERS {
                assert_eq!(library.wares.stocking(&seed, tier as u64, tier), library.wares.stocking(&seed, tier as u64, tier), "seed {s} tier {tier}");
            }
            differ += usize::from(library.wares.stocking(&seed, 0, 1) != library.wares.stocking(&seed, 1, 1));
        }
        assert!(differ >= 45, "a second stocking is a fresh draw: {differ} of 50 differed");
    }

    #[test]
    fn no_ware_is_below_nought_an_artifact_gold_or_a_scroll_that_only_harms_over_a_span_of_seeds() {
        let library = library();
        for tier in TIERS {
            for stocking in stockings(&library, tier) {
                for ware in &stocking.wares {
                    let def = library.armory.defs.get(ware.def);
                    assert!(ware.level >= 0, "{} at {} in tier {tier}", def.name, ware.level);
                    assert!(def.price.is_some(), "{} has no price", def.name);
                    assert!(!never_stocked(ware.def, &library.armory), "{} on {}'s shelf", def.name, stocking.keeper);
                }
            }
        }
    }

    #[test]
    fn plate_armor_waits_for_tier_four_and_a_potion_of_life_for_tier_five_over_a_span_of_seeds() {
        let library = library();
        let seen = |tier: u32, name: &str| stockings(&library, tier).any(|s| names(&library, &s).iter().any(|n| n == name));
        for tier in 1..=3 {
            assert!(!seen(tier, "plate armor"), "plate armor at tier {tier}");
        }
        for tier in 1..=4 {
            assert!(!seen(tier, "potion of life"), "a potion of life at tier {tier}");
        }
        assert!(seen(4, "plate armor") && seen(5, "potion of life"));
    }

    #[test]
    fn a_potion_of_life_is_on_the_curios_shelf_in_about_a_quarter_of_its_deepest_stockings_over_a_span_of_seeds() {
        let library = library();
        let curio: Vec<Stocking> = stockings(&library, 5).filter(|s| s.keeper == "curio").collect();
        let life = curio.iter().filter(|s| names(&library, s).iter().any(|n| n == "potion of life")).count();
        assert!((30..=70).contains(&life), "{life} of {}", curio.len());
    }

    #[test]
    fn no_shelf_holds_two_of_the_same_item_over_a_span_of_seeds() {
        let library = library();
        for tier in TIERS {
            for stocking in stockings(&library, tier) {
                let names = names(&library, &stocking);
                let distinct: std::collections::BTreeSet<&String> = names.iter().collect();
                assert_eq!(distinct.len(), names.len(), "{} at tier {tier}: {names:?}", stocking.keeper);
            }
        }
    }

    #[test]
    fn arrows_three_potions_of_healing_and_three_waystones_are_always_there_and_each_keeper_draws_its_count_over_a_span_of_seeds() {
        let library = library();
        let healing = library.armory.defs.expect("potion of healing");
        let arrow = library.armory.defs.expect("arrow");
        let waystone = library.armory.defs.expect(crate::waystone::WAYSTONE);
        for tier in TIERS {
            for stocking in stockings(&library, tier) {
                let first = stocking.wares.first().map(|w| (w.def, w.count));
                match stocking.keeper.as_str() {
                    "smith" => assert_eq!((first, stocking.wares.len()), (Some((arrow, 10)), 7)),
                    "apothecary" => assert_eq!((first, stocking.wares.len()), (Some((healing, 3)), 5)),
                    "curio" => assert!((4..=5).contains(&stocking.wares.len()), "four drawn, and a potion of life by chance"),
                    "temple" => assert_eq!((first, stocking.wares.len()), (Some((waystone, 3)), 1), "its rites are not wares"),
                    other => panic!("a keeper called {other}"),
                }
            }
        }
    }

    #[test]
    fn a_weapon_on_the_shelf_is_plain_at_tier_one_one_better_from_tier_two_and_two_from_tier_four_over_a_span_of_seeds() {
        let library = library();
        let weapon = library.registries.tags.expect("weapon");
        let levels = |tier: u32| -> Vec<i32> {
            let weapons = stockings(&library, tier).flat_map(|s| s.wares).filter(|w| library.armory.defs.get(w.def).tags.iter().any(|t| t.id() == weapon));
            let mut levels: Vec<i32> = weapons.map(|w| w.level).collect();
            levels.sort_unstable();
            levels.dedup();
            levels
        };
        assert_eq!(levels(1), [0]);
        assert_eq!(levels(2), [0, 1]);
        assert_eq!(levels(4), [0, 1, 2]);
    }

    #[test]
    fn a_ring_or_a_staff_on_the_shelf_is_at_least_as_good_as_one_found_plain_over_a_span_of_seeds() {
        let library = library();
        let r = &library.registries;
        let (ring, staff) = (r.tags.expect("ring"), r.tags.expect("staff"));
        for ware in stockings(&library, 2).flat_map(|s| s.wares) {
            let tags: Vec<TagId> = library.armory.defs.get(ware.def).tags.iter().map(|t| t.id()).collect();
            if tags.contains(&ring) {
                assert!(ware.level >= *RING_LEVELS.start());
            }
            if tags.contains(&staff) {
                assert!(ware.level >= STAFF_LEVEL);
            }
        }
    }

    /// The gold `post`'s keeper holds a tier, as `wares.ron` sets it.
    fn purse_a_tier(post: Post) -> u32 {
        if post.keeper == "curio" { 400 } else { 200 }
    }

    #[test]
    fn a_keepers_purse_is_two_hundred_gold_a_tier_and_the_curios_four_hundred() {
        let library = library();
        let mut rng = StdRng::seed_from_u64(1);
        for post in POSTS {
            for tier in TIERS {
                assert_eq!(library.wares.stock(post.keeper, tier, &mut rng).purse, purse_a_tier(post) * tier, "{} at tier {tier}", post.keeper);
            }
        }
    }

    #[test]
    fn every_keeper_of_the_town_is_in_the_file_and_stocks_only_what_it_buys() {
        let library = library();
        let wares = &library.wares;
        assert_eq!(wares.keepers().iter().map(|k| k.name.as_str()).collect::<Vec<_>>(), ["smith", "apothecary", "curio", "temple"]);
        for tier in TIERS {
            for stocking in stockings(&library, tier) {
                let keeper = wares.keeper(&stocking.keeper).expect("a keeper of the file");
                for ware in &stocking.wares {
                    assert!(keeper.buys(library.armory.defs.get(ware.def)), "{} sells what it would not buy", keeper.name);
                }
            }
        }
    }

    #[test]
    fn a_ware_not_in_items_ron_is_a_load_error_naming_it() {
        let library = library();
        let text =
            r#"(levels: [], keepers: [(name: "smith", buys: Anything, purse: 200, draws: 1, parts: [(item: "sword of legend", bands: (1, 5), weight: 1)])])"#;
        let error = Wares::read(text, &library.registries, &library.armory).expect_err("no such item").to_string();
        assert!(error.contains("sword of legend"), "{error}");
    }

    #[test]
    fn gold_an_artifact_a_bad_scroll_a_kind_the_keeper_does_not_buy_a_part_always_there_or_an_empty_purse_is_refused_naming_each() {
        let library = library();
        let errors = refused(
            &library,
            r#"(name: "smith", buys: Tags(["weapon"]), purse: 0, always: [(item: "gold coin", count: 1), (item: "dagger", count: 1)], draws: 1, parts: [
                (item: "dagger", bands: (1, 5), weight: 1),
                (item: "the Widow's Fang", bands: (1, 5), weight: 1),
                (item: "scroll of summon monsters", bands: (1, 5), weight: 1),
                (item: "sword", bands: (0, 6), weight: 1, chance: 10),
            ])"#,
        );
        let joined = errors.join("\n");
        for said in [
            "gold coin has no price",
            "the Widow's Fang is never stocked",
            "scroll of summon monsters is nothing the keeper buys",
            "tiers 0 to 6",
            "not both",
            "dagger: a part of something always on the shelf",
            "smith: a purse of nothing",
        ] {
            assert!(joined.contains(said), "{said:?} not in:\n{joined}");
        }
    }

    /// A run begun in Mereholt with the trade screen's view kept as the
    /// binary keeps it: the engine's rows, and the shelf's names over them.
    fn in_mereholt(seed: u64) -> (App, Entity) {
        let mut app = headless(RunSeed(seed));
        app.add_plugins(TradeViewPlugin).add_systems(Update, shelf_names.in_set(ViewSet::Annotate));
        let me = adventurer(&mut app);
        (app, me)
    }

    /// Every ware on Mereholt's first shelves is at its kind's plain level,
    /// and worth just its `items.ron` price: a staff at two charges costs
    /// what the price list says, as a plain sword does, and a level is
    /// worth something only above plain, so a staff one better is worth
    /// a level more. Over a span of seeds.
    #[test]
    fn every_plain_ware_a_keeper_stocks_is_worth_its_price_over_a_span_of_seeds() {
        for s in 0..4 {
            let (mut app, _) = in_mereholt(s);
            let mut staffs = Vec::new();
            for post in POSTS {
                let k = keeper(&mut app, post.role);
                let world = app.world();
                let armory = world.resource::<Armory>();
                for (thing, def) in shelf(&app, k) {
                    let d = armory.defs.get(def);
                    let level = world.get::<Enchant>(thing).map(|e| e.level);
                    assert_eq!(level, armory.plain_level(def), "seed {s}: {} on the {}'s first shelf is plain", d.name, post.role);
                    assert_eq!(world.get::<Worth>(thing).map(|w| w.0), d.price, "seed {s}: {} on the {}'s shelf at its price", d.name, post.role);
                    if armory.plain_level(def).is_some_and(|plain| plain > 0)
                        && d.tags.iter().any(|t| world.resource::<Registries>().tags.name(t.id()) == "staff")
                    {
                        staffs.push((thing, d.price.expect("a staff has a price")));
                    }
                }
            }
            for (staff, price) in staffs {
                app.world_mut().get_mut::<Enchant>(staff).expect("a staff takes levels").level += 1;
                app.update();
                assert_eq!(app.world().get::<Worth>(staff).map(|w| w.0), Some(price + WORTH_PER_LEVEL), "seed {s}: a level above plain is worth one more");
            }
        }
    }

    /// Standing beside each of Mereholt's keepers, the offers screen lists
    /// the trade as trading with them, `trade with the priest`, never as
    /// trading the keeper, which is what the engine's verb and name read.
    #[test]
    fn every_keepers_trade_reads_on_the_offers_screen_as_trading_with_them() {
        let mut app = headless(RunSeed(3));
        app.add_plugins(OffersViewPlugin).add_systems(Update, trade_rows.in_set(ViewSet::Annotate));
        let me = adventurer(&mut app);
        let trade = app.world().resource::<Verbs>().get(TRADE).expect("TradePlugin declares it");
        for post in POSTS {
            let keeper = keeper(&mut app, post.role);
            let cell = beside(&mut app, keeper).first().copied().expect("somewhere to stand beside the keeper");
            stand_on(&mut app, me, cell);
            settle(&mut app);
            let view = app.world().resource::<OffersView>();
            let row = view.rows.iter().find(|r| r.target == keeper && r.verb == trade).expect("the keeper offers trade");
            assert_eq!(row.label, format!("trade with {}", name_of(&app, keeper)), "by the name the run gave the {}", post.role);
        }
    }

    /// The keeper of `role`.
    fn keeper(app: &mut App, role: &str) -> Entity {
        let world = app.world_mut();
        let found = world.query_filtered::<(Entity, &Townsperson), With<Trader>>().iter(world).find(|(_, p)| p.0 == role).map(|(e, _)| e);
        found.unwrap_or_else(|| panic!("no keeper is the {role}"))
    }

    /// Every cell beside `keeper` the adventurer could stand on: open
    /// ground nothing stands in the way on.
    fn beside(app: &mut App, keeper: Entity) -> Vec<Point> {
        let at = app.world().get::<Position>(keeper).expect("standing").0;
        let world = app.world_mut();
        let taken: Vec<Point> =
            world.query_filtered::<(&Position, &OnMap), With<Blocks>>().iter(world).filter(|(_, on)| on.0 == TOWN).map(|(p, _)| p.0).collect();
        let map = app.world().resource::<WorldMap>();
        (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| at.offset(dx, dy))).filter(|p| map.is_walkable_on(TOWN, *p) && !taken.contains(p)).collect()
    }

    /// Stands the adventurer beside `keeper` and takes up its trade.
    fn open_trade(app: &mut App, me: Entity, keeper: Entity) {
        let cell = beside(app, keeper).first().copied().expect("somewhere to stand beside the keeper");
        stand_on(app, me, cell);
        let trade = app.world().resource::<Verbs>().get(TRADE).expect("TradePlugin declares it");
        app.world_mut().write_message(Intent::new(me, Interact { target: keeper, verb: trade }));
        settle(app);
        assert!(app.world().resource::<Trading>().open().is_some(), "the trade opened");
    }

    /// What `keeper` carries but its gold, each thing with its kind.
    fn shelf(app: &App, keeper: Entity) -> Vec<(Entity, Id<ItemDef>)> {
        let world = app.world();
        let gold = world.resource::<Armory>().defs.expect("gold coin");
        let bag = &world.get::<Inventory>(keeper).expect("a shelf").items;
        bag.iter().map(|i| (*i, world.get::<ItemKind>(*i).expect("a thing of a kind").0)).filter(|(_, def)| *def != gold).collect()
    }

    /// The first thing on the apothecary's shelf that is a potion of a
    /// kind the adventurer does not know yet, with its kind's name.
    fn unknown_potion(app: &mut App) -> Option<(Entity, String)> {
        let apothecary = keeper(app, "apothecary");
        let (armory, lore) = (app.world().resource::<Armory>(), app.world().resource::<Lore>());
        shelf(app, apothecary)
            .into_iter()
            .map(|(thing, def)| (thing, armory.defs.get(def).name.clone()))
            .find(|(_, name)| name.starts_with("potion of") && !lore.knows(name))
    }

    /// Every keeper stands at its own mark the first time a run is in
    /// Mereholt: one of the townsfolk, called by its role, offering trade,
    /// and the priest the temple's rites besides, at a quarter of a thing's worth; and a cell of
    /// the shop floor the adventurer can walk to from where the run
    /// arrives is beside each, since an offer is taken up from beside.
    #[test]
    fn every_keeper_stands_at_its_mark_on_the_first_arrival_in_mereholt_on_the_townsfolk_side_offering_trade_and_the_priest_the_rites_over_a_span_of_seeds() {
        for s in 0..3 {
            let (mut app, me) = in_mereholt(s);
            let townsfolk = app.world().resource::<Registries>().factions.expect(TOWNSFOLK);
            let trade = app.world().resource::<Verbs>().get(TRADE).expect("TradePlugin declares it");
            let spots = app.world().resource::<WorldMap>().place(TOWN).expect("Mereholt is built").spots.clone();
            let arrived = at(&app, me);
            let walked = walkable_from(&mut app, arrived);
            for post in POSTS {
                let k = keeper(&mut app, post.role);
                let mark = app.world().resource::<Townsfolk>().role(post.role).expect("every keeper has a row").post;
                let mark = spots.iter().find(|spot| spot.tag == mark as u32).expect("the mark is in mereholt.ron").at;
                let world = app.world();
                assert_eq!(world.get::<Position>(k).map(|p| p.0), Some(mark), "seed {s}: {} at their mark", post.role);
                assert_eq!(world.get::<OnMap>(k).map(|m| m.0), Some(TOWN), "seed {s}: {} in Mereholt", post.role);
                assert_eq!(world.get::<Faction>(k).map(|f| f.0), Some(townsfolk), "seed {s}: {} of the townsfolk", post.role);
                let offers: Vec<VerbId> = world.get::<Offers>(k).expect("offers").0.iter().map(|o| o.verb).collect();
                let mut offered = vec![trade];
                if post.keeper == TEMPLE {
                    offered.extend(world.resource::<Rites>().offerings(world.resource::<Verbs>()).iter().map(|o| o.verb));
                }
                assert_eq!(offers, offered, "seed {s}: {} offers trade, and the priest the rites besides", post.role);
                assert_eq!(world.get::<Trader>(k).map(|t| t.pays_pct), Some(25), "seed {s}: {} pays a quarter", post.role);
                assert!(beside(&mut app, k).iter().any(|cell| walked.contains(cell)), "seed {s}: {} can be walked up to", post.role);
            }
            let world = app.world_mut();
            assert_eq!(world.query_filtered::<(), With<Trader>>().iter(world).count(), POSTS.len(), "seed {s}: one keeper a post");
        }
    }

    /// Every cell the adventurer could walk to in Mereholt from `from`,
    /// through its doors, which a step opens, and round whatever stands in
    /// the way.
    fn walkable_from(app: &mut App, from: Point) -> Vec<Point> {
        let world = app.world_mut();
        let taken: Vec<Point> = world
            .query_filtered::<(&Position, &OnMap), (With<Blocks>, Without<crate::run::Adventurer>)>()
            .iter(world)
            .filter(|(_, on)| on.0 == TOWN)
            .map(|(p, _)| p.0)
            .collect();
        let map = app.world().resource::<WorldMap>();
        let door = crate::dungeon::registry().expect("door");
        let town = &map.place(TOWN).expect("Mereholt is built").terrain;
        let open = |q: Point| map.is_walkable_on(TOWN, q) || town.get(q) == Some(door);
        let (mut seen, mut next) = (vec![from], vec![from]);
        while let Some(p) = next.pop() {
            for q in (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| p.offset(dx, dy))) {
                if open(q) && !taken.contains(&q) && !seen.contains(&q) {
                    seen.push(q);
                    next.push(q);
                }
            }
        }
        seen
    }

    /// `drawn`'s wares as a keeper carries them: a thing that takes levels
    /// one apiece, at the level rolled for it.
    fn as_carried(armory: &Armory, drawn: &Stocking) -> Vec<Ware> {
        drawn.wares.iter().flat_map(|w| if armory.takes_levels(w.def) { vec![Ware { count: 1, ..*w }; w.count as usize] } else { vec![*w] }).collect()
    }

    /// What `keeper` carries but its gold, as wares, and its gold.
    fn carried(app: &App, keeper: Entity) -> (Vec<Ware>, u32) {
        let world = app.world();
        let gold = world.resource::<Armory>().defs.expect("gold coin");
        let (mut wares, mut purse) = (Vec::new(), 0);
        for item in &world.get::<Inventory>(keeper).expect("a shelf").items {
            let def = world.get::<ItemKind>(*item).expect("a kind").0;
            let count = world.get::<Stack>(*item).map_or(1, |st| st.count);
            if def == gold {
                purse += count;
            } else {
                wares.push(Ware { def, count, level: world.get::<Enchant>(*item).map_or(0, |e| e.level) });
            }
        }
        (wares, purse)
    }

    /// Every keeper's bag, thing by thing, in [`POSTS`] order: which shows
    /// whether a shelf was drawn again as well as what is on it.
    fn bags(app: &mut App) -> Vec<Vec<Entity>> {
        keepers(app).into_iter().map(|k| app.world().get::<Inventory>(k).expect("a shelf").items.clone()).collect()
    }

    /// Every keeper, in [`POSTS`] order.
    fn keepers(app: &mut App) -> Vec<Entity> {
        POSTS.iter().map(|post| keeper(app, post.role)).collect()
    }

    /// The adventurer, given gold enough, buys the whole of the last ware
    /// on the shelf of the keeper called `called`; returns the thing bought.
    fn buy_last(app: &mut App, me: Entity, called: &str) -> Entity {
        let gold = make(app, "gold coin");
        app.world_mut().get_mut::<Stack>(gold).expect("gold stacks").count = 5000;
        give(app, me, gold);
        let k = keeper(app, called);
        open_trade(app, me, k);
        let (item, _) = *shelf(app, k).last().expect("something on the shelf");
        let count = app.world().get::<Stack>(item).map_or(1, |st| st.count);
        app.world_mut().write_message(Buy { customer: me, trader: k, item, count });
        settle(app);
        assert!(app.world().get::<Inventory>(me).expect("a pack").contains(item), "{called} sold it");
        item
    }

    /// A keeper's shelf is the run's first stocking at the first tier,
    /// ware for ware, a thing that takes levels one apiece at the level
    /// rolled for it, and its purse beside it, two hundred gold or the
    /// curio's four hundred.
    #[test]
    fn a_keepers_shelf_is_stocking_nought_at_tier_one_and_its_purse_a_tiers_gold_over_a_span_of_seeds() {
        for s in 0..3 {
            let (mut app, _) = in_mereholt(s);
            let (wares, armory) = (app.world().resource::<Wares>().clone(), app.world().resource::<Armory>().clone());
            let stocking = wares.stocking(&Seed(RunSeed(s)), 0, 1);
            assert_eq!(*app.world().resource::<Shelves>(), Shelves { stocking: 0, tier: 1 }, "seed {s}");
            for post in POSTS {
                let k = keeper(&mut app, post.role);
                let (wares, purse) = carried(&app, k);
                assert_eq!(wares, as_carried(&armory, shelf_of(&stocking, post)), "seed {s}: {}'s shelf", post.role);
                assert_eq!(purse, purse_a_tier(post), "seed {s}: {}'s purse", post.role);
            }
        }
    }

    /// Home again from no deeper a tier than the shelves were drawn for,
    /// nothing is drawn: every bag is the same things as when the
    /// adventurer left, so what they bought is still gone.
    #[test]
    fn a_return_from_the_same_tier_finds_the_shelves_as_they_were_left_less_what_was_bought_over_a_span_of_seeds() {
        for s in 0..3 {
            let (mut app, me) = in_mereholt(s);
            let smith = keeper(&mut app, "smith");
            let before = shelf(&app, smith);
            let bought = buy_last(&mut app, me, "smith");
            let left = bags(&mut app);
            assert_eq!(shelf(&app, smith), before[..before.len() - 1], "seed {s}: the smith's shelf less what was bought");
            crate::testing::arrive(&mut app, crate::dungeon::map_of(2));
            crate::testing::arrive(&mut app, TOWN);
            assert_eq!(bags(&mut app), left, "seed {s}: the shelves as they were left");
            assert!(!shelf(&app, smith).iter().any(|(item, _)| *item == bought), "seed {s}: and what was bought not back on them");
            assert_eq!(*app.world().resource::<Shelves>(), Shelves { stocking: 0, tier: 1 }, "seed {s}: no new stocking");
        }
    }

    /// Home from a deeper tier, every keeper's old wares and purse are gone
    /// and the next stocking, drawn for that tier, is on the shelves, with
    /// a purse for the tier however much the last one had taken in; home
    /// again from the same tier, those shelves are as they were left.
    #[test]
    fn a_return_from_a_deeper_tier_draws_the_next_stocking_and_sets_each_purse_for_it_over_a_span_of_seeds() {
        for s in 0..3 {
            let (mut app, me) = in_mereholt(s);
            buy_last(&mut app, me, "smith");
            let old: Vec<Entity> = bags(&mut app).concat();
            crate::testing::arrive(&mut app, crate::dungeon::map_of(3));
            crate::testing::arrive(&mut app, TOWN);
            assert_eq!(*app.world().resource::<Shelves>(), Shelves { stocking: 1, tier: 2 }, "seed {s}: the next stocking, for tier two");
            assert!(old.iter().all(|e| app.world().get_entity(*e).is_err()), "seed {s}: the old wares and purses are gone");
            let (wares, armory) = (app.world().resource::<Wares>().clone(), app.world().resource::<Armory>().clone());
            let stocking = wares.stocking(&Seed(RunSeed(s)), 1, 2);
            for post in POSTS {
                let k = keeper(&mut app, post.role);
                let (wares, purse) = carried(&app, k);
                assert_eq!(wares, as_carried(&armory, shelf_of(&stocking, post)), "seed {s}: {}'s shelf", post.role);
                assert_eq!(purse, 2 * purse_a_tier(post), "seed {s}: {}'s purse set for tier two", post.role);
            }
            let restocked = bags(&mut app);
            crate::testing::arrive(&mut app, crate::dungeon::map_of(4));
            crate::testing::arrive(&mut app, TOWN);
            assert_eq!(bags(&mut app), restocked, "seed {s}: home again from tier two, the same shelves");
            assert_eq!(*app.world().resource::<Shelves>(), Shelves { stocking: 1, tier: 2 }, "seed {s}");
        }
    }

    /// Two runs of the same seed that go home from the same depth meet the
    /// same restocked shelves, even when the depth skips a tier.
    #[test]
    fn the_same_seed_restocks_the_same_shelves_over_a_span_of_seeds() {
        for s in 0..3 {
            let restocked = |s: u64| {
                let (mut app, _) = in_mereholt(s);
                crate::testing::arrive(&mut app, crate::dungeon::map_of(5));
                crate::testing::arrive(&mut app, TOWN);
                let shelves: Vec<(Vec<Ware>, u32)> = keepers(&mut app).into_iter().map(|k| carried(&app, k)).collect();
                (*app.world().resource::<Shelves>(), shelves)
            };
            let (first, second) = (restocked(s), restocked(s));
            assert_eq!(first.0, Shelves { stocking: 1, tier: 3 }, "seed {s}: one stocking more, for the tier reached");
            assert_eq!(first, second, "seed {s}");
        }
    }

    /// Every keeper as a continued run would have to give it back, in
    /// [`POSTS`] order: its name, where it stands, its side, what it
    /// offers, what it buys and pays, its shelf by name and its purse.
    #[allow(clippy::type_complexity)]
    fn keepers_as_seen(
        app: &mut App,
    ) -> Vec<(String, Option<(Point, MapId)>, Option<FactionId>, Vec<VerbId>, Option<(Vec<TagId>, u32)>, Vec<(String, u32, i32)>, u32)> {
        keepers(app)
            .into_iter()
            .map(|k| {
                let world = app.world();
                let (wares, purse) = carried(app, k);
                let armory = world.resource::<Armory>();
                (
                    name_of(app, k),
                    world.get::<Position>(k).zip(world.get::<OnMap>(k)).map(|(p, m)| (p.0, m.0)),
                    world.get::<Faction>(k).map(|f| f.0),
                    world.get::<Offers>(k).map(|o| o.0.iter().map(|o| o.verb).collect()).unwrap_or_default(),
                    world.get::<Trader>(k).map(|t| (t.buys.clone(), t.pays_pct)),
                    wares.iter().map(|w| (armory.defs.get(w.def).name.clone(), w.count, w.level)).collect(),
                    purse,
                )
            })
            .collect()
    }

    /// The run's save as the slot holds it now.
    fn saved(app: &App) -> String {
        app.world().resource::<Saves>().load(crate::save::SLOT).expect("the slot reads").expect("the arrival wrote the run down")
    }

    /// A run saved in Mereholt, after a restock and a purchase from each of
    /// two stockings, continues with every keeper at its mark with the
    /// same shelf, purse and offer of trade, and the same stocking; and one
    /// saved below, before the restock, meets on going home the same
    /// keepers restocked the same as the run that was not saved.
    #[test]
    fn a_run_saved_and_continued_in_mereholt_or_below_keeps_its_keepers_shelves_purses_offers_and_stocking() {
        for s in 0..2 {
            let (mut app, me) = in_mereholt(s);
            buy_last(&mut app, me, "smith");
            crate::testing::arrive(&mut app, crate::dungeon::map_of(3));
            let below = saved(&app);
            crate::testing::arrive(&mut app, TOWN);
            let restocked = keepers_as_seen(&mut app);
            buy_last(&mut app, me, "apothecary");
            crate::testing::arrive(&mut app, crate::dungeon::map_of(4));
            crate::testing::arrive(&mut app, TOWN);
            let in_town = saved(&app);
            let seen = keepers_as_seen(&mut app);
            assert_ne!(seen, restocked, "seed {s}: the apothecary sold something");

            let mut carried_on = crate::testing::continued(&in_town);
            assert_eq!(crate::testing::here(&carried_on), TOWN, "seed {s}: continued in Mereholt");
            assert_eq!(keepers_as_seen(&mut carried_on), seen, "seed {s}: the same keepers, shelves and purses");
            assert_eq!(*carried_on.world().resource::<Shelves>(), Shelves { stocking: 1, tier: 2 }, "seed {s}: and the same stocking");
            let me = adventurer(&mut carried_on);
            let smith = keeper(&mut carried_on, "smith");
            open_trade(&mut carried_on, me, smith);

            let mut from_below = crate::testing::continued(&below);
            assert_eq!(*from_below.world().resource::<Shelves>(), Shelves { stocking: 0, tier: 1 }, "seed {s}: saved before the restock");
            crate::testing::arrive(&mut from_below, TOWN);
            assert_eq!(keepers_as_seen(&mut from_below), restocked, "seed {s}: home, restocked as the run not saved was");
            assert_eq!(*from_below.world().resource::<Shelves>(), Shelves { stocking: 1, tier: 2 }, "seed {s}");
        }
    }

    /// Keepers name what they sell: an unknown potion on the shelf is for
    /// sale at its kind's price, where one of the same kind in the pack is
    /// worth nothing to anybody until its kind is known.
    #[test]
    fn an_unknown_potion_on_a_shelf_has_its_kinds_worth_and_is_for_sale_over_a_span_of_seeds() {
        for s in 0..3 {
            let (mut app, me) = in_mereholt(s);
            let Some((potion, name)) = unknown_potion(&mut app) else { panic!("seed {s}: the apothecary sells no potion not yet known") };
            let price = app.world().resource::<Armory>().defs.get(app.world().resource::<Armory>().defs.expect(&name)).price;
            assert_eq!(app.world().get::<Worth>(potion).map(|w| w.0), price, "seed {s}: {name} on the shelf");
            let mine = make(&mut app, &name);
            give(&mut app, me, mine);
            settle(&mut app);
            assert_eq!(app.world().get::<Worth>(mine), None, "seed {s}: {name} in the pack");
            let apothecary = keeper(&mut app, "apothecary");
            open_trade(&mut app, me, apothecary);
            let view = app.world().resource::<TradeView>();
            let row = view.wares.iter().find(|r| r.row.entity == potion).unwrap_or_else(|| panic!("seed {s}: {name} on the screen"));
            assert_eq!(row.each, price, "seed {s}: {name} priced");
        }
    }

    /// The trade screen lists a ware not yet known by its kind's true name
    /// in the shop's own colour, never by the face or the face's colour it
    /// wears in the pack.
    #[test]
    fn on_the_trade_screen_a_ware_not_yet_known_reads_by_its_true_name_in_the_shops_colour_not_its_face() {
        let (mut app, me) = in_mereholt(2);
        let (potion, name) = unknown_potion(&mut app).expect("the apothecary sells a potion not yet known");
        let apothecary = keeper(&mut app, "apothecary");
        open_trade(&mut app, me, apothecary);
        let face = name_of(&app, potion);
        assert_ne!(face, name, "the potion wears its face");
        let view = app.world().resource::<TradeView>();
        let row = view.wares.iter().find(|r| r.row.entity == potion).expect("on the screen");
        assert_eq!(row.row.label, listed(&name, row.count), "by its true name");
        let color = app.world().resource::<Townsfolk>().role("apothecary").expect("a row").color();
        assert_eq!(row.row.glyph.fg, color, "in the apothecary's colour, not its face's");
    }

    /// Looking over the shelf teaches nothing; buying a potion makes its
    /// kind known, so the pack calls it by its name.
    #[test]
    fn buying_a_potion_makes_its_kind_known_and_browsing_does_not() {
        let (mut app, me) = in_mereholt(2);
        let (potion, name) = unknown_potion(&mut app).expect("the apothecary sells a potion not yet known");
        let apothecary = keeper(&mut app, "apothecary");
        let gold = make(&mut app, "gold coin");
        app.world_mut().get_mut::<Stack>(gold).expect("gold stacks").count = 5000;
        give(&mut app, me, gold);
        open_trade(&mut app, me, apothecary);
        for _ in 0..4 {
            app.update();
        }
        assert!(!app.world().resource::<Lore>().knows(&name), "browsing taught nothing of {name}");
        app.world_mut().write_message(Buy { customer: me, trader: apothecary, item: potion, count: 1 });
        app.update();
        assert!(app.world().resource::<Lore>().knows(&name), "bought, {name} is known");
        app.update();
        assert!(pack(&app, me).iter().any(|(called, _)| *called == name), "and called so in the pack: {:?}", pack(&app, me));
    }

    /// Each keeper's `Trader` buys exactly the kinds `wares.ron` says it
    /// does, by the engine's own rule; and the smith turns a potion away.
    #[test]
    fn each_keeper_buys_only_its_own_kinds_and_the_smith_refuses_a_potion() {
        let library = library();
        for keeper in library.wares.keepers() {
            let trader = Trader { buys: trader_tags(keeper, &library.armory), pays_pct: PAYS_PCT };
            for (_, def) in library.armory.defs.iter().filter(|(_, d)| d.price.is_some()) {
                let tags: Vec<TagId> = def.tags.iter().map(|t| t.id()).collect();
                assert_eq!(trader.buys(&tags), keeper.buys(def), "{} and {}", keeper.name, def.name);
            }
        }
        let (mut app, me) = in_mereholt(0);
        let healing = make(&mut app, "potion of healing");
        give(&mut app, me, healing);
        app.world_mut().resource_mut::<Lore>().learn("potion of healing");
        let smith = keeper(&mut app, "smith");
        open_trade(&mut app, me, smith);
        let view = app.world().resource::<TradeView>();
        let row = view.pack.iter().find(|r| r.row.entity == healing).expect("the potion on the pack's side");
        assert_eq!(row.refused, Some(TradeRefusal::NotBought), "the smith buys no potions");
        app.world_mut().write_message(Sell { customer: me, trader: smith, item: healing, count: 1 });
        app.update();
        assert!(app.world().get::<Inventory>(me).expect("a pack").contains(healing), "and the potion stays in the pack");
    }

    /// An artifact sold to the curio fetches a quarter of its 1600, the
    /// price list's 400, and is never for sale after: on the curio's side
    /// it has no worth, so the shelf does not offer it and a buy of it is
    /// refused.
    #[test]
    fn an_artifact_sold_to_the_curio_fetches_four_hundred_and_is_never_offered_back() {
        let (mut app, me) = in_mereholt(0);
        let crown = make(&mut app, "the Crown of Grubnak");
        give(&mut app, me, crown);
        let curio = keeper(&mut app, "curio");
        open_trade(&mut app, me, curio);
        let gold = |app: &App| -> u32 {
            let world = app.world();
            let coin = world.resource::<Armory>().defs.expect("gold coin");
            let bag = &world.get::<Inventory>(me).expect("a pack").items;
            bag.iter().filter(|i| world.get::<ItemKind>(**i).is_some_and(|k| k.0 == coin)).map(|i| world.get::<Stack>(*i).map_or(1, |s| s.count)).sum()
        };
        let before = gold(&app);
        app.world_mut().write_message(Sell { customer: me, trader: curio, item: crown, count: 1 });
        app.update();
        assert_eq!(gold(&app), before + 400, "a quarter of 1600");
        assert!(app.world().get::<Inventory>(curio).expect("a shelf").contains(crown), "the curio has it");
        app.update();
        assert_eq!(app.world().get::<Worth>(crown), None, "on the curio's side it has no worth");
        let view = app.world().resource::<TradeView>();
        assert!(!view.wares.iter().any(|r| r.row.entity == crown), "and is not on the counter: {:?}", view.wares);
        app.world_mut().write_message(Buy { customer: me, trader: curio, item: crown, count: 1 });
        app.update();
        assert!(app.world().get::<Inventory>(curio).expect("a shelf").contains(crown), "nor can it be bought back");
        assert_eq!(gold(&app), before + 400, "and nothing was paid");
    }

    /// At the first tier the curio's purse is four hundred, an artifact's
    /// price to it, so the one shop that buys the rare can pay for one the
    /// first evening home; the sale takes the purse whole.
    #[test]
    fn at_tier_one_the_curio_pays_four_hundred_for_an_artifact_from_its_own_purse() {
        let (mut app, me) = in_mereholt(0);
        assert_eq!(app.world().resource::<Shelves>().tier, 1, "the first tier");
        let crown = make(&mut app, "the Crown of Grubnak");
        give(&mut app, me, crown);
        let curio = keeper(&mut app, "curio");
        assert_eq!(carried(&app, curio).1, 400, "the curio's purse at tier one");
        open_trade(&mut app, me, curio);
        app.world_mut().write_message(Sell { customer: me, trader: curio, item: crown, count: 1 });
        app.update();
        assert!(app.world().get::<Inventory>(curio).expect("a shelf").contains(crown), "the curio bought it");
        assert_eq!(carried(&app, curio).1, 0, "with all four hundred of its purse");
    }

    /// What the vitals say of the gold carried, and the tone it is said in.
    fn gold_said(app: &App) -> Vec<(String, ToneId)> {
        let key = app.world().resource::<Facets>().get(GOLD_TONE);
        let view = app.world().resource::<VitalsView>();
        view.facets.iter().filter(|f| Some(f.key) == key).map(|f| (f.text.clone(), f.tone)).collect()
    }

    /// The vitals say the fifty gold the adventurer sets out with, in the
    /// coin's own tone; a pile taken up is counted at once, and a ware
    /// bought counts it down by the ware's price.
    #[test]
    fn the_vitals_say_the_gold_carried_and_follow_a_pickup_and_a_purchase_over_a_span_of_seeds() {
        for s in 0..3 {
            let mut app = headless(RunSeed(s));
            app.add_plugins(VitalsViewPlugin);
            let me = adventurer(&mut app);
            settle(&mut app);
            let tone = app.world().resource::<Tones>().get(GOLD_TONE).expect("the coin's tone is declared");
            assert_eq!(gold_said(&app), [("gold 50".to_string(), tone)], "seed {s}: what the adventurer sets out with");

            let pile = make(&mut app, "gold coin");
            app.world_mut().get_mut::<Stack>(pile).expect("gold stacks").count = 500;
            let here = at(&app, me);
            lay(&mut app, pile, here);
            take_up(&mut app, me);
            assert_eq!(gold_said(&app), [("gold 550".to_string(), tone)], "seed {s}: a pile taken up");

            let smith = keeper(&mut app, "smith");
            open_trade(&mut app, me, smith);
            let (item, _) = shelf(&app, smith)[0];
            let price = app.world().get::<Worth>(item).expect("a ware has a worth").0;
            app.world_mut().write_message(Buy { customer: me, trader: smith, item, count: 1 });
            settle(&mut app);
            assert_eq!(gold_said(&app), [(format!("gold {}", 550 - price), tone)], "seed {s}: a ware bought for {price}");
        }
    }
}
