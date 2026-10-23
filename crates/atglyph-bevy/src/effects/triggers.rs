//! Triggers: what a prop, a thing or an actor does at the moments it
//! answers.
//!
//! A subsystem that owns a moment, the items resolver for a use, throwing
//! for a landing, combat for a shot made, a shot that struck and the one it
//! struck, props for a cell stepped on and a prop broken, and this module
//! for an actor's death, writes [`Fired`] and nothing else.
//! `land_triggers` is the one system that answers: it finds the carrier's
//! [`Triggers`] for that moment, works out the cells its [`EffectArea`] covers
//! around where the moment happened, and lands the effects on everyone
//! standing there. A use aimed somewhere is landed as an ability is, on
//! its footprint from its user to the cell it was pointed at, and with a
//! look is seen to fly there before it lands. What a use costs the thing
//! is the consumables' business, read off the same messages.
//!
//! TriggerKinds are interned in [`TriggerKinds`] rather than enumerated, so a game
//! adds its own, an overheating gun or a charged console, with
//! [`AddTriggerKind::add_trigger_kind`] and reports it from a system of its own.

use std::sync::Arc;

use atglyph_core::{Interner, Point};
use atglyph_rules::{Asks, EffectArea, EffectSpec, Names, Targeting, TriggerSpec};
use bevy::prelude::*;

use super::{EffectKinds, EffectWorld, Effects, Landing, Source};
use crate::ability::{Aimed, Bystanders, LandedOn};
use crate::cue::{Airborne, CueWait, Cued, LookOf};
use crate::turn::Turns;
use crate::world::WorldMap;

/// What sets a trigger off: a thing used, a throw come to rest, an attack
/// made or landed, a cell stepped on, a prop broken, a worn thing's clock,
/// an actor struck or dead, or one a game names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriggerKind;

/// A registered moment.
pub type TriggerKindId = atglyph_core::Id<TriggerKind>;

/// Every moment in play, in the order they were first named.
///
/// Interned rather than a closed enum so a game adds its own with one line
/// and reports it from a system of its own. The engine's are interned
/// first, so their ids are the constants on this type.
#[derive(Resource, Debug, Clone)]
pub struct TriggerKinds(Interner<TriggerKind>);

impl TriggerKinds {
    /// A thing in the bag was used.
    pub const USE: TriggerKindId = TriggerKindId::from_raw(0);
    /// A thrown thing came to rest.
    pub const LAND: TriggerKindId = TriggerKindId::from_raw(1);
    /// An attack was made with a worn thing.
    pub const FIRE: TriggerKindId = TriggerKindId::from_raw(2);
    /// An attack struck someone: reported on the worn thing it was made
    /// with, and on the actor who made it.
    pub const HIT: TriggerKindId = TriggerKindId::from_raw(3);
    /// Somebody stepped onto a prop's cell.
    pub const ENTERED: TriggerKindId = TriggerKindId::from_raw(4);
    /// A prop was broken.
    pub(crate) const DESTROYED: TriggerKindId = TriggerKindId::from_raw(5);
    /// A worn thing's own clock came round: see
    /// [`Pulse`](crate::items::Pulse).
    pub const PULSE: TriggerKindId = TriggerKindId::from_raw(6);
    /// An attack landed on an actor: reported on the one it landed on, as
    /// [`HIT`](Self::HIT) is on what struck it.
    pub(crate) const STRUCK: TriggerKindId = TriggerKindId::from_raw(7);
    /// An actor died: reported a pass later, from a [`Remnant`], since the
    /// actor itself is out of the world by then.
    pub(crate) const DIED: TriggerKindId = TriggerKindId::from_raw(8);

    /// The engine's moments, in the order their ids are handed out.
    pub(crate) const BUILT_IN: [&'static str; 9] = ["use", "land", "fire", "hit", "entered", "destroyed", "pulse", "struck", "died"];

    /// The id for `name`, assigning a new one if it is unseen.
    pub(crate) fn declare(&mut self, name: &str) -> TriggerKindId {
        self.0.intern(name)
    }

    /// The id for `name`, if it has been declared.
    pub fn get(&self, name: &str) -> Option<TriggerKindId> {
        self.0.get(name)
    }

    /// The name behind `id`.
    pub fn name(&self, id: TriggerKindId) -> &str {
        self.0.name(id)
    }
}

impl Default for TriggerKinds {
    fn default() -> Self {
        let mut names = Interner::new();
        for name in Self::BUILT_IN {
            names.intern(name);
        }
        Self(names)
    }
}

/// Declares a kind of trigger while the app is being built.
pub trait AddTriggerKind {
    /// Declares the trigger kind `name`. Look its id up again with [`TriggerKinds::get`].
    fn add_trigger_kind(&mut self, name: &str) -> &mut Self;
}

impl AddTriggerKind for App {
    fn add_trigger_kind(&mut self, name: &str) -> &mut Self {
        self.init_resource::<TriggerKinds>();
        self.world_mut().resource_mut::<TriggerKinds>().declare(name);
        self
    }
}

/// One thing a carrier does at one moment.
///
/// The list behind an `Arc`, built once per definition and shared by every
/// copy, and the fire count beside it, which is each carrier's own.
#[derive(Clone)]
pub struct Trigger {
    /// What sets it off.
    pub on: TriggerKindId,
    /// Where it lands, around where the moment happened.
    pub area: EffectArea,
    /// Times it may still go off; `None` for every time.
    pub fires: Option<u32>,
    /// What it lands.
    pub effects: Arc<Effects>,
    /// What shows over the cells it lands on, if anything does.
    pub look: Option<atglyph_rules::ability::ParticleLook>,
    /// Hundredths of a step between pulses, `Some` on a `pulse` trigger and
    /// nowhere else. A game reads its carrier's base period back off it
    /// rather than keeping a second copy.
    pub every: Option<u32>,
    /// How a `use` is pointed, `Some` on the one `use` trigger of a thing
    /// used at somewhere and nowhere else: it lands on the footprint where
    /// it was pointed rather than around where its moment happened.
    pub targeting: Option<Targeting>,
    /// What a `use` asks its user for, `Some` on the one `use` trigger of a
    /// thing used on another of its user's things and nowhere else: it
    /// lands where its user stands, with the thing chosen as the landing's
    /// [`on`](Landing::on).
    pub asks: Option<Asks>,
}

/// Written by hand, since the effects themselves are boxed trait objects:
/// what a trigger answers, where, and how often, which is what a log or a
/// failed assertion needs.
impl std::fmt::Debug for Trigger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Trigger")
            .field("on", &self.on)
            .field("area", &self.area)
            .field("fires", &self.fires)
            .field("look", &self.look)
            .field("every", &self.every)
            .field("targeting", &self.targeting)
            .field("asks", &self.asks)
            .finish_non_exhaustive()
    }
}

