//! Light on the decks: a dim first deck, dark ones below it, the lamps
//! that hang in the supply stores, and the commando's own shoulder lamp.
//!
//! The dark is the design's stealth trade, and the lamp is the player's
//! side of it. Lit, the commando sees about six tiles in the dark and
//! every droid with a line to them sees them back; switched off, a droid
//! with no radar has to be touching them to know they are there, and so
//! does the commando. A probe's radar, or the rangefinder helmet, is what
//! sees through the dark either way.
//!
//! Without a lamp at all a dark deck is unplayable: the engine's light
//! gate cuts every viewshed down to what is lit, so the player saw one
//! tile and droids without radar never saw the player. That is why the
//! lamp starts lit.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_grid::{Light, Rgb, TileId};

use crate::decks::deck_of;
use crate::input::Binds;

/// The commando's shoulder lamp: a steady, cool light reaching six tiles.
/// Steady rather than flickering, since it is a lamp and not a flame.
pub const SHOULDER_LAMP: LightSource = LightSource::new(200, 6, Rgb::new(215, 230, 255));

/// What one of a store's wall lamps sheds: warmer than the commando's,
/// and enough to light the small room it hangs in and a little of the
/// corridor outside its door.
pub const WALL_LAMP: LightSource = LightSource::new(170, 5, Rgb::new(255, 225, 150));

/// The light everywhere on `deck`. Deck one keeps a dim working light, lit
/// enough to be seen, so the first fight is learned with the lights on;
/// every deck below it is dark, which is where a probe's radar starts to
/// matter.
pub fn ambient_of(deck: u32) -> Light {
    match deck {
        1 => Light::new(36, Rgb::new(185, 200, 225)),
        _ => Light::DARK,
    }
}

/// The tile a wall lamp is, from `decks::Foundry`, kept so a deck's lamps
/// can be found after the map has been handed to the engine.
#[derive(Resource, Debug, Clone, Copy)]
pub struct LampTile(pub TileId);

/// Sets the current deck's ambient, between the turns and the light, so
/// the frame a lift lands on is drawn in the new deck's light rather than
/// the old one's.
pub fn set_ambient(map: Res<WorldMap>, mut lighting: ResMut<Lighting>) {
    let ambient = ambient_of(deck_of(map.current()));
    if lighting.ambient != ambient {
        lighting.ambient = ambient;
    }
}

/// Hangs a [`WALL_LAMP`] on every lamp tile of a deck the first time it
/// is entered. A fixture: an entity with no turns, so the engine keeps it
/// in its static layer and recasts it only when the map changes.
pub fn light_the_lamps(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, map: Res<WorldMap>, lamp: Res<LampTile>) {
    for ev in entered.read() {
        if ev.first {
            hang_lamps(&mut commands, &map, ev.map, lamp.0);
        }
    }
}

/// Hangs a [`WALL_LAMP`] on every lamp tile of the built deck `deck`.
///
/// Read off the deck's own terrain rather than kept, so a continued run
/// hangs them again from the decks it restored, the way the first arrival
/// hung them, and the two can never disagree about where a lamp is.
pub fn hang_lamps(commands: &mut Commands, map: &WorldMap, deck: MapId, lamp: TileId) {
    let Some(place) = map.place(deck) else { return };
    for (p, _) in place.terrain.iter().filter(|(_, t)| *t == lamp) {
        commands.spawn((Position(p), OnMap(deck), WALL_LAMP));
    }
}

/// The player holding the turn, and whether its lamp is lit.
type LampBearer = (Entity, Has<LightSource>);

/// What switching the lamp writes.
#[derive(bevy::ecs::system::SystemParam)]
pub struct LampSwitch<'w, 's> {
    commands: Commands<'w, 's>,
    waits: MessageWriter<'w, Intent<Wait>>,
    tell: MessageWriter<'w, Tell>,
}

