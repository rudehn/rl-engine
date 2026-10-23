//! A walk through generated rooms, and a fight with the rats in them, on the Bevy-free
//! crates alone.
//!
//! The way bracket-lib is used: the game owns its loop and calls the toolkit for a map
//! (`atglyph-mapgen`), a field of view (`atglyph-grid`), a turn order and the shared vocabulary
//! (`atglyph-core`), and a rat's mind and the arithmetic of a blow (`atglyph-rules`). Nothing here
//! touches a terminal, so every rule is tested as plain functions; `main.rs` is only the loop
//! that reads keys and prints [`render`]'s rows.

#![deny(missing_docs)]

use atglyph_core::turn::BASE_ACTION_COST;
use atglyph_core::{Direction, Grid2D, Point, Rect, RunSeed, SeedDomain, TurnQueue};
use atglyph_grid::{BitGrid, OpacitySource, Terrain, TileRegistry, fov};
use atglyph_mapgen::dungeon::{RandomStart, Rooms};
use atglyph_mapgen::passes::StartPoint;
use atglyph_mapgen::{BaseContext, Chain};
use atglyph_rules::ai::tactics::{Hunt, MeleeAdjacent};
use atglyph_rules::ai::{ActorView, Brain, Decision, NoFields, Snapshot, TacticCtx, Vitals};
use atglyph_rules::damage::apply_hit;
use rand::rngs::StdRng;
use rand::seq::IndexedRandom;

/// Map width in cells, one terminal column each.
pub const WIDTH: i32 = 80;
/// Map height in cells, one terminal row each: three short of a classic 24-row terminal,
/// which leaves the loop a status line and two lines of messages.
pub const HEIGHT: i32 = 21;
/// How far the walker and the rats see, in cells.
pub const SIGHT: i32 = 8;
/// How many rats a generated walk holds.
pub const RATS_PER_WALK: usize = 3;
/// The walker's health at the start.
pub const YOUR_HEALTH: i32 = 10;
/// A rat's health at the start.
pub const RAT_HEALTH: i32 = 3;
/// What the walker's blow takes from a rat.
pub const YOUR_BLOW: i32 = 2;
/// What a rat's bite takes from the walker.
pub const RAT_BITE: i32 = 1;

/// How many times a seed's rooms are regrown from the next attempt before giving up.
///
/// Room placement can fail on a map this small; a failed attempt is thrown away rather than
/// half used, so a seed always yields a map unless every attempt fails.
const ATTEMPTS: u32 = 32;

/// The stream the rats' starting cells are drawn from.
const RATS: SeedDomain = SeedDomain::new(b"terminal.rats");
/// The stream a rat's mind draws from while it decides. Neither tactic a rat has draws today,
/// but a brain is handed a stream, so it gets a named one rather than one made from a constant.
const MINDS: SeedDomain = SeedDomain::new(b"terminal.minds");

/// Whose turn it is: the walker, or a rat by its place in [`Walk::rats`].
///
/// `Ord` because a [`Snapshot`] breaks ties between two actors at one distance by identity, so
/// two runs agree on which is nearest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Who {
    /// The walker.
    You,
    /// The rat at this index of [`Walk::rats`].
    Rat(usize),
}

/// A rat: where it stands and what health it has left.
///
/// A dead rat keeps its place in [`Walk::rats`], so the index a [`Who::Rat`] names never
/// shifts; it is simply not drawn, not in anyone's way, and dropped from the queue the next
/// time its turn comes up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rat {
    /// Where it stands.
    pub at: Point,
    /// What is left of it.
    pub health: Vitals,
}

impl Rat {
    /// Whether it has any health left.
    pub fn is_alive(&self) -> bool {
        self.health.current > 0
    }
}

/// A map, the walker and the rats on it, whose turn it is and what has happened.
pub struct Walk {
    /// The grown map.
    pub terrain: Terrain,
    /// What each tile of the map means.
    pub tiles: TileRegistry,
    /// Where the walker stands.
    pub at: Point,
    /// The walker's health; at nought it takes no more steps.
    pub health: Vitals,
    /// The rats, living and dead.
    pub rats: Vec<Rat>,
    /// Who acts when. Between two calls into this crate it never holds the walker: it is the
    /// walker's turn, and [`step`] or [`wait`] spends it.
    pub queue: TurnQueue<Who>,
    /// What happened, oldest first.
    pub log: Vec<String>,
    /// The cells in sight from `at` now.
    pub seen: BitGrid,
    /// Every cell ever in sight, kept so the map is remembered once out of view.
    pub explored: BitGrid,
    /// The rats' minds' stream.
    minds: StdRng,
}

