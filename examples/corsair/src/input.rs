//! Keys to intents.

use bevy::prelude::*;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::Direction;
use rl_engine::rl_ui::{AimFire, AimThrow, DirectionKeys, MessageLog, Modals, Tones};

/// How long a held key waits before repeating, and between repeats.
const REPEAT_DELAY: f32 = 0.25;
const REPEAT_EVERY: f32 = 0.08;

#[derive(Default)]
pub struct Repeat {
    held_for: f32,
    since_last: f32,
}

/// The player, while it holds the turn.
type PlayerTurn<'w, 's> = Query<'w, 's, (Entity, &'static Position), (With<Player>, With<MyTurn>)>;

/// What input reads.
#[derive(bevy::ecs::system::SystemParam)]
pub struct InputWorld<'w, 's> {
    keys: Res<'w, ButtonInput<KeyCode>>,
    time: Res<'w, Time>,
    binds: Res<'w, DirectionKeys>,
    modals: Res<'w, Modals>,
    occupancy: Res<'w, Occupancy>,
    map: Res<'w, WorldMap>,
    player: PlayerTurn<'w, 's>,
}

/// What aiming a shot reads.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Aim<'w, 's> {
    keys: Res<'w, ButtonInput<KeyCode>>,
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
    if aim.modals.any_open() || !aim.keys.just_pressed(KeyCode::KeyF) {
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
    keys: Res<'w, ButtonInput<KeyCode>>,
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
    if hands.modals.any_open() || !hands.keys.just_pressed(KeyCode::KeyR) {
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
    if hands.modals.any_open() || !hands.keys.just_pressed(KeyCode::KeyE) {
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
pub fn player_input(world: InputWorld, mut repeat: Local<Repeat>, mut intents: PlayerIntents) {
    let InputWorld { keys, time, binds, modals, occupancy, map, player } = world;
    // One gate for every screen there is, and every screen a game adds
    // later: the stack is empty or the world does not have the keys.
    if modals.any_open() {
        return;
    }
    let Ok((entity, pos)) = player.single() else { return };

    let held = binds.pressed(&keys);
    let fresh = binds.just_pressed(&keys);
    let walk = if let Some(dir) = fresh {
        repeat.held_for = 0.0;
        repeat.since_last = 0.0;
        Some(dir)
    } else if let Some(dir) = held {
        repeat.held_for += time.delta_secs();
        repeat.since_last += time.delta_secs();
        if repeat.held_for >= REPEAT_DELAY && repeat.since_last >= REPEAT_EVERY {
            repeat.since_last = 0.0;
            Some(dir)
        } else {
            None
        }
    } else {
        *repeat = Repeat::default();
        None
    };
    if let Some(dir) = walk {
        debug!("player walks {dir:?} (fresh {fresh:?}, held {held:?})");
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
    if keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) && keys.any_just_pressed([KeyCode::Period, KeyCode::Comma]) {
        // `>` and `<`: through whatever stands here.
        intents.transits.write(Intent::new(entity, GoThrough));
    } else if keys.just_pressed(KeyCode::Period) || keys.just_pressed(KeyCode::Numpad5) {
        intents.waits.write(Intent::new(entity, Wait));
    } else if keys.just_pressed(KeyCode::KeyG) || keys.just_pressed(KeyCode::Comma) {
        intents.pick_ups.write(Intent::new(entity, PickUp));
    } else if keys.just_pressed(KeyCode::KeyC) {
        // `c`: shut the open door beside you. Walking into a shut one opens it.
        if let Some(dir) = Direction::ALL.into_iter().find(|d| map.closes(pos.0 + d.offset()).is_some()) {
            intents.closes.write(Intent::new(entity, Close(dir)));
        }
    } else if keys.just_pressed(KeyCode::Enter) {
        intents.transits.write(Intent::new(entity, GoThrough));
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
