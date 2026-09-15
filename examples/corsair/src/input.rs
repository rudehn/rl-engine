//! Keys to intents.
//!
//! Every key Corsair answers to is declared once, in [`declare_controls`],
//! and read by name through [`Binds`]. The `?` screen lists that same
//! declaration, so the keys it shows are the keys the systems here check.

use bevy::prelude::*;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::Direction;
use rl_engine::rl_ui::{AddControls, AimFire, AimThrow, Chord, ControlId, ControlInput, Keys, MessageLog, Modals, Tones};

/// Every key Corsair answers to, by name.
#[derive(Resource, Clone, Copy)]
pub struct Binds {
    pub walk: ControlId,
    pub go_through: ControlId,
    pub wait: ControlId,
    pub pick_up: ControlId,
    pub close_door: ControlId,
    pub fire: ControlId,
    pub throw: ControlId,
    pub put_on: ControlId,
    pub chest: ControlId,
    pub ledger: ControlId,
    pub call_on: ControlId,
    pub menu_up: ControlId,
    pub menu_down: ControlId,
    pub chest_equip: ControlId,
    pub chest_drop: ControlId,
    pub chest_use: ControlId,
    pub chest_throw: ControlId,
    pub save: ControlId,
    pub quit: ControlId,
}

/// Declares the keys, under the headings the `?` screen groups them by.
/// The engine's own screens declare theirs alongside: the cursors', the
/// log's, the map's, and `?` itself.
pub fn declare_controls(app: &mut App) {
    let binds = Binds {
        walk: app.add_control("Move", "walk, or strike whoever is there", Keys::Directions { shift: false }),
        go_through: app.add_control(
            "Move",
            "go through stairs or a portal",
            [Chord::key(KeyCode::Enter), Chord::shift(KeyCode::Period), Chord::shift(KeyCode::Comma)],
        ),
        wait: app.add_control("Act", "wait a turn", [KeyCode::Period, KeyCode::Numpad5]),
        pick_up: app.add_control("Act", "pick up what is here", [KeyCode::KeyG, KeyCode::Comma]),
        put_on: app.add_control("Act", "put on what is here", KeyCode::KeyE),
        close_door: app.add_control("Act", "shut the door beside you", KeyCode::KeyC),
        fire: app.add_control("Act", "fire the pistol", KeyCode::KeyF),
        throw: app.add_control("Act", "throw a knife", KeyCode::KeyR),
        call_on: app.add_control("Abilities", "call on one", [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4]),
        chest: app.add_control("Screens", "open the sea chest", KeyCode::KeyI),
        ledger: app.add_control("Screens", "open the ledger", KeyCode::KeyT),
        menu_up: app.add_control("Screens", "up a row", [KeyCode::ArrowUp, KeyCode::KeyK]),
        menu_down: app.add_control("Screens", "down a row", [KeyCode::ArrowDown, KeyCode::KeyJ]),
        chest_equip: app.add_control("Sea chest", "wear it, or take it off", KeyCode::KeyE),
        chest_drop: app.add_control("Sea chest", "drop it", KeyCode::KeyD),
        chest_use: app.add_control("Sea chest", "use it", [KeyCode::KeyU, KeyCode::Enter]),
        chest_throw: app.add_control("Sea chest", "throw it", KeyCode::KeyT),
        save: app.add_control("Game", "write the run to the log book", Chord::shift(KeyCode::KeyS)),
        quit: app.add_control("Game", "save and quit", KeyCode::KeyQ),
    };
    app.insert_resource(binds);
}

/// The player, while it holds the turn.
type PlayerTurn<'w, 's> = Query<'w, 's, (Entity, &'static Position), (With<Player>, With<MyTurn>)>;

/// What input reads.
#[derive(bevy::ecs::system::SystemParam)]
pub struct InputWorld<'w, 's> {
    keys: ControlInput<'w>,
    binds: Res<'w, Binds>,
    modals: Res<'w, Modals>,
    occupancy: Res<'w, Occupancy>,
    map: Res<'w, WorldMap>,
    player: PlayerTurn<'w, 's>,
}

/// What aiming a shot reads.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Aim<'w, 's> {
    keys: ControlInput<'w>,
    binds: Res<'w, Binds>,
    modals: Res<'w, Modals>,
    turns: Res<'w, Turns>,
    log: ResMut<'w, MessageLog>,
    player: Query<'w, 's, Gunner, (With<Player>, With<MyTurn>)>,
}

