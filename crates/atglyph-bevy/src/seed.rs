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
//! A game's own draws are `seed.rng(DOMAIN, index)`, [`RunSeed::rng`]
//! through [`Seed`]'s deref, over a domain the game names once as a
//! constant. The engine's own domains are listed in [`ENGINE_DOMAINS`].
//!
//! Every engine stream is a [`RunRng`] over a [`StreamDomain`], so the
//! deriving is written once and a stream is its domain and nothing else.

use std::marker::PhantomData;

use atglyph_core::{PlaceSeed, RunSeed, SeedDomain};
use bevy::ecs::schedule::common_conditions::resource_exists_and_changed;
use bevy::prelude::*;
use rand::rngs::StdRng;

use crate::places::MapId;

use crate::app::EngineApp;
use crate::plugin::EngineSet;

/// The seed the whole run derives from.
///
/// A resource the game inserts before play begins: fixed for a replay or a
/// test, [`from_args`](Self::from_args) for a game run from a terminal, or
/// the one a save recorded when a run is continued.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Deref)]
pub struct Seed(pub RunSeed);

impl Seed {
    /// `--seed N` from the command line, or a fresh seed when there is none:
    /// [`LaunchArgs::from_env`]'s seed, for a game with no flags of its own.
    ///
    /// # Panics
    /// Panics naming the argument when `N` is not a whole number, because a
    /// typo that quietly started a random run is a replay that was never
    /// replayed.
    ///
    /// When [`REPLAY_VAR`](crate::replay::REPLAY_VAR) names a recording,
    /// the recording's seed, whatever the command line says: a replay is
    /// the run it was recorded from.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_args() -> Self {
        LaunchArgs::from_env().seed_or_fresh()
    }

    /// The seed of place `map` in this run: where every draw about that
    /// one place comes from, `seed.place(map).rng(DOMAIN, 0)`, so what a
    /// floor holds never depends on which floor was entered first.
    pub fn place(&self, map: MapId) -> PlaceSeed {
        PlaceSeed::of(self.0, map.0)
    }
}

/// What a game was launched with: the run's seed, if one was named, and
/// the rest of its arguments, its own flags.
///
/// The recording's when [`REPLAY_VAR`](crate::replay::REPLAY_VAR) names
/// one, the command line's otherwise, so a game's own flags replay with
/// the run they were recorded under, and a game reads them from here
/// rather than from the recording or the command line itself. The seed is
/// the engine's to read, since a replay must run on its recorded one, so
/// `--seed N` is taken out of [`rest`](Self::rest) and the game matches on
/// its own flags alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchArgs {
    /// The seed the run was launched on: the recording's when replaying,
    /// `--seed N` otherwise, and none when neither named one.
    pub seed: Option<RunSeed>,
    /// Every other argument, in the order given, the program's name left
    /// off.
    pub rest: Vec<String>,
}

impl LaunchArgs {
    /// What this process was launched with.
    ///
    /// # Panics
    /// Panics naming the argument when `--seed` is not followed by a whole
    /// number, as [`Seed::from_args`] does; and when the recording
    /// [`REPLAY_VAR`](crate::replay::REPLAY_VAR) names cannot be read,
    /// since a replay that quietly started a fresh run never happened.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_env() -> Self {
        let (args, recorded) = match crate::replay::replaying() {
            Some(recording) => (recording.args, Some(RunSeed(recording.seed))),
            None => (crate::replay::own_args(), None),
        };
        Self::parse(args, recorded).unwrap_or_else(|e| panic!("{e}"))
    }

    /// Reads `args`, the program's name left off, with `recorded` the
    /// seed of the recording being replayed, if one is: `--seed N` taken
    /// out, and the recording's seed winning over it. The pure half of
    /// [`from_env`](Self::from_env), so it is tested without a process.
    pub fn parse(args: Vec<String>, recorded: Option<RunSeed>) -> Result<Self, String> {
        let (mut seed, mut rest) = (None, Vec::with_capacity(args.len()));
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            if arg != "--seed" {
                rest.push(arg);
                continue;
            }
            let n = args.next().ok_or("--seed takes a whole number, and none followed it")?;
            seed = Some(n.parse().map(RunSeed).map_err(|_| format!("--seed takes a whole number, not {n:?}"))?);
        }
        Ok(Self { seed: recorded.or(seed), rest })
    }

    /// The seed to insert: the one launched on, or a fresh one when none
    /// was named.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn seed_or_fresh(&self) -> Seed {
        Seed(self.seed.unwrap_or_else(RunSeed::fresh))
    }
}

