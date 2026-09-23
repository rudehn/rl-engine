//! Noise: a second sense beside sight, and what a listener does with it.
//!
//! Opt-in. Without [`NoisePlugin`] nothing is heard, and every mind acts on
//! what it can see exactly as it always has. With it, every noise made in a
//! pass floods once from where it was made, walls stop it and a closed door
//! muffles it, and every listener it reaches with enough left goes to look.
//!
//! A listener hears a place, never a who. Who made a sound decides nothing
//! but that its maker does not hear it, because a monster behind a wall
//! cannot tell a friend's footsteps from an enemy's; it hears something, it
//! goes to see, and whether it then sees anyone is the ordinary sight and
//! notice roll. A sound at a cell it can already see is not followed: it
//! has looked.
//!
//! What was heard is a [`Heard`] of its own rather than an entry in
//! stealth's [`Aware`](crate::stealth::Aware), so a game with noise and no
//! stealth still has monsters that come round the corner to see what the
//! clatter was. Both offer their trail to [`Thinking`], which follows the
//! freshest.
//!
//! Heard in [`TurnSet::Listen`](crate::plugin::TurnSet::Listen), after the
//! pass's reactions, so a noise a game writes in `React` is heard in the
//! same pass as the engine's own. Nothing persists between turns except
//! what each listener remembers, which is not saved, as awareness is not.

use std::cmp::Reverse;

use bevy::prelude::*;
use rl_core::{Grid2D, Id, Interner, Point, Rect, geometry};
use rl_grid::{CostSource, DijkstraMap, PathRules};
use rl_rules::ai::awareness::Awareness;
use rl_rules::ai::hearing::{self, HearingStats};

use crate::combat::{DamageEvent, Dead};
use crate::components::{MyTurn, Player, Position};
use crate::doors::DoorEvent;
use crate::items::ItemEvent;
use crate::minds::{Sight, Thinking};
use crate::places::{MapId, OnMap};
use crate::turn::Stepped;
use crate::world::WorldMap;

/// What a sound was, as an interned name. Never constructed; it only
/// types [`SoundId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Sound {}

/// What a sound was, for a game phrasing a log line or reacting in its
/// own systems. Interned by [`Sounds`], so there is no closed list of what
/// can make a noise.
pub type SoundId = Id<Sound>;

/// Every kind of sound in play, in the order they were first named.
///
/// The engine's own are interned first, so their ids are the constants on
/// this type; a game adds its own with [`AddSound::add_sound`].
#[derive(Resource, Debug, Clone)]
pub struct Sounds(Interner<Sound>);

impl Sounds {
    /// Someone took a step.
    pub const STEP: SoundId = SoundId::from_raw(0);
    /// Someone struck a blow, with whatever it was struck with.
    pub const STRIKE: SoundId = SoundId::from_raw(1);
    /// A door opened or closed.
    pub const DOOR: SoundId = SoundId::from_raw(2);
    /// A thrown thing came to rest.
    pub const LANDING: SoundId = SoundId::from_raw(3);

    /// The engine's sounds, in the order their ids are handed out.
    pub const BUILT_IN: [&'static str; 4] = ["step", "strike", "door", "landing"];

    /// The id for `name`, assigning a new one if it is unseen.
    pub fn declare(&mut self, name: &str) -> SoundId {
        self.0.intern(name)
    }

    /// The id for `name`, if it has been declared.
    pub fn get(&self, name: &str) -> Option<SoundId> {
        self.0.get(name)
    }

    /// The name behind `id`.
    pub fn name(&self, id: SoundId) -> &str {
        self.0.name(id)
    }
}

impl Default for Sounds {
    fn default() -> Self {
        let mut names = Interner::new();
        for name in Self::BUILT_IN {
            names.intern(name);
        }
        Self(names)
    }
}

/// Declares a sound while the app is being built.
pub trait AddSound {
    /// Declares the sound `name`. Look its id up again with [`Sounds::get`]
    /// where it is made.
    fn add_sound(&mut self, name: &str) -> &mut Self;
}

impl AddSound for App {
    fn add_sound(&mut self, name: &str) -> &mut Self {
        self.init_resource::<Sounds>();
        self.world_mut().resource_mut::<Sounds>().declare(name);
        self
    }
}

/// How loud the engine's own actions are, in whole steps of how far they
/// carry over open ground, and how much a closed door takes off.
///
/// No defaults: how loud a step is, is balance. Zero makes no noise at all,
/// which is how a game leaves one of the engine's sources out.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoiseRules {
    /// A step, for an actor with no [`Footfall`] of its own.
    pub step: i32,
    /// A blow, made where the attacker stands.
    pub strike: i32,
    /// A door opening or closing.
    pub door: i32,
    /// A thrown thing coming to rest.
    pub landing: i32,
    /// Steps of loudness a closed door takes off a sound passing it.
    pub door_muffle: i32,
}