/// The player, and whether it has anything to shoot with.
type Gunner = (Entity, Has<RangedAttack>);

/// `f`: fire through the targeting cursor, which opens on the nearest foe,
/// cycles the rest with Tab, previews the line of fire, and shoots on
/// confirm.
pub fn fire(mut aim: Aim, mut aims: MessageWriter<AimFire>) {
    if aim.modals.any_open() || !aim.keys.just_pressed(aim.binds.fire) {
        return;
    }
    let Ok((me, armed)) = aim.player.single() else { return };
    if armed {
        aims.write(AimFire { user: me });
    } else {
        aim.log.push("You have nothing to shoot with.", Tones::MUTED, aim.turns.turn_number());
    }
}

/// The player while it holds the turn, and what it carries.
type Holder = (Entity, &'static Position, &'static Inventory);
/// Something lying about that can be put on.
type LyingWearable = (Entity, &'static Position, Option<&'static OnMap>);

/// What throwing and putting on from the ground read.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Hands<'w, 's> {
    keys: ControlInput<'w>,
    binds: Res<'w, Binds>,
    modals: Res<'w, Modals>,
    map: Res<'w, WorldMap>,
    turns: Res<'w, Turns>,
    log: ResMut<'w, MessageLog>,
    player: Query<'w, 's, Holder, (With<Player>, With<MyTurn>)>,
    missiles: Query<'w, 's, (), With<Throwable>>,
    ground: Query<'w, 's, LyingWearable, (With<Item>, With<Wearable>)>,
}

/// `r`: throw the first thing carried that can be thrown, through the
/// targeting cursor, which picks the nearest foe and throws on confirm.
pub fn hurl(mut hands: Hands, mut aims: MessageWriter<AimThrow>) {
    if hands.modals.any_open() || !hands.keys.just_pressed(hands.binds.throw) {
        return;
    }
    let Ok((me, _, bag)) = hands.player.single() else { return };
    match bag.items.iter().copied().find(|item| hands.missiles.contains(*item)) {
        Some(item) => {
            aims.write(AimThrow { user: me, item });
        }
        None => hands.log.push("You have nothing to throw.", Tones::MUTED, hands.turns.turn_number()),
    }
}

/// `e`: put on what lies underfoot, in one action and half again, which is
/// quicker than picking it up and putting it on.
pub fn equip_underfoot(mut hands: Hands, mut intents: MessageWriter<Intent<EquipFromGround>>) {
    if hands.modals.any_open() || !hands.keys.just_pressed(hands.binds.put_on) {
        return;
    }
    let Ok((me, at, _)) = hands.player.single() else { return };
    let here = hands.map.current();
    match hands.ground.iter().find(|(_, p, on)| p.0 == at.0 && on.map(|m| m.0).unwrap_or(MapId::SURFACE) == here) {
        Some((item, _, _)) => {
            intents.write(Intent::new(me, EquipFromGround(item)));
        }
        None => hands.log.push("There is nothing here to put on.", Tones::MUTED, hands.turns.turn_number()),
    }
}

/// What the player's keys can ask for.
#[derive(bevy::ecs::system::SystemParam)]
pub struct PlayerIntents<'w> {
    steps: MessageWriter<'w, Intent<Step>>,
    attacks: MessageWriter<'w, Intent<Attack>>,
    waits: MessageWriter<'w, Intent<Wait>>,
    transits: MessageWriter<'w, Intent<GoThrough>>,
    pick_ups: MessageWriter<'w, Intent<PickUp>>,
    closes: MessageWriter<'w, Intent<Close>>,
}

