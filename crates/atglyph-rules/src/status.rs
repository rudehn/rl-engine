//! Statuses as data: a registered definition, and the instances an actor
//! carries with their remaining duration, which for one put on
//! [`UNTIL_CURED`] never runs down.
//!
//! What a status *does* is expressed as stat modifiers the engine can
//! apply and remove, plus an optional damage-over-time the game's damage
//! pipeline resolves, whether it holds its holder's turns, and whether it
//! sleeps, a hold a noise, a blow or a foe close by ends early; and, each
//! optional and absent meaning no such mechanic, how many of it may stack,
//! what its holder resists while it lasts, the side its holder fights for,
//! and how often its holder's steps go astray. Anything richer is the
//! game's, keyed by the id.
//!
//! [`load`] reads definitions from RON by name, resolving every stat and
//! damage kind through [`Names`], so a game authors statuses in the words
//! its other content uses and never mirrors this schema in a type of its
//! own.

use crate::content::{ContentError, Named, Registry};
use atglyph_core::{Direction, Id};
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::damage::{DamageKindId, Resistances};
use crate::faction::{FactionDef, FactionId};
use crate::names::Names;
use crate::stats::{Op, Source, StatId, Stats};

/// What happens when a status is applied to an actor that has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stacking {
    /// The new duration replaces the old if longer.
    Refresh,
    /// Durations add.
    Extend,
    /// A second instance sits beside the first.
    Stack,
    /// The second application is ignored.
    Ignore,
}

/// A stat change a status applies while it lasts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusModifier {
    /// The stat.
    pub stat: StatId,
    /// The change.
    pub op: Op,
}

/// A registered status.
///
/// Built by [`load`] from names, or by hand with the methods below, and
/// never deserialized as it stands: its ids index registries a content file
/// cannot see, and a number written in one is a modifier that lands on
/// another stat the day the stat list is reordered.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusDef {
    /// The name content refers to it by.
    pub name: String,
    /// How a repeat application behaves.
    pub stacking: Stacking,
    /// Stat changes while active.
    pub modifiers: Vec<StatusModifier>,
    /// Damage per turn as `(kind, amount)`, if any.
    pub tick_damage: Option<(DamageKindId, i32)>,
    /// Health restored per turn, if any. Not a tick of a kind: mending is
    /// not damage, so no armor, resistance or stage is asked about it.
    pub tick_mend: Option<i32>,
    /// Glyph for a badge, if the game wants one.
    pub badge: Option<char>,
    /// Whether nothing sees whoever holds it while it lasts. What that
    /// means is stealth's to say: no mind perceives the holder and no
    /// observer notices it, and an attack it makes ends it.
    pub unseen: bool,
    /// Whether holding it is good for the holder, a cloak or a mend rather
    /// than a poison. The rules never read it; it is what a narrator tells
    /// good news from bad by, so a game's log does not announce a gift in
    /// the tone of a wound.
    pub boon: bool,
    /// Whether whoever carries it loses its turns while it lasts: each is
    /// spent as a wait before its mind or its player is asked, so
    /// paralysis and a snare are each one of these. It ends as any status
    /// does, when its time runs out or it is cured; one that something
    /// should break early is [`sleeps`](Self::sleeps). Not
    /// [`ActiveStatus::held_by`], the worn thing an instance lasts only
    /// while.
    pub holds: bool,
    /// Whether it is sleep: it holds its holder's turns as
    /// [`holds`](Self::holds) does, whether or not that is set, and it ends
    /// the moment the holder is woken, by a noise it hears, by harm landing
    /// on it, or by a foe it notices within [`SLEEPER_REACH`]. Harm that is
    /// the sleep's own tick wakes nobody, nor does harm landing in the pass
    /// the sleep is put on, so a sleep that bites and a wound and a sleep
    /// landed together both leave their holder asleep. What wakes
    /// a sleeper is the engine's, so every game's sleepers wake alike;
    /// who is put to sleep, and for how long, is the game's.
    pub sleeps: bool,
    /// The most instances held at once, read only under
    /// [`Stacking::Stack`]: at the cap a new one is refused, so a stat a
    /// stacking curse lowers stops falling. `None` stacks without limit.
    /// A cap rather than a stacking rule of its own, because "stack, up to
    /// three" is still stacking and every other rule has nothing to count.
    pub max: Option<u32>,
    /// What its holder resists while it lasts, per instance, added to the
    /// holder's own and its gear's where a blow is weighed. Empty resists
    /// nothing. A field rather than a stat, because a resistance is per
    /// damage kind and the kinds are the game's.
    pub resists: Resistances,
    /// The side its holder fights for while it lasts, going back to the one
    /// it had when the last such status ends. `None` leaves the side alone.
    /// Which sides there are and who is hostile to whom stay the game's;
    /// this only says which of them.
    pub side: Option<FactionId>,
    /// The percent of its holder's chosen steps and bumps turned a random
    /// way among those open to it, `1..=100`. `None` never turns one. A
    /// percent rather than a flag, so a stagger that sends one step in
    /// four astray is the same field as a confusion that sends every one.
    pub stumbles: Option<u32>,
}

fn refresh() -> Stacking {
    Stacking::Refresh
}

impl StatusDef {
    /// A status with no effects.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            stacking: Stacking::Refresh,
            modifiers: Vec::new(),
            tick_damage: None,
            tick_mend: None,
            badge: None,
            unseen: false,
            boon: false,
            holds: false,
            sleeps: false,
            max: None,
            resists: Resistances::new(),
            side: None,
            stumbles: None,
        }
    }

    /// Sets the stacking rule.
    pub fn stacking(mut self, s: Stacking) -> Self {
        self.stacking = s;
        self
    }

    /// Adds a stat modifier.
    pub fn modifies(mut self, stat: StatId, op: Op) -> Self {
        self.modifiers.push(StatusModifier { stat, op });
        self
    }

    /// Sets damage per turn.
    pub fn ticks(mut self, kind: DamageKindId, amount: i32) -> Self {
        self.tick_damage = Some((kind, amount));
        self
    }

    /// Sets health restored per turn, so regeneration is a status as
    /// poison is.
    pub fn mends(mut self, amount: i32) -> Self {
        self.tick_mend = Some(amount);
        self
    }

    /// Makes whoever holds it unseen while it lasts.
    pub fn unseen(mut self) -> Self {
        self.unseen = true;
        self
    }

    /// Marks it good for whoever holds it.
    pub fn boon(mut self) -> Self {
        self.boon = true;
        self
    }

    /// Makes whoever carries it lose its turns while it lasts.
    pub fn holds(mut self) -> Self {
        self.holds = true;
        self
    }

    /// Makes it sleep: it holds its holder until it runs out or the holder
    /// is woken.
    pub fn sleeps(mut self) -> Self {
        self.sleeps = true;
        self
    }

    /// Caps how many instances may be held at once, for a status that
    /// [`Stacking::Stack`]s.
    pub fn at_most(mut self, instances: u32) -> Self {
        self.max = Some(instances);
        self
    }

    /// Makes its holder resist `pct` percent of `kind` while it lasts.
    pub fn resists(mut self, kind: DamageKindId, pct: i32) -> Self {
        self.resists.add(kind, pct);
        self
    }

    /// Puts its holder on `side` while it lasts.
    pub fn sides_with(mut self, side: FactionId) -> Self {
        self.side = Some(side);
        self
    }

    /// Turns `pct` percent of its holder's steps a random way while it
    /// lasts.
    pub fn stumbles(mut self, pct: u32) -> Self {
        self.stumbles = Some(pct);
        self
    }
}

