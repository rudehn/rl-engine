//! Minds: what decides the turn of everyone but the player.
//!
//! A [`Mind`] holds a brain, a priority list of tactics from
//! [`rl_rules::ai`]. On its turn the engine builds the [`Snapshot`] its
//! tactics read and turns the [`Decision`] that comes back into the intent
//! of the action that answers it: a step, an attack, a wait, an ability, a
//! pickup, a throw, or a choice of the game's own.
//!
//! The snapshot is not built here. [`Thinking`] holds it while it is
//! being filled, and every plugin that knows something a mind should adds
//! its part in [`DecideSet::Perceive`](crate::plugin::DecideSet::Perceive):
//! combat sorts who is seen into sides, stealth takes out what has not
//! been noticed, items say what is carried and what lies about, abilities
//! what may be used, fire where not to step, and a game pushes a [`Sense`]
//! of its own. This module opens the snapshot, closes it, and decides;
//! nothing here knows what a faction, a knife or a flame is. A subsystem
//! added later adds a contributor and edits nothing here.
//!
//! Its own plugin rather than part of combat, because this is the one place
//! every action a monster can choose meets. Combat resolves a blow whoever
//! struck it and abilities resolve a use whoever chose it; only a mind has to
//! know that both exist, so neither of them has to know about the other.

use std::collections::BTreeMap;
use std::sync::Arc;

use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::world::DeferredWorld;
use bevy::prelude::*;
use rand::rngs::StdRng;
use rl_core::{Direction, Grid2D, Point, geometry};
use rl_grid::{BitGrid, DijkstraMap, PathRules};
use rl_rules::{ActorView, Brain, Choice, Decision, MovementProfile, Snapshot, TacticCtx, Wits};

use crate::ability::Use;
use crate::combat::{Attack, CombatRng, Faction, Health};
use crate::components::{MyTurn, Player, Position, Viewshed};
use crate::doors::Open;
use crate::items::{EquipFromGround, PickUp};
use crate::lighting::{DarkSight, Lighting, perceives};
use crate::places::{MapId, OnMap};
use crate::throwing::Throw;
use crate::turn::{Acting, Action, AddAction, Intent, Occupancy, Step, Turns, Wait};
use crate::world::WorldMap;

/// How far a non-player notices things, in tiles. Sight is symmetric,
/// so a monster sees the player exactly when the player sees it and it
/// is within this range.
#[derive(Component, Debug, Clone, Copy)]
pub struct Perception(pub i32);

/// The movement class an actor paths with.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Profile(pub MovementProfile);

/// The brain deciding a non-player's turns. Shared, since most monsters of
/// a kind think alike.
///
/// Requires [`Intelligence`], sapient unless the spawn says otherwise, and
/// [`CameFrom`], so a wanderer knows not to step straight back.
#[derive(Component, Clone)]
#[component(on_add = report_mind_without_plugin)]
#[require(Intelligence, CameFrom)]
pub struct Mind(pub Arc<Brain<Entity>>);

/// What an actor is able to do, whatever its brain would like: whether it
/// runs, searches, and works doors.
///
/// Every [`Mind`] carries one, sapient unless the spawn says otherwise, so a
/// monster nobody flagged does all its brain asks. Flag the ones that should
/// not: `Intelligence(Wits::ANIMAL)` stops at a door, and
/// `Intelligence(Wits::MINDLESS)` fights to the death and forgets what it
/// cannot see. On an actor with no mind, the player included, it still
/// decides what its moves may do, such as open a door.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Deref)]
pub struct Intelligence(pub Wits);

/// The cell a mind stepped from on its last decision, if it stepped.
///
/// Written when a mind decides a step and read into
/// [`Snapshot::came_from`] on its next turn, so a wanderer drifts rather
/// than dithers. Nothing else reads it; a game that wants a longer trail
/// keeps its own.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CameFrom(pub Option<Point>);

/// The map a movement class paths by: its profile, and whether it opens
/// doors, which changes what a closed door costs.
type FieldKey = (MovementProfile, bool);

/// One approach map per movement class, rebuilt when the player moves or
/// the map changes where anyone may walk.
#[derive(Resource, Default)]
pub struct FlowFields {
    /// The player's cell and the map's cost epoch the maps were built for.
    built_at: Option<(Point, u64)>,
    approach: BTreeMap<FieldKey, DijkstraMap>,
    escape: BTreeMap<FieldKey, DijkstraMap>,
}

impl FlowFields {
    fn ensure(&mut self, key: FieldKey, player: Point, map: &WorldMap) {
        let stamp = (player, map.cost_epoch());
        if self.built_at != Some(stamp) {
            self.approach.clear();
            self.escape.clear();
            self.built_at = Some(stamp);
        }
        if self.approach.contains_key(&key) {
            return;
        }
        let view = if key.1 { map.opening_view() } else { map.view() };
        let Some(local) = map.to_local(player) else { return };
        let mut approach = DijkstraMap::covering(&view);
        approach.build(&view, [local], PathRules::default());
        let mut escape = approach.clone();
        escape.scale(-12, 10);
        escape.rescan(&view, PathRules::default());
        self.approach.insert(key, approach);
        self.escape.insert(key, escape);
    }

