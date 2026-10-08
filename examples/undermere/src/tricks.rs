//! The bestiary's tricks that needed a game system of their own, named by
//! [`Tricks`] off a monster's traits at load: a pink jelly splitting when
//! struck, a monkey stealing and fleeing for it, a wraith unseen in a
//! dark cell, a vampire bat's erratic step, a salamander's trail of fire,
//! a golem turning magic back, a troll whose regeneration stops while it
//! burns, and a rat's own pack or a band's own leader, which are brain
//! shapes rather than anything reactive.
//!
//! `splitting` reads `DamageDealt` rather than the trait model's own
//! `struck`, which fires before a blow's damage is known; `fire-trail`
//! reads `Stepped`; `shrouded` and `burns-badly`'s halted mending read
//! what is true of a body every turn, the way `keep_fireproof` does.
//! `stealing`'s taking and `withering`'s capped stacking are effects,
//! landed by the engine's actor triggers exactly as `venomous`'s bite is,
//! each registered in [`crate::content::add_game_effects`].

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_bevy::EffectRng;
use rl_engine::rl_rules::ability::{RawValue, read_args};
use rl_engine::rl_rules::ai::brain::Tactic;
use rl_engine::rl_rules::ai::tactics::Ways;

use crate::bestiary::{Kind, spawn_monster_now};
use crate::effects::free_cells_near;
use crate::traits::TraitDef;

/// Which of the tricks needing a game system of its own a kind's traits
/// name, read once at load against the traits a monster lists rather
/// than by checking a name on every turn. Every other field the trait
/// model already folds; these are brain shapes and marker components
/// instead.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tricks {
    /// `splitting`: splits in two when a blow leaves it alive.
    pub splits: bool,
    /// `stealing`: a landed blow takes a thing, then flees with it.
    pub steals: bool,
    /// `shrouded`: unseen while it stands in a dark cell.
    pub shrouds: bool,
    /// `flitting`: one step in three is random.
    pub flits: bool,
    /// `fire-trail`: its steps ignite the ground behind it.
    pub fire_trail: bool,
    /// `reflecting`: the engine's own `Reflects`, marked at spawn.
    pub reflects: bool,
    /// `packing`: keeps station on its own kind.
    pub packs: bool,
    /// `led`: keeps station on its one leader.
    pub led: bool,
    /// `burns-badly`: its regeneration stops while it burns.
    pub burns_badly: bool,
    /// `draining`: mends itself what a landed blow of its own dealt.
    pub drains: bool,
}

impl Tricks {
    /// Reads every trick off `listed`, the trait ids a monster names, in
    /// `traits`.
    pub fn from_listed(listed: &[Id<TraitDef>], traits: &Registry<TraitDef>) -> Self {
        let has = |name: &str| traits.id(name).is_some_and(|id| listed.contains(&id));
        Self {
            splits: has("splitting"),
            steals: has("stealing"),
            shrouds: has("shrouded"),
            flits: has("flitting"),
            fire_trail: has("fire-trail"),
            reflects: has("reflecting"),
            packs: has("packing"),
            led: has("led"),
            burns_badly: has("burns-badly"),
            drains: has("draining"),
        }
    }

    /// Whether any of these needs a marker component at spawn, for a
    /// `TurnSet::React` system to find by: every one but `packs`, `led`
    /// and `flits`, which are brain shapes built once at load and nothing
    /// a running turn has to look up.
    pub fn needs_marker(self) -> bool {
        self.splits || self.shrouds || self.fire_trail || self.reflects || self.burns_badly || self.drains
    }
}

/// Splits into two when a blow leaves it alive, each with half its
/// health: a pink jelly's `splitting`. Marked at spawn off [`Tricks::splits`],
/// and judged by [`judge_splits`] against `DamageDealt` rather than the
/// trait model's own `struck`, which fires before a blow's damage is
/// known.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Splits;

/// The least health a split leaves either half with, the trait's own
/// words: "none below 2".
const SPLIT_MIN_HEALTH: i32 = 2;

