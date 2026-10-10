//! The temple of the Still Water: Mereholt's priest, who mends, unbinds
//! and names for a little gold.
//!
//! The priest is one of [`shops`](crate::shops)'s keepers, the one whose
//! post is [`TEMPLE`], and offers the engine's `trade` as every keeper
//! does, and beside it the three rites of [`Rites`], `assets/rites.ron`:
//! `mend`, `unbind` and `name`, each an [`Offering::priced`] at the price
//! the file gives it, the same all run and never out of stock. A rite is a
//! verb, not a thing: nothing is spawned for an instant to be bought and
//! used, so nothing about it can end up in a bag, a save or the score.
//!
//! The engine does the asking and the paying. A priced offer is never
//! taken up by walking into the priest, so a bump opens the offers screen
//! with each rite's price on its row; one the adventurer cannot pay for is
//! greyed there and refused; and one taken up has its price paid into the
//! priest's purse before the game hears of it. [`answer_rites`] is the
//! rest: it lands the rite's effects on whoever took it up, through the
//! same effects a scroll or a potion lands, written in the same words in
//! the file, so the temple's `unbind` and the scroll of remove curse are
//! one `RemoveCurse` and cannot drift apart.
//!
//! The rites are the priest's offers, which the engine does not save, so
//! [`townsfolk::spawn_person`](crate::townsfolk::spawn_person) puts them on
//! whenever a priest is made, since the priest's row in `townsfolk.ron`
//! offers `Rites`: when Mereholt is first entered and when a continued run
//! puts the people back.

use bevy::ecs::message::MessageCursor;
use bevy::ecs::system::SystemState;
use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_bevy::Source;
use rl_engine::rl_ui::view::offers::OffersView;
use rl_engine::rl_ui::view::sum_of;
use ron::options::Options;
use serde::Deserialize;

const RITES_RON: &str = include_str!("../assets/rites.ron");

/// The keeper in `wares.ron` who keeps the temple, whose offers are the
/// rites as well as `trade`.
pub const TEMPLE: &str = "temple";

/// What taking up a rite costs, in hundredths of a step: one step, the
/// turn the walk up to the altar would have been, as `trade` costs.
pub const RITE_TIME: u32 = 100;

/// One rite: the verb the priest offers it by, what it costs, and what it
/// lands on whoever takes it up.
pub struct Rite {
    /// The verb, as `Verbs` interns it.
    pub verb: String,
    /// What the offers screen lists it as, by [`rite_rows`].
    pub says: String,
    /// Its price in gold coins, more than nought.
    pub price: u32,
    /// What the priest is said to find when there is nothing for it to
    /// do, by [`answer_rites`], who gives the price back.
    pub idle: String,
    /// What it lands, built once from the file.
    pub effects: Effects,
    /// The effect words it lands, as the file writes them: what
    /// [`Rite::finds_work`] asks of each.
    words: Vec<String>,
}

impl Rite {
    /// Whether taking the rite up would do anything for `taker`: true
    /// when any word it lands has something to do, a wound to close, a
    /// status that lasts until cured, a worn thing that will not come off
    /// or one not known whole. A word this cannot judge is taken to have
    /// something to do, so a rite is never refused for a word it does not
    /// know.
    pub fn finds_work(&self, world: &World, taker: Entity) -> bool {
        use crate::effects::mending::{CureLasting, Whole, lasting, wounded};
        self.words.iter().any(|word| match word.as_str() {
            Whole::KIND => wounded(world, taker),
            CureLasting::KIND => !lasting(world, taker).is_empty(),
            crate::enchant::RemoveCurse::KIND => !crate::enchant::held_on(world, taker).is_empty(),
            crate::lore::NameWorn::KIND => !crate::lore::untold_worn(world, taker).is_empty(),
            _ => true,
        })
    }
}

