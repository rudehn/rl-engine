//! Stealth: who has noticed whom, kept on the observers.
//!
//! Opt-in. Without [`StealthPlugin`] no actor carries an [`Aware`], and a
//! mind acts on everything its own sight reaches exactly as it always
//! has. With it, a subject carrying
//! [`Stealth`] enters an observer's list of enemies only once that observer
//! has noticed it, and an observer that loses the trail searches where it
//! last saw something before it forgets.
//!
//! Both sides have to be authored before anything changes. A [`Notice`]
//! absent means the observer sees on sight, which is the old behaviour; a
//! [`Stealth`] absent means the subject never hides. That is the right way
//! round, and the likely first report is "I added the plugin and nothing
//! happened".
//!
//! A status registered as `unseen` hides its holder from everything, adjacency
//! included, until it runs out or its holder attacks: see [`Unseen`]. Noise
//! is still heard, since hearing is not sight.
//!
//! Awareness ticks in [`DecideSet::Notice`](crate::plugin::DecideSet::Notice),
//! for the actor holding the turn and only that one, so it costs a roll per
//! subject per monster-turn and nothing per frame.

use std::collections::BTreeMap;

use bevy::prelude::*;
use rand::Rng;
use rl_core::Point;
use rl_rules::ai::awareness::{self, Awareness, NoticeStats, StealthStats};

use crate::combat::{CombatRules, DamageDealt, Dead, Faction};
use crate::components::{MyTurn, Player, Position, Viewshed};
use crate::minds::{DEFAULT_PERCEPTION, Mind, Perception, Thinking};
use crate::places::{MapId, OnMap};
use crate::registries::Registries;
use crate::status::{Afflicted, Cure};
use crate::world::WorldMap;

/// How this actor notices what is trying not to be seen.
///
/// Brings an [`Aware`] with it, so an observer is ready to remember the
/// moment it is spawned.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default, Deref, DerefMut)]
#[require(Aware)]
pub struct Notice(pub NoticeStats);

/// How hard this actor is to notice.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default, Deref, DerefMut)]
pub struct Stealth(pub StealthStats);

/// Nothing sees this actor: it holds a status registered as `unseen`.
///
/// Kept by [`mark_unseen`] from the statuses, never inserted by hand in a
/// game, so it lasts exactly as long as the status and a continued run
/// gets it back from the statuses it saved. While it is on, no mind
/// perceives its holder at any distance and no observer notices it. That
/// departs on purpose from the rule that no stack of [`Stealth`] makes
/// somebody standing next to you invisible: quiet is for good, and this
/// lasts turns and ends the moment its holder strikes.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Unseen;

/// What this observer knows about each subject it has an opinion of.
///
/// Keyed only by entities carrying [`Stealth`], which in most games is the
/// player alone, so this is a map for correctness and one entry in
/// practice. A `BTreeMap` because it is read on the decision path and
/// iterated in a fixed order.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct Aware(pub BTreeMap<Entity, Awareness>);

impl Aware {
    /// What it knows of `subject`.
    pub fn of(&self, subject: Entity) -> Awareness {
        self.0.get(&subject).copied().unwrap_or_default()
    }

    /// Whether it has noticed `subject` and not yet forgotten.
    pub fn knows(&self, subject: Entity) -> bool {
        self.of(subject).is_alert()
    }
}

/// An observer has just noticed a subject it was unaware of.
///
/// Written once, on the flip, not on every turn the awareness holds. A game
/// reads it to shout, log a line, change the music, or wake a squad; how
/// far a shout carries and who it reaches are content, so the engine
/// propagates nothing.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Noticed {
    /// Who noticed.
    pub observer: Entity,
    /// Whom.
    pub subject: Entity,
    /// Where.
    pub at: Point,
}

/// Whether [`StealthPlugin`] was added, for the systems outside this module
/// that read awareness.
///
/// Asked of the plugin rather than of the components, because [`Notice`]
/// brings an [`Aware`] with it: a game that authors observers and never adds
/// the plugin would otherwise get monsters that notice nothing, forever,
/// instead of the old see-on-sight behaviour the opt-in promises. The
/// plugin's own message is the proof it was added.
#[derive(bevy::ecs::system::SystemParam)]
pub struct StealthRunning<'w> {
    noticed: Option<Res<'w, Messages<Noticed>>>,
}

impl StealthRunning<'_> {
    /// Whether stealth is running.
    pub fn get(&self) -> bool {
        self.noticed.is_some()
    }
}

/// Anything that can notice a subject: a mind, or an authored observer.
type WatcherData = (
    Entity,
    &'static Position,
    Option<&'static Faction>,
    Option<&'static Perception>,
    Option<&'static Viewshed>,
    Option<&'static Aware>,
    Option<&'static OnMap>,
);

/// What counts as a watcher: something that decides, or something authored
/// to notice, that is neither the player nor dead.
type CanWatch = (Or<(With<Mind>, With<Notice>)>, Without<Player>, Without<Dead>);
/// One watcher, as the query hands it back.
type Watcher<'a> = (Entity, &'a Position, Option<&'a Faction>, Option<&'a Perception>, Option<&'a Viewshed>, Option<&'a Aware>, Option<&'a OnMap>);

