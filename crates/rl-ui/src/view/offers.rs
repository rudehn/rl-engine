//! What you could do here, when here is more than one thing.
//!
//! The engine works out every offer in reach of whoever holds the turn,
//! with what it would cost and why it cannot be taken up. This is that
//! list as plain rows, for the screen that asks which one the player
//! meant.
//!
//! A row's words are the verb's registered name and the `Name` of what
//! offers it, said the way the look panel lists it, so the engine still
//! invents nothing: a game that calls its verb `charge` and its prop
//! `reactor console` gets "charge reactor console", one whose captive is
//! `Mirela` gets "free Mirela", and a game that wants other words names
//! them other things. A price is named by what a piece of the currency is
//! called, as the trade screen names it, so a game whose piece is `scrip`
//! is asked for "50 scrips" on both screens.

use bevy::prelude::*;
use rl_bevy::prelude::*;

/// One thing the player could do here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferRow {
    /// What it would be done to: a prop, or someone standing by.
    pub target: Entity,
    /// Which of the things it offers.
    pub verb: VerbId,
    /// What it reads as: the verb, then what it is done to.
    pub label: String,
    /// What the turn would cost, in hundredths of a step.
    pub time: u32,
    /// What it would cost in the currency, when it has a price.
    pub price: Option<u32>,
    /// Why it cannot be taken up, in words, when it cannot.
    pub refused: Option<String>,
}

impl OfferRow {
    /// Whether it can be taken up at all.
    pub fn open(&self) -> bool {
        self.refused.is_none()
    }
}

/// Everything in reach of whoever holds the turn.
#[derive(Resource, Debug, Default)]
pub struct OffersView {
    /// Whose reach, while somebody holds the turn.
    pub actor: Option<Entity>,
    /// The offers, in the order the engine worked them out.
    pub rows: Vec<OfferRow>,
    /// What one piece of the currency is called, as the trade screen calls
    /// it: the `Name` of a piece the player or someone offering carries,
    /// else of any piece in play; empty when no piece has a name. Said
    /// with [`sum_of`](crate::view::sum_of).
    pub currency: String,
}

impl OffersView {
    /// The row at `index`, for a menu cursor.
    pub fn row(&self, index: usize) -> Option<&OfferRow> {
        self.rows.get(index)
    }

    /// How many can be taken up.
    pub fn open_rows(&self) -> usize {
        self.rows.iter().filter(|r| r.open()).count()
    }
}

/// Keeps [`OffersView`] current.
pub struct OffersViewPlugin;

impl Plugin for OffersViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OffersView>().add_systems(Update, collect_offers.in_set(crate::ViewSet::Collect));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::UiPlugin>(app, "OffersViewPlugin");
        rl_bevy::depends_on::<PropsPlugin>(app, "OffersViewPlugin");
    }
}

/// What a price is paid in and what a piece of it is called, as the
/// collector looks it up: the currency, every named piece, and the packs
/// a piece near at hand is carried in.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Purses<'w, 's> {
    currency: Option<Res<'w, Currency>>,
    pieces: crate::view::NamedPieces<'w, 's>,
    bags: Query<'w, 's, &'static Inventory>,
}

