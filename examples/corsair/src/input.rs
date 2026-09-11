//! Keys to intents.

use bevy::prelude::*;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::Direction;
use rl_engine::rl_overworld::OverworldScreen;

use crate::inventory::InventoryScreen;

/// How long a held key waits before repeating, and between repeats.
const REPEAT_DELAY: f32 = 0.25;
const REPEAT_EVERY: f32 = 0.08;

#[derive(Default)]
pub struct Repeat {
    held_for: f32,
    since_last: f32,
}

const MOVES: [(&[KeyCode], Direction); 8] = [
    (&[KeyCode::ArrowUp, KeyCode::KeyK, KeyCode::Numpad8], Direction::North),
    (&[KeyCode::ArrowDown, KeyCode::KeyJ, KeyCode::Numpad2], Direction::South),
    (&[KeyCode::ArrowLeft, KeyCode::KeyH, KeyCode::Numpad4], Direction::West),
    (&[KeyCode::ArrowRight, KeyCode::KeyL, KeyCode::Numpad6], Direction::East),
    (&[KeyCode::KeyY, KeyCode::Numpad7], Direction::NorthWest),
    (&[KeyCode::KeyU, KeyCode::Numpad9], Direction::NorthEast),
    (&[KeyCode::KeyB, KeyCode::Numpad1], Direction::SouthWest),
    (&[KeyCode::KeyN, KeyCode::Numpad3], Direction::SouthEast),
];

/// The player, while it holds the turn.
type PlayerTurn<'w, 's> = Query<'w, 's, (Entity, &'static Position), (With<Player>, With<MyTurn>)>;

/// What input reads.
#[derive(bevy::ecs::system::SystemParam)]
pub struct InputWorld<'w, 's> {
    keys: Res<'w, ButtonInput<KeyCode>>,
    time: Res<'w, Time>,
    screen: Res<'w, OverworldScreen>,
    chest: Res<'w, InventoryScreen>,
    occupancy: Res<'w, Occupancy>,
    player: PlayerTurn<'w, 's>,
}

/// Turns keys into an [`Intent`] for the player while it holds the turn.
pub fn player_input(world: InputWorld, mut repeat: Local<Repeat>, mut intents: MessageWriter<Intent>, mut exit: MessageWriter<AppExit>) {
    let InputWorld { keys, time, screen, chest, occupancy, player } = world;
    if keys.just_pressed(KeyCode::KeyQ) {
        exit.write(AppExit::Success);
        return;
    }
    if screen.open || chest.open {
        return;
    }
    let Ok((entity, pos)) = player.single() else { return };

    let held = MOVES.iter().find(|(codes, _)| keys.any_pressed(codes.iter().copied()));
    let fresh = MOVES.iter().find(|(codes, _)| keys.any_just_pressed(codes.iter().copied()));
    let action = if let Some((_, dir)) = fresh {
        repeat.held_for = 0.0;
        repeat.since_last = 0.0;
        Some(Action::Move(*dir))
    } else if let Some((_, dir)) = held {
        repeat.held_for += time.delta_secs();
        repeat.since_last += time.delta_secs();
        if repeat.held_for >= REPEAT_DELAY && repeat.since_last >= REPEAT_EVERY {
            repeat.since_last = 0.0;
            Some(Action::Move(*dir))
        } else {
            None
        }
    } else if keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) && keys.any_just_pressed([KeyCode::Period, KeyCode::Comma]) {
        // `>` and `<`: through whatever stands here.
        Some(Action::Enter)
    } else if keys.just_pressed(KeyCode::Period) || keys.just_pressed(KeyCode::Numpad5) {
        Some(Action::Wait)
    } else if keys.just_pressed(KeyCode::KeyG) || keys.just_pressed(KeyCode::Comma) {
        Some(Action::PickUp)
    } else if keys.just_pressed(KeyCode::Enter) {
        Some(Action::Enter)
    } else {
        *repeat = Repeat::default();
        None
    };
    if let Some(action) = action {
        // Bump to attack: walking into someone is a strike.
        let action = match action {
            Action::Move(dir) => match occupancy.first_at(pos.0 + dir.offset()) {
                Some(other) => Action::Attack(other),
                None => Action::Move(dir),
            },
            other => other,
        };
        debug!("player intent {action:?} (fresh {:?}, held {:?})", fresh.map(|m| m.1), held.map(|m| m.1));
        intents.write(Intent { actor: entity, action });
    }
}
