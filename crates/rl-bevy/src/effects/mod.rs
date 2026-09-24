//! Effects: what every carrier of an effect list shares.
//!
//! Three things in the engine land a list of effects and none of them is
//! the others: an ability an actor knows, a prop's offer, and a trigger,
//! which is what a prop or a thing does at a moment it answers. What they
//! share is here: [`Effects`], the list itself, built once from content and
//! landed with its chances rolled; [`Landing`], where a list lands and on
//! whom, with its [`Source`]; [`EffectWorld`], what an effect may change;
//! the [`Effect`] trait and the [`EffectKinds`] registry a content file's
//! names resolve against; and [`EffectRng`], the one stream every effect
//! rolls from, so a trap cannot shift a spell's dice.
//!
//! `engine` holds the effects the engine ships, and `triggers` the moments,
//! [`Triggers`] and the one system that lands them. [`EffectsPlugin`] is the
//! whole of it, added by whichever of abilities, props or consumables a game
//! adds first. A game's own effects sit beside the engine's through
//! [`AddEffect::add_effect`], and nothing that lands them can tell the
//! difference.

mod engine;
mod triggers;

pub use engine::*;
pub use triggers::*;

use std::collections::BTreeMap;

use bevy::prelude::*;
use rand::rngs::StdRng;
use rl_core::{Point, RunSeed, SeedDomain};
use rl_rules::Names;
use rl_rules::ability::AbilityId;
use rl_rules::ability::RawValue;

use crate::combat::DamageEvent;
use crate::components::{Blocks, Position, Viewshed};
use crate::cue::Cued;
use crate::registries::Registries;
use crate::status::{Afflict, Cure};
use crate::turn::Occupancy;
use crate::world::WorldMap;

/// The stream every effect rolls from, whatever lands it.
///
/// Its own domain, so adding an effect does not shift the combat stream
/// and change every monster's rolls in a run that was going fine, and one
/// stream for every carrier, so a trap cannot shift a spell's dice by
/// landing in a different pass. The domain is still `ability`, which is
/// what it was called before effects were a subsystem of their own: a new
/// name would have re-dealt every seed. Derived from the run's
/// [`Seed`](crate::seed::Seed) by [`EffectsPlugin`].
#[derive(Resource, Deref, DerefMut)]
pub struct EffectRng(pub StdRng);

impl crate::seed::Stream for EffectRng {
    fn for_run(seed: RunSeed) -> Self {
        Self(seed.rng(SeedDomain::new(b"ability"), 0))
    }
}

/// What landed an effect list.
///
/// So an effect can tell a spell from a trap without the landing type
/// assuming every list is an ability's, which is what `Option<AbilityId>`
/// used to say by leaving it empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// An ability resolved.
    Ability(AbilityId),
    /// A trigger went off on `on`, at `moment`.
    Trigger {
        /// What carried the trigger: a prop, a thing in a bag, a thrown
        /// thing where it came down, a weapon.
        on: Entity,
        /// What set it off.
        moment: MomentId,
    },
    /// A prop's offer was taken up.
    Offer(Entity),
}

impl Landing {
    /// The ability that landed this, when an ability did.
    pub fn ability(&self) -> Option<AbilityId> {
        match self.source {
            Source::Ability(id) => Some(id),
            _ => None,
        }
    }
}

/// One use, resolved: where it went and what was under it.
#[derive(Debug, Clone)]
pub struct Landing {
    /// Who used it. A prop that sprang a trap is as much a user as an
    /// actor that spent a turn.
    pub user: Entity,
    /// What landed it: an ability, a trigger, or a prop's offer. Only an
    /// ability draws an ability's look.
    pub source: Source,
    /// Where the user stood.
    pub origin: Point,
    /// Where it was pointed.
    pub aim: Point,
    /// Every cell it covers.
    pub cells: Vec<Point>,
    /// The cells a projectile flew through, landing included.
    pub path: Vec<Point>,
    /// Where a projectile stopped, if the shape had one.
    pub landed_at: Option<Point>,
    /// Everyone under the footprint the ability's
    /// [`Aim`](rl_rules::ability::Aim) wanted there.
    /// An effect that means to hit whoever is standing in the fire reads
    /// this; one that means to change the ground reads `cells`.
    pub targets: Vec<Entity>,
}