// The engine's domains, every one under `atglyph.`, which a game's
// `SeedDomain::new` refuses. Each is named here once, so two subsystems can
// never draw under one name by accident, which
// `every_engine_stream_derives_under_a_name_of_its_own` checks.
pub(crate) const COMBAT: SeedDomain = SeedDomain::engine(b"combat");
pub(crate) const MINDS: SeedDomain = SeedDomain::engine(b"minds");
pub(crate) const LOOT: SeedDomain = SeedDomain::engine(b"loot");
pub(crate) const LOOT_PLACE: SeedDomain = SeedDomain::engine(b"loot.place");
pub(crate) const LOOT_CONTAINER: SeedDomain = SeedDomain::engine(b"loot.container");
pub(crate) const PREFAB_CONTENT: SeedDomain = SeedDomain::engine(b"prefabs.content");
pub(crate) const PROPS: SeedDomain = SeedDomain::engine(b"props");
pub(crate) const STEALTH: SeedDomain = SeedDomain::engine(b"stealth");
pub(crate) const STUMBLE: SeedDomain = SeedDomain::engine(b"status.stumble");
pub(crate) const EFFECTS: SeedDomain = SeedDomain::engine(b"effects");
pub(crate) const REFLECT: SeedDomain = SeedDomain::engine(b"ability.reflect");
pub(crate) const FIRE: SeedDomain = SeedDomain::engine(b"fire");
pub(crate) const GAS_SPILL: SeedDomain = SeedDomain::engine(b"gas.spill");
pub(crate) const GAS_SWIRL: SeedDomain = SeedDomain::engine(b"gas.swirl");
pub(crate) const RESTART: SeedDomain = SeedDomain::engine(b"restart");

/// Every domain the engine draws under, by name: its streams, a place's
/// loot and a piece's contents, the salts fire and gas hash a turn from,
/// the next run's seed on the web, and a place's own seed.
///
/// So a game can check none of its own domains is one of these, as
/// Undermere's and Foundry's `seeds` modules do; [`SeedDomain::new`] already
/// refuses every name under `atglyph.`, and this list is what that leaves to
/// check.
pub const ENGINE_DOMAINS: &[(&str, SeedDomain)] = &[
    ("atglyph.combat", COMBAT),
    ("atglyph.minds", MINDS),
    ("atglyph.loot", LOOT),
    ("atglyph.loot.place", LOOT_PLACE),
    ("atglyph.loot.container", LOOT_CONTAINER),
    ("atglyph.prefabs.content", PREFAB_CONTENT),
    ("atglyph.props", PROPS),
    ("atglyph.stealth", STEALTH),
    ("atglyph.status.stumble", STUMBLE),
    ("atglyph.effects", EFFECTS),
    ("atglyph.ability.reflect", REFLECT),
    ("atglyph.fire", FIRE),
    ("atglyph.gas.spill", GAS_SPILL),
    ("atglyph.gas.swirl", GAS_SWIRL),
    ("atglyph.restart", RESTART),
    ("atglyph.place", SeedDomain::engine(b"place")),
];

/// A random stream a subsystem owns, derived from the run's [`Seed`].
pub trait Stream: Resource {
    /// The stream for `seed`, under the subsystem's own domain.
    fn for_run(seed: RunSeed) -> Self;
}

/// The domain a [`RunRng`] draws under: a marker type per stream, so each
/// stream is a resource of its own.
pub trait StreamDomain: Send + Sync + 'static {
    /// The domain the stream is derived under, index 0.
    const DOMAIN: SeedDomain;
}

/// A subsystem's stream: a generator derived from the run's seed under
/// `D`'s domain, kept from one draw to the next for the whole run.
///
/// Generic rather than one hand-written type per subsystem, so a stream is
/// its domain and nothing else, and `CombatRng` and the rest are aliases of
/// it.
#[derive(Resource)]
pub struct RunRng<D: StreamDomain>(pub StdRng, PhantomData<D>);

impl<D: StreamDomain> std::ops::Deref for RunRng<D> {
    type Target = StdRng;

    fn deref(&self) -> &StdRng {
        &self.0
    }
}

impl<D: StreamDomain> std::ops::DerefMut for RunRng<D> {
    fn deref_mut(&mut self) -> &mut StdRng {
        &mut self.0
    }
}

impl<D: StreamDomain> Stream for RunRng<D> {
    fn for_run(seed: RunSeed) -> Self {
        Self(seed.rng(D::DOMAIN, 0), PhantomData)
    }
}

/// Present once `S` is being derived, so a second plugin asking for the
/// same stream adds no second deriving system. Two subsystems may honestly
/// want the same stream: a trap and an ability land the same effects, and
/// those effects roll from one.
#[derive(Resource)]
struct Derived<S: Stream>(std::marker::PhantomData<S>);

