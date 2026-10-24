//! Statuses on actors: applied by request, ticked by the turn, expired
//! by the clock.
//!
//! The rules crate holds the model: a registered status with stat
//! modifiers and damage per turn, and the instances an actor carries.
//! This module gives every actor a [`StatBlock`] the modifiers land in
//! and an [`ActiveStatuses`] list, resolves [`ApplyStatus`] requests, and on every
//! whole turn ticks the statuses of the actors on the current map,
//! sending their damage through the same pipeline a blow goes through.
//! A status that holds takes its holder's turns: `pass_held_turns`
//! spends each as a wait before its mind or its player is asked. A status
//! that sleeps holds as well, and ends the moment its holder is woken:
//! by harm landing on it (`wake_on_harm`), by a noise it hears
//! (`wake_on_noise`), or by a foe close by that it notices
//! (`wake_on_sight`). `Sleepers` answers who is asleep, which a blow
//! asks before it lands, since a sleeper is unaware of whoever strikes it.
//!
//! What a status lends its holder while it lasts is kept in
//! [`ReactSet::Statuses`](crate::plugin::ReactSet::Statuses), read off
//! what it holds every pass so a status put back by a save does it again
//! with nothing else saved: the side it names (`keep_sides`, with
//! [`SideWas`] the side to go back to), what it resists
//! (`keep_status_resists`, into `StatusResists`), and what the
//! holder's own kind changes while it holds one (`keep_while_holding`,
//! from `WhileHolding`). A status that stumbles turns its holder's
//! chosen step a random open way (`stumble`, on `StumbleRng`).
//! Part of [`CorePlugin`](crate::plugin::CorePlugin): the statuses
//! themselves are content, in [`Registries`], and a game that registers
//! none has nothing to tick, so there is nothing to turn on. Without
//! `Registries` at all nobody is afflicted.

use atglyph_core::Direction;
use atglyph_core::turn::BASE_ACTION_COST;
use atglyph_rules::faction::stance;
use atglyph_rules::stats::{Modifier, ModifierSource, Op};
use atglyph_rules::{Hit, Relation, Resistances, StatId, Stats, StatusId, Statuses};
use bevy::prelude::*;

use crate::combat::{CombatRules, DamageDealt, DamageEvent, Dead, Faction, Heal};
use crate::components::{Actor, MyTurn, Position, Viewshed};
use crate::places::{MapId, OnMap};
use crate::registries::Registries;
use crate::turn::{Acting, ActionDone, Intent, Step, TurnEnd};
use crate::world::WorldMap;

/// An actor's stats: base values and every modifier from gear, statuses
/// and whatever else the game folds in.
#[derive(Component, Debug, Clone, Default, Deref, DerefMut)]
pub struct StatBlock(pub Stats);

/// The statuses an actor carries.
#[derive(Component, Debug, Clone, Default, Deref, DerefMut)]
pub struct ActiveStatuses(pub Statuses);

/// A request to put a status on an actor. Resolved in
/// [`TurnSet::Resolve`](crate::plugin::TurnSet::Resolve), before damage.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApplyStatus {
    /// Who.
    pub target: Entity,
    /// Which.
    pub status: StatusId,
    /// For how many whole turns.
    pub turns: u32,
    /// Who did it, for credit on the damage it deals.
    pub by: Option<Entity>,
    /// The worn thing it lasts only while, or `None` for a status that runs
    /// its time whatever is worn.
    ///
    /// Held, it ends through the ordinary [`RemoveStatus`] in the pass its holder
    /// is not wearing that thing, however that came about; see
    /// `end_unworn_statuses`. One landed on
    /// somebody who is not wearing the thing, as an area use reaching a
    /// bystander, ends in the pass after it goes on. A refresh or an
    /// extension by a held request is held by the new thing, and one by an
    /// unheld request stays held by the old, which is why a held refresh
    /// reaching a bystander who already carries the same status unheld
    /// takes their instance over and cures it the very next pass: `held_by`
    /// is meant for a trigger that only ever lands on its own wearer, a
    /// `use` on oneself, not one that reaches an area or a `hit`.
    pub held_by: Option<Entity>,
}

/// A request to take a status off an actor.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoveStatus {
    /// Who.
    pub target: Entity,
    /// Which.
    pub status: StatusId,
}

/// What happened to a status.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusEvent {
    /// It was put on, and was not on before: it began.
    Applied {
        /// Who.
        target: Entity,
        /// Which.
        status: StatusId,
    },
    /// It was put on again while it was still on: its time refreshed or
    /// extended, or another of it stacked beside the first.
    ///
    /// Apart from [`Applied`](Self::Applied) because a cloud keeps its
    /// status on whoever breathes it by putting it on every turn, and a
    /// reader that tells a status as news tells it when it begins, not
    /// every turn it is kept up. A put-on that changed nothing, a shorter
    /// refresh or an ignored second, is neither.
    Renewed {
        /// Who.
        target: Entity,
        /// Which.
        status: StatusId,
    },
    /// Its time ran out.
    Expired {
        /// Who.
        target: Entity,
        /// Which.
        status: StatusId,
    },
    /// It was taken off.
    Cured {
        /// Who.
        target: Entity,
        /// Which.
        status: StatusId,
    },
}

/// Who was put to sleep this pass, kept from where the sleep is put on to
/// where harm would wake it, which takes the list.
///
/// A resource the two status systems share rather than the [`ApplyStatus`]s
/// read a second time: a second reader of those would be one more system
/// touching them unordered beside every game system that writes one.
#[derive(Resource, Debug, Default)]
pub struct FellAsleep(Vec<Entity>);

/// Applies and removes statuses as asked, and notes who was put to sleep
/// in [`FellAsleep`]. With no [`Registries`] there is no status to apply.
/// `FellAsleep` is read as optional so the system can run on its own, in a
/// game's test of what it afflicts, with nothing to wake anyone on harm.
pub(crate) fn resolve_afflictions(
    mut afflicts: MessageReader<ApplyStatus>,
    mut cures: MessageReader<RemoveStatus>,
    mut events: MessageWriter<StatusEvent>,
    mut fell: Option<ResMut<FellAsleep>>,
    registries: Option<Res<Registries>>,
    mut actors: Query<(&mut ActiveStatuses, &mut StatBlock)>,
) {
    let Some(registries) = registries.as_deref() else { return };
    for a in afflicts.read() {
        if let Some(fell) = fell.as_deref_mut()
            && registries.statuses.get(a.status).sleeps
        {
            fell.0.push(a.target);
        }
        let Ok((mut statuses, mut stats)) = actors.get_mut(a.target) else { continue };
        let by = a.by.map(|e| e.to_bits());
        let began = !statuses.0.has(a.status);
        let changed = match a.held_by {
            Some(item) => statuses.0.apply_held(a.status, a.turns, by, item.to_bits(), &registries.statuses, &mut stats.0),
            None => statuses.0.apply(a.status, a.turns, by, &registries.statuses, &mut stats.0),
        };
        if changed {
            let (target, status) = (a.target, a.status);
            events.write(if began { StatusEvent::Applied { target, status } } else { StatusEvent::Renewed { target, status } });
        }
    }
    for c in cures.read() {
        let Ok((mut statuses, mut stats)) = actors.get_mut(c.target) else { continue };
        if statuses.0.cure(c.status, &mut stats.0) {
            events.write(StatusEvent::Cured { target: c.target, status: c.status });
        }
    }
}

