//! What a place is, and how a game builds one: plain data, below Bevy.
//!
//! A game's [`PlaceRules`] build each place from its own [`PlaceSeed`]
//! into a [`PlaceBuild`]: the tiles, where an arrival stands, the
//! [`Spot`]s it reports for the game to populate, its [`Lock`]s and the
//! cells it hid. A game whose places are floors one under another says
//! so with a [`Ladder`], whose [`stairs`](Ladder::stairs) are each floor's
//! [`LadderStair`]s.
//!
//! Kept here rather than in the Bevy layer so a terminal game, a tool or a
//! test builds the very places the framework does, with no `App`. The
//! framework, `atglyph-bevy`, re-exports every one of these from its
//! `places` module, keeps each build as a place of its world map, and lays
//! each ladder stair as an entity a traveller goes through.

use atglyph_core::{PlaceSeed, Point};
use atglyph_grid::{Light, Rgb, Terrain, TileTables};

use crate::accrete::{Join, Opening};
use crate::chain::BuildError;
use crate::context::BaseContext;
use crate::dungeon::ExitPoint;
use crate::passes::StartPoint;
use crate::prefab::Stamped;
use crate::secrets::SecretCell;

/// Which map something is on. A game numbers its places however it
/// likes; zero is where the framework puts an entity that names no map.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub struct MapId(pub u32);

impl MapId {
    /// Map zero, where an entity that names no map stands.
    pub const ZERO: MapId = MapId(0);

    /// Map one, the first place a game that numbers its places from one
    /// builds, so a game with one place need not number it at all.
    pub const FIRST: MapId = MapId(1);
}

/// A point of interest the builder reports, for the game to populate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Spot {
    /// What kind, in the game's own numbering.
    pub tag: u32,
    /// Where.
    pub at: Point,
    /// The key of the prefab whose mark this is, when it was keyed, so the
    /// engine can find what the mark stands for. `None` for a spot a game
    /// made itself or a mark of an unkeyed piece. A mark a later stamp
    /// covered is never a spot, since the piece drawn on top owns its
    /// cells and what the earlier one meant there is no longer on the map.
    #[serde(default)]
    pub prefab: Option<u32>,
}

/// A cell of a place only some may open: a door locked to a tag, or a
/// gate nothing opens until the game lifts it.
///
/// Kept by the place rather than the tile, so one locked-door tile serves
/// every lock and each opens to its own key. In the framework an opener
/// carries the tag when it is itself tagged with it or holds an item
/// tagged with it, so a game says who carries a key by giving it to
/// someone, a guard or the player, or by tagging whoever should open it.
/// Opening a lock spends it: the door opens into its open tile, which
/// closes as an ordinary door.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Lock {
    /// Where.
    pub at: Point,
    /// The raw id of the tag an opener must carry, as
    /// [`Join::Locked`] carries it, which `atglyph_rules::TagId::from_raw`
    /// turns back into the tag; `None` for a seal, which nothing opens
    /// until the game takes it off. Raw because this crate sits below the
    /// rules that name tags, and a save writes a tag id as this same
    /// number, so the two read each other's.
    pub key: Option<u32>,
}

/// What building a place produced.
#[derive(Debug)]
pub struct PlaceBuild {
    /// The tiles.
    pub terrain: Terrain,
    /// Where an arrival from above stands.
    pub entry: Point,
    /// Where an arrival from below stands, if the place goes deeper.
    pub exit: Option<Point>,
    /// Points of interest for the game's spawns.
    pub spots: Vec<Spot>,
    /// Every door locked to a tag and every seal.
    pub locks: Vec<Lock>,
    /// Every cell the builder hid, walled over in `terrain`, in the order
    /// [`HideOpenings`](crate::HideOpenings) published them.
    pub secrets: Vec<SecretCell>,
    /// The light everywhere in the place before anything in it is lit,
    /// which the framework's lighting makes the ambient whenever the place
    /// is current. Kept with the place and its save, so the builder that
    /// knew why a floor is lit is the only thing that says so.
    /// [`Light::DARK`] is no ambient, and is what
    /// [`from_context`](Self::from_context) gives.
    pub ambient: Light,
    /// Whether nothing in the place is harmed: every hit on anyone standing
    /// in it lands as nothing, and no fight with anyone in it is forecast.
    /// A town, where the people are not there to be fought. Kept with the
    /// place and its save, so the rule is the place's and not a flag a
    /// game keeps in step with whichever map is current. False from
    /// [`from_context`](Self::from_context).
    pub peaceful: bool,
}

