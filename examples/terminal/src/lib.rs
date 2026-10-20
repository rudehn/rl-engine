//! A walk through generated rooms on the Bevy-free crates alone.
//!
//! The way bracket-lib is used: the game owns its loop and calls the toolkit for a map
//! (`rl-mapgen`), a field of view (`rl-grid`) and the shared vocabulary (`rl-core`).
//! Nothing here touches a terminal, so every rule is tested as plain functions; `main.rs`
//! is only the loop that reads keys and prints [`render`]'s rows.

#![deny(missing_docs)]

use rl_core::{Direction, Point, RunSeed};
use rl_grid::{BitGrid, Terrain, TileRegistry, fov};
use rl_mapgen::dungeon::{RandomStart, Rooms};
use rl_mapgen::passes::StartPoint;
use rl_mapgen::{BaseContext, BuildContext, Chain};

/// Map width in cells, one terminal column each.
pub const WIDTH: i32 = 60;
/// Map height in cells, one terminal row each.
pub const HEIGHT: i32 = 24;
/// How far the walker sees, in cells.
pub const SIGHT: i32 = 8;

/// How many times a seed's rooms are regrown from the next attempt before giving up.
///
/// Room placement can fail on a map this small; a failed attempt is thrown away rather than
/// half used, so a seed always yields a map unless every attempt fails.
const ATTEMPTS: u32 = 32;

/// A map, where the walker stands and what it has seen.
pub struct Walk {
    /// The grown map.
    pub terrain: Terrain,
    /// What each tile of the map means.
    pub tiles: TileRegistry,
    /// Where the walker stands.
    pub at: Point,
    /// The cells in sight from `at` now.
    pub seen: BitGrid,
    /// Every cell ever in sight, kept so the map is remembered once out of view.
    pub explored: BitGrid,
}

/// Grows a map of rooms from `seed` and stands the walker on the chain's start.
///
/// # Panics
/// Panics if all attempts of the seed fail to place rooms, which means the chain, not the seed,
/// is wrong for a map this size.
pub fn generate(seed: u64) -> Walk {
    let tiles = TileRegistry::standard();
    let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
    let chain = Chain::new().then(Rooms { floor, ..Default::default() }).then(RandomStart);
    let ctx = chain.run_retrying(|| BaseContext::blank(WIDTH, HEIGHT, tiles.clone(), wall), RunSeed(seed), ATTEMPTS).expect("rooms fit a map this size");
    let at = ctx.outputs().first::<StartPoint>().expect("RandomStart records a start").0;
    let (terrain, _) = ctx.finish();
    let mut walk = Walk { terrain, tiles, at, seen: BitGrid::new(WIDTH, HEIGHT), explored: BitGrid::new(WIDTH, HEIGHT) };
    look(&mut walk);
    walk
}

/// Moves one cell when the target is walkable, then looks. Returns whether it moved.
pub fn step(walk: &mut Walk, dir: Direction) -> bool {
    let to = walk.at + dir.offset();
    if !walk.terrain.view(&walk.tiles).is_walkable(to) {
        return false;
    }
    walk.at = to;
    look(walk);
    true
}

/// Recomputes what is in sight and adds it to what is remembered.
pub fn look(walk: &mut Walk) {
    fov::compute(&walk.terrain.view(&walk.tiles), walk.at, SIGHT, &mut walk.seen);
    walk.explored.union_with(&walk.seen);
}

/// The picture as `HEIGHT` rows of `WIDTH` characters; colour is the terminal loop's business.
///
/// Sight and memory draw alike here, so a loop that wants remembered cells dimmer asks
/// `walk.seen` for each cell itself.
pub fn render(walk: &Walk) -> Vec<String> {
    let view = walk.terrain.view(&walk.tiles);
    (0..HEIGHT)
        .map(|y| {
            (0..WIDTH)
                .map(|x| {
                    let p = Point::new(x, y);
                    if p == walk.at {
                        '@'
                    } else if !walk.explored.contains(p) {
                        ' '
                    } else if view.is_walkable(p) {
                        '.'
                    } else {
                        '#'
                    }
                })
                .collect()
        })
        .collect()
}