/// Every rite the priest offers, in the file's order, which is the order
/// the offers screen lists them in after `trade`.
///
/// Read once with the rest of the content and never changed by a run.
#[derive(Resource, Default)]
pub struct Rites(Vec<Rite>);

impl Rites {
    /// Reads `assets/rites.ron` against `registries` and the effect
    /// `kinds` the app registered. Panics naming the file and every
    /// problem in it.
    pub fn load(registries: &Registries, kinds: &EffectKinds) -> Self {
        Self::read(RITES_RON, registries, kinds).unwrap_or_else(|e| panic!("assets/rites.ron: {e}"))
    }

    /// Reads a rites file, reporting every problem at once, each under its
    /// verb: a verb written twice or one that is the engine's `trade`, a
    /// price of nought, a rite that lands nothing, and every effect that
    /// does not build.
    pub fn read(text: &str, registries: &Registries, kinds: &EffectKinds) -> Result<Self, ContentError> {
        let authored: Vec<AuthoredRite> = Options::default().from_str(text).map_err(|e| ContentError::Parse(e.to_string()))?;
        let names = registries.names();
        let (mut rites, mut errors): (Vec<Rite>, Vec<String>) = (Vec::new(), Vec::new());
        for a in authored {
            if rites.iter().any(|r| r.verb == a.verb) {
                errors.push(format!("{}: a second rite of the verb", a.verb));
            }
            if a.verb == TRADE {
                errors.push(format!("{}: the engine's own verb, which every keeper offers already", a.verb));
            }
            if a.price == 0 {
                errors.push(format!("{}: a price of nought, which is no price", a.verb));
            }
            if a.says.trim().is_empty() {
                errors.push(format!("{}: says nothing on the offers screen", a.verb));
            }
            if a.idle.trim().is_empty() {
                errors.push(format!("{}: says nothing when there is nothing to do", a.verb));
            }
            if a.effects.is_empty() {
                errors.push(format!("{}: lands nothing", a.verb));
            }
            let effects = Effects::build(&a.effects, kinds, &names).unwrap_or_else(|mine| {
                errors.extend(mine.into_iter().map(|e| format!("{}: {e}", a.verb)));
                Effects::default()
            });
            let words = a.effects.iter().map(|e| e.kind.clone()).collect();
            rites.push(Rite { verb: a.verb, says: a.says, price: a.price, idle: a.idle, effects, words });
        }
        if !errors.is_empty() {
            return Err(ContentError::Invalid(errors));
        }
        Ok(Self(rites))
    }

    /// Every rite, in the file's order.
    pub fn iter(&self) -> impl Iterator<Item = &Rite> {
        self.0.iter()
    }

    /// The rite offered as `verb`, if one is.
    pub fn get(&self, verb: &str) -> Option<&Rite> {
        self.0.iter().find(|r| r.verb == verb)
    }

    /// What the priest offers for the rites, each at its price, in the
    /// file's order. Every verb must have been declared, which
    /// [`UndermerePlugin`](crate::plugin::UndermerePlugin) does for each
    /// rite as it reads the file.
    pub fn offerings(&self, verbs: &Verbs) -> Vec<Offering> {
        self.0
            .iter()
            .map(|r| {
                let verb = verbs.get(&r.verb).unwrap_or_else(|| panic!("the rite {:?} is declared as a verb", r.verb));
                Offering::new(verb, RITE_TIME).priced(r.price)
            })
            .collect()
    }
}

/// A rite as the file writes it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredRite {
    verb: String,
    says: String,
    price: u32,
    idle: String,
    effects: Vec<EffectSpec>,
}

/// Lists each rite on the offers screen by what it does, its `says` in
/// the file, `name what you wear`, over the engine's verb and the
/// priest's name, `name the priest`, which says the rite is done to the
/// priest. The price and any refusal stay the engine's.
///
/// In `ViewSet::Annotate`, over the engine's rows.
pub fn rite_rows(mut view: ResMut<OffersView>, rites: Res<Rites>, verbs: Res<Verbs>) {
    for row in &mut view.rows {
        if let Some(rite) = rites.get(verbs.name(row.verb)) {
            row.label.clone_from(&rite.says);
        }
    }
}