/// What an effect may do to the world.
///
/// Requests, not changes: an effect asks for damage, a status or a move,
/// and the systems that already own those answer. That is what keeps an
/// ability's damage going through the same mitigation a sword's does.
/// `commands` is the escape hatch, and it is a real one: an effect a game
/// writes can do anything a system can, including write its own messages.
#[derive(bevy::ecs::system::SystemParam)]
pub struct EffectWorld<'w, 's> {
    /// Anything the engine did not think of.
    pub commands: Commands<'w, 's>,
    /// Damage, through the pipeline that mitigates it.
    pub damage: MessageWriter<'w, crate::combat::DamageEvent>,
    /// A status on.
    pub afflict: MessageWriter<'w, Afflict>,
    /// A status off.
    pub cure: MessageWriter<'w, Cure>,
    /// The effect stream, for an effect that rolls.
    pub rng: ResMut<'w, EffectRng>,
    /// What is worth seeing, for whatever draws. The flight and the burst
    /// of the use itself are cued before any effect runs, so a cue an
    /// effect adds plays after them.
    pub cues: MessageWriter<'w, Cued>,
    actors: Query<'w, 's, (&'static mut Position, Option<&'static mut Viewshed>, Has<Blocks>)>,
    occupancy: ResMut<'w, Occupancy>,
    map: Res<'w, WorldMap>,
}

impl EffectWorld<'_, '_> {
    /// The map the effects land on.
    pub fn map(&self) -> &WorldMap {
        &self.map
    }

    /// Who stands where, for working out who is under a footprint.
    pub fn occupancy(&self) -> &Occupancy {
        &self.occupancy
    }

    /// Where `e` is, if it is anywhere.
    pub fn position(&self, e: Entity) -> Option<Point> {
        self.actors.get(e).ok().map(|(p, _, _)| p.0)
    }

    /// Whether `who` can see the cell `p`, or `None` when it carries no
    /// viewshed and the question cannot be asked of it.
    ///
    /// Most non-players carry none: the minds read the player's viewshed
    /// as the oracle, since lines of sight are symmetric. So a monster is
    /// not held to a sight requirement it has no way to answer, and the
    /// tactic that chose the aim is trusted instead.
    ///
    /// Asked here rather than through a query of its own because this one
    /// already holds every viewshed, and two queries over the same
    /// component, one of them writing, is a system Bevy refuses to run.
    pub fn sight_of(&self, who: Entity, p: Point) -> Option<bool> {
        let (_, viewshed, _) = self.actors.get(who).ok()?;
        Some(viewshed?.can_see(p))
    }

    /// Whether `p` is somewhere an actor could stand right now.
    pub fn is_free(&self, p: Point) -> bool {
        self.map.is_walkable(p) && !self.occupancy.is_occupied(p)
    }

    /// Puts `e` on `p`, keeping the occupancy index and its sight
    /// straight. False when the cell will not take it.
    pub fn place(&mut self, e: Entity, p: Point) -> bool {
        if !self.is_free(p) {
            return false;
        }
        let Ok((mut pos, viewshed, blocks)) = self.actors.get_mut(e) else { return false };
        let from = pos.0;
        if from == p {
            return true;
        }
        if blocks {
            self.occupancy.relocate(e, from, p);
        }
        pos.0 = p;
        if let Some(mut v) = viewshed {
            v.dirty = true;
        }
        true
    }

    /// Slides `e` up to `cells` steps along the line from `from` to `to`,
    /// stopping at the last cell it can stand on. Returns where it ended.
    ///
    /// A push and a pull are the same walk with the ends swapped, so both
    /// stop at a wall rather than through it.
    pub fn slide(&mut self, e: Entity, from: Point, to: Point, cells: i32) -> Point {
        let (dx, dy) = ((to.x - from.x).signum(), (to.y - from.y).signum());
        if dx == 0 && dy == 0 {
            return from;
        }
        let mut at = from;
        for _ in 0..cells.max(0) {
            let next = at.offset(dx, dy);
            if !self.is_free(next) {
                break;
            }
            at = next;
        }
        if at != from {
            self.place(e, at);
        }
        at
    }
}

/// One thing an ability does when it lands.
///
/// A type per effect, registered with [`AddEffect::add_effect`], parsing
/// its own arguments out of the RON the ability named it with. The engine
/// ships one per subsystem it owns; a game's own sit beside them and the
/// resolver cannot tell the difference.
pub trait Effect: Send + Sync + 'static {
    /// What this does to `landing`.
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>);

    /// What this does, in a few words for a menu, with every id named
    /// through `registries`: `3d6 fire`, `scorched for 4 turns`. Empty
    /// means the menu says nothing about it, which is the default so an
    /// effect a game writes in a hurry still loads.
    fn describe(&self, registries: &crate::registries::Registries) -> String {
        let _ = registries;
        String::new()
    }
}

