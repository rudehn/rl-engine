//! The run's seed, and the random streams the engine rolls from.
//!
//! A game supplies one [`Seed`] and nothing else. Every subsystem that rolls
//! owns a [`Stream`], a generator under a domain of its own, and derives it
//! from the seed by itself: a game never inserts a combat stream or an
//! ability stream, adding a subsystem never adds a line to a game, and one
//! subsystem's draws never shift another's.
//!
//! Streams are derived again whenever the seed changes, so a game that
//! learns its seed late, from a save it is continuing, sets [`Seed`] and the
//! streams follow.
//!
//! A game's own draws come from [`Seed::stream`], named and indexed, which
//! is [`RunSeed::rng`] with the seed already in hand.

use bevy::ecs::schedule::common_conditions::resource_exists_and_changed;
use bevy::prelude::*;
use rand::rngs::StdRng;
use rl_core::{RunSeed, SeedDomain};

use crate::plugin::{EngineSet, Needs};

/// The seed the whole run derives from.
///
/// A resource the game inserts before play begins: fixed for a replay or a
/// test, [`from_args`](Self::from_args) for a game run from a terminal, or
/// the one a save recorded when a run is continued.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Deref)]
pub struct Seed(pub RunSeed);

impl Seed {
    /// `--seed N` from the command line, or a fresh seed when there is none.
    ///
    /// # Panics
    /// Panics naming the argument when `N` is not a whole number, because a
    /// typo that quietly started a random run is a replay that was never
    /// replayed.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_args() -> Self {
        match seed_argument(std::env::args().skip(1)) {
            Some(Ok(seed)) => Seed(seed),
            Some(Err(e)) => panic!("{e}"),
            None => Seed(RunSeed::fresh()),
        }
    }

    /// A generator for one named domain at one index: a floor's monsters, a
    /// region's loot.
    ///
    /// Named rather than shared, so a spawner added later draws from its own
    /// stream and cannot change what an existing one places.
    pub fn stream(&self, domain: &[u8], index: u64) -> StdRng {
        self.0.rng(SeedDomain::new(domain), index)
    }
}

/// The seed in `args`, if one was asked for.
#[cfg(not(target_arch = "wasm32"))]
fn seed_argument(mut args: impl Iterator<Item = String>) -> Option<Result<RunSeed, String>> {
    args.by_ref().find(|a| a == "--seed")?;
    Some(match args.next() {
        Some(n) => n.parse().map(RunSeed).map_err(|_| format!("--seed takes a whole number, not {n:?}")),
        None => Err("--seed takes a whole number, and none followed it".to_string()),
    })
}

/// A random stream a subsystem owns, derived from the run's [`Seed`].
pub trait Stream: Resource {
    /// The stream for `seed`, under the subsystem's own domain.
    fn for_run(seed: RunSeed) -> Self;
}

/// Registers a stream to be derived from the run's seed.
pub trait AddStream {
    /// Derives `S` from [`Seed`] before the first frame that needs it, and
    /// again whenever the seed changes. `plugin` names who asked, for the
    /// report when no seed was given.
    fn add_stream<S: Stream>(&mut self, plugin: &'static str) -> &mut Self;
}

impl AddStream for App {
    fn add_stream<S: Stream>(&mut self, plugin: &'static str) -> &mut Self {
        self.needs::<Seed>(plugin, "`Seed(RunSeed(n))`, the run's seed every stream derives from, or `Seed::from_args()` for `--seed`")
            // Not gated on play: a seed inserted before the app runs, or by a
            // startup system continuing a save, is in place before the first
            // turn is dealt.
            .add_systems(Update, derive_stream::<S>.run_if(resource_exists_and_changed::<Seed>).before(EngineSet::Stream))
    }
}

/// Derives `S` from the current seed.
fn derive_stream<S: Stream>(mut commands: Commands, seed: Res<Seed>) {
    commands.insert_resource(S::for_run(seed.0));
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    #[derive(Resource)]
    struct Draws {
        from: RunSeed,
        drawn: bool,
    }

    impl Stream for Draws {
        fn for_run(seed: RunSeed) -> Self {
            Self { from: seed, drawn: false }
        }
    }

    fn app() -> App {
        let mut app = crate::plugin::headless_app();
        app.add_stream::<Draws>("a test");
        app
    }

    #[test]
    fn a_stream_is_derived_from_the_seed_and_again_only_when_it_changes() {
        let mut app = app();
        app.insert_resource(Seed(RunSeed(1)));
        app.update();
        assert_eq!(app.world().resource::<Draws>().from, RunSeed(1));

        app.world_mut().resource_mut::<Draws>().drawn = true;
        app.update();
        assert!(app.world().resource::<Draws>().drawn, "an unchanged seed leaves the stream where it had got to");

        app.insert_resource(Seed(RunSeed(2)));
        app.update();
        let draws = app.world().resource::<Draws>();
        assert_eq!(draws.from, RunSeed(2), "a continued run's seed reseeds it");
        assert!(!draws.drawn, "from the start");
    }

    #[test]
    fn a_named_stream_repeats_for_its_seed_and_differs_by_name_and_index() {
        let seed = Seed(RunSeed(7));
        let roll = |domain: &[u8], index| seed.stream(domain, index).random::<u64>();
        assert_eq!(roll(b"beasts", 1), roll(b"beasts", 1));
        assert_ne!(roll(b"beasts", 1), roll(b"loot", 1));
        assert_ne!(roll(b"beasts", 1), roll(b"beasts", 2));
    }

    #[test]
    fn the_seed_argument_is_read_or_refused_by_name() {
        let args = |s: &str| s.split_whitespace().map(String::from).collect::<Vec<_>>().into_iter();
        assert_eq!(seed_argument(args("--floor 3 --seed 42")), Some(Ok(RunSeed(42))));
        assert_eq!(seed_argument(args("--floor 3")), None, "no seed asked for");
        assert!(seed_argument(args("--seed forty")).is_some_and(|r| r.is_err_and(|e| e.contains("\"forty\""))));
        assert!(seed_argument(args("--seed")).is_some_and(|r| r.is_err()));
    }
}