impl Walk {
    /// A walk on `terrain` with the walker at `at` and a rat on each of `rats`, everyone at full
    /// health and the walker first to act.
    pub fn new(terrain: Terrain, tiles: TileRegistry, at: Point, rats: &[Point], seed: RunSeed) -> Walk {
        let (width, height) = (terrain.width(), terrain.height());
        let mut queue = TurnQueue::new();
        queue.insert_now(Who::You);
        for i in 0..rats.len() {
            queue.insert_now(Who::Rat(i));
        }
        let mut walk = Walk {
            terrain,
            tiles,
            at,
            health: Vitals { current: YOUR_HEALTH, max: YOUR_HEALTH },
            rats: rats.iter().map(|&at| Rat { at, health: Vitals { current: RAT_HEALTH, max: RAT_HEALTH } }).collect(),
            queue,
            log: Vec::new(),
            seen: BitGrid::new(width, height),
            explored: BitGrid::new(width, height),
            minds: seed.rng(MINDS, 0),
        };
        look(&mut walk);
        // The walker was queued first, so this hands it the first turn and nobody acts.
        rats_act(&mut walk);
        walk
    }

    /// Whether the walker has any health left.
    pub fn is_alive(&self) -> bool {
        self.health.current > 0
    }

    /// The living rat standing on `p`, if one is.
    pub fn rat_at(&self, p: Point) -> Option<usize> {
        self.rats.iter().position(|r| r.is_alive() && r.at == p)
    }
}

/// Grows a map of rooms from `seed`, stands the walker on the chain's start and puts
/// [`RATS_PER_WALK`] rats on floor out of its sight, so the first turn is never a bite.
///
/// # Panics
/// Panics if all attempts of the seed fail to place rooms, which means the chain, not the seed,
/// is wrong for a map this size.
pub fn generate(seed: u64) -> Walk {
    let seed = RunSeed(seed);
    let tiles = TileRegistry::standard();
    let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
    let chain = Chain::new().then(Rooms { floor, ..Default::default() }).then(RandomStart);
    let ctx = chain.run_retrying(|| BaseContext::blank(WIDTH, HEIGHT, tiles.clone(), wall), seed, ATTEMPTS).expect("rooms fit a map this size");
    let at = ctx.outputs().first::<StartPoint>().expect("RandomStart records a start").0;
    let (terrain, _) = ctx.finish();
    let view = terrain.view(&tiles);
    let mut seen = BitGrid::new(WIDTH, HEIGHT);
    fov::compute(&view, at, SIGHT, &mut seen);
    let hidden: Vec<Point> = terrain.iter().map(|(p, _)| p).filter(|p| view.is_walkable(*p) && !seen.contains(*p)).collect();
    let rats: Vec<Point> = hidden.choose_multiple(&mut seed.rng(RATS, 0), RATS_PER_WALK).copied().collect();
    Walk::new(terrain, tiles, at, &rats, seed)
}

/// The walker's turn spent moving one cell, or striking the rat standing there. Refused, and
/// no turn spent, into a wall or once the walker is dead. Returns whether the turn was spent;
/// when it was, the rats have answered it by the time this returns.
pub fn step(walk: &mut Walk, dir: Direction) -> bool {
    if !walk.is_alive() {
        return false;
    }
    let to = walk.at + dir.offset();
    if let Some(i) = walk.rat_at(to) {
        let blow = apply_hit(walk.rats[i].health, YOUR_BLOW);
        walk.rats[i].health.current = blow.current;
        walk.log.push(if blow.died { "You kill the rat." } else { "You hit the rat." }.to_string());
        if blow.died && !walk.rats.iter().any(Rat::is_alive) {
            walk.log.push("That was the last of them.".to_string());
        }
    } else if walk.terrain.view(&walk.tiles).is_walkable(to) {
        walk.at = to;
        look(walk);
    } else {
        return false;
    }
    end_turn(walk);
    true
}

/// The walker's turn spent standing still, which lets a rat come to it. Refused once the walker
/// is dead. Returns whether the turn was spent.
pub fn wait(walk: &mut Walk) -> bool {
    if !walk.is_alive() {
        return false;
    }
    end_turn(walk);
    true
}

/// Puts the walker back in the queue one action from now and lets everyone due before it act.
fn end_turn(walk: &mut Walk) {
    walk.queue.insert_after(Who::You, BASE_ACTION_COST);
    rats_act(walk);
}