/// An effect that knows how to build itself out of an ability's RON.
///
/// Separate from [`Effect`] so the trait a game writes stays object-safe
/// and the constructor can fail with a message naming what was wrong.
pub trait FromArgs: Effect + Sized {
    /// The name abilities call this effect by in RON.
    const KIND: &'static str;

    /// Builds one from the arguments, resolving any names through `names`.
    fn from_args(args: &RawValue, names: &Names<'_>) -> Result<Self, String>;
}

/// How an effect is built, once its name has been matched.
type Builder = fn(&RawValue, &Names<'_>) -> Result<Box<dyn Effect>, String>;

/// Every effect kind a game has registered.
///
/// Filled while the app is built and read once, when abilities load, so an
/// unknown effect name is a startup failure naming the ability rather than
/// a surprise the first time someone presses the key.
#[derive(Resource, Default)]
pub struct EffectKinds(BTreeMap<String, Builder>);

impl EffectKinds {
    /// Registers `E` under its own name.
    pub fn declare<E: FromArgs>(&mut self) {
        self.0.insert(E::KIND.to_string(), |args, names| E::from_args(args, names).map(|e| Box::new(e) as Box<dyn Effect>));
    }

    /// Whether `kind` is registered.
    pub fn has(&self, kind: &str) -> bool {
        self.0.contains_key(kind)
    }

    /// Every registered name, in order, for an error that lists them.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(|s| s.as_str())
    }

    /// Builds the effect registered as `kind` from its own arguments.
    ///
    /// For whatever holds effects as content and is not an ability: a
    /// prop's trigger, an offer it answers. The error names what was
    /// wrong, or lists what is registered when the kind is not.
    pub fn build(&self, kind: &str, args: &RawValue, names: &Names<'_>) -> Result<Box<dyn Effect>, String> {
        match self.0.get(kind) {
            Some(build) => build(args, names),
            None => {
                let known: Vec<&str> = self.names().collect();
                Err(format!("no effect is registered as {kind:?}; registered: {}", known.join(", ")))
            }
        }
    }
}

/// Registers an effect with the engine.
pub trait AddEffect {
    /// Registers `E` so abilities may name it in their RON.
    fn add_effect<E: FromArgs>(&mut self) -> &mut Self;
}

impl AddEffect for App {
    fn add_effect<E: FromArgs>(&mut self) -> &mut Self {
        self.init_resource::<EffectKinds>();
        self.world_mut().resource_mut::<EffectKinds>().declare::<E>();
        self
    }
}

/// One effect of a list, built, with the chance it lands.
struct Built {
    chance: u8,
    effect: Box<dyn Effect>,
}

/// A list of effects, built once from content and landed together.
///
/// Three things in the engine own such a list and none of them is the
/// others: an ability, a prop's trigger or one of its offers, and an item
/// that does something when it is used. Each decides for itself what the
/// list means and when it lands; what they share is reading the same
/// `(kind, chance, args)` out of content, failing loudly on a name nobody
/// registered, and rolling the ones that may miss. That sharing is here,
/// because it was written three times before it was written once, and the
/// prop's copy had already drifted from the ability's.
///
/// Not `Clone`, since an effect is a boxed trait object: a game that wants
/// one list on many entities puts it behind an `Arc`, which is what
/// [`Triggers`] does.
#[derive(Default)]
pub struct Effects(Vec<Built>);

impl Effects {
    /// Builds every spec, or reports every one that would not build.
    ///
    /// Every failure rather than the first, because content is fixed a
    /// file at a time and a loader that stops at the first error hides the
    /// other four. The caller says which thing the list belongs to when it
    /// prints them: this cannot know whether it is reading an ability, a
    /// trap or a medkit.
    pub fn build(specs: &[rl_rules::EffectSpec], kinds: &EffectKinds, names: &Names<'_>) -> Result<Self, Vec<String>> {
        let mut built = Vec::new();
        let mut errors = Vec::new();
        for spec in specs {
            match kinds.build(&spec.kind, &spec.args, names) {
                Ok(effect) => built.push(Built { chance: spec.chance, effect }),
                Err(e) => errors.push(e),
            }
        }
        if errors.is_empty() { Ok(Self(built)) } else { Err(errors) }
    }