/// A sound, made this pass.
///
/// The engine writes one for its own actions; a game writes one for
/// anything else, a shout, a spell, a shelf coming down, and it is heard
/// the same way. Heard in the pass it was written in when written by
/// [`TurnSet::React`](crate::plugin::TurnSet::React) at the latest.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MakeNoise {
    /// Where it was made.
    pub at: Point,
    /// How far it carries over open ground, in whole steps.
    pub loudness: i32,
    /// What it was.
    pub sound: SoundId,
    /// Who made it, for a game's own reactions. The engine reads it only
    /// so its maker does not hear it: a listener hears a place, not a who,
    /// but it knows its own footsteps.
    pub maker: Option<Entity>,
}

/// A listener heard a noise.
///
/// Written for every listener that heard it, the player included when the
/// player has [`Hearing`], so a game can log "you hear a door open".
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoiseHeard {
    /// Who heard it.
    pub listener: Entity,
    /// Where it was made.
    pub at: Point,
    /// What it was.
    pub sound: SoundId,
    /// Who made it, as the noise said.
    pub maker: Option<Entity>,
    /// How much of its loudness was still on it when it arrived, in
    /// hundredths of a step over open ground, the same unit every clock
    /// here counts in: what a meter reads, and what `Hearing::threshold`
    /// is measured against. A shout next door arrives louder than the
    /// same shout across the deck.
    pub left: i32,
}

/// How keenly this actor hears. Absent, it is deaf, which is how every
/// actor behaved before noise.
///
/// Brings a [`Heard`] with it, so a listener is ready to remember the
/// moment it is spawned.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default, Deref, DerefMut)]
#[require(Heard)]
pub struct Hearing(pub HearingStats);

/// What this listener last heard, and how many of its turns ago.
///
/// An [`Awareness`], because "heard something there, this long ago" is
/// exactly an alert that decays, and it is forgotten the way a lost trail
/// is. Not saved, as [`Aware`](crate::stealth::Aware) is not: a monster on
/// its way to look at a sound forgets it on load.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default, Deref)]
pub struct Heard(pub Awareness);

/// How loud this actor's steps are, in place of [`NoiseRules::step`]:
/// heavier for something that clatters, zero for something that pads.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Deref)]
pub struct Footfall(pub i32);

/// Whether [`NoisePlugin`] was added, for the systems outside this module
/// that read what was heard.
///
/// Asked of the plugin rather than of the components, as
/// [`StealthRunning`](crate::stealth::StealthRunning) is, because
/// [`Hearing`] brings a [`Heard`] with it whether or not anything will ever
/// fill it. The plugin's own message is the proof it was added.
#[derive(bevy::ecs::system::SystemParam)]
pub struct NoiseRunning<'w> {
    heard: Option<Res<'w, Messages<NoiseHeard>>>,
}

impl NoiseRunning<'_> {
    /// Whether noise is running.
    pub fn get(&self) -> bool {
        self.heard.is_some()
    }
}

/// The flood every noise reuses, so hearing allocates nothing once it has
/// grown to the loudest sound in play.
#[derive(Resource, Debug)]
pub struct Earshot(DijkstraMap);

impl Default for Earshot {
    fn default() -> Self {
        Self(DijkstraMap::new(Rect::new(0, 0, 0, 0)))
    }
}

/// The loaded window as sound reads it, in window-local coordinates like
/// [`WindowView`](crate::world::WindowView).
struct SoundView<'a> {
    map: &'a WorldMap,
    muffle: i32,
}

