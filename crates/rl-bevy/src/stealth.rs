//! Stealth: who has noticed whom, kept on the observers.
//!
//! Opt-in. Without [`StealthPlugin`] no actor carries an [`Aware`], and
//! [`decide_minds`](crate::minds::decide_minds) sees everything it has a
//! line to exactly as it always has. With it, a subject carrying
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
//! Awareness ticks in [`DecideSet::Notice`](crate::plugin::DecideSet::Notice),
//! for the actor holding the turn and only that one, so it costs a roll per
//! subject per monster-turn and nothing per frame.

use std::collections::BTreeMap;

use bevy::prelude::*;
use rand::Rng;
use rl_core::Point;
use rl_rules::ai::awareness::{self, Awareness, NoticeStats, StealthStats};

use crate::combat::{CombatRng, CombatRules, DamageDealt, Dead, Faction};
use crate::components::{MyTurn, Player, Position, Viewshed};
use crate::lighting::{DarkSight, Lighting};
use crate::minds::{Mind, Perception, perceivable};
use crate::places::{MapId, OnMap};
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
    Option<&'static DarkSight>,
    Option<&'static Aware>,
    Option<&'static OnMap>,
);

/// What counts as a watcher: something that decides, or something authored
/// to notice, that is neither the player nor dead.
type CanWatch = (Or<(With<Mind>, With<Notice>)>, Without<Player>, Without<Dead>);
/// One watcher, as the query hands it back.
type Watcher<'a> = (Entity, &'a Position, Option<&'a Faction>, Option<&'a Perception>, Option<&'a DarkSight>, Option<&'a Aware>, Option<&'a OnMap>);

/// Who is watching whom right now, by the rule the minds act on.
///
/// Stealth only hides a subject from observers that keep an [`Aware`]. One
/// that does not - a monster with no [`Notice`] - sees on sight, exactly as
/// it did before stealth existed, and it will attack a hider it can see.
/// Asking only the `Aware` keepers whether a player has been seen therefore
/// answers "hidden" while such a monster cuts the player down, which is how
/// this came to exist. The rule here is the one `decide_minds` applies: an
/// observer that keeps an `Aware` watches what it knows about, alert or
/// searching; one that does not watches whatever it can perceive.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Watchers<'w, 's> {
    running: StealthRunning<'w>,
    player: Query<'w, 's, (&'static Position, &'static Viewshed), With<Player>>,
    watchers: Query<'w, 's, WatcherData, CanWatch>,
    subjects: Query<'w, 's, (&'static Position, Option<&'static Faction>, Option<&'static OnMap>)>,
    lighting: Option<Res<'w, Lighting>>,
    map: Option<Res<'w, WorldMap>>,
    rules: Option<Res<'w, CombatRules>>,
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

    fn judge(&self, (watcher, pos, faction, perception, dark, aware, on): Watcher<'_>, subject: Entity) -> bool {
        if watcher == subject {
            return false;
        }
        let (Some(map), Some(rules), Ok((at, theirs, subject_on))) = (self.map.as_deref(), self.rules.as_deref(), self.subjects.get(subject)) else {
            return false;
        };
        // Only what is at odds with the subject: an ally looking on is not
        // being seen by an enemy.
        match (faction, theirs) {
            (Some(mine), Some(theirs)) if rules.factions.is_hostile(mine.0, theirs.0) => {}
            _ => return false,
        }
        if let Some(aware) = aware {
            return aware.knows(subject);
        }
        let here = map.current();
        if on.map(|m| m.0).unwrap_or(MapId::SURFACE) != here || subject_on.map(|m| m.0).unwrap_or(MapId::SURFACE) != here {
            return false;
        }
        let Ok((player_pos, sight)) = self.player.single() else { return false };
        awareness::within_reach(pos.0, at.0, perception.map(|p| p.0).unwrap_or(8))
            && perceivable(player_pos.0, sight, self.lighting.as_deref(), pos.0, dark.map(|d| d.0).unwrap_or(0), at.0)
    }
}

/// Adds noticing, forgetting, and being woken by a blow.
pub struct StealthPlugin;

impl Plugin for StealthPlugin {
    fn build(&self, app: &mut App) {
        use crate::plugin::{DecideSet, Turn, TurnSet};
        app.add_message::<Noticed>().add_systems(Turn, update_awareness.in_set(DecideSet::Notice)).add_systems(Turn, wake_on_damage.in_set(TurnSet::React));
    }

    fn finish(&self, app: &mut App) {
        crate::plugin::depends_on::<crate::minds::MindsPlugin>(app, "StealthPlugin");
    }
}

/// The observer holding the turn.
type Observer =
    (Entity, &'static Position, &'static Notice, &'static mut Aware, Option<&'static Perception>, Option<&'static DarkSight>, Option<&'static Faction>);
/// Anything that might be hiding from it.
type Subject = (Entity, &'static Position, &'static Stealth, Option<&'static Faction>, Option<&'static OnMap>);
/// One subject, as the query hands it back.
type Hiding<'a> = (Entity, &'a Position, &'a Stealth, Option<&'a Faction>, Option<&'a OnMap>);