/// Everything a prop, a thing or an actor does at its moments.
///
/// One component for all three, since a trap, a grenade and a venomous
/// bite differ in which moments they answer and never in how the answer
/// lands.
#[derive(Component, Clone, Debug, Default)]
pub struct Triggers(pub Vec<Trigger>);

impl Triggers {
    /// Builds a carrier's triggers from content, once per definition.
    ///
    /// A spec with no list of its own takes `shared`, which is how a thing
    /// says what it contains once and lets each trigger deliver it. Every
    /// problem is reported, not the first: an unknown moment by its name,
    /// an unregistered effect by [`Effects::build`]'s own message, a
    /// trigger with neither a list nor a shared one as the typo it is, a
    /// targeting anywhere but on a thing's one `use` trigger, or aimed at
    /// nobody but its user, and an `asks` anywhere but there or beside a
    /// targeting.
    pub fn build(specs: &[TriggerSpec], shared: &[EffectSpec], moments: &TriggerKinds, kinds: &EffectKinds, names: &Names<'_>) -> Result<Self, Vec<String>> {
        let mut errors = Vec::new();
        // A use is pointed or made on a thing once, so an aimed or asking
        // one is the thing's only use: a second would land around where the
        // first was pointed, or with a thing nobody chose for it.
        let uses: Vec<&TriggerSpec> = specs.iter().filter(|s| moments.get(&s.on) == Some(TriggerKinds::USE)).collect();
        if uses.len() > 1 && uses.iter().any(|s| s.targeting.is_some() || s.asks.is_some()) {
            errors.push("a thing whose use is aimed or asks for a thing has one `use` trigger, that one; write what it lands in its one list".to_string());
        }
        let shared = if shared.is_empty() {
            None
        } else {
            match Effects::build(shared, kinds, names) {
                Ok(e) => Some(Arc::new(e)),
                Err(e) => {
                    errors.extend(e);
                    None
                }
            }
        };
        let mut built = Vec::new();
        for spec in specs {
            let Some(on) = moments.get(&spec.on) else {
                errors.push(format!("no moment is registered as {:?}; the engine's are {}", spec.on, TriggerKinds::BUILT_IN.join(", ")));
                continue;
            };
            if on == TriggerKinds::PULSE {
                match spec.every {
                    None => errors.push("a pulse trigger needs `every`, the hundredths between pulses".to_string()),
                    Some(0) => errors.push("a pulse trigger's `every` of 0 would never come round again".to_string()),
                    Some(_) => {}
                }
            } else if spec.every.is_some() {
                errors.push(format!("the trigger on {:?} carries an `every`; `every` is only for a pulse trigger", spec.on));
            }
            match spec.targeting {
                Some(_) if on != TriggerKinds::USE => {
                    errors.push(format!("the trigger on {:?} carries a `targeting`; only a \"use\" is pointed by whoever makes it", spec.on));
                }
                Some(targeting) if !targeting.aim.needs_cursor() => {
                    errors.push("a use aimed at its own user is a use with no targeting; leave `targeting` out".to_string());
                }
                _ => {}
            }
            if spec.asks.is_some() && on != TriggerKinds::USE {
                errors.push(format!("the trigger on {:?} carries an `asks`; only a \"use\" is made by someone choosing what it is made on", spec.on));
            }
            if spec.asks.is_some() && spec.targeting.is_some() {
                errors.push("a use is aimed at somewhere or asks for a thing, not both".to_string());
            }
            let effects = match (&spec.effects, &shared) {
                (Some(own), _) => match Effects::build(own, kinds, names) {
                    Ok(e) => Arc::new(e),
                    Err(e) => {
                        errors.extend(e);
                        continue;
                    }
                },
                (None, Some(shared)) => shared.clone(),
                (None, None) => {
                    errors.push(format!("the trigger on {:?} has no effects of its own and nothing shared to deliver", spec.on));
                    continue;
                }
            };
            built.push(Trigger {
                on,
                area: spec.area,
                fires: spec.fires,
                effects,
                look: spec.look,
                every: spec.every,
                targeting: spec.targeting,
                asks: spec.asks.clone(),
            });
        }
        if errors.is_empty() { Ok(Self(built)) } else { Err(errors) }
    }

    /// How this carrier's use is pointed, when it is pointed somewhere:
    /// what the bag asks the targeting cursor for in place of using it at
    /// once, and what the items resolver refuses an aim by. `None` for a
    /// thing whose use lands on its user, or that has none.
    pub fn targeting(&self) -> Option<Targeting> {
        self.on(TriggerKinds::USE).find_map(|t| t.targeting)
    }

    /// What this carrier's use asks its user for, when it is used on
    /// another of its user's things: what the bag chooses for in place of
    /// using it at once, and what the items resolver refuses a choice by.
    /// `None` for a thing whose use asks for nothing, or that has none.
    pub fn asks(&self) -> Option<&Asks> {
        self.on(TriggerKinds::USE).find_map(|t| t.asks.as_ref())
    }

    /// The triggers on `moment`, in the order they were written.
    pub fn on(&self, moment: TriggerKindId) -> impl Iterator<Item = &Trigger> {
        self.0.iter().filter(move |t| t.on == moment)
    }

    /// The first pulse trigger's period, for a game that builds a
    /// [`Pulse`](crate::items::Pulse) from its carrier's own triggers rather
    /// than keeping a second copy of the number. `None` when the carrier has
    /// no pulse trigger at all.
    pub(crate) fn pulse_every(&self) -> Option<u32> {
        self.0.iter().find(|t| t.on == TriggerKinds::PULSE).and_then(|t| t.every)
    }
}

/// Something happened to a carrier at a moment.
///
/// The one thing a subsystem that owns a moment writes. What lands is
/// `land_triggers`' business, and what a use costs is the consumables'.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fired {
    /// The carrier: a prop, a thing used or thrown, a weapon.
    pub on: Entity,
    /// Which moment.
    pub moment: TriggerKindId,
    /// Who set it off, when anyone did.
    pub by: Option<Entity>,
    /// Where it happened: for a use pointed somewhere, the cell it was
    /// pointed at, which its aimed trigger flies to from wherever `by`
    /// stands.
    pub at: Point,
}

