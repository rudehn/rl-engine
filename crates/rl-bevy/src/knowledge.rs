//! What the player has seen.

use std::collections::BTreeMap;

use bevy::prelude::*;
use rl_core::Point;
use rl_grid::{BitGrid, Rgb};

use crate::places::MapId;

/// Tiles are kept in square buckets this many tiles a side, allocated only
/// where something was seen, so knowledge never costs more than what was
/// actually looked at.
///
/// A storage detail and nothing else. It is part of the save shape, so a game bumps its save version if this
/// ever changes.
const BUCKET: i32 = 64;

/// A set of tiles of one map, of any extent: what has been seen of it.
///
/// What [`Knowledge`] keeps for the player and a
/// [`Charts`](crate::charts::Charts) chart keeps for whoever fills it.
/// Ordered, so two runs of a seed walk it in one order.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TileSet(BTreeMap<Point, BitGrid>);

impl TileSet {
    fn split(p: Point) -> (Point, Point) {
        let bucket = Point::new(p.x.div_euclid(BUCKET), p.y.div_euclid(BUCKET));
        let local = Point::new(p.x.rem_euclid(BUCKET), p.y.rem_euclid(BUCKET));
        (bucket, local)
    }

    /// Whether `p` is in the set.
    pub fn contains(&self, p: Point) -> bool {
        let (bucket, local) = Self::split(p);
        self.0.get(&bucket).is_some_and(|g| g.contains(local))
    }

    /// Adds `p`.
    pub fn insert(&mut self, p: Point) {
        let (bucket, local) = Self::split(p);
        self.0.entry(bucket).or_insert_with(|| BitGrid::new(BUCKET, BUCKET)).insert(local);
    }

    /// How many tiles are in the set.
    pub fn count(&self) -> usize {
        self.0.values().map(|g| g.count()).sum()
    }

    /// Whether the set holds nothing.
    pub fn is_empty(&self) -> bool {
        self.0.values().all(|g| g.is_clear())
    }

    /// Every tile in the set, bucket by bucket, each row-major.
    pub fn iter(&self) -> impl Iterator<Item = Point> + '_ {
        self.0.iter().flat_map(|(bucket, grid)| grid.iter().map(move |local| Point::new(bucket.x * BUCKET + local.x, bucket.y * BUCKET + local.y)))
    }

    /// Adds every tile of `other`.
    pub fn union_with(&mut self, other: &TileSet) {
        for (bucket, grid) in &other.0 {
            match self.0.get_mut(bucket) {
                Some(mine) => mine.union_with(grid),
                None => {
                    self.0.insert(*bucket, grid.clone());
                }
            }
        }
    }

    /// The set as a save holds it: its buckets and the bits of each.
    pub fn export(&self) -> Vec<(Point, BitGrid)> {
        self.0.iter().map(|(p, g)| (*p, g.clone())).collect()
    }

    /// A set from what [`export`](Self::export) produced.
    pub fn import(buckets: Vec<(Point, BitGrid)>) -> Self {
        Self(buckets.into_iter().collect())
    }
}

/// How a still thing looked when it was last seen on a tile: what the map
/// draws there once the tile is out of sight.
///
/// A character and a plain colour rather than the renderer's own glyph,
/// because the save keeps it and the save does not sit above the renderer.
/// Eight bits a channel is what a definition authors anyway, so nothing a
/// player could tell apart is lost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Recalled {
    /// The character.
    pub glyph: char,
    /// Its colour, as it was in full light; whatever draws fades it.
    pub color: Rgb,
}

/// What has been seen of one map.
#[derive(Debug, Default)]
struct Seen {
    explored: TileSet,
    /// What stood still where, as last seen. A map keyed by tile rather
    /// than a grid, since a place holds a few hundred things over any
    /// extent; ordered, so a save writes it in one order.
    things: BTreeMap<Point, Recalled>,
    /// Tiles known without being seen whose still things the next look
    /// takes in. Not saved: that look is the frame they were learned in.
    learned: TileSet,
}

/// Explored tiles and the still things last seen on them.
///
/// Both are per map: the current map's are read, the others' kept aside.
#[derive(Resource, Debug, Default)]
pub struct Knowledge {
    here: Seen,
    current: MapId,
    stash: BTreeMap<MapId, Seen>,
}