impl Grid2D for SoundView<'_> {
    fn width(&self) -> i32 {
        self.map.window_tiles().width
    }

    fn height(&self) -> i32 {
        self.map.window_tiles().height
    }
}

impl CostSource for SoundView<'_> {
    fn cost_idx(&self, idx: usize) -> Option<u32> {
        let p = self.map.to_world(self.idx_point(idx));
        hearing::carries(self.map.blocks_projectiles(p), self.map.opens(p).is_some(), self.muffle)
    }
}

/// Adds hearing: noise from the engine's own actions and a game's, carried
/// through walls and doors, and listeners that go to look.
///
/// ```
/// use bevy::prelude::*;
/// use rl_bevy::noise::{NoisePlugin, NoiseRules};
///
/// let mut app = App::new();
/// app.add_plugins(NoisePlugin::new(NoiseRules { step: 2, strike: 8, door: 5, landing: 6, door_muffle: 3 }));
/// ```
pub struct NoisePlugin {
    rules: NoiseRules,
}

impl NoisePlugin {
    /// Hearing, with the engine's own actions as loud as `rules` says.
    pub fn new(rules: NoiseRules) -> Self {
        Self { rules }
    }
}

impl Plugin for NoisePlugin {
    fn build(&self, app: &mut App) {
        use crate::plugin::{DecideSet, PerceiveSet, Turn, TurnSet};
        // What the engine's sources read exists whether or not the game
        // added combat or items; registering one twice is what
        // `add_message` is built for.
        app.insert_resource(self.rules)
            .init_resource::<Sounds>()
            .init_resource::<Earshot>()
            .add_message::<MakeNoise>()
            .add_message::<NoiseHeard>()
            .add_message::<DamageEvent>()
            .add_message::<ItemEvent>()
            .add_systems(Turn, age_heard.in_set(DecideSet::Notice))
            .add_systems(Turn, follow_heard.in_set(PerceiveSet::Annotate))
            .add_systems(Turn, (make_engine_noise, resolve_noise).chain().in_set(TurnSet::Listen));
    }

    fn finish(&self, app: &mut App) {
        crate::plugin::depends_on::<crate::plugin::CorePlugin>(app, "NoisePlugin");
    }
}

/// What the engine's own actions did this pass, as its noise reads them.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Sources<'w, 's> {
    stepped: MessageReader<'w, 's, Stepped>,
    blows: MessageReader<'w, 's, DamageEvent>,
    doors: MessageReader<'w, 's, DoorEvent>,
    items: MessageReader<'w, 's, ItemEvent>,
    footfalls: Query<'w, 's, &'static Footfall>,
    positions: Query<'w, 's, &'static Position>,
}

/// Writes the noise of the engine's own actions this pass: every step, a
/// blow where its attacker stands, a door, and a thrown thing where it
/// came to rest.
///
/// A blow is one sound per attacker however many strikes it carried, so a
/// flurry is not heard as four. A mend is not a blow, and damage over time
/// has no attacker, so neither makes a sound. How loud each is comes from
/// [`NoiseRules`], and a stepper's own [`Footfall`] in place of the rule.
pub fn make_engine_noise(mut sources: Sources, rules: Res<NoiseRules>, mut noise: MessageWriter<MakeNoise>) {
    for step in sources.stepped.read() {
        let loudness = sources.footfalls.get(step.actor).map_or(rules.step, |f| f.0);
        noise.write(MakeNoise { at: step.to, loudness, sound: Sounds::STEP, maker: Some(step.actor) });
    }
    let mut attackers: Vec<Entity> = Vec::new();
    for blow in sources.blows.read() {
        if let Some(attacker) = blow.hit.attacker.filter(|a| blow.hit.amount >= 0 && !attackers.contains(a)) {
            attackers.push(attacker);
        }
    }
    for attacker in attackers {
        if let Ok(at) = sources.positions.get(attacker) {
            noise.write(MakeNoise { at: at.0, loudness: rules.strike, sound: Sounds::STRIKE, maker: Some(attacker) });
        }
    }
    for door in sources.doors.read() {
        let (DoorEvent::Opened { actor, at } | DoorEvent::Closed { actor, at }) = *door;
        noise.write(MakeNoise { at, loudness: rules.door, sound: Sounds::DOOR, maker: Some(actor) });
    }
    for item in sources.items.read() {
        if let ItemEvent::Thrown { actor, at, .. } = *item {
            noise.write(MakeNoise { at: at.0, loudness: rules.landing, sound: Sounds::LANDING, maker: Some(actor) });
        }
    }
}

