//! What a place's tiles are kept in: the whole of them, or, for a
//! [`Streamed`] place, the chunks within reach of the player.
//!
//! A streamed place keeps a window of chunks loaded. A chunk that leaves
//! the window is dropped and only what was changed on it is kept, by tile,
//! the last change to each; when the chunk comes back into reach it is
//! built again from the rules and those changes are laid back on it. So an
//! untouched chunk costs nothing to keep, however much of the place has
//! been walked.

use std::collections::BTreeMap;

use atglyph_core::{Grid2D, GridRect, Point};
use atglyph_grid::{BitGrid, Terrain, TileId};

use crate::places::{BuildError, Streamed};

/// A place's tiles.
#[derive(Debug)]
pub(crate) enum Ground {
    /// Every tile, built at once.
    Whole(Terrain),
    /// The chunks within reach, built as they come into it.
    Streamed(Surface),
}

impl Ground {
    /// The whole place, loaded or not.
    pub(crate) fn bounds(&self) -> GridRect {
        match self {
            Ground::Whole(terrain) => terrain.bounds(),
            Ground::Streamed(surface) => surface.layout.bounds(),
        }
    }

    /// The tile at `p`, or `None` off the place or on a chunk not loaded.
    pub(crate) fn get(&self, p: Point) -> Option<TileId> {
        match self {
            Ground::Whole(terrain) => terrain.get(p),
            Ground::Streamed(surface) => surface.get(p),
        }
    }

    /// Sets the tile at `p`, answering what was there, or `None` if `p` is
    /// off the place or on a chunk not loaded.
    pub(crate) fn set(&mut self, p: Point, id: TileId) -> Option<TileId> {
        match self {
            Ground::Whole(terrain) => {
                let was = terrain.get(p)?;
                terrain.set(p, id).then_some(was)
            }
            Ground::Streamed(surface) => surface.set(p, id),
        }
    }

    /// The tiles readers can read: the whole place, or its loaded window.
    pub(crate) fn window(&self) -> GridRect {
        match self {
            Ground::Whole(terrain) => terrain.bounds(),
            Ground::Streamed(surface) => surface.window_tiles(),
        }
    }
}

/// One loaded chunk: its tiles, and what has been changed on it since it
/// was built, the last change to each tile, by its index in the chunk.
#[derive(Debug)]
struct Chunk {
    terrain: Terrain,
    edits: BTreeMap<u32, TileId>,
}

/// A streamed place's chunks: the window loaded round the player, the
/// edits of every chunk out of it, and which chunks have ever been built.
#[derive(Debug)]
pub(crate) struct Surface {
    pub(crate) layout: Streamed,
    /// The loaded chunks, in chunks; empty until the first load.
    window: GridRect,
    /// The window's chunks, row by row.
    chunks: Vec<Chunk>,
    /// The edits of every chunk out of the window that has any.
    kept: BTreeMap<Point, BTreeMap<u32, TileId>>,
    /// Every chunk ever built, so a game populates each once.
    made: BitGrid,
    /// Every loaded chunk whose glowing tiles have their light up. Never
    /// saved, as a whole place's light is not: a restored run lights each
    /// chunk again as it loads.
    lit: BitGrid,
}

/// A chunk that came into the window: which, and whether for the first
/// time in the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Loaded {
    pub(crate) chunk: Point,
    pub(crate) first: bool,
}

impl Surface {
    /// A surface laid out as `layout`, nothing loaded and nothing built.
    pub(crate) fn new(layout: Streamed) -> Surface {
        let (w, h) = layout.chunks;
        Surface { layout, window: GridRect::new(0, 0, 0, 0), chunks: Vec::new(), kept: BTreeMap::new(), made: BitGrid::new(w, h), lit: BitGrid::new(w, h) }
    }

    /// A surface restored from its edits and the chunks once built, with
    /// nothing loaded: the next load rebuilds what it needs.
    pub(crate) fn restored(layout: Streamed, edits: Vec<(Point, Vec<(u32, TileId)>)>, made: Vec<Point>) -> Surface {
        let mut surface = Surface::new(layout);
        surface.kept = edits.into_iter().map(|(chunk, e)| (chunk, e.into_iter().collect())).collect();
        for chunk in made {
            surface.made.insert(chunk);
        }
        surface
    }

    /// The window in tiles; empty until the first load.
    pub(crate) fn window_tiles(&self) -> GridRect {
        let (w, s) = (self.window, self.layout.chunk);
        GridRect::new(w.x * s, w.y * s, w.width * s, w.height * s)
    }

