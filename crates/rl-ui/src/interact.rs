//! The key that does what is here.
//!
//! Walking into a crate opens it, because a bump into a prop that offers
//! one thing comes to an interaction. What a bump cannot reach is what the
//! player is standing on: a body underfoot, a plate already found. This is
//! the key for that, and for a game whose props do not block at all.
//!
//! It takes the one thing on offer. Where several are offered it asks,
//! by opening the offers screen, if the game added one; a game with no
//! such screen gets nothing rather than a guess.
//!
//! Opt-in with [`InteractKey`], beside the panels: a game with no props
//! adds nothing and the key does not exist.

use bevy::prelude::*;
use rl_bevy::EngineSet;
use rl_bevy::prelude::*;

use crate::controls::{AddControls, Chord, ControlInput};
use crate::modal::Modals;

/// The key that takes what is offered here.
///
/// A default, not a rule: a game that wants another inserts its own.
/// `Enter` is the cursors' confirm key, which is what every other screen
/// in the engine already means by "do the thing in front of me".
#[derive(Resource, Debug, Clone)]
pub struct InteractKeys {
    /// Takes the one offer in reach.
    pub interact: Chord,
}

impl Default for InteractKeys {
    fn default() -> Self {
        Self { interact: Chord::key(KeyCode::Enter) }
    }
}

/// The key that does what is here: reads [`OfferedHere`] and writes the
/// interaction.
///
/// Adds nothing else. What an interaction costs, whether it is possible
/// and what it means are all the engine's, worked out before the key was
/// pressed.
pub struct InteractKey;

impl Plugin for InteractKey {
    fn build(&self, app: &mut App) {
        app.init_resource::<InteractKeys>().add_systems(Update, interact_key.in_set(EngineSet::Input));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::UiPlugin>(app, "InteractKey");
        rl_bevy::depends_on::<PropsPlugin>(app, "InteractKey");
        let key = app.world().get_resource::<InteractKeys>().map(|k| k.interact).unwrap_or_else(|| InteractKeys::default().interact);
        app.add_control(crate::focus::SCREENS_GROUP, "do what is here", [key]);
    }
}

/// Writes the interaction for the one offer in reach.
///
/// Nothing while a screen is up: a key that acted through an open bag
/// would act on what the player cannot see.
pub fn interact_key(
    keys: ControlInput,
    binds: Res<InteractKeys>,
    mut modals: ResMut<Modals>,
    offered: Res<OfferedHere>,
    mut intents: MessageWriter<Intent<Interact>>,
    holding: Query<Entity, (With<Player>, With<MyTurn>)>,
) {
    if modals.any_open() || !binds.interact.just_pressed(keys.input()) {
        return;
    }
    let Ok(player) = holding.single() else { return };
    let mut open = offered.open_to(player);
    let Some(offer) = open.next().copied() else { return };
    if open.next().is_none() {
        intents.write(Intent::new(player, Interact { prop: offer.prop, verb: offer.verb }));
        return;
    }
    // Several: a question, asked by the screen that exists to ask it. A
    // game without one gets nothing, which is better than a guess.
    if let Some(offers) = modals.get(crate::panel::offers::OFFERS_MODAL) {
        modals.open(offers);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use rl_bevy::props::Stocked;
    use rl_bevy::testing::TEST_SEED;

    /// A player standing on a body that offers one thing, with a bench
    /// that offers two three cells off, out of reach until it is walked to.
    fn stage() -> (Stage, Entity, Entity) {
        let mut stage = Stage::new_with(InteractKey, |app| {
            app.add_plugins(PropsPlugin);
            app.insert_resource(Seed(TEST_SEED));
            app.add_verb("strip");
            app.add_verb("tip");
        });
        let props = rl_rules::prop::load(
            r#"#![enable(implicit_some)]
            [
                (name: "droid wreck", glyph: '%', color: (r: 120, g: 120, b: 130), offers: [(verb: "strip", time: 100)]),
                (name: "workbench", glyph: 'T', color: (r: 150, g: 120, b: 90), blocks: true,
                 offers: [(verb: "strip", time: 100), (verb: "tip", time: 100)]),
            ]"#,
            &rl_rules::Names::new(),
        )
        .expect("the props load");
        stage.app.world_mut().resource_mut::<Registries>().props = props.clone();
        let registries = stage.app.world().resource::<Registries>().clone();
        let (under, beside) = {
            let (wreck, bench) = (props.expect("droid wreck"), props.expect("workbench"));
            let at = stage.at;
            let mut commands = stage.app.world_mut().commands();
            (spawn_prop(&mut commands, &registries, wreck, at, MapId::SURFACE), spawn_prop(&mut commands, &registries, bench, at.offset(3, 0), MapId::SURFACE))
        };
        stage.app.world_mut().flush();
        stage.app.world_mut().entity_mut(under).insert(Stocked);
        stage.tick();
        (stage, under, beside)
    }

    /// The key is for what a bump cannot reach: what you are standing on.
    #[test]
    fn the_key_takes_the_one_offer_underfoot() {
        let (mut stage, under, _) = stage();
        #[derive(Resource, Default)]
        struct Seen(Vec<Interacted>);
        stage.app.init_resource::<Seen>().add_systems(Turn, |mut seen: ResMut<Seen>, mut done: MessageReader<Interacted>| {
            seen.0.extend(done.read().copied());
        });

        stage.press(KeyCode::Enter);
        stage.tick();
        let seen = &stage.app.world().resource::<Seen>().0;
        assert_eq!(seen.len(), 1, "the one thing underfoot was taken up: {seen:?}");
        assert_eq!(seen[0].prop, under);
    }

    /// Two offers in reach are a question, and a key is not a question.
    #[test]
    fn the_key_takes_nothing_when_two_things_are_offered() {
        let (mut stage, under, beside) = stage();
        // Beside the bench, which offers two, and off the body.
        stage.app.world_mut().entity_mut(under).despawn();
        let at = stage.app.world().get::<Position>(beside).expect("the bench stands somewhere").0;
        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert(Position(at.offset(-1, 0)));
        stage.tick();
        #[derive(Resource, Default)]
        struct Seen(Vec<Interacted>);
        stage.app.init_resource::<Seen>().add_systems(Turn, |mut seen: ResMut<Seen>, mut done: MessageReader<Interacted>| {
            seen.0.extend(done.read().copied());
        });

        stage.press(KeyCode::Enter);
        stage.tick();
        assert!(stage.app.world().resource::<Seen>().0.is_empty(), "nothing was guessed at");
    }

    /// A key that acted through an open screen would act on what the
    /// player cannot see.
    #[test]
    fn the_key_does_nothing_while_a_screen_is_up() {
        let (mut stage, _, _) = stage();
        let modal = stage.app.world_mut().resource_mut::<Modals>().declare("something");
        stage.app.world_mut().resource_mut::<Modals>().open(modal);
        #[derive(Resource, Default)]
        struct Seen(Vec<Interacted>);
        stage.app.init_resource::<Seen>().add_systems(Turn, |mut seen: ResMut<Seen>, mut done: MessageReader<Interacted>| {
            seen.0.extend(done.read().copied());
        });

        stage.press(KeyCode::Enter);
        stage.tick();
        assert!(stage.app.world().resource::<Seen>().0.is_empty(), "the key was the screen's, not the world's");
    }
}