/// Splits whoever a blow of this pass left alive and marked [`Splits`]:
/// half its current health each, rounded down and never below
/// [`SPLIT_MIN_HEALTH`], the second made on a free cell beside the first.
/// Nothing for one the blow killed, read fresh off its `Health` rather
/// than off the event, and nothing for one with no free cell to stand a
/// second half on.
///
/// In `TurnSet::React`, after `apply_damage` has written `DamageDealt`.
pub fn judge_splits(mut dealt: MessageReader<DamageDealt>, splitting: Query<(), With<Splits>>, mut commands: Commands) {
    let mut struck: Vec<Entity> = Vec::new();
    for hit in dealt.read().filter(|d| d.dealt > 0 && splitting.contains(d.target)) {
        if !struck.contains(&hit.target) {
            struck.push(hit.target);
        }
    }
    for target in struck {
        commands.queue(move |world: &mut World| {
            try_split(world, target);
        });
    }
}

/// `target`, as [`judge_splits`] says, and whether it did.
fn try_split(world: &mut World, target: Entity) -> bool {
    if world.get::<Splits>(target).is_none() {
        return false;
    }
    let Some(health) = world.get::<Health>(target).copied() else { return false };
    if health.current <= 0 {
        return false;
    }
    let Some(kind) = world.get::<Kind>(target).map(|k| k.0) else { return false };
    let Some(at) = world.get::<Position>(target).map(|p| p.0) else { return false };
    let map = world.get::<OnMap>(target).map_or(MapId::SURFACE, |m| m.0);
    let beside = free_cells_near(world.resource::<WorldMap>(), world.resource::<Occupancy>(), at, 1, 1);
    let Some(second_at) = beside.first().copied() else { return false };
    let half = (health.current / 2).max(SPLIT_MIN_HEALTH).min(health.max);
    world.entity_mut(target).insert(Health { current: half, max: health.max });
    let made = spawn_monster_now(world, kind, second_at, map);
    world.entity_mut(made).insert(Health { current: half, max: health.max });
    // Told only where it is seen, as `withering`'s own touch is by the
    // engine's own narrator: a split in a room nobody is watching is not
    // something the adventurer learns of from the log either.
    if seen_by_player(world, at) {
        let name = world.get::<Name>(target).map_or_else(String::new, |n| n.as_str().to_string());
        world.write_message(Tell::new(format!("{} splits into two!", crate::effects::the(&name)), Tones::NOTICE));
    }
    true
}

/// Whether the adventurer can see `at` right now: the one check every
/// trick's own narration below makes before naming what it did, so the
/// log never says what a trick is before the look panel would.
fn seen_by_player(world: &mut World, at: Point) -> bool {
    world.query_filtered::<&Viewshed, With<Player>>().iter(world).any(|v| v.can_see(at))
}

/// Marks a kind that mends itself what a landed blow of its own dealt: a
/// vampire bat's and a wight's `draining`. Judged by [`judge_draining`]
/// against `DamageDealt`, since nothing yet knows what a bite "dealt" at
/// the moment `hit` itself fires, which is before a blow's damage is
/// resolved.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Drains;

/// Mends every `Drains` attacker what its own landed blow dealt this
/// pass, by the engine's `Heal`, scaled by its own mending as any heal is.
///
/// `hit.attacker` rather than `hit.credit`: the one who triggers an
/// on-hit rider, which is what draining is, and never damage over time or
/// splash crediting a kill to somebody who threw nothing this pass.
///
/// In `TurnSet::React`, after `apply_damage` has written `DamageDealt`.
pub fn judge_draining(mut dealt: MessageReader<DamageDealt>, draining: Query<(), With<Drains>>, mut heals: MessageWriter<Heal>) {
    for hit in dealt.read().filter(|d| d.dealt > 0) {
        if let Some(attacker) = hit.hit.attacker
            && draining.contains(attacker)
        {
            heals.write(Heal::new(attacker, hit.dealt).by(attacker));
        }
    }
}

/// Marks a thief that has just taken something and should run rather than
/// fight for it: set the moment [`steal`] lands, and read as the
/// [`Stole`] sense by [`sense_stole`] for [`FleeAfterStealing`] to find.
/// Never taken off again: a monkey with the way out in sight keeps
/// fleeing it until it is through or killed, there being no "it got away
/// clean" to undo this by.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Stole;