impl Named for StatusDef {
    fn name(&self) -> &str {
        &self.name
    }
}

/// A registered status id.
pub type StatusId = Id<StatusDef>;

/// A status as authored, before its names are resolved.
#[derive(Debug, Deserialize)]
struct Authored {
    name: String,
    #[serde(default = "refresh")]
    stacking: Stacking,
    #[serde(default)]
    modifies: Vec<(String, Op)>,
    #[serde(default)]
    ticks: Option<(String, i32)>,
    #[serde(default)]
    mends: Option<i32>,
    #[serde(default)]
    badge: Option<char>,
    #[serde(default)]
    unseen: bool,
    #[serde(default)]
    boon: bool,
    #[serde(default)]
    holds: bool,
    #[serde(default)]
    sleeps: bool,
    #[serde(default)]
    max: Option<u32>,
    #[serde(default)]
    resists: Vec<(String, i32)>,
    #[serde(default)]
    side: Option<String>,
    #[serde(default)]
    stumbles: Option<u32>,
}

impl Named for Authored {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Loads statuses from RON, resolving every stat, damage kind and faction
/// through `names`.
///
/// One entry per status, and every field but `name` may be left out:
///
/// - `name`: unique; what a requirement, an effect or the game's code calls it.
/// - `stacking`: what a second application does. `Refresh`, the default,
///   keeps the longer duration; `Extend` adds them; `Stack` puts a second
///   instance beside the first; `Ignore` does nothing.
/// - `modifies`: stat changes while it lasts, each `(stat, op)` with `op`
///   one of `Add(n)`, `MulPct(n)`, `AtLeast(n)` or `AtMost(n)`.
/// - `ticks`: `(damage kind, amount)` dealt every whole turn. Never
///   negative: a status that mends says `mends`.
/// - `mends`: health restored every whole turn, at least 1.
/// - `badge`: one character a panel may draw beside a health bar.
/// - `unseen`: `true` for a status whose holder nothing can see while it
///   lasts; `false`, the default, otherwise.
/// - `boon`: `true` for a status that is good for its holder, which a
///   narrator tells as good news; `false`, the default, otherwise.
/// - `holds`: `true` for a status whose holder loses its turns while it
///   lasts, each spent as a wait; `false`, the default, otherwise.
/// - `sleeps`: `true` for sleep, a status that holds its holder until a
///   noise, harm or a foe close by wakes it and ends it; `false`, the
///   default, otherwise. It holds whether or not `holds` is written.
/// - `max`: the most instances held at once, at least 1, for a status
///   whose `stacking` is `Stack` and only there; a further one is refused.
///   Absent, it stacks without limit.
/// - `resists`: `(damage kind, percent)` its holder resists while it lasts,
///   per instance, a kind written twice resisting by both.
/// - `side`: the faction its holder fights for while it lasts, going back
///   to its own when it ends. Absent, the side is left alone.
/// - `stumbles`: the percent, `1` to `100`, of its holder's steps and bumps
///   turned a random open way while it lasts. Absent, none are.
///
/// Reports every unknown name in the file at once.
pub fn load(text: &str, names: &Names<'_>) -> Result<Registry<StatusDef>, ContentError> {
    let authored: Registry<Authored> = Registry::from_ron_str(text)?;
    let mut errors = Vec::new();
    let mut defs = Vec::new();
    for (_, a) in authored.iter() {
        let mut modifiers = Vec::new();
        for (stat, op) in &a.modifies {
            match names.stat(stat) {
                Ok(stat) => modifiers.push(StatusModifier { stat, op: *op }),
                Err(e) => errors.push(format!("{}: {e}", a.name)),
            }
        }
        let tick_damage = a.ticks.as_ref().and_then(|(kind, amount)| match names.damage_kind(kind) {
            // Refused rather than read as a mend: mending was once a
            // negative tick of a kind a game invented for it, and a file
            // still written that way should say so, not quietly hurt.
            Ok(_) if *amount < 0 => {
                errors.push(format!("{}: `ticks` of {amount} is not damage; a status that mends says `mends: {}`", a.name, -amount));
                None
            }
            Ok(kind) => Some((kind, *amount)),
            Err(e) => {
                errors.push(format!("{}: {e}", a.name));
                None
            }
        });
        if let Some(amount) = a.mends.filter(|amount| *amount < 1) {
            errors.push(format!("{}: `mends` of {amount} restores nothing; it is at least 1", a.name));
        }
        // A cap on a status that never stacks is a rule nothing plays, so
        // it is refused rather than read as a promise.
        match a.max {
            Some(0) => errors.push(format!("{}: `max` of 0 refuses every instance; it is at least 1", a.name)),
            Some(_) if a.stacking != Stacking::Stack => {
                errors.push(format!("{}: `max` caps instances, and only `stacking: Stack` holds more than one", a.name))
            }
            _ => {}
        }
        if let Some(pct) = a.stumbles.filter(|pct| !(1..=100).contains(pct)) {
            errors.push(format!("{}: `stumbles` of {pct} is not a percent of steps; it runs from 1 to 100", a.name));
        }
        let mut resists = Resistances::new();
        for (kind, pct) in &a.resists {
            match names.damage_kind(kind) {
                Ok(kind) => resists.add(kind, *pct),
                Err(e) => errors.push(format!("{}: {e}", a.name)),
            }
        }
        let side = a.side.as_deref().and_then(|side| match names.id::<FactionDef>(side) {
            Ok(side) => Some(side),
            Err(e) => {
                errors.push(format!("{}: {e}", a.name));
                None
            }
        });
        defs.push(StatusDef {
            name: a.name.clone(),
            stacking: a.stacking,
            modifiers,
            tick_damage,
            tick_mend: a.mends,
            badge: a.badge,
            unseen: a.unseen,
            boon: a.boon,
            holds: a.holds,
            sleeps: a.sleeps,
            max: a.max,
            resists,
            side,
            stumbles: a.stumbles,
        });
    }
    if !errors.is_empty() {
        return Err(ContentError::Invalid(errors));
    }
    Registry::from_defs(defs)
}

/// How long a status lasts that never runs out: one put on for this many
/// turns is ticked every turn, its damage and its mending land, and it is
/// never counted down, so it ends only when it is cured.
///
/// For what a body is rather than what happened to it: a body that mends
/// by its nature, or what a worn thing gives for as long as it is worn,
/// which the thing's own hold ends. The largest count there is rather
/// than a flag beside the count, so a save keeps it as it keeps any count,
/// and the stacking rules leave it as it is: a refresh keeps the longer,
/// and an extension saturates. A panel that counts a status's turns shows
/// none for it.
pub const UNTIL_CURED: u32 = u32::MAX;

/// How far a sleeper notices anything, in cells in any direction: a foe
/// this close that it notices wakes it, and one further off it does not
/// notice at all, so a sleeper does not wake already knowing of something
/// across the room. "A cell or two", for every game alike; a sleeper that
/// should be harder to creep past is one whose notice is keener.
pub const SLEEPER_REACH: i32 = 2;

/// One status on one actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveStatus {
    /// Which status.
    pub id: StatusId,
    /// Whole turns left, or [`UNTIL_CURED`] for one that never runs out.
    pub turns: u32,
    /// Who applied it, for credit on a tick. Opaque.
    pub source: Option<u64>,
    /// The worn thing it lasts only while, if one holds it. Opaque, like
    /// `source`: the rules never read it, and whoever answers wearing ends
    /// the instance the pass its holder is not worn. Absent in a save
    /// written before a status could be held, which reads as held by
    /// nothing.
    #[serde(default)]
    pub held_by: Option<u64>,
}