/// Fills [`OffersView`] from what the engine offered the player.
pub fn collect_offers(
    mut view: ResMut<OffersView>,
    offered: Res<OfferedHere>,
    verbs: Res<Verbs>,
    registries: Option<Res<Registries>>,
    player: Query<Entity, With<Player>>,
    named: Query<&Name>,
    purses: Purses,
) {
    view.rows.clear();
    view.actor = None;
    view.currency.clear();
    let Ok(player) = player.single() else { return };
    if offered.actor() != Some(player) {
        return;
    }
    view.actor = Some(player);
    // Only worth looking for when something here has a price.
    if offered.for_actor(player).iter().any(|o| o.price.is_some()) {
        let Purses { currency, pieces, bags } = &purses;
        let near = [player].into_iter().chain(offered.for_actor(player).iter().map(|o| o.target));
        let carried = near.filter_map(|who| bags.get(who).ok()).flat_map(|bag| bag.items.iter().copied()).collect::<Vec<_>>();
        view.currency = crate::view::currency_name(currency.as_deref(), carried, pieces);
    }
    let currency = view.currency.clone();
    for offer in offered.for_actor(player) {
        let what = named.get(offer.target).map(|n| rl_core::noun::listed(n.as_str(), 1)).unwrap_or_default();
        let label = if what.is_empty() { verbs.name(offer.verb).to_string() } else { format!("{} {what}", verbs.name(offer.verb)) };
        let refused = offer.refused.map(|why| match why {
            Refused::Needs(tag) => match registries.as_deref() {
                Some(registries) => format!("needs a {}", registries.tags.name(tag)),
                None => "needs something you do not carry".to_string(),
            },
            Refused::Short(price) => format!("needs {}", crate::view::sum_of(&currency, price)),
        });
        view.rows.push(OfferRow { target: offer.target, verb: offer.verb, label, time: offer.time, price: offer.price, refused });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use rl_bevy::props::Stocked;
    use rl_bevy::testing::TEST_SEED;
    use rl_rules::{Registry, TagDef};

    /// The rows are the engine's offers in the engine's words, with a
    /// refusal that names what is wanted.
    #[test]
    fn the_rows_read_as_the_verb_and_what_it_is_done_to_and_say_why_not() {
        let mut stage = Stage::new_with(OffersViewPlugin, |app| {
            app.add_plugins(PropsPlugin);
            app.insert_resource(Seed(TEST_SEED));
            let tags = Registry::from_defs(vec![TagDef::new("cutter")]).expect("one tag");
            let props = rl_rules::prop::load(
                r#"#![enable(implicit_some)]
                [
                    (name: "supply crate", glyph: '&', color: (r: 1, g: 2, b: 3), blocks: true,
                     container: (contents: []), offers: [(verb: "open", time: 200)]),
                    (name: "locked cache", glyph: '&', color: (r: 1, g: 2, b: 3), blocks: true,
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
        let (crate_id, cache_id) = (registries.props.expect("supply crate"), registries.props.expect("locked cache"));
        let at = stage.at;
        {
            let mut commands = stage.app.world_mut().commands();
            spawn_prop(&mut commands, &registries, crate_id, at.offset(1, 0), MapId::SURFACE);
            spawn_prop(&mut commands, &registries, cache_id, at.offset(0, 1), MapId::SURFACE);
        }
        stage.app.world_mut().flush();
        let mut props = stage.app.world_mut().query_filtered::<Entity, With<Prop>>();
        let all: Vec<Entity> = props.iter(stage.app.world()).collect();
        for prop in all {
            stage.app.world_mut().entity_mut(prop).insert(Stocked);
        }
        stage.tick();
        stage.tick();

        let view = stage.app.world().resource::<OffersView>();
        let labels: Vec<&str> = view.rows.iter().map(|r| r.label.as_str()).collect();
        assert!(labels.contains(&"open supply crate"), "the verb, then what it is done to: {labels:?}");
        assert!(labels.contains(&"open locked cache"), "{labels:?}");
        let cache = view.rows.iter().find(|r| r.label == "open locked cache").expect("listed");
        assert_eq!(cache.refused.as_deref(), Some("needs a cutter"), "and a refusal names what is wanted");
        assert_eq!(view.open_rows(), 1, "one of the two can be taken up");
    }

    /// A priced row carries its price, and one the player cannot pay says
    /// how much it wants in the currency's own name, as a need names its
    /// tag.
    #[test]
    fn a_priced_row_carries_its_price_and_one_the_player_cannot_pay_says_how_much_it_wants() {
        let mut stage = Stage::new_with(OffersViewPlugin, |app| {
            app.add_plugins(PropsPlugin).add_verb("bless");
            app.insert_resource(Seed(TEST_SEED));
            let tags = Registry::from_defs(vec![TagDef::new("scrip")]).expect("one tag");
            app.insert_resource(Currency(tags.expect("scrip")));
            app.world_mut().resource_mut::<Registries>().tags = tags;
        });
        let bless = stage.app.world().resource::<Verbs>().get("bless").expect("declared");
        let scrip = stage.app.world().resource::<Registries>().tags.expect("scrip");
        let (at, player) = (stage.at, stage.player);
        let w = stage.app.world_mut();
        let coins = w.spawn((Item, Name::new("scrip"), Tagged(vec![scrip]), Stack { key: 1, count: 30 })).id();
        w.entity_mut(player).insert(Inventory { items: vec![coins] });
        let cheap =
            w.spawn((Actor, Position(at.offset(1, 0)), OnMap(MapId::SURFACE), Name::new("Odda"), Offers(vec![Offering::new(bless, 100).priced(20)]))).id();
        let dear =
            w.spawn((Actor, Position(at.offset(-1, 0)), OnMap(MapId::SURFACE), Name::new("Mirela"), Offers(vec![Offering::new(bless, 100).priced(50)]))).id();
        stage.tick();
        stage.tick();

        let view = stage.app.world().resource::<OffersView>();
        let row = |who: Entity| view.rows.iter().find(|r| r.target == who).map(|r| (r.price, r.refused.clone())).expect("listed");
        assert_eq!(row(cheap), (Some(20), None), "within the purse, open with its price");
        assert_eq!(row(dear), (Some(50), Some("needs 50 scrips".to_string())), "past it, refused saying how much");
        assert_eq!(view.currency, "scrip", "named by the piece, as the trade screen names it");
    }

    /// Someone standing by who offers something is listed by their own
    /// name as the look panel lists it, with no word for a prop in it.
    #[test]
    fn someone_who_offers_is_listed_by_the_name_the_look_panel_gives_them() {
        let mut stage = Stage::new_with(OffersViewPlugin, |app| {
            app.add_plugins(PropsPlugin).add_verb("free");
            app.insert_resource(Seed(TEST_SEED));
        });
        let free = stage.app.world().resource::<Verbs>().get("free").expect("declared");
        let at = stage.at.offset(1, 0);
        let captive =
            stage.app.world_mut().spawn((Actor, Position(at), OnMap(MapId::SURFACE), Name::new("Mirela"), Offers(vec![Offering::new(free, 300)]))).id();
        stage.tick();
        stage.tick();

        let view = stage.app.world().resource::<OffersView>();
        let row = view.rows.iter().find(|r| r.target == captive).expect("she is listed");
        assert_eq!(row.label, "free Mirela");
        assert_eq!(row.time, 300);
    }
}