    /// Where `p` is kept: the slot of its chunk in the window, and its
    /// index in the chunk.
    fn locate(&self, p: Point) -> Option<(usize, u32)> {
        let chunk = self.layout.chunk_of(p)?;
        let w = self.window;
        if !w.contains(chunk) {
            return None;
        }
        let s = self.layout.chunk;
        let slot = ((chunk.y - w.y) * w.width + (chunk.x - w.x)) as usize;
        Some((slot, ((p.y - chunk.y * s) * s + (p.x - chunk.x * s)) as u32))
    }

    fn get(&self, p: Point) -> Option<TileId> {
        let (slot, i) = self.locate(p)?;
        Some(self.chunks[slot].terrain.get_idx(i as usize))
    }

    fn set(&mut self, p: Point, id: TileId) -> Option<TileId> {
        let (slot, i) = self.locate(p)?;
        let chunk = &mut self.chunks[slot];
        let was = chunk.terrain.get_idx(i as usize);
        if was != id {
            chunk.terrain.set_idx(i as usize, id);
            chunk.edits.insert(i, id);
        }
        Some(was)
    }

    /// Whether the window is `chunks` already.
    pub(crate) fn holds(&self, chunks: GridRect) -> bool {
        self.window == chunks && !self.chunks.is_empty()
    }

    /// Moves the window onto `chunks`: keeps what it already holds there,
    /// builds the rest with `build` and lays each one's kept edits back on
    /// it, and lets go of what left, keeping the edits of each. Answers
    /// every chunk that came in.
    ///
    /// Nothing is changed if a build fails, so a failed load leaves the
    /// window where it was.
    pub(crate) fn load(&mut self, chunks: GridRect, mut build: impl FnMut(Point) -> Result<Terrain, BuildError>) -> Result<Vec<Loaded>, BuildError> {
        if self.holds(chunks) {
            return Ok(Vec::new());
        }
        let side = self.layout.chunk;
        let mut built: Vec<(Point, Terrain)> = Vec::new();
        for chunk in cells(chunks) {
            if self.window.contains(chunk) && !self.chunks.is_empty() {
                continue;
            }
            let terrain = build(chunk)?;
            if (terrain.width(), terrain.height()) != (side, side) {
                return Err(BuildError::new("build_chunk", format!("chunk {chunk:?} is {}x{}, not {side}x{side}", terrain.width(), terrain.height())));
            }
            built.push((chunk, terrain));
        }
        let mut old: BTreeMap<Point, Chunk> = cells(self.window).zip(self.chunks.drain(..)).collect();
        let mut fresh: BTreeMap<Point, Terrain> = built.into_iter().collect();
        let mut loaded = Vec::new();
        for chunk in cells(chunks) {
            let next = match old.remove(&chunk) {
                Some(kept) => kept,
                None => {
                    let mut terrain = fresh.remove(&chunk).expect("every chunk not kept was built");
                    let edits = self.kept.remove(&chunk).unwrap_or_default();
                    for (&i, &id) in &edits {
                        terrain.set_idx(i as usize, id);
                    }
                    let first = !self.made.contains(chunk);
                    self.made.insert(chunk);
                    loaded.push(Loaded { chunk, first });
                    Chunk { terrain, edits }
                }
            };
            self.chunks.push(next);
        }
        for (chunk, left) in old {
            self.lit.set(chunk, false);
            if !left.edits.is_empty() {
                self.kept.insert(chunk, left.edits);
            }
        }
        self.window = chunks;
        Ok(loaded)
    }