/// Lands a rite's effects on whoever took it up, as their own use of it:
/// its user and its one target, so a word that lands on the user, as
/// `RemoveCurse` does, and one that lands on its targets, as `Mend` does,
/// both reach them. Credited to the priest as the offer it came from.
///
/// The price is already paid, into the priest's purse, before this runs,
/// and silently, so this says it first, `You give Hesk 30 gold coins.`,
/// by the priest's name this run, in the plain tone the narrator gives
/// the adventurer's own doings, before whatever the rite's effects say.
/// The coins are named as the trade screen names them, by a piece's own
/// name.
///
/// A rite with nothing to do for its taker, by [`Rite::finds_work`], lands
/// nothing and the priest gives the price back, by the engine's own
/// [`pay`](rl_engine::rl_bevy::pay), saying so in the rite's `idle` words
/// and naming only what was handed back,
/// `Hesk finds nothing to mend, and gives back your 30 gold coins.`:
/// the engine takes the price before the answer runs, and a rite that
/// sold the adventurer nothing should cost them nothing. What comes back
/// is the adventurer's own, by
/// [`count_as_owned`](crate::tally::count_as_owned), never a find.
///
/// In `TurnSet::React`, where every answer to `Interacted` is read; every
/// other verb is left for whatever answers it. Exclusive, since whether a
/// rite finds work reads the taker's whole state as the effects that would
/// land on it read it.
pub fn answer_rites(world: &mut World, mut cursor: Local<MessageCursor<Interacted>>) {
    let taken: Vec<Interacted> = cursor.read(world.resource::<Messages<Interacted>>()).copied().collect();
    if taken.is_empty() {
        return;
    }
    world.resource_scope(|world, rites: Mut<Rites>| {
        for ev in taken {
            let Some(rite) = rites.get(world.resource::<Verbs>().name(ev.verb)) else { continue };
            let works = rite.finds_work(world, ev.actor);
            let currency = world.get_resource::<Currency>().copied();
            let back = match currency {
                Some(currency) if !works => rl_engine::rl_bevy::pay(world, ev.target, ev.actor, rite.price, currency),
                _ => 0,
            };
            // The price handed back is the adventurer's own, not a find.
            if back > 0 {
                crate::tally::count_as_owned(world, ev.actor);
            }
            let mut state = SystemState::<(Purse, EffectWorld)>::new(world);
            let Ok((purse, mut effects)) = state.get_mut(world) else { continue };
            let coin = purse.coin_name([ev.target, ev.actor]);
            if works {
                let paid = sum_of(&coin, rite.price);
                effects.commands.write_message(Tell::new(format!("You give {{whom}} {paid}."), Tones::TEXT).to(ev.target));
                let at = effects.position(ev.actor).unwrap_or_default();
                rite.effects.land_on(Source::Offer(ev.target), ev.actor, at, vec![ev.actor], &mut effects);
            } else {
                // Only what was handed back is said to be: the purse the
                // priest was just paid into holds the price, so it is all
                // of it unless something else emptied it first.
                let line = match back {
                    0 => format!("{{Whom}} {}.", rite.idle),
                    _ => format!("{{Whom}} {}, and gives back your {}.", rite.idle, sum_of(&coin, back)),
                };
                effects.commands.write_message(Tell::new(line, Tones::MUTED).to(ev.target));
            }
            state.apply(world);
        }
    });
}

/// Where the currency's name is read from: what each side carries, and
/// every named piece in play.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Purse<'w, 's> {
    currency: Option<Res<'w, Currency>>,
    bags: Query<'w, 's, &'static Inventory>,
    pieces: Query<'w, 's, (Entity, &'static Name, &'static Tagged)>,
}

