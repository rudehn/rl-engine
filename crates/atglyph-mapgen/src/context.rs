//! What a pass reads and writes.

use std::any::Any;

#[cfg(test)]
use atglyph_core::Grid2D;
use atglyph_grid::{Terrain, TileRegistry};
use rand::rngs::StdRng;

/// Typed values passes publish, collected by type after the chain runs.
#[derive(Default)]
pub struct Outputs {
    items: Vec<Box<dyn Any + Send>>,
}

impl std::fmt::Debug for Outputs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Outputs({} items)", self.items.len())
    }
}

impl Outputs {
    /// Adds a value.
    pub fn push<T: Any + Send>(&mut self, value: T) {
        self.items.push(Box::new(value));
    }

    /// Every value of type `T`, in emission order.
    pub fn iter<T: Any>(&self) -> impl Iterator<Item = &T> {
        self.items.iter().filter_map(|b| b.downcast_ref::<T>())
    }

    /// Every value of type `T`, mutably, in emission order: for a pass
    /// that changes what an earlier one published, as a lake takes the
    /// floor it covers out of its area, without moving it in the order.
    pub(crate) fn iter_mut<T: Any>(&mut self) -> impl Iterator<Item = &mut T> {
        self.items.iter_mut().filter_map(|b| b.downcast_mut::<T>())
    }

    /// Keeps the values of type `T` that `keep` says to and drops the
    /// rest, leaving every other value and the order as they were: for a
    /// pass that undoes what an earlier one published, as a lake that
    /// covers a room whole leaves no room there.
    pub(crate) fn retain<T: Any>(&mut self, mut keep: impl FnMut(&T) -> bool) {
        self.items.retain(|b| b.downcast_ref::<T>().is_none_or(&mut keep));
    }

    /// Removes and returns every value of type `T`, in emission order.
    #[cfg(test)]
    pub(crate) fn take<T: Any>(&mut self) -> Vec<T> {
        let mut taken = Vec::new();
        let mut kept = Vec::new();
        for item in self.items.drain(..) {
            match item.downcast::<T>() {
                Ok(v) => taken.push(*v),
                Err(other) => kept.push(other),
            }
        }
        self.items = kept;
        taken
    }

    /// The first value of type `T`, if any.
    pub fn first<T: Any>(&self) -> Option<&T> {
        self.iter::<T>().next()
    }

    /// Number of values of any type.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }
}

/// The engine's own context: a terrain, a registry, a stream, outputs, and
/// optional snapshots.
///
/// It holds no stream until one is installed, by a [`Chain`](crate::Chain)
/// before each pass or by [`set_rng`](Self::set_rng) for a pass
/// applied by hand, and a pass that draws before then panics. A placeholder
/// stream made from a constant would let a pass run outside a chain and
/// draw the same numbers for every run, quietly, which is the one thing a
/// seeded generator must never do; taking a stream in [`new`](Self::new)
/// instead would make every caller that hands the context to a chain, the
/// common case, invent a seed the chain then throws away.
#[derive(Debug)]
pub struct BaseContext {
    terrain: Terrain,
    tiles: TileRegistry,
    rng: Option<StdRng>,
    outputs: Outputs,
    snapshots: Option<Vec<Terrain>>,
}

impl BaseContext {
    /// A context over `terrain`, with no stream until a chain installs a
    /// pass's own.
    pub fn new(terrain: Terrain, tiles: TileRegistry) -> Self {
        Self { terrain, tiles, rng: None, outputs: Outputs::default(), snapshots: None }
    }

    /// A blank terrain of the given size filled with `fill`.
    pub fn blank(width: i32, height: i32, tiles: TileRegistry, fill: atglyph_grid::TileId) -> Self {
        Self::new(Terrain::filled(width, height, fill), tiles)
    }

    /// Turns on snapshot capture: a copy of the terrain after every pass.
    pub fn with_snapshots(mut self) -> Self {
        self.snapshots = Some(Vec::new());
        self
    }

    /// The snapshots captured, if capture was on.
    #[cfg(test)]
    pub(crate) fn snapshots(&self) -> &[Terrain] {
        self.snapshots.as_deref().unwrap_or(&[])
    }

    /// The terrain being built.
    pub fn terrain(&self) -> &Terrain {
        &self.terrain
    }

