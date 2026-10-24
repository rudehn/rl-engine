//! What a game and a plugin call on the [`App`] while it is put together,
//! in one trait.
//!
//! One trait for the crate rather than one per subsystem, so a game that
//! imports a narrow prelude, or none, still finds every method: a method
//! of a trait that is not in scope is a method the compiler says does not
//! exist, and twenty traits were twenty imports to remember. The bodies
//! live beside what they register, in each subsystem's module; this is
//! only the surface.

use atglyph_rules::Choice;
use bevy::prelude::*;

use crate::effects::{FromArgs, TriggerKindId};
use crate::places::PlaceRules;
use crate::seed::Stream;
use crate::settings::{Setting, SettingId};
use crate::turn::Action;

/// The engine's methods on [`App`], for a game and for the plugins that
/// make it up.
///
/// Each says what it registers and why, and all of them may be called
/// twice: a second registration of the same thing adds nothing.
pub trait EngineApp {
    /// Builds every place of the game by `rules`, and the run's
    /// [`WorldMap`](crate::world::WorldMap) over their
    /// [`tiles`](PlaceRules::tiles) when a start makes none.
    ///
    /// Once, where the game is put together, rather than in every run's
    /// start: the rules are handed each place's seed, so nothing in them
    /// changes from run to run. Says the game needs a
    /// [`Seed`](crate::seed::Seed), since a place is built from one, so a
    /// game that forgot it is told when play begins rather than at its
    /// first warp.
    fn places(&mut self, rules: impl PlaceRules + 'static) -> &mut Self;

    /// Registers `A` as an action: its intents become a message, and an
    /// intent nobody resolves is refused rather than left to hang. Once:
    /// two plugins that both register the same action, as combat and the
    /// minds both do for a blow, get one sweeper between them.
    fn add_action<A: Action>(&mut self) -> &mut Self;

    /// Lets a mind choose an action of the game's own: registers `A` as an
    /// action, with its sweeper, and routes every
    /// [`MindChose`](crate::minds::MindChose) carrying an `A` to an
    /// `Intent<A>` in [`DecideSet::Game`](crate::plugin::DecideSet::Game).
    ///
    /// A game writes the action type, the resolver and the tactic, the
    /// same three things tutorial chapter 9 writes for a player's action,
    /// and this line. A choice nobody routed is a choice nobody resolves,
    /// which the sweeper refuses rather than losing.
    fn add_choice<A: Action + Choice + Clone>(&mut self) -> &mut Self;

    /// Makes `moment` spend a charge from what it happens to.
    fn spends_on(&mut self, moment: TriggerKindId) -> &mut Self;

    /// Declares the kind of work called `name`, the word a panel shows
    /// while an actor does it. Look its id up with
    /// [`WorkKinds::get`](crate::work::WorkKinds::get).
    fn add_work(&mut self, name: &str) -> &mut Self;

    /// Derives the stream `S` from [`Seed`](crate::seed::Seed) before the
    /// first frame that needs it, and again whenever the seed changes.
    /// `plugin` names who asked, for the report when no seed was given.
    fn add_stream<S: Stream>(&mut self, plugin: &'static str) -> &mut Self;

    /// Registers the effects the engine ships, `Harm`, `Mend`, `Inflict`,
    /// `Cleanse`, `Shove`, `Pull` and `Teleport`, together.
    ///
    /// A convenience, not a requirement: a game that wants three of them
    /// registers three, and one that wants none registers none. Nothing is
    /// registered by default, because an ability file naming an effect the
    /// game did not ask for should fail at load rather than work by accident.
    fn add_engine_effects(&mut self) -> &mut Self;

    /// Registers the effect `E` so abilities may name it in their RON.
    fn add_effect<E: FromArgs>(&mut self) -> &mut Self;

    /// Declares a setting and returns its id. See
    /// `Settings::add`.
    fn add_setting(&mut self, setting: Setting) -> SettingId;

    /// Declares the verb `name`. Look its id up again with
    /// [`Verbs::get`](crate::props::Verbs::get).
    ///
    /// A prop's definition names its verbs in words; this is what turns one
    /// into the id a game filters [`Interacted`](crate::props::Interacted) by.
    fn add_verb(&mut self, name: &str) -> &mut Self;

    /// Declares the trigger kind `name`. Look its id up again with
    /// [`TriggerKinds::get`](crate::effects::TriggerKinds::get).
    fn add_trigger_kind(&mut self, name: &str) -> &mut Self;

    /// Says that a remnant takes `C` from what it stands in for, as it
    /// takes the [`Name`].
    fn keep_on_remnant<C: Component + Clone>(&mut self) -> &mut Self;

    /// Declares the sound `name`. Look its id up again with
    /// [`Sounds::get`](crate::noise::Sounds::get) where it is made.
    fn add_sound(&mut self, name: &str) -> &mut Self;

    /// Puts `R` back to its default when a run ends, if it is there.
    ///
    /// The counterpart of [`needs`](Self::needs) for what a subsystem keeps
    /// rather than what it requires: a `Fire` field, an airborne queue,
    /// what a mind was offered. A resource that is not there when the run
    /// ends is left alone, so this is safe to declare for something the
    /// game inserts.
    ///
    /// What [`CorePlugin`](crate::plugin::CorePlugin) owns itself, the
    /// clock, the occupancy index and what has acted, is put back by
    /// [`clear_run`](crate::plugin::clear_run) directly, since it is there
    /// in every game and its absence would be a broken world rather than a
    /// plugin nobody added.
    fn reset_on_new_run<R: Resource + Default>(&mut self) -> &mut Self;

    /// Puts `R` back to what `make` returns when a run ends, as
    /// [`reset_on_new_run`](Self::reset_on_new_run) does, for a resource
    /// whose empty state is not its `Default`, or which has none.
    fn reset_on_new_run_with<R: Resource>(&mut self, make: fn() -> R) -> &mut Self;

    /// Says that this plugin reads the message `M` another plugin writes,
    /// and that an empty queue is a perfectly good answer: registers `M`
    /// unless it is already registered.
    ///
    /// Bevy refuses a reader for a message nobody registered, so a system that
    /// reads what an optional plugin writes panics in every game that left
    /// that plugin out: a vitals panel that reads what the player heard breaks
    /// a game with no noise, a prop that bursts breaks one with no combat.
    /// Registering it here says the reading is optional and means it: with the
    /// writer's plugin added the queue fills, without it the queue stays
    /// empty, and the reader asks either way.
    ///
    /// The opposite of [`needs`](Self::needs). `needs` is for what a plugin
    /// cannot work without, and fails loudly when play begins; this is for
    /// what it can work without, and says so once, where the reading is
    /// written.
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use atglyph_bevy::app::EngineApp;
    /// # #[derive(Message)]
    /// # struct Heard;
    /// # let mut app = App::new();
    /// // A panel that shows what was heard, in a game that may not have noise.
    /// app.reads::<Heard>();
    /// ```
    fn reads<M: Message>(&mut self) -> &mut Self;

    /// Records that `plugin` needs `R` inserted before play begins, with
    /// `hint` saying how a game makes one.
    ///
    /// Everything the added plugins need is checked together when play
    /// begins, and a game missing any of it is told every piece at once.
    fn needs<R: Resource>(&mut self, plugin: &'static str, hint: &'static str) -> &mut Self;

    /// Asserts, while the app is built, that the plugin `P` was added
    /// before `plugin`, which cannot be built without it.
    ///
    /// Unlike [`needs`](Self::needs), which waits for play to begin, since
    /// a resource may be inserted at any point before then, a plugin is
    /// added before the one that depends on it or not at all, so this
    /// fails at once.
    fn depends_on<P: Plugin>(&self, plugin: &'static str);
}

impl EngineApp for App {
    fn places(&mut self, rules: impl PlaceRules + 'static) -> &mut Self {
        crate::places::places(self, rules)
    }

    fn add_action<A: Action>(&mut self) -> &mut Self {
        crate::turn::add_action::<A>(self)
    }

    fn add_choice<A: Action + Choice + Clone>(&mut self) -> &mut Self {
        crate::minds::add_choice::<A>(self)
    }

    fn spends_on(&mut self, moment: TriggerKindId) -> &mut Self {
        crate::consumable::spends_on(self, moment)
    }

    fn add_work(&mut self, name: &str) -> &mut Self {
        crate::work::add_work(self, name)
    }

    fn add_stream<S: Stream>(&mut self, plugin: &'static str) -> &mut Self {
        crate::seed::add_stream::<S>(self, plugin)
    }

    fn add_engine_effects(&mut self) -> &mut Self {
        crate::effects::add_engine_effects(self)
    }

    fn add_effect<E: FromArgs>(&mut self) -> &mut Self {
        crate::effects::add_effect::<E>(self)
    }

    fn add_setting(&mut self, setting: Setting) -> SettingId {
        crate::settings::add_setting(self, setting)
    }

    fn add_verb(&mut self, name: &str) -> &mut Self {
        crate::props::add_verb(self, name)
    }

    fn add_trigger_kind(&mut self, name: &str) -> &mut Self {
        crate::effects::add_trigger_kind(self, name)
    }

    fn keep_on_remnant<C: Component + Clone>(&mut self) -> &mut Self {
        crate::effects::keep_on_remnant::<C>(self)
    }

    fn add_sound(&mut self, name: &str) -> &mut Self {
        crate::noise::add_sound(self, name)
    }

    fn reset_on_new_run<R: Resource + Default>(&mut self) -> &mut Self {
        crate::plugin::reset_on_new_run_with::<R>(self, R::default)
    }

    fn reset_on_new_run_with<R: Resource>(&mut self, make: fn() -> R) -> &mut Self {
        crate::plugin::reset_on_new_run_with::<R>(self, make)
    }

    fn reads<M: Message>(&mut self) -> &mut Self {
        // `add_message` is itself idempotent; this is about saying why, in
        // one place, rather than five plugins each explaining themselves.
        self.add_message::<M>()
    }

    fn needs<R: Resource>(&mut self, plugin: &'static str, hint: &'static str) -> &mut Self {
        crate::plugin::needs::<R>(self, plugin, hint)
    }

    fn depends_on<P: Plugin>(&self, plugin: &'static str) {
        assert!(self.is_plugin_added::<P>(), "{plugin} needs {} added as well", std::any::type_name::<P>());
    }
}