/// What a moment was made on besides its carrier: the thing a use was used
/// on, for a use that asks its user for one of their own things.
///
/// Written beside the carrier's [`Fired`], in the same pass, by whatever
/// reports such a use, which is the items resolver for a
/// [`UseOn`](crate::items::UseOn). Its own message rather than a field of
/// `Fired`, since every other moment of every subsystem, a shot made, a
/// plate stepped on, would otherwise carry a field it never fills.
/// `land_triggers` pairs it with the `Fired` of the same carrier and
/// moment and lands the trigger with it as [`Landing::on`].
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FiredOn {
    /// The carrier, as its `Fired` names it: the thing used.
    pub carrier: Entity,
    /// The moment, as its `Fired` names it, so another moment of the same
    /// carrier in the pass, a worn thing's pulse, is not made on it.
    pub moment: TriggerKindId,
    /// What it was used on.
    pub on: Entity,
}

/// The cells `area` covers around `at`: the one cell, or the burst of that
/// radius inside the loaded map, stopped by walls, which is what an
/// ability's ball covers once it has landed on `at`, so a grenade and a
/// fireball of one radius reach the same cells.
///
/// Not the targeting call itself: that flies a projectile first and bursts
/// where it stops, and a moment has already happened where it happened, so
/// there is nothing to fly.
pub fn area_cells(area: EffectArea, at: Point, map: &WorldMap) -> Vec<Point> {
    match area {
        EffectArea::Here => vec![at],
        EffectArea::Burst { radius } => atglyph_grid::burst(at, radius, map.window_tiles(), |p| map.blocks_projectiles(p)),
    }
}

/// A carrier whose triggers land as its own doing: a plate, a barrel.
///
/// Without it a landing's user is whoever set the moment off, which is
/// right for a thing used, thrown or fired, since the stim is the
/// drinker's and the grenade the thrower's. A trap is not the doing of
/// whoever stepped on it: credited to them, the log would say they hurt
/// themselves, and a droid a plate killed would have killed itself.
/// [`Fired::by`] still names who set it off, for a game that asks.
///
/// An [`Actor`](crate::components::Actor) needs no marker: whatever it
/// answers is always its own doing, a bite that poisons and an aura that
/// burns whoever struck it alike, since an actor is the someone a thing is
/// used by rather than a thing anyone uses.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct LandsAsItself;

/// A carrier that exists only to land its triggers once, for something that
/// is gone by the time they land: what a broken prop leaves for its
/// `destroyed` moment and a dead actor for its `died`, since either is
/// despawned at the end of the frame it died in.
///
/// The moment is written on the carrier rather than sent as a message,
/// because it lands a pass later and a pass may be a long time coming:
/// the loop holds while something is shown, and a message nobody reads for
/// two frames is gone, where an entity stays. `report_remnants` reports
/// it in the pass that lands it, and `land_triggers` takes its triggers
/// once it has.
///
/// It then stays, with nothing left to land, until `bury_remnants`
/// despawns it at the end of the frame, as the dead are, rather than at
/// once: a remnant that lands as itself is credited with what it did, and
/// whatever reads that later in the pass, the log saying who hurt whom or
/// a game noting what killed its player, must still find its name.
#[derive(Component, Debug, Clone, Copy)]
pub struct Remnant {
    /// The moment it answers.
    pub moment: TriggerKindId,
    /// Who set it off, when anyone did.
    pub by: Option<Entity>,
    /// Where it happened.
    pub at: Point,
}

/// Reports every [`Remnant`]'s moment, in the pass that lands it.
///
/// In [`ResolveSet::Triggers`](crate::plugin::ResolveSet::Triggers) ahead
/// of `land_triggers`, so the report and the landing are one pass
/// however long the remnant waited for it.
pub(crate) fn report_remnants(remnants: Query<(Entity, &Remnant), With<Triggers>>, mut fired: MessageWriter<Fired>) {
    for (on, remnant) in &remnants {
        fired.write(Fired { on, moment: remnant.moment, by: remnant.by, at: remnant.at });
    }
}

/// Despawns every [`Remnant`] that has landed, once every system has read
/// this frame's landings: in `Last`, beside the dead.
pub(crate) fn bury_remnants(mut commands: Commands, landed: Query<Entity, (With<Remnant>, Without<Triggers>)>) {
    for remnant in &landed {
        commands.entity(remnant).despawn();
    }
}

/// Reports the `died` moment for every actor that died answering it.
///
/// A death is only known once damage has been applied, so this is read in
/// [`TurnSet::React`](crate::plugin::TurnSet::React) and lands the pass
/// after, as a broken prop's `destroyed` does and by the same road: the
/// actor is out of the world by the end of the pass and despawned by the
/// end of the frame, so its triggers and its name go on a [`Remnant`] that
/// is reported in its place where it died, and its `by` is whoever killed
/// it. The remnant [`LandsAsItself`] and carries the actor's name, so it is
/// the actor's stand-in: what it lands is credited to it, a foe its last
/// breath kills is its kill and not its killer's, and the log and a record
/// of the death name the actor, though its body is gone.
///
/// Once per actor however many deaths are reported for it in a pass, and
/// never for one that answers nothing on dying.
pub(crate) fn report_died(
    mut commands: Commands,
    mut deaths: MessageReader<crate::combat::DeathEvent>,
    actors: Query<&Triggers, With<crate::components::Actor>>,
    keeps: Res<RemnantKeeps>,
) {
    let mut reported: Vec<Entity> = Vec::new();
    for death in deaths.read() {
        let Ok(triggers) = actors.get(death.entity) else { continue };
        if triggers.on(TriggerKinds::DIED).next().is_none() || reported.contains(&death.entity) {
            continue;
        }
        reported.push(death.entity);
        let remnant = Remnant { moment: TriggerKinds::DIED, by: death.credit, at: death.at };
        let carrier = commands.spawn((triggers.clone(), remnant, LandsAsItself)).id();
        keeps.copy(&mut commands, death.entity, carrier);
    }
}

/// What a remnant takes from what it stands in for, besides its triggers:
/// the [`Name`] it is credited under, and whatever a plugin that reads a
/// remnant said it reads, such as the look a log colours a name in.
///
/// A list a plugin adds to rather than a fixed set, since the components
/// that say how a thing looks belong to crates this one cannot see; the
/// narrator adds its glyph with [`KeepOnRemnant::keep_on_remnant`].
#[derive(Resource, Debug, Clone)]
pub struct RemnantKeeps(Vec<std::any::TypeId>);