impl PlaceBuild {
    /// A build from a finished chain: the entry is the chain's
    /// [`StartPoint`], the exit its [`ExitPoint`] if one was emitted, and
    /// every prefab mark becomes a spot tagged with its character.
    /// Fails if no start was emitted.
    ///
    /// A mark inside the bounds of a stamp emitted after its own is
    /// dropped, keyed or not: the piece drawn on top owns its cells, so a
    /// slot or a mark it painted over never fills a cell that now belongs
    /// to another piece. Every cell of an [`Opening`] joined
    /// [`Locked`](Join::Locked) becomes a [`Lock`] to that tag, and every
    /// one joined [`Sealed`](Join::Sealed) a seal. Every [`SecretCell`] a
    /// pass published is carried as it was. The ambient is dark and the
    /// place is not peaceful; a builder lighting the place or keeping its
    /// peace sets [`ambient`](Self::ambient) or [`peaceful`](Self::peaceful)
    /// on what this returns.
    pub fn from_context(ctx: BaseContext) -> Result<Self, BuildError> {
        let entry = ctx.outputs().first::<StartPoint>().ok_or_else(|| BuildError::new("place", "the chain emitted no start point"))?.0;
        let exit = ctx.outputs().first::<ExitPoint>().map(|e| e.0);
        let stamps: Vec<&Stamped> = ctx.outputs().iter::<Stamped>().collect();
        let covered = |i: usize, p: Point| stamps[i + 1..].iter().any(|later| later.bounds.contains(p));
        let spots = stamps
            .iter()
            .enumerate()
            .flat_map(|(i, s)| s.marks.iter().filter(move |(_, p)| !covered(i, *p)).map(|(c, p)| Spot { tag: *c as u32, at: *p, prefab: s.prefab }))
            .collect();
        let locks = ctx
            .outputs()
            .iter::<Opening>()
            .filter_map(|o| match o.join {
                Join::Locked(tag) => Some((o, Some(tag))),
                Join::Sealed => Some((o, None)),
                _ => None,
            })
            .flat_map(|(o, key)| o.cells.iter().map(move |at| Lock { at: *at, key }))
            .collect();
        let secrets = ctx.outputs().iter::<SecretCell>().copied().collect();
        let (terrain, _) = ctx.finish();
        Ok(Self { terrain, entry, exit, spots, locks, secrets, ambient: Light::DARK, peaceful: false })
    }
}

/// How the game builds a place the first time it is entered.
///
/// The framework asks it once a place, the first time anything enters;
/// a tool or a terminal game asks it directly, with nothing but this crate
/// and the two below it:
///
/// ```
/// use atglyph_core::{PlaceSeed, RunSeed};
/// use atglyph_grid::{TileRegistry, TileTables};
/// use atglyph_mapgen::dungeon::{FarthestExit, RandomStart, Rooms};
/// use atglyph_mapgen::place::{MapId, PlaceBuild, PlaceRules};
/// use atglyph_mapgen::{BaseContext, BuildError, Chain};
///
/// /// Two rooms or more, a start in one and the way down as far from it
/// /// as the floor allows.
/// struct TwoRooms(TileRegistry);
///
/// impl PlaceRules for TwoRooms {
///     fn build(&self, _: MapId, seed: PlaceSeed) -> Result<PlaceBuild, BuildError> {
///         let (wall, floor) = (self.0.expect("wall"), self.0.expect("floor"));
///         let mut ctx = BaseContext::blank(40, 20, self.0.clone(), wall);
///         let rooms = Rooms { floor, min_rooms: 2, ..Default::default() };
///         Chain::new().then(rooms).then(RandomStart).then(FarthestExit).run(&mut ctx, seed)?;
///         PlaceBuild::from_context(ctx)
///     }
///
///     fn tiles(&self) -> TileTables {
///         self.0.tables()
///     }
/// }
///
/// let rules = TwoRooms(TileRegistry::standard());
/// let place = rules.build(MapId::FIRST, PlaceSeed::of(RunSeed(7), MapId::FIRST.0)).unwrap();
/// let view = place.terrain.view(&rules.0);
/// assert!(view.is_walkable(place.entry), "an arrival stands on floor");
/// assert!(place.exit.is_some_and(|exit| exit != place.entry), "and the way down is elsewhere");
/// ```
pub trait PlaceRules: Send + Sync {
    /// Builds `map` from `seed`, the place's own share of the run's seed.
    ///
    /// Handed in rather than kept in the rules, so the rules are made once
    /// with the game and not again for every run, and derived from the run
    /// and the map alone, so the order places are built in changes none of
    /// them. A framework game with no run seed builds nothing, and its warp
    /// fails with a [`BuildError`] naming it.
    fn build(&self, map: MapId, seed: PlaceSeed) -> Result<PlaceBuild, BuildError>;