/// A listener, as the flood reads it.
type Listener = (Entity, &'static Position, &'static Hearing, &'static mut Heard, Option<&'static OnMap>, Has<Player>);

/// Floods every noise made this pass and tells each listener it reaches.
///
/// A listener that heard several keeps the one that arrived loudest, ties
/// to the lower cell, so the order they were written in cannot change where
/// it goes. A noise with no listener within its loudness in a straight
/// line is skipped before any flood, since no flood carries further.
pub fn resolve_noise(
    mut made: MessageReader<MakeNoise>,
    map: Res<WorldMap>,
    rules: Res<NoiseRules>,
    mut earshot: ResMut<Earshot>,
    mut listeners: Query<Listener, Without<Dead>>,
    mut heard: MessageWriter<NoiseHeard>,
) {
    let noises: Vec<MakeNoise> = made.read().filter(|n| n.loudness > 0).copied().collect();
    if noises.is_empty() {
        return;
    }
    let here = map.current();
    let mut near: Vec<_> = listeners.iter_mut().filter(|(.., on, _)| on.map(|m| m.0).unwrap_or(MapId::SURFACE) == here).collect();
    near.sort_by_key(|(e, ..)| *e);
    let view = SoundView { map: &map, muffle: rules.door_muffle };
    let mut loudest: Vec<Option<(i32, Reverse<Point>)>> = vec![None; near.len()];
    for noise in &noises {
        let reach = noise.loudness;
        if !near.iter().any(|(_, pos, ..)| geometry::chebyshev(pos.0, noise.at) <= reach) {
            continue;
        }
        let Some(from) = map.to_local(noise.at) else { continue };
        // A flood cannot carry further than its loudness, and nothing lies
        // beyond the loaded window to hear it.
        let square = Rect::new(from.x - reach, from.y - reach, 2 * reach + 1, 2 * reach + 1);
        let Some(region) = square.intersection(&view.bounds()) else { continue };
        earshot.0.reset(region);
        earshot.0.build(&view, [from], PathRules::EIGHT_WAY);
        for (i, (listener, pos, ear, ..)) in near.iter().enumerate() {
            // Its own noise is the one thing a listener knows the maker of.
            if noise.maker == Some(*listener) {
                continue;
            }
            let Some(travelled) = map.to_local(pos.0).and_then(|p| earshot.0.value(p)) else { continue };
            let left = hearing::left_after(noise.loudness, travelled);
            if !hearing::heard(left, ear.threshold) {
                continue;
            }
            heard.write(NoiseHeard { listener: *listener, at: noise.at, sound: noise.sound, maker: noise.maker, left });
            let arrived = (left, Reverse(noise.at));
            if loudest[i].is_none_or(|held| arrived > held) {
                loudest[i] = Some(arrived);
            }
        }
    }
    for ((.., memory, _, player), arrived) in near.iter_mut().zip(loudest) {
        // The player is told, and decides for themselves what to do about it.
        if let (Some((_, Reverse(at))), false) = (arrived, *player) {
            memory.0.alerted_to(at);
        }
    }
}

/// A listener holding the turn that is not the player, whose turn is the
/// game's.
type HoldingTheTurn = (With<MyTurn>, Without<Player>);

/// Ages what the listener holding the turn heard, and forgets it once its
/// memory runs out.
///
/// In [`DecideSet::Notice`](crate::plugin::DecideSet::Notice), beside
/// stealth's own aging, so a sound and a sighting grow stale at the same
/// point in a turn.
pub fn age_heard(mut listeners: Query<(&Hearing, &mut Heard), HoldingTheTurn>) {
    let Ok((ear, mut heard)) = listeners.single_mut() else { return };
    if heard.0.is_alert() {
        heard.0.lost(ear.memory);
    }
}