/// Damage one status dealt on a tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tick {
    /// Which status.
    pub status: StatusId,
    /// The damage kind.
    pub kind: DamageKindId,
    /// How much, before mitigation.
    pub amount: i32,
    /// Who applied the status, for credit.
    pub source: Option<u64>,
}

/// Health one status restored on a tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mended {
    /// Which status.
    pub status: StatusId,
    /// How much it offers, before the holder's own mending is asked.
    pub amount: i32,
    /// Who applied the status.
    pub source: Option<u64>,
}

/// What one turn did to an actor's statuses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TickReport {
    /// Damage to resolve, in status order.
    pub ticks: Vec<Tick>,
    /// Health to restore, in status order.
    pub mends: Vec<Mended>,
    /// Statuses that ran out this turn.
    pub expired: Vec<StatusId>,
}

/// The statuses on one actor.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Statuses {
    active: Vec<ActiveStatus>,
}

/// The source under which a status's modifiers sit in [`Stats`]: unique
/// per status id and instance, so removal is exact.
fn source_tag(id: StatusId, instance: usize) -> Source {
    Source::Status { status: id, instance: instance.min(u16::MAX as usize) as u16 }
}

impl Statuses {
    /// No statuses.
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies `id` for `turns`, following the definition's stacking rule,
    /// and installs its modifiers into `stats`. Returns whether anything
    /// changed.
    ///
    /// Held by nothing; a refresh or an extension of an instance a worn
    /// thing holds leaves it held, since a second source of the same status
    /// is no reason for the thing's to outlast the thing.
    pub fn apply(&mut self, id: StatusId, turns: u32, source: Option<u64>, defs: &Registry<StatusDef>, stats: &mut Stats) -> bool {
        self.put(id, turns, source, None, defs, stats)
    }

    /// As [`apply`](Self::apply), held by `holder`: the instance lasts only
    /// while that thing is worn.
    ///
    /// An instance it refreshes or extends is taken over by `holder`,
    /// whether or not its time changed, so what holds a status is always
    /// the thing that last put it on. The return value still says only
    /// whether the time or the instances changed, which is what is
    /// reported.
    pub fn apply_held(&mut self, id: StatusId, turns: u32, source: Option<u64>, holder: u64, defs: &Registry<StatusDef>, stats: &mut Stats) -> bool {
        self.put(id, turns, source, Some(holder), defs, stats)
    }

    fn put(&mut self, id: StatusId, turns: u32, source: Option<u64>, held_by: Option<u64>, defs: &Registry<StatusDef>, stats: &mut Stats) -> bool {
        let def = defs.get(id);
        let instances = self.active.iter().filter(|s| s.id == id).count();
        let existing = self.active.iter_mut().find(|s| s.id == id);
        match (def.stacking, existing) {
            (Stacking::Ignore, Some(_)) => false,
            // At the cap a further instance is refused outright: nothing is
            // refreshed in its place, since which of the held ones would be
            // is a rule nobody wrote.
            (Stacking::Stack, Some(_)) if def.max.is_some_and(|max| instances >= max as usize) => false,
            (Stacking::Refresh, Some(s)) => {
                let changed = turns > s.turns;
                s.turns = s.turns.max(turns);
                s.held_by = held_by.or(s.held_by);
                changed
            }
            (Stacking::Extend, Some(s)) => {
                s.turns = s.turns.saturating_add(turns);
                s.held_by = held_by.or(s.held_by);
                true
            }
            _ => {
                let tag = source_tag(id, instances);
                for m in &def.modifiers {
                    stats.add(crate::stats::Modifier::new(m.stat, m.op, tag));
                }
                self.active.push(ActiveStatus { id, turns, source, held_by });
                true
            }
        }
    }

    /// Whether `id` is active.
    pub fn has(&self, id: StatusId) -> bool {
        self.active.iter().any(|s| s.id == id)
    }

    /// Every active status.
    pub fn iter(&self) -> impl Iterator<Item = &ActiveStatus> {
        self.active.iter()
    }