impl Knowledge {
    /// The map whose explored tiles are being read.
    pub fn current(&self) -> MapId {
        self.current
    }

    /// Swaps in what was seen of `map`, keeping the current map's aside.
    pub fn switch(&mut self, map: MapId) {
        if map == self.current {
            return;
        }
        let incoming = self.stash.remove(&map).unwrap_or_default();
        let outgoing = std::mem::replace(&mut self.here, incoming);
        self.stash.insert(self.current, outgoing);
        self.current = map;
    }

    /// Whether the tile `p` of the current map has been seen.
    pub fn is_explored(&self, p: Point) -> bool {
        self.here.explored.contains(p)
    }

    /// Records that `p` has been seen on the current map.
    pub fn mark(&mut self, p: Point) {
        self.here.explored.insert(p);
    }

    /// Records that `p` of the current map is known without being seen, as
    /// a home town is known by heart: explored at once, and whatever stands
    /// still on it remembered as it stands at the next look, as though it
    /// had been seen then.
    ///
    /// The next look rather than now, because what stands there may not be
    /// drawable yet: a place's things are put down and dressed in the same
    /// frame that it is first entered.
    pub fn learn(&mut self, p: Point) {
        self.here.explored.insert(p);
        self.here.learned.insert(p);
    }

    /// What was last seen standing still on `p` of the current map, if
    /// anything was.
    ///
    /// Memory is what was seen, not what is true: a thing taken away out
    /// of sight is still here until its tile is seen again, and a thing put
    /// down out of sight is not here until then.
    pub fn recalled(&self, p: Point) -> Option<Recalled> {
        self.here.things.get(&p).copied()
    }

    /// Looks at the still things of the current map: every tile
    /// `in_sight`, and every one learned since the last look, now holds
    /// what `standing` puts on it, or nothing if it puts nothing. Every
    /// other tile keeps what it last held.
    ///
    /// `standing` is one look a tile, since which of several things on a
    /// tile is the one seen is a question about drawing; a tile not seen
    /// is skipped, so it may name every still thing on the map.
    pub fn see_still(&mut self, in_sight: impl Fn(Point) -> bool, standing: impl IntoIterator<Item = (Point, Recalled)>) {
        let learned = std::mem::take(&mut self.here.learned);
        let seen = |p: Point| in_sight(p) || learned.contains(p);
        self.here.things.retain(|p, _| !seen(*p));
        self.here.things.extend(standing.into_iter().filter(|(p, _)| seen(*p)));
    }

    /// Explored tiles of the current map.
    pub fn explored_count(&self) -> usize {
        self.here.explored.count()
    }

    /// Everything known, for saving.
    pub fn export(&self) -> KnowledgeSave {
        let maps: Vec<(MapId, &Seen)> = self.stash.iter().map(|(m, s)| (*m, s)).chain(std::iter::once((self.current, &self.here))).collect();
        KnowledgeSave {
            current: self.current,
            explored: maps.iter().map(|(m, s)| (*m, s.explored.export())).collect(),
            recalled: maps.iter().filter(|(_, s)| !s.things.is_empty()).map(|(m, s)| (*m, s.things.iter().map(|(p, r)| (*p, *r)).collect())).collect(),
        }
    }

    /// Replaces everything known with a save.
    pub fn import(&mut self, save: KnowledgeSave) {
        self.stash = BTreeMap::new();
        for (m, e) in save.explored {
            self.stash.entry(m).or_default().explored = TileSet::import(e);
        }
        for (m, things) in save.recalled {
            self.stash.entry(m).or_default().things = things.into_iter().collect();
        }
        self.here = self.stash.remove(&save.current).unwrap_or_default();
        self.current = save.current;
    }
}