/// Offers what the mind holding the turn heard as a trail to follow, or
/// forgets it if the mind can see where it came from.
///
/// Seeing the cell is having looked: anyone there is already in its
/// snapshot, or hiding and up to the notice roll. This is what ends a
/// search that arrived.
pub fn follow_heard(mut thinking: ResMut<Thinking>, sight: Sight, mut listeners: Query<(&mut Heard, Option<&OnMap>)>) {
    let Some(thinker) = thinking.actor() else { return };
    let Ok((mut heard, on)) = listeners.get_mut(thinker) else { return };
    let Awareness::Alert { at, stale_turns } = heard.0 else { return };
    if sight.perceives(&thinking, at, on) {
        heard.0 = Awareness::Unaware;
    } else {
        thinking.offer_trail(at, stale_turns);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::components::{Actor, Blocks, RevealsMap, Viewshed};
    use crate::fov::FovPlugin;
    use crate::minds::{Mind, MindsPlugin, Perception};
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use crate::turn::{Intent, Wait};
    use crate::world::StreamingPlugin;
    use rl_rules::Brain;
    use rl_rules::ai::tactics::{SearchLastKnown, Wander};

    const RULES: NoiseRules = NoiseRules { step: 0, strike: 0, door: 0, landing: 0, door_muffle: 2 };

    /// An open field with the player at `start` and whatever listeners a
    /// test spawns, each searching what it heard and otherwise standing.
    struct Field {
        app: App,
        player: Entity,
        start: Point,
    }

    impl Field {
        fn new(rules: NoiseRules) -> Field {
            Field::with(Some(rules))
        }

        /// The field with hearing only if `rules` are given.
        fn with(rules: Option<NoiseRules>) -> Field {
            let mut app = headless_app();
            app.add_plugins((FovPlugin, MindsPlugin, StreamingPlugin));
            if let Some(rules) = rules {
                app.add_plugins(NoisePlugin::new(rules));
            }
            let start = crate::testing::surface(&mut app);
            app.insert_resource(crate::seed::Seed(crate::testing::TEST_SEED));
            let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(16), RevealsMap)).id();
            app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
            app.update();
            app.update();
            Field { app, player, start }
        }

        /// A listener `dx, dy` from the start that sees `reach` tiles.
        fn listener(&mut self, dx: i32, dy: i32, threshold: i32, reach: i32) -> Entity {
            let brain = Brain::new().then(SearchLastKnown).then(Wander { chance_pct: 0 });
            let at = self.start.offset(dx, dy);
            let hearing = Hearing(HearingStats { threshold, memory: 3 });
            let e = self.app.world_mut().spawn((Actor, Blocks, Position(at), hearing, Perception(reach), Mind(Arc::new(brain)))).id();
            self.app.update();
            e
        }

        fn tile(&mut self, dx: i32, dy: i32, name: &str) {
            let id = rl_grid::TileRegistry::standard().expect(name);
            let at = self.start.offset(dx, dy);
            assert!(self.app.world_mut().resource_mut::<WorldMap>().set_tile(at, id));
        }

        fn noise(&mut self, dx: i32, dy: i32, loudness: i32) {
            let at = self.start.offset(dx, dy);
            self.app.world_mut().write_message(MakeNoise { at, loudness, sound: Sounds::STRIKE, maker: None });
        }

        /// The player waits a whole turn and everyone else takes theirs.
        fn wait(&mut self) {
            self.app.world_mut().write_message(Intent::new(self.player, Wait));
            self.app.update();
        }

        /// The player steps once toward `dir`, and everyone else takes
        /// their turn.
        fn step(&mut self, dir: rl_core::Direction) {
            self.app.world_mut().write_message(Intent::new(self.player, crate::turn::Step(dir)));
            self.app.update();
        }

        fn heard(&self, e: Entity) -> Awareness {
            self.app.world().get::<Heard>(e).unwrap().0
        }

        fn at(&self, e: Entity) -> Point {
            self.app.world().get::<Position>(e).unwrap().0
        }
    }

    #[test]
    fn a_sound_carries_through_a_closed_door_but_not_through_a_wall() {
        for (gap, expect) in [("door_closed", true), ("wall", false)] {
            let mut field = Field::new(RULES);
            // A wall three east of the noise, running further than any
            // sound here carries, with one gap in line with the listener.
            for dy in -12..=12 {
                field.tile(3, dy, if dy == 0 { gap } else { "wall" });
            }
            let listener = field.listener(6, 0, 0, 1);
            field.noise(0, 0, 8);
            field.wait();
            let heard = field.heard(listener).is_alert();
            assert_eq!(heard, expect, "through a {gap}: six steps and a muffle of two is eight, which a loudness of eight just reaches");
        }
    }

    #[test]
    fn a_listener_walks_to_a_sound_it_cannot_see_and_forgets_it_when_its_memory_runs_out() {
        let mut field = Field::new(RULES);
        let listener = field.listener(8, 0, 0, 2);
        field.noise(1, 0, 10);
        field.wait();
        assert_eq!(field.heard(listener).last_known(), Some(field.start.offset(1, 0)), "it heard where");
        let before = field.at(listener);
        field.wait();
        field.wait();
        assert!(field.at(listener).x < before.x, "and went toward it: {:?} from {before:?}", field.at(listener));
        for _ in 0..4 {
            field.wait();
        }
        assert!(!field.heard(listener).is_alert(), "and forgot it, having seen the place or run out of memory");
    }

    #[test]
    fn a_sound_the_listener_can_see_is_not_followed() {
        let mut field = Field::new(RULES);
        let listener = field.listener(5, 0, 0, 8);
        let post = field.at(listener);
        field.noise(2, 3, 10);
        field.wait();
        field.wait();
        assert!(!field.heard(listener).is_alert(), "it looked, and there was nothing to follow");
        assert_eq!(field.at(listener), post, "so it stayed where it stood");
    }

    #[test]
    fn of_two_sounds_in_one_pass_the_one_that_arrives_loudest_is_followed_whatever_the_order() {
        for louder_first in [true, false] {
            let mut field = Field::new(RULES);
            let listener = field.listener(0, 6, 0, 1);
            // Four steps from a loudness of nine leaves five; six steps
            // from a loudness of ten leaves four.
            let (near, far) = ((0, 2, 9), (6, 6, 10));
            let order = if louder_first { [near, far] } else { [far, near] };
            for (dx, dy, loud) in order {
                field.noise(dx, dy, loud);
            }
            field.wait();
            assert_eq!(field.heard(listener).last_known(), Some(field.start.offset(0, 2)), "louder written first: {louder_first}");
        }
    }

    #[test]
    fn a_threshold_keeps_a_sound_that_arrives_too_faint_from_being_heard() {
        let mut field = Field::new(RULES);
        let keen = field.listener(5, 0, 0, 1);
        let dull = field.listener(0, 5, 3, 1);
        field.noise(0, 0, 6);
        field.wait();
        assert!(field.heard(keen).is_alert(), "one step to spare");
        assert!(!field.heard(dull).is_alert(), "one arrives, three were needed");
    }

    /// Every `NoiseHeard` written, by a reader that sees each once.
    #[derive(Resource, Default)]
    struct Told(Vec<NoiseHeard>);

    fn tell(mut heard: MessageReader<NoiseHeard>, mut told: ResMut<Told>) {
        told.0.extend(heard.read().copied());
    }

    #[test]
    fn the_player_is_told_what_it_heard_and_left_to_decide_what_to_do() {
        let mut field = Field::new(RULES);
        field.app.init_resource::<Told>().add_systems(PostUpdate, tell);
        let player = field.player;
        field.app.world_mut().entity_mut(player).insert(Hearing(HearingStats { threshold: 0, memory: 3 }));
        let shout = field.app.world_mut().resource_mut::<Sounds>().declare("shout");
        let at = field.start.offset(4, 0);
        field.app.world_mut().write_message(MakeNoise { at, loudness: 6, sound: shout, maker: None });
        field.wait();
        let told = &field.app.world().resource::<Told>().0;
        assert_eq!(told.len(), 1, "one shout, one telling: {told:?}");
        assert_eq!((told[0].listener, told[0].at, told[0].sound, told[0].maker), (player, at, shout, None));
        assert!(told[0].left > 0, "and it arrived with something still on it: {}", told[0].left);

        // And it arrives quieter the further it has come, which is what a
        // meter of the noise around a listener reads. Further still and it
        // does not arrive at all, which is the threshold's business.
        let close = told[0].left;
        let far = field.start.offset(5, 0);
        field.app.world_mut().resource_mut::<Told>().0.clear();
        field.app.world_mut().write_message(MakeNoise { at: far, loudness: 6, sound: shout, maker: None });
        field.wait();
        let then = &field.app.world().resource::<Told>().0;
        assert_eq!(then.len(), 1, "still within earshot: {then:?}");
        assert!(then[0].left < close, "further off, quieter: {close} then {}", then[0].left);
        assert!(!field.heard(player).is_alert(), "the player's own turn is the game's");
    }

    const LOUD_STEPS: NoiseRules = NoiseRules { step: 4, strike: 6, door: 5, landing: 0, door_muffle: 2 };

    #[test]
    fn a_step_is_heard_where_it_lands_and_a_footfall_of_its_own_replaces_the_rule() {
        let mut field = Field::new(LOUD_STEPS);
        let listener = field.listener(5, 0, 0, 1);
        field.step(rl_core::Direction::East);
        assert_eq!(field.heard(listener).last_known(), Some(field.start.offset(1, 0)), "four steps off, a step of four is heard");

        let mut field = Field::new(LOUD_STEPS);
        let player = field.player;
        field.app.world_mut().entity_mut(player).insert(Footfall(3));
        let listener = field.listener(5, 0, 0, 1);
        field.step(rl_core::Direction::East);
        assert!(!field.heard(listener).is_alert(), "a lighter foot falls short of it");
    }

    #[test]
    fn a_flurry_is_one_sound_where_the_attacker_stands_and_a_mend_is_none() {
        let mut field = Field::new(LOUD_STEPS);
        field.app.init_resource::<Told>().add_systems(PostUpdate, tell);
        let (player, start) = (field.player, field.start);
        let listener = field.listener(3, 0, 0, 1);
        let healer = field.app.world_mut().spawn(Position(start.offset(0, 2))).id();
        let kind = rl_rules::damage::DamageKindId::from_raw(0);
        for hit in [rl_rules::Hit::by(player, kind, 3), rl_rules::Hit::by(player, kind, 0), rl_rules::Hit::by(healer, kind, -4)] {
            field.app.world_mut().write_message(DamageEvent::new(listener, hit));
        }
        field.wait();
        let told = &field.app.world().resource::<Told>().0;
        assert_eq!(told.len(), 1, "{told:?}");
        assert_eq!((told[0].listener, told[0].at, told[0].sound, told[0].maker), (listener, start, Sounds::STRIKE, Some(player)));
    }

    #[test]
    fn a_door_opened_is_heard_at_the_door() {
        let mut field = Field::new(LOUD_STEPS);
        field.app.init_resource::<Told>().add_systems(PostUpdate, tell);
        field.tile(1, 0, "door_closed");
        let listener = field.listener(4, 3, 0, 1);
        let player = field.player;
        field.app.world_mut().write_message(Intent::new(player, crate::doors::Open(rl_core::Direction::East)));
        field.app.update();
        let door = field.start.offset(1, 0);
        let told = &field.app.world().resource::<Told>().0;
        assert_eq!(told.len(), 1, "{told:?}");
        assert_eq!((told[0].listener, told[0].at, told[0].sound, told[0].maker), (listener, door, Sounds::DOOR, Some(player)));
    }

    #[test]
    fn without_the_plugin_a_listener_hears_nothing_however_close() {
        let mut field = Field::with(None);
        let listener = field.listener(2, 0, 0, 1);
        field.step(rl_core::Direction::East);
        field.wait();
        assert!(!field.heard(listener).is_alert(), "no NoisePlugin, no noise");
    }

    #[test]
    fn a_game_declares_sounds_after_the_engines_and_finds_them_by_name() {
        let mut app = App::new();
        app.add_sound("shout");
        let sounds = app.world().resource::<Sounds>();
        let shout = sounds.get("shout").expect("declared");
        assert_eq!(shout, SoundId::from_raw(Sounds::BUILT_IN.len() as u32));
        assert_eq!(sounds.name(Sounds::DOOR), "door");
        assert_eq!(sounds.name(shout), "shout");
    }
}
