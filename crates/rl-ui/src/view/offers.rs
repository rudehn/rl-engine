//! What you could do here, when here is more than one thing.
//!
//! The engine works out every offer in reach of whoever holds the turn,
//! with what it would cost and why it cannot be taken up. This is that
//! list as plain rows, for the screen that asks which one the player
//! meant.
//!
//! A row's words are the verb's registered name and the prop's own
//! `Name`, so the engine still invents nothing: a game that calls its
//! verb `charge` and its prop `reactor console` gets "charge reactor
//! console", and a game that wants other words names them other things.

use bevy::prelude::*;
use rl_bevy::prelude::*;

/// One thing the player could do here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferRow {
    /// What it would be done to.
    pub prop: Entity,
    /// Which of the things it offers.
    pub verb: VerbId,
    /// What it reads as: the verb, then what it is done to.
    pub label: String,
    /// What the turn would cost, in hundredths of a step.
    pub time: u32,
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

/// Fills [`OffersView`] from what the engine offered the player.
pub fn collect_offers(
    mut view: ResMut<OffersView>,
    offered: Res<OfferedHere>,
    verbs: Res<Verbs>,
    registries: Option<Res<Registries>>,
    player: Query<Entity, With<Player>>,
    named: Query<&Name>,
) {
    view.rows.clear();
    view.actor = None;
    let Ok(player) = player.single() else { return };
    if offered.actor() != Some(player) {
        return;
    }
    view.actor = Some(player);
    for offer in offered.for_actor(player) {
        let what = named.get(offer.prop).map(|n| rl_core::noun::listed(n.as_str(), 1)).unwrap_or_default();
        let label = if what.is_empty() { verbs.name(offer.verb).to_string() } else { format!("{} {what}", verbs.name(offer.verb)) };
        let refused = offer.refused.map(|why| match why {
            Refused::Needs(tag) => match registries.as_deref() {
                Some(registries) => format!("needs a {}", registries.tags.name(tag)),
                None => "needs something you do not carry".to_string(),
            },
        });
        view.rows.push(OfferRow { prop: offer.prop, verb: offer.verb, label, time: offer.time, refused });
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
}
