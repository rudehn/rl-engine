//! The key that does what is here.
//!
//! Walking into a crate opens it, because a bump into a prop that offers
//! one thing comes to an interaction. What a bump cannot reach is what the
//! player is standing on: a body underfoot, a plate already found. This is
//! the key for that, and for a game whose props do not block at all.
//!
//! It takes the one thing on offer. Where several are offered it asks,
//! by opening the offers screen, if the game added one; a game with no
//! such screen gets nothing rather than a guess. The one thing on offer
//! that asks first is asked about on that screen, its question up at once,
//! and the one that chooses first opens the bag choosing for it, as
//! [`InventoryPanel`](crate::InventoryPanel) says.
//!
//! Part of [`UiPlugin`](crate::UiPlugin) in a game with props: a game
//! that did not add `PropsPlugin` has nothing on offer, and the key does
//! not exist.
//!
//! One press is one action. A game may bind the same key to its way
//! through, as Enter is the obvious key for both; the press that takes the
//! stairs then is not also a reach for what is beside them, so the key
//! stands aside, as [`interact_key`] says, reading [`WayAsked`].

use bevy::prelude::*;
use rl_bevy::EngineSet;
use rl_bevy::prelude::*;

use crate::controls::{AddControls, Chord, ControlInput};
use crate::modal::Modals;
use crate::panel::offers::{Confirming, OffersMenu};

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

/// The key that does what is here, in [`UiPlugin`](crate::UiPlugin)'s
/// `finish` when the game added `PropsPlugin`, so the order the two were
/// added in does not matter: reads [`OfferedHere`] and writes the
/// interaction.
///
/// Adds nothing else. What an interaction costs, whether it is possible
/// and what it means are all the engine's, worked out before the key was
/// pressed.
pub(crate) fn finish(app: &mut App) {
    if !app.is_plugin_added::<PropsPlugin>() {
        return;
    }
    // After the input phase rather than in it, so every game key read from
    // the same press has written what it asked for: see `interact_key`.
    // Gated on play as that phase is, since outside the set it does not
    // inherit its run condition.
    app.init_resource::<InteractKeys>()
        .add_systems(Update, interact_key.after(EngineSet::Input).before(EngineSet::Turns).run_if(in_state(EngineState::Playing)));
    let key = app.world().resource::<InteractKeys>().interact;
    app.add_control(crate::focus::SCREENS_GROUP, "do what is here", [key]);
}

/// Whether the player asked this frame to go through a way it stands on.
///
/// Read from the [`GoThrough`] a game's input wrote, since which key takes
/// a way is the game's to bind and the engine cannot see it otherwise. The
/// way underfoot matters too: a game that writes a [`GoThrough`] for every
/// press of its key, way or none, leaves it refused off one, and that
/// press took nothing, so the key should still reach.
#[derive(bevy::ecs::system::SystemParam)]
pub struct WayAsked<'w, 's> {
    asked: MessageReader<'w, 's, Intent<GoThrough>>,
    standing: Query<'w, 's, (&'static Position, Option<&'static OnMap>)>,
    ways: Query<'w, 's, (&'static Position, Option<&'static OnMap>), With<Transition>>,
}

impl WayAsked<'_, '_> {
    /// Whether `player` asked to go through a way on its own cell.
    ///
    /// Reads every ask written since the last call, with a player or
    /// without one, so none is left over to be mistaken for a later press's.
    pub fn by(&mut self, player: Option<Entity>) -> bool {
        let mut asked = false;
        for intent in self.asked.read() {
            asked |= Some(intent.actor) == player;
        }
        let Some((at, on)) = player.and_then(|p| self.standing.get(p).ok()) else { return false };
        let map = |on: Option<&OnMap>| on.map(|m| m.0).unwrap_or(MapId::SURFACE);
        asked && self.ways.iter().any(|(way, way_on)| way.0 == at.0 && map(way_on) == map(on))
    }
}