    /// The approach map for `profile`, as a mover that does or does not open
    /// doors reads it, if built this turn.
    pub fn approach(&self, profile: MovementProfile, opens_doors: bool) -> Option<&DijkstraMap> {
        self.approach.get(&(profile, opens_doors))
    }

    /// Forgets every map, so the next mind rebuilds them: the player
    /// changed maps, or the terrain changed under everyone.
    pub fn invalidate(&mut self) {
        self.built_at = None;
        self.approach.clear();
        self.escape.clear();
    }
}

/// The snapshot of the mind holding the turn, while the perceive stage
/// fills it.
///
/// Core state, inserted by `CorePlugin` beside `Acting` and `FlowFields`,
/// so a plugin that contributes to it in a game with no minds finds it
/// present and empty rather than gating on a resource that happens to be
/// there. Empty means no mind holds the turn this pass, or a game already
/// claimed the decision: every contributor asks [`snapshot_mut`](Self::snapshot_mut)
/// and gets `None`, and does nothing.
///
/// `hazards` is a `BitGrid` over the loaded window rather than a list of
/// cells, because [`TacticCtx::can_step`] is asked once per cell of a
/// scavenger's search and a fire is hundreds of cells.
#[derive(Resource)]
pub struct Thinking {
    actor: Option<Entity>,
    snapshot: Option<Snapshot<Entity>>,
    at: Point,
    reach: i32,
    dark_sight: i32,
    hazards: BitGrid,
    origin: Point,
    fields_allowed: bool,
}

impl Default for Thinking {
    fn default() -> Self {
        Self { actor: None, snapshot: None, at: Point::ZERO, reach: 0, dark_sight: 0, hazards: BitGrid::new(0, 0), origin: Point::ZERO, fields_allowed: true }
    }
}

impl Thinking {
    /// The mind deciding this pass, if one is.
    pub fn actor(&self) -> Option<Entity> {
        self.actor
    }

    /// Where it stands.
    pub fn at(&self) -> Point {
        self.at
    }

    /// How far it notices things.
    pub fn reach(&self) -> i32 {
        self.reach
    }

    /// Whether `p` is within its reach.
    pub fn within_reach(&self, p: Point) -> bool {
        geometry::chebyshev(self.at, p) <= self.reach
    }

    /// The snapshot being filled, for a contributor.
    pub fn snapshot_mut(&mut self) -> Option<&mut Snapshot<Entity>> {
        self.snapshot.as_mut()
    }

    /// The snapshot as it stands, for a contributor that reads before it
    /// writes.
    pub fn snapshot(&self) -> Option<&Snapshot<Entity>> {
        self.snapshot.as_ref()
    }

    /// Marks the world cell `p` as somewhere no mind will step this pass.
    pub fn mark_hazard(&mut self, p: Point) {
        self.hazards.insert(p - self.origin);
    }

    /// Whether `p` was marked.
    pub fn is_hazard(&self, p: Point) -> bool {
        self.hazards.contains(p - self.origin)
    }

    /// Whether the mind may descend the shared flow fields this pass.
    ///
    /// The fields are built toward the player, so a contributor that knows
    /// the mind has not noticed the player forbids them, or it would walk
    /// straight to someone it never saw. Allowed unless something says no.
    pub fn allow_fields(&mut self, allowed: bool) {
        self.fields_allowed = allowed;
    }

    fn open(&mut self, actor: Entity, snapshot: Snapshot<Entity>, at: Point, reach: i32, dark_sight: i32, map: &WorldMap) {
        let window = map.window_tiles();
        if self.hazards.width() != window.width || self.hazards.height() != window.height {
            self.hazards = BitGrid::new(window.width, window.height);
        } else {
            self.hazards.clear();
        }
        self.origin = window.origin();
        self.actor = Some(actor);
        self.snapshot = Some(snapshot);
        self.at = at;
        self.reach = reach;
        self.dark_sight = dark_sight;
        self.fields_allowed = true;
    }

    fn close(&mut self) -> Option<(Entity, Snapshot<Entity>)> {
        let actor = self.actor.take()?;
        let snapshot = self.snapshot.take()?;
        Some((actor, snapshot))
    }
}

/// Whether the mind holding the turn can perceive a cell, for a contributor
/// deciding what goes into the snapshot.
///
/// The one line-of-sight answer, shared with noticing so the two cannot
/// disagree about who could be seen. Lines are read off the player's
/// viewshed, which is symmetric, so an observer has a line to the player
/// exactly when the player has one to it, and to anyone else when the
/// player has one to them both. Light is not symmetric, so what it then
/// perceives along that line is whatever is lit, within its dark sight or
/// adjacent.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Sight<'w, 's> {
    player: Query<'w, 's, (&'static Position, &'static Viewshed), With<Player>>,
    lighting: Option<Res<'w, Lighting>>,
    map: Res<'w, WorldMap>,
}

impl Sight<'_, '_> {
    /// The map every thinker this pass stands on.
    pub fn current_map(&self) -> MapId {
        self.map.current()
    }