/// Tells the mind holding the turn that it has [`Stole`]n, if it has.
///
/// In `PerceiveSet::Annotate`, beside every other sense a game pushes.
pub fn sense_stole(mut thinking: ResMut<Thinking>, stolen: Query<(), With<Stole>>) {
    let Some(thinker) = thinking.actor() else { return };
    if stolen.contains(thinker)
        && let Some(snapshot) = thinking.snapshot_mut()
    {
        snapshot.add_sense(Stole);
    }
}

/// Runs for the floor's own way out, or away from the nearest enemy with
/// none in sight, for a mind that has [`Stole`]n: a monkey's `stealing`.
///
/// Outranks every other tactic in the brain that carries it, `stealing`'s
/// own, above even `FleeWhenHurt`: the job is done, and nothing is worth
/// trading a blow for now. `stealing` grants `Wits::TRAVELS` itself, so
/// the `Ways` sense a mind without it never gets is always there to flee
/// by once something has actually been taken.
#[derive(Debug, Clone, Copy, Default)]
pub struct FleeAfterStealing;

impl Tactic<Entity> for FleeAfterStealing {
    fn name(&self) -> &'static str {
        "flee_after_stealing"
    }

    fn evaluate(&self, ctx: &mut TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
        ctx.snapshot.sense::<Stole>()?;
        let me = ctx.snapshot.me.pos;
        if let Some(ways) = ctx.snapshot.sense::<Ways>() {
            let nearest = ways.0.iter().copied().min_by_key(|p| geometry::chebyshev(me, *p));
            if let Some(way) = nearest {
                if way == me {
                    return Some(Decision::GoThrough);
                }
                if let Some(step) = ctx.step_toward(&[way]) {
                    return Some(Decision::Step(step));
                }
            }
        }
        let foes: Vec<Point> = ctx.snapshot.enemies.iter().map(|e| e.pos).collect();
        ctx.step_away_from(&foes).map(Decision::Step)
    }
}

/// Takes a random thing its target carries but does not wear, then marks
/// the taker [`Stole`] so its brain flees with it: a monkey's `stealing`.
/// Takes no arguments.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Steal;

impl Effect for Steal {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let (thief, targets, landing) = (landing.user, landing.targets.clone(), landing.clone());
        world.commands.queue(move |world: &mut World| {
            let took = targets.iter().any(|victim| steal(world, thief, *victim));
            if !took {
                crate::effects::showed_nothing(world, &landing);
            }
        });
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        "takes something it is not wearing, then runs".to_string()
    }
}

impl FromArgs for Steal {
    const KIND: &'static str = "Steal";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        crate::effects::moving::no_args(args, Self::KIND).map(|_| Self)
    }
}

/// `thief` takes a random thing `victim` carries but does not wear, as
/// [`Steal`] says, marks `thief` [`Stole`] and says whether it took
/// anything: nothing from an empty pack, nor from what `victim` wears.
pub fn steal(world: &mut World, thief: Entity, victim: Entity) -> bool {
    if thief == victim {
        return false;
    }
    let Some(carried) = world.get::<Inventory>(victim).map(|bag| bag.items.clone()) else { return false };
    let worn = world.get::<Equipped>(victim);
    let free: Vec<Entity> = carried.into_iter().filter(|item| !worn.is_some_and(|w| w.0.contains(*item))).collect();
    if free.is_empty() {
        return false;
    }
    let pick = world.resource_mut::<EffectRng>().random_range(0..free.len());
    let item = free[pick];
    if let Some(mut bag) = world.get_mut::<Inventory>(victim) {
        bag.items.retain(|i| *i != item);
    }
    match world.get_mut::<Inventory>(thief) {
        Some(mut bag) => bag.items.push(item),
        None => {
            world.entity_mut(thief).insert(Inventory { items: vec![item] });
        }
    }
    world.entity_mut(thief).insert(Stole);
    if let Some(at) = world.get::<Position>(victim).map(|p| p.0)
        && seen_by_player(world, at)
    {
        let name = |e: Entity, world: &World| world.get::<Name>(e).map_or_else(String::new, |n| n.as_str().to_string());
        let (thief_name, victim_name, item_name) = (name(thief, world), name(victim, world), name(item, world));
        let line = format!("{} snatches {}'s {} and bolts!", crate::effects::capital(&thief_name), victim_name, item_name);
        world.write_message(Tell::new(line, Tones::BAD));
    }
    true
}