/// Who is watching whom right now, by the rule the minds act on.
///
/// Stealth only hides a subject from observers that keep an [`Aware`]. One
/// that does not - a monster with no [`Notice`] - sees on sight, exactly as
/// it did before stealth existed, and it will attack a hider it can see.
/// Asking only the `Aware` keepers whether a player has been seen therefore
/// answers "hidden" while such a monster cuts the player down, which is how
/// this came to exist. The rule here is the one the minds perceive by: an
/// observer that keeps an `Aware` watches what it knows about, alert or
/// searching; one that does not watches whatever its own sight reaches,
/// read off the same [`Viewshed`] its turns are decided from. Nobody
/// watches the [`Unseen`], since no mind perceives them.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Watchers<'w, 's> {
    running: StealthRunning<'w>,
    watchers: Query<'w, 's, WatcherData, CanWatch>,
    subjects: Query<'w, 's, (&'static Position, Option<&'static Faction>, Option<&'static OnMap>)>,
    map: Option<Res<'w, WorldMap>>,
    rules: Option<Res<'w, CombatRules>>,
    unseen: Query<'w, 's, (), With<Unseen>>,
}

impl Watchers<'_, '_> {
    /// Whether stealth is running at all. Without it nothing is hidden and
    /// "seen" is not a question worth asking.
    pub fn running(&self) -> bool {
        self.running.get()
    }

    /// Whether `entity` is something that notices.
    pub fn is_watcher(&self, entity: Entity) -> bool {
        self.watchers.contains(entity)
    }

    /// Whether `watcher` is watching `subject` right now.
    pub fn sees(&self, watcher: Entity, subject: Entity) -> bool {
        self.watchers.get(watcher).is_ok_and(|w| self.judge(w, subject))
    }

    /// Whether anything at odds with `subject` is watching it.
    pub fn watched(&self, subject: Entity) -> bool {
        self.watchers.iter().any(|w| self.judge(w, subject))
    }

    fn judge(&self, (watcher, pos, faction, perception, sight, aware, on): Watcher<'_>, subject: Entity) -> bool {
        if watcher == subject || self.unseen.contains(subject) {
            return false;
        }
        let (Some(map), Ok((at, theirs, subject_on))) = (self.map.as_deref(), self.subjects.get(subject)) else {
            return false;
        };
        // Only what is at odds with the subject: an ally looking on is not
        // being seen by an enemy. With no sides, everyone is at odds.
        if !at_odds(self.rules.as_deref(), faction, theirs) {
            return false;
        }
        if let Some(aware) = aware {
            return aware.knows(subject);
        }
        let here = map.current();
        if on.map(|m| m.0).unwrap_or(MapId::SURFACE) != here || subject_on.map(|m| m.0).unwrap_or(MapId::SURFACE) != here {
            return false;
        }
        awareness::within_reach(pos.0, at.0, perception.map(|p| p.0).unwrap_or(DEFAULT_PERCEPTION)) && sight.is_some_and(|s| s.can_see(at.0))
    }
}

/// Whether `mine` has anything against `theirs`: hostile by the rules
/// when there are rules and both take a side, and otherwise yes, since a
/// game with no sides has nobody to be friends with.
fn at_odds(rules: Option<&CombatRules>, mine: Option<&Faction>, theirs: Option<&Faction>) -> bool {
    match (rules, mine, theirs) {
        (Some(rules), Some(mine), Some(theirs)) => rules.factions.is_hostile(mine.0, theirs.0),
        _ => true,
    }
}

/// Stealth's own stream for the notice roll, so a tactic added to the
/// minds or a blow struck elsewhere cannot change who gets noticed.
#[derive(Resource, Debug)]
pub struct StealthRng(pub rand::rngs::StdRng);

impl crate::seed::Stream for StealthRng {
    fn for_run(seed: rl_core::RunSeed) -> Self {
        Self(seed.rng(rl_core::SeedDomain::new(b"stealth"), 0))
    }
}

/// Adds noticing, forgetting, being woken by a blow, and the unseen.
pub struct StealthPlugin;

impl Plugin for StealthPlugin {
    fn build(&self, app: &mut App) {
        use crate::plugin::{DecideSet, PerceiveSet, Reads, ResolveSet, Turn, TurnSet};
        use crate::seed::AddStream;
        // The blows it wakes on, registered here as well so a game with
        // stealth and no combat has nothing to wake on rather than a panic.
        app.add_message::<Noticed>()
            .add_message::<DamageDealt>()
            .add_message::<crate::accuracy::Missed>()
            // What ending the unseen reads and writes, registered here so a
            // game with stealth and no items, abilities or statuses has
            // nothing to reveal on rather than a panic.
            .add_message::<Cure>()
            .reads::<crate::combat::Struck>()
            .reads::<crate::items::ItemEvent>()
            .reads::<crate::ability::AbilityEvent>()
            .add_stream::<StealthRng>("StealthPlugin")
            .add_systems(Turn, update_awareness.in_set(DecideSet::Notice))
            .add_systems(Turn, (filter_unseen, filter_unnoticed).chain().in_set(PerceiveSet::Filter))
            .add_systems(Turn, wake_on_damage.in_set(TurnSet::React))
            .add_systems(Turn, reveal_attackers.in_set(ResolveSet::Effects).before(crate::status::resolve_afflictions))
            .add_systems(Turn, mark_unseen.in_set(ResolveSet::Effects).after(crate::status::tick_statuses));
    }