    /// The first active status that [`holds`](StatusDef::holds) or
    /// [`sleeps`](StatusDef::sleeps), if any: what takes the actor's turns
    /// while it is on.
    pub fn holding(&self, defs: &Registry<StatusDef>) -> Option<StatusId> {
        self.active.iter().map(|s| s.id).find(|id| {
            let def = defs.get(*id);
            def.holds || def.sleeps
        })
    }

    /// The first active status that [`sleeps`](StatusDef::sleeps), if any:
    /// whether the actor is asleep.
    pub fn sleeping(&self, defs: &Registry<StatusDef>) -> Option<StatusId> {
        self.active.iter().map(|s| s.id).find(|id| defs.get(*id).sleeps)
    }

    /// What every active status makes the actor resist, instance by
    /// instance, so two stacks of a status that resists a tenth resist a
    /// fifth.
    pub fn resistances(&self, defs: &Registry<StatusDef>) -> Resistances {
        let mut total = Resistances::new();
        for s in &self.active {
            total.plus(&defs.get(s.id).resists);
        }
        total
    }

    /// The side the first active status that names one puts the actor on,
    /// if any.
    pub fn side(&self, defs: &Registry<StatusDef>) -> Option<FactionId> {
        self.active.iter().find_map(|s| defs.get(s.id).side)
    }

    /// The highest percent of the actor's steps any active status turns
    /// astray, if any does.
    pub fn stumbles(&self, defs: &Registry<StatusDef>) -> Option<u32> {
        self.active.iter().filter_map(|s| defs.get(s.id).stumbles).max()
    }

    /// Wakes the actor: cures every status that sleeps, modifiers and all,
    /// and returns which, in the order they were on. A status that only
    /// holds stays, since a snare is not slept off.
    pub fn wake(&mut self, defs: &Registry<StatusDef>, stats: &mut Stats) -> Vec<StatusId> {
        let mut sleeping: Vec<StatusId> = Vec::new();
        for s in &self.active {
            if defs.get(s.id).sleeps && !sleeping.contains(&s.id) {
                sleeping.push(s.id);
            }
        }
        for id in &sleeping {
            self.cure(*id, stats);
        }
        sleeping
    }

    /// Advances one turn: collects the damage each status deals, then
    /// expires what ran out and removes its modifiers. A status put on
    /// [`UNTIL_CURED`] bites and mends like any other and is never
    /// counted down.
    pub fn tick(&mut self, defs: &Registry<StatusDef>, stats: &mut Stats) -> TickReport {
        let (mut ticks, mut mends) = (Vec::new(), Vec::new());
        for s in &self.active {
            let def = defs.get(s.id);
            if let Some((kind, amount)) = def.tick_damage {
                ticks.push(Tick { status: s.id, kind, amount, source: s.source });
            }
            if let Some(amount) = def.tick_mend {
                mends.push(Mended { status: s.id, amount, source: s.source });
            }
        }
        let mut expired = Vec::new();
        let mut kept = Vec::with_capacity(self.active.len());
        let mut instance_of: Vec<(StatusId, usize)> = Vec::new();
        for s in self.active.drain(..) {
            let instance = instance_of.iter().filter(|(id, _)| *id == s.id).count();
            instance_of.push((s.id, instance));
            if s.turns == UNTIL_CURED {
                kept.push(s);
            } else if s.turns <= 1 {
                stats.remove_source(source_tag(s.id, instance));
                expired.push(s.id);
            } else {
                kept.push(ActiveStatus { turns: s.turns - 1, ..s });
            }
        }
        // Surviving instances keep their original tags: removal above used
        // the instance index at the time of application, which the order of
        // `active` preserves.
        self.active = kept;
        TickReport { ticks, mends, expired }
    }

    /// Removes `id` outright, modifiers included. Returns whether it was there.
    pub fn cure(&mut self, id: StatusId, stats: &mut Stats) -> bool {
        let mut removed = false;
        let mut instance = 0;
        self.active.retain(|s| {
            if s.id == id {
                stats.remove_source(source_tag(id, instance));
                instance += 1;
                removed = true;
                false
            } else {
                true
            }
        });
        removed
    }
}

/// The way a stumbling holder is turned, if it is, among the `open` ways it
/// may go: the percent is rolled first, then one open way is picked.
///
/// The draws are the run's stream spent in the order a seed has always
/// replayed: nothing for a holder that never stumbles (`None`) or has no
/// open way; otherwise one for the percent when it is under a hundred
/// (zero included, which then never passes), and one for the pick when the
/// percent passed or was a hundred.
pub fn stumble(pct: Option<u32>, open: &[Direction], rng: &mut impl Rng) -> Option<Direction> {
    let pct = pct?;
    if open.is_empty() {
        return None;
    }
    if pct < 100 && rng.random_range(0..100) >= pct {
        return None;
    }
    Some(open[rng.random_range(0..open.len())])
}

/// Whether a blow wakes the sleeper it lands on.
///
/// Two harms wake nobody: a sleeping status's own tick, which would cure a
/// sleep that also bites the first turn it bit, and harm landing in the
/// pass the sleep was put on, since a dart that wounds and sends to sleep
/// is one landing and a sleeper woken by its own wound never slept.
pub fn wakes_from_harm(ticked_by_its_own_sleep: bool, fell_asleep_this_pass: bool) -> bool {
    !ticked_by_its_own_sleep && !fell_asleep_this_pass
}