/// `L` switches the shoulder lamp off, or on again, and spends the turn,
/// so the dark is a choice with a price rather than a free flicker between
/// two of a droid's looks.
pub fn toggle_lamp(
    keys: ControlInput,
    binds: Res<Binds>,
    modals: Res<Modals>,
    player: Query<LampBearer, (With<Player>, With<MyTurn>)>,
    mut switch: LampSwitch,
) {
    if modals.any_open() || !keys.just_pressed(binds.lamp) {
        return;
    }
    let Ok((me, lit)) = player.single() else { return };
    if lit {
        switch.commands.entity(me).remove::<LightSource>();
        switch.tell.write(Tell::new("You switch the lamp off. The dark hides you, and them.", Tones::MUTED));
    } else {
        switch.commands.entity(me).insert(SHOULDER_LAMP);
        switch.tell.write(Tell::new("You switch the lamp on.", Tones::NOTICE));
    }
    switch.waits.write(Intent::new(me, Wait));
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_bevy::testing::{KeyScript, KeyScriptPlugin};
    use rl_engine::rl_core::{Point, RunSeed};

    use super::*;

    fn player(app: &mut App) -> Entity {
        app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap()
    }

    #[test]
    fn deck_one_is_lit_enough_to_see_and_the_decks_below_are_dark() {
        assert!(ambient_of(1).intensity >= rl_engine::rl_bevy::lighting::DEFAULT_THRESHOLD, "deck one's working light is enough to see by");
        assert_eq!(ambient_of(2), Light::DARK);
        assert_eq!(ambient_of(3), Light::DARK);
    }

    /// Presses Shift and `l` together, the way a player types `L`.
    fn press_shifted(app: &mut App, key: KeyCode) {
        let mut script = app.world_mut().resource_mut::<KeyScript>();
        script.hold(KeyCode::ShiftLeft);
        script.press(key);
        app.update();
        app.update();
        app.world_mut().resource_mut::<KeyScript>().release(KeyCode::ShiftLeft);
        app.update();
    }

    #[test]
    fn on_a_dark_deck_the_lamp_lights_the_commando_and_l_puts_it_out_for_a_turn() {
        let mut app = crate::testing::headless(RunSeed(4));
        app.add_plugins(KeyScriptPlugin);
        crate::testing::arrive_on(&mut app, 2);
        crate::testing::settle(&mut app);
        let me = player(&mut app);
        let at = app.world().get::<Position>(me).unwrap().0;
        let lit = app.world().resource::<Lighting>().at(at).intensity;
        assert_eq!(app.world().resource::<Lighting>().ambient, Light::DARK, "deck two is dark");
        assert!(app.world().resource::<Lighting>().is_lit(at), "the lamp lights the commando's own tile");
        let before = crate::testing::clock(&app);

        press_shifted(&mut app, KeyCode::KeyL);
        crate::testing::settle(&mut app);
        assert!(app.world().get::<LightSource>(me).is_none(), "L put the lamp out");
        assert!(crate::testing::clock(&app) > before, "and it spent the turn");
        assert!(app.world().resource::<Lighting>().at(at).intensity < lit, "the commando's tile is darker with the lamp out");

        press_shifted(&mut app, KeyCode::KeyL);
        crate::testing::settle(&mut app);
        assert_eq!(app.world().get::<LightSource>(me), Some(&SHOULDER_LAMP), "L again lights it");
    }

    #[test]
    fn every_wall_lamp_lights_the_floor_beside_it_over_a_span_of_seeds() {
        for s in 0..8 {
            let mut app = crate::testing::headless(RunSeed(s));
            crate::testing::arrive_on(&mut app, 2);
            let me = player(&mut app);
            app.world_mut().entity_mut(me).remove::<LightSource>();
            crate::testing::settle(&mut app);
            let here = app.world().resource::<WorldMap>().current();
            let mut fixtures = app.world_mut().query_filtered::<(&Position, &OnMap), (With<LightSource>, Without<Player>)>();
            let lamps: Vec<Point> = fixtures.iter(app.world()).filter(|(_, on)| on.0 == here).map(|(p, _)| p.0).collect();
            assert!(!lamps.is_empty(), "seed {s}: deck two's store hangs a lamp");
            let (map, lighting) = (app.world().resource::<WorldMap>(), app.world().resource::<Lighting>());
            for lamp in lamps {
                let beside: Vec<Point> = rl_engine::rl_core::geometry::square(lamp, 1).filter(|p| map.is_walkable(*p)).collect();
                assert!(!beside.is_empty(), "seed {s}: the lamp at {lamp:?} hangs on a wall with floor beside it");
                assert!(beside.iter().all(|p| lighting.is_lit(*p)), "seed {s}: the floor beside the lamp at {lamp:?} is dark");
            }
        }
    }
}