    /// The tile tables every place of the game is built of. The framework
    /// makes the run's world map over them when a start makes none.
    fn tiles(&self) -> TileTables;

    /// The floors this game's places stand in one under another, if they
    /// do, for the engine to lay the stairs between them. `None`, the
    /// default, is a game with no ladder, whose ways through are all its
    /// own: it gets no stairs.
    ///
    /// Asked each time a place is built rather than once, so it may be
    /// made on the spot from the game's own constants.
    fn ladder(&self) -> Option<Ladder> {
        None
    }
}

/// Which way a stair goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub enum Climb {
    /// To the floor above, from a floor's entry to that floor's exit.
    Up,
    /// To the floor below, from a floor's exit to that floor's entry.
    Down,
}

/// How a ladder's stairs one way look and are called: the glyph and the
/// colour whoever draws gives them, and the name a panel and the look
/// cursor show.
///
/// Plain data, so the engine keeps it on every stair it lays and in the
/// save, and a stair comes back looking as it was laid. A colour in bytes,
/// as a prop's look is, since nothing below the drawing names a `Color`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StairLook {
    /// The character drawn.
    pub glyph: char,
    /// Its colour.
    pub color: Rgb,
    /// What it is called.
    pub name: String,
}

impl StairLook {
    /// A stair drawn `glyph` in `color` and called `name`.
    pub fn new(glyph: char, color: Rgb, name: impl Into<String>) -> Self {
        Self { glyph, color, name: name.into() }
    }
}

/// What one end of a [`Ladder`] has beyond its last stair.
///
/// The engine lays no stair past either end whichever it is: the top floor
/// has no stair up and the bottom none down. The word is the game's
/// account of that end, kept on the ladder so whatever reads the ladder,
/// a test of every floor's ways or a tool listing them, knows a way there
/// is meant and is the game's to lay, rather than missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LadderEnd {
    /// Nothing: the run goes no further that way.
    Nothing,
    /// A way of the game's own, which it lays itself at that floor's
    /// entry for the top or exit for the bottom: a gate to a town, a lift
    /// out that ends a mission, a crack of daylight.
    Game,
}

/// A run's floors, one under another, and how the stairs between them
/// look.
///
/// Every floor but the top gets a stair up at its entry, to the floor
/// above's exit, and every floor but the bottom a stair down at its exit,
/// to the floor below's entry, so going up and coming back down lands
/// where the way was taken. The framework lays them when the floor is
/// built, by whoever gets there first, so a monster that ran down ahead
/// of the player has a way back up. A floor whose build has no exit gets
/// no stair down.
#[derive(Debug, Clone, PartialEq)]
pub struct Ladder {
    /// The floors, top first: floor `n` is `floors[n - 1]`, and its
    /// neighbours are the entries either side.
    pub floors: Vec<MapId>,
    /// How a stair up looks.
    pub up: StairLook,
    /// How a stair down looks.
    pub down: StairLook,
    /// What the top floor has instead of a stair up.
    pub top: LadderEnd,
    /// What the bottom floor has instead of a stair down.
    pub bottom: LadderEnd,
}

/// One stair a [`Ladder`] lays on a floor: where it stands, which way it
/// goes, how it looks, and the floor it leads to.
///
/// Where on that floor it lands follows from the way: a stair up arrives
/// at the floor above's exit and a stair down at the floor below's entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LadderStair {
    /// The cell it stands on: the floor's entry for a stair up, its exit
    /// for a stair down.
    pub at: Point,
    /// Up or down.
    pub climb: Climb,
    /// How it is drawn and called.
    pub look: StairLook,
    /// The floor it leads to.
    pub to: MapId,
}

impl Ladder {
    /// `floors`, top first, with stairs looking `up` and `down`, and
    /// nothing past either end.
    pub fn new(floors: impl IntoIterator<Item = MapId>, up: StairLook, down: StairLook) -> Self {
        Self { floors: floors.into_iter().collect(), up, down, top: LadderEnd::Nothing, bottom: LadderEnd::Nothing }
    }

    /// The same ladder with `end` at its top.
    pub fn top(mut self, end: LadderEnd) -> Self {
        self.top = end;
        self
    }