    fn finish(&self, app: &mut App) {
        crate::plugin::depends_on::<crate::minds::MindsPlugin>(app, "StealthPlugin");
    }
}

/// Puts [`Unseen`] on whoever holds an unseen status and takes it off
/// whoever no longer does.
///
/// In [`ResolveSet::Effects`](crate::plugin::ResolveSet::Effects) after the
/// statuses are applied, ticked and cured, so the pass the status goes on
/// is the pass its holder vanishes, and the pass a blow ends it is the pass
/// it is seen again.
pub fn mark_unseen(mut commands: Commands, registries: Option<Res<Registries>>, actors: Query<(Entity, &Afflicted, Has<Unseen>), Changed<Afflicted>>) {
    let Some(registries) = registries else { return };
    for (actor, afflicted, marked) in &actors {
        let unseen = afflicted.0.iter().any(|s| registries.statuses.get(s.id).unseen);
        match (unseen, marked) {
            (true, false) => {
                commands.entity(actor).insert(Unseen);
            }
            (false, true) => {
                commands.entity(actor).remove::<Unseen>();
            }
            _ => {}
        }
    }
}

/// Ends every unseen status on whoever made an attack this pass: a blow or
/// a shot, landed or not, a throw, or an ability landed on anyone but
/// themselves.
///
/// Before the statuses resolve in the same pass, so the cure lands before
/// the damage does and the one struck wakes to an attacker it can see.
/// Being hurt is not here: a grenade in the dark does not light you up.
pub fn reveal_attackers(
    mut struck: MessageReader<crate::combat::Struck>,
    mut items: MessageReader<crate::items::ItemEvent>,
    mut abilities: MessageReader<crate::ability::AbilityEvent>,
    registries: Option<Res<Registries>>,
    unseen: Query<&Afflicted, With<Unseen>>,
    mut cure: MessageWriter<Cure>,
) {
    let mut attackers: Vec<Entity> = struck.read().map(|s| s.attacker).collect();
    attackers.extend(items.read().filter_map(|e| match e {
        crate::items::ItemEvent::Thrown { actor, .. } => Some(*actor),
        _ => None,
    }));
    attackers.extend(abilities.read().filter_map(|e| match e {
        crate::ability::AbilityEvent::Used { user, targets, .. } if targets.iter().any(|t| t != user) => Some(*user),
        _ => None,
    }));
    let Some(registries) = registries else { return };
    attackers.sort();
    attackers.dedup();
    for who in attackers {
        let Ok(afflicted) = unseen.get(who) else { continue };
        for status in afflicted.0.iter().filter(|s| registries.statuses.get(s.id).unseen) {
            cure.write(Cure { target: who, status: status.id });
        }
    }
}

/// Takes the unseen out of what the mind holding the turn perceives,
/// enemies, allies and others alike.
///
/// In [`PerceiveSet::Filter`](crate::plugin::PerceiveSet::Filter) before
/// [`filter_unnoticed`], so a mind that was alert to a subject that has
/// just vanished is offered the trail to where it last saw it, as for
/// anything else out of sight. Every mind, noticing or not: a monster that
/// sees on sight sees nothing here either.
pub fn filter_unseen(mut thinking: ResMut<Thinking>, unseen: Query<(), With<Unseen>>) {
    let Some(snapshot) = thinking.snapshot_mut() else { return };
    snapshot.enemies.retain(|e| !unseen.contains(e.id));
    snapshot.allies.retain(|e| !unseen.contains(e.id));
    snapshot.others.retain(|e| !unseen.contains(e.id));
}

/// Takes the hiders the mind holding the turn has not noticed out of its
/// enemies, and offers every trail it is on for its search to follow.
///
/// Stealth's contribution to a mind's knowledge, in
/// [`PerceiveSet::Filter`](crate::plugin::PerceiveSet::Filter), after
/// combat put everyone it could see in. A mind that keeps no [`Aware`] sees
/// on sight and is left alone.
pub fn filter_unnoticed(mut thinking: ResMut<Thinking>, aware: Query<&Aware>, hidden: Query<(), With<Stealth>>) {
    let Some(thinker) = thinking.actor() else { return };
    let Ok(aware) = aware.get(thinker) else { return };
    let Some(snapshot) = thinking.snapshot_mut() else { return };
    snapshot.enemies.retain(|e| !hidden.contains(e.id) || aware.knows(e.id));
    let lost: Vec<(u32, Point)> = aware
        .0
        .iter()
        .filter(|(subject, _)| !snapshot.enemies.iter().any(|e| e.id == **subject))
        .filter_map(|(_, state)| Some((state.stale_turns()?, state.last_known()?)))
        .collect();
    for (stale, at) in lost {
        thinking.offer_trail(at, stale);
    }
}