impl Default for RemnantKeeps {
    fn default() -> Self {
        Self(vec![std::any::TypeId::of::<Name>()])
    }
}

impl RemnantKeeps {
    /// Adds `C` to what a remnant takes, once however often it is asked.
    pub(crate) fn keep<C: Component + Clone>(&mut self) {
        let id = std::any::TypeId::of::<C>();
        if !self.0.contains(&id) {
            self.0.push(id);
        }
    }

    /// Queues the copy of whichever of these `from` has onto `to`, at the
    /// next sync point, while `from` is still there to copy.
    pub(crate) fn copy(&self, commands: &mut Commands, from: Entity, to: Entity) {
        let ids = self.0.clone();
        commands.entity(from).clone_with_opt_in(to, move |builder| {
            builder.allow_by_ids(ids);
        });
    }
}

/// Says, while the app is built, that a remnant takes a component from
/// what it stands in for.
pub trait KeepOnRemnant {
    /// A remnant takes `C` as it takes the [`Name`].
    fn keep_on_remnant<C: Component + Clone>(&mut self) -> &mut Self;
}

impl KeepOnRemnant for App {
    fn keep_on_remnant<C: Component + Clone>(&mut self) -> &mut Self {
        self.init_resource::<RemnantKeeps>();
        self.world_mut().resource_mut::<RemnantKeeps>().keep::<C>();
        self
    }
}

/// What a carrier is, for `land_triggers`: its triggers, whether it is a
/// remnant, lands as itself or is an actor, its
/// [`EffectBonus`](super::EffectBonus) if it has one, and what it has left
/// to spend if it counts charges.
type Carrier = (
    &'static mut Triggers,
    Has<Remnant>,
    (Has<LandsAsItself>, Has<crate::components::Actor>),
    Option<&'static super::EffectBonus>,
    Option<&'static crate::consumable::Consumable>,
);

/// A use aimed somewhere that has left its user and not yet come down:
/// the landing its trigger worked out as the use was made, what it lands,
/// and how it is seen.
///
/// Worked out when the use is made, as an ability's is when it is cast, so
/// it comes down on whoever stood under it then, followed by the cues
/// wherever they have gone. The list is held rather than looked up again,
/// since the thing may be spent to nothing and gone before its last use
/// lands.
#[derive(Clone)]
pub struct UseLanding {
    landing: Landing,
    effects: Arc<Effects>,
    look: Option<atglyph_rules::ability::ParticleLook>,
}

/// Written by hand, since the effects are boxed trait objects.
impl std::fmt::Debug for UseLanding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UseLanding").field("landing", &self.landing).field("look", &self.look).finish_non_exhaustive()
    }
}

impl crate::cue::Lands for UseLanding {}

impl UseLanding {
    /// Comes down: the burst over the footprint in the trigger's look, when
    /// it has one, then the effects, so a cue an effect adds plays after it,
    /// and the report of whom it came down on.
    fn land(self, world: &mut EffectWorld<'_, '_>, landed: &mut MessageWriter<LandedOn>) {
        if let Some(burst) = self.look.and_then(|look| self.landing.burst(world, LookOf::Given(look))) {
            world.cues.write(Cued { actor: self.landing.user, cue: burst });
        }
        self.effects.land(&self.landing, world);
        landed.write(LandedOn::of(&self.landing));
    }
}

/// Where a use aimed somewhere goes and who it catches, and the hold its
/// flight keeps on the turns while it is seen.
#[derive(bevy::ecs::system::SystemParam)]
pub struct AimedUses<'w, 's> {
    bystanders: Bystanders<'w, 's>,
    cue_wait: ResMut<'w, CueWait>,
    airborne: ResMut<'w, Airborne<UseLanding>>,
    /// Whom each came down on.
    landed: MessageWriter<'w, LandedOn>,
}

impl AimedUses<'_, '_> {
    /// Lands `trigger` as `aimed` points it, on its footprint, with `bonus`
    /// added: at once, or once its flight has been seen when it is seen to
    /// fly and something watches.
    ///
    /// Whether it might be pointed there was the resolver's question, asked
    /// before the use was spent, so nothing here refuses: a moment reported
    /// is a moment that happened.
    fn land(&mut self, aimed: Aimed, trigger: &Trigger, bonus: super::EffectBonus, world: &mut EffectWorld) {
        let mut landing = self.bystanders.land(aimed, world.map(), world.occupancy()).landing;
        // An aimed use is never refused by the time it gets here, unlike
        // an ability: the items resolver already asked the same question
        // before spending the use, so this is the one and only landing of
        // it, and the right moment for a reflecting body to turn it back.
        self.bystanders.reflect(&mut landing, aimed.targeting, world.map());
        landing.bonus = bonus;
        let shot = UseLanding { landing, effects: trigger.effects.clone(), look: trigger.look };
        let flights = shot.look.map(|look| shot.landing.flights(world, LookOf::Given(look))).unwrap_or_default();
        if flights.is_empty() {
            shot.land(world, &mut self.landed);
            return;
        }
        for flight in flights {
            world.cues.write(Cued { actor: aimed.user, cue: flight });
        }
        if let Some(shot) = self.airborne.launched(&mut self.cue_wait, shot) {
            shot.land(world, &mut self.landed);
        }
    }
}

/// Lands every use aimed somewhere that is in the air, on the first pass
/// after its flight has been seen. Landing counts as progress, so the loop
/// goes on to deal the next turn once the burst has been seen in turn.
///
/// In [`ResolveSet::Triggers`](crate::plugin::ResolveSet::Triggers), where
/// triggers land, ahead of `land_triggers`: no turn is dealt while
/// anything flies, so nothing new is used in the pass that lands it.
pub(crate) fn land_aimed_uses(
    mut cue_wait: ResMut<CueWait>,
    mut airborne: ResMut<Airborne<UseLanding>>,
    mut turns: ResMut<Turns>,
    mut world: EffectWorld,
    mut landed: MessageWriter<LandedOn>,
) {
    for shot in airborne.landing(&mut cue_wait, &mut turns) {
        shot.land(&mut world, &mut landed);
    }
}