/// Once per whole turn, ticks every afflicted actor on the current map:
/// damage over time becomes [`DamageEvent`]s credited to whoever applied
/// the status, a status that mends becomes [`Heal`]s, and what ran out is
/// reported.
pub(crate) fn tick_statuses(
    mut ends: MessageReader<TurnEnd>,
    mut damage: MessageWriter<DamageEvent>,
    mut heals: MessageWriter<Heal>,
    mut events: MessageWriter<StatusEvent>,
    registries: Option<Res<Registries>>,
    map: Res<WorldMap>,
    mut actors: Query<(Entity, &mut ActiveStatuses, &mut StatBlock, Option<&OnMap>), With<Actor>>,
) {
    let turns = ends.read().count();
    let Some(registries) = registries.as_deref() else { return };
    if turns == 0 {
        return;
    }
    let here = map.current();
    for (entity, mut statuses, mut stats, on) in &mut actors {
        if on.map(|m| m.0).unwrap_or(MapId::ZERO) != here {
            continue;
        }
        for _ in 0..turns {
            let report = statuses.0.tick(&registries.statuses, &mut stats.0);
            for t in report.ticks {
                let credit = t.source.and_then(Entity::try_from_bits);
                damage.write(DamageEvent::new(entity, Hit::from_status(credit, t.status, t.kind, t.amount)));
            }
            for m in report.mends {
                let by = m.source.and_then(Entity::try_from_bits);
                heals.write(Heal { target: entity, amount: m.amount, by, status: Some(m.status) });
            }
            for status in report.expired {
                events.write(StatusEvent::Expired { target: entity, status });
            }
        }
    }
}

/// Spends the turn of whoever holds it while a status that
/// [`holds`](atglyph_rules::StatusDef::holds) or
/// [`sleeps`](atglyph_rules::StatusDef::sleeps) is on them, as a wait costs,
/// before its mind or its player is asked.
///
/// In [`DecideSet::Sense`](crate::plugin::DecideSet::Sense), where work
/// claims a worker's turns, and for the player as for anyone. The decision
/// is claimed, so its mind is not asked what to do, though what it senses
/// still runs as on any turn it holds, its sight recast and its noticing
/// rolled. The turn is spent here rather than by an intent resolved later,
/// so a key the player pressed finds it spent and comes to nothing
/// whichever stage would have resolved it. Taken whatever claimed the
/// decision first, so a worker caught at its work loses the turn rather
/// than working it.
///
/// A sleeper senses less than that. Its sight is recast, but it notices
/// only within [`SLEEPER_REACH`](atglyph_rules::SLEEPER_REACH), by its chance
/// and never by its certain radius, so a hider can stand over it; what it
/// notices there, or a foe that does not hide in its sight there, wakes it
/// ([`wake_on_sight`]). It hears what anyone hears, and a noise it hears
/// wakes it; harm landing on it wakes it whenever it lands.
pub(crate) fn pass_held_turns(
    mut acting: ResMut<Acting>,
    mut spent: MessageWriter<ActionDone>,
    registries: Option<Res<Registries>>,
    holders: Query<(Entity, &ActiveStatuses), With<MyTurn>>,
) {
    let Some(registries) = registries.as_deref() else { return };
    for (actor, statuses) in &holders {
        if statuses.taking_turns(&registries.statuses).is_none() {
            continue;
        }
        acting.claim_decision(actor);
        if acting.claim_action(actor) {
            spent.write(ActionDone { actor, cost: BASE_ACTION_COST });
        }
    }
}

/// Who is asleep: holding a status that [`sleeps`](atglyph_rules::StatusDef::sleeps).
///
/// Read off the statuses as they are now rather than kept in a marker
/// component beside them, so a sleeper woken by the blow of one pass is
/// awake to the blow of the next, whichever stage of a pass asks. Nobody is
/// asleep in a game with no [`Registries`].
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Sleepers<'w, 's> {
    registries: Option<Res<'w, Registries>>,
    statuses: Query<'w, 's, &'static ActiveStatuses>,
}

impl Sleepers<'_, '_> {
    /// Whether `actor` is asleep.
    pub(crate) fn asleep(&self, actor: Entity) -> bool {
        let Some(registries) = self.registries.as_deref() else { return false };
        self.statuses.get(actor).is_ok_and(|s| s.sleeping(&registries.statuses).is_some())
    }
}

/// Wakes `target`: cures every status on it that sleeps and reports each.
fn wake(target: Entity, statuses: &mut Statuses, stats: &mut Stats, registries: &Registries, events: &mut MessageWriter<StatusEvent>) {
    for status in statuses.wake(&registries.statuses, stats) {
        events.write(StatusEvent::Cured { target, status });
    }
}

/// Wakes whoever harm landed on this pass: a blow, a shot, a throw, a burn
/// or a status ticking, one that armor stopped at nothing included, since
/// it was felt. A miss is not here: it landed nothing, and the noise of the
/// swing is the noise plugin's to carry.
///
/// In [`TurnSet::React`](crate::plugin::TurnSet::React), where a turn's
/// consequences land, so the blow that wakes a sleeper was rolled on a
/// sleeper and lands as on the unaware, and the sleeper's next turn is its
/// own. Its statuses are cured here rather than by a [`RemoveStatus`], which would
/// not be read until the next pass's statuses resolve.
///
/// Two harms wake nobody. A sleeping status's own tick: a sleep that also
/// bites would otherwise cure itself the first turn it bit. And harm that
/// lands in the pass the sleep was put on: a dart that wounds and sends to
/// sleep is one landing, its sleep resolved before its harm, and woken by
/// its own wound it never slept at all. Who was put to sleep this pass is
/// [`FellAsleep`], taken here so the next pass starts it empty.
pub(crate) fn wake_on_harm(
    mut dealt: MessageReader<DamageDealt>,
    mut fell: ResMut<FellAsleep>,
    mut events: MessageWriter<StatusEvent>,
    registries: Option<Res<Registries>>,
    mut actors: Query<(&mut ActiveStatuses, &mut StatBlock)>,
) {
    let fresh = std::mem::take(&mut fell.0);
    let Some(registries) = registries.as_deref() else { return };
    let sleeps = |status: StatusId| registries.statuses.get(status).sleeps;
    for d in dealt.read() {
        if !atglyph_rules::status::wakes_from_harm(d.hit.status.is_some_and(sleeps), fresh.contains(&d.target)) {
            continue;
        }
        let Ok((mut statuses, mut stats)) = actors.get_mut(d.target) else { continue };
        wake(d.target, &mut statuses.0, &mut stats.0, registries, &mut events);
    }
}

/// Wakes whoever heard a noise this pass, which is a noise that reached
/// it with at least its [`Hearing`](crate::noise::Hearing) threshold left
/// on it; a sleeper with no hearing sleeps through any noise. It wakes
/// with the sound in mind, as anyone who heard it, and goes to look.
///
/// Added by the [`NoisePlugin`](crate::noise::NoisePlugin), whose message
/// it reads and which only it may register, in
/// [`TurnSet::Listen`](crate::plugin::TurnSet::Listen) after the noise is
/// heard; cured directly for the reason [`wake_on_harm`] is. Nobody is
/// asleep in a game with noise and no statuses.
pub(crate) fn wake_on_noise(
    mut heard: MessageReader<crate::noise::NoiseHeard>,
    mut events: MessageWriter<StatusEvent>,
    registries: Option<Res<Registries>>,
    mut actors: Query<(&mut ActiveStatuses, &mut StatBlock)>,
) {
    let Some(registries) = registries else { return };
    for h in heard.read() {
        let Ok((mut statuses, mut stats)) = actors.get_mut(h.listener) else { continue };
        wake(h.listener, &mut statuses.0, &mut stats.0, &registries, &mut events);
    }
}

