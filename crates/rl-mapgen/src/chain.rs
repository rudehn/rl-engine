//! The pass chain.

use std::fmt;

use rl_core::{RunSeed, SeedDomain};

use crate::context::BuildContext;

/// When a pass runs, relative to the others. The order of the variants is
/// the pipeline order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Phase {
    /// Laying down what the ground is made of.
    Ground,
    /// Growing or scattering what covers it.
    Growth,
    /// Everything built by hand: rooms, buildings, roads, walls.
    Structures,
    /// Making sure what was built can be walked between.
    Connect,
    /// Ways in and out.
    Exits,
    /// Decoration and anything that must see the finished shape.
    Finish,
}

/// Why a pass could not do its work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildError {
    /// The pass that failed.
    pub pass: &'static str,
    /// What went wrong.
    pub reason: String,
}

impl BuildError {
    /// An error from `pass`.
    pub fn new(pass: &'static str, reason: impl Into<String>) -> Self {
        Self {
            pass,
            reason: reason.into(),
        }
    }
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pass `{}` failed: {}", self.pass, self.reason)
    }
}

impl std::error::Error for BuildError {}

/// One step of generation.
pub trait Pass<C: BuildContext>: Send + Sync {
    /// A stable name. Keys this pass's random stream, so renaming a pass
    /// rerolls it and reordering the chain does not.
    fn name(&self) -> &'static str;

    /// When this pass runs.
    fn phase(&self) -> Phase;

    /// Does the work, or says why it could not.
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError>;
}

/// An ordered sequence of passes.
///
/// Assembly checks phase order and name uniqueness and panics on either,
/// because both are mistakes in how the chain was written rather than
/// conditions to recover from.
pub struct Chain<C: BuildContext> {
    passes: Vec<Box<dyn Pass<C>>>,
}

impl<C: BuildContext> Default for Chain<C> {
    fn default() -> Self {
        Self { passes: Vec::new() }
    }
}

impl<C: BuildContext> fmt::Debug for Chain<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.passes.iter().map(|p| p.name())).finish()
    }
}

impl<C: BuildContext> Chain<C> {
    /// An empty chain.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a pass.
    ///
    /// # Panics
    /// Panics if the pass's phase precedes the previous pass's, or if a
    /// pass with the same name is already in the chain. A name keys a
    /// stream, so two passes sharing one would draw an identical sequence
    /// and lay the same pattern twice.
    pub fn then(mut self, pass: impl Pass<C> + 'static) -> Self {
        if let Some(last) = self.passes.last() {
            assert!(
                pass.phase() >= last.phase(),
                "pass `{}` is {:?} but follows `{}`, which is {:?}",
                pass.name(),
                pass.phase(),
                last.name(),
                last.phase()
            );
        }
        assert!(
            !self.passes.iter().any(|p| p.name() == pass.name()),
            "two passes are both named `{}`; a name keys a random stream",
            pass.name()
        );
        self.passes.push(Box::new(pass));
        self
    }