/// The direction a vi key (`h j k l y u b n`) walks in.
pub fn direction_of(key: char) -> Option<Direction> {
    Some(match key {
        'h' => Direction::West,
        'j' => Direction::South,
        'k' => Direction::North,
        'l' => Direction::East,
        'y' => Direction::NorthWest,
        'u' => Direction::NorthEast,
        'b' => Direction::SouthWest,
        'n' => Direction::SouthEast,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn walkable(walk: &Walk, p: Point) -> bool {
        walk.terrain.view(&walk.tiles).is_walkable(p)
    }

    #[test]
    fn the_player_starts_on_floor_over_a_span_of_seeds() {
        for seed in 0..8 {
            let walk = generate(seed);
            assert!(walkable(&walk, walk.at), "seed {seed} starts the player in a wall");
        }
    }

    #[test]
    fn a_step_into_a_wall_is_refused_and_leaves_the_player_where_it_was() {
        let mut walk = generate(1);
        let dir = Direction::ALL.into_iter().find(|d| !walkable(&walk, walk.at + d.offset())).expect("a room has a wall somewhere near");
        let before = walk.at;
        assert!(!step(&mut walk, dir));
        assert_eq!(walk.at, before);
    }

    #[test]
    fn a_step_onto_floor_moves_the_player_and_what_it_saw_stays_explored() {
        let mut walk = generate(1);
        let dir = Direction::ALL.into_iter().find(|d| walkable(&walk, walk.at + d.offset())).expect("a start has a floor neighbour");
        let (from, explored) = (walk.at, walk.explored.clone());
        assert!(step(&mut walk, dir));
        assert_eq!(walk.at, from + dir.offset());
        for i in 0..(WIDTH * HEIGHT) as usize {
            assert!(!explored.get_idx(i) || walk.explored.get_idx(i), "cell {i} was forgotten");
        }
    }

    #[test]
    fn the_picture_is_the_map_size_and_shows_the_player_once() {
        let walk = generate(1);
        let rows = render(&walk);
        assert_eq!(rows.len(), HEIGHT as usize);
        assert!(rows.iter().all(|r| r.chars().count() == WIDTH as usize));
        assert_eq!(rows.iter().flat_map(|r| r.chars()).filter(|c| *c == '@').count(), 1);
        assert_eq!(rows[walk.at.y as usize].chars().nth(walk.at.x as usize), Some('@'));
    }

    #[test]
    fn a_cell_never_seen_is_blank() {
        let walk = generate(1);
        let rows = render(&walk);
        let blank = (0..(WIDTH * HEIGHT) as usize).find(|i| !walk.explored.get_idx(*i)).expect("sight of eight does not cover the map");
        let (x, y) = (blank % WIDTH as usize, blank / WIDTH as usize);
        assert_eq!(rows[y].chars().nth(x), Some(' '));
    }

    #[test]
    fn a_remembered_cell_out_of_sight_is_drawn_as_its_terrain_and_not_as_blank() {
        let mut walk = generate(1);
        let view_is_floor = |walk: &Walk, p: Point| walk.terrain.view(&walk.tiles).is_walkable(p);
        for i in 0..400 {
            step(&mut walk, Direction::ALL[(i / 5) % 8]);
            let remembered =
                (0..WIDTH * HEIGHT).map(|n| Point::new(n % WIDTH, n / WIDTH)).find(|p| walk.explored.contains(*p) && !walk.seen.contains(*p) && *p != walk.at);
            if let Some(p) = remembered {
                let glyph = render(&walk)[p.y as usize].chars().nth(p.x as usize);
                assert_eq!(glyph, Some(if view_is_floor(&walk, p) { '.' } else { '#' }));
                return;
            }
        }
        panic!("a long walk never left a seen cell behind");
    }

    #[test]
    fn vi_keys_map_to_the_eight_directions() {
        let keys: Vec<_> = "hjklyubn".chars().filter_map(direction_of).collect();
        assert_eq!(keys.len(), 8);
        assert_eq!(direction_of('h'), Some(Direction::West));
        assert_eq!(direction_of('x'), None);
    }
}