/// A sleeper holding the turn.
type Sleeper = (
    Entity,
    &'static Position,
    Option<&'static Faction>,
    Option<&'static Viewshed>,
    Option<&'static crate::stealth::Aware>,
    Option<&'static OnMap>,
    &'static mut ActiveStatuses,
    &'static mut StatBlock,
);
/// Anyone it might wake to.
type Stirring = (Entity, &'static Position, &'static Faction, Option<&'static OnMap>, Has<crate::stealth::Stealth>, Has<crate::stealth::Unseen>);

/// Wakes the sleeper holding the turn if it notices a foe within
/// [`SLEEPER_REACH`](atglyph_rules::SLEEPER_REACH).
///
/// A foe is whoever the faction matrix calls hostile, standing on its map
/// within that reach, in its sight and not unseen. One that hides from a
/// sleeper that keeps an [`Aware`](crate::stealth::Aware), while stealth
/// runs, wakes it only once it has been noticed, which
/// [`update_awareness`](crate::stealth::update_awareness) rolls for a
/// sleeper by chance alone and only within the reach; any other it wakes to
/// on sight, as it would see them awake. A sleeper with no sight wakes only
/// to harm and noise.
///
/// In [`DecideSet::Notice`](crate::plugin::DecideSet::Notice), after the
/// noticing, on the sleeper's own turn, which
/// [`pass_held_turns`] has already spent: waking takes the turn it happens
/// in, and the next is its own.
pub(crate) fn wake_on_sight(
    mut sleepers: Query<Sleeper, With<MyTurn>>,
    others: Query<Stirring, (With<Actor>, Without<Dead>)>,
    rules: Option<Res<CombatRules>>,
    stealth: crate::stealth::StealthRunning,
    registries: Option<Res<Registries>>,
    mut events: MessageWriter<StatusEvent>,
) {
    let (Some(rules), Some(registries)) = (rules.as_deref(), registries.as_deref()) else { return };
    for (sleeper, pos, faction, sight, aware, on, mut statuses, mut stats) in &mut sleepers {
        let (Some(mine), Some(sight)) = (faction, sight) else { continue };
        if statuses.sleeping(&registries.statuses).is_none() {
            continue;
        }
        let map = on.map(|m| m.0).unwrap_or(MapId::ZERO);
        let stirs = others.iter().any(|(other, at, theirs, other_on, hides, unseen)| {
            let close = other != sleeper
                && !unseen
                && other_on.map(|m| m.0).unwrap_or(MapId::ZERO) == map
                && atglyph_core::geometry::chebyshev(pos.0, at.0) <= atglyph_rules::SLEEPER_REACH
                && stance(Some(&rules.factions), Some(mine.0), Some(theirs.0)) == Some(Relation::Hostile)
                && sight.can_see(at.0);
            let noticed = match aware {
                Some(aware) if hides && stealth.get() => aware.knows(other),
                _ => true,
            };
            close && noticed
        });
        if stirs {
            wake(sleeper, &mut statuses.0, &mut stats.0, registries, &mut events);
        }
    }
}

/// The side a holder fought for before a status put it on another, kept
/// while any status naming a side is on it and given back the pass the
/// last ends.
///
/// Public, and saved, because what a holder goes back to is nowhere else
/// once its [`Faction`] is the status's: a game writing down which side a
/// body is on reads this in place of the faction while it is here.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SideWas(pub Faction);

/// Keeps every holder of a status that names a
/// [`side`](atglyph_rules::StatusDef::side) on that side, and puts each back on
/// the side it was on, its [`SideWas`], the pass the last such status
/// ends.
///
/// Turned once, when the status comes on: a holder some other rule moves
/// while it lasts is left where that rule put it until the status ends.
/// The faction matrix is read live by every mind and every blow, so the
/// side is the whole of it; who is hostile to whom stays the game's. A
/// holder put back by a save, its status asked for again, is turned again
/// here from whatever side it was given back.
pub(crate) fn keep_sides(
    mut commands: Commands,
    registries: Option<Res<Registries>>,
    mut bodies: Query<(Entity, &ActiveStatuses, &mut Faction, Option<&SideWas>)>,
) {
    let Some(registries) = registries.as_deref() else { return };
    for (body, held, mut faction, was) in &mut bodies {
        let Some((side, kept)) = atglyph_rules::status::side_change(held.side(&registries.statuses), faction.0, was.map(|w| w.0.0)) else { continue };
        *faction = Faction(side);
        match kept {
            Some(kept) => commands.entity(body).insert(SideWas(Faction(kept))),
            None => commands.entity(body).remove::<SideWas>(),
        };
    }
}

/// What a holder resists because of the statuses on it, kept by
/// [`keep_status_resists`] and added to its own and its gear's by
/// [`Loadout::resistances`](crate::combat::Loadout::resistances).
///
/// Its own component rather than written into the holder's
/// [`Resists`](crate::combat::Resists), so taking it off is exact whatever
/// else changed those. Kept once a pass rather than read live from the
/// statuses where a blow is weighed: a status takes hold where the pass's
/// consequences land, after its damage, so a draught and a burn landed in
/// one pass are weighed as things stood before it, and the last tick of a
/// burn is resisted by a ward running out in the same pass.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub(crate) struct StatusResists(pub Resistances);

/// Keeps every holder's [`StatusResists`] at what its statuses
/// [`resist`](atglyph_rules::StatusDef::resists), and takes it off once they
/// resist nothing. Leaves alone what already reads right.
pub(crate) fn keep_status_resists(
    mut commands: Commands,
    registries: Option<Res<Registries>>,
    bodies: Query<(Entity, &ActiveStatuses, Option<&StatusResists>)>,
) {
    let Some(registries) = registries.as_deref() else { return };
    for (body, held, kept) in &bodies {
        let lent = held.resistances(&registries.statuses);
        let none = lent == Resistances::new();
        match kept {
            Some(_) if none => drop(commands.entity(body).remove::<StatusResists>()),
            Some(kept) if kept.0 == lent => {}
            None if none => {}
            _ => drop(commands.entity(body).insert(StatusResists(lent))),
        }
    }
}

/// What an actor's own kind changes about its stats while it holds a
/// status: for each status, the changes, kept by [`keep_while_holding`].
///
/// The kind's rule rather than the status's, which is why it is on the
/// actor and not on the [`StatusDef`](atglyph_rules::StatusDef): burning halts
/// one kind's mending and nobody else's. Put on from
/// [`ActorParts::while_holding`](crate::parts::ActorParts::while_holding),
/// or by hand.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub(crate) struct WhileHolding(pub Vec<(StatusId, Vec<(StatId, Op)>)>);

/// Puts each [`WhileHolding`] change on its actor's stats while it holds
/// the status, under [`ModifierSource::Holding`], and takes them off the pass it
/// no longer does.
///
/// Read off the stats as they stand rather than a marker beside them, so
/// a status put back by a save puts its changes back with nothing else
/// saved. Only the living: a body that keeps its kind's rule is not kept.
pub(crate) fn keep_while_holding(mut bodies: Query<(&WhileHolding, &ActiveStatuses, &mut StatBlock), With<Actor>>) {
    for (rule, held, mut stats) in &mut bodies {
        for (status, changes) in &rule.0 {
            let source = ModifierSource::Holding(*status);
            let on = stats.0.modifiers().iter().any(|m| m.source == source);
            match (held.has(*status), on) {
                (true, false) => {
                    for (stat, op) in changes {
                        stats.0.add(Modifier::new(*stat, *op, source));
                    }
                }
                (false, true) => drop(stats.0.remove_source(source)),
                _ => {}
            }
        }
    }
}

/// The draws a stumbling holder's steps are turned by: a stream of the
/// run's own, so a stumble never shifts a blow's roll or a cloud's spread.
pub(crate) type StumbleRng = crate::seed::RunRng<StumbleDomain>;

/// The domain [`StumbleRng`] draws under.
pub(crate) struct StumbleDomain;

impl crate::seed::StreamDomain for StumbleDomain {
    const DOMAIN: atglyph_core::SeedDomain = crate::seed::STUMBLE;
}

/// Turns a holder's chosen step or bump a random way among those open to
/// it, as often as the keenest status on it that
/// [`stumbles`](atglyph_rules::StatusDef::stumbles) says, the player's walk and
/// a mind's step alike: a walk into someone it did not mean to strike
/// strikes them.
///
/// Among the open ways, cells that can be walked on whoever stands there:
/// a way into a wall would be refused, which for the player costs nothing
/// and would only ask again. A holder walled in on every side steps as it
/// meant to, and no draw is made for it; nor is the percent rolled at a
/// hundred, so a holder that always stumbles spends one draw a step.
///
/// In [`DecideSet::Statuses`](crate::plugin::DecideSet::Statuses), after
/// every mind and the game have decided and before anything resolves, and
/// by rewriting the intent where it stands rather than replacing it, so
/// nothing else that reads it reads the old way. A game whose statuses
/// never stumble draws nothing.
pub(crate) fn stumble(
    mut steps: MessageMutator<Intent<Step>>,
    mut walks: MessageMutator<Intent<crate::bump::Bump>>,
    bodies: Query<(&ActiveStatuses, &Position)>,
    registries: Option<Res<Registries>>,
    map: Res<WorldMap>,
    mut rng: Option<ResMut<StumbleRng>>,
) {
    let Some(registries) = registries.as_deref() else { return };
    let mut way = |who: Entity| {
        let (held, at) = bodies.get(who).ok()?;
        let pct = held.stumbles(&registries.statuses)?;
        let open: Vec<Direction> = Direction::ALL.into_iter().filter(|d| map.is_walkable(at.0 + d.offset())).collect();
        if open.is_empty() {
            return None;
        }
        let rng = &mut rng.as_deref_mut().expect("a status that stumbles draws from the run's seed, which CorePlugin asks for").0;
        atglyph_rules::status::stumble(Some(pct), &open, rng)
    };
    for step in steps.read() {
        if let Some(turned) = way(step.actor) {
            step.action.0 = turned;
        }
    }
    for walk in walks.read() {
        if let Some(turned) = way(walk.actor) {
            walk.action.0 = turned;
        }
    }
}

/// Whether any registered status stumbles: what makes the run's seed a
/// requirement of the statuses, which otherwise draw nothing.
fn any_stumbles(world: &World) -> bool {
    world.get_resource::<Registries>().is_some_and(|r| r.statuses.iter().any(|(_, def)| def.stumbles.is_some()))
}

/// Statuses: what is afflicted, what it costs each turn, what cures it,
/// as [`CorePlugin`](crate::plugin::CorePlugin) registers them.
///
/// Ticks go through the damage pipeline, and so land only where the game
/// added combat to apply them; the messages are registered here either
/// way, so a game without combat has statuses that hold and sleep and
/// harm nobody. A sleeper wakes to harm and to a foe close by with this
/// alone, and to a noise where the game added the
/// [`NoisePlugin`](crate::noise::NoisePlugin) as well, which wakes it.
///
/// Every [`Actor`] is given an empty [`ActiveStatuses`] and [`StatBlock`] the
/// moment it is spawned, so a monster spawned without them still takes a
/// status rather than silently shrugging it off.
pub(crate) fn build(app: &mut App) {
    use crate::plugin::{DecideSet, ResolveSet, Turn};
    // Before anything spawns, which is when Bevy allows it: `CorePlugin`
    // is the first plugin a game adds.
    app.register_required_components::<Actor, ActiveStatuses>();
    app.register_required_components::<Actor, StatBlock>();
    app.add_message::<ApplyStatus>()
        .add_message::<RemoveStatus>()
        .add_message::<StatusEvent>()
        // What a tick deals and mends, and the harm that wakes a sleeper:
        // combat's, which a game without it never resolves.
        .add_message::<DamageEvent>()
        .add_message::<Heal>()
        .add_message::<DamageDealt>()
        .init_resource::<FellAsleep>()
        .add_systems(Turn, pass_held_turns.in_set(DecideSet::Sense))
        // After the noticing, which is stealth's and runs only if the
        // game added it; with no stealth there is nothing to be after.
        .add_systems(Turn, wake_on_sight.in_set(DecideSet::Notice).after(crate::stealth::update_awareness))
        .add_systems(Turn, (resolve_afflictions, tick_statuses).chain().in_set(ResolveSet::Effects))
        .add_systems(Turn, wake_on_harm.in_set(crate::plugin::ReactSet::Settle))
        .add_systems(Turn, (keep_sides, keep_status_resists, keep_while_holding).chain().in_set(crate::plugin::ReactSet::Statuses))
        .add_systems(Turn, stumble.in_set(DecideSet::Statuses));
    // Derived whenever a seed is given, and asked for only where a status
    // stumbles: a game whose statuses never draw needs no seed for them.
    crate::seed::derive_stream::<StumbleRng>(app);
    crate::plugin::needs_when::<crate::seed::Seed>(app, "CorePlugin's statuses", "`Seed(RunSeed(n))`, which a status that stumbles draws from", any_stumbles);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::EngineApp;
    use crate::bump::Bump;
    use crate::combat::{Armor, Attack, CombatRules, DamageStages, Faction, Health, MeleeAttack};
    use crate::components::{Blocks, MyTurn, Player, Position, RevealsMap, Viewshed};
    use crate::minds::{Mind, Perception};
    use crate::noise::{Hearing, MakeNoise, NoisePlugin, NoiseRules, Sounds};
    use crate::state::EngineState;
    use crate::stealth::{Aware, Notice, Stealth, StealthPlugin};
    use crate::testing::headless_app;
    use crate::turn::Wait;
    use crate::turn::{Intent, Turns};
    use crate::work::{WorkKinds, Working};
    use atglyph_core::DiceRoll;
    use atglyph_core::turn::BASE_ACTION_COST;
    use atglyph_core::{Direction, Point, RunSeed};
    use atglyph_rules::ai::awareness::NoticeStats;
    use atglyph_rules::ai::hearing::HearingStats;
    use atglyph_rules::ai::{Brain, Decision, Tactic, TacticCtx};
    use atglyph_rules::damage::{DamageKind, SubtractArmor};
    use atglyph_rules::faction::FactionDef;
    use atglyph_rules::work::Work;
    use atglyph_rules::{Op, Registry, Stacking, StatDef, StatusDef, UNTIL_CURED};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[derive(Resource, Default)]
    struct Heard(Vec<StatusEvent>);

    fn hear(mut events: MessageReader<StatusEvent>, mut heard: ResMut<Heard>) {
        heard.0.extend(events.read().copied());
    }

    #[test]
    fn a_status_ticks_damage_each_turn_modifies_stats_and_expires() {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::combat::CombatPlugin));
        let start = crate::testing::ground(&mut app);
        let kinds = Registry::from_defs(vec![DamageKind::new("venom")]).unwrap();
        let venom_kind = kinds.expect("venom");
        let facs = Registry::from_defs(vec![FactionDef::new("us")]).unwrap();
        let stats = Registry::from_defs(vec![StatDef::new("armor", 0)]).unwrap();
        let armor_stat = stats.expect("armor");
        let defs = Registry::from_defs(vec![
            StatusDef::new("venom").ticks(venom_kind, 2).stacking(Stacking::Refresh),
            StatusDef::new("hearty").modifies(armor_stat, Op::Add(3)),
        ])
        .unwrap();
        let (venom, hearty) = (defs.expect("venom"), defs.expect("hearty"));
        app.insert_resource(CombatRules::new());
        app.insert_resource(DamageStages(vec![Box::new(SubtractArmor)]));
        app.insert_resource(crate::seed::Seed(RunSeed(5)));
        app.insert_resource(Registries { damage_kinds: kinds, factions: facs, stats: stats.clone(), statuses: defs, ..Default::default() });
        let player = app
            .world_mut()
            .spawn((
                Actor,
                Player,
                Blocks,
                Position(start),
                Viewshed::new(6),
                RevealsMap,
                Health::full(30),
                Armor(0),
                // No `ActiveStatuses` and no `StatBlock`: the plugin gives every
                // actor both, and the statuses below must still land.
            ))
            .id();
        // Every status event, recorded by a reader: when a headless app
        // swaps its message buffers depends on wall time, so peeking at the
        // buffer after an update can miss a message that was written.
        app.init_resource::<Heard>().add_systems(PostUpdate, hear);
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        app.world_mut().write_message(ApplyStatus { target: player, status: venom, turns: 3, by: None, held_by: None });
        app.world_mut().write_message(ApplyStatus { target: player, status: hearty, turns: 2, by: None, held_by: None });
        app.update();
        {
            let w = app.world();
            assert!(w.get::<ActiveStatuses>(player).unwrap().has(venom));
            assert_eq!(w.get::<StatBlock>(player).unwrap().value(armor_stat, &stats), 3, "hearty's modifier landed");
            assert_eq!(w.get::<Health>(player).unwrap().current, 30, "nothing ticks until a turn passes");
        }
        // Each wait is a whole turn: venom deals 2 for three turns, hearty
        // lasts two.
        for turn in 1..=3u32 {
            assert!(app.world().get::<MyTurn>(player).is_some());
            app.world_mut().write_message(Intent::new(player, Wait));
            app.update();
            assert_eq!(app.world().resource::<Turns>().turn_number(), turn);
            assert_eq!(app.world().get::<Health>(player).unwrap().current, 30 - 2 * turn as i32, "turn {turn}");
        }
        let w = app.world();
        assert!(!w.get::<ActiveStatuses>(player).unwrap().has(venom), "venom ran out");
        assert!(!w.get::<ActiveStatuses>(player).unwrap().has(hearty));
        assert_eq!(w.get::<StatBlock>(player).unwrap().value(armor_stat, &stats), 0, "hearty's modifier left with it");
        let expired = &w.resource::<Heard>().0;
        assert!(expired.contains(&StatusEvent::Expired { target: player, status: venom }), "{expired:?}");
    }

    /// A status put on is `Applied` when it begins and `Renewed` when it is
    /// put on again while still on, refreshed longer or extended; a refresh
    /// no longer than what is left changes nothing and says nothing. So a
    /// reader tells a status beginning from one a cloud keeps up.
    #[test]
    fn a_status_begun_is_applied_one_kept_up_is_renewed_and_one_that_changed_nothing_is_neither() {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::combat::CombatPlugin));
        let start = crate::testing::ground(&mut app);
        crate::testing::two_sides(&mut app);
        let defs = Registry::from_defs(vec![StatusDef::new("dazed"), StatusDef::new("bleeding").stacking(Stacking::Extend)]).unwrap();
        let (dazed, bleeding) = (defs.expect("dazed"), defs.expect("bleeding"));
        app.world_mut().resource_mut::<Registries>().statuses = defs;
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(6), RevealsMap, Health::full(30))).id();
        app.init_resource::<Heard>().add_systems(PostUpdate, hear);
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        for (status, turns) in [(dazed, 3), (dazed, 5), (dazed, 2), (bleeding, 2), (bleeding, 2)] {
            app.world_mut().write_message(ApplyStatus { target: player, status, turns, by: None, held_by: None });
            app.update();
        }
        let target = player;
        assert_eq!(
            app.world().resource::<Heard>().0,
            [
                StatusEvent::Applied { target, status: dazed },
                StatusEvent::Renewed { target, status: dazed },
                StatusEvent::Applied { target, status: bleeding },
                StatusEvent::Renewed { target, status: bleeding },
            ],
            "begun, refreshed longer, refreshed shorter for nothing, begun, extended"
        );
    }

    /// A status that mends restores its amount every whole turn, to no
    /// more than the most its holder has, and is reported as a heal by
    /// whoever applied it and never as damage dealt: there is no kind for
    /// armor or a resistance to be asked about.
    #[test]
    fn a_status_that_mends_restores_health_each_turn_and_stops_at_the_most() {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::combat::CombatPlugin));
        let start = crate::testing::ground(&mut app);
        let facs = Registry::from_defs(vec![FactionDef::new("us")]).unwrap();
        let defs = Registry::from_defs(vec![StatusDef::new("mending").mends(4).boon()]).unwrap();
        let mending = defs.expect("mending");
        app.insert_resource(CombatRules::new());
        app.insert_resource(crate::seed::Seed(RunSeed(5)));
        app.insert_resource(Registries { factions: facs, statuses: defs, ..Default::default() });
        let player =
            app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(6), RevealsMap, Health { current: 21, max: 30 }, Armor(50))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        app.world_mut().write_message(ApplyStatus { target: player, status: mending, turns: 3, by: Some(player), held_by: None });
        app.update();
        let mut healed = Vec::new();
        for want in [25, 29, 30] {
            app.world_mut().write_message(Intent::new(player, Wait));
            app.update();
            assert_eq!(app.world().get::<Health>(player).unwrap().current, want);
            healed.extend(app.world_mut().resource_mut::<Messages<crate::combat::Healed>>().drain());
        }
        assert_eq!(healed.iter().map(|h| (h.restored, h.by, h.status)).collect::<Vec<_>>(), [4, 4, 1].map(|n| (n, Some(player), Some(mending))).to_vec());
        assert!(app.world_mut().resource_mut::<Messages<crate::combat::DamageDealt>>().drain().next().is_none(), "nothing was dealt");
    }

    /// A world with minds and statuses in it, and `snared`, a status that
    /// holds, the only one; whatever else a test adds, added before the app
    /// first runs; and a player to pass the turns, holding the first.
    fn held_arena(extra: impl FnOnce(&mut App)) -> (App, Point, Entity, StatusId) {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::combat::CombatPlugin, crate::minds::MindsPlugin));
        extra(&mut app);
        let start = crate::testing::ground(&mut app);
        crate::testing::two_sides(&mut app);
        let statuses = Registry::from_defs(vec![StatusDef::new("snared").skips_turns()]).unwrap();
        let snared = statuses.expect("snared");
        app.world_mut().resource_mut::<Registries>().statuses = statuses;
        app.init_resource::<Spent>().add_systems(crate::plugin::Turn, record_spent.in_set(crate::plugin::TurnSet::Record));
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), Health::full(30))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        (app, start, player, snared)
    }

    /// Every turn spent, by whom and for how much, in the order spent.
    #[derive(Resource, Default)]
    struct Spent(Vec<(Entity, u32)>);

    fn record_spent(mut done: MessageReader<crate::turn::ActionDone>, mut spent: ResMut<Spent>) {
        spent.0.extend(done.read().map(|d| (d.actor, d.cost)));
    }

    /// What `who` spent its turns on, so far.
    fn spent_by(app: &App, who: Entity) -> Vec<u32> {
        app.world().resource::<Spent>().0.iter().filter(|(actor, _)| *actor == who).map(|(_, cost)| *cost).collect()
    }

    /// One player turn: a wait, if the player holds the turn, and a frame.
    fn pass(app: &mut App, player: Entity) {
        if app.world().get::<MyTurn>(player).is_some() {
            app.world_mut().write_message(Intent::new(player, Wait));
        }
        app.update();
    }

    /// Steps south whenever it is asked, and counts each asking, so a test
    /// sees both whether the mind was asked and whether it acted.
    struct Walk(Arc<AtomicU32>);

    impl Tactic<Entity> for Walk {
        fn name(&self) -> &'static str {
            "walk"
        }
        fn evaluate(&self, ctx: &mut TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Some(Decision::Step(ctx.snapshot.me.pos.offset(0, 1)))
        }
    }

    /// An actor three cells from the player that walks whenever its mind
    /// is asked, and the count of the askings.
    fn walker(app: &mut App, start: Point) -> (Entity, Arc<AtomicU32>) {
        let asked = Arc::new(AtomicU32::new(0));
        let brain = Arc::new(Brain::new().then(Walk(asked.clone())));
        let walker = app.world_mut().spawn((Actor, Blocks, Position(start.offset(3, 0)), Health::full(5), Perception(6), Mind(brain))).id();
        (walker, asked)
    }

    /// Put on before the walker's first turn, a snare of three turns takes
    /// exactly three of them, each spent as a wait costs, with the mind
    /// never asked; the turn after it runs out is the walker's own.
    #[test]
    fn a_held_actors_turns_pass_as_waits_and_its_mind_is_not_asked_until_the_status_runs_out() {
        let (mut app, start, player, snared) = held_arena(|_| {});
        let (walker, asked) = walker(&mut app, start);
        app.world_mut().write_message(ApplyStatus { target: walker, status: snared, turns: 3, by: None, held_by: None });
        for _ in 0..3 {
            pass(&mut app, player);
        }
        assert!(!app.world().get::<ActiveStatuses>(walker).unwrap().has(snared), "the snare has run out");
        assert_eq!(spent_by(&app, walker), vec![BASE_ACTION_COST; 3], "three turns lost, each spent as a wait");
        assert_eq!(asked.load(Ordering::Relaxed), 0, "and its mind asked on none of them");
        assert_eq!(app.world().get::<Position>(walker).unwrap().0, start.offset(3, 0), "nor did it move");
        pass(&mut app, player);
        assert_eq!(asked.load(Ordering::Relaxed), 1, "free, it is asked again");
        assert_eq!(app.world().get::<Position>(walker).unwrap().0, start.offset(3, 1), "and walks");
    }

    /// The one way a key reaches a player who is held: the status landed
    /// while the player held its turn, after the decide stage had passed
    /// it by, so the player still holds the turn when the next frame reads
    /// the keys. The key is spent with that turn, and the turns after it
    /// pass with no key at all.
    #[test]
    fn a_held_players_keys_do_nothing_and_its_turns_pass_until_the_status_runs_out() {
        let (mut app, start, player, snared) = held_arena(|_| {});
        app.world_mut().write_message(ApplyStatus { target: player, status: snared, turns: 2, by: None, held_by: None });
        app.update();
        assert!(app.world().get::<ActiveStatuses>(player).unwrap().has(snared), "caught");
        assert!(app.world().get::<MyTurn>(player).is_some(), "holding its turn");
        app.world_mut().write_message(Intent::new(player, Bump(Direction::East)));
        app.update();
        assert_eq!(app.world().get::<Position>(player).unwrap().0, start, "the key did nothing");
        assert!(!app.world().get::<ActiveStatuses>(player).unwrap().has(snared), "the turns passed until the snare ran out");
        assert_eq!(spent_by(&app, player), vec![BASE_ACTION_COST; 3], "the turn it was caught in and the two the snare lasted, each a wait");
        assert!(app.world().get::<MyTurn>(player).is_some(), "free, it holds a turn again");
        app.world_mut().write_message(Intent::new(player, Bump(Direction::East)));
        app.update();
        assert_eq!(app.world().get::<Position>(player).unwrap().0, start.offset(1, 0), "and its keys work again");
    }

    #[test]
    fn a_status_that_holds_until_cured_frees_its_holder_on_the_turn_after_the_cure() {
        let (mut app, start, player, snared) = held_arena(|_| {});
        let (walker, asked) = walker(&mut app, start);
        app.world_mut().write_message(ApplyStatus { target: walker, status: snared, turns: UNTIL_CURED, by: None, held_by: None });
        for _ in 0..3 {
            pass(&mut app, player);
        }
        assert_eq!(asked.load(Ordering::Relaxed), 0, "held for as long as it is on");
        app.world_mut().write_message(RemoveStatus { target: walker, status: snared });
        pass(&mut app, player);
        assert_eq!(asked.load(Ordering::Relaxed), 1, "cured, its next turn is its own");
        assert_eq!(app.world().get::<Position>(walker).unwrap().0, start.offset(3, 1));
    }

    /// The hold takes the turn whatever claimed the decision first, so a
    /// worker caught at its work loses the turn rather than working it.
    #[test]
    fn a_worker_caught_by_a_status_that_holds_does_no_work_on_the_turns_it_loses() {
        let (mut app, start, player, snared) = held_arena(|app| {
            app.add_work("mending");
        });
        let kind = app.world().resource::<WorkKinds>().get("mending").expect("declared");
        let worker = app.world_mut().spawn((Actor, Blocks, Position(start.offset(3, 0)), Health::full(5), Working(Work::new(kind, 4)))).id();
        app.world_mut().write_message(ApplyStatus { target: worker, status: snared, turns: 3, by: None, held_by: None });
        for _ in 0..3 {
            pass(&mut app, player);
        }
        assert_eq!(app.world().get::<Working>(worker).map(|w| w.0.done), Some(0), "no work done on the turns it lost");
        for _ in 0..4 {
            pass(&mut app, player);
        }
        assert!(app.world().get::<Working>(worker).is_none(), "free, it works its four turns and is done");
    }

    /// A world for sleep: `asleep`, a status that sleeps, and `snared`, one
    /// that only holds; the player on our side with a blow of two, in a
    /// game whose blows on the unaware land at three times; whatever
    /// `extra` adds before the app first runs; the player holding the turn.
    struct Bedroom {
        app: App,
        start: Point,
        player: Entity,
        theirs: atglyph_rules::FactionId,
        asleep: StatusId,
        snared: StatusId,
        /// A sleep that bites every turn it lasts.
        drowsing: StatusId,
        kind: atglyph_rules::damage::DamageKindId,
    }

    fn bedroom(extra: impl FnOnce(&mut App)) -> Bedroom {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::combat::CombatPlugin, crate::minds::MindsPlugin));
        extra(&mut app);
        let start = crate::testing::ground(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        app.world_mut().resource_mut::<CombatRules>().unaware = Some(300);
        let defs = vec![StatusDef::new("asleep").sleeps(), StatusDef::new("snared").skips_turns(), StatusDef::new("drowsing").sleeps().ticks(sides.kind, 1)];
        let statuses = Registry::from_defs(defs).unwrap();
        let (asleep, snared, drowsing) = (statuses.expect("asleep"), statuses.expect("snared"), statuses.expect("drowsing"));
        app.world_mut().resource_mut::<Registries>().statuses = statuses;
        app.init_resource::<Spent>().add_systems(crate::plugin::Turn, record_spent.in_set(crate::plugin::TurnSet::Record));
        let blow = MeleeAttack::new(sides.kind, DiceRoll::flat(2));
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), Health::full(30), Faction(sides.ours), blow)).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        Bedroom { app, start, player, theirs: sides.theirs, asleep, snared, drowsing, kind: sides.kind }
    }

    impl Bedroom {
        /// A foe `dx` cells east of the player that waits whenever its mind
        /// is asked, put to sleep until something wakes it; and the count
        /// of the askings. Asleep from the next pass on.
        fn sleeper(&mut self, dx: i32) -> (Entity, Arc<AtomicU32>) {
            let asked = Arc::new(AtomicU32::new(0));
            let brain = Arc::new(Brain::new().then(Stand(asked.clone())));
            let at = Position(self.start.offset(dx, 0));
            let sleeper = self.app.world_mut().spawn((Actor, Blocks, at, Health::full(20), Perception(6), Faction(self.theirs), Mind(brain))).id();
            self.app.world_mut().write_message(ApplyStatus { target: sleeper, status: self.asleep, turns: UNTIL_CURED, by: None, held_by: None });
            (sleeper, asked)
        }

        fn asleep(&self, who: Entity) -> bool {
            self.app.world().get::<ActiveStatuses>(who).unwrap().has(self.asleep)
        }

        fn health(&self, who: Entity) -> i32 {
            self.app.world().get::<Health>(who).unwrap().current
        }

        fn strike(&mut self, target: Entity) -> i32 {
            let before = self.health(target);
            self.app.world_mut().write_message(Intent::new(self.player, Attack(target)));
            self.app.update();
            before - self.health(target)
        }

        /// A noise of `loudness` where the player stands, made by nobody,
        /// heard in the pass the player waits.
        fn noise(&mut self, loudness: i32) {
            let at = self.start;
            self.app.world_mut().write_message(MakeNoise { at, loudness, sound: Sounds::STRIKE, maker: None });
            pass(&mut self.app, self.player);
        }
    }

    /// Waits whenever it is asked, and counts each asking.
    struct Stand(Arc<AtomicU32>);

    impl Tactic<Entity> for Stand {
        fn name(&self) -> &'static str {
            "stand"
        }
        fn evaluate(&self, _: &mut TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Some(Decision::Wait)
        }
    }

    /// Quiet, a foe three cells off in plain sight, and nothing else: the
    /// sleeper's turns pass as waits, its mind never asked, for as long as
    /// nothing wakes it.
    #[test]
    fn a_sleepers_turns_pass_held_and_it_sleeps_on_while_nothing_wakes_it() {
        let mut room = bedroom(|_| {});
        let (sleeper, asked) = room.sleeper(3);
        for _ in 0..5 {
            pass(&mut room.app, room.player);
        }
        assert!(room.asleep(sleeper), "a foe three cells off does not wake it");
        assert_eq!(spent_by(&room.app, sleeper), vec![BASE_ACTION_COST; 5], "five turns, each spent as a wait");
        assert_eq!(asked.load(Ordering::Relaxed), 0, "and its mind asked on none of them");
    }

    /// Hearing with every engine noise silent, so the only sound is the
    /// one a test makes.
    fn with_hearing(app: &mut App) {
        app.add_plugins(NoisePlugin::new(NoiseRules { step: 0, strike: 0, door: 0, landing: 0, door_muffle: 2 }));
    }

    /// A sleeper four cells off that hears a sound with three steps of it
    /// left: one of six arrives with two and is not heard, one of eight
    /// arrives with four and wakes it, and its next turn is its own.
    #[test]
    fn a_noise_heard_wakes_a_sleeper_and_one_too_quiet_to_hear_does_not() {
        let mut room = bedroom(with_hearing);
        let (sleeper, asked) = room.sleeper(4);
        room.app.world_mut().entity_mut(sleeper).insert(Hearing(HearingStats { threshold: 3, memory: 3 }));
        pass(&mut room.app, room.player);
        room.noise(6);
        pass(&mut room.app, room.player);
        assert!(room.asleep(sleeper), "a sound it cannot hear does not wake it");
        room.noise(8);
        assert!(!room.asleep(sleeper), "one it hears does");
        assert_eq!(asked.load(Ordering::Relaxed), 1, "in the player's pass, so the turn after is its own");
    }

    /// The blow that wakes a sleeper is still a blow on a sleeper: decided
    /// as it is rolled, it lands at the house's three times, and only then
    /// does the sleeper wake, so the next lands as on anyone. Stealth runs
    /// and the player hides, and the sleeper's notice could not pick it
    /// out, so nothing but the blow wakes it.
    #[test]
    fn a_blow_on_a_sleeper_lands_as_on_the_unaware_and_then_wakes_it() {
        let mut room = bedroom(|app| {
            app.add_plugins(StealthPlugin);
        });
        room.app.world_mut().entity_mut(room.player).insert(Stealth::default());
        let (sleeper, asked) = room.sleeper(1);
        room.app.world_mut().entity_mut(sleeper).insert(Notice(NoticeStats { certain: 10, chance_pct: 0, lit_bonus: 0, memory: 6 }));
        for _ in 0..2 {
            pass(&mut room.app, room.player);
        }
        assert!(room.asleep(sleeper), "a sleeper notices by chance alone, and this one has none");
        assert_eq!(room.strike(sleeper), 6, "the blow on a sleeper lands at three times");
        assert!(!room.asleep(sleeper), "and woke it");
        assert!(room.app.world().get::<Aware>(sleeper).unwrap().knows(room.player), "knowing who struck it");
        assert_eq!(asked.load(Ordering::Relaxed), 1, "its turn after the blow is its own");
        assert_eq!(room.strike(sleeper), 2, "so the next blow lands as on anyone");
    }

    /// A sleep that bites does not wake its holder with its own bite: the
    /// tick is harm, and harm wakes, but not harm that is the sleep itself.
    #[test]
    fn a_sleep_that_bites_every_turn_never_wakes_its_holder_by_its_own_bite() {
        let mut room = bedroom(|_| {});
        let (sleeper, _) = room.sleeper(4);
        let drowsing = room.drowsing;
        room.app.world_mut().write_message(ApplyStatus { target: sleeper, status: drowsing, turns: 10, by: None, held_by: None });
        pass(&mut room.app, room.player);
        room.app.world_mut().write_message(RemoveStatus { target: sleeper, status: room.asleep });
        for _ in 0..4 {
            pass(&mut room.app, room.player);
        }
        assert!(room.health(sleeper) < 20, "the sleep bit");
        assert!(room.app.world().get::<ActiveStatuses>(sleeper).unwrap().has(drowsing), "and its holder sleeps on through it");
    }

    /// A wound and a sleep landed in one pass, as a dart that does both
    /// lands them, leave the one they land on wounded and asleep: the sleep
    /// is put on before the wound in the pass, and the wound does not undo
    /// it. A blow in a later pass still wakes it.
    #[test]
    fn harm_landed_in_the_pass_a_sleep_is_put_on_does_not_wake_it() {
        let mut room = bedroom(|_| {});
        let (target, _) = room.sleeper(4);
        let (asleep, kind) = (room.asleep, room.kind);
        pass(&mut room.app, room.player);
        room.app.world_mut().write_message(RemoveStatus { target, status: asleep });
        pass(&mut room.app, room.player);
        assert!(!room.asleep(target), "awake to begin with");
        room.app.world_mut().write_message(ApplyStatus { target, status: asleep, turns: UNTIL_CURED, by: None, held_by: None });
        room.app.world_mut().write_message(DamageEvent::new(target, Hit::from_source(None, kind, 3)));
        pass(&mut room.app, room.player);
        assert_eq!(room.health(target), 17, "wounded");
        assert!(room.asleep(target), "and asleep");
        room.app.world_mut().write_message(DamageEvent::new(target, Hit::from_source(None, kind, 3)));
        pass(&mut room.app, room.player);
        assert!(!room.asleep(target), "until harm lands in a later pass");
    }

    /// Without stealth a sleeper sees on sight: a foe it can see within two
    /// cells wakes it on its own turn, which waking spends.
    #[test]
    fn a_foe_seen_within_two_cells_wakes_a_sleeper_on_its_turn() {
        let mut room = bedroom(|_| {});
        let (sleeper, asked) = room.sleeper(2);
        pass(&mut room.app, room.player);
        assert!(!room.asleep(sleeper), "it saw the player two cells off");
        assert_eq!(spent_by(&room.app, sleeper), vec![BASE_ACTION_COST], "on a turn that waking spent");
        pass(&mut room.app, room.player);
        assert_eq!(asked.load(Ordering::Relaxed), 1, "and the next is its own");
    }

    /// A sleeper notices only within two cells, and there only by its
    /// chance: one certain to notice anything stirs for the hider two cells
    /// off and knows nothing of the one three cells off, so it does not
    /// wake already knowing of something across the room.
    #[test]
    fn a_sleeper_notices_a_hider_within_two_cells_and_nothing_further() {
        let mut room = bedroom(|app| {
            app.add_plugins(StealthPlugin);
        });
        room.app.world_mut().entity_mut(room.player).insert(Stealth::default());
        let keen = Notice(NoticeStats { certain: 10, chance_pct: 100, lit_bonus: 0, memory: 6 });
        let (near, _) = room.sleeper(2);
        let (far, _) = room.sleeper(-3);
        room.app.world_mut().entity_mut(near).insert(keen);
        room.app.world_mut().entity_mut(far).insert(keen);
        for _ in 0..3 {
            pass(&mut room.app, room.player);
        }
        assert!(!room.asleep(near), "two cells off, noticed, and woken");
        assert!(room.app.world().get::<Aware>(near).unwrap().knows(room.player));
        assert!(room.asleep(far), "three off, asleep");
        assert!(!room.app.world().get::<Aware>(far).unwrap().knows(room.player), "and knowing nothing of it");
    }

    /// Without `StealthPlugin` a sleeper that keeps an `Aware` still sees
    /// a hider on sight, though something else registered the message the
    /// plugin writes, as a narration view does: the `Aware` nothing fills
    /// is not read as the sleeper knowing nothing.
    #[test]
    fn without_stealth_a_sleeper_with_an_aware_wakes_to_a_hider_on_sight_though_its_message_is_registered() {
        let mut room = bedroom(|app| {
            app.add_message::<crate::stealth::Noticed>();
        });
        room.app.world_mut().entity_mut(room.player).insert(Stealth::default());
        let (sleeper, _) = room.sleeper(2);
        room.app.world_mut().entity_mut(sleeper).insert(Notice(NoticeStats { certain: 0, chance_pct: 0, lit_bonus: 0, memory: 6 }));
        pass(&mut room.app, room.player);
        pass(&mut room.app, room.player);
        assert!(!room.asleep(sleeper), "it saw the hider two cells off, as it would with no stealth at all");
    }

    /// A status with no `sleeps` behaves as it did: a noise heard and a
    /// blow taken leave a snare on, and the snared is struck as anyone.
    #[test]
    fn a_noise_and_a_blow_leave_a_status_that_only_holds_on() {
        let mut room = bedroom(with_hearing);
        let at = Position(room.start.offset(1, 0));
        let snared =
            room.app.world_mut().spawn((Actor, Blocks, at, Health::full(20), Faction(room.theirs), Hearing(HearingStats { threshold: 0, memory: 3 }))).id();
        room.app.world_mut().write_message(ApplyStatus { target: snared, status: room.snared, turns: UNTIL_CURED, by: None, held_by: None });
        pass(&mut room.app, room.player);
        room.noise(8);
        assert_eq!(room.strike(snared), 2, "struck as anyone");
        assert!(room.app.world().get::<ActiveStatuses>(snared).unwrap().has(room.snared), "and still snared");
    }

    /// A field with a player in it and two warring sides, where `defs` are
    /// the statuses, play begun.
    fn field(defs: Vec<StatusDef>) -> (App, crate::testing::TwoSides, Entity, Point) {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::combat::CombatPlugin));
        let start = crate::testing::ground(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        app.world_mut().resource_mut::<Registries>().statuses = Registry::from_defs(defs).unwrap();
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(6), RevealsMap, Faction(sides.ours))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        (app, sides, player, start)
    }

    fn status(app: &App, name: &str) -> StatusId {
        app.world().resource::<Registries>().statuses.expect(name)
    }

    /// A status naming a side puts its holder on it the pass it lands and
    /// back on its own side the pass it runs out, with the side it had kept
    /// beside it meanwhile.
    #[test]
    fn a_status_naming_a_side_turns_its_holder_until_it_ends_and_gives_its_own_side_back() {
        let theirs = atglyph_rules::FactionId::from_raw(1);
        let (mut app, sides, player, start) = field(vec![StatusDef::new("turned").sides_with(theirs)]);
        let other = app.world_mut().spawn((Actor, Blocks, Position(start.offset(3, 0)), Faction(sides.ours))).id();
        let turned = status(&app, "turned");
        app.world_mut().write_message(ApplyStatus { target: other, status: turned, turns: 2, by: None, held_by: None });
        app.update();
        assert_eq!(app.world().get::<Faction>(other), Some(&Faction(sides.theirs)), "turned the pass it landed");
        assert_eq!(app.world().get::<SideWas>(other), Some(&SideWas(Faction(sides.ours))));
        for _ in 0..2 {
            app.world_mut().write_message(Intent::new(player, Wait));
            app.update();
        }
        assert!(!app.world().get::<ActiveStatuses>(other).unwrap().has(turned), "run out");
        assert_eq!(app.world().get::<Faction>(other), Some(&Faction(sides.ours)), "and back on its own side");
        assert!(app.world().get::<SideWas>(other).is_none());
    }

    /// What a status resists is resisted where a blow is weighed while the
    /// status is on, and not once it is cured.
    #[test]
    fn what_a_status_resists_is_resisted_while_it_is_held_and_not_after() {
        let kind = atglyph_rules::damage::DamageKindId::from_raw(0);
        let (mut app, _, player, _) = field(vec![StatusDef::new("warded").resists(kind, 40)]);
        let warded = status(&app, "warded");
        let mut own = Resistances::new();
        own.add(kind, 10);
        app.world_mut().entity_mut(player).insert(crate::combat::Resists(own));
        let resisted = |app: &mut App| {
            use bevy::ecs::system::RunSystemOnce;
            app.world_mut().run_system_once(move |loadout: crate::combat::Loadout| loadout.resistances(player).get(kind)).unwrap()
        };
        assert_eq!(resisted(&mut app), 10, "its own");
        app.world_mut().write_message(ApplyStatus { target: player, status: warded, turns: UNTIL_CURED, by: None, held_by: None });
        app.update();
        assert_eq!(resisted(&mut app), 50, "its own and the ward's");
        app.world_mut().write_message(RemoveStatus { target: player, status: warded });
        app.update();
        assert_eq!(resisted(&mut app), 10, "its own again");
        assert!(app.world().get::<StatusResists>(player).is_none(), "nothing kept for a status that is gone");
    }

    /// A kind's own rule for a status changes its stats while it holds it
    /// and gives them back when it lets go, and an actor without the rule
    /// holding the same status is not changed.
    #[test]
    fn a_kind_holding_a_status_has_its_stats_changed_until_it_lets_go_and_nobody_else_does() {
        let (mut app, _, player, start) = field(vec![StatusDef::new("burning")]);
        let stat_defs = Registry::from_defs(vec![StatDef::new("mending", 0)]).unwrap();
        let mending = stat_defs.expect("mending");
        app.world_mut().resource_mut::<Registries>().stats = stat_defs.clone();
        let burning = status(&app, "burning");
        let kind = app.world_mut().spawn((Actor, Blocks, Position(start.offset(2, 0)), WhileHolding(vec![(burning, vec![(mending, Op::Add(-100))])]))).id();
        let value = |app: &App, who: Entity| app.world().get::<StatBlock>(who).unwrap().value(mending, &stat_defs);
        for who in [kind, player] {
            app.world_mut().write_message(ApplyStatus { target: who, status: burning, turns: UNTIL_CURED, by: None, held_by: None });
        }
        app.update();
        assert_eq!((value(&app, kind), value(&app, player)), (-100, 0), "only the kind with the rule");
        app.update();
        assert_eq!(value(&app, kind), -100, "put on once, not every pass");
        app.world_mut().write_message(RemoveStatus { target: kind, status: burning });
        app.update();
        assert_eq!(value(&app, kind), 0, "and given back");
    }

    /// A holder that always stumbles, walled in on every side but one,
    /// steps that one way whichever it meant; walled in on every side, it
    /// steps as it meant, into the wall, and goes nowhere.
    #[test]
    fn a_holder_that_always_stumbles_steps_the_one_open_way_and_a_walled_in_one_goes_nowhere() {
        let (mut app, _, player, start) = field(vec![StatusDef::new("reeling").stumbles(100)]);
        let reeling = status(&app, "reeling");
        app.world_mut().write_message(ApplyStatus { target: player, status: reeling, turns: UNTIL_CURED, by: None, held_by: None });
        app.update();
        let around: Vec<Point> = Direction::ALL.into_iter().map(|d| start + d.offset()).filter(|p| *p != start.offset(1, 0)).collect();
        crate::testing::wall_off(&mut app, &around);
        app.world_mut().write_message(Intent::new(player, Step(Direction::West)));
        app.update();
        assert_eq!(app.world().get::<Position>(player).unwrap().0, start.offset(1, 0), "turned the one open way");
        let all: Vec<Point> = Direction::ALL.into_iter().map(|d| start.offset(1, 0) + d.offset()).collect();
        crate::testing::wall_off(&mut app, &all);
        app.world_mut().write_message(Intent::new(player, Step(Direction::West)));
        app.update();
        assert_eq!(app.world().get::<Position>(player).unwrap().0, start.offset(1, 0), "walled in, it went nowhere");
    }

    /// The run's seed is asked for by the statuses only where one of them
    /// stumbles, and then loudly when play begins.
    #[test]
    #[should_panic(expected = "a status that stumbles draws from")]
    fn a_game_with_a_status_that_stumbles_and_no_seed_is_told_so_when_play_begins() {
        let mut app = headless_app();
        app.add_plugins(crate::fov::FovPlugin);
        let start = crate::testing::ground(&mut app);
        app.insert_resource(Registries { statuses: Registry::from_defs(vec![StatusDef::new("reeling").stumbles(100)]).unwrap(), ..Default::default() });
        app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(6), RevealsMap));
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
    }

    #[test]
    fn a_game_whose_statuses_never_stumble_plays_with_no_seed() {
        let mut app = headless_app();
        app.add_plugins(crate::fov::FovPlugin);
        let start = crate::testing::ground(&mut app);
        app.insert_resource(Registries { statuses: Registry::from_defs(vec![StatusDef::new("dazed")]).unwrap(), ..Default::default() });
        app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(6), RevealsMap));
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
    }
}