/// What [`Knowledge::export`] produces.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KnowledgeSave {
    /// The map being read.
    pub current: MapId,
    /// Explored tiles per map, in buckets.
    pub explored: Vec<(MapId, Vec<(Point, BitGrid)>)>,
    /// What stood still where, as last seen, per map. Empty in a save
    /// written before still things were remembered, which a run continues
    /// as though none had been seen yet.
    #[serde(default)]
    pub recalled: Vec<(MapId, Vec<(Point, Recalled)>)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_survive_across_buckets_and_negative_coordinates() {
        let mut k = Knowledge::default();
        k.mark(Point::new(BUCKET + 1, 3));
        k.mark(Point::new(-1, -1));
        assert!(k.is_explored(Point::new(BUCKET + 1, 3)));
        assert!(k.is_explored(Point::new(-1, -1)));
        assert!(!k.is_explored(Point::new(BUCKET + 2, 3)));
        assert_eq!(k.explored_count(), 2);
    }

    #[test]
    fn a_tile_set_unions_iterates_and_round_trips_across_buckets() {
        let mut a = TileSet::default();
        assert!(a.is_empty());
        a.insert(Point::new(3, 3));
        a.insert(Point::new(-1, BUCKET + 2));
        let mut b = TileSet::default();
        b.insert(Point::new(3, 4));
        b.insert(Point::new(-1, BUCKET + 2));
        b.insert(Point::new(BUCKET * 2, 0));
        a.union_with(&b);
        assert_eq!(a.count(), 4, "the one both held is counted once");
        let mut all: Vec<Point> = a.iter().collect();
        all.sort();
        let mut expected = vec![Point::new(-1, BUCKET + 2), Point::new(3, 3), Point::new(3, 4), Point::new(BUCKET * 2, 0)];
        expected.sort();
        assert_eq!(all, expected);
        assert_eq!(TileSet::import(a.export()), a);
        assert!(!a.contains(Point::new(4, 4)));
    }

    #[test]
    fn a_save_keeps_every_map_apart() {
        let mut k = Knowledge::default();
        k.mark(Point::new(3, 3));
        k.switch(MapId(4));
        k.mark(Point::new(5, 5));

        let mut back = Knowledge::default();
        back.import(k.export());
        assert_eq!(back.current(), MapId(4));
        assert!(back.is_explored(Point::new(5, 5)));
        assert!(!back.is_explored(Point::new(3, 3)));
        back.switch(MapId::ZERO);
        assert!(back.is_explored(Point::new(3, 3)));
    }

    fn look(glyph: char) -> Recalled {
        Recalled { glyph, color: Rgb::new(200, 180, 90) }
    }

    /// Only what is in sight is looked at: a tile out of sight keeps what
    /// it held whatever stands there now, and one in sight holds what
    /// stands there now or nothing.
    #[test]
    fn a_look_replaces_what_is_in_sight_and_keeps_the_rest() {
        let mut k = Knowledge::default();
        let (near, far) = (Point::new(1, 0), Point::new(9, 0));
        k.see_still(|_| true, [(near, look('>')), (far, look('!'))]);
        k.see_still(|p| p == near, [(far, look('?'))]);
        assert_eq!(k.recalled(near), None, "in sight, and nothing there now");
        assert_eq!(k.recalled(far), Some(look('!')), "out of sight, as it was seen");
    }

    /// A tile learned is looked at once, by the next look, and then kept
    /// like any other out of sight.
    #[test]
    fn a_learned_tile_is_looked_at_by_the_next_look_and_only_that_one() {
        let mut k = Knowledge::default();
        let p = Point::new(4, 4);
        k.learn(p);
        assert!(k.is_explored(p));
        k.see_still(|_| false, [(p, look('&'))]);
        assert_eq!(k.recalled(p), Some(look('&')));
        k.see_still(|_| false, std::iter::empty());
        assert_eq!(k.recalled(p), Some(look('&')), "the next look does not see it again");
    }

    /// What stood where is per map, as explored tiles are: going down a
    /// staircase and back up remembers the floor above.
    #[test]
    fn still_things_are_remembered_per_map_across_a_switch_and_a_save() {
        let mut k = Knowledge::default();
        let p = Point::new(3, 3);
        k.see_still(|_| true, [(p, look('>'))]);
        k.switch(MapId(2));
        assert_eq!(k.recalled(p), None, "another map remembers nothing there");
        k.see_still(|_| true, [(p, look('<'))]);
        k.switch(MapId::ZERO);
        assert_eq!(k.recalled(p), Some(look('>')), "and coming back remembers the first");

        let mut back = Knowledge::default();
        back.import(k.export());
        assert_eq!((back.current(), back.recalled(p)), (MapId::ZERO, Some(look('>'))));
        back.switch(MapId(2));
        assert_eq!(back.recalled(p), Some(look('<')), "every map's memory is saved");
    }
}