/// The observer holding the turn.
type Observer =
    (Entity, &'static Position, &'static Notice, &'static mut Aware, Option<&'static Perception>, Option<&'static Viewshed>, Option<&'static Faction>);
/// Anything that might be hiding from it.
type Subject = (Entity, &'static Position, &'static Stealth, Option<&'static Faction>, Option<&'static OnMap>, Has<Unseen>);
/// One subject, as the query hands it back.
type Hiding<'a> = (Entity, &'a Position, &'a Stealth, Option<&'a Faction>, Option<&'a OnMap>, bool);

/// Everything noticing reads.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Watch<'w, 's> {
    observers: Query<'w, 's, Observer, (With<MyTurn>, Without<Player>)>,
    subjects: Query<'w, 's, Subject>,
    lighting: Option<Res<'w, crate::lighting::Lighting>>,
    map: Res<'w, WorldMap>,
    /// Who is at odds with whom; absent, everyone is.
    rules: Option<Res<'w, CombatRules>>,
    rng: ResMut<'w, StealthRng>,
}

/// Rolls to notice, for the observer holding the turn, and ages what it
/// failed to see.
///
/// A roll only decides whether an unaware observer becomes aware. One that
/// is already alert keeps its subject for as long as it can perceive it, so
/// a monster in plain view of you does not lose you to a bad roll; losing
/// takes `memory` turns out of sight and noticing takes one, and that
/// asymmetry is the whole of the hysteresis.
pub fn update_awareness(mut watch: Watch, mut noticed: MessageWriter<Noticed>) {
    let here = watch.map.current();
    let lighting = watch.lighting.as_deref();
    let rules = watch.rules.as_deref();
    let Ok((observer, pos, notice, mut aware, perception, sight, faction)) = watch.observers.single_mut() else { return };
    let reach = perception.map(|p| p.0).unwrap_or(DEFAULT_PERCEPTION);
    let mut seen = Vec::new();
    // In spawn order rather than the order the query walks the archetypes
    // in, since each subject in view costs a roll and which subject gets
    // which roll must not depend on how the world happens to be laid out.
    let mut subjects: Vec<Hiding<'_>> = watch.subjects.iter().collect();
    subjects.sort_by_key(|(e, ..)| (e.index(), *e));
    for (subject, at, stealth, theirs, on, hidden) in subjects {
        if subject == observer {
            continue;
        }
        // Only what it is at odds with, so an ally slipping past it does
        // not make the vitals strip say the player has been seen.
        if !at_odds(rules, faction, theirs) {
            continue;
        }
        let on_this_map = on.map(|m| m.0).unwrap_or(MapId::SURFACE) == here;
        let in_view = !hidden && on_this_map && awareness::within_reach(pos.0, at.0, reach) && sight.is_some_and(|s| s.can_see(at.0));
        let mut state = aware.of(subject);
        if in_view && state.is_alert() {
            state.saw(at.0);
        } else if in_view {
            // Lit means the lit band, not merely seen: a subject in a
            // lamp's dim ring is seen, and is harder to pick out there.
            let lit = crate::lighting::band_at(lighting, at.0) == rl_grid::LightBand::Lit;
            let roll = watch.rng.0.random_range(0..100);
            let distance = rl_core::geometry::chebyshev(pos.0, at.0);
            if awareness::notices(distance, &notice.0, &stealth.0, lit, roll) && state.saw(at.0) {
                noticed.write(Noticed { observer, subject, at: at.0 });
            }
        } else {
            state.lost(notice.memory);
        }
        seen.push(subject);
        if state.is_alert() {
            aware.0.insert(subject, state);
        } else {
            aware.0.remove(&subject);
        }
    }
    // A subject that is gone - dead, or no longer hiding - is not a
    // subject, and a stale entry would keep a monster searching for it.
    aware.0.retain(|subject, _| seen.contains(subject));
}