/// The body of [`EngineApp::add_stream`](crate::app::EngineApp::add_stream).
pub(crate) fn add_stream<'a, S: Stream>(app: &'a mut App, plugin: &'static str) -> &'a mut App {
    derive_stream::<S>(app);
    app.needs::<Seed>(plugin, "`Seed(RunSeed(n))`, the run's seed every stream derives from, or `Seed::from_args()` for `--seed`")
}

/// Derives `S` from [`Seed`] whenever one is given, as
/// [`add_stream`](crate::app::EngineApp::add_stream) does, without saying the seed is
/// needed: for a subsystem that draws only for some content, which says
/// when it needs the seed itself.
pub(crate) fn derive_stream<S: Stream>(app: &mut App) {
    if app.world().contains_resource::<Derived<S>>() {
        return;
    }
    app.insert_resource(Derived::<S>(std::marker::PhantomData));
    // Not gated on play: a seed inserted before the app runs, or by a
    // startup system continuing a save, is in place before the first turn
    // is dealt.
    app.add_systems(Update, derive::<S>.run_if(resource_exists_and_changed::<Seed>).before(EngineSet::Stream));
}

/// Derives `S` from the current seed.
fn derive<S: Stream>(mut commands: Commands, seed: Res<Seed>) {
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
        let mut app = crate::testing::headless_app();
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
        const BEASTS: SeedDomain = SeedDomain::new(b"beasts");
        const LOOT: SeedDomain = SeedDomain::new(b"loot");
        let seed = Seed(RunSeed(7));
        let roll = |domain, index| seed.rng(domain, index).random::<u64>();
        assert_eq!(roll(BEASTS, 1), roll(BEASTS, 1));
        assert_ne!(roll(BEASTS, 1), roll(LOOT, 1));
        assert_ne!(roll(BEASTS, 1), roll(BEASTS, 2));
    }

    #[test]
    fn every_engine_stream_derives_under_a_name_of_its_own() {
        for (i, (name, domain)) in ENGINE_DOMAINS.iter().enumerate() {
            let bare = name.strip_prefix("atglyph.").unwrap_or_else(|| panic!("{name} is not under atglyph."));
            assert_eq!(SeedDomain::engine(bare.as_bytes()), *domain, "{name} is listed under another name than it draws under");
            for (other, theirs) in &ENGINE_DOMAINS[i + 1..] {
                assert_ne!(name, other, "{name} is listed twice");
                assert_ne!(domain, theirs, "{name} and {other} share a stream");
            }
        }
    }

    #[test]
    fn a_seed_s_place_is_the_place_seed_of_its_run_and_map() {
        assert_eq!(Seed(RunSeed(42)).place(MapId(3)), PlaceSeed::of(RunSeed(42), 3));
    }

    #[test]
    fn a_run_rng_draws_under_its_domain() {
        struct Beasts;
        impl StreamDomain for Beasts {
            const DOMAIN: SeedDomain = SeedDomain::new(b"beasts");
        }
        let mut rng = RunRng::<Beasts>::for_run(RunSeed(7));
        assert_eq!(rng.random::<u64>(), RunSeed(7).rng(Beasts::DOMAIN, 0).random::<u64>());
    }

    fn launched(line: &str, recorded: Option<u64>) -> Result<LaunchArgs, String> {
        LaunchArgs::parse(line.split_whitespace().map(String::from).collect(), recorded.map(RunSeed))
    }

    fn words(line: &str) -> Vec<String> {
        line.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn a_seed_on_the_command_line_is_taken_out_of_the_game_s_arguments() {
        assert_eq!(launched("--floor 3 --seed 42", None), Ok(LaunchArgs { seed: Some(RunSeed(42)), rest: words("--floor 3") }));
        assert_eq!(launched("--seed 42 --floor 3 --recipe caves", None), Ok(LaunchArgs { seed: Some(RunSeed(42)), rest: words("--floor 3 --recipe caves") }));
    }

    #[test]
    fn a_recording_s_seed_wins_over_the_command_line() {
        assert_eq!(launched("--seed 42 --deck 2", Some(7)), Ok(LaunchArgs { seed: Some(RunSeed(7)), rest: words("--deck 2") }));
        assert_eq!(launched("", Some(7)).map(|l| l.seed), Ok(Some(RunSeed(7))), "and stands with no seed named at all");
    }

    #[test]
    fn a_seed_that_is_not_a_number_is_refused_naming_it() {
        assert!(launched("--seed forty", None).is_err_and(|e| e.contains("\"forty\"")));
        assert!(launched("--floor 3 --seed", None).is_err_and(|e| e.contains("none followed")));
    }

    #[test]
    fn with_no_seed_the_arguments_are_left_as_they_were() {
        assert_eq!(launched("--prefabs", None), Ok(LaunchArgs { seed: None, rest: words("--prefabs") }));
        assert_eq!(launched("", None), Ok(LaunchArgs { seed: None, rest: Vec::new() }));
    }
}
