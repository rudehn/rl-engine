//! What a pass reads and writes.

use std::any::Any;

use rand::SeedableRng;
use rand::rngs::StdRng;
use rl_core::Grid2D;
use rl_grid::{Terrain, TileRegistry};

/// The boundary between engine passes and a game's context.
///
/// Engine passes see only this trait. A game either uses [`BaseContext`]
/// directly or wraps it in a struct that adds its own state and implements
/// this trait by delegation.
pub trait BuildContext {
    /// The terrain being built.
    fn terrain(&self) -> &Terrain;

    /// The terrain being built, mutably.
    fn terrain_mut(&mut self) -> &mut Terrain;

    /// The tile registry the terrain's ids refer to.
    fn tiles(&self) -> &TileRegistry;

    /// This pass's own random stream. Replaced by the chain before each pass.
    fn rng(&mut self) -> &mut StdRng;

    /// Installs the stream for the pass about to run.
    fn set_rng(&mut self, rng: StdRng);

    /// Publishes a structured result (placed rooms, spawn points, the
    /// entrance) for whatever runs after the chain to collect.
    fn emit<T: Any + Send>(&mut self, value: T);

    /// Everything emitted so far.
    fn outputs(&self) -> &Outputs;

    /// Everything emitted so far, for taking.
    fn outputs_mut(&mut self) -> &mut Outputs;

    /// Hook for snapshot capture between passes. Default no-op.
    fn take_snapshot(&mut self) {}
}

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

    /// Removes and returns every value of type `T`, in emission order.
    pub fn take<T: Any>(&mut self) -> Vec<T> {
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
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether nothing has been emitted.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// The engine's own context: a terrain, a registry, a stream, outputs, and
/// optional snapshots.
#[derive(Debug)]
pub struct BaseContext {
    terrain: Terrain,
    tiles: TileRegistry,
    rng: StdRng,
    outputs: Outputs,
    snapshots: Option<Vec<Terrain>>,
}

impl BaseContext {
    /// A context over `terrain`. The stream is a placeholder until the chain
    /// installs a pass's own.
    pub fn new(terrain: Terrain, tiles: TileRegistry) -> Self {
        Self {
            terrain,
            tiles,
            rng: StdRng::seed_from_u64(0),
            outputs: Outputs::default(),
            snapshots: None,
        }
    }

    /// A blank terrain of the given size filled with `fill`.
    pub fn blank(width: i32, height: i32, tiles: TileRegistry, fill: rl_grid::TileId) -> Self {
        Self::new(Terrain::filled(width, height, fill), tiles)
    }

    /// Turns on snapshot capture: a copy of the terrain after every pass.
    pub fn with_snapshots(mut self) -> Self {
        self.snapshots = Some(Vec::new());
        self
    }

    /// The snapshots captured, if capture was on.
    pub fn snapshots(&self) -> &[Terrain] {
        self.snapshots.as_deref().unwrap_or(&[])
    }

    /// Takes the finished terrain and outputs apart.
    pub fn finish(self) -> (Terrain, Outputs) {
        (self.terrain, self.outputs)
    }

    /// The terrain's width.
    pub fn width(&self) -> i32 {
        self.terrain.width()
    }

    /// The terrain's height.
    pub fn height(&self) -> i32 {
        self.terrain.height()
    }
}

impl BuildContext for BaseContext {
    fn terrain(&self) -> &Terrain {
        &self.terrain
    }

    fn terrain_mut(&mut self) -> &mut Terrain {
        &mut self.terrain
    }

    fn tiles(&self) -> &TileRegistry {
        &self.tiles
    }

    fn rng(&mut self) -> &mut StdRng {
        &mut self.rng
    }

    fn set_rng(&mut self, rng: StdRng) {
        self.rng = rng;
    }

    fn emit<T: Any + Send>(&mut self, value: T) {
        self.outputs.push(value);
    }

    fn outputs(&self) -> &Outputs {
        &self.outputs
    }

    fn outputs_mut(&mut self) -> &mut Outputs {
        &mut self.outputs
    }

    fn take_snapshot(&mut self) {
        if let Some(s) = &mut self.snapshots {
            s.push(self.terrain.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outputs_are_collected_by_type() {
        let mut out = Outputs::default();
        out.push(1u32);
        out.push("a");
        out.push(2u32);
        assert_eq!(out.iter::<u32>().copied().collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(out.first::<&str>(), Some(&"a"));
        assert_eq!(out.take::<u32>(), vec![1, 2]);
        assert_eq!(out.len(), 1);
        assert!(out.take::<u32>().is_empty());
    }
}