/// Steps a random way one time in [`Flit::chance_pct`] out of a hundred,
/// letting every other tactic choose the rest of the time: a vampire
/// bat's `flitting`, erratic rather than a beeline to its prey.
///
/// The same roll-then-step shape as [`rl_engine::rl_rules::ai::tactics::Wander`],
/// without its "wait, there is nothing better to do": this lets the next
/// tactic, `Hunt` wherever a brain puts it, decide both when the roll
/// fails and when every step is blocked.
#[derive(Debug, Clone, Copy)]
pub struct Flit {
    /// Chance per turn of a random step rather than the next tactic's own.
    pub chance_pct: u32,
}

impl Tactic<Entity> for Flit {
    fn name(&self) -> &'static str {
        "flit"
    }

    fn evaluate(&self, ctx: &mut TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
        if !ctx.rng.random_ratio(self.chance_pct.min(100), 100) {
            return None;
        }
        let me = ctx.snapshot.me.pos;
        let back = ctx.snapshot.came_from;
        let start = ctx.rng.random_range(0..8);
        let mut only_back = None;
        for i in 0..8 {
            let step: Point = me + Direction::from_index((start + i) % 8).offset();
            if !(ctx.can_step)(step) {
                continue;
            }
            if Some(step) == back {
                only_back = Some(step);
                continue;
            }
            return Some(Decision::Step(step));
        }
        only_back.map(Decision::Step)
    }
}

/// Marks a kind whose steps leave fire behind them: a salamander's
/// `fire-trail`.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct FireTrail;

/// How many turns a fire-trail's own step burns: short, so it marks the
/// kind's own path rather than setting the whole floor alight behind it.
const FIRE_TRAIL_TURNS: u8 = 3;

/// Ignites the cell a fire-trailing kind just stepped off, `Stepped`'s
/// `from` rather than `to`: it stands on `to` now, and need not catch
/// fire under its own feet.
///
/// In `TurnSet::React`, after the engine has reported every step of the
/// pass.
pub fn trail_fire(mut stepped: MessageReader<Stepped>, trailing: Query<(), With<FireTrail>>, mut kindle: MessageWriter<Kindle>) {
    for step in stepped.read().filter(|s| trailing.contains(s.actor)) {
        kindle.write(Kindle { at: step.from, turns: FIRE_TRAIL_TURNS });
    }
}

/// Marks a kind that is unseen while it stands in the dark: a wraith's
/// `shrouded`.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Shrouds;

/// Keeps the engine's own `shrouded` status, a potion of darkness's, on
/// every [`Shrouds`] kind while its cell is dark, and takes it off the
/// moment it is not: a wraith standing in an unlit cell is exactly as
/// hard to notice as a drinker of that potion, since both are read by the
/// same stealth stat.
///
/// Edge-triggered, as `keep_fireproof` is: `Afflict` only the turn a
/// `Shrouds` kind enters the dark, `Cure` only the turn it leaves it.
///
/// In `TurnSet::React`.
pub fn keep_shrouded(
    registries: Res<Registries>,
    lighting: Res<Lighting>,
    bodies: Query<(Entity, &Position, &Afflicted), With<Shrouds>>,
    mut afflict: MessageWriter<Afflict>,
    mut cure: MessageWriter<Cure>,
) {
    let Some(status) = registries.statuses.id(crate::content::SHROUDED) else { return };
    for (e, pos, held) in &bodies {
        let dark = !lighting.is_lit(pos.0);
        let has = held.0.has(status);
        if dark && !has {
            afflict.write(Afflict { target: e, status, turns: crate::bestiary::FOR_LIFE, by: None, held_by: None });
        } else if !dark && has {
            cure.write(Cure { target: e, status });
        }
    }
}

/// Marks a kind whose regeneration stops while it burns: a troll's
/// `burns-badly`, the second half of its trait.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct BurnsBadly;

/// Marks a [`BurnsBadly`] body whose mending is halted this moment, so it
/// is lifted exactly once when the burning ends.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct RegenHalted;