/// Everything noticing reads.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Watch<'w, 's> {
    observers: Query<'w, 's, Observer, (With<MyTurn>, Without<Player>)>,
    subjects: Query<'w, 's, Subject>,
    player: Query<'w, 's, (&'static Position, &'static Viewshed), With<Player>>,
    lighting: Option<Res<'w, Lighting>>,
    map: Res<'w, WorldMap>,
    rules: Res<'w, CombatRules>,
    rng: ResMut<'w, CombatRng>,
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
    let Ok((player_pos, player_sight)) = watch.player.single() else { return };
    let (player_pos, player_sight) = (player_pos.0, player_sight.clone());
    let here = watch.map.current();
    let lighting = watch.lighting.as_deref();
    let Ok((observer, pos, notice, mut aware, perception, dark, faction)) = watch.observers.single_mut() else { return };
    let reach = perception.map(|p| p.0).unwrap_or(8);
    let dark_sight = dark.map(|d| d.0).unwrap_or(0);
    let mut seen = Vec::new();
    // In spawn order rather than the order the query walks the archetypes
    // in, since each subject in view costs a roll and which subject gets
    // which roll must not depend on how the world happens to be laid out.
    let mut subjects: Vec<Hiding<'_>> = watch.subjects.iter().collect();
    subjects.sort_by_key(|(e, ..)| (e.index(), *e));
    for (subject, at, stealth, theirs, on) in subjects {
        if subject == observer {
            continue;
        }
        // Only what it is at odds with, so an ally slipping past it does
        // not make the vitals strip say the player has been seen.
        if let (Some(mine), Some(theirs)) = (faction, theirs)
            && !watch.rules.factions.is_hostile(mine.0, theirs.0)
        {
            continue;
        }
        let on_this_map = on.map(|m| m.0).unwrap_or(MapId::SURFACE) == here;
        let in_view = on_this_map && awareness::within_reach(pos.0, at.0, reach) && perceivable(player_pos, &player_sight, lighting, pos.0, dark_sight, at.0);
        let mut state = aware.of(subject);
        if in_view && state.is_alert() {
            state.saw(at.0);
        } else if in_view {
            let lit = lighting.is_none_or(|l| l.is_lit(at.0));
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

/// Wakes whoever takes a blow from something hiding, and points them at it.
///
/// Being hit is noticing, however quiet the attacker was. In
/// [`TurnSet::React`](crate::plugin::TurnSet::React) because that is where
/// a turn's consequences land, and inside the pass, so the monster that was
/// struck already knows where from when it next decides.
pub fn wake_on_damage(
    mut dealt: MessageReader<DamageDealt>,
    mut observers: Query<&mut Aware>,
    attackers: Query<&Position, With<Stealth>>,
    mut noticed: MessageWriter<Noticed>,
) {
    for ev in dealt.read() {
        // A mend is a negative hit down the same pipeline, and a hider who
        // patches a sleeper up has not struck it. A blow that armor stopped
        // at zero still woke it.
        if ev.dealt < 0 {
            continue;
        }
        let Some(attacker) = ev.hit.attacker else { continue };
        let (Ok(mut aware), Ok(at)) = (observers.get_mut(ev.target), attackers.get(attacker)) else { continue };
        let mut state = aware.of(attacker);
        if state.alerted_to(at.0) {
            noticed.write(Noticed { observer: ev.target, subject: attacker, at: at.0 });
        }
        aware.0.insert(attacker, state);
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
        fn new(notice: NoticeStats, gap: i32, reach: i32, plugin: bool) -> Field {
            let mut app = headless_app();
            app.add_plugins((FovPlugin, CombatPlugin, MindsPlugin, StreamingPlugin));
            if plugin {
                app.add_plugins(StealthPlugin);
            }
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
                    (MeleeAttack { kind, dice: DiceRoll::flat(0) }, Perception(reach), Mind(Arc::new(brain)), Notice(notice)),
                ))
                .id();
            app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
            app.update();
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

    #[test]
    fn a_hider_nobody_noticed_is_left_alone_and_one_they_did_is_hunted() {
        let mut quiet = Field::new(blind(), 5, 10, true);
        for _ in 0..4 {
            quiet.wait();
        }
        assert_eq!(quiet.distance(), 5, "an unnoticed player is not approached");
        assert!(!quiet.aware().knows(quiet.player));

        let mut loud = Field::new(keen(), 5, 10, true);
        for _ in 0..3 {
            loud.wait();
        }
        assert!(loud.distance() < 5, "a noticed player is hunted: {}", loud.distance());
        assert!(loud.aware().knows(loud.player));
    }

    #[test]
    fn without_the_plugin_an_authored_observer_still_sees_on_sight() {
        let mut field = Field::new(blind(), 5, 10, false);
        for _ in 0..3 {
            field.wait();
        }
        assert!(field.distance() < 5, "no StealthPlugin means the old behaviour, Notice or not: {}", field.distance());
    }

    #[test]
    fn a_blow_wakes_an_observer_that_never_saw_it_coming() {
        let mut field = Field::new(blind(), 5, 10, true);
        field.wait();
        assert!(!field.aware().knows(field.player));
        let (watcher, player) = (field.watcher, field.player);
        let kind = field.app.world().resource::<crate::registries::Registries>().damage_kinds.expect("kinetic");
        field.app.world_mut().write_message(DamageDealt { target: watcher, hit: Hit::by(player, kind, 1), dealt: 1 });
        field.wait();
        assert!(field.aware().knows(player), "struck, so it knows where from");
        field.wait();
        assert!(field.distance() < 5, "and goes for it: {}", field.distance());
    }

    #[test]
    fn a_lost_trail_is_walked_to_and_then_forgotten() {
        // Reach three and a gap of eight: the player is out of range, so
        // every turn is a turn without a sighting.
        let mut field = Field::new(blind(), 8, 3, true);
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
        let mut field = Field::new(blind(), 2, 10, true);
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
        let mut field = Field::new(keen(), 6, 10, true);
        field.app.init_resource::<Heard>().add_systems(Update, listen);
        for _ in 0..5 {
            field.wait();
        }
        assert!(field.aware().knows(field.player), "still aware after five turns");
        assert_eq!(field.app.world().resource::<Heard>().0, 1, "one flip, one message");
    }
}