/// Deals turns to the rats until the walker is next, or dead.
///
/// [`step`] and [`wait`] call this; a loop that adds an action of its own queues the walker
/// after it and calls this too. With the walker not in the queue it returns at once rather than
/// dealing the rats turns forever.
pub fn rats_act(walk: &mut Walk) {
    if !walk.queue.contains(Who::You) {
        return;
    }
    let brain = Brain::new().then(MeleeAdjacent).then(Hunt);
    while walk.is_alive() {
        let rats = &walk.rats;
        match walk.queue.pop_due(|who| match who {
            Who::You => true,
            Who::Rat(i) => rats[i].is_alive(),
        }) {
            Some(Who::You) => return,
            Some(Who::Rat(i)) => {
                rat_turn(walk, i, &brain);
                walk.queue.insert_after(Who::Rat(i), BASE_ACTION_COST);
            }
            None => {
                if !walk.queue.advance_to_next() {
                    return;
                }
            }
        }
    }
}

/// One rat's turn: what it sees, what its brain makes of it, and the bite or the step.
///
/// The walker is in its snapshot only when the rat can see it from its own cell, so a rat out of
/// sight has nothing to hunt and waits. It steps only where nobody stands.
fn rat_turn(walk: &mut Walk, i: usize, brain: &Brain<Who>) {
    let me = walk.rats[i];
    let view = walk.terrain.view(&walk.tiles);
    let mut snapshot = Snapshot::alone(ActorView { id: Who::Rat(i), pos: me.at, health: Some(me.health), faction: None });
    if fov::can_see(&view, me.at, walk.at, SIGHT) {
        snapshot.enemies.push(ActorView { id: Who::You, pos: walk.at, health: Some(walk.health), faction: None });
    }
    let (you, rats) = (walk.at, &walk.rats);
    let can_step = |p: Point| view.is_walkable(p) && p != you && !rats.iter().any(|r| r.is_alive() && r.at == p);
    let blocks = |p: Point| view.is_opaque(p);
    let (decision, _) = brain.decide(&mut TacticCtx {
        snapshot: &snapshot,
        fields: &mut NoFields,
        can_step: &can_step,
        blocks_shot: &blocks,
        blocks_burst: &blocks,
        bounds: Rect::new(0, 0, view.width(), view.height()),
        rng: &mut walk.minds,
    });
    match decision {
        Decision::Attack(Who::You) => {
            let bite = apply_hit(walk.health, RAT_BITE);
            walk.health.current = bite.current.max(0);
            walk.log.push("The rat bites you.".to_string());
            if bite.died {
                walk.log.push("You die.".to_string());
            }
        }
        Decision::Step(to) => walk.rats[i].at = to,
        _ => {}
    }
}

/// Recomputes what is in sight and adds it to what is remembered.
pub fn look(walk: &mut Walk) {
    fov::compute(&walk.terrain.view(&walk.tiles), walk.at, SIGHT, &mut walk.seen);
    walk.explored.union_with(&walk.seen);
}

/// The status line: the walker's health, and how many rats are left.
///
/// Health is right-aligned to the width of its most, so the line keeps one length as it falls.
pub fn status(walk: &Walk) -> String {
    let left = walk.rats.iter().filter(|r| r.is_alive()).count();
    let width = walk.health.max.to_string().len();
    format!("HP {:>width$}/{}   Rats left {left}", walk.health.current, walk.health.max)
}