    /// Whether the thinker in `thinking` perceives `at` on the map `on`:
    /// the same map, within its reach, and in its sight.
    pub fn perceives(&self, thinking: &Thinking, at: Point, on: Option<&OnMap>) -> bool {
        if on.map(|m| m.0).unwrap_or(MapId::SURFACE) != self.map.current() || !thinking.within_reach(at) {
            return false;
        }
        let Ok((player_pos, player_sight)) = self.player.single() else { return false };
        perceivable(player_pos.0, player_sight, self.lighting.as_deref(), thinking.at, thinking.dark_sight, at)
    }
}

/// Whether an observer at `from` perceives `to`: a line through the
/// player's viewshed, then light, dark sight or adjacency.
pub fn perceivable(player_pos: Point, player_sight: &Viewshed, lighting: Option<&Lighting>, from: Point, dark_sight: i32, to: Point) -> bool {
    let in_line = player_sight.in_line(from) && (to == player_pos || player_sight.in_line(to));
    in_line && perceives(lighting, from, dark_sight, to)
}

/// A mind holding the turn: not the player, whose turn is the game's.
type MindsTurn = (With<Mind>, With<MyTurn>, Without<Player>);

/// The mind holding the turn, as the stage opens it.
type Thinker = (
    Entity,
    &'static Position,
    &'static Health,
    &'static Faction,
    Option<&'static Perception>,
    Option<&'static Intelligence>,
    Option<&'static DarkSight>,
    &'static CameFrom,
);

/// Whether a mind holds the turn, which is when the perceive stage has
/// anything to do. A run condition on the whole stage, so a pass for the
/// player costs no contributor a dispatch.
pub fn a_mind_holds_the_turn(minds: Query<(), MindsTurn>) -> bool {
    !minds.is_empty()
}

/// Opens the snapshot for the mind holding the turn: who it is, where it
/// stands, what it can do and where it came from, and nothing it sees yet.
/// Leaves it closed when a game has already decided for the actor, so no
/// contributor works for a decision nobody will make.
pub fn begin_thinking(mut thinking: ResMut<Thinking>, acting: Res<Acting>, map: Res<WorldMap>, minds: Query<Thinker, MindsTurn>) {
    thinking.close();
    let Ok((actor, pos, health, faction, perception, intelligence, dark, came_from)) = minds.single() else { return };
    if acting.has_decided(actor) {
        return;
    }
    let me = ActorView { id: actor, pos: pos.0, hp: health.current, max_hp: health.max, faction: faction.0 };
    let mut snapshot = Snapshot::alone(me);
    snapshot.wits = intelligence.map(|i| i.0).unwrap_or_default();
    snapshot.came_from = came_from.0;
    let reach = perception.map(|p| p.0).unwrap_or(8);
    thinking.open(actor, snapshot, pos.0, reach, dark.map(|d| d.0).unwrap_or(0), &map);
}

/// The shared state a mind reads and the stream it draws from.
#[derive(bevy::ecs::system::SystemParam)]
pub struct MindWorld<'w, 's> {
    fields: ResMut<'w, FlowFields>,
    rng: ResMut<'w, CombatRng>,
    map: Res<'w, WorldMap>,
    occupancy: Res<'w, Occupancy>,
    turns: Res<'w, Turns>,
    /// Where the shared fields point.
    player: Query<'w, 's, &'static Position, With<Player>>,
}

/// A mind chose something of the game's own: what its tactic returned,
/// and who chose it.
///
/// Written in [`DecideSet::Minds`](crate::plugin::DecideSet::Minds) and
/// answered in [`DecideSet::Game`](crate::plugin::DecideSet::Game). A game
/// rarely reads it: [`AddChoice::add_choice`] turns a choice that is also
/// an action into its intent, once, for every game. The actor's decision
/// is already claimed, so nothing else will decide for it this pass.
#[derive(Message, Debug)]
pub struct MindChose {
    /// Who chose.
    pub actor: Entity,
    /// What.
    pub choice: Box<dyn Choice>,
}

impl MindChose {
    /// The choice, if it is an `A`.
    pub fn as_choice<A: Choice>(&self) -> Option<&A> {
        let any: &dyn std::any::Any = &*self.choice;
        any.downcast_ref::<A>()
    }
}

/// Lets a mind choose an action of the game's own.
pub trait AddChoice {
    /// Registers `A` as an action, with its sweeper, and routes every
    /// [`MindChose`] carrying an `A` to an `Intent<A>` in
    /// [`DecideSet::Game`](crate::plugin::DecideSet::Game).
    ///
    /// A game writes the action type, the resolver and the tactic, the
    /// same three things tutorial chapter 9 writes for a player's action,
    /// and this line. A choice nobody routed is a choice nobody resolves,
    /// which the sweeper refuses rather than losing.
    fn add_choice<A: Action + Choice + Clone>(&mut self) -> &mut Self;
}

impl AddChoice for App {
    fn add_choice<A: Action + Choice + Clone>(&mut self) -> &mut Self {
        // Once: a game that already registered the action for its player
        // must not get a second sweeper.
        if !self.world().contains_resource::<Messages<Intent<A>>>() {
            self.add_action::<A>();
        }
        self.add_message::<MindChose>().add_systems(crate::plugin::Turn, route_choice::<A>.in_set(crate::plugin::DecideSet::Game))
    }
}