    /// Whether it does nothing at all, which is what content that named no
    /// effects builds to.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Lands every effect on `landing`, rolling each that may miss.
    ///
    /// The roll comes from the ability stream, whoever is landing: a trap
    /// and a potion are dealt from the same deck as a spell, so a subsystem
    /// cannot shift another's dice by landing something of its own.
    pub fn land(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        use rand::Rng;
        for built in &self.0 {
            if built.chance < 100 && !world.rng.random_ratio(u32::from(built.chance), 100) {
                continue;
            }
            built.effect.apply(landing, world);
        }
    }

    /// What these do, one line per effect that has something to say, with
    /// its chance in front when it is not certain.
    ///
    /// What a menu lists under an ability, and what a screen could list
    /// under a prop's offer or a thing in the bag: the list can say what it
    /// is without anyone knowing what carries it.
    pub fn describe(&self, registries: &Registries) -> Vec<String> {
        self.0
            .iter()
            .filter_map(|b| {
                let what = b.effect.describe(registries);
                match (what.is_empty(), b.chance >= 100) {
                    (true, _) => None,
                    (false, true) => Some(what),
                    (false, false) => Some(format!("{}% chance of {what}", b.chance)),
                }
            })
            .collect()
    }

    /// Lands every effect on one cell, as `user` setting them off there.
    ///
    /// The shape for everything that happens where it already is rather
    /// than where it was aimed: a trap underfoot, a crate levered open, a
    /// stim in the arm. No ability, so nothing draws an ability's look, and
    /// one cell, so a footprint is not invented for something that never
    /// flew.
    pub fn land_on(&self, source: Source, user: Entity, at: Point, targets: Vec<Entity>, world: &mut EffectWorld<'_, '_>) {
        let landing = Landing { user, source, origin: at, aim: at, cells: vec![at], path: Vec::new(), landed_at: None, targets };
        self.land(&landing, world);
    }
}

/// Effects: what every carrier of an effect list shares.
///
/// The registry of effect kinds, the stream every effect rolls from, and
/// the messages an effect writes. Added by whatever lands effects,
/// abilities, props or consumables, when a game has not already added it,
/// so a game names the subsystems it wants and this comes with them.
pub struct EffectsPlugin;

/// Marks that [`EffectsPlugin`] has built, so a second copy builds nothing.
#[derive(Resource)]
struct EffectsBuilt;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        use crate::plugin::Reads;
        use crate::plugin::{ResolveSet, Turn};
        use crate::seed::AddStream;
        // Once, however many times it is added: every plugin that lands
        // effects adds it when it is missing, and a game that also adds it
        // by hand, after one of those, must not have its triggers landed
        // twice or its plugin list refused.
        if app.world().contains_resource::<EffectsBuilt>() {
            return;
        }
        app.insert_resource(EffectsBuilt);
        app.init_resource::<EffectKinds>()
            .init_resource::<Moments>()
            .add_message::<Fired>()
            .add_stream::<EffectRng>("EffectsPlugin")
            // `EffectWorld` writes these, so they are this plugin's to
            // register: a writer for a message nobody registered fails the
            // system at startup, and a game with effects should not have to
            // add the status or combat plugins to find that out. Registering
            // one twice is harmless.
            .add_message::<Afflict>()
            .add_message::<Cure>()
            .add_message::<DamageEvent>()
            .reads::<Cued>()
            .add_systems(Turn, (report_remnants, land_triggers).chain().in_set(ResolveSet::Triggers));
    }

    /// Not unique, so adding it after a plugin that already added it is
    /// not an error; the build above makes the second copy a no-op.
    fn is_unique(&self) -> bool {
        false
    }
}

/// Adds [`EffectsPlugin`] to `app` unless something already has: what every
/// plugin that lands effects calls first, so the order a game lists its
/// plugins in never matters and the subsystem is added exactly once.
pub(crate) fn ensure(app: &mut App) {
    if !app.is_plugin_added::<EffectsPlugin>() {
        app.add_plugins(EffectsPlugin);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What `AbilityRng` drew first for seed 7 on the day the stream moved
    /// here and was renamed.
    const FIRST_DRAW_FOR_SEED_SEVEN: u64 = 6_240_403_554_423_139_800;

    /// Fingerprint tripwire: the effect stream is the ability stream renamed,
    /// derived from the same domain, so every seed rolls the dice it rolled
    /// before the move.
    #[test]
    fn fingerprint_tripwire_the_effect_stream_draws_what_the_ability_stream_drew() {
        use crate::seed::Stream;
        use rand::RngCore;
        let mut rng = EffectRng::for_run(rl_core::RunSeed(7));
        assert_eq!(rng.next_u64(), FIRST_DRAW_FOR_SEED_SEVEN);
    }
}