/// The tag [`keep_regen_halted`]'s own modifier sits under, so it removes
/// exactly the one it put on the mending stat and nothing else there.
const REGEN_HALT_TAG: u64 = u64::from_be_bytes(*b"burnbad1");

/// Zeroes a [`BurnsBadly`] body's mending while it holds `scorched`, and
/// lifts the zero the moment the burning ends: regeneration, and nothing
/// else that mends it, heals it while it burns, as the trait's words say.
///
/// A blunt tool beside the trait's one line: every heal is scaled by the
/// mending stat, so this halts a mend ally's hand along with a troll's
/// own regeneration while it burns, not only the regeneration by name.
/// Judged the simpler reading over threading a second,
/// regeneration-only scale through every heal in the engine for one
/// trait's one kind; no row of this bestiary both regenerates and is
/// mended by another's ability, so nothing today tells the two apart.
///
/// In `TurnSet::React`.
pub fn keep_regen_halted(
    registries: Res<Registries>,
    mut bodies: Query<(Entity, &Afflicted, &mut StatBlock, Has<RegenHalted>), With<BurnsBadly>>,
    mut commands: Commands,
) {
    let Some(scorched) = registries.statuses.id("scorched") else { return };
    let Some(mending) = registries.stats.id(crate::content::MENDING) else { return };
    for (e, held, mut stats, halted) in &mut bodies {
        let burning = held.0.has(scorched);
        if burning && !halted {
            stats.0.add(Modifier::new(mending, Op::Add(-100), Source::Game(REGEN_HALT_TAG)));
            commands.entity(e).insert(RegenHalted);
        } else if !burning && halted {
            stats.0.remove_source(Source::Game(REGEN_HALT_TAG));
            commands.entity(e).remove::<RegenHalted>();
        }
    }
}

/// Caps `weakened`'s own stacking at [`WEAKENED_MAX_STACKS`]: a wraith's
/// `withering`. `Inflict` has nowhere to say "no more than this many
/// instances of me at once", since the engine's `Stacking::Stack` has no
/// maximum of its own; this is that maximum, for one status. Written
/// `(kind: "Wither", args: (turns: 20))`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wither {
    /// How long each stack lasts.
    pub turns: u32,
}

impl Effect for Wither {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let (by, turns, targets) = (Some(landing.user), self.turns, landing.targets.clone());
        world.commands.queue(move |world: &mut World| {
            for target in targets {
                wither(world, target, by, turns);
            }
        });
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        format!("saps strength for {} turns, up to three stacks", self.turns)
    }
}

impl FromArgs for Wither {
    const KIND: &'static str = "Wither";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Args {
            turns: u32,
        }
        let a: Args = read_args(args)?;
        Ok(Self { turns: a.turns })
    }
}

/// How many stacks of `weakened` [`Wither`] lets pile onto one target: the
/// content sheet's own cap.
pub const WEAKENED_MAX_STACKS: usize = 3;