impl Purse<'_, '_> {
    /// What one piece of the currency is called: a piece in the first of
    /// `near`'s bags that holds one, else any piece in play, the lowest
    /// entity first, as the trade screen finds it. Empty with none, which
    /// `sum_of` says as the bare number.
    fn coin_name(&self, near: [Entity; 2]) -> String {
        let Some(currency) = self.currency.as_deref() else { return String::new() };
        let is = |(_, _, tags): &(Entity, &Name, &Tagged)| currency.is(Some(*tags));
        let carried = near.iter().filter_map(|who| self.bags.get(*who).ok()).flat_map(|bag| bag.items.iter().copied());
        let nearby = carried.filter_map(|e| self.pieces.get(e).ok()).find(is);
        nearby.or_else(|| self.pieces.iter().filter(is).min_by_key(|(e, _, _)| *e)).map(|(_, name, _)| name.as_str().to_string()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_core::RunSeed;
    use rl_engine::rl_rules::UNTIL_CURED;

    use super::*;
    use crate::armory::{Armory, ItemKind};
    use crate::dungeon::{TOWN, map_of};
    use crate::lore::{Lore, Unfamiliar};
    use crate::shops::Shopkeeper;
    use crate::testing::{
        adventurer, arrive, continued, equip, find, give, headless, make, name_of, pass_turns, put_down, said, settle, stand_on, take_up, unequip,
    };

    /// The rites as the spec prices them, in the order the priest offers
    /// them.
    const PRICED: [(&str, u32); 3] = [("mend", 30), ("unbind", 60), ("name", 60)];

    /// A run begun in Mereholt, and its priest.
    fn in_mereholt(seed: u64) -> (App, Entity, Entity) {
        let mut app = headless(RunSeed(seed));
        let me = adventurer(&mut app);
        let priest = priest(&mut app);
        (app, me, priest)
    }

    /// The keeper of the temple.
    fn priest(app: &mut App) -> Entity {
        let world = app.world_mut();
        let found = world.query::<(Entity, &Shopkeeper)>().iter(world).find(|(_, k)| k.0.keeper == TEMPLE).map(|(e, _)| e);
        found.expect("the priest stands in Mereholt")
    }

    /// A cell beside `who` the adventurer could stand on.
    fn beside(app: &mut App, who: Entity) -> Point {
        let at = app.world().get::<Position>(who).expect("standing").0;
        let world = app.world_mut();
        let taken: Vec<Point> =
            world.query_filtered::<(&Position, &OnMap), With<Blocks>>().iter(world).filter(|(_, on)| on.0 == TOWN).map(|(p, _)| p.0).collect();
        let map = app.world().resource::<WorldMap>();
        let mut cells = (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| at.offset(dx, dy)));
        cells.find(|p| map.is_walkable_on(TOWN, *p) && !taken.contains(p)).expect("somewhere to stand beside the priest")
    }

    /// The gold coins `who` carries.
    fn gold(app: &App, who: Entity) -> u32 {
        let world = app.world();
        let coin = world.resource::<Armory>().defs.expect("gold coin");
        let bag = world.get::<Inventory>(who).map(|b| b.items.clone()).unwrap_or_default();
        bag.iter().filter(|i| world.get::<ItemKind>(**i).is_some_and(|k| k.0 == coin)).map(|i| world.get::<Stack>(*i).map_or(1, |s| s.count)).sum()
    }

    /// Leaves the adventurer carrying exactly `coins` gold coins.
    fn set_gold(app: &mut App, me: Entity, coins: u32) {
        let coin = app.world().resource::<Armory>().defs.expect("gold coin");
        let bag = app.world().get::<Inventory>(me).expect("a pack").items.clone();
        let purses: Vec<Entity> = bag.into_iter().filter(|i| app.world().get::<ItemKind>(*i).is_some_and(|k| k.0 == coin)).collect();
        for purse in purses {
            app.world_mut().get_mut::<Inventory>(me).expect("a pack").items.retain(|i| *i != purse);
            app.world_mut().despawn(purse);
        }
        let purse = make(app, "gold coin");
        app.world_mut().get_mut::<Stack>(purse).expect("gold stacks").count = coins;
        give(app, me, purse);
        assert_eq!(gold(app, me), coins);
    }

    /// Stands the adventurer beside the priest and takes up `rite`, as the
    /// offers screen's choice does.
    fn take_rite(app: &mut App, me: Entity, priest: Entity, rite: &str) {
        let cell = beside(app, priest);
        stand_on(app, me, cell);
        let verb = app.world().resource::<Verbs>().get(rite).expect("the rite is a verb");
        app.world_mut().write_message(Intent::new(me, Interact { target: priest, verb }));
        settle(app);
    }

    /// Gives every rite something to do for the adventurer: a wound, and a
    /// cursed dagger on, its level not yet known. Returns the dagger.
    fn needing_every_rite(app: &mut App, me: Entity) -> Entity {
        let cursed = find(app, "dagger", -1);
        equip(app, me, cursed);
        app.world_mut().get_mut::<Health>(me).expect("health").current = 3;
        cursed
    }

    /// Whether `who` wears `thing`.
    fn wears(app: &App, who: Entity, thing: Entity) -> bool {
        app.world().get::<Equipped>(who).is_some_and(|e| e.contains(thing))
    }

    /// The priest offers the engine's `trade` and then the three rites, in
    /// the file's order, each at the price the spec gives it, and every
    /// other keeper offers trade alone.
    #[test]
    fn the_priest_offers_trade_and_each_rite_at_its_price() {
        let (app, _, priest) = in_mereholt(0);
        let verbs = app.world().resource::<Verbs>();
        let mut offered = vec![(TRADE.to_string(), None)];
        offered.extend(PRICED.map(|(verb, price)| (verb.to_string(), Some(price))));
        let offers = app.world().get::<Offers>(priest).expect("the priest offers");
        assert_eq!(offers.0.iter().map(|o| (verbs.name(o.verb).to_string(), o.price)).collect::<Vec<_>>(), offered);
        assert!(offers.0.iter().all(|o| o.needs.is_none()), "nothing asked but gold");
    }

    /// Every rite taken up by an adventurer with gold enough is paid for
    /// in full, out of their purse and into the priest's, and nothing
    /// more is taken.
    #[test]
    fn each_rite_taken_up_spends_its_price_into_the_priests_purse() {
        for (rite, price) in PRICED {
            let (mut app, me, priest) = in_mereholt(1);
            needing_every_rite(&mut app, me);
            set_gold(&mut app, me, 100);
            let theirs = gold(&app, priest);
            take_rite(&mut app, me, priest, rite);
            assert_eq!(gold(&app, me), 100 - price, "{rite}: the adventurer paid");
            assert_eq!(gold(&app, priest), theirs + price, "{rite}: into the priest's purse");
        }
    }

    /// Taking up a rite says what was given and to whom, once, in the plain
    /// tone of the adventurer's own doings and before what the rite itself
    /// says; a rite refused for want of gold says nothing of giving.
    #[test]
    fn a_rite_taken_up_says_the_gold_given_to_the_priest_before_what_it_does() {
        for (rite, price) in PRICED {
            let (mut app, me, priest) = in_mereholt(4);
            set_gold(&mut app, me, 100);
            needing_every_rite(&mut app, me);
            take_rite(&mut app, me, priest, rite);
            let lines: Vec<(String, ToneId)> = app.world().resource::<MessageLog>().iter().map(|l| (l.text.clone(), l.tone)).collect();
            let given = format!("You give {} {price} gold coins.", name_of(&app, priest));
            let at = lines.iter().position(|(t, _)| *t == given).unwrap_or_else(|| panic!("{rite}: {given:?} not in {lines:#?}"));
            assert_eq!(lines[at].1, Tones::TEXT, "{rite}: in the plain tone");
            assert_eq!(lines.iter().filter(|(t, _)| t.starts_with("You give")).count(), 1, "{rite}: said once: {lines:#?}");
            if rite == "mend" {
                let closed = lines.iter().position(|(t, _)| t == "Your wounds close.").expect("the mending is told");
                assert!(at < closed, "the paying before the mending: {lines:#?}");
            }
        }
        let (mut app, me, priest) = in_mereholt(4);
        set_gold(&mut app, me, 10);
        take_rite(&mut app, me, priest, "mend");
        assert!(!said(&app).iter().any(|t| t.starts_with("You give")), "nothing given: {:#?}", said(&app));
    }

    /// A rite with nothing to do, taken up by an adventurer whole, wearing
    /// nothing cursed and knowing all they wear, costs nothing: the price
    /// the engine took is given back, from the priest's purse to theirs,
    /// and the log says so in the rite's own words, in the muted tone, and
    /// never that anything was given or that they were whole already.
    #[test]
    fn a_rite_with_nothing_to_do_costs_nothing_and_says_the_price_was_given_back() {
        let idle = [("mend", "finds nothing to mend"), ("unbind", "finds nothing bound on you"), ("name", "has nothing to tell you of what you wear")];
        for ((rite, price), (_, words)) in PRICED.into_iter().zip(idle) {
            let (mut app, me, priest) = in_mereholt(5);
            set_gold(&mut app, me, 100);
            let theirs = gold(&app, priest);
            take_rite(&mut app, me, priest, rite);
            assert_eq!((gold(&app, me), gold(&app, priest)), (100, theirs), "{rite}: both purses as they were");
            let lines: Vec<(String, ToneId)> = app.world().resource::<MessageLog>().iter().map(|l| (l.text.clone(), l.tone)).collect();
            let back = format!("{} {words}, and gives back your {price} gold coins.", name_of(&app, priest));
            assert!(lines.contains(&(back.clone(), Tones::MUTED)), "{rite}: {back:?} not in {lines:#?}");
            let silent = ["You give", "You were whole already.", "A cleansing light", "You know everything"];
            assert!(!lines.iter().any(|(t, _)| silent.iter().any(|s| t.starts_with(s))), "{rite}: nothing landed: {lines:#?}");
        }
    }

    /// The price a rite with nothing to do hands back is the adventurer's
    /// own, not treasure: paid out of their whole purse and handed back
    /// out of the priest's, then dropped and taken up, it counts nothing.
    #[test]
    fn the_price_a_rite_hands_back_is_never_counted_as_treasure() {
        let (mut app, me, priest) = in_mereholt(5);
        set_gold(&mut app, me, 30);
        let purse = app.world().get::<Inventory>(me).expect("a pack").items.iter().copied().find(|i| name_of(&app, *i) == "gold coin").expect("a purse");
        app.world_mut().entity_mut(purse).insert(crate::tally::Counted(30));
        take_rite(&mut app, me, priest, "mend");
        assert_eq!(gold(&app, me), 30, "the price came back");
        let back = app.world().get::<Inventory>(me).expect("a pack").items.iter().copied().find(|i| name_of(&app, *i) == "gold coin").expect("a purse");
        put_down(&mut app, me, back);
        take_up(&mut app, me);
        assert_eq!(gold(&app, me), 30, "every coin taken up again");
        assert_eq!(app.world().resource::<crate::tally::Tally>().gold_found, 0, "handed back, not found");
    }

    /// A rite the adventurer cannot pay for is listed as wanting its price
    /// and refused: neither purse moves and nothing lands, so a wound
    /// stays open, a curse holds and a level stays unknown.
    #[test]
    fn each_rite_is_refused_when_the_gold_is_short_and_both_purses_are_untouched() {
        for (rite, price) in PRICED {
            let (mut app, me, priest) = in_mereholt(2);
            let cursed = find(&mut app, "dagger", -1);
            equip(&mut app, me, cursed);
            app.world_mut().get_mut::<Health>(me).expect("health").current = 3;
            set_gold(&mut app, me, price - 1);
            let theirs = gold(&app, priest);
            take_rite(&mut app, me, priest, rite);
            let verb = app.world().resource::<Verbs>().get(rite).expect("a verb");
            let offer = app.world().resource::<OfferedHere>().find(me, priest, verb).copied();
            assert_eq!(offer.and_then(|o| o.refused), Some(Refused::Short(price)), "{rite}: wants its price");
            assert_eq!((gold(&app, me), gold(&app, priest)), (price - 1, theirs), "{rite}: neither purse moved");
            assert_eq!(app.world().get::<Health>(me).map(|h| h.current), Some(3), "{rite}: no wound closed");
            assert!(app.world().get::<Stuck>(cursed).is_some(), "{rite}: the curse holds");
            assert!(app.world().get::<Unfamiliar>(cursed).is_some(), "{rite}: its level unknown");
        }
    }

    /// `mend` heals the adventurer whole and cures what would never pass
    /// on its own, and leaves a status with turns to run and one a worn
    /// thing holds, which its thing ends.
    #[test]
    fn mending_fills_health_and_cures_what_lasts_until_cured_but_not_what_runs_out_or_a_worn_thing_holds() {
        let (mut app, me, priest) = in_mereholt(3);
        let worn = app.world().get::<Equipped>(me).and_then(|e| e.0.worn().next()).map(|(_, item)| item).expect("the adventurer wears something");
        let statuses = app.world().resource::<Registries>().statuses.clone();
        let (poisoned, hasted, regenerating) = (statuses.expect("poisoned"), statuses.expect("hasted"), statuses.expect("regenerating"));
        app.world_mut().write_message(Afflict { target: me, status: poisoned, turns: UNTIL_CURED, by: None, held_by: None });
        app.world_mut().write_message(Afflict { target: me, status: hasted, turns: 50, by: None, held_by: None });
        app.world_mut().write_message(Afflict { target: me, status: regenerating, turns: UNTIL_CURED, by: None, held_by: Some(worn) });
        pass_turns(&mut app, 1);
        let held = |app: &App| app.world().get::<Afflicted>(me).map(|a| a.0.iter().map(|s| s.id).collect::<Vec<_>>()).unwrap_or_default();
        assert!([poisoned, hasted, regenerating].iter().all(|s| held(&app).contains(s)), "all three on before the rite");
        app.world_mut().get_mut::<Health>(me).expect("health").current = 3;
        take_rite(&mut app, me, priest, "mend");
        let health = app.world().get::<Health>(me).map(|h| (h.current, h.max)).expect("health");
        assert_eq!(health.0, health.1, "made whole");
        pass_turns(&mut app, 1);
        let now = held(&app);
        assert!(!now.contains(&poisoned), "the poison that would never pass is cured: {now:?}");
        assert!(now.contains(&hasted) && now.contains(&regenerating), "what runs out and what a worn thing holds stay: {now:?}");
    }

    /// `unbind` frees a cursed thing the adventurer wears, which would not
    /// come off before, and it comes off after.
    #[test]
    fn unbinding_lets_a_cursed_worn_thing_come_off() {
        let (mut app, me, priest) = in_mereholt(4);
        let cursed = find(&mut app, "dagger", -1);
        equip(&mut app, me, cursed);
        unequip(&mut app, me, cursed);
        assert!(wears(&app, me, cursed), "the curse holds it on");
        set_gold(&mut app, me, 60);
        take_rite(&mut app, me, priest, "unbind");
        assert!(app.world().get::<Stuck>(cursed).is_none(), "unbound");
        unequip(&mut app, me, cursed);
        assert!(!wears(&app, me, cursed), "and it comes off");
    }

    /// `name` makes everything the adventurer wears known, a cursed thing's
    /// level and so its curse with the rest, and leaves what they carry
    /// and do not wear as unknown as it was.
    #[test]
    fn naming_makes_everything_worn_known_and_leaves_what_is_carried_unknown() {
        let (mut app, me, priest) = in_mereholt(5);
        let (cursed, helm) = (find(&mut app, "dagger", -1), find(&mut app, "helm", 1));
        equip(&mut app, me, cursed);
        equip(&mut app, me, helm);
        let (carried, potion) = (find(&mut app, "scale mail", 1), make(&mut app, "potion of strength"));
        give(&mut app, me, carried);
        give(&mut app, me, potion);
        settle(&mut app);
        assert!(!app.world().resource::<Lore>().knows("potion of strength"), "a potion of strength is not known at the start");
        set_gold(&mut app, me, 60);
        take_rite(&mut app, me, priest, "name");
        for thing in [cursed, helm] {
            assert!(wears(&app, me, thing) && app.world().get::<Unfamiliar>(thing).is_none(), "{} is known", name_of(&app, thing));
        }
        pass_turns(&mut app, 1);
        assert_eq!(name_of(&app, cursed), "-1 dagger", "its curse named with its level");
        assert_eq!(name_of(&app, helm), "+1 helm");
        assert!(app.world().get::<Unfamiliar>(carried).is_some(), "the scale mail carried is not known");
        assert!(!app.world().resource::<Lore>().knows("potion of strength"), "nor the potion carried");
    }

    /// A run saved in Mereholt and continued has a priest offering the
    /// same rites at the same prices, since the engine saves no offer and
    /// the game puts them back.
    #[test]
    fn the_priests_offers_survive_a_save() {
        let (mut app, _, theirs) = in_mereholt(6);
        let offered = app.world().get::<Offers>(theirs).expect("offers").clone();
        arrive(&mut app, map_of(1));
        arrive(&mut app, TOWN);
        let text = app.world().resource::<Saves>().load(crate::save::SLOT).expect("the slot reads").expect("the arrival saved the run");
        let mut carried_on = continued(&text);
        let priest = priest(&mut carried_on);
        assert_eq!(carried_on.world().get::<Offers>(priest), Some(&offered));
    }

    /// The file's rites read, each saying what it does, and a broken one is refused naming each
    /// fault under its verb.
    #[test]
    fn a_rites_file_with_a_second_verb_trade_a_free_rite_or_an_unknown_effect_is_refused_naming_each() {
        let library = crate::content::Library::load(&crate::content::effect_kinds(), &Moments::default());
        assert_eq!(library.rites.iter().map(|r| (r.verb.as_str(), r.price)).collect::<Vec<_>>(), PRICED.to_vec());
        let says: Vec<&str> = library.rites.iter().map(|r| r.says.as_str()).collect();
        assert_eq!(says, ["mend your wounds", "unbind what you wear", "name what you wear"]);
        let text = r#"[
            (verb: "mend", says: "mend", idle: "rests", price: 30, effects: [(kind: "Whole")]),
            (verb: "mend", says: "mend", idle: "rests", price: 30, effects: [(kind: "Whole")]),
            (verb: "trade", says: "trade", idle: "rests", price: 10, effects: [(kind: "Whole")]),
            (verb: "bless", says: " ", idle: "", price: 0, effects: []),
            (verb: "curse", says: "curse", idle: "rests", price: 5, effects: [(kind: "Smite")]),
        ]"#;
        let Err(ContentError::Invalid(errors)) = Rites::read(text, &library.registries, &crate::content::effect_kinds()) else { panic!("refused") };
        let joined = errors.join("\n");
        for said in [
            "mend: a second rite",
            "trade: the engine's own verb",
            "bless: a price of nought",
            "bless: says nothing on",
            "bless: says nothing when",
            "bless: lands nothing",
            "curse: ",
        ] {
            assert!(joined.contains(said), "{said:?} not in:\n{joined}");
        }
    }
}