/// Writes the interaction for the one offer in reach.
///
/// Nothing while a screen is up: a key that acted through an open bag
/// would act on what the player cannot see.
///
/// Nothing either when the same press asked to take a way through, which
/// a game binding its way to this key does. Both read the press in the
/// same frame, before the turn resolves either, so the reach would be for
/// the offers beside the way being left: two of them opened a screen that
/// stood over the new floor, and one was a second ask from one key.
/// So this runs after the input phase, where whatever the game asked for
/// has been written, and stands aside for the way. For a way only: a game
/// binding another action of its own to this key still writes two asks.
pub fn interact_key(
    keys: ControlInput,
    binds: Res<InteractKeys>,
    mut screens: Screens,
    offered: Res<OfferedHere>,
    mut intents: MessageWriter<Intent<Interact>>,
    mut way: WayAsked,
    holding: Query<Entity, (With<Player>, With<MyTurn>)>,
) {
    // Before any early return, so an ask from a frame the key was not
    // pressed is never mistaken for one from a frame it was.
    let took_way = way.by(holding.single().ok());
    let Screens { modals, menu, commands } = &mut screens;
    if took_way || modals.any_open() || !binds.interact.just_pressed(keys.input()) {
        return;
    }
    let Ok(player) = holding.single() else { return };
    let mut open = offered.open_to(player);
    let Some(offer) = open.next().cloned() else { return };
    let lone = open.next().is_none();
    match (lone, &offer.asks) {
        (true, None) => {
            intents.write(Intent::new(player, Interact { target: offer.target, verb: offer.verb, thing: None }));
            return;
        }
        // Chosen for in the bag, with no screen between: the one offer
        // needs no question but which thing.
        (true, Some(AsksFirst::Thing { prompt, among })) => {
            crate::panel::inventory::choose_for_offer(commands, player, offer.target, offer.verb, prompt.clone(), *among);
            return;
        }
        _ => {}
    }
    // Several, or one that asks first: a question, asked by the screen
    // that exists to ask it, on the lone offer's own question at once. A
    // game without one gets nothing, which is better than a guess.
    let Some(offers) = modals.get(crate::panel::offers::OFFERS_MODAL) else { return };
    modals.open(offers);
    if let (true, Some(menu)) = (lone, menu.as_deref_mut()) {
        menu.confirming = Some(Confirming { target: offer.target, verb: offer.verb, yes: false });
    }
}