/// Turns keys into an [`Intent`] for the player while it holds the turn.
pub fn player_input(world: InputWorld, mut intents: PlayerIntents) {
    let InputWorld { keys, binds, modals, occupancy, map, player } = world;
    // One gate for every screen there is, and every screen a game adds
    // later: the stack is empty or the world does not have the keys.
    if modals.any_open() {
        return;
    }
    let Ok((entity, pos)) = player.single() else { return };

    if let Some(dir) = keys.direction(binds.walk) {
        // Bump to attack: walking into someone is a strike.
        match occupancy.first_at(pos.0 + dir.offset()) {
            Some(other) => {
                intents.attacks.write(Intent::new(entity, Attack(other)));
            }
            None => {
                intents.steps.write(Intent::new(entity, Step(dir)));
            }
        }
        return;
    }
    if keys.just_pressed(binds.go_through) {
        // Through whatever stands here: stairs, a cave mouth, a portal.
        intents.transits.write(Intent::new(entity, GoThrough));
    } else if keys.just_pressed(binds.wait) {
        intents.waits.write(Intent::new(entity, Wait));
    } else if keys.just_pressed(binds.pick_up) {
        intents.pick_ups.write(Intent::new(entity, PickUp));
    } else if keys.just_pressed(binds.close_door) {
        // Shut the open door beside you. Walking into a shut one opens it.
        if let Some(dir) = Direction::ALL.into_iter().find(|d| map.closes(pos.0 + d.offset()).is_some()) {
            intents.closes.write(Intent::new(entity, Close(dir)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rl_engine::rl_bevy::testing::{KeyScriptPlugin, press};
    use rl_engine::rl_core::{Point, RunSeed};

    /// Through the real keys: walking into a shut door opens it and leaves
    /// you where you stood, `c` shuts it again, and the log says both.
    #[test]
    fn walking_into_a_door_opens_it_and_c_shuts_it_again() {
        let dir = std::env::temp_dir().join(format!("corsair-doors-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = crate::testing::headless(RunSeed(7), false, &dir);
        app.add_plugins(KeyScriptPlugin)
            .add_systems(Update, player_input.in_set(EngineSet::Input))
            .add_systems(Update, crate::monsters::narrate_doors.in_set(PresentSet::Narrate));
        app.update();
        app.update();
        let me = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        let at = app.world().get::<Position>(me).unwrap().0;

        // A door on whichever side of the player is clear ground.
        let tiles = crate::content::Content::new().tiles().clone();
        let (shut, open) = (tiles.expect("door"), tiles.expect("open door"));
        let lying: Vec<Point> = app.world_mut().query_filtered::<&Position, With<Item>>().iter(app.world()).map(|p| p.0).collect();
        let sides = [
            (KeyCode::ArrowRight, Direction::East),
            (KeyCode::ArrowLeft, Direction::West),
            (KeyCode::ArrowUp, Direction::North),
            (KeyCode::ArrowDown, Direction::South),
        ];
        let (key, door) = sides
            .into_iter()
            .map(|(key, dir)| (key, at + dir.offset()))
            .find(|(_, p)| app.world().resource::<WorldMap>().is_walkable(*p) && !app.world().resource::<Occupancy>().is_occupied(*p) && !lying.contains(p))
            .expect("clear ground beside the player");
        app.world_mut().resource_mut::<WorldMap>().set_tile(door, shut);
        app.update();

        let said = |app: &App, line: &str| app.world().resource::<MessageLog>().iter().any(|e| e.text == line);
        press(&mut app, key);
        assert_eq!(app.world().resource::<WorldMap>().tile(door), Some(open), "the door opened");
        assert_eq!(app.world().get::<Position>(me).unwrap().0, at, "and the player stayed put");
        assert!(said(&app, "You open the door."));

        press(&mut app, KeyCode::KeyC);
        assert_eq!(app.world().resource::<WorldMap>().tile(door), Some(shut), "`c` shut it");
        assert!(said(&app, "You close the door."));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
