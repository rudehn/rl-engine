//! The eight directions, off whichever keys a game's players expect.
//!
//! Every roguelike binds arrows, vi keys and the numpad to the same eight
//! directions, and every one of them writes the same table. It is here so
//! that the engine's own cursor and a game's movement read the same keys,
//! and so that a game that wants only the numpad changes one resource.

use bevy::prelude::*;
use rl_core::Direction;

/// Which keys mean which direction.
///
/// Several keys per direction, because a player who reaches for `k` and a
/// player who reaches for the numpad are both right.
#[derive(Resource, Debug, Clone)]
pub struct DirectionKeys(pub Vec<(KeyCode, Direction)>);

impl Default for DirectionKeys {
    fn default() -> Self {
        use Direction::*;
        use KeyCode::*;
        Self(vec![
            (ArrowUp, North),
            (KeyK, North),
            (Numpad8, North),
            (ArrowDown, South),
            (KeyJ, South),
            (Numpad2, South),
            (ArrowLeft, West),
            (KeyH, West),
            (Numpad4, West),
            (ArrowRight, East),
            (KeyL, East),
            (Numpad6, East),
            (KeyY, NorthWest),
            (Numpad7, NorthWest),
            (KeyU, NorthEast),
            (Numpad9, NorthEast),
            (KeyB, SouthWest),
            (Numpad1, SouthWest),
            (KeyN, SouthEast),
            (Numpad3, SouthEast),
        ])
    }
}

impl DirectionKeys {
    /// A binding of nothing, to build up from.
    pub fn none() -> Self {
        Self(Vec::new())
    }

    /// Binds `key` to `direction`.
    pub fn bind(mut self, key: KeyCode, direction: Direction) -> Self {
        self.0.push((key, direction));
        self
    }

    /// The direction of the first bound key pressed this frame, if any.
    pub fn just_pressed(&self, keys: &ButtonInput<KeyCode>) -> Option<Direction> {
        self.0.iter().find(|(key, _)| keys.just_pressed(*key)).map(|(_, dir)| *dir)
    }

    /// The direction of the first bound key held down, if any. For a
    /// cursor a game wants to repeat.
    pub fn pressed(&self, keys: &ButtonInput<KeyCode>) -> Option<Direction> {
        self.0.iter().find(|(key, _)| keys.pressed(*key)).map(|(_, dir)| *dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_direction_answers_to_an_arrow_a_letter_and_a_number() {
        let binds = DirectionKeys::default();
        for key in [KeyCode::ArrowUp, KeyCode::KeyK, KeyCode::Numpad8] {
            let mut input = ButtonInput::<KeyCode>::default();
            input.press(key);
            assert_eq!(binds.just_pressed(&input), Some(Direction::North), "{key:?}");
        }
    }

    #[test]
    fn a_game_may_bind_nothing_but_its_own_keys() {
        let binds = DirectionKeys::none().bind(KeyCode::KeyW, Direction::North);
        let mut input = ButtonInput::<KeyCode>::default();
        input.press(KeyCode::ArrowUp);
        assert_eq!(binds.just_pressed(&input), None, "the arrows are not bound");
        input.press(KeyCode::KeyW);
        assert_eq!(binds.just_pressed(&input), Some(Direction::North));
    }
}