    /// The terrain being built, mutably.
    pub fn terrain_mut(&mut self) -> &mut Terrain {
        &mut self.terrain
    }

    /// The tile registry the terrain's ids refer to.
    pub fn tiles(&self) -> &TileRegistry {
        &self.tiles
    }

    /// This pass's own random stream. Replaced by the chain before each pass.
    ///
    /// # Panics
    /// When nothing has installed a stream yet.
    pub fn rng(&mut self) -> &mut StdRng {
        self.rng.as_mut().expect("a pass drew from its context before a stream was installed: run it in a Chain, or hand it one with set_rng from RunSeed::rng")
    }

    /// Installs the stream for the pass about to run.
    pub fn set_rng(&mut self, rng: StdRng) {
        self.rng = Some(rng);
    }

    /// Publishes a structured result (placed rooms, spawn points, the
    /// entrance) for whatever runs after the chain to collect.
    pub fn emit<T: Any + Send>(&mut self, value: T) {
        self.outputs.push(value);
    }

    /// Everything emitted so far.
    pub fn outputs(&self) -> &Outputs {
        &self.outputs
    }

    /// Everything emitted so far, for taking.
    pub fn outputs_mut(&mut self) -> &mut Outputs {
        &mut self.outputs
    }

    /// Copies the terrain into the snapshots, when capture is on: the chain
    /// calls it after every pass.
    pub(crate) fn take_snapshot(&mut self) {
        if let Some(s) = &mut self.snapshots {
            s.push(self.terrain.clone());
        }
    }

    /// Takes the finished terrain and outputs apart.
    pub fn finish(self) -> (Terrain, Outputs) {
        (self.terrain, self.outputs)
    }

    /// The terrain's width.
    #[cfg(test)]
    pub(crate) fn width(&self) -> i32 {
        self.terrain.width()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atglyph_core::{RunSeed, SeedDomain};
    use rand::Rng;

    fn blank() -> BaseContext {
        let tiles = TileRegistry::standard();
        let floor = tiles.expect("floor");
        BaseContext::blank(4, 4, tiles, floor)
    }

    #[test]
    #[should_panic(expected = "before a stream was installed")]
    fn a_pass_that_draws_before_any_stream_is_installed_is_stopped_rather_than_handed_a_constant_one() {
        let _: u32 = blank().rng().random();
    }

    #[test]
    fn a_context_draws_exactly_the_stream_it_was_handed() {
        let stream = || RunSeed(7).rng(SeedDomain::new(b"test"), 0);
        let mut ctx = blank();
        ctx.set_rng(stream());
        let drawn: [u32; 4] = std::array::from_fn(|_| ctx.rng().random());
        let mut expected = stream();
        assert_eq!(drawn, std::array::from_fn(|_| expected.random()));
    }

    #[test]
    fn retain_drops_only_the_values_of_its_type_it_is_told_to_and_keeps_every_order() {
        let mut out = Outputs::default();
        for (i, v) in [1u32, 2, 3, 4].into_iter().enumerate() {
            out.push(v);
            out.push(if i % 2 == 0 { "a" } else { "b" });
            out.push(i as i64);
        }
        out.retain::<u32>(|v| v % 2 == 0);
        assert_eq!(out.iter::<u32>().copied().collect::<Vec<_>>(), vec![2, 4]);
        assert_eq!(out.iter::<&str>().copied().collect::<Vec<_>>(), vec!["a", "b", "a", "b"], "other types are left alone");
        assert_eq!(out.iter::<i64>().copied().collect::<Vec<_>>(), vec![0, 1, 2, 3]);
        out.retain::<u32>(|_| false);
        assert_eq!(out.iter::<u32>().count(), 0);
        assert_eq!(out.iter::<i64>().count(), 4);
    }

    #[test]
    fn outputs_are_collected_by_type() {
        let mut out = Outputs::default();
        out.push(1u32);
        out.push("a");
        out.push(2u32);
        assert_eq!(out.iter::<u32>().copied().collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(out.first::<&str>(), Some(&"a"));
        for v in out.iter_mut::<u32>() {
            *v += 10;
        }
        assert_eq!(out.iter::<u32>().copied().collect::<Vec<_>>(), vec![11, 12], "changed in place, in order");
        assert_eq!(out.take::<u32>(), vec![11, 12]);
        assert_eq!(out.len(), 1);
        assert!(out.take::<u32>().is_empty());
    }
}