/// Turns each choice of type `A` into the intent for it.
fn route_choice<A: Action + Choice + Clone>(mut chose: MessageReader<MindChose>, mut out: MessageWriter<Intent<A>>) {
    for c in chose.read() {
        if let Some(choice) = c.as_choice::<A>() {
            out.write(Intent::new(c.actor, choice.clone()));
        }
    }
}

/// What a mind writes when it decides.
#[derive(bevy::ecs::system::SystemParam)]
pub struct MindIntents<'w> {
    moves: MessageWriter<'w, Intent<Step>>,
    opens: MessageWriter<'w, Intent<Open>>,
    abilities: MessageWriter<'w, Intent<Use>>,
    attacks: MessageWriter<'w, Intent<Attack>>,
    waits: MessageWriter<'w, Intent<Wait>>,
    pick_ups: MessageWriter<'w, Intent<PickUp>>,
    equips: MessageWriter<'w, Intent<EquipFromGround>>,
    throws: MessageWriter<'w, Intent<Throw>>,
    chose: MessageWriter<'w, MindChose>,
}

/// The mind holding the turn, as the decision reads it.
type Deciding = (&'static Mind, &'static Position, Option<&'static Profile>, Option<&'static Intelligence>, &'static mut CameFrom);

/// Lets the mind holding the turn decide it, from the snapshot the
/// perceive stage filled.
///
/// The snapshot is sorted here and nowhere else, so the order the
/// contributors ran in cannot reach a tactic. A game that decides for an
/// actor itself claims it in [`TurnSet::Decide`](crate::plugin::TurnSet::Decide),
/// and the stage never opens for that actor.
pub fn decide_minds(
    mut thinking: ResMut<Thinking>,
    mut intents: MindIntents,
    mut acting: ResMut<Acting>,
    mut world: MindWorld,
    mut minds: Query<Deciding, With<MyTurn>>,
) {
    let Some((thinker, mut snapshot)) = thinking.close() else { return };
    let Ok((mind, my_pos, profile, intelligence, mut came_from)) = minds.get_mut(thinker) else { return };
    let thinking = &*thinking;
    let MindWorld { fields, rng, map, occupancy, turns, player } = &mut world;
    let (fields, rng, map, occupancy, turns) = (&mut **fields, &mut **rng, &**map, &**occupancy, &**turns);
    let wits = intelligence.map(|i| i.0).unwrap_or_default();
    let key = (profile.map(|p| p.0).unwrap_or_default(), wits.has(Wits::OPENS_DOORS));
    snapshot.sort();

    // The shared flow fields are built toward the player, and only for a
    // mind that has something to hunt and that no contributor forbade them.
    let maps_allowed = thinking.fields_allowed;
    if !snapshot.enemies.is_empty()
        && maps_allowed
        && let Ok(player) = player.single()
    {
        fields.ensure(key, player.0, map);
    }
    let origin = map.window_tiles().origin();
    // Maps are window-local; translate through a local copy of the
    // decision so tactics stay in world coordinates.
    let approach = fields.approach.get(&key);
    let escape = fields.escape.get(&key);
    // A closed door is a step for a mind that opens doors, since stepping
    // into one opens it, and a wall for one that does not. Nor is a cell
    // some contributor marked a hazard anywhere a mind will step.
    let can_step = |p: Point| (map.is_walkable(p) || (key.1 && map.opens(p).is_some())) && !occupancy.is_occupied(p) && !thinking.is_hazard(p);
    // The predicate the ability resolver uses, so what a tactic thinks a
    // shape will cover is what it does cover.
    let blocks_shot = |p: Point| map.blocks_projectiles(p) || occupancy.is_occupied(p);
    let mut turn_rng: StdRng =
        rand::SeedableRng::seed_from_u64(rl_core::seed::position_hash(turns.now() as u64 ^ rand::RngCore::next_u64(&mut rng.0), my_pos.0.x, my_pos.0.y));
    let shifted = |m: &DijkstraMap| shift_map(m, origin);
    let approach_world = approach.filter(|_| maps_allowed).map(shifted);
    let escape_world = escape.filter(|_| maps_allowed).map(shifted);
    let mut ctx = TacticCtx {
        snapshot: &snapshot,
        approach: approach_world.as_ref(),
        escape: escape_world.as_ref(),
        can_step: &can_step,
        blocks_shot: &blocks_shot,
        bounds: map.window_tiles(),
        rng: &mut turn_rng,
    };
    let (decision, _which) = mind.0.decide(&mut ctx);
    if !acting.claim_decision(thinker) {
        return;
    }
    came_from.0 = None;
    match decision {
        // A step onto a shut door is the turn spent opening it: the door is
        // its own action, and the mind knows what it is walking into.
        Decision::Step(to) => match Direction::between(my_pos.0, to) {
            Some(d) if map.opens(to).is_some() => {
                intents.opens.write(Intent::new(thinker, Open(d)));
            }
            Some(d) => {
                came_from.0 = Some(my_pos.0);
                intents.moves.write(Intent::new(thinker, Step(d)));
            }
            None => {
                intents.waits.write(Intent::new(thinker, Wait));
            }
        },
        Decision::Attack(target) => {
            intents.attacks.write(Intent::new(thinker, Attack(target)));
        }
        Decision::Ability { ability, aim } => {
            intents.abilities.write(Intent::new(thinker, Use { ability, aim }));
        }
        Decision::Wait => {
            intents.waits.write(Intent::new(thinker, Wait));
        }
        Decision::PickUp => {
            intents.pick_ups.write(Intent::new(thinker, PickUp));
        }
        Decision::EquipFromGround(item) => {
            intents.equips.write(Intent::new(thinker, EquipFromGround(item)));
        }
        Decision::Throw { item, at } => {
            intents.throws.write(Intent::new(thinker, Throw { item, at }));
        }
        // The game's own: hand it back and let the game act on it.
        Decision::Own(choice) => {
            intents.chose.write(MindChose { actor: thinker, choice });
        }
    }
}

/// A copy of a window-local map re-addressed in world coordinates.
fn shift_map(m: &DijkstraMap, origin: Point) -> DijkstraMap {
    let r = m.region();
    let mut out = DijkstraMap::new(rl_core::Rect::new(r.x + origin.x, r.y + origin.y, r.width, r.height));
    out.copy_values_from(m);
    out
}

/// Minds: every non-player carrying a [`Mind`] decides its own turn.
///
/// Needs combat, for the stream a mind rolls from, and the field of view,
/// because the player's viewshed is the line-of-sight oracle. What a mind
/// knows comes from whichever plugins the game added: with none of them a
/// mind sees nobody and drifts; with combat it sees sides; with abilities
/// it is offered what it may use; and so on.
///
/// A game that decides every monster's turn with systems of its own leaves
/// this out. One that spawns a [`Mind`] without it is told so, once, rather
/// than left watching monsters that never move.
pub struct MindsPlugin;

impl Plugin for MindsPlugin {
    fn build(&self, app: &mut App) {
        use crate::plugin::{DecideSet, PerceiveSet, Turn};
        // The minds may choose an ability, a pickup or a throw, so the
        // messages they would write them into exist whether or not the game
        // added abilities, items or throwing. Registering one twice is what
        // `add_message` is built for.
        app.init_resource::<MindsRunning>()
            .add_message::<Intent<Use>>()
            .add_message::<Intent<PickUp>>()
            .add_message::<Intent<EquipFromGround>>()
            .add_message::<Intent<Throw>>()
            .add_message::<MindChose>()
            .add_systems(Turn, begin_thinking.in_set(PerceiveSet::Begin))
            .add_systems(Turn, decide_minds.in_set(DecideSet::Minds));
    }

    fn finish(&self, app: &mut App) {
        crate::plugin::depends_on::<crate::combat::CombatPlugin>(app, "MindsPlugin");
        crate::plugin::depends_on::<crate::fov::FovPlugin>(app, "MindsPlugin");
    }
}

/// Present while [`MindsPlugin`] is added, so a [`Mind`] spawned without it
/// can say so.
#[derive(Resource, Default)]
struct MindsRunning;

/// Set once a [`Mind`] without [`MindsPlugin`] has been reported, so the
/// report is made once and not per monster.
#[derive(Resource, Default)]
struct ToldMindsAreOff;

/// Reports the first [`Mind`] added to a world with no [`MindsPlugin`].
fn report_mind_without_plugin(mut world: DeferredWorld, _: HookContext) {
    if world.contains_resource::<MindsRunning>() || world.contains_resource::<ToldMindsAreOff>() {
        return;
    }
    world.commands().init_resource::<ToldMindsAreOff>();
    error!("an actor was given a `Mind`, but `MindsPlugin` was not added, so nothing will ever decide its turns; add `MindsPlugin` beside `CombatPlugin`");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{Armor, CombatPlugin, MeleeAttack};
    use crate::components::{Actor, Blocks};
    use crate::items::{Equipped, GearScore, Inventory, Item, Wearable};
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use crate::throwing::Throwable;
    use crate::turn::Resolution;
    use rl_core::DiceRoll;
    use rl_rules::ai::tactics::{Hunt, MeleeAdjacent};
    use rl_rules::damage::DamageKindId;

    fn arena() -> (App, Point, DamageKindId) {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, MindsPlugin, crate::world::StreamingPlugin));
        let start = crate::testing::surface(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        (app, start, sides.kind)
    }

    /// A tactic of the game's own, at the top of the priority list.
    struct Shove;
    impl rl_rules::ai::Tactic<Entity> for Shove {
        fn name(&self) -> &'static str {
            "shove"
        }
        fn evaluate(&self, ctx: &mut rl_rules::ai::TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
            ctx.snapshot.enemies.first().map(|e| Decision::own(Shoved(e.id)))
        }
    }

    /// The game's action, which the engine has never heard of, and the
    /// choice a tactic makes of it: one type, two traits.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Shoved(Entity);
    impl crate::turn::Action for Shoved {}
    impl Choice for Shoved {
        fn name(&self) -> &'static str {
            "shoved"
        }
    }

    #[derive(Resource, Default)]
    struct Shoves(u32);

    fn resolve_shoves(mut intents: MessageReader<Intent<Shoved>>, mut resolution: Resolution, mut count: ResMut<Shoves>) {
        for intent in intents.read() {
            if !resolution.claim(intent.actor) {
                continue;
            }
            count.0 += 1;
            resolution.done(intent.actor, rl_core::turn::BASE_ACTION_COST);
        }
    }

    #[test]
    fn a_mind_can_choose_an_action_the_engine_never_heard_of() {
        let (mut app, start, blunt) = arena();
        app.init_resource::<Shoves>().add_choice::<Shoved>().add_systems(crate::plugin::Turn, resolve_shoves.in_set(crate::plugin::ResolveSet::Act));
        let us = rl_rules::FactionId::from_raw(0);
        let them = rl_rules::FactionId::from_raw(1);
        let player = app
            .world_mut()
            .spawn((
                Actor,
                Player,
                Blocks,
                Position(start),
                Viewshed::new(8),
                Health::full(30),
                Faction(us),
                MeleeAttack { kind: blunt, dice: DiceRoll::flat(1) },
            ))
            .id();
        // Shove first, strike second: the game's tactic outranks the
        // engine's, which is what the priority list is for.
        let brain = Arc::new(Brain::new().then(Shove).then(MeleeAdjacent));
        app.world_mut().spawn((
            Actor,
            Blocks,
            Position(start.offset(1, 0)),
            Health::full(5),
            Faction(them),
            Perception(8),
            MeleeAttack { kind: blunt, dice: DiceRoll::flat(3) },
            Mind(brain),
        ));
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);

        for _ in 0..6 {
            if app.world().get::<MyTurn>(player).is_some() {
                app.world_mut().write_message(Intent::new(player, Wait));
            }
            app.update();
        }
        assert!(app.world().resource::<Shoves>().0 > 0, "the monster shoved");
        assert_eq!(app.world().get::<Health>(player).unwrap().current, 30, "and never struck, because shoving outranks it");
    }

    #[test]
    fn a_monster_hunts_strikes_and_dies() {
        let (mut app, start, blunt) = arena();
        let us = rl_rules::FactionId::from_raw(0);
        let them = rl_rules::FactionId::from_raw(1);
        let player = app
            .world_mut()
            .spawn((
                Actor,
                Player,
                Blocks,
                Position(start),
                Viewshed::new(8),
                Health::full(30),
                Faction(us),
                Armor(1),
                MeleeAttack { kind: blunt, dice: DiceRoll::flat(50) },
            ))
            .id();
        let brain = Arc::new(Brain::new().then(MeleeAdjacent).then(Hunt));
        let monster = app
            .world_mut()
            .spawn((
                Actor,
                Blocks,
                Position(start.offset(4, 0)),
                Health::full(5),
                Faction(them),
                Perception(8),
                MeleeAttack { kind: blunt, dice: DiceRoll::flat(3) },
                Mind(brain),
            ))
            .id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        // The player waits; the monster closes and strikes.
        let mut hits = 0;
        for _ in 0..40 {
            if app.world().get::<MyTurn>(player).is_some() {
                app.world_mut().write_message(Intent::new(player, Wait));
            }
            app.update();
            let hp = app.world().get::<Health>(player).unwrap().current;
            if hp < 30 {
                hits = 30 - hp;
                break;
            }
        }
        assert_eq!(hits, 2, "3 damage minus 1 armor");
        let mpos = app.world().get::<Position>(monster).unwrap().0;
        assert!(geometry::is_adjacent(mpos, start), "the monster closed in: {mpos:?}");
        // The player strikes back and the monster is removed.
        for _ in 0..10 {
            if app.world().get::<MyTurn>(player).is_some() {
                app.world_mut().write_message(Intent::new(player, Attack(monster)));
            }
            app.update();
            if app.world().get_entity(monster).is_err() {
                break;
            }
        }
        assert!(app.world().get_entity(monster).is_err(), "the monster despawned by the end of the frame");
        assert!(!app.world().resource::<Occupancy>().is_occupied(mpos));
        assert!(!app.world().resource::<Turns>().contains(monster));
    }

    /// The player behind a ring of wall with one gate in it, a gate that can
    /// be seen through, and a monster of `wits` outside it hunting.
    fn behind_a_gate(wits: Wits) -> (App, Entity, Entity, Point, rl_grid::TileRegistry) {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, MindsPlugin, crate::world::StreamingPlugin));
        let mut tiles = rl_grid::TileRegistry::standard();
        tiles.register(rl_grid::TileProps::named("gate").passable(true).opens_to("gate_open")).unwrap();
        tiles.register(rl_grid::TileProps::floor("gate_open").closes_to("gate")).unwrap();
        let start = crate::testing::surface_with(&mut app, tiles.clone());
        let sides = crate::testing::two_sides(&mut app);
        let player = app
            .world_mut()
            .spawn((
                Actor,
                Player,
                Blocks,
                Position(start),
                Viewshed::new(8),
                Health::full(30),
                Faction(sides.ours),
                MeleeAttack { kind: sides.kind, dice: DiceRoll::flat(1) },
            ))
            .id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        let gate = start.offset(3, 0);
        {
            let mut map = app.world_mut().resource_mut::<WorldMap>();
            for dy in -3..=3 {
                for dx in -3..=3 {
                    let p = start.offset(dx, dy);
                    if geometry::chebyshev(p, start) == 3 {
                        map.set_tile(p, if p == gate { tiles.expect("gate") } else { tiles.expect("wall") });
                    }
                }
            }
        }
        let monster = app
            .world_mut()
            .spawn((
                Actor,
                Blocks,
                Position(start.offset(6, 0)),
                Health::full(20),
                Faction(sides.theirs),
                Perception(8),
                MeleeAttack { kind: sides.kind, dice: DiceRoll::flat(3) },
                Mind(Arc::new(Brain::new().then(MeleeAdjacent).then(Hunt))),
                Intelligence(wits),
            ))
            .id();
        for _ in 0..40 {
            if app.world().get::<MyTurn>(player).is_some() {
                app.world_mut().write_message(Intent::new(player, Wait));
            }
            app.update();
        }
        (app, player, monster, gate, tiles)
    }

    #[test]
    fn a_mind_that_works_doors_comes_through_a_gate_an_animal_cannot() {
        let (app, player, monster, gate, tiles) = behind_a_gate(Wits::SAPIENT);
        assert_eq!(app.world().resource::<WorldMap>().tile(gate), Some(tiles.expect("gate_open")), "it opened the gate");
        assert!(app.world().get::<Health>(player).unwrap().current < 30, "and came through to strike");

        let (app, player, monster_outside, gate, tiles) = behind_a_gate(Wits::ANIMAL);
        assert_eq!(app.world().resource::<WorldMap>().tile(gate), Some(tiles.expect("gate")), "the gate held");
        assert_eq!(app.world().get::<Health>(player).unwrap().current, 30, "and nothing reached the player");
        let outside = app.world().get::<Position>(monster_outside).unwrap().0;
        let start = app.world().get::<Position>(player).unwrap().0;
        assert!(geometry::chebyshev(outside, start) > 3, "it waits outside the ring: {outside:?}");
        let _ = monster;
    }

    /// A thrower with nothing in hand, a knife a step away and the player six
    /// off, run for a dozen turns of the player waiting.
    fn with_a_knife_in_reach(wits: Wits) -> (App, Entity, Entity) {
        use rl_rules::ai::tactics::{Scavenge, ThrowAtRange};
        let mut app = headless_app();
        app.add_plugins((
            crate::fov::FovPlugin,
            CombatPlugin,
            MindsPlugin,
            crate::items::ItemsPlugin,
            crate::throwing::ThrowingPlugin,
            crate::world::StreamingPlugin,
        ));
        let start = crate::testing::surface(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        let player = app
            .world_mut()
            .spawn((
                Actor,
                Player,
                Blocks,
                Position(start),
                Viewshed::new(8),
                Health::full(30),
                Faction(sides.ours),
                MeleeAttack { kind: sides.kind, dice: DiceRoll::flat(1) },
            ))
            .id();
        let knife = app.world_mut().spawn((Item, Position(start.offset(5, 0)), Throwable { range: 6, strike: Some((sides.kind, DiceRoll::flat(4))) })).id();
        app.world_mut().spawn((
            Actor,
            Blocks,
            Position(start.offset(6, 0)),
            Health::full(10),
            Faction(sides.theirs),
            Perception(8),
            Inventory::default(),
            Mind(Arc::new(Brain::new().then(MeleeAdjacent).then(ThrowAtRange::default()).then(Scavenge { reach: 3 }))),
            Intelligence(wits),
        ));
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        for _ in 0..12 {
            if app.world().get::<MyTurn>(player).is_some() {
                app.world_mut().write_message(Intent::new(player, Wait));
            }
            app.update();
        }
        (app, player, knife)
    }

    #[test]
    fn a_sapient_mind_fetches_a_knife_it_sees_and_throws_it_where_an_animal_walks_past() {
        let (app, player, knife) = with_a_knife_in_reach(Wits::SAPIENT);
        assert_eq!(app.world().get::<Health>(player).unwrap().current, 26, "it took the knife up and threw it");
        let at_player = app.world().get::<Position>(player).unwrap().0;
        assert_eq!(app.world().get::<Position>(knife).map(|p| p.0), Some(at_player), "and the knife lies at the player's feet");

        let (app, player, knife) = with_a_knife_in_reach(Wits::ANIMAL);
        assert_eq!(app.world().get::<Health>(player).unwrap().current, 30, "an animal has no use for a knife");
        let start = app.world().get::<Position>(player).unwrap().0;
        assert_eq!(app.world().get::<Position>(knife).map(|p| p.0), Some(start.offset(5, 0)), "which lies where it lay");
    }

    #[test]
    fn a_mind_that_equips_puts_on_better_gear_it_finds_and_leaves_worse_alone() {
        use rl_rules::ai::tactics::Scavenge;
        use rl_rules::{EquipShape, Equipment, SlotId};
        let (mut app, start, _) = arena();
        app.add_plugins(crate::items::ItemsPlugin);
        let sides = crate::testing::two_sides(&mut app);
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), Health::full(30), Faction(sides.ours))).id();
        let hand = EquipShape::in_slot(SlotId::from_raw(0));
        let old = app.world_mut().spawn((Item, Wearable(hand.clone()), GearScore(1))).id();
        let better = app.world_mut().spawn((Item, Position(start.offset(4, 0)), Wearable(hand.clone()), GearScore(3))).id();
        let worse = app.world_mut().spawn((Item, Position(start.offset(2, 0)), Wearable(hand.clone()), GearScore(0))).id();
        let mut worn = Equipment::with_slot_count(1);
        worn.equip(old, &hand).unwrap();
        let marine = app
            .world_mut()
            .spawn((
                Actor,
                Blocks,
                Position(start.offset(3, 0)),
                Health::full(10),
                Faction(sides.theirs),
                Perception(8),
                Inventory { items: vec![old] },
                Equipped(worn),
                Mind(Arc::new(Brain::new().then(Scavenge { reach: 3 }))),
                Intelligence(Wits::SAPIENT),
            ))
            .id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        for _ in 0..8 {
            if app.world().get::<MyTurn>(player).is_some() {
                app.world_mut().write_message(Intent::new(player, Wait));
            }
            app.update();
        }
        let w = app.world();
        assert_eq!(w.get::<Equipped>(marine).unwrap().in_slot(SlotId::from_raw(0)), Some(better), "it put on the better one");
        assert!(w.get::<Inventory>(marine).unwrap().contains(old), "and kept the old one in its bag");
        assert_eq!(w.get::<Position>(worse).map(|p| p.0), Some(start.offset(2, 0)), "and left the worse one lying");
    }

    #[test]
    fn a_mind_spawned_without_the_plugin_is_reported_once() {
        let mut without = headless_app();
        without.add_plugins((crate::fov::FovPlugin, CombatPlugin));
        without.world_mut().spawn(Mind(Arc::new(Brain::new())));
        without.world_mut().spawn(Mind(Arc::new(Brain::new())));
        without.update();
        assert!(without.world().contains_resource::<ToldMindsAreOff>(), "a mind nothing will run is reported");

        let mut with = headless_app();
        with.add_plugins((crate::fov::FovPlugin, CombatPlugin, MindsPlugin));
        with.world_mut().spawn(Mind(Arc::new(Brain::new())));
        with.update();
        assert!(!with.world().contains_resource::<ToldMindsAreOff>(), "and one the plugin runs is not");
    }

    /// What a game knows and the engine does not reaches the game's own
    /// tactic through the snapshot, by type: a scent laid in
    /// `PerceiveSet::Annotate`, followed by a tactic that reads it, with
    /// no id anywhere and nothing in the minds module edited.
    #[test]
    fn a_games_own_sense_pushed_in_perceive_reaches_its_own_tactic() {
        use crate::plugin::{PerceiveSet, Turn};

        /// Where the game says the thing worth walking to is.
        #[derive(Debug, Clone, Copy)]
        struct Scent(Point);

        /// The game's contributor: every mind smells the same spot.
        fn lay_scent(mut thinking: ResMut<Thinking>, marker: Res<Marker>) {
            let at = marker.0;
            if let Some(snapshot) = thinking.snapshot_mut() {
                snapshot.add_sense(Scent(at));
            }
        }

        #[derive(Resource)]
        struct Marker(Point);

        /// The game's tactic: walk toward the scent.
        struct FollowScent;
        impl rl_rules::ai::Tactic<Entity> for FollowScent {
            fn name(&self) -> &'static str {
                "follow_scent"
            }
            fn evaluate(&self, ctx: &mut rl_rules::ai::TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
                let target = ctx.snapshot.sense::<Scent>()?.0;
                let me = ctx.snapshot.me.pos;
                let d = Direction::between(me, target)?;
                let step = me + d.offset();
                (ctx.can_step)(step).then_some(Decision::Step(step))
            }
        }

        let (mut app, start, _) = arena();
        app.insert_resource(Marker(start.offset(-5, 0))).add_systems(Turn, lay_scent.in_set(PerceiveSet::Annotate));
        let player =
            app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), Health::full(30), Faction(rl_rules::FactionId::from_raw(0)))).id();
        let sniffer = app
            .world_mut()
            .spawn((
                Actor,
                Blocks,
                Position(start.offset(4, 0)),
                Health::full(5),
                Faction(rl_rules::FactionId::from_raw(1)),
                Perception(2),
                Mind(Arc::new(Brain::new().then(FollowScent))),
            ))
            .id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        for _ in 0..4 {
            if app.world().get::<MyTurn>(player).is_some() {
                app.world_mut().write_message(Intent::new(player, Wait));
            }
            app.update();
        }
        let at = app.world().get::<Position>(sniffer).unwrap().0;
        assert!(at.x < start.x + 4, "it walked toward the scent, which only the game knew: {at:?}");
        assert!(app.world().get::<CameFrom>(sniffer).unwrap().0.is_some(), "and remembers where it stepped from");
    }
}