    /// Every loaded chunk whose light is not up yet.
    pub(crate) fn unlit(&self) -> impl Iterator<Item = Point> + '_ {
        cells(self.window).filter(|c| !self.lit.contains(*c))
    }

    /// Notes that `chunk`'s light is up.
    pub(crate) fn mark_lit(&mut self, chunk: Point) {
        self.lit.insert(chunk);
    }

    /// The tiles of a loaded chunk, by world cell.
    pub(crate) fn chunk_tiles(&self, chunk: Point) -> impl Iterator<Item = (Point, TileId)> + '_ {
        let b = self.layout.chunk_bounds(chunk);
        self.locate(b.origin()).into_iter().flat_map(move |(slot, _)| self.chunks[slot].terrain.iter().map(move |(p, t)| (p + b.origin(), t)))
    }

    /// Every edit, of chunks in the window and out of it alike, by chunk.
    pub(crate) fn edits(&self) -> Vec<(Point, Vec<(u32, TileId)>)> {
        let mut all: BTreeMap<Point, Vec<(u32, TileId)>> = self.kept.iter().map(|(c, e)| (*c, e.iter().map(|(&i, &t)| (i, t)).collect())).collect();
        for (chunk, loaded) in cells(self.window).zip(&self.chunks) {
            if !loaded.edits.is_empty() {
                all.insert(chunk, loaded.edits.iter().map(|(&i, &t)| (i, t)).collect());
            }
        }
        all.into_iter().collect()
    }

    /// Every chunk ever built.
    pub(crate) fn made(&self) -> Vec<Point> {
        self.made.iter().collect()
    }
}

/// Every cell of `rect`, row by row.
fn cells(rect: GridRect) -> impl Iterator<Item = Point> {
    (rect.y..rect.bottom()).flat_map(move |y| (rect.x..rect.right()).map(move |x| Point::new(x, y)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use atglyph_grid::Light;

    /// Four chunks across and three down, each four tiles a side.
    fn layout() -> Streamed {
        Streamed { chunk: 4, chunks: (4, 3), reach: 1, entry: Point::new(1, 1), ambient: Light::DARK, peaceful: false }
    }

    /// Every tile of a chunk is the chunk's column, so where a tile came
    /// from can be read off it.
    fn build(chunk: Point) -> Result<Terrain, BuildError> {
        Ok(Terrain::filled(4, 4, TileId(chunk.x as u16)))
    }

    #[test]
    fn a_load_builds_what_is_new_and_says_which_chunks_are_first() {
        let mut surface = Surface::new(layout());
        let loaded = surface.load(GridRect::new(0, 0, 2, 2), build).unwrap();
        assert_eq!(loaded.len(), 4);
        assert!(loaded.iter().all(|l| l.first));
        assert_eq!(surface.get(Point::new(5, 1)), Some(TileId(1)), "the second chunk across");
        assert_eq!(surface.get(Point::new(9, 1)), None, "the third is not loaded");
        let moved = surface.load(GridRect::new(1, 0, 2, 2), build).unwrap();
        assert_eq!(moved.iter().map(|l| l.chunk).collect::<Vec<_>>(), [Point::new(2, 0), Point::new(2, 1)], "only the new column is built");
        assert_eq!(surface.get(Point::new(1, 1)), None, "the first column left");
    }

    #[test]
    fn an_edit_outlives_its_chunk_leaving_and_comes_back_with_it() {
        let mut surface = Surface::new(layout());
        surface.load(GridRect::new(0, 0, 2, 2), build).unwrap();
        assert_eq!(surface.set(Point::new(1, 1), TileId(9)), Some(TileId(0)));
        surface.load(GridRect::new(2, 0, 2, 2), build).unwrap();
        assert_eq!(surface.edits(), [(Point::new(0, 0), vec![(5, TileId(9))])], "kept while out of the window");
        let back = surface.load(GridRect::new(0, 0, 2, 2), build).unwrap();
        assert!(back.iter().all(|l| !l.first), "built before, so none is a first");
        assert_eq!(surface.get(Point::new(1, 1)), Some(TileId(9)), "laid back on the rebuilt chunk");
    }

    #[test]
    fn a_failed_build_leaves_the_window_where_it_was() {
        let mut surface = Surface::new(layout());
        surface.load(GridRect::new(0, 0, 2, 2), build).unwrap();
        let refused = surface.load(GridRect::new(1, 0, 2, 2), |_| Err(BuildError::new("test", "no")));
        assert!(refused.is_err());
        assert_eq!(surface.get(Point::new(1, 1)), Some(TileId(0)), "the first column is still loaded");
    }

    #[test]
    fn a_restored_surface_keeps_its_edits_and_what_was_built() {
        let mut surface = Surface::new(layout());
        surface.load(GridRect::new(0, 0, 2, 2), build).unwrap();
        surface.set(Point::new(6, 6), TileId(7));
        let mut back = Surface::restored(layout(), surface.edits(), surface.made());
        let loaded = back.load(GridRect::new(0, 0, 2, 2), build).unwrap();
        assert!(loaded.iter().all(|l| !l.first));
        assert_eq!(back.get(Point::new(6, 6)), Some(TileId(7)));
    }
}