/// Wakes whoever takes a blow from something hiding, or is missed by one, and
/// points them at it.
///
/// Being hit is noticing, however quiet the attacker was. In
/// [`TurnSet::React`](crate::plugin::TurnSet::React) because that is where
/// a turn's consequences land, and inside the pass, so the monster that was
/// struck already knows where from when it next decides.
///
/// Not by an attacker still [`Unseen`]: a blow has ended that before the
/// damage lands, so what is left is harm credited to it that was no
/// attack, a status it put on earlier ticking, and that gives nothing
/// away.
pub fn wake_on_damage(
    mut dealt: MessageReader<DamageDealt>,
    mut missed: MessageReader<crate::accuracy::Missed>,
    mut observers: Query<&mut Aware>,
    attackers: Query<&Position, (With<Stealth>, Without<Unseen>)>,
    mut noticed: MessageWriter<Noticed>,
) {
    let mut wake = |target: Entity, attacker: Entity| {
        let (Ok(mut aware), Ok(at)) = (observers.get_mut(target), attackers.get(attacker)) else { return };
        let mut state = aware.of(attacker);
        if state.alerted_to(at.0) {
            noticed.write(Noticed { observer: target, subject: attacker, at: at.0 });
        }
        aware.0.insert(attacker, state);
    };
    for ev in dealt.read() {
        // A mend is a negative hit down the same pipeline, and a hider who
        // patches a sleeper up has not struck it, whole or not. A blow that
        // armor stopped at zero still woke it.
        if ev.is_mend() {
            continue;
        }
        let Some(attacker) = ev.hit.attacker else { continue };
        wake(ev.target, attacker);
    }
    // A blow or a shot that went wide was still aimed at it, and is at
    // least as plain as one armor stopped: otherwise a hider could fire at
    // a sleeper until one landed.
    for m in missed.read() {
        wake(m.target, m.attacker);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::combat::{Armor, CombatPlugin, Health, MeleeAttack};
    use crate::components::{Actor, Blocks, RevealsMap};
    use crate::fov::FovPlugin;
    use crate::minds::MindsPlugin;
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use crate::turn::{Intent, Wait};
    use crate::world::StreamingPlugin;
    use rl_core::DiceRoll;
    use rl_rules::ai::tactics::{Hunt, MeleeAdjacent, SearchLastKnown, Wander};
    use rl_rules::{Brain, Hit};

    /// An open field, a quiet player, and a watcher `gap` tiles east of it
    /// that hunts what it notices and waits otherwise.
    struct Field {
        app: App,
        player: Entity,
        watcher: Entity,
    }

    impl Field {
        /// With `light`, lighting is on and the whole field stands under an
        /// ambient of that intensity and nothing else.
        fn new(notice: NoticeStats, gap: i32, reach: i32, plugin: bool, light: Option<u8>) -> Field {
            Field::build(notice, gap, reach, plugin, light, |_| {})
        }

        /// The field, with `extra` run on the app after the field's own
        /// plugins are added and before anything is spawned or updated.
        fn build(notice: NoticeStats, gap: i32, reach: i32, plugin: bool, light: Option<u8>, extra: impl FnOnce(&mut App)) -> Field {
            let mut app = headless_app();
            app.add_plugins((FovPlugin, CombatPlugin, MindsPlugin, StreamingPlugin));
            if plugin {
                app.add_plugins(StealthPlugin);
            }
            if light.is_some() {
                app.add_plugins(crate::lighting::LightingPlugin);
            }
            extra(&mut app);
            let start = crate::testing::surface(&mut app);
            let crate::testing::Sides { ours: you, theirs: them, kind } = crate::testing::two_sides(&mut app);
            let player = app
                .world_mut()
                .spawn((
                    (Actor, Player, Blocks, Position(start), Viewshed::new(16), RevealsMap),
                    (Health::full(100), Armor(0), Faction(you), Stealth(StealthStats::default())),
                ))
                .id();
            let brain = Brain::new().then(MeleeAdjacent).then(Hunt).then(SearchLastKnown).then(Wander { chance_pct: 0 });
            let watcher = app
                .world_mut()
                .spawn((
                    (Actor, Blocks, Position(start.offset(gap, 0)), Health::full(100), Armor(0), Faction(them)),
                    (MeleeAttack::new(kind, DiceRoll::flat(0)), Perception(reach), Mind(Arc::new(brain)), Notice(notice)),
                ))
                .id();
            app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
            app.update();
            // After play begins, since a new run resets lighting to dark.
            if let Some(intensity) = light {
                app.world_mut().resource_mut::<crate::lighting::Lighting>().ambient = rl_grid::Light::white(intensity);
            }
            app.update();
            Field { app, player, watcher }
        }

        /// The player waits a whole turn and everyone else takes theirs.
        fn wait(&mut self) {
            self.app.world_mut().write_message(Intent::new(self.player, Wait));
            self.app.update();
        }

        fn at(&self, e: Entity) -> Point {
            self.app.world().get::<Position>(e).unwrap().0
        }

        fn distance(&self) -> i32 {
            rl_core::geometry::chebyshev(self.at(self.player), self.at(self.watcher))
        }

        fn aware(&self) -> Aware {
            self.app.world().get::<Aware>(self.watcher).cloned().unwrap_or_default()
        }
    }

    fn blind() -> NoticeStats {
        NoticeStats { certain: 1, chance_pct: 0, lit_bonus: 0, memory: 3 }
    }

    fn keen() -> NoticeStats {
        NoticeStats { certain: 1, chance_pct: 100, lit_bonus: 0, memory: 3 }
    }

    /// A watcher whose certain radius only reaches the player in light.
    fn light_hunter() -> NoticeStats {
        NoticeStats { certain: 1, chance_pct: 0, lit_bonus: 10, memory: 3 }
    }

    #[test]
    fn a_watcher_gets_its_light_bonus_in_the_lit_band_and_not_in_the_dim_one() {
        // 40 is above the seen threshold of 16 and below bright at 64: the
        // player is seen but dim, so the bonus of ten does not reach five
        // tiles and a watcher that never rolls a chance never notices.
        let mut dim = Field::new(light_hunter(), 5, 10, true, Some(40));
        for _ in 0..3 {
            dim.wait();
        }
        assert!(!dim.aware().knows(dim.player), "dim light hides the player from a watcher that sees by light");

        let mut lit = Field::new(light_hunter(), 5, 10, true, Some(100));
        lit.wait();
        assert!(lit.aware().knows(lit.player), "in the lit band the bonus of ten covers five tiles");
    }

    #[test]
    fn a_hider_nobody_noticed_is_left_alone_and_one_they_did_is_hunted() {
        let mut quiet = Field::new(blind(), 5, 10, true, None);
        for _ in 0..4 {
            quiet.wait();
        }
        assert_eq!(quiet.distance(), 5, "an unnoticed player is not approached");
        assert!(!quiet.aware().knows(quiet.player));

        let mut loud = Field::new(keen(), 5, 10, true, None);
        for _ in 0..3 {
            loud.wait();
        }
        assert!(loud.distance() < 5, "a noticed player is hunted: {}", loud.distance());
        assert!(loud.aware().knows(loud.player));
    }

    #[test]
    fn without_the_plugin_an_authored_observer_still_sees_on_sight() {
        let mut field = Field::new(blind(), 5, 10, false, None);
        for _ in 0..3 {
            field.wait();
        }
        assert!(field.distance() < 5, "no StealthPlugin means the old behaviour, Notice or not: {}", field.distance());
    }

    #[test]
    fn a_blow_wakes_an_observer_that_never_saw_it_coming() {
        let mut field = Field::new(blind(), 5, 10, true, None);
        field.wait();
        assert!(!field.aware().knows(field.player));
        let (watcher, player) = (field.watcher, field.player);
        let kind = field.app.world().resource::<crate::registries::Registries>().damage_kinds.expect("kinetic");
        field.app.world_mut().write_message(DamageDealt { target: watcher, hit: Hit::by(player, kind, 1), dealt: 1, reach: crate::combat::Reach::Melee });
        field.wait();
        assert!(field.aware().knows(player), "struck, so it knows where from");
        field.wait();
        assert!(field.distance() < 5, "and goes for it: {}", field.distance());
    }

    /// A heal is a negative hit down the same pipeline, and one that found
    /// its target whole restored nothing; neither makes it a blow.
    #[test]
    fn a_heal_on_a_sleeper_at_full_health_does_not_wake_it() {
        let mut field = Field::new(blind(), 5, 10, true, None);
        field.wait();
        let (watcher, player) = (field.watcher, field.player);
        let kind = field.app.world().resource::<crate::registries::Registries>().damage_kinds.expect("kinetic");
        field.app.world_mut().write_message(DamageDealt { target: watcher, hit: Hit::by(player, kind, -3), dealt: 0, reach: crate::combat::Reach::Melee });
        field.wait();
        assert!(!field.aware().knows(player), "patched up while whole, and none the wiser");
    }

    #[test]
    fn a_blow_that_misses_wakes_an_observer_as_surely_as_one_that_lands() {
        let mut field = Field::new(blind(), 5, 10, true, None);
        field.wait();
        assert!(!field.aware().knows(field.player));
        let (watcher, player) = (field.watcher, field.player);
        field.app.world_mut().write_message(crate::accuracy::Missed { attacker: player, target: watcher, with: None, reach: crate::combat::Reach::Shot });
        field.wait();
        assert!(field.aware().knows(player), "shot at and missed, so it knows where from");
    }

    #[test]
    fn a_lost_trail_is_walked_to_and_then_forgotten() {
        // Reach three and a gap of eight: the player is out of range, so
        // every turn is a turn without a sighting.
        let mut field = Field::new(blind(), 8, 3, true, None);
        let (watcher, player) = (field.watcher, field.player);
        let start = field.at(watcher);
        let rumour = start.offset(-3, 0);
        field.app.world_mut().get_mut::<Aware>(watcher).unwrap().0.insert(player, Awareness::Alert { at: rumour, stale_turns: 0 });
        field.wait();
        assert_eq!(field.at(watcher), start.offset(-1, 0), "it heads for where the player was last seen");
        for _ in 0..3 {
            field.wait();
        }
        assert!(!field.aware().knows(player), "and forgets once its memory of three turns runs out");
        let resting = field.at(watcher);
        field.wait();
        field.wait();
        assert_eq!(field.at(watcher), resting, "then waits, having nothing left to look for");
    }

    /// Whether the player was watched, as the vitals strip would ask.
    #[derive(Resource, Default)]
    struct Watched(Option<bool>);

    fn read_watched(watchers: Watchers, player: Query<Entity, With<Player>>, mut out: ResMut<Watched>) {
        out.0 = player.single().ok().map(|p| watchers.watched(p));
    }

    #[test]
    fn a_watcher_that_sees_on_sight_counts_and_an_observer_that_has_not_noticed_does_not() {
        let mut field = Field::new(blind(), 2, 10, true, None);
        field.app.init_resource::<Watched>().add_systems(PostUpdate, read_watched);
        field.wait();
        assert_eq!(field.app.world().resource::<Watched>().0, Some(false), "two tiles off and never noticed: not watching");

        // The same monster with no Notice sees on sight, the way every
        // surface monster in Corsair does, and it is watching.
        let watcher = field.watcher;
        field.app.world_mut().entity_mut(watcher).remove::<(Notice, Aware)>();
        field.wait();
        assert_eq!(field.app.world().resource::<Watched>().0, Some(true));
    }

    /// How many `Noticed` messages have been read, by a reader that sees
    /// each exactly once however long the buffer keeps it.
    #[derive(Resource, Default)]
    struct Heard(usize);

    fn listen(mut noticed: MessageReader<Noticed>, mut heard: ResMut<Heard>) {
        heard.0 += noticed.read().count();
    }

    #[test]
    fn noticed_is_written_once_on_the_flip_and_not_while_the_awareness_holds() {
        let mut field = Field::new(keen(), 6, 10, true, None);
        field.app.init_resource::<Heard>().add_systems(Update, listen);
        for _ in 0..5 {
            field.wait();
        }
        assert!(field.aware().knows(field.player), "still aware after five turns");
        assert_eq!(field.app.world().resource::<Heard>().0, 1, "one flip, one message");
    }

    /// How many blows have been swung at the player, counted as they are
    /// written, since the watcher's blow is `flat(0)` and health cannot
    /// show one.
    #[derive(Resource, Default)]
    struct Swings(usize);

    fn count_swings(mut struck: MessageReader<crate::combat::Struck>, player: Query<(), With<Player>>, mut swings: ResMut<Swings>) {
        swings.0 += struck.read().filter(|s| player.contains(s.target)).count();
    }

    /// An unseen player is not noticed, however keen the watcher and
    /// however close: a watcher that notices anything in reach every turn
    /// stands one step away and never learns it is there.
    #[test]
    fn an_unseen_subject_is_not_noticed_even_adjacent() {
        let mut field = Field::new(keen(), 1, 10, true, None);
        field.app.init_resource::<Swings>().add_systems(PostUpdate, count_swings);
        let player = field.player;
        field.app.world_mut().entity_mut(player).insert(Unseen);
        for _ in 0..4 {
            field.wait();
        }
        assert!(!field.aware().knows(player), "adjacent, keen, and none the wiser");
        assert_eq!(field.app.world().resource::<Swings>().0, 0, "and never swung at");
    }

    /// A watcher alert to the player loses it the moment it is unseen and
    /// searches where it last saw it, then forgets.
    #[test]
    fn a_watcher_that_knew_loses_the_unseen_and_forgets_after_its_memory() {
        let mut field = Field::new(keen(), 4, 10, true, None);
        field.wait();
        assert!(field.aware().knows(field.player), "noticed first");
        let player = field.player;
        field.app.world_mut().entity_mut(player).insert(Unseen);
        for _ in 0..4 {
            field.wait();
        }
        assert!(!field.aware().knows(player), "a memory of three turns, and it forgot");
    }

    /// Without awareness, a mind that sees on sight still does not see the
    /// unseen.
    #[test]
    fn a_mind_that_sees_on_sight_does_not_see_the_unseen() {
        let mut field = Field::new(blind(), 1, 10, true, None);
        field.app.init_resource::<Swings>().add_systems(PostUpdate, count_swings);
        let (watcher, player) = (field.watcher, field.player);
        field.app.world_mut().entity_mut(watcher).remove::<(Notice, Aware)>();
        field.wait();
        assert!(field.app.world().resource::<Swings>().0 > 0, "seen, adjacent, it swings: the test can fail");
        field.app.world_mut().resource_mut::<Swings>().0 = 0;
        field.app.world_mut().entity_mut(player).insert(Unseen);
        for _ in 0..3 {
            field.wait();
        }
        assert_eq!(field.app.world().resource::<Swings>().0, 0, "unseen, adjacent, and never swung at");
    }

    /// The vitals strip's question: nobody watches the unseen.
    #[test]
    fn nobody_is_watching_the_unseen() {
        let mut field = Field::new(keen(), 2, 10, true, None);
        field.app.init_resource::<Watched>().add_systems(PostUpdate, read_watched);
        field.wait();
        assert_eq!(field.app.world().resource::<Watched>().0, Some(true));
        let player = field.player;
        field.app.world_mut().entity_mut(player).insert(Unseen);
        field.wait();
        assert_eq!(field.app.world().resource::<Watched>().0, Some(false));
    }

    /// The field, with statuses on and a `hidden` status registered as
    /// unseen, returned with its id.
    fn hiding_field(notice: NoticeStats, gap: i32) -> (Field, rl_rules::StatusId) {
        let mut field = Field::build(notice, gap, 10, true, None, |app| {
            app.add_plugins(crate::status::StatusPlugin);
        });
        let statuses = rl_rules::Registry::from_defs(vec![rl_rules::StatusDef::new("hidden").unseen()]).unwrap();
        let hidden = statuses.expect("hidden");
        field.app.world_mut().resource_mut::<crate::registries::Registries>().statuses = statuses;
        (field, hidden)
    }

    fn hide(field: &mut Field, status: rl_rules::StatusId) {
        let player = field.player;
        field.app.world_mut().write_message(crate::status::Afflict { target: player, status, turns: 5, by: None });
        field.wait();
    }

    /// Holding an unseen status is being unseen, and losing it is being
    /// seen again.
    #[test]
    fn an_unseen_status_marks_its_holder_while_it_lasts() {
        let (mut field, hidden) = hiding_field(blind(), 6);
        hide(&mut field, hidden);
        assert!(field.app.world().get::<Unseen>(field.player).is_some());
        for _ in 0..6 {
            field.wait();
        }
        assert!(field.app.world().get::<Unseen>(field.player).is_none(), "five turns, and it wore off");
    }

    /// A blow ends it in the pass it is struck, before the damage lands, and
    /// the one struck knows where from and answers on its very next turn.
    ///
    /// The answer is what shows the order: one update runs every pass up to
    /// the player's next turn, so a cure that landed a pass late would still
    /// be gone by the end of it, but the watcher would have decided its turn
    /// with the player still unseen and swung at nothing.
    #[test]
    fn striking_from_the_unseen_ends_it_and_wakes_the_one_struck() {
        let (mut field, hidden) = hiding_field(blind(), 1);
        field.app.init_resource::<Swings>().add_systems(PostUpdate, count_swings);
        hide(&mut field, hidden);
        let (player, watcher) = (field.player, field.watcher);
        let kind = field.app.world().resource::<crate::registries::Registries>().damage_kinds.expect("kinetic");
        field.app.world_mut().entity_mut(player).insert(MeleeAttack::new(kind, DiceRoll::flat(1)));
        field.app.world_mut().write_message(Intent::new(player, crate::combat::Attack(watcher)));
        field.app.update();
        assert!(field.app.world().get::<Unseen>(player).is_none(), "the blow ended it");
        assert!(field.aware().knows(player), "and the one struck knows where from");
        assert_eq!(field.app.world().resource::<Swings>().0, 1, "and swings back at an attacker it can see");
    }

    /// An unseen status hides its holder in the pass it goes on, so the next
    /// mind to decide, even one that sees on sight standing next to it,
    /// already cannot see it.
    #[test]
    fn a_status_hides_its_holder_from_the_very_next_mind_to_decide() {
        let (mut field, hidden) = hiding_field(blind(), 1);
        field.app.init_resource::<Swings>().add_systems(PostUpdate, count_swings);
        let watcher = field.watcher;
        field.app.world_mut().entity_mut(watcher).remove::<(Notice, Aware)>();
        field.wait();
        assert!(field.app.world().resource::<Swings>().0 > 0, "seen, adjacent, it swings: the test can fail");
        field.app.world_mut().resource_mut::<Swings>().0 = 0;
        hide(&mut field, hidden);
        assert_eq!(field.app.world().resource::<Swings>().0, 0, "hidden on the player's turn, and not swung at on the watcher's");
    }

    /// Harm credited to the unseen that was no attack, a status it put on
    /// earlier ticking, does not give it away: only an attack does.
    #[test]
    fn harm_that_is_no_attack_does_not_give_the_unseen_away() {
        let mut field = Field::new(blind(), 5, 10, true, None);
        let (watcher, player) = (field.watcher, field.player);
        field.app.world_mut().entity_mut(player).insert(Unseen);
        let kind = field.app.world().resource::<crate::registries::Registries>().damage_kinds.expect("kinetic");
        field.app.world_mut().write_message(DamageDealt { target: watcher, hit: Hit::by(player, kind, 1), dealt: 1, reach: crate::combat::Reach::Melee });
        field.wait();
        assert!(!field.aware().knows(player), "hurt by something it never saw, and none the wiser");
    }

    /// Throwing and an ability aimed at someone else end it as a blow does;
    /// an ability on oneself does not.
    #[test]
    fn a_throw_and_an_ability_at_another_end_it_and_one_on_yourself_does_not() {
        use crate::ability::AbilityEvent;
        let (mut field, hidden) = hiding_field(blind(), 6);
        let (player, watcher) = (field.player, field.watcher);
        let ability = rl_rules::ability::AbilityId::from_raw(0);
        let at = field.at(player);
        hide(&mut field, hidden);
        field.app.world_mut().write_message(AbilityEvent::Used { user: player, ability, aim: at, targets: vec![player] });
        field.wait();
        assert!(field.app.world().get::<Unseen>(player).is_some(), "a stim in the arm is not an attack");
        field.app.world_mut().write_message(AbilityEvent::Used { user: player, ability, aim: at, targets: vec![watcher] });
        field.wait();
        assert!(field.app.world().get::<Unseen>(player).is_none(), "one aimed at another is");

        hide(&mut field, hidden);
        let thing = field.app.world_mut().spawn(crate::items::Item).id();
        field.app.world_mut().write_message(crate::items::ItemEvent::Thrown { actor: player, item: thing, at: Position(at), struck: None });
        field.wait();
        assert!(field.app.world().get::<Unseen>(player).is_none(), "and so is a throw, whatever it hit");
    }
}