/// The screens the interact key opens: the stack, the offers screen's own
/// state, for a lone offer that asks first, when the game added it, and
/// the commands that open the bag for a lone offer that chooses first.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Screens<'w, 's> {
    modals: ResMut<'w, Modals>,
    menu: Option<ResMut<'w, OffersMenu>>,
    commands: Commands<'w, 's>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use rl_bevy::props::Stocked;
    use rl_bevy::testing::TEST_SEED;
    use rl_core::Point;

    /// A player standing on a body that offers one thing, with a bench
    /// that offers two three cells off, out of reach until it is walked to.
    fn stage() -> (Stage, Entity, Entity) {
        let mut stage = Stage::new_with((), |app| {
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
        assert_eq!(seen[0].target, under);
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

    /// A game's own input that takes the way underfoot on Enter, the
    /// interact key, as one binding its stairs to Enter does: for every
    /// press, way or none, and so refused off one.
    fn way_on_enter(keys: Res<ButtonInput<KeyCode>>, holding: Query<Entity, (With<Player>, With<MyTurn>)>, mut ways: MessageWriter<Intent<GoThrough>>) {
        if keys.just_pressed(KeyCode::Enter)
            && let Ok(player) = holding.single()
        {
            ways.write(Intent::new(player, GoThrough));
        }
    }

    /// Every interaction that happened, recorded from now on.
    #[derive(Resource, Default)]
    struct Done(Vec<Interacted>);

    /// The stage with the game's way on Enter, the offers screen declared
    /// as a game with one has it, and the interactions recorded.
    fn shared_key() -> (Stage, Entity, Entity) {
        let (mut stage, under, beside) = stage();
        stage.app.add_systems(Update, way_on_enter.in_set(EngineSet::Input));
        stage.app.world_mut().resource_mut::<Modals>().declare(crate::panel::offers::OFFERS_MODAL);
        stage.app.init_resource::<Done>().add_systems(Turn, |mut done: ResMut<Done>, mut read: MessageReader<Interacted>| {
            done.0.extend(read.read().copied());
        });
        (stage, under, beside)
    }

    /// A way on `from` down to `to`, on the same ground for brevity.
    fn way(stage: &mut Stage, from: Point, to: Point) {
        stage.app.world_mut().spawn((Position(from), Transition { to: Destination::Surface(to) }));
        stage.tick();
    }

    /// The press that took the stairs is not also a reach for the two
    /// things beside them, which would open a screen over the far side.
    #[test]
    fn a_press_of_the_shared_key_on_a_way_travels_and_opens_no_offers_on_arrival() {
        let (mut stage, under, bench) = shared_key();
        stage.app.world_mut().entity_mut(under).despawn();
        let at = stage.app.world().get::<Position>(bench).expect("the bench stands somewhere").0;
        let (from, landing) = (at.offset(-1, 0), at.offset(-7, 0));
        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert(Position(from));
        // A second bench beside the landing, so the far side offers two as
        // well and a screen opened there would have something to show.
        let registries = stage.app.world().resource::<Registries>().clone();
        let kind = registries.props.expect("workbench");
        let mut commands = stage.app.world_mut().commands();
        spawn_prop(&mut commands, &registries, kind, landing.offset(-1, 0), MapId::SURFACE);
        stage.app.world_mut().flush();
        way(&mut stage, from, landing);

        stage.press(KeyCode::Enter);
        stage.tick();
        let w = stage.app.world();
        assert_eq!(w.get::<Position>(player).map(|p| p.0), Some(landing), "the press took the way");
        assert!(!w.resource::<Modals>().any_open(), "and opened no offers over the far side");
        assert!(w.resource::<Done>().0.is_empty(), "nor took anything up: {:?}", w.resource::<Done>().0);
    }

    /// One thing on offer on the way is no question to ask, so the key
    /// would have asked to take it up as well: two asks from one press,
    /// which only the turn's one-action claim kept from being two actions.
    #[test]
    fn a_press_of_the_shared_key_on_a_way_asks_for_the_way_and_nothing_else() {
        let (mut stage, _, _) = shared_key();
        #[derive(Resource, Default)]
        struct Asked(usize);
        stage.app.init_resource::<Asked>().add_systems(PostUpdate, |mut asked: ResMut<Asked>, mut read: MessageReader<Intent<Interact>>| {
            asked.0 += read.read().count();
        });
        let (from, landing) = (stage.at, stage.at.offset(-6, 0));
        way(&mut stage, from, landing);

        stage.press(KeyCode::Enter);
        stage.tick();
        let w = stage.app.world();
        assert_eq!(w.get::<Position>(stage.player).map(|p| p.0), Some(landing), "the press took the way");
        assert_eq!(w.resource::<Asked>().0, 0, "and asked to take up nothing left behind");
    }

    /// Off any way the game's ask is refused and took nothing, so the
    /// same key still reaches: two things beside it are a question.
    #[test]
    fn a_press_of_the_shared_key_off_any_way_still_asks_which_of_two() {
        let (mut stage, under, bench) = shared_key();
        stage.app.world_mut().entity_mut(under).despawn();
        let at = stage.app.world().get::<Position>(bench).expect("the bench stands somewhere").0;
        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert(Position(at.offset(-1, 0)));
        stage.tick();

        stage.press(KeyCode::Enter);
        let offers = stage.app.world().resource::<Modals>().get(crate::panel::offers::OFFERS_MODAL).expect("declared");
        assert!(stage.app.world().resource::<Modals>().is_open(offers), "the offers were asked about");
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