/// What a holder's side becomes this pass, as `(side now, side to give
/// back)`, or `None` to leave it be.
///
/// `held` is the side its statuses name, `now` the side it is on and `was`
/// the side it kept when it was turned. Turned once, when the first such
/// status comes on, so a holder some other rule moves meanwhile stays
/// where that rule put it; given back the pass the last ends.
pub fn side_change(held: Option<FactionId>, now: FactionId, was: Option<FactionId>) -> Option<(FactionId, Option<FactionId>)> {
    match (held, was) {
        (Some(side), None) => Some((side, Some(now))),
        (None, Some(was)) => Some((was, None)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::StatDef;

    fn world() -> (Registry<StatDef>, Registry<StatusDef>) {
        let stats = Registry::from_defs(vec![StatDef::new("speed", 100)]).unwrap();
        let speed = stats.expect("speed");
        let statuses = Registry::from_defs(vec![
            StatusDef::new("slowed").modifies(speed, Op::MulPct(50)),
            StatusDef::new("burning").ticks(DamageKindId::from_raw(0), 2).stacking(Stacking::Extend),
            StatusDef::new("poisoned").ticks(DamageKindId::from_raw(1), 1).stacking(Stacking::Stack),
        ])
        .unwrap();
        (stats, statuses)
    }

    #[test]
    fn a_status_modifies_stats_until_it_expires() {
        let (stat_defs, defs) = world();
        let speed = stat_defs.expect("speed");
        let slowed = defs.expect("slowed");
        let mut stats = Stats::new();
        let mut st = Statuses::new();
        assert!(st.apply(slowed, 2, None, &defs, &mut stats));
        assert_eq!(stats.value(speed, &stat_defs), 50);
        assert!(!st.apply(slowed, 1, None, &defs, &mut stats), "a shorter refresh changes nothing");
        assert!(st.apply(slowed, 3, None, &defs, &mut stats));
        for _ in 0..2 {
            assert!(st.tick(&defs, &mut stats).expired.is_empty());
        }
        assert_eq!(st.tick(&defs, &mut stats).expired, vec![slowed]);
        assert_eq!(stats.value(speed, &stat_defs), 100);
        assert!(!st.has(slowed));
    }

    #[test]
    fn ticks_extend_stack_and_cure() {
        let (_, defs) = world();
        let burning = defs.expect("burning");
        let poisoned = defs.expect("poisoned");
        let mut stats = Stats::new();
        let mut st = Statuses::new();
        st.apply(burning, 2, Some(9), &defs, &mut stats);
        st.apply(burning, 2, Some(9), &defs, &mut stats);
        assert_eq!(st.iter().next().unwrap().turns, 4, "extend adds");
        st.apply(poisoned, 3, None, &defs, &mut stats);
        st.apply(poisoned, 3, None, &defs, &mut stats);
        assert_eq!(st.iter().filter(|s| s.id == poisoned).count(), 2, "stack sits beside");
        let report = st.tick(&defs, &mut stats);
        assert_eq!(report.ticks.len(), 3);
        assert_eq!(report.ticks[0], Tick { status: burning, kind: DamageKindId::from_raw(0), amount: 2, source: Some(9) });
        assert!(st.cure(poisoned, &mut stats));
        assert!(!st.has(poisoned));
        assert!(!st.cure(poisoned, &mut stats));
    }

    /// A status put on until it is cured bites and mends every turn like
    /// any other, and is never counted down: a thousand turns later it has
    /// as long left as it had, neither stacking rule shortens it, and only
    /// a cure takes it off.
    #[test]
    fn a_status_put_on_until_cured_is_never_counted_down_and_ends_only_by_a_cure() {
        let (stat_defs, defs) = world();
        let speed = stat_defs.expect("speed");
        let (slowed, burning) = (defs.expect("slowed"), defs.expect("burning"));
        let mut stats = Stats::new();
        let mut st = Statuses::new();
        st.apply(slowed, UNTIL_CURED, None, &defs, &mut stats);
        st.apply(burning, UNTIL_CURED, Some(9), &defs, &mut stats);
        for turn in 0..1000 {
            let report = st.tick(&defs, &mut stats);
            assert!(report.expired.is_empty(), "turn {turn}: nothing ran out");
            assert_eq!(report.ticks.len(), 1, "turn {turn}: the burn bit");
        }
        assert!(st.iter().all(|s| s.turns == UNTIL_CURED), "{:?}", st.iter().collect::<Vec<_>>());
        st.apply(slowed, 3, None, &defs, &mut stats);
        st.apply(burning, 3, None, &defs, &mut stats);
        assert!(st.iter().all(|s| s.turns == UNTIL_CURED), "a refresh keeps the longer and an extension saturates");
        assert_eq!(stats.value(speed, &stat_defs), 50, "still slowed");
        assert!(st.cure(slowed, &mut stats));
        assert_eq!(stats.value(speed, &stat_defs), 100, "and cured");
    }

    fn vocabulary() -> (Registry<StatDef>, Registry<crate::damage::DamageKind>) {
        let stats = Registry::from_defs(vec![StatDef::new("speed", 100), StatDef::new("armor", 0)]).unwrap();
        let kinds = Registry::from_defs(vec![crate::damage::DamageKind::new("bite")]).unwrap();
        (stats, kinds)
    }

    fn held_by(st: &Statuses, id: StatusId) -> Vec<Option<u64>> {
        st.iter().filter(|s| s.id == id).map(|s| s.held_by).collect()
    }

    #[test]
    fn a_status_applied_with_a_holder_records_it_and_one_applied_without_has_none() {
        let (_, defs) = world();
        let (slowed, burning) = (defs.expect("slowed"), defs.expect("burning"));
        let mut stats = Stats::new();
        let mut st = Statuses::new();
        assert!(st.apply_held(slowed, 3, Some(9), 42, &defs, &mut stats));
        assert!(st.apply(burning, 3, Some(9), &defs, &mut stats));
        assert_eq!(held_by(&st, slowed), vec![Some(42)]);
        assert_eq!(held_by(&st, burning), vec![None]);
    }

    #[test]
    fn a_held_application_takes_over_what_it_refreshes_or_extends_and_an_unheld_one_leaves_the_holder() {
        let (_, defs) = world();
        let (slowed, burning) = (defs.expect("slowed"), defs.expect("burning"));
        let mut stats = Stats::new();
        let mut st = Statuses::new();
        st.apply(slowed, 3, None, &defs, &mut stats);
        st.apply_held(slowed, 2, None, 7, &defs, &mut stats);
        assert_eq!(held_by(&st, slowed), vec![Some(7)], "a shorter held refresh still takes the instance over");
        st.apply_held(slowed, 5, None, 8, &defs, &mut stats);
        assert_eq!(held_by(&st, slowed), vec![Some(8)], "held by the newest holder");
        st.apply(slowed, 9, None, &defs, &mut stats);
        assert_eq!(held_by(&st, slowed), vec![Some(8)], "an unheld refresh leaves it held");
        assert_eq!(st.iter().find(|s| s.id == slowed).map(|s| s.turns), Some(9));
        st.apply(burning, 2, None, &defs, &mut stats);
        st.apply_held(burning, 2, None, 7, &defs, &mut stats);
        st.apply(burning, 2, None, &defs, &mut stats);
        assert_eq!(held_by(&st, burning), vec![Some(7)], "extend follows the same rule");
        assert_eq!(st.iter().find(|s| s.id == burning).map(|s| s.turns), Some(6));
    }

    /// A save written before a status could be held still reads.
    #[test]
    fn a_status_saved_without_a_holder_reads_as_held_by_nothing() {
        let old: ActiveStatus = ron::from_str("(id: 3, turns: 4, source: None)").expect("an old status reads");
        assert_eq!(old.held_by, None);
    }

    #[test]
    fn a_status_file_is_written_in_names_and_loaded_as_ids() {
        let (stats, kinds) = vocabulary();
        let names = Names::new().stats(&stats).damage_kinds(&kinds);
        let r = load(
            r#"#![enable(implicit_some)]
            [
                (name: "hasted", modifies: [("speed", MulPct(200))], badge: 'H'),
                (name: "venom", stacking: Extend, ticks: ("bite", 1)),
                (name: "mending", mends: 2),
                (name: "stunned", stacking: Ignore),
            ]"#,
            &names,
        )
        .expect("the file loads");
        let hasted = r.get(r.expect("hasted"));
        assert_eq!(hasted.modifiers, vec![StatusModifier { stat: stats.expect("speed"), op: Op::MulPct(200) }]);
        assert_eq!(hasted.badge, Some('H'));
        assert_eq!(hasted.stacking, Stacking::Refresh, "the default");
        assert_eq!(r.get(r.expect("venom")).tick_damage, Some((kinds.expect("bite"), 1)));
        let mending = r.get(r.expect("mending"));
        assert_eq!((mending.tick_damage, mending.tick_mend), (None, Some(2)), "a mend is not a tick of any kind");
        assert_eq!(r.get(r.expect("stunned")).stacking, Stacking::Ignore);
    }

    /// A tick below zero was how a status mended when healing was damage
    /// of a kind; it is refused with the word that replaced it rather
    /// than loaded as a tick that now does nothing. So is a mend of none.
    #[test]
    fn a_negative_tick_and_a_mend_of_nothing_are_refused_naming_what_to_write() {
        let (stats, kinds) = vocabulary();
        let names = Names::new().stats(&stats).damage_kinds(&kinds);
        let old = load(r#"#![enable(implicit_some)] [(name: "mending", ticks: ("bite", -2))]"#, &names).expect_err("a negative tick is not damage");
        assert!(old.to_string().contains("mends: 2"), "{old}");
        let none = load(r#"#![enable(implicit_some)] [(name: "mending", mends: 0)]"#, &names).expect_err("a mend of nothing");
        assert!(none.to_string().contains("at least 1"), "{none}");
    }

    /// A status that mends is reported as a mend each turn, beside and
    /// apart from the damage another deals.
    #[test]
    fn a_mending_status_reports_a_mend_each_turn_and_no_tick_of_damage() {
        let (_, kinds) = vocabulary();
        let defs = Registry::from_defs(vec![StatusDef::new("mending").mends(2), StatusDef::new("venom").ticks(kinds.expect("bite"), 1)]).unwrap();
        let (mending, venom) = (defs.expect("mending"), defs.expect("venom"));
        let (mut statuses, mut stats) = (Statuses::default(), Stats::default());
        statuses.apply(mending, 2, Some(9), &defs, &mut stats);
        statuses.apply(venom, 2, None, &defs, &mut stats);
        let report = statuses.tick(&defs, &mut stats);
        assert_eq!(report.mends, vec![Mended { status: mending, amount: 2, source: Some(9) }]);
        assert_eq!(report.ticks.iter().map(|t| (t.status, t.amount)).collect::<Vec<_>>(), vec![(venom, 1)]);
    }

    #[test]
    fn a_status_can_be_written_unseen_and_is_seen_unless_it_says_so() {
        let names = Names::new();
        let loaded = load(r#"[(name: "hidden", unseen: true), (name: "dazed")]"#, &names).expect("it loads");
        assert!(loaded.get(loaded.expect("hidden")).unseen);
        assert!(!loaded.get(loaded.expect("dazed")).unseen);
        assert!(StatusDef::new("hidden").unseen().unseen);
    }

    #[test]
    fn a_status_can_be_written_a_boon_and_is_not_one_unless_it_says_so() {
        let names = Names::new();
        let loaded = load(r#"[(name: "hearty", boon: true), (name: "dazed")]"#, &names).expect("it loads");
        assert!(loaded.get(loaded.expect("hearty")).boon);
        assert!(!loaded.get(loaded.expect("dazed")).boon);
        assert!(StatusDef::new("hearty").boon().boon);
    }

    #[test]
    fn a_status_can_be_written_to_hold_its_holder_and_holds_nobody_unless_it_says_so() {
        let names = Names::new();
        let loaded = load(r#"[(name: "snared", holds: true), (name: "dazed")]"#, &names).expect("it loads");
        assert!(loaded.get(loaded.expect("snared")).holds);
        assert!(!loaded.get(loaded.expect("dazed")).holds);
        assert!(StatusDef::new("snared").holds().holds);
    }

    /// Held is asked of what is on the actor now, so it ends the turn the
    /// status does, by its time running out or by a cure, and a status
    /// that does not hold never holds anyone however long it lasts.
    #[test]
    fn an_actor_is_held_while_a_status_that_holds_is_on_it_and_free_once_it_runs_out_or_is_cured() {
        let defs = Registry::from_defs(vec![StatusDef::new("snared").holds(), StatusDef::new("dazed")]).unwrap();
        let (snared, dazed) = (defs.expect("snared"), defs.expect("dazed"));
        let (mut statuses, mut stats) = (Statuses::new(), Stats::new());
        statuses.apply(dazed, UNTIL_CURED, None, &defs, &mut stats);
        assert_eq!(statuses.holding(&defs), None, "a status that does not hold leaves its holder free");
        statuses.apply(snared, 2, None, &defs, &mut stats);
        assert_eq!(statuses.holding(&defs), Some(snared));
        statuses.tick(&defs, &mut stats);
        assert_eq!(statuses.holding(&defs), Some(snared), "a turn left");
        statuses.tick(&defs, &mut stats);
        assert_eq!(statuses.holding(&defs), None, "run out");
        statuses.apply(snared, UNTIL_CURED, None, &defs, &mut stats);
        assert_eq!(statuses.holding(&defs), Some(snared), "put on until cured");
        assert!(statuses.cure(snared, &mut stats));
        assert_eq!(statuses.holding(&defs), None, "and cured");
    }

    #[test]
    fn a_status_can_be_written_to_sleep_and_sleeps_nobody_unless_it_says_so() {
        let names = Names::new();
        let loaded = load(r#"[(name: "asleep", sleeps: true), (name: "snared", holds: true)]"#, &names).expect("it loads");
        assert!(loaded.get(loaded.expect("asleep")).sleeps);
        assert!(!loaded.get(loaded.expect("snared")).sleeps);
        assert!(StatusDef::new("asleep").sleeps().sleeps);
    }

    /// Sleeping is holding that something can end early, so a status that
    /// sleeps takes its holder's turns whether or not it also says it holds.
    #[test]
    fn a_status_that_sleeps_holds_its_holder_without_saying_it_holds() {
        let names = Names::new();
        let loaded = load(r#"[(name: "asleep", sleeps: true)]"#, &names).expect("it loads");
        let asleep = loaded.expect("asleep");
        let (mut statuses, mut stats) = (Statuses::new(), Stats::new());
        statuses.apply(asleep, UNTIL_CURED, None, &loaded, &mut stats);
        assert_eq!(statuses.holding(&loaded), Some(asleep));
        assert_eq!(statuses.sleeping(&loaded), Some(asleep));
    }

    /// Waking takes off every status that sleeps, modifiers and all, and
    /// leaves one that only holds: a snare is not slept off by a shout.
    #[test]
    fn waking_cures_every_status_that_sleeps_and_none_that_only_holds() {
        let stat_defs = Registry::from_defs(vec![StatDef::new("speed", 100)]).unwrap();
        let speed = stat_defs.expect("speed");
        let defs = Registry::from_defs(vec![
            StatusDef::new("asleep").sleeps().modifies(speed, Op::Add(-50)),
            StatusDef::new("dozing").sleeps(),
            StatusDef::new("snared").holds(),
        ])
        .unwrap();
        let (asleep, dozing, snared) = (defs.expect("asleep"), defs.expect("dozing"), defs.expect("snared"));
        let (mut statuses, mut stats) = (Statuses::new(), Stats::new());
        assert!(statuses.wake(&defs, &mut stats).is_empty(), "nothing to wake from");
        statuses.apply(asleep, 5, None, &defs, &mut stats);
        statuses.apply(dozing, UNTIL_CURED, None, &defs, &mut stats);
        statuses.apply(snared, 5, None, &defs, &mut stats);
        assert_eq!(stats.value(speed, &stat_defs), 50);
        assert_eq!(statuses.wake(&defs, &mut stats), vec![asleep, dozing]);
        assert_eq!(statuses.sleeping(&defs), None, "awake");
        assert_eq!(statuses.holding(&defs), Some(snared), "and still snared");
        assert_eq!(stats.value(speed, &stat_defs), 100, "the sleep's modifier left with it");
    }

    /// A capped stack refuses the instance past its cap and nothing else:
    /// none is refreshed in its place, and once one runs out another fits.
    #[test]
    fn a_capped_stack_refuses_an_instance_past_its_cap_until_one_runs_out() {
        let stat_defs = Registry::from_defs(vec![StatDef::new("might", 10)]).unwrap();
        let might = stat_defs.expect("might");
        let defs = Registry::from_defs(vec![StatusDef::new("sapped").modifies(might, Op::Add(-1)).stacking(Stacking::Stack).at_most(3)]).unwrap();
        let sapped = defs.expect("sapped");
        let (mut statuses, mut stats) = (Statuses::new(), Stats::new());
        assert!(statuses.apply(sapped, 1, None, &defs, &mut stats));
        for _ in 0..2 {
            assert!(statuses.apply(sapped, 5, None, &defs, &mut stats));
        }
        assert!(!statuses.apply(sapped, 9, None, &defs, &mut stats), "a fourth is refused");
        assert_eq!(statuses.iter().map(|s| s.turns).collect::<Vec<_>>(), vec![1, 5, 5], "and refreshes nothing");
        assert_eq!(stats.value(might, &stat_defs), 7);
        statuses.tick(&defs, &mut stats);
        assert!(statuses.apply(sapped, 5, None, &defs, &mut stats), "the first ran out, so another fits");
        assert_eq!(stats.value(might, &stat_defs), 7);
    }

    /// What statuses resist adds instance by instance, the first side named
    /// is the one taken, and the keenest stumble is the one that counts.
    #[test]
    fn held_statuses_answer_what_their_holder_resists_its_side_and_how_often_it_stumbles() {
        let (_, kinds) = vocabulary();
        let bite = kinds.expect("bite");
        let (one, two) = (FactionId::from_raw(1), FactionId::from_raw(2));
        let defs = Registry::from_defs(vec![
            StatusDef::new("hide").resists(bite, 10).stacking(Stacking::Stack),
            StatusDef::new("turned").sides_with(one),
            StatusDef::new("charmed").sides_with(two),
            StatusDef::new("dizzy").stumbles(25),
            StatusDef::new("reeling").stumbles(100),
        ])
        .unwrap();
        let (mut statuses, mut stats) = (Statuses::new(), Stats::new());
        assert_eq!(
            (statuses.resistances(&defs), statuses.side(&defs), statuses.stumbles(&defs)),
            (Resistances::new(), None, None),
            "nothing held, nothing lent"
        );
        for name in ["hide", "hide", "turned", "charmed", "dizzy", "reeling"] {
            statuses.apply(defs.expect(name), 3, None, &defs, &mut stats);
        }
        assert_eq!(statuses.resistances(&defs).get(bite), 20);
        assert_eq!(statuses.side(&defs), Some(one));
        assert_eq!(statuses.stumbles(&defs), Some(100));
    }

    #[test]
    fn a_status_file_writes_a_cap_resistances_a_side_and_a_stumble_by_name() {
        let (stats, kinds) = vocabulary();
        let factions = Registry::from_defs(vec![FactionDef::new("crowd"), FactionDef::new("rogue")]).unwrap();
        let names = Names::new().stats(&stats).damage_kinds(&kinds).with("faction", &factions);
        let r = load(
            r#"#![enable(implicit_some)]
            [
                (name: "sapped", stacking: Stack, max: 3),
                (name: "thick-skinned", resists: [("bite", 50), ("bite", 25)]),
                (name: "turned", side: "rogue"),
                (name: "dizzy", stumbles: 100),
                (name: "plain"),
            ]"#,
            &names,
        )
        .expect("the file loads");
        assert_eq!(r.get(r.expect("sapped")).max, Some(3));
        assert_eq!(r.get(r.expect("thick-skinned")).resists.get(kinds.expect("bite")), 75);
        assert_eq!(r.get(r.expect("turned")).side, Some(factions.expect("rogue")));
        assert_eq!(r.get(r.expect("dizzy")).stumbles, Some(100));
        let plain = r.get(r.expect("plain"));
        assert_eq!((plain.max, &plain.resists, plain.side, plain.stumbles), (None, &Resistances::new(), None, None), "absent is no mechanic");
    }

    /// A cap where nothing stacks, a cap of nothing and a stumble that is
    /// no percent are each refused, with every unknown name beside them.
    #[test]
    fn a_cap_nothing_stacks_under_a_stumble_out_of_range_and_unknown_names_are_refused() {
        let (stats, kinds) = vocabulary();
        let factions = Registry::from_defs(vec![FactionDef::new("crowd")]).unwrap();
        let names = Names::new().stats(&stats).damage_kinds(&kinds).with("faction", &factions);
        let bad = r#"#![enable(implicit_some)]
        [
            (name: "a", max: 2),
            (name: "b", stacking: Stack, max: 0),
            (name: "c", stumbles: 0),
            (name: "d", stumbles: 101),
            (name: "e", resists: [("frost", 10)], side: "nobody"),
        ]"#;
        let Err(ContentError::Invalid(errs)) = load(bad, &names) else { panic!("a bad file loaded") };
        assert_eq!(errs.len(), 6, "{errs:#?}");
    }

    #[test]
    fn every_unknown_name_in_a_status_file_is_reported_at_once() {
        let (stats, kinds) = vocabulary();
        let names = Names::new().stats(&stats).damage_kinds(&kinds);
        let bad = r#"#![enable(implicit_some)]
        [
            (name: "cursed", modifies: [("luck", Add(-1)), ("wisdom", Add(-1))]),
            (name: "burning", ticks: ("fire", 2)),
            (name: "fine"),
        ]"#;
        let Err(ContentError::Invalid(errs)) = load(bad, &names) else { panic!("a file of unknown names loaded") };
        assert_eq!(errs.len(), 3, "{errs:#?}");
        assert!(errs.contains(&"cursed: unknown stat \"luck\"".to_string()), "{errs:#?}");
        assert!(errs.contains(&"burning: unknown damage kind \"fire\"".to_string()), "{errs:#?}");
    }

    #[test]
    fn a_stumble_draws_nothing_when_no_status_stumbles_or_there_is_nowhere_to_go() {
        use rand::{SeedableRng, rngs::StdRng};
        let open = [Direction::North, Direction::East];
        let (mut a, mut b) = (StdRng::seed_from_u64(7), StdRng::seed_from_u64(7));
        assert_eq!(stumble(None, &open, &mut a), None, "no status stumbles");
        assert_eq!(stumble(Some(100), &[], &mut a), None, "certain, but walled in");
        assert_eq!(stumble(Some(50), &[], &mut a), None, "walled in before the percent is rolled");
        assert_eq!(a.random::<u64>(), b.random::<u64>());
    }

    #[test]
    fn a_stumble_that_never_happens_still_spends_the_one_draw_that_says_so() {
        use rand::{SeedableRng, rngs::StdRng};
        let open = [Direction::North, Direction::East];
        let (mut a, mut b) = (StdRng::seed_from_u64(9), StdRng::seed_from_u64(9));
        assert_eq!(stumble(Some(0), &open, &mut a), None);
        b.random_range(0..100);
        assert_eq!(a.random::<u64>(), b.random::<u64>(), "exactly the percent's draw");
    }

    #[test]
    fn a_certain_stumble_spends_one_draw_on_the_pick_and_a_chancy_one_rolls_the_percent_first() {
        use rand::{SeedableRng, rngs::StdRng};
        let open = [Direction::North, Direction::East, Direction::South];
        let (mut a, mut b) = (StdRng::seed_from_u64(3), StdRng::seed_from_u64(3));
        assert_eq!(stumble(Some(100), &open, &mut a), Some(open[b.random_range(0..3)]));
        assert_eq!(a.random::<u64>(), b.random::<u64>(), "one draw, for the pick");
        for seed in 0..40 {
            let (mut a, mut b) = (StdRng::seed_from_u64(seed), StdRng::seed_from_u64(seed));
            let gate = b.random_range(0..100) >= 50;
            let want = if gate { None } else { Some(open[b.random_range(0..3)]) };
            assert_eq!(stumble(Some(50), &open, &mut a), want);
            assert_eq!(a.random::<u64>(), b.random::<u64>());
        }
    }

    #[test]
    fn a_stumble_that_goes_astray_only_some_of_the_time_does_so_on_some_steps_and_not_others() {
        use rand::{SeedableRng, rngs::StdRng};
        let open = [Direction::North, Direction::East];
        let mut rng = StdRng::seed_from_u64(1);
        let astray = (0..200).filter(|_| stumble(Some(50), &open, &mut rng).is_some()).count();
        assert!(astray > 0 && astray < 200, "{astray} of 200");
    }

    #[test]
    fn harm_wakes_a_sleeper_unless_it_is_its_own_sleeps_tick_or_landed_the_pass_it_fell_asleep() {
        assert!(wakes_from_harm(false, false));
        assert!(!wakes_from_harm(true, false), "a sleep that bites does not wake its holder");
        assert!(!wakes_from_harm(false, true), "a wound and a sleep landed together leave it asleep");
        assert!(!wakes_from_harm(true, true));
    }

    #[test]
    fn a_holder_is_turned_once_and_given_its_own_side_back_when_the_last_status_ends() {
        let (ours, theirs, other) = (FactionId::from_raw(0), FactionId::from_raw(1), FactionId::from_raw(2));
        assert_eq!(side_change(Some(theirs), ours, None), Some((theirs, Some(ours))), "turned, its side kept");
        assert_eq!(side_change(Some(theirs), theirs, Some(ours)), None, "turned only once");
        assert_eq!(side_change(Some(theirs), other, Some(ours)), None, "moved by another rule, left where it is");
        assert_eq!(side_change(None, theirs, Some(ours)), Some((ours, None)), "given back");
        assert_eq!(side_change(None, ours, None), None, "nothing held, nothing kept");
    }
}