    /// The names of the passes, in order.
    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.passes.iter().map(|p| p.name())
    }

    /// Number of passes.
    pub fn len(&self) -> usize {
        self.passes.len()
    }

    /// Whether the chain has no passes.
    pub fn is_empty(&self) -> bool {
        self.passes.is_empty()
    }

    /// Runs every pass over `ctx`.
    ///
    /// Each pass is handed a stream derived from `seed` and its own name, so
    /// adding, removing or reordering a pass cannot change what any other
    /// pass draws. Stops at the first failure and reports it.
    pub fn run(&self, ctx: &mut C, seed: RunSeed) -> Result<(), BuildError> {
        for pass in &self.passes {
            ctx.set_rng(seed.rng(SeedDomain::new(pass.name().as_bytes()), 0));
            pass.apply(ctx)?;
            ctx.take_snapshot();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::BaseContext;
    use rand::Rng;
    use rl_core::Point;
    use rl_grid::{TileId, TileRegistry};

    struct Marker {
        name: &'static str,
        phase: Phase,
        row: i32,
        tile: TileId,
    }

    impl Pass<BaseContext> for Marker {
        fn name(&self) -> &'static str {
            self.name
        }
        fn phase(&self) -> Phase {
            self.phase
        }
        fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
            for x in 0..ctx.width() {
                ctx.terrain_mut().set(Point::new(x, self.row), self.tile);
            }
            ctx.emit(self.row);
            Ok(())
        }
    }

    struct Roll(&'static str, i32);

    impl Pass<BaseContext> for Roll {
        fn name(&self) -> &'static str {
            self.0
        }
        fn phase(&self) -> Phase {
            Phase::Ground
        }
        fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
            let v = ctx.rng().random_range(0..1000u16);
            ctx.terrain_mut().set(Point::new(0, self.1), TileId(v));
            Ok(())
        }
    }

    struct Fails;

    impl Pass<BaseContext> for Fails {
        fn name(&self) -> &'static str {
            "fails"
        }
        fn phase(&self) -> Phase {
            Phase::Structures
        }
        fn apply(&self, _: &mut BaseContext) -> Result<(), BuildError> {
            Err(BuildError::new("fails", "no room"))
        }
    }

    fn blank() -> BaseContext {
        let tiles = TileRegistry::standard();
        let floor = tiles.expect("floor");
        BaseContext::blank(8, 6, tiles, floor)
    }

    #[test]
    fn passes_run_in_order_and_publish_outputs() {
        let chain = Chain::new()
            .then(Marker { name: "a", phase: Phase::Ground, row: 0, tile: TileId(1) })
            .then(Marker { name: "b", phase: Phase::Growth, row: 0, tile: TileId(2) })
            .then(Marker { name: "c", phase: Phase::Finish, row: 1, tile: TileId(3) });
        let mut ctx = blank().with_snapshots();
        chain.run(&mut ctx, RunSeed(1)).unwrap();
        assert_eq!(ctx.terrain().get(Point::new(3, 0)), Some(TileId(2)), "later pass wins");
        assert_eq!(ctx.terrain().get(Point::new(3, 1)), Some(TileId(3)));
        assert_eq!(ctx.outputs().iter::<i32>().copied().collect::<Vec<_>>(), vec![0, 0, 1]);
        assert_eq!(ctx.snapshots().len(), 3);
        assert_eq!(ctx.snapshots()[0].get(Point::new(3, 0)), Some(TileId(1)));
    }

    #[test]
    #[should_panic(expected = "follows")]
    fn a_chain_refuses_to_go_backwards() {
        let _ = Chain::new()
            .then(Marker { name: "late", phase: Phase::Exits, row: 0, tile: TileId(1) })
            .then(Marker { name: "early", phase: Phase::Ground, row: 0, tile: TileId(1) });
    }

    #[test]
    #[should_panic(expected = "both named")]
    fn a_chain_refuses_duplicate_names() {
        let _ = Chain::new().then(Roll("same", 0)).then(Roll("same", 1));
    }

    #[test]
    fn adding_a_pass_does_not_disturb_the_ones_before_it() {
        let seed = RunSeed(42);
        let mut alone = blank();
        Chain::new().then(Roll("x", 0)).run(&mut alone, seed).unwrap();
        let mut with_more = blank();
        Chain::new().then(Roll("y", 1)).then(Roll("x", 0)).then(Roll("z", 2)).run(&mut with_more, seed).unwrap();
        assert_eq!(alone.terrain().get(Point::new(0, 0)), with_more.terrain().get(Point::new(0, 0)));
    }

    #[test]
    fn a_different_seed_or_name_rolls_differently() {
        let mut a = blank();
        Chain::new().then(Roll("x", 0)).run(&mut a, RunSeed(1)).unwrap();
        let mut b = blank();
        Chain::new().then(Roll("x", 0)).run(&mut b, RunSeed(2)).unwrap();
        let mut c = blank();
        Chain::new().then(Roll("w", 0)).run(&mut c, RunSeed(1)).unwrap();
        let at = Point::new(0, 0);
        assert_ne!(a.terrain().get(at), b.terrain().get(at));
        assert_ne!(a.terrain().get(at), c.terrain().get(at));
    }

    #[test]
    fn a_failing_pass_stops_the_chain_and_names_itself() {
        let chain = Chain::new()
            .then(Marker { name: "a", phase: Phase::Ground, row: 0, tile: TileId(1) })
            .then(Fails)
            .then(Marker { name: "c", phase: Phase::Finish, row: 1, tile: TileId(3) });
        let mut ctx = blank();
        let err = chain.run(&mut ctx, RunSeed(1)).unwrap_err();
        assert_eq!(err.pass, "fails");
        assert_eq!(ctx.terrain().get(Point::new(0, 1)), TileRegistry::standard().id("floor"), "c never ran");
        assert_eq!(chain.names().collect::<Vec<_>>(), vec!["a", "fails", "c"]);
    }
}