    /// The same ladder with `end` at its bottom.
    pub fn bottom(mut self, end: LadderEnd) -> Self {
        self.bottom = end;
        self
    }

    /// Which floor `map` is, counting from one at the top, if it is one.
    #[cfg(test)]
    pub(crate) fn floor_of(&self, map: MapId) -> Option<usize> {
        self.floors.iter().position(|m| *m == map).map(|i| i + 1)
    }

    /// The floor above `map`, if `map` is a floor and not the top.
    pub(crate) fn above(&self, map: MapId) -> Option<MapId> {
        let i = self.floors.iter().position(|m| *m == map)?;
        i.checked_sub(1).map(|i| self.floors[i])
    }

    /// The floor below `map`, if `map` is a floor and not the bottom.
    pub(crate) fn below(&self, map: MapId) -> Option<MapId> {
        let i = self.floors.iter().position(|m| *m == map)?;
        self.floors.get(i + 1).copied()
    }

    /// The stairs floor `map` gets, built with `entry` and `exit`: the
    /// stair up first, then the stair down. Empty for a map that is no
    /// floor of the ladder.
    pub fn stairs(&self, map: MapId, entry: Point, exit: Option<Point>) -> Vec<LadderStair> {
        let mut stairs = Vec::new();
        if let Some(above) = self.above(map) {
            stairs.push(LadderStair { at: entry, climb: Climb::Up, look: self.up.clone(), to: above });
        }
        if let (Some(below), Some(exit)) = (self.below(map), exit) {
            stairs.push(LadderStair { at: exit, climb: Climb::Down, look: self.down.clone(), to: below });
        }
        stairs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Chain;
    use crate::prefab::{Orient, Placement, Prefab, StampPrefab};
    use atglyph_core::Direction;
    use atglyph_grid::TileRegistry;

    #[test]
    fn a_keyed_stamps_marks_become_spots_carrying_its_key() {
        let tiles = TileRegistry::standard();
        let wall = tiles.expect("wall");
        let piece = Prefab::parse(&["#A#"], |c| (c == '#').then_some(wall)).unwrap().keyed(3);
        let mut ctx = BaseContext::blank(10, 10, tiles.clone(), tiles.expect("floor"));
        Chain::new()
            .then(StampPrefab { name: "keyed", prefab: piece, at: Placement::At(Point::new(1, 1)), orient: Orient::Fixed })
            .run(&mut ctx, atglyph_core::RunSeed(0))
            .unwrap();
        ctx.emit(StartPoint(Point::new(5, 5)));
        let build = PlaceBuild::from_context(ctx).unwrap();
        assert_eq!(build.spots, vec![Spot { tag: 'A' as u32, at: Point::new(2, 1), prefab: Some(3) }]);
    }

    #[test]
    fn a_build_carries_out_every_secret_its_chain_hid_as_it_was_published() {
        let tiles = TileRegistry::standard();
        let mut ctx = BaseContext::blank(10, 10, tiles.clone(), tiles.expect("floor"));
        let hid = [
            SecretCell { at: Point::new(3, 1), is: tiles.expect("floor"), spot: 6, group: 0, joins: (0, 1) },
            SecretCell { at: Point::new(3, 2), is: tiles.expect("floor"), spot: 6, group: 0, joins: (0, 1) },
        ];
        for s in hid {
            ctx.emit(s);
        }
        ctx.emit(StartPoint(Point::new(5, 5)));
        assert_eq!(PlaceBuild::from_context(ctx).unwrap().secrets, hid.to_vec());
    }

    #[test]
    fn a_mark_under_a_later_stamp_is_dropped_and_the_piece_on_top_keeps_its_own() {
        let tiles = TileRegistry::standard();
        let wall = tiles.expect("wall");
        let under = Prefab::parse(&["A.B"], |c| (c == '.').then_some(wall)).unwrap().keyed(1);
        let over = Prefab::parse(&["C"], |_| None).unwrap().keyed(2);
        let mut ctx = BaseContext::blank(10, 10, tiles.clone(), tiles.expect("floor"));
        Chain::new()
            .then(StampPrefab { name: "under", prefab: under, at: Placement::At(Point::new(1, 1)), orient: Orient::Fixed })
            .then(StampPrefab { name: "over", prefab: over, at: Placement::At(Point::new(3, 1)), orient: Orient::Fixed })
            .run(&mut ctx, atglyph_core::RunSeed(0))
            .unwrap();
        ctx.emit(StartPoint(Point::new(5, 5)));
        let build = PlaceBuild::from_context(ctx).unwrap();
        assert_eq!(
            build.spots,
            vec![Spot { tag: 'A' as u32, at: Point::new(1, 1), prefab: Some(1) }, Spot { tag: 'C' as u32, at: Point::new(3, 1), prefab: Some(2) }],
            "the first piece's `B` at (3, 1) is under the second, whose `C` owns the cell"
        );
    }

    /// Every cell of a locked opening is a lock to the opening's tag, as
    /// the raw id the join carried, every cell of a sealed one a seal, and
    /// an opening joined any other way no lock at all.
    #[test]
    fn a_locked_opening_locks_each_cell_to_its_tag_and_a_sealed_one_seals_them() {
        let tiles = TileRegistry::standard();
        let mut ctx = BaseContext::blank(10, 10, tiles.clone(), tiles.expect("floor"));
        let opening = |cells: Vec<Point>, join| Opening { cells, facing: Direction::East, areas: (0, 1), join };
        ctx.emit(opening(vec![Point::new(2, 1), Point::new(2, 2)], Join::Locked(5)));
        ctx.emit(opening(vec![Point::new(6, 1)], Join::Sealed));
        ctx.emit(opening(vec![Point::new(8, 1)], Join::Arch));
        ctx.emit(StartPoint(Point::new(5, 5)));
        assert_eq!(
            PlaceBuild::from_context(ctx).unwrap().locks,
            vec![Lock { at: Point::new(2, 1), key: Some(5) }, Lock { at: Point::new(2, 2), key: Some(5) }, Lock { at: Point::new(6, 1), key: None }]
        );
    }

    /// A chain that placed no start has nowhere for an arrival to stand,
    /// so it is no place.
    #[test]
    fn a_chain_with_no_start_builds_no_place() {
        let tiles = TileRegistry::standard();
        let ctx = BaseContext::blank(10, 10, tiles.clone(), tiles.expect("floor"));
        assert!(PlaceBuild::from_context(ctx).is_err());
    }

    /// The ladder's neighbours are read off its list, top first, and a map
    /// that is no floor of it has none and no stairs.
    #[test]
    fn a_ladder_names_each_floors_neighbours_and_nothing_for_a_map_off_it() {
        let look = StairLook::new('<', Rgb::WHITE, "stairs");
        let ladder = Ladder::new([MapId(3), MapId(4), MapId(5)], look.clone(), look);
        assert_eq!((ladder.floor_of(MapId(3)), ladder.floor_of(MapId(5)), ladder.floor_of(MapId(9))), (Some(1), Some(3), None));
        assert_eq!(
            (ladder.above(MapId(3)), ladder.above(MapId(4)), ladder.below(MapId(4)), ladder.below(MapId(5))),
            (None, Some(MapId(3)), Some(MapId(5)), None)
        );
        assert!(ladder.stairs(MapId(9), Point::ZERO, Some(Point::new(1, 1))).is_empty(), "a map off the ladder gets no stairs");
        assert_eq!((ladder.top, ladder.bottom), (LadderEnd::Nothing, LadderEnd::Nothing), "nothing past either end unless the game says");
    }

    /// A middle floor gets a stair up on its entry to the floor above and
    /// a stair down on its exit to the floor below, each looking as the
    /// ladder says; the top gets only the way down, the bottom only the
    /// way up, and a floor built with no exit no way down.
    #[test]
    fn a_ladder_lays_an_up_stair_at_the_entry_and_a_down_stair_at_the_exit() {
        let (up, down) = (StairLook::new('<', Rgb::WHITE, "stairs up"), StairLook::new('>', Rgb::WHITE, "stairs down"));
        let ladder = Ladder::new([MapId(1), MapId(2), MapId(3)], up.clone(), down.clone());
        let (entry, exit) = (Point::new(2, 3), Point::new(7, 8));
        let up_from = |to| LadderStair { at: entry, climb: Climb::Up, look: up.clone(), to };
        let down_from = |to| LadderStair { at: exit, climb: Climb::Down, look: down.clone(), to };
        assert_eq!(ladder.stairs(MapId(2), entry, Some(exit)), vec![up_from(MapId(1)), down_from(MapId(3))], "a middle floor");
        assert_eq!(ladder.stairs(MapId(1), entry, Some(exit)), vec![down_from(MapId(2))], "the top");
        assert_eq!(ladder.stairs(MapId(3), entry, Some(exit)), vec![up_from(MapId(2))], "the bottom");
        assert_eq!(ladder.stairs(MapId(2), entry, None), vec![up_from(MapId(1))], "a floor with no exit");
    }
}