/// The picture as one row of characters for each row of the map; colour is the terminal loop's
/// business.
///
/// Sight and memory draw alike here, so a loop that wants remembered cells dimmer asks
/// `walk.seen` for each cell itself. A rat is drawn only while in sight: where one was last seen
/// is not where it is.
pub fn render(walk: &Walk) -> Vec<String> {
    let view = walk.terrain.view(&walk.tiles);
    (0..view.height())
        .map(|y| {
            (0..view.width())
                .map(|x| {
                    let p = Point::new(x, y);
                    if p == walk.at {
                        '@'
                    } else if walk.seen.contains(p) && walk.rat_at(p).is_some() {
                        'r'
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

    /// A walk on a hand-drawn map: `#` wall, `.` floor, `@` the walker and `r` a rat, each
    /// standing on floor.
    fn drawn(rows: &[&str]) -> Walk {
        let tiles = TileRegistry::standard();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        let cell = |p: Point| rows[p.y as usize].as_bytes()[p.x as usize];
        let terrain = Terrain::from_fn(rows[0].len() as i32, rows.len() as i32, |p| if cell(p) == b'#' { wall } else { floor });
        let cells: Vec<Point> = terrain.iter().map(|(p, _)| p).collect();
        let at = cells.iter().copied().find(|p| cell(*p) == b'@').expect("a drawn map has a walker");
        let rats: Vec<Point> = cells.iter().copied().filter(|p| cell(*p) == b'r').collect();
        Walk::new(terrain, tiles, at, &rats, RunSeed(1))
    }

    #[test]
    fn a_step_into_a_rat_strikes_it_and_does_not_move() {
        let mut walk = drawn(&["#####", "#@r.#", "#####"]);
        assert!(step(&mut walk, Direction::East));
        assert_eq!(walk.at, Point::new(1, 1));
        assert_eq!(walk.rats[0].at, Point::new(2, 1));
        assert_eq!(walk.rats[0].health.current, RAT_HEALTH - YOUR_BLOW);
    }

    #[test]
    fn a_rat_beside_you_bites_you_on_its_turn() {
        let mut walk = drawn(&["#####", "#@r.#", "#####"]);
        assert!(wait(&mut walk));
        assert_eq!(walk.health.current, YOUR_HEALTH - RAT_BITE);
        assert_eq!(walk.log.last().map(String::as_str), Some("The rat bites you."));
    }

    #[test]
    fn a_rat_that_sees_you_closes_in() {
        let mut walk = drawn(&["########", "#@....r#", "########"]);
        assert!(wait(&mut walk));
        assert_eq!(walk.rats[0].at, Point::new(5, 1));
    }

    #[test]
    fn a_rat_that_cannot_see_you_does_not_close_in() {
        let mut walk = drawn(&["#########", "#@..#..r#", "#...#...#", "#########"]);
        assert!(wait(&mut walk));
        assert_eq!(walk.rats[0].at, Point::new(7, 1), "a wall between them leaves the rat where it was");
    }

    #[test]
    fn two_rats_never_stand_on_one_cell() {
        // Both rats' straight line to the walker runs through (3, 2).
        let mut walk = drawn(&["#######", "#.r...#", "#....@#", "#.r...#", "#######"]);
        walk.health = Vitals { current: 100, max: 100 };
        for _ in 0..4 {
            assert!(wait(&mut walk));
            let (a, b) = (walk.rats[0].at, walk.rats[1].at);
            assert_ne!(a, b, "two rats on one cell");
            assert!(a != walk.at && b != walk.at, "a rat on the walker");
        }
    }

    #[test]
    fn a_rat_struck_to_nothing_is_gone_and_never_acts_again() {
        let mut walk = drawn(&["#####", "#@r.#", "#####"]);
        walk.rats[0].health.current = YOUR_BLOW;
        assert!(step(&mut walk, Direction::East));
        assert!(!walk.rats[0].is_alive());
        assert_eq!(walk.health.current, YOUR_HEALTH, "a dead rat does not bite");
        assert!(wait(&mut walk));
        assert_eq!(walk.health.current, YOUR_HEALTH, "nor on any turn after");
        assert!(!walk.queue.contains(Who::Rat(0)), "its turn is gone from the queue");
        assert!(!render(&walk).concat().contains('r'), "it is not drawn");
        assert!(step(&mut walk, Direction::East), "its cell is free to step on");
        assert_eq!(walk.at, Point::new(2, 1));
    }

    #[test]
    fn a_walker_struck_to_nothing_takes_no_more_steps() {
        let mut walk = drawn(&["######", "#.@r.#", "######"]);
        walk.health.current = RAT_BITE;
        assert!(wait(&mut walk));
        assert!(walk.health.current <= 0);
        assert_eq!(walk.log.last().map(String::as_str), Some("You die."));
        assert!(!step(&mut walk, Direction::West));
        assert!(!wait(&mut walk));
        assert_eq!(walk.at, Point::new(2, 1));
    }

    #[test]
    fn a_rat_is_drawn_only_while_in_sight() {
        let walk = drawn(&["#########", "#@.r#..r#", "#...#...#", "#########"]);
        let rows = render(&walk);
        assert_eq!(rows[1].chars().nth(3), Some('r'));
        assert_eq!(rows[1].chars().nth(7), Some(' '), "behind the wall is unexplored");
    }

    #[test]
    fn the_status_line_keeps_one_length_as_health_falls() {
        let mut walk = drawn(&["####", "#@.#", "####"]);
        let full = status(&walk);
        walk.health.current = 9;
        assert_eq!(status(&walk), "HP  9/10   Rats left 0");
        assert_eq!(status(&walk).len(), full.len());
    }

    #[test]
    fn every_rat_starts_on_floor_out_of_sight_and_alone_over_a_span_of_seeds() {
        for seed in 0..8 {
            let walk = generate(seed);
            assert_eq!(walk.rats.len(), RATS_PER_WALK, "seed {seed}");
            for (i, rat) in walk.rats.iter().enumerate() {
                assert!(walkable(&walk, rat.at), "seed {seed} puts a rat in a wall");
                assert!(!walk.seen.contains(rat.at), "seed {seed} starts a rat in sight");
                assert!(walk.rats[..i].iter().all(|r| r.at != rat.at), "seed {seed} stacks two rats");
            }
        }
    }
}