/// Inflicts `weakened` on `target` for `turns`, credited to `by`, unless
/// it already holds [`WEAKENED_MAX_STACKS`] instances: a fourth touch
/// before the first's clock runs out is refused rather than let the stat
/// climb without bound.
pub fn wither(world: &mut World, target: Entity, by: Option<Entity>, turns: u32) {
    let Some(weakened) = world.resource::<Registries>().statuses.id(crate::content::WEAKENED) else { return };
    let stacks = world.get::<Afflicted>(target).map_or(0, |a| a.0.iter().filter(|s| s.id == weakened).count());
    if stacks >= WEAKENED_MAX_STACKS {
        return;
    }
    world.write_message(Afflict { target, status: weakened, turns, by, held_by: None });
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rl_engine::rl_bevy::status::resolve_afflictions;
    use rl_engine::rl_core::Rect;

    use super::*;

    /// A `Shrouds` body standing in the pitch dark of [`Lighting::dark`]
    /// is given the engine's own `shrouded` status, the same one a
    /// potion of darkness gives: `keep_shrouded`, with `resolve_afflictions`
    /// chained after it to carry out the `Afflict` it writes.
    #[test]
    fn a_shrouds_kind_standing_in_the_dark_is_shrouded() {
        let mut app = App::new();
        app.add_message::<Afflict>().add_message::<Cure>().add_message::<rl_engine::rl_bevy::status::StatusEvent>();
        app.insert_resource(crate::content::registries());
        app.insert_resource(Lighting::dark());
        app.add_systems(Update, (keep_shrouded, resolve_afflictions).chain());
        let e = app.world_mut().spawn((Position(Point::ZERO), Shrouds, Afflicted(Statuses::new()), StatBlock(Stats::new()))).id();
        app.update();
        let status = app.world().resource::<Registries>().statuses.expect(crate::content::SHROUDED);
        assert!(app.world().get::<Afflicted>(e).unwrap().0.has(status), "pitch dark, so shrouded");
    }

    /// A `BurnsBadly` body's mending is zeroed while `resolve_afflictions`
    /// holds `scorched` on it, and lifted the moment it is cured.
    #[test]
    fn a_burns_badly_kinds_mending_is_halted_while_it_burns_and_lifted_once_it_is_not() {
        let mut app = App::new();
        app.add_message::<Afflict>().add_message::<Cure>().add_message::<rl_engine::rl_bevy::status::StatusEvent>();
        let registries = crate::content::registries();
        let (scorched, mending) = (registries.statuses.expect("scorched"), registries.stats.expect(crate::content::MENDING));
        app.insert_resource(registries);
        app.add_systems(Update, (resolve_afflictions, keep_regen_halted).chain());
        let e = app.world_mut().spawn((BurnsBadly, Afflicted(Statuses::new()), StatBlock(Stats::new()))).id();
        // The same formula `Loadout::mending` reads the stat by: a
        // hundred and the stat, never below nothing.
        let mending_of = |app: &App| (100 + app.world().get::<StatBlock>(e).unwrap().0.value(mending, &app.world().resource::<Registries>().stats)).max(0);
        app.update();
        assert_eq!(mending_of(&app), 100, "not burning yet");
        app.world_mut().write_message(Afflict { target: e, status: scorched, turns: 3, by: None, held_by: None });
        app.update();
        assert_eq!(mending_of(&app), 0, "burning, so no mending reaches it");
        app.world_mut().write_message(Cure { target: e, status: scorched });
        app.update();
        assert_eq!(mending_of(&app), 100, "the burning is over, and so is the halt");
    }

    /// `Wither` refuses a fourth and a fifth stack of `weakened` rather
    /// than let it climb without bound.
    #[test]
    fn wither_caps_weakened_at_three_stacks() {
        let mut app = App::new();
        app.add_message::<Afflict>().add_message::<Cure>().add_message::<rl_engine::rl_bevy::status::StatusEvent>();
        app.insert_resource(crate::content::registries());
        app.add_systems(Update, resolve_afflictions);
        let e = app.world_mut().spawn((Afflicted(Statuses::new()), StatBlock(Stats::new()))).id();
        for _ in 0..5 {
            wither(app.world_mut(), e, None, 20);
            app.update();
        }
        let weakened = app.world().resource::<Registries>().statuses.expect(crate::content::WEAKENED);
        let stacks = app.world().get::<Afflicted>(e).unwrap().0.iter().filter(|s| s.id == weakened).count();
        assert_eq!(stacks, WEAKENED_MAX_STACKS, "a fourth and a fifth touch are both refused");
    }

    /// `Flit`'s roll decides a random step about one turn in three, and
    /// leaves every other turn to `None`, for the tactic after it to
    /// decide, over a span of seeds.
    #[test]
    fn flit_steps_randomly_about_one_turn_in_three_over_a_span_of_seeds() {
        let me = ActorView { id: Entity::PLACEHOLDER, pos: Point::ZERO, health: None, faction: None };
        let snapshot = Snapshot::alone(me);
        let mut rolled = 0;
        for seed in 0..200u64 {
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            let ctx = &mut TacticCtx {
                snapshot: &snapshot,
                fields: &mut NoFields,
                can_step: &|_| true,
                blocks_shot: &|_| false,
                blocks_burst: &|_| false,
                bounds: Rect::new(-20, -20, 40, 40),
                rng: &mut rng,
            };
            if (Flit { chance_pct: 33 }).evaluate(ctx).is_some() {
                rolled += 1;
            }
        }
        assert!((40..100).contains(&rolled), "about a third of two hundred rolls stepped randomly: {rolled}");
    }
}
