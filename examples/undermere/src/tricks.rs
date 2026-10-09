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
//!
//! Task 9's own: [`Spawns`] and [`Wells`] are counted off the engine's own
//! `ActionDone`, its own turns rather than the floor's clock, so a slow
//! spawner's trick comes no faster for standing beside a swift one. The
//! Mother Below's `spawning` opens a wall cell in her own sight into the
//! floor tile she herself stands on, so this reads no tile name of
//! `dungeon.rs`'s own; the Drowned King's `welling` is `call_drowned`
//! again, read as a forced turn rather than a chosen ability, over the
//! one water his hall holds, a black pool.

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_bevy::EffectRng;
use rl_engine::rl_rules::ability::{RawValue, read_args};
use rl_engine::rl_rules::ai::brain::Tactic;
use rl_engine::rl_rules::ai::tactics::Ways;

use crate::bestiary::{Bestiary, Kind, spawn_monster_now};
use crate::bestiary_lore::MonsterLore;
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
    /// `singing`: a choir plays while allies within [`SING_RANGE`] stand
    /// near it, the Choirmaster's own.
    pub sings: bool,
    /// `spawning`: a spawnling appears at a wall cell in its sight every
    /// [`SPAWN_INTERVAL`]th of its own turns, the Mother Below's own.
    pub spawns: bool,
    /// `welling`: a drowned thrall rises from the water within reach
    /// every [`RISE_INTERVAL`]th of its own turns, the Drowned King's own.
    pub wells: bool,
    /// `sleepy`: twice as likely to start a floor asleep as a plain kind.
    pub sleepy: bool,
    /// `wakeful`: never starts a floor asleep.
    pub wakeful: bool,
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
            sings: has("singing"),
            spawns: has("spawning"),
            wells: has("welling"),
            sleepy: has("sleepy"),
            wakeful: has("wakeful"),
        }
    }

    /// Whether any of these needs a marker component at spawn, for a
    /// `TurnSet::React` system to find by: every one but `packs`, `led`
    /// and `flits`, which are brain shapes built once at load and nothing
    /// a running turn has to look up.
    pub fn needs_marker(self) -> bool {
        self.splits || self.shrouds || self.fire_trail || self.reflects || self.burns_badly || self.drains || self.sings || self.spawns || self.wells
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
/// Dark is anything short of [`LightBand::Lit`], a light's own pool: every
/// floor's ambient is bright enough to be seen by, so the floor's shape
/// reads beyond the lantern, and a wraith judged by whether its cell is
/// seen at all would never once be shrouded below Mereholt.
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
        let dark = lighting.band(pos.0) != LightBand::Lit;
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

/// Marks a kind whose choir mends whoever of its own side stands near it:
/// the Choirmaster's `singing`. Marked at spawn off [`Tricks::sings`].
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Choir;

/// How far a [`Choir`]'s own song reaches: the content sheet's own four.
const SING_RANGE: i32 = 4;

/// A body a choir might sing to: where it stands, its side, what it
/// suffers, and whether the song already gave it its regeneration.
type SingingTo = (Entity, &'static Position, &'static Faction, &'static Afflicted, Has<Sung>);

/// Keeps `regenerating` on every actor of a [`Choir`]'s own side within
/// [`SING_RANGE`] of it, and takes it off the turn it falls out of range:
/// a Choirmaster's song is heard exactly as far as it carries. Plain
/// rather than a blunt mending-wide change, since a status already
/// scaled by the mending stat is what "regenerate" already means
/// everywhere else in this bestiary.
///
/// Edge-triggered, as `keep_shrouded` is: `Afflict` only the turn a body
/// comes into range, `Cure` only the turn it leaves it. Only what the song
/// gave is taken back, marked [`Sung`]: a body already regenerating by its
/// own trait or a worn ring keeps it, in range or out.
///
/// In `TurnSet::React`.
pub fn sing_choir(
    mut commands: Commands,
    registries: Res<Registries>,
    choir: Query<(&Position, &Faction), With<Choir>>,
    bodies: Query<SingingTo, With<Actor>>,
    mut afflict: MessageWriter<Afflict>,
    mut cure: MessageWriter<Cure>,
) {
    let Some(status) = registries.statuses.id("regenerating") else { return };
    for (e, pos, side, held, sung) in &bodies {
        let sung_to = choir.iter().any(|(at, theirs)| theirs.0 == side.0 && geometry::chebyshev(at.0, pos.0) <= SING_RANGE);
        if sung_to && !sung && !held.0.has(status) {
            afflict.write(Afflict { target: e, status, turns: crate::bestiary::FOR_LIFE, by: None, held_by: None });
            commands.entity(e).insert(Sung);
        } else if !sung_to && sung {
            cure.write(Cure { target: e, status });
            commands.entity(e).remove::<Sung>();
        }
    }
}

/// Marks a body whose `regenerating` a [`Choir`] gave, so leaving the song
/// takes back only that and never a regeneration of its own.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Sung;

/// The prop the Widow's cave scatters: `assets/props.ron`'s own name.
const EGG_SAC: &str = "egg sac";

/// What an egg sac hatches into, and how many: the content sheet's own
/// two cave spiders, shared by both ways a sac hatches so one number says
/// it once.
const EGG_SAC_HATCH: &str = "cave spider";

/// How many hatch at once.
const EGG_SAC_HATCH_COUNT: u32 = 2;

/// Hatches an egg sac into [`EGG_SAC_HATCH_COUNT`] of [`EGG_SAC_HATCH`]
/// the moment something steps beside it, despawning the sac so the same
/// step cannot hatch it twice: the Widow's "struck or stepped beside",
/// the half a blow does not cover. Read by `PropKind` against the
/// registry each pass rather than a marker component, since nothing hands
/// this game the entity a prefab's own slot spawned when it stood the sac
/// up. A blow that spends the sac's one health hatches it too, through
/// `props.ron`'s own `destroyed` trigger and
/// [`crate::effects::bodies::Hatch`] instead of here, since combat
/// already knows where and when it broke.
///
/// In `TurnSet::React`.
pub fn hatch_beside(
    mut commands: Commands,
    mut steps: MessageReader<Stepped>,
    registries: Res<Registries>,
    map: Res<WorldMap>,
    props: Query<(Entity, &PropKind, &Position, &OnMap), With<Prop>>,
) {
    let Some(sac) = registries.props.id(EGG_SAC) else { return };
    let here = map.current();
    for step in steps.read() {
        for (prop, kind, pos, on) in &props {
            if kind.0 == sac && on.0 == here && geometry::chebyshev(pos.0, step.to) <= 1 {
                commands.entity(prop).despawn();
                let at = pos.0;
                commands.queue(move |world: &mut World| crate::effects::bodies::hatch(world, EGG_SAC_HATCH, EGG_SAC_HATCH_COUNT, at, here));
            }
        }
    }
}

/// How many of a [`Spawns`] kind's own turns pass between each spawnling:
/// the content sheet's own three.
const SPAWN_INTERVAL: u32 = 3;

/// What a [`Spawns`] kind spawns: the content sheet's own spawnling.
const SPAWNLING: &str = "spawnling";

/// Marks a kind that spawns young from the walls near it, a counter
/// toward [`SPAWN_INTERVAL`] of its own turns: the Mother Below's
/// `spawning`. Marked at spawn off [`Tricks::spawns`].
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Spawns {
    /// Its own turns since the last one spawned.
    elapsed: u32,
}

/// Links a spawnling to whoever's own `spawning` made it, so
/// [`die_with_mother`] takes it out of play the moment its maker dies:
/// the Mother Below's "spawnlings die with her".
#[derive(Component, Debug, Clone, Copy)]
pub struct Spawned(Entity);

/// Ticks every [`Spawns`] kind's own counter on its own `ActionDone`, and
/// stands a [`SPAWNLING`] at a wall cell in its sight every
/// [`SPAWN_INTERVAL`]th: the Mother Below's `spawning`, "a spawnling
/// appears at a wall cell in its sight".
///
/// In `TurnSet::React`, after the engine has reported this pass's own
/// `ActionDone`.
pub fn spawn_young(mut done: MessageReader<ActionDone>, mut spawners: Query<&mut Spawns>, mut commands: Commands) {
    for d in done.read() {
        let Ok(mut spawns) = spawners.get_mut(d.actor) else { continue };
        spawns.elapsed += 1;
        if spawns.elapsed < SPAWN_INTERVAL {
            continue;
        }
        spawns.elapsed = 0;
        let spawner = d.actor;
        commands.queue(move |world: &mut World| spawn_one_young(world, spawner));
    }
}

/// `spawner`'s own one spawnling, as [`spawn_young`] says: the nearest by
/// row of every wall cell in its sight, opened to the floor tile
/// `spawner` itself stands on rather than a tile named here, so this
/// reads nothing of `dungeon.rs`'s own and works wherever a future
/// `spawns` kind is put down; stood with a [`Spawned`] link back to
/// `spawner`, so its own death takes the spawnling with it. Nothing with
/// no such cell in `spawner`'s sight.
fn spawn_one_young(world: &mut World, spawner: Entity) {
    let Some(young) = world.resource::<Bestiary>().defs.id(SPAWNLING) else { return };
    let Some(at) = world.get::<Position>(spawner).map(|p| p.0) else { return };
    let map = world.get::<OnMap>(spawner).map_or(MapId::SURFACE, |m| m.0);
    let Some(view) = world.get::<Viewshed>(spawner).cloned() else { return };
    let cell = {
        let worldmap = world.resource::<WorldMap>();
        let tables = worldmap.tables();
        let mut walls: Vec<Point> =
            geometry::square(at, view.range).filter(|p| view.can_see(*p)).filter(|p| worldmap.tile(*p).is_some_and(|t| !tables.walkable[t.index()])).collect();
        walls.sort_by_key(|p| (p.y, p.x));
        walls.into_iter().next()
    };
    let Some(cell) = cell else { return };
    let Some(own_floor) = world.resource::<WorldMap>().tile(at) else { return };
    world.resource_mut::<WorldMap>().set_tile(cell, own_floor);
    let made = spawn_monster_now(world, young, cell, map);
    world.entity_mut(made).insert(Spawned(spawner));
    if seen_by_player(world, at) {
        if let Some(kind) = world.get::<Kind>(spawner).map(|k| k.0) {
            let name = world.resource::<Bestiary>().defs.get(kind).name.clone();
            world.resource_mut::<MonsterLore>().show(&name, "spawning");
        }
        world.write_message(Tell::new("A spawnling tears its way out of the wall!".to_string(), Tones::BAD));
    }
}

/// Takes every [`Spawned`] whose own maker just died out of play with it,
/// by `allies::vanish` rather than a death of its own, since nothing
/// struck it down: the Mother Below's spawnlings, "die with her".
///
/// In `TurnSet::React`, after the engine has written this pass's own
/// `DeathEvent`.
pub fn die_with_mother(mut deaths: MessageReader<DeathEvent>, spawned: Query<(Entity, &Spawned)>, mut commands: Commands) {
    let dead: Vec<Entity> = deaths.read().map(|d| d.entity).collect();
    if dead.is_empty() {
        return;
    }
    for (e, link) in &spawned {
        if dead.contains(&link.0) {
            commands.queue(move |world: &mut World| crate::allies::vanish(world, e));
        }
    }
}

/// How many of a [`Wells`] kind's own turns pass between each thrall: the
/// content sheet's own eight.
const RISE_INTERVAL: u32 = 8;

/// How far from a [`Wells`] kind a cell of water may lie and still rise a
/// thrall: wide enough to cover the one hall a `welling` kind's own
/// water, a black pool, ever stands in.
const RISE_RANGE: i32 = 12;

/// Marks a kind whose own water rises a drowned thrall every
/// [`RISE_INTERVAL`] of its own turns: the Drowned King's `welling`.
/// Marked at spawn off [`Tricks::wells`].
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Wells {
    /// Its own turns since the last one rose.
    elapsed: u32,
}

/// Ticks every [`Wells`] kind's own counter on its own `ActionDone`, and
/// calls [`crate::effects::bodies::call_drowned`] for one thrall every
/// [`RISE_INTERVAL`]th: the Drowned King's `welling`, forced rather than
/// chosen, unlike the Drowned Abbess's own `calling the drowned` ability.
///
/// In `TurnSet::React`, after the engine has reported this pass's own
/// `ActionDone`.
pub fn rise_from_the_pool(mut done: MessageReader<ActionDone>, mut wells: Query<&mut Wells>, mut commands: Commands) {
    for d in done.read() {
        let Ok(mut w) = wells.get_mut(d.actor) else { continue };
        w.elapsed += 1;
        if w.elapsed < RISE_INTERVAL {
            continue;
        }
        w.elapsed = 0;
        let king = d.actor;
        commands.queue(move |world: &mut World| {
            if crate::effects::bodies::call_drowned(world, king, 1, RISE_RANGE) > 0
                && let Some(kind) = world.get::<Kind>(king).map(|k| k.0)
            {
                let name = world.resource::<Bestiary>().defs.get(kind).name.clone();
                world.resource_mut::<MonsterLore>().show(&name, "welling");
            }
        });
    }
}

/// A loop of cells a monster walks for task 10's own: not a trait's trick,
/// since nothing in the bestiary names it, but a floor's population may
/// put it on anyone it stands, the same way it decides who sleeps.
///
/// The engine keeps no list of a built floor's rooms once it is built, so
/// `population::place_patrol` draws its own waypoints off the floor's open
/// ground rather than true room centres; the effect for a watcher is the
/// same, a mind that goes room to room rather than standing or wandering.
#[derive(Component, Debug, Clone)]
pub struct PatrolRoute {
    /// At least two cells, each visited in turn before the loop starts
    /// over.
    pub waypoints: Vec<Point>,
    /// Which of `waypoints` is next.
    pub at: usize,
}

/// Where a patrolling mind is walking to next, pushed by [`sense_patrols`]
/// for [`Patrol`] to read: the same shape as the engine's own `Posted`,
/// one point rather than a post that never moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Patrolling(pub Point);

/// Advances the turn holder's own [`PatrolRoute`] past whichever waypoint
/// it just reached, and senses the one it is walking to now.
///
/// In `PerceiveSet::Annotate`, beside the engine's own `sense_posts` and
/// `sense_leaders`: nothing for an actor with no route, so one brain
/// serves a floor's own patrol and everybody else alike, the same reason
/// `KeepPost` passes over a monster with no post.
pub fn sense_patrols(mut thinking: ResMut<Thinking>, mut routes: Query<(&Position, &mut PatrolRoute)>) {
    let Some(actor) = thinking.actor() else { return };
    let Ok((at, mut route)) = routes.get_mut(actor) else { return };
    if route.waypoints.is_empty() {
        return;
    }
    if at.0 == route.waypoints[route.at] {
        route.at = (route.at + 1) % route.waypoints.len();
    }
    let target = route.waypoints[route.at];
    if let Some(snapshot) = thinking.snapshot_mut() {
        snapshot.add_sense(Patrolling(target));
    }
}

/// Walks toward whatever [`sense_patrols`] says a patrolling mind is
/// walking to, waiting the one pass it stands there before the next
/// waypoint is sensed. Leaves the turn to the next tactic for anyone with
/// no [`Patrolling`] sense, the same shape as the engine's own `KeepPost`,
/// so one brain serves a kind whether or not a floor put a route on it.
#[derive(Debug, Clone, Copy, Default)]
pub struct Patrol;

impl Tactic<Entity> for Patrol {
    fn name(&self) -> &'static str {
        "patrol"
    }

    fn evaluate(&self, ctx: &mut TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
        let target = ctx.snapshot.sense::<Patrolling>()?.0;
        let me = ctx.snapshot.me.pos;
        if me == target {
            return Some(Decision::Wait);
        }
        Some(ctx.step_toward(&[target]).map_or(Decision::Wait, Decision::Step))
    }
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

    /// Both ways, played on a real floor: a wraith stood in the catacombs
    /// beyond every light, where the floor's own ambient lets it be seen
    /// but nothing lights it, is shrouded, and stood beside the adventurer,
    /// in their lantern's light, is shrouded no longer.
    #[test]
    fn a_wraith_is_shrouded_in_the_dark_and_loses_it_the_turn_it_stands_in_the_light() {
        use crate::testing::{arena_on, pass_turns, stand_on, still};
        let (mut app, _, here) = arena_on(7, 7);
        let status = app.world().resource::<Registries>().statuses.expect(crate::content::SHROUDED);
        let dark = {
            let (map, lighting) = (app.world().resource::<WorldMap>(), app.world().resource::<Lighting>());
            geometry::square(here, 40).find(|p| map.is_walkable(*p) && lighting.band(*p) != LightBand::Lit && geometry::chebyshev(*p, here) > 10)
        };
        let dark = dark.expect("somewhere on the floor is dark");
        let wraith = still(&mut app, "wraith", dark);
        pass_turns(&mut app, 1);
        let shrouded = |app: &App| app.world().get::<Afflicted>(wraith).is_some_and(|a| a.0.has(status));
        assert!(shrouded(&app), "in the dark at {dark:?}, shrouded");
        let beside = here.offset(1, 0);
        assert_eq!(app.world().resource::<Lighting>().band(beside), LightBand::Lit, "the lantern lights the cell beside the adventurer");
        stand_on(&mut app, wraith, beside);
        pass_turns(&mut app, 1);
        assert!(!shrouded(&app), "in the lantern's light beside the adventurer, shrouded no longer");
    }

    /// The Choirmaster's `singing`: an ally of its own side within
    /// `SING_RANGE` regenerates, and loses it the pass it steps past
    /// that range, as `keep_shrouded`'s own toggle does.
    #[test]
    fn a_choirs_own_side_within_its_range_regenerates_and_loses_it_once_out_of_range() {
        let mut app = App::new();
        app.add_message::<Afflict>().add_message::<Cure>().add_message::<rl_engine::rl_bevy::status::StatusEvent>();
        let registries = crate::content::registries();
        let undead = registries.factions.expect("undead");
        app.insert_resource(registries);
        app.add_systems(Update, (sing_choir, resolve_afflictions).chain());
        app.world_mut().spawn((Position(Point::ZERO), Faction(undead), Choir));
        let near = app.world_mut().spawn((Actor, Position(Point::new(3, 0)), Faction(undead), Afflicted(Statuses::new()), StatBlock(Stats::new()))).id();
        let far = app.world_mut().spawn((Actor, Position(Point::new(10, 0)), Faction(undead), Afflicted(Statuses::new()), StatBlock(Stats::new()))).id();
        app.update();
        let regenerating = app.world().resource::<Registries>().statuses.expect("regenerating");
        assert!(app.world().get::<Afflicted>(near).unwrap().0.has(regenerating), "within the choir's range, it regenerates");
        assert!(!app.world().get::<Afflicted>(far).unwrap().0.has(regenerating), "past it, nothing");
        app.world_mut().entity_mut(near).insert(Position(Point::new(10, 0)));
        app.update();
        assert!(!app.world().get::<Afflicted>(near).unwrap().0.has(regenerating), "stepped past the range, the choir's own gift is lifted");
    }

    /// The Widow's egg sac, stepped beside: two cave spiders hatch and
    /// the sac itself is gone, so the same step cannot hatch it twice.
    #[test]
    fn an_egg_sac_hatches_two_cave_spiders_when_stepped_beside_and_then_is_gone() {
        use crate::bestiary::Bestiary;
        use crate::testing::{arena, pass_turns};
        let (mut app, me, here) = arena(31);
        let sac_id = app.world().resource::<Registries>().props.expect(EGG_SAC);
        let map = app.world().resource::<WorldMap>().current();
        let at = here.offset(3, 0);
        let sac = app.world_mut().spawn((Prop, PropKind(sac_id), Position(at), OnMap(map))).id();
        app.world_mut().write_message(Stepped { actor: me, from: here, to: at.offset(-1, 0) });
        pass_turns(&mut app, 1);
        assert!(app.world().get::<Prop>(sac).is_none(), "the sac is gone, hatched");
        let spider_id = app.world().resource::<Bestiary>().defs.expect(EGG_SAC_HATCH);
        let world = app.world_mut();
        let spiders = world.query::<&Kind>().iter(world).filter(|k| k.0 == spider_id).count();
        assert_eq!(spiders, EGG_SAC_HATCH_COUNT as usize, "two cave spiders hatched");
    }

    /// The same sac, struck rather than stepped beside: its one health
    /// spent lands `props.ron`'s own `destroyed` trigger, `Hatch`, the
    /// other half of "struck or stepped beside".
    #[test]
    fn an_egg_sac_also_hatches_when_struck() {
        use crate::bestiary::Bestiary;
        use crate::testing::{arena, kill};
        let (mut app, _, here) = arena(32);
        let sac_id = app.world().resource::<Registries>().props.expect(EGG_SAC);
        let map = app.world().resource::<WorldMap>().current();
        let at = here.offset(2, 0);
        let sac = app.world_mut().spawn((Prop, PropKind(sac_id), Position(at), OnMap(map), Health::full(1))).id();
        // A pass so `arm_props` gives the freshly spawned sac its own
        // `Triggers`, built from `props.ron`'s `destroyed` the moment
        // the app was built; only then does breaking it land `Hatch`.
        app.update();
        kill(&mut app, sac, None);
        let spider_id = app.world().resource::<Bestiary>().defs.expect(EGG_SAC_HATCH);
        let world = app.world_mut();
        let spiders = world.query::<&Kind>().iter(world).filter(|k| k.0 == spider_id).count();
        assert_eq!(spiders, EGG_SAC_HATCH_COUNT as usize, "two cave spiders hatched when the sac was struck down");
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

    /// The Mother Below's `spawning`: left alone near a wall, she stands a
    /// spawnling at it given enough of her own turns, as the content
    /// sheet's "every 3 of its turns while alive" says.
    #[test]
    fn a_spawns_kind_stands_a_spawnling_at_a_wall_cell_in_its_sight_given_enough_of_its_own_turns() {
        use crate::bestiary::Bestiary;
        use crate::testing::{arena, monster, pass_turns, sturdy, wall_off};
        let (mut app, me, here) = arena(60);
        sturdy(&mut app, me);
        monster(&mut app, "the Mother Below", here.offset(4, 0));
        wall_off(&mut app, &[here.offset(4, 2)]);
        pass_turns(&mut app, 40);
        let spawnling = app.world().resource::<Bestiary>().defs.expect(SPAWNLING);
        let world = app.world_mut();
        let count = world.query::<&Kind>().iter(world).filter(|k| k.0 == spawnling).count();
        assert!(count >= 1, "no spawnling stood after forty of the adventurer's own turns");
    }

    /// The Drowned King's `welling`: left alone near a black pool, he
    /// wells a drowned thrall up out of it given enough of his own turns,
    /// as the content sheet's "every 8 turns" says.
    #[test]
    fn a_wells_kind_wells_a_drowned_thrall_up_from_water_in_reach_given_enough_of_its_own_turns() {
        use crate::bestiary::Bestiary;
        use crate::testing::{arena, monster, pass_turns, sturdy};
        let (mut app, me, here) = arena(61);
        sturdy(&mut app, me);
        monster(&mut app, "the Drowned King", here.offset(4, 0));
        let pool_at = here.offset(4, 2);
        {
            let mut map = app.world_mut().resource_mut::<WorldMap>();
            let pool = TileId(map.tables().names.iter().position(|n| n == "black pool").expect("a black pool is a tile") as u16);
            map.set_tile(pool_at, pool);
        }
        pass_turns(&mut app, 70);
        let thrall = app.world().resource::<Bestiary>().defs.expect("drowned thrall");
        let world = app.world_mut();
        let count = world.query::<&Kind>().iter(world).filter(|k| k.0 == thrall).count();
        assert!(count >= 1, "no drowned thrall rose after seventy of the adventurer's own turns");
    }

    /// On a real floor ten, the Mother Below's spawnlings and the Drowned
    /// King's thralls come out of the throne's own geometry: each stands on
    /// open ground inside the floor and off its outermost ring, so a wall
    /// torn open for a spawnling never opens the floor onto nothing. The
    /// adventurer is at peace with everyone, so what the boss does in forty
    /// turns is its own tricks and nothing it was provoked into.
    #[test]
    fn on_a_real_throne_floor_spawnlings_and_thralls_stand_on_open_ground_inside_it() {
        use crate::bestiary::Bestiary;
        use crate::dungeon::{FLOORS, Undermere, map_of};
        use crate::testing::{adventurer, arrive, headless, pass_turns, sturdy};
        use rl_engine::rl_core::RunSeed;
        for (piece, young) in [("the Mother Below", SPAWNLING), ("the throne", "drowned thrall")] {
            let seed = (0..100).map(RunSeed).find(|s| Undermere::new(*s).final_boss().piece == piece).expect("some seed draws each boss");
            let mut app = headless(seed);
            let me = adventurer(&mut app);
            sturdy(&mut app, me);
            let peace = CombatRules::new(&app.world().resource::<Registries>().factions);
            app.insert_resource(peace);
            let floor = map_of(FLOORS);
            arrive(&mut app, floor);
            pass_turns(&mut app, 40);
            let young = app.world().resource::<Bestiary>().defs.expect(young);
            let bounds = app.world().resource::<WorldMap>().place(floor).expect("floor ten is built").terrain.bounds();
            let world = app.world_mut();
            let stood: Vec<Point> =
                world.query::<(&Kind, &Position, &OnMap)>().iter(world).filter(|(k, _, on)| k.0 == young && on.0 == floor).map(|(_, p, _)| p.0).collect();
            assert!(!stood.is_empty(), "{seed:?}: under {piece}, nothing came in forty turns");
            let map = app.world().resource::<WorldMap>();
            for at in stood {
                assert!(
                    bounds.contains(at) && !bounds.is_border(at),
                    "{seed:?}: under {piece}, one stands at {at:?}, on or past the floor's own edge {bounds:?}"
                );
                assert!(map.is_walkable_on(floor, at), "{seed:?}: under {piece}, one stands at {at:?}, which is not open ground");
            }
        }
    }

    /// The Mother Below's own spawnlings stop the moment she dies: killing
    /// her takes every `Spawned` link of hers out of play with her, as
    /// `allies::vanish` does, since nothing struck them down.
    #[test]
    fn the_mother_belows_spawnlings_are_taken_out_of_play_the_moment_she_dies() {
        use crate::testing::{arena, kill, monster, settle};
        let (mut app, _, here) = arena(70);
        let mother = monster(&mut app, "the Mother Below", here.offset(3, 0));
        let young = monster(&mut app, "spawnling", here.offset(3, 1));
        app.world_mut().entity_mut(young).insert(Spawned(mother));
        kill(&mut app, mother, None);
        settle(&mut app);
        assert!(app.world().get_entity(young).is_err(), "the spawnling is gone with her");
    }

    /// Task 10's own patrol: a `PatrolRoute` of three waypoints is walked
    /// in order and the loop starts over, every waypoint visited at least
    /// twice over enough passes, by the universal brain's own `Patrol`
    /// slot, with no trait and no marker beyond the route itself.
    #[test]
    fn a_monster_given_a_patrol_route_walks_every_waypoint_in_order_and_loops_back_to_the_first() {
        use crate::testing::{arena, at, monster, pass_turns, stand_on};
        let (mut app, me, here) = arena(80);
        // Far across the arena's own open ground, outside the rat's sight,
        // so nothing but its own route ever has a say in where it walks.
        stand_on(&mut app, me, here.offset(-5, -5));
        let waypoints = vec![here.offset(5, 5), here.offset(5, 2), here.offset(2, 5)];
        let walker = monster(&mut app, "rat", waypoints[0]);
        app.world_mut().entity_mut(walker).insert(PatrolRoute { waypoints: waypoints.clone(), at: 0 });
        let mut visits = [0u32; 3];
        let mut trace = Vec::new();
        for _ in 0..60 {
            pass_turns(&mut app, 1);
            let now = at(&app, walker);
            trace.push(now);
            if let Some(i) = waypoints.iter().position(|w| *w == now) {
                visits[i] += 1;
            }
        }
        assert!(visits.iter().all(|v| *v >= 2), "every waypoint visited at least twice, looping: {visits:?}, waypoints {waypoints:?}, trace {trace:?}");
    }
}