/// Lands every carrier's triggers for each moment reported this pass.
///
/// In [`ResolveSet::Triggers`](crate::plugin::ResolveSet::Triggers), after
/// every system that reports a moment in the same pass and before fire,
/// gas, statuses and damage, so whatever a trigger starts is resolved in
/// the pass that set it off. Every actor under the footprint is a target,
/// the one who set it off included: a grenade does not ask whose it was.
/// An actor's own triggers are the one exception, and spare the actor
/// they belong to, since a bite, an aura or a cloud it gives off is aimed
/// at whoever is round it: a monster's fire aura does not burn the monster.
/// The user is whoever set it off, or the carrier when it is
/// [`LandsAsItself`], an actor, or nobody set it off. A carrier with an
/// [`EffectBonus`](super::EffectBonus) lands its effects with it added,
/// and a plain one adds nothing.
///
/// A use aimed somewhere is the exception, and is landed the way an
/// ability is, by [`Bystanders::land`]: from where its user stands to the
/// cell it was pointed at, on whoever its aim wants under its shape. With
/// a look it is seen to fly there first, and with something watching it
/// waits in the air for `land_aimed_uses` until it has been.
///
/// A use made on one of its user's things lands where its user stands,
/// with the thing its [`FiredOn`] names as the landing's
/// [`on`](Landing::on), for the game's own effect that reads it.
///
/// A use of a thing with nothing left to spend lands nothing, and nothing
/// is seen to fly. Only a thing not yet known is used empty, since a known
/// one is refused before its turn is spent; one not yet known is used,
/// because refusing it would say it was empty, and comes to nothing.
pub(crate) fn land_triggers(
    mut commands: Commands,
    mut fired: MessageReader<Fired>,
    mut fired_on: MessageReader<FiredOn>,
    mut carriers: Query<Carrier>,
    alive: Query<(), (With<crate::combat::Health>, Without<crate::combat::Dead>)>,
    mut uses: AimedUses,
    mut world: EffectWorld,
) {
    let mut made_on: Vec<FiredOn> = fired_on.read().copied().collect();
    for f in fired.read() {
        // Taken once, so a second use of the same carrier in the pass is
        // not made on the first one's thing.
        let on = made_on.iter().position(|m| m.carrier == f.on && m.moment == f.moment).map(|i| made_on.remove(i).on);
        let Ok((mut triggers, remnant, (itself, actor), bonus, held)) = carriers.get_mut(f.on) else { continue };
        let user = atglyph_rules::effects::landing_user(itself, actor, f.on, f.by);
        let bonus = bonus.copied().unwrap_or_default();
        // Read before the charge this use spends is taken off, which comes
        // after the landing: empty now is empty when it was used.
        let used_empty = f.moment == TriggerKinds::USE && held.is_some_and(|c| c.is_empty());
        for trigger in triggers.0.iter_mut().filter(|t| t.on == f.moment && !used_empty) {
            if trigger.fires == Some(0) {
                continue;
            }
            let source = Source::Trigger { on: f.on, moment: f.moment };
            match trigger.targeting {
                // From wherever its user stands to the cell it was pointed
                // at, which is where the use is reported to have happened.
                Some(targeting) => {
                    let origin = world.position(user).unwrap_or(f.at);
                    uses.land(Aimed { user, source, targeting, origin, aim: f.at, sees_aim: None }, trigger, bonus, &mut world);
                }
                None => {
                    let cells = area_cells(trigger.area, f.at, world.map());
                    let mut targets: Vec<Entity> = Vec::new();
                    for cell in &cells {
                        for who in world.occupancy().at(*cell) {
                            // An actor's own answer spares it: an aura
                            // burns whoever stands round its bearer.
                            if alive.contains(*who) && !targets.contains(who) && !(actor && *who == f.on) {
                                targets.push(*who);
                            }
                        }
                    }
                    let landing = Landing {
                        user,
                        source,
                        origin: f.at,
                        aim: f.at,
                        cells,
                        path: Vec::new(),
                        landed_at: Some(f.at),
                        targets,
                        bonus,
                        on,
                        turned_by: Vec::new(),
                    };
                    // Cued before any effect runs, as an ability's burst is,
                    // so a cue an effect adds plays after it.
                    if let Some(burst) = trigger.look.and_then(|look| landing.burst(&world, LookOf::Given(look))) {
                        world.cues.write(Cued { actor: user, cue: burst });
                    }
                    trigger.effects.land(&landing, &mut world);
                }
            }
            if let Some(left) = trigger.fires.as_mut() {
                *left -= 1;
            }
        }
        // Landed: nothing is left to report, and its name stays until the
        // frame ends for whatever reads who did it.
        if remnant {
            commands.entity(f.on).remove::<Triggers>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{CombatPlugin, DamageDealt, Health};
    use crate::components::{Actor, Blocks, Position};
    use crate::effects::AddEngineEffects;
    use crate::registries::Registries;
    use crate::state::EngineState;
    use crate::testing::headless_app;

    /// A world with combat and the engine's effects, open land to stand on,
    /// and one damage kind, `kinetic`, as combat's own tests build it.
    fn floor() -> (App, Point) {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, crate::effects::EffectsPlugin));
        app.add_engine_effects();
        let start = crate::testing::ground(&mut app);
        crate::testing::two_sides(&mut app);
        // A turn wants a player, so one stands a few cells off, with no health, which no burst in these tests reaches.
        app.world_mut().spawn((Actor, crate::components::Player, Position(start.offset(0, -6))));
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        (app, start)
    }

    fn harm(roll: &str) -> Vec<atglyph_rules::EffectSpec> {
        ron::from_str(&format!(r#"[(kind: "Harm", args: (kind: "kinetic", roll: "{roll}"))]"#)).unwrap()
    }

    fn build(app: &App, specs: &str, shared: &[atglyph_rules::EffectSpec]) -> Result<Triggers, Vec<String>> {
        let specs: Vec<atglyph_rules::TriggerSpec> =
            ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME).from_str(specs).unwrap();
        let world = app.world();
        let registries = world.resource::<Registries>();
        Triggers::build(&specs, shared, world.resource::<TriggerKinds>(), world.resource::<EffectKinds>(), &registries.names())
    }

    /// Reports `moment` on `on` at `at` and runs one pass of the turn loop,
    /// which is where a moment is landed.
    fn fire(app: &mut App, on: Entity, moment: TriggerKindId, at: Point) {
        app.world_mut().write_message(Fired { on, moment, by: None, at });
        app.world_mut().run_schedule(crate::plugin::Turn);
        app.update();
    }

    fn health(app: &App, e: Entity) -> i32 {
        app.world().get::<Health>(e).unwrap().current
    }

    fn stand(app: &mut App, at: Point) -> Entity {
        let e = app.world_mut().spawn((Actor, Blocks, Position(at), Health::full(10))).id();
        app.update();
        e
    }

    #[test]
    fn a_moment_nobody_registered_fails_the_build_naming_it() {
        let (app, _) = floor();
        let errs = build(&app, r#"[(on: "explode")]"#, &harm("1")).expect_err("an unknown moment refuses");
        assert!(errs.iter().any(|e| e.contains("\"explode\"")), "{errs:?}");
    }

    #[test]
    fn a_trigger_with_neither_a_list_nor_a_shared_one_fails_the_build() {
        let (app, _) = floor();
        assert!(build(&app, r#"[(on: "use")]"#, &[]).is_err(), "a trigger that does nothing is a typo");
    }

    #[test]
    fn a_trigger_with_no_list_of_its_own_lands_the_shared_one() {
        let (mut app, at) = floor();
        let triggers = build(&app, r#"[(on: "use")]"#, &harm("3")).unwrap();
        let under = stand(&mut app, at);
        let carrier = app.world_mut().spawn(triggers).id();
        fire(&mut app, carrier, TriggerKinds::USE, at);
        assert_eq!(health(&app, under), 7);
    }

    #[test]
    fn here_lands_on_the_one_cell_and_nobody_beside_it() {
        let (mut app, at) = floor();
        let triggers = build(&app, r#"[(on: "use")]"#, &harm("3")).unwrap();
        let under = stand(&mut app, at);
        let beside = stand(&mut app, at.offset(1, 0));
        let carrier = app.world_mut().spawn(triggers).id();
        fire(&mut app, carrier, TriggerKinds::USE, at);
        assert_eq!((health(&app, under), health(&app, beside)), (7, 10));
    }

    #[test]
    fn a_burst_lands_on_everyone_within_its_radius_and_nobody_past_it() {
        let (mut app, at) = floor();
        let triggers = build(&app, r#"[(on: "land", area: Burst(radius: 1))]"#, &harm("3")).unwrap();
        let near = stand(&mut app, at.offset(1, 0));
        let far = stand(&mut app, at.offset(3, 0));
        let carrier = app.world_mut().spawn(triggers).id();
        fire(&mut app, carrier, TriggerKinds::LAND, at);
        assert_eq!((health(&app, near), health(&app, far)), (7, 10));
    }

    /// Every cue written, copied out as it is written.
    #[derive(Resource, Default)]
    struct Cues(Vec<crate::cue::Cue>);

    fn keep_cues(mut cued: MessageReader<crate::cue::Cued>, mut cues: ResMut<Cues>) {
        cues.0.extend(cued.read().map(|c| c.cue.clone()));
    }

    /// A trigger with a look is seen where it lands, spreading out from
    /// there over every cell it covers, the way an ability with a look is;
    /// one with none cues
    /// nothing, as a trap nobody should see go off does not.
    #[test]
    fn a_trigger_with_a_look_bursts_over_its_cells_and_one_without_shows_nothing() {
        let (mut app, at) = floor();
        app.init_resource::<Cues>().add_systems(bevy::app::PostUpdate, keep_cues);
        let seen = build(&app, r#"[(on: "land", area: Burst(radius: 1), look: (glyph: '*', color: (r: 255, g: 128, b: 0)))]"#, &harm("3")).unwrap();
        let carrier = app.world_mut().spawn(seen).id();
        fire(&mut app, carrier, TriggerKinds::LAND, at);
        let look = atglyph_rules::ability::ParticleLook { glyph: '*', color: atglyph_grid::Rgb::new(255, 128, 0) };
        let cues = std::mem::take(&mut app.world_mut().resource_mut::<Cues>().0);
        let [crate::cue::Cue::Burst { on, look: shown, from }] = cues.as_slice() else { panic!("one burst: {cues:?}") };
        assert_eq!(*shown, crate::cue::LookOf::Given(look));
        assert_eq!(from.map(|a| a.at), Some(at), "going off where it landed");
        let mut cells: Vec<Point> = on.iter().map(|a| a.at).collect();
        let mut expected = area_cells(atglyph_rules::EffectArea::Burst { radius: 1 }, at, app.world().resource::<WorldMap>());
        cells.sort();
        expected.sort();
        assert_eq!(cells, expected, "over every cell it covers");

        let unseen = build(&app, r#"[(on: "land", area: Burst(radius: 1))]"#, &harm("3")).unwrap();
        let carrier = app.world_mut().spawn(unseen).id();
        fire(&mut app, carrier, TriggerKinds::LAND, at);
        assert!(app.world().resource::<Cues>().0.is_empty(), "no look, nothing shown");
    }

    /// A wall between the burst and someone standing behind it shelters
    /// them; someone the same distance off in the open is caught.
    #[test]
    fn a_burst_stops_at_a_wall_and_catches_whoever_stands_in_the_open() {
        let (mut app, at) = floor();
        app.world_mut().resource_mut::<WorldMap>().set_tile(at.offset(1, 0), atglyph_grid::TileId(1));
        let triggers = build(&app, r#"[(on: "land", area: Burst(radius: 2))]"#, &harm("3")).unwrap();
        let sheltered = stand(&mut app, at.offset(2, 0));
        let open = stand(&mut app, at.offset(-2, 0));
        let carrier = app.world_mut().spawn(triggers).id();
        fire(&mut app, carrier, TriggerKinds::LAND, at);
        assert_eq!((health(&app, sheltered), health(&app, open)), (10, 7));
    }

    /// A burst reaches exactly what an ability's ball of the same radius
    /// reaches, because it is the same call.
    #[test]
    fn a_burst_covers_the_cells_an_abilitys_ball_of_that_radius_covers_once_it_lands() {
        let (app, at) = floor();
        let map = app.world().resource::<WorldMap>();
        // A real ball, thrown from two cells off and landing on `at`, so the
        // comparison cannot pass on two empty lists.
        let ball = atglyph_grid::footprint(
            atglyph_grid::TargetMode::Ball { range: 5, radius: 2 },
            at.offset(-2, 0),
            at,
            map.window_tiles(),
            |_| false,
            |p| map.blocks_projectiles(p),
        );
        assert_eq!(ball.landing, Some(at), "the ball landed where it was aimed");
        assert!(ball.cells.len() > 9, "and covers a disc, not a cell");
        let mut burst = area_cells(atglyph_rules::EffectArea::Burst { radius: 2 }, at, map);
        let mut cells = ball.cells.clone();
        burst.sort();
        cells.sort();
        assert_eq!(burst, cells);
        assert_eq!(area_cells(atglyph_rules::EffectArea::Here, at, map), vec![at]);
    }

    #[test]
    fn a_trigger_fires_as_often_as_it_says_per_carrier_and_not_per_definition() {
        let (mut app, at) = floor();
        let triggers = build(&app, r#"[(on: "entered", fires: 1)]"#, &harm("1")).unwrap();
        let victim = stand(&mut app, at);
        let one = app.world_mut().spawn(triggers.clone()).id();
        let two = app.world_mut().spawn(triggers).id();
        fire(&mut app, one, TriggerKinds::ENTERED, at);
        fire(&mut app, one, TriggerKinds::ENTERED, at);
        assert_eq!(health(&app, victim), 9, "the first carrier went off once");
        fire(&mut app, two, TriggerKinds::ENTERED, at);
        assert_eq!(health(&app, victim), 8, "and springing it spent nothing of the second");
    }

    #[test]
    fn a_moment_a_game_registered_lands_like_one_of_the_engines() {
        let (mut app, at) = floor();
        app.add_trigger_kind("overheat");
        let overheat = app.world().resource::<TriggerKinds>().get("overheat").unwrap();
        let triggers = build(&app, r#"[(on: "overheat")]"#, &harm("2")).unwrap();
        let wielder = stand(&mut app, at);
        let gun = app.world_mut().spawn(triggers).id();
        fire(&mut app, gun, overheat, at);
        assert_eq!(health(&app, wielder), 8);
    }

    #[test]
    fn a_moment_the_carrier_has_no_trigger_for_lands_nothing() {
        let (mut app, at) = floor();
        let triggers = build(&app, r#"[(on: "use")]"#, &harm("3")).unwrap();
        let under = stand(&mut app, at);
        let carrier = app.world_mut().spawn(triggers).id();
        fire(&mut app, carrier, TriggerKinds::LAND, at);
        assert_eq!(health(&app, under), 10);
    }

    #[test]
    fn two_triggers_on_one_moment_land_in_the_order_they_were_written() {
        let (mut app, at) = floor();
        let triggers = build(
            &app,
            r#"[(on: "use", effects: [(kind: "Harm", args: (kind: "kinetic", roll: "2"))]), (on: "use", effects: [(kind: "Harm", args: (kind: "kinetic", roll: "5"))])]"#,
            &[],
        )
        .unwrap();
        stand(&mut app, at);
        app.init_resource::<DealtLog>().add_systems(bevy::app::PostUpdate, keep_dealt);
        let carrier = app.world_mut().spawn(triggers).id();
        fire(&mut app, carrier, TriggerKinds::USE, at);
        app.update();
        assert_eq!(app.world().resource::<DealtLog>().0, vec![2, 5]);
    }

    #[derive(Resource, Default)]
    struct DealtLog(Vec<i32>);

    fn keep_dealt(mut dealt: MessageReader<DamageDealt>, mut log: ResMut<DealtLog>) {
        log.0.extend(dealt.read().map(|d| d.dealt));
    }

    /// The pulse is the engine's seventh moment, added at the end so every
    /// moment before it keeps the id a save or a content file already has.
    #[test]
    fn the_pulse_is_a_built_in_moment_after_the_six_that_came_first() {
        let moments = TriggerKinds::default();
        assert_eq!(moments.get("pulse"), Some(TriggerKinds::PULSE));
        assert_eq!(TriggerKinds::PULSE, TriggerKindId::from_raw(6));
        assert_eq!(moments.get("destroyed"), Some(TriggerKinds::DESTROYED), "the old ids did not move");
    }

    /// An actor's two moments come after the pulse, for the same reason the
    /// pulse came after the six, and a game's own come after them.
    #[test]
    fn struck_and_died_are_built_in_moments_after_the_seven_that_came_first() {
        let mut moments = TriggerKinds::default();
        assert_eq!((moments.get("struck"), moments.get("died")), (Some(TriggerKinds::STRUCK), Some(TriggerKinds::DIED)));
        assert_eq!((TriggerKinds::STRUCK, TriggerKinds::DIED), (TriggerKindId::from_raw(7), TriggerKindId::from_raw(8)));
        assert_eq!(moments.get("pulse"), Some(TriggerKinds::PULSE), "the old ids did not move");
        assert_eq!(moments.declare("overheat"), TriggerKindId::from_raw(9));
    }

    #[test]
    fn a_pulse_trigger_with_no_every_fails_the_build_naming_the_reason() {
        let (app, _) = floor();
        let errs = build(&app, r#"[(on: "pulse")]"#, &harm("1")).expect_err("a pulse with no period refuses");
        assert!(errs.iter().any(|e| e.contains("needs `every`")), "{errs:?}");
    }

    #[test]
    fn an_every_on_any_trigger_but_a_pulse_fails_the_build_naming_the_reason() {
        let (app, _) = floor();
        let errs = build(&app, r#"[(on: "use", every: 1000)]"#, &harm("1")).expect_err("every belongs only to a pulse");
        assert!(errs.iter().any(|e| e.contains("only for a pulse trigger")), "{errs:?}");
    }

    #[test]
    fn a_pulse_trigger_with_an_every_of_zero_fails_the_build_naming_the_reason() {
        let (app, _) = floor();
        let errs = build(&app, r#"[(on: "pulse", every: 0)]"#, &harm("1")).expect_err("zero never waits");
        assert!(errs.iter().any(|e| e.contains("never come round again")), "{errs:?}");
    }

    /// Every problem in one item is reported at once, the three `every`
    /// mistakes included: fixing one at a time is how a morning goes.
    #[test]
    fn the_three_every_mistakes_are_reported_together_with_the_items_other_errors() {
        let (app, _) = floor();
        let errs =
            build(&app, r#"[(on: "pulse"), (on: "use", every: 500), (on: "pulse", every: 0), (on: "explode")]"#, &harm("1")).expect_err("four mistakes refuse");
        assert!(errs.iter().any(|e| e.contains("needs `every`")), "{errs:?}");
        assert!(errs.iter().any(|e| e.contains("only for a pulse trigger")), "{errs:?}");
        assert!(errs.iter().any(|e| e.contains("never come round again")), "{errs:?}");
        assert!(errs.iter().any(|e| e.contains("\"explode\"")), "{errs:?}");
        assert_eq!(errs.len(), 4, "{errs:?}");
    }

    #[test]
    fn a_pulse_trigger_with_an_every_builds_and_pulse_every_returns_its_period() {
        let (app, _) = floor();
        let triggers = build(&app, r#"[(on: "pulse", every: 1000)]"#, &harm("1")).expect("a pulse trigger with its period builds");
        assert_eq!(triggers.pulse_every(), Some(1000));
    }

    #[test]
    fn a_carrier_with_no_pulse_trigger_has_no_pulse_period() {
        let (app, _) = floor();
        let triggers = build(&app, r#"[(on: "use")]"#, &harm("1")).unwrap();
        assert_eq!(triggers.pulse_every(), None);
    }

    /// Only a use is pointed by whoever makes it: a targeting on a landing
    /// or a hit has nobody choosing where it goes.
    #[test]
    fn a_targeting_on_any_trigger_but_a_use_fails_the_build_naming_the_reason() {
        let (app, _) = floor();
        let errs = build(&app, r#"[(on: "land", targeting: (mode: Bolt(range: 5)))]"#, &harm("1")).expect_err("a landing is not pointed");
        assert!(errs.iter().any(|e| e.contains("only a \"use\"")), "{errs:?}");
    }

    /// A use aimed at its user is a use with no targeting, and saying both
    /// is a mistake rather than a choice.
    #[test]
    fn a_use_aimed_at_its_own_user_fails_the_build() {
        let (app, _) = floor();
        let errs = build(&app, r#"[(on: "use", targeting: (mode: Own, aim: SelfOnly))]"#, &harm("1")).expect_err("aimed at nobody but its user");
        assert!(errs.iter().any(|e| e.contains("leave `targeting` out")), "{errs:?}");
    }

    /// A use is pointed once, so a thing whose use is aimed has that one
    /// `use` trigger: a second would land around where the first was
    /// pointed, which nobody chose.
    #[test]
    fn a_thing_whose_use_is_aimed_has_no_other_use_trigger() {
        let (app, _) = floor();
        let errs = build(&app, r#"[(on: "use", targeting: (mode: Bolt(range: 5))), (on: "use")]"#, &harm("1")).expect_err("two uses, one aimed");
        assert!(errs.iter().any(|e| e.contains("one `use` trigger")), "{errs:?}");
    }

    /// Only a use is made by someone who could choose what it is made on,
    /// and a use is made at one thing: an `asks` on a landing, one beside
    /// a targeting, and one beside a second `use` trigger each fail the
    /// build.
    #[test]
    fn an_ask_on_any_trigger_but_a_things_one_use_or_beside_a_targeting_fails_the_build_naming_the_reason() {
        let (app, _) = floor();
        let landing = build(&app, r#"[(on: "land", asks: (prompt: "Which?"))]"#, &harm("1")).expect_err("a landing asks nobody");
        assert!(landing.iter().any(|e| e.contains("only a \"use\"")), "{landing:?}");
        let both = build(&app, r#"[(on: "use", targeting: (mode: Bolt(range: 5)), asks: (prompt: "Which?"))]"#, &harm("1")).expect_err("aimed and asking");
        assert!(both.iter().any(|e| e.contains("not both")), "{both:?}");
        let two = build(&app, r#"[(on: "use", asks: (prompt: "Which?")), (on: "use")]"#, &harm("1")).expect_err("two uses, one asking");
        assert!(two.iter().any(|e| e.contains("one `use` trigger")), "{two:?}");
    }

    /// What the bag asks of a thing: what its use asks for, when it asks.
    #[test]
    fn a_carriers_ask_is_its_use_triggers_and_none_for_a_use_that_asks_for_nothing() {
        let (app, _) = floor();
        let asking = build(&app, r#"[(on: "use", asks: (prompt: "Which?")), (on: "land")]"#, &harm("1")).unwrap();
        assert_eq!(asking.asks().map(|a| a.prompt.as_str()), Some("Which?"));
        assert_eq!(asking.targeting(), None, "and it is not aimed");
        assert!(build(&app, r#"[(on: "use"), (on: "land")]"#, &harm("1")).unwrap().asks().is_none());
    }

    /// What a landing was made on, as a test effect sees it.
    #[derive(Resource, Default)]
    struct LandedOn(Vec<Option<Entity>>);

    /// Notes what each landing it is part of was made on.
    struct NoteOn;

    impl super::super::Effect for NoteOn {
        fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
            let on = landing.on;
            world.commands.queue(move |w: &mut World| w.resource_mut::<LandedOn>().0.push(on));
        }
    }

    impl super::super::FromArgs for NoteOn {
        const KIND: &'static str = "NoteOn";
        fn from_args(_: &atglyph_rules::ability::RawValue, _: &Names<'_>) -> Result<Self, String> {
            Ok(Self)
        }
    }

    /// A use reported with what it was made on lands with that thing on
    /// its landing, for a game's own effect to read; another carrier's use,
    /// and another moment of the same carrier in the same pass, land on
    /// nothing in particular.
    #[test]
    fn a_use_made_on_a_thing_lands_with_that_thing_and_nothing_else_does() {
        use crate::effects::AddEffect;
        let (mut app, at) = floor();
        app.add_effect::<NoteOn>().init_resource::<LandedOn>();
        let triggers =
            build(&app, r#"[(on: "use", asks: (prompt: "Which?"), effects: [(kind: "NoteOn")]), (on: "land", effects: [(kind: "NoteOn")])]"#, &[]).unwrap();
        let (carrier, other) = (app.world_mut().spawn(triggers.clone()).id(), app.world_mut().spawn(triggers).id());
        let thing = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(FiredOn { carrier, moment: TriggerKinds::USE, on: thing });
        app.world_mut().write_message(Fired { on: other, moment: TriggerKinds::USE, by: None, at });
        app.world_mut().write_message(Fired { on: carrier, moment: TriggerKinds::LAND, by: None, at });
        fire(&mut app, carrier, TriggerKinds::USE, at);
        assert_eq!(app.world().resource::<LandedOn>().0, vec![None, None, Some(thing)], "only the use it was made on carries the thing");
    }

    /// What the bag and the cursor ask of a thing: how its use is pointed,
    /// when it is.
    #[test]
    fn a_carriers_targeting_is_its_use_triggers_and_none_for_a_use_that_lands_on_its_user() {
        let (app, _) = floor();
        let aimed = build(&app, r#"[(on: "use", targeting: (mode: Bolt(range: 5), aim: Ground)), (on: "land")]"#, &harm("1")).unwrap();
        let bolt = atglyph_rules::Targeting { mode: atglyph_grid::TargetMode::Bolt { range: 5 }, aim: atglyph_rules::Aim::Ground, sight: true };
        assert_eq!(aimed.targeting(), Some(bolt));
        assert_eq!(build(&app, r#"[(on: "use"), (on: "land")]"#, &harm("1")).unwrap().targeting(), None);
    }
}
