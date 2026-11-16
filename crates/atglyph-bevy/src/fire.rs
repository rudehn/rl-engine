//! Fire over the map.
//!
//! How fire catches and spreads is [`atglyph_rules::fire`]; this owns where it
//! burns. Three things feed it. A tile burns when its
//! [`TileProps`](atglyph_grid::TileProps) says how, and leaves the tile it names.
//! Anything else a game wants to burn, a crate, a corpse, a haystack, carries
//! `Flammable`. And a gas that burns catches too, and is used up.
//!
//! Every whole turn the fire steps: what burnt out of the ground is replaced,
//! whoever stands in the flames is reported and given
//! [`FireRules::inflicts`], what catches starts [`Burning`], and the burning
//! cells glow and give off smoke.
//!
//! What burning means to a crate, a corpse or a scroll is the game's. The
//! engine says [`FireEvent::Caught`] and [`FireEvent::BurntOut`], and takes
//! `Flammable` off whatever burnt out, since it has burnt; a game despawns,
//! chars or loots it in answer.

use crate::app::EngineApp;
use std::collections::BTreeMap;

use atglyph_core::seed::position_hash;
use atglyph_core::{DiceRoll, Point};
use atglyph_grid::{Rgb, TileId};
use atglyph_rules::fire::{self, Tinder};
use atglyph_rules::gas::GasId;
use atglyph_rules::status::StatusId;
use bevy::prelude::*;

use crate::components::{Actor, Position};
use crate::fields::{MapFields, SavedField};
use crate::gas::Gases;
use crate::lighting::{LightSource, Lighting};
use crate::places::{MapId, OnMap};
use crate::registries::Registries;
use crate::seed::Seed;
use crate::status::ActiveStatuses;
use crate::status::ApplyStatus;
use crate::turn::{TurnEnd, Turns};
use crate::world::WorldMap;

/// Every burning cell on every map, and how many turns each has left.
#[derive(Resource, Debug, Default)]
pub struct Fire {
    flames: MapFields<u8>,
}

impl Fire {
    /// Whether world cell `p` on the current map is alight.
    pub fn is_burning(&self, p: Point) -> bool {
        self.flames.get(p) > 0
    }

    /// Every burning cell on the current map, in world coordinates.
    pub fn burning(&self) -> impl Iterator<Item = Point> + '_ {
        self.flames.cells().map(|(p, _)| p)
    }

    /// Every burning cell on every map, for saving.
    pub fn export(&self) -> Vec<SavedField<u8>> {
        self.flames.export()
    }

    /// Replaces every fire with what a save held.
    pub fn import(&mut self, saved: impl IntoIterator<Item = SavedField<u8>>) {
        self.flames.import(saved);
    }
}

/// Asks for fire at `at` on the current map, for at least `turns`.
///
/// A cell with nothing to burn still burns that long and goes out, which is
/// a fireball scorching bare stone; one with something to burn catches and
/// burns as that does. A wall that does not burn does not take it.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kindle {
    /// Where.
    pub at: Point,
    /// For at least how many turns.
    pub turns: u8,
}

/// Catches fire from the cell it stands or lies on.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flammable {
    /// Percent chance a turn that its cell catches from each burning
    /// neighbour.
    pub catch_pct: u8,
    /// Turns it burns once alight.
    pub turns: u8,
}

/// Alight: keeps its own cell burning while it lasts.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Burning {
    /// Turns left, or `None` for a fire that never goes out, a brazier's.
    pub turns: Option<u32>,
}

impl Burning {
    /// Alight for `turns` more turns.
    pub fn for_turns(turns: u32) -> Self {
        Self { turns: Some(turns) }
    }

    /// Alight for good.
    pub fn forever() -> Self {
        Self { turns: None }
    }
}

/// What fire does to what is in it, beyond burning it: the game's to say.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FireRules {
    /// A status, for dice of whole turns, on whoever stands in a burning
    /// cell, rolled from the seed, the turn and the cell.
    pub inflicts: Option<(StatusId, DiceRoll)>,
    /// A status whose holder sets alight what can burn where it stands:
    /// the ground if its tile burns, a flammable thing there, a vapour
    /// there that burns. Never bare stone, so a burning creature lights
    /// the door it walks through and not every cell it crosses, and fuel
    /// burning out is what ends it.
    pub kindled_by: Option<StatusId>,
    /// A gas, and how much a turn, that each burning cell gives off.
    pub smoke: Option<(GasId, u16)>,
    /// The light each burning cell sheds, or `None` for a fire that does not
    /// light what is around it.
    pub glow: Option<LightSource>,
}

/// What a burning cell sheds, unless a game says otherwise.
pub(crate) const FIRE_GLOW: LightSource = LightSource::new(180, 3, Rgb::new(255, 130, 50)).flickering(150);

impl FireRules {
    /// Fire that glows and does nothing else to what stands in it.
    pub fn new() -> Self {
        Self { inflicts: None, kindled_by: None, smoke: None, glow: Some(FIRE_GLOW) }
    }

    /// Standing in fire inflicts `status` for `turns`, a number or dice.
    pub fn inflicts(mut self, status: StatusId, turns: impl Into<DiceRoll>) -> Self {
        self.inflicts = Some((status, turns.into()));
        self
    }

    /// Whoever holds `status` sets alight what can burn where it stands.
    pub fn kindled_by(mut self, status: StatusId) -> Self {
        self.kindled_by = Some(status);
        self
    }

    /// Each burning cell gives off `amount` of `gas` a turn.
    pub fn smoke(mut self, gas: GasId, amount: u16) -> Self {
        self.smoke = Some((gas, amount));
        self
    }

    /// Each burning cell sheds `glow`, or nothing.
    pub fn glow(mut self, glow: Option<LightSource>) -> Self {
        self.glow = glow;
        self
    }
}

impl Default for FireRules {
    fn default() -> Self {
        Self::new()
    }
}

/// What fire did.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FireEvent {
    /// An actor spent a whole turn standing in fire.
    Scorched {
        /// Who.
        entity: Entity,
        /// Where.
        at: Point,
    },
    /// Something `Flammable` caught.
    Caught {
        /// What.
        entity: Entity,
    },
    /// Something that was [`Burning`] burnt out, and is no longer flammable.
    BurntOut {
        /// What.
        entity: Entity,
    },
    /// A tile burnt away into the tile it leaves.
    TileBurnt {
        /// Where.
        at: Point,
        /// What it was.
        was: TileId,
        /// What it left.
        now: TileId,
    },
}

/// Sets alight what was asked for this pass.
pub(crate) fn kindle(mut requests: MessageReader<Kindle>, mut fire: ResMut<Fire>, map: Res<WorldMap>) {
    fire.flames.follow(&map);
    for request in requests.read() {
        let takes = map.tile(request.at).is_some_and(|t| map.tables().burns[t.index()].is_some() || !map.tables().blocks_projectiles[t.index()]);
        if takes {
            fire.flames.update(request.at, |now| now.max(request.turns.max(1)));
        }
    }
}

/// Keeps the cell of everything [`Burning`] alight, at the start of every
/// pass, so a brazier spawned this frame is fire before anyone decides where
/// to step, rather than once the first turn has passed.
pub(crate) fn keep_alight(mut fire: ResMut<Fire>, map: Res<WorldMap>, lit: Query<(&Position, &Burning, Option<&OnMap>)>) {
    fire.flames.follow(&map);
    let here = map.current();
    for (pos, burning, on) in &lit {
        if on.map_or(MapId::ZERO, |m| m.0) == here && burning.turns != Some(0) {
            fire.flames.update(pos.0, |t| t.max(2));
        }
    }
}

/// What stepping fire reads and changes besides the fire itself.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Blaze<'w> {
    map: ResMut<'w, WorldMap>,
    rules: Res<'w, FireRules>,
    seed: Res<'w, Seed>,
    clock: Res<'w, Turns>,
    registries: Res<'w, Registries>,
    gases: Option<ResMut<'w, Gases>>,
    lighting: Option<ResMut<'w, Lighting>>,
}

/// Something flammable that has not caught.
type Unlit = (Entity, &'static Position, &'static Flammable, Option<&'static OnMap>);
/// Someone who might stand in the flames, and what it holds.
type Standing = (Entity, &'static Position, Option<&'static OnMap>, Option<&'static ActiveStatuses>);
/// Something alight.
type Lit = (Entity, &'static Position, &'static mut Burning, Option<&'static OnMap>);

/// Everything that might be in the flames.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct InTheFire<'w, 's> {
    commands: Commands<'w, 's>,
    unlit: Query<'w, 's, Unlit, Without<Burning>>,
    lit: Query<'w, 's, Lit>,
    actors: Query<'w, 's, Standing, With<Actor>>,
    events: MessageWriter<'w, FireEvent>,
    afflict: MessageWriter<'w, ApplyStatus>,
}

/// Every whole turn: what is alight keeps its cell burning, the fire spreads
/// and burns down, the ground it used up is replaced, burning vapour is used
/// up, what stands in it catches or is scorched, and the fire smokes and
/// glows.
///
/// The rolls are hashed from the run's seed, the turn and the cell, so the
/// fire spreads the same whichever order it is stepped in and a save holds no
/// generator for it.
pub(crate) fn step_fire(mut ends: MessageReader<TurnEnd>, mut fire: ResMut<Fire>, blaze: Blaze, things: InTheFire) {
    let passed = ends.read().count() as u32;
    if passed == 0 {
        return;
    }
    let Blaze { mut map, rules, seed, clock, registries, mut gases, lighting } = blaze;
    let InTheFire { mut commands, unlit, mut lit, actors, mut events, mut afflict } = things;
    fire.flames.follow(&map);
    if let Some(gases) = gases.as_deref_mut() {
        gases.fit(registries.gases.len(), &map);
    }
    let here = map.current();
    let on_here = |on: Option<&OnMap>| on.map_or(MapId::ZERO, |m| m.0) == here;
    let vapours: Vec<GasId> = registries.gases.iter().filter(|(_, def)| def.burns).map(|(id, _)| id).collect();
    // What caught in an earlier step of this call, whose `Burning` is still
    // queued, so it does not catch again.
    let mut caught: Vec<Entity> = Vec::new();
    for step in 0..passed {
        // What is alight keeps its own cell burning.
        for (_, pos, burning, on) in &lit {
            if on_here(on) && burning.turns != Some(0) {
                fire.flames.update(pos.0, |t| t.max(2));
            }
        }
        let turn = u64::from(clock.turn_number() + 1 - passed + step);
        // Whoever holds the kindling status lights what can burn under it,
        // for as long as that burns.
        if let Some(kindling) = rules.kindled_by {
            for (_, pos, on, held) in &actors {
                // Only what is not alight already: lit once, it burns down as it
                // would, and its burning out is what ends it, where lighting
                // it again each turn would keep its holder burning for good.
                if !on_here(on) || !held.is_some_and(|h| h.has(kindling)) || fire.is_burning(pos.0) {
                    continue;
                }
                let ground = map.tile(pos.0).and_then(|t| map.tables().burns[t.index()]).map(|k| k.turns);
                let thing = unlit.iter().filter(|(_, p, _, at)| p.0 == pos.0 && on_here(*at)).map(|(.., f, _)| f.turns).max();
                let vapour = gases.as_deref().filter(|g| vapours.iter().any(|v| g.at(*v, pos.0) > 0)).map(|_| 1);
                // A turn over the fuel's own: lit before the step, it burns
                // down once in it, and then burns as long as fuel that
                // caught in the step does, a vapour alight long enough to
                // be burnt away.
                if let Some(turns) = [ground, thing, vapour].into_iter().flatten().max() {
                    fire.flames.update(pos.0, |t| t.max(turns.max(1).saturating_add(1)));
                }
            }
        }
        // What spent the turn in the flames, before they burn down: whatever
        // can catch does, and whoever stood there is scorched.
        for (entity, pos, flammable, on) in &unlit {
            if on_here(on) && fire.is_burning(pos.0) && !caught.contains(&entity) {
                caught.push(entity);
                commands.entity(entity).insert(Burning::for_turns(u32::from(flammable.turns.max(1))));
                events.write(FireEvent::Caught { entity });
            }
        }
        for (entity, pos, on, _) in &actors {
            if on_here(on) && fire.is_burning(pos.0) {
                events.write(FireEvent::Scorched { entity, at: pos.0 });
                if let Some((status, dice)) = rules.inflicts {
                    let turns = crate::seed::hashed_roll(Some(&seed), crate::seed::FIRE_INFLICT, turn, pos.0, 0, dice).max(1) as u32;
                    afflict.write(ApplyStatus { target: entity, status, turns, by: None, held_by: None });
                }
            }
        }
        let things: BTreeMap<Point, Tinder> = unlit.iter().filter(|(.., on)| on_here(*on)).fold(BTreeMap::new(), |mut fuel, (_, pos, f, _)| {
            let t = Tinder { catch_pct: f.catch_pct, turns: f.turns };
            fuel.entry(pos.0).and_modify(|have: &mut Tinder| *have = have.or(Some(t))).or_insert(t);
            fuel
        });
        let origin = fire.flames.origin();
        let salt = seed.0.derive(crate::seed::FIRE, turn);
        let burnt = {
            let (map, gases) = (&*map, gases.as_deref());
            let tinder = |local: Point| {
                let p = local + origin;
                let ground = map.tile(p).and_then(|t| map.tables().burns[t.index()]).map(|k| Tinder { catch_pct: k.catch_pct, turns: k.turns });
                let vapour = gases.filter(|g| vapours.iter().any(|v| g.at(*v, p) > 0)).map(|_| Tinder { catch_pct: 100, turns: 1 });
                [ground, things.get(&p).copied(), vapour].into_iter().flatten().reduce(|a, b| a.or(Some(b)))
            };
            let roll = |local: Point| {
                let p = local + origin;
                position_hash(salt, p.x, p.y) as u32
            };
            fire::spread(fire.flames.field_mut(), tinder, roll)
        };

        // The ground it used up leaves what it said it would.
        for local in burnt {
            let p = local + origin;
            let Some(was) = map.tile(p) else { continue };
            if let Some(kindling) = map.tables().burns[was.index()] {
                map.set_tile(p, kindling.leaves);
                events.write(FireEvent::TileBurnt { at: p, was, now: kindling.leaves });
            }
        }
        let alight: Vec<Point> = fire.burning().collect();
        if let Some(gases) = gases.as_deref_mut() {
            // Vapour that caught is burnt away.
            for p in &alight {
                for vapour in &vapours {
                    gases.clear_at(*vapour, *p);
                }
            }
            if let Some((smoke, amount)) = rules.smoke {
                let spill = crate::gas::spill_salt(&seed, turn as u32);
                for p in &alight {
                    gases.release(smoke, *p, amount, &map, spill);
                }
            }
        }

        for (entity, _, mut burning, on) in &mut lit {
            let Some(left) = burning.turns.as_mut() else { continue };
            if !on_here(on) || *left == 0 {
                continue;
            }
            *left -= 1;
            if *left == 0 {
                commands.entity(entity).remove::<(Burning, Flammable)>();
                events.write(FireEvent::BurntOut { entity });
            }
        }
    }
    if let Some(mut lighting) = lighting {
        let glow = rules.glow.map(|glow| fire.burning().map(|p| (p, glow)).collect()).unwrap_or_default();
        lighting.set_glow(here, glow);
    }
}

/// Refuses play when fire gives off a gas with no gas to give off.
fn check_fire_rules(rules: Option<Res<FireRules>>, gases: Option<Res<Gases>>) {
    let Some(rules) = rules else { return };
    assert!(
        rules.smoke.is_none() || gases.is_some(),
        "FirePlugin: FireRules::smoke names a gas, but GasPlugin was not added, so fire would give off nothing; add GasPlugin"
    );
}

/// Marks every burning cell as somewhere the mind holding the turn will
/// not step, in [`PerceiveSet::Annotate`](crate::plugin::PerceiveSet::Annotate).
pub(crate) fn perceive_fire(mut thinking: ResMut<crate::minds::Thinking>, fire: Res<Fire>) {
    if thinking.actor().is_none() {
        return;
    }
    for p in fire.burning() {
        thinking.mark_hazard(p);
    }
}

/// Fire: [`Fire`], set alight by [`Kindle`] and by what is [`Burning`],
/// spread through burning tiles, `Flammable` things and burning gas every
/// whole turn, glowing and smoking as [`FireRules`] says.
///
/// Needs [`FireRules`], [`Registries`] for which gases burn, and the run's
/// [`Seed`] the flames roll from; and, when its rules give off smoke,
/// [`GasPlugin`](crate::gas::GasPlugin), which it checks when play begins.
/// Registers the `Ignite` ability effect.
pub struct FirePlugin;

impl Plugin for FirePlugin {
    fn build(&self, app: &mut App) {
        use crate::app::EngineApp;
        use crate::plugin::{FieldSet, Turn};
        app.init_resource::<Fire>()
            .reset_on_new_run::<Fire>()
            .add_message::<Kindle>()
            .add_message::<FireEvent>()
            .add_message::<ApplyStatus>()
            .needs::<FireRules>("FirePlugin", "`FireRules::new()`, with `.inflicts(status, turns)` for what standing in fire does")
            .needs::<Registries>("FirePlugin", "`Registries`, which says which gases burn")
            .needs::<Seed>("FirePlugin", "`Seed(RunSeed(n))`, which the flames roll from")
            .add_effect::<crate::effects::Ignite>()
            .add_systems(Turn, keep_alight.in_set(crate::plugin::TurnSet::Schedule))
            .add_systems(Turn, perceive_fire.in_set(crate::plugin::PerceiveSet::Annotate))
            .add_systems(Turn, (kindle, step_fire).chain().in_set(FieldSet::Fire))
            .add_systems(OnEnter(crate::state::EngineState::Playing), check_fire_rules);
    }

    fn finish(&self, app: &mut App) {
        app.depends_on::<crate::plugin::CorePlugin>("FirePlugin");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{CombatPlugin, Faction, Health, MeleeAttack};
    use crate::components::{Blocks, Player, RevealsMap, Viewshed};
    use crate::gas::{GasPlugin, Release};
    use crate::lighting::LightingPlugin;
    use crate::state::EngineState;
    use crate::status::ActiveStatuses;
    use crate::testing::headless_app;
    use crate::turn::{Intent, Wait};
    use atglyph_grid::{TileProps, TileRegistry};
    use atglyph_rules::gas::GasDef;
    use atglyph_rules::{Registry, StatusDef};

    /// Every fire event, recorded by a reader.
    #[derive(Resource, Default)]
    struct Seen(Vec<FireEvent>);

    fn see(mut events: MessageReader<FireEvent>, mut seen: ResMut<Seen>) {
        seen.0.extend(events.read().copied());
    }

    struct Rig {
        app: App,
        player: Entity,
        start: Point,
        tiles: TileRegistry,
        burning: StatusId,
        smoke: GasId,
        spirits: GasId,
    }

    impl Rig {
        /// Open ground with grass that burns to ash, a player, fire that burns
        /// whoever stands in it, and, `with_gas`, smoke and a vapour that burns.
        fn new(with_gas: bool) -> Self {
            let mut app = headless_app();
            app.add_plugins((crate::fov::FovPlugin, CombatPlugin, LightingPlugin, FirePlugin));
            if with_gas {
                app.add_plugins(GasPlugin);
            }
            app.init_resource::<Seen>().add_systems(PostUpdate, see);
            let mut tiles = TileRegistry::standard();
            tiles.register(TileProps::floor("ash")).unwrap();
            tiles.register(TileProps::floor("grass").burns(100, 2, "ash")).unwrap();
            let start = crate::testing::ground_with(&mut app, tiles.clone());
            crate::testing::two_sides(&mut app);
            let statuses = Registry::from_defs(vec![StatusDef::new("burning")]).unwrap();
            let burning = statuses.expect("burning");
            let gases = Registry::from_defs(vec![GasDef::new("smoke").spread(40).fade(20), GasDef::new("spirits").spread(0).fade(1).burns()]).unwrap();
            let (smoke, spirits) = (gases.expect("smoke"), gases.expect("spirits"));
            let mut rules = FireRules::new().inflicts(burning, 2);
            {
                let mut registries = app.world_mut().resource_mut::<Registries>();
                registries.statuses = statuses;
                if with_gas {
                    registries.gases = gases;
                    rules = rules.smoke(smoke, 40);
                }
            }
            app.insert_resource(rules);
            let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(10), RevealsMap, Health::full(30))).id();
            app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
            app.update();
            app.update();
            Self { app, player, start, tiles, burning, smoke, spirits }
        }

        fn set(&mut self, at: Point, tile: &str) {
            let id = self.tiles.expect(tile);
            assert!(self.app.world_mut().resource_mut::<WorldMap>().set_tile(at, id));
        }

        fn is(&self, at: Point, tile: &str) -> bool {
            self.app.world().resource::<WorldMap>().tile(at) == Some(self.tiles.expect(tile))
        }

        fn kindle(&mut self, at: Point, turns: u8) {
            self.app.world_mut().write_message(Kindle { at, turns });
        }

        fn wait(&mut self, turns: usize) {
            for _ in 0..turns {
                self.app.world_mut().write_message(Intent::new(self.player, Wait));
                self.app.update();
            }
        }

        fn seen(&self) -> &[FireEvent] {
            &self.app.world().resource::<Seen>().0
        }
    }

    #[test]
    fn fire_runs_through_grass_leaves_ash_and_lights_what_burns() {
        let mut rig = Rig::new(false);
        let strip: Vec<Point> = (2..=8).map(|x| rig.start.offset(x, 0)).collect();
        for p in &strip {
            rig.set(*p, "grass");
        }
        rig.kindle(strip[0], 1);
        rig.wait(1);
        let lit_while_burning = rig.app.world().resource::<Lighting>().at(strip[1]).intensity;
        assert!(rig.app.world().resource::<Fire>().is_burning(strip[1]), "it caught the next blade along");
        assert!(lit_while_burning > 0, "and the flames light the dark");

        rig.wait(20);
        assert!(strip.iter().all(|p| rig.is(*p, "ash")), "the whole strip burnt to ash");
        assert!(rig.is(rig.start.offset(2, 1), "floor"), "and not the bare ground beside it");
        assert_eq!(rig.app.world().resource::<Fire>().burning().count(), 0, "and it went out");
        assert_eq!(rig.app.world().resource::<Lighting>().at(strip[3]).intensity, 0, "and the dark came back");
        let burnt = rig.seen().iter().filter(|e| matches!(e, FireEvent::TileBurnt { .. })).count();
        assert_eq!(burnt, strip.len(), "every blade reported burning away once");
    }

    #[test]
    fn whoever_stands_in_fire_is_scorched_and_given_the_status_the_game_named() {
        let mut rig = Rig::new(false);
        let at = rig.start;
        rig.kindle(at, 3);
        rig.wait(1);
        let (player, burning) = (rig.player, rig.burning);
        assert!(rig.seen().contains(&FireEvent::Scorched { entity: player, at }));
        assert!(rig.app.world().get::<ActiveStatuses>(player).is_some_and(|a| a.has(burning)), "and burns");
    }

    /// Whoever holds the status fire is lit by sets alight what can burn
    /// where it stands, grass here, which burns on to ash.
    #[test]
    fn a_holder_of_the_kindling_status_lights_the_grass_it_stands_on() {
        let mut rig = Rig::new(false);
        let rules = rig.app.world().resource::<FireRules>().kindled_by(rig.burning);
        rig.app.insert_resource(rules);
        let at = rig.start;
        rig.set(at, "grass");
        let (player, burning) = (rig.player, rig.burning);
        rig.app.world_mut().write_message(ApplyStatus { target: player, status: burning, turns: 5, by: None, held_by: None });
        rig.wait(1);
        assert!(rig.app.world().resource::<Fire>().is_burning(at), "the grass under it caught");
        rig.wait(10);
        assert!(rig.is(at, "ash"), "and burnt away");
        assert!(!rig.app.world().get::<ActiveStatuses>(player).unwrap().has(burning), "and with the fuel gone its holder burns no more");
        assert_eq!(rig.app.world().resource::<Fire>().burning().count(), 0, "and nothing is alight");
    }

    /// Whoever holds the kindling status lights a flammable thing it
    /// stands over, and a vapour that burns where it stands.
    #[test]
    fn a_holder_of_the_kindling_status_lights_a_flammable_thing_and_a_vapour_under_it() {
        let mut rig = Rig::new(true);
        let rules = rig.app.world().resource::<FireRules>().kindled_by(rig.burning);
        rig.app.insert_resource(rules);
        let (player, burning, at) = (rig.player, rig.burning, rig.start);
        let crate_ = rig.app.world_mut().spawn((Position(at), Flammable { catch_pct: 100, turns: 2 })).id();
        rig.app.world_mut().write_message(ApplyStatus { target: player, status: burning, turns: 5, by: None, held_by: None });
        rig.wait(1);
        assert!(rig.seen().contains(&FireEvent::Caught { entity: crate_ }), "the crate under it caught");
        rig.wait(12);
        let beside = at.offset(5, 0);
        let spirits = rig.spirits;
        // Fire steps before gas, so the vapour lies there a turn first.
        rig.app.world_mut().write_message(Release { gas: spirits, at: beside, amount: 40 });
        rig.wait(1);
        crate::testing::stand_on(&mut rig.app, player, beside);
        rig.app.world_mut().write_message(ApplyStatus { target: player, status: burning, turns: 5, by: None, held_by: None });
        rig.wait(1);
        let left = rig.app.world().resource::<crate::gas::Gases>().at(spirits, beside);
        assert!(rig.app.world().resource::<Fire>().is_burning(beside) && left == 0, "and the vapour where it stood went up, burnt away: {left} left");
    }

    /// Bare stone has nothing to burn: a holder of the kindling status
    /// standing on it lights nothing, however long it stands there.
    #[test]
    fn a_holder_of_the_kindling_status_never_lights_bare_stone() {
        let mut rig = Rig::new(false);
        let rules = rig.app.world().resource::<FireRules>().kindled_by(rig.burning);
        rig.app.insert_resource(rules);
        let (player, burning) = (rig.player, rig.burning);
        rig.app.world_mut().write_message(ApplyStatus { target: player, status: burning, turns: 50, by: None, held_by: None });
        for _ in 0..5 {
            rig.wait(1);
            assert_eq!(rig.app.world().resource::<Fire>().burning().count(), 0, "nothing alight on bare ground");
        }
    }

    /// The turns fire inflicts may be dice: rolled within them, from the
    /// seed, the turn and the cell, so the same run burns the same.
    #[test]
    fn fire_rules_inflict_dice_turns_the_same_for_the_same_seed() {
        let burnt_for = || {
            let mut rig = Rig::new(false);
            let rules = FireRules { inflicts: Some((rig.burning, DiceRoll { num: 1, sides: 3, bonus: 20 })), ..*rig.app.world().resource::<FireRules>() };
            rig.app.insert_resource(rules);
            let at = rig.start;
            rig.kindle(at, 1);
            rig.wait(1);
            let (player, burning) = (rig.player, rig.burning);
            rig.app.world().get::<ActiveStatuses>(player).unwrap().iter().find(|s| s.id == burning).map(|s| s.turns).expect("burning")
        };
        let turns = burnt_for();
        assert!((20..=23).contains(&turns), "{turns}: within 1d3+20, less what has ticked");
        assert_eq!(turns, burnt_for(), "the same seed, the same burn");
    }

    #[test]
    fn a_flammable_thing_catches_burns_out_and_is_left_for_the_game() {
        let mut rig = Rig::new(false);
        let at = rig.start.offset(3, 0);
        let crate_ = rig.app.world_mut().spawn((Position(at), Flammable { catch_pct: 100, turns: 2 })).id();
        rig.app.update();
        rig.kindle(at, 1);
        rig.wait(1);
        assert!(rig.app.world().get::<Burning>(crate_).is_some(), "it caught");
        assert!(rig.seen().contains(&FireEvent::Caught { entity: crate_ }));

        rig.wait(5);
        let w = rig.app.world();
        assert!(w.get::<Burning>(crate_).is_none() && w.get::<Flammable>(crate_).is_none(), "burnt out, and nothing left in it to burn");
        assert!(rig.seen().contains(&FireEvent::BurntOut { entity: crate_ }));
        assert!(w.get_entity(crate_).is_ok(), "and what is left of it is the game's");
        assert!(!w.resource::<Fire>().is_burning(at), "and its cell has gone out");
    }

    #[test]
    fn fire_catches_in_a_vapour_that_burns_and_gives_off_smoke() {
        let mut rig = Rig::new(true);
        let (smoke, spirits) = (rig.smoke, rig.spirits);
        let line: Vec<Point> = (2..=6).map(|x| rig.start.offset(x, 0)).collect();
        for p in &line {
            rig.app.world_mut().write_message(Release { gas: spirits, at: *p, amount: 100 });
        }
        rig.wait(1);
        rig.kindle(line[0], 1);
        rig.wait(6);
        let gases = rig.app.world().resource::<Gases>();
        assert!(line.iter().all(|p| gases.at(spirits, *p) == 0), "the vapour burnt away along the whole line");
        assert!(gases.cells(smoke).count() > 0, "and smoke hangs where it burned");
    }

    #[test]
    fn a_mind_will_not_step_into_fire() {
        use crate::minds::{Mind, MindsPlugin, Perception};
        use atglyph_rules::Brain;
        use atglyph_rules::ai::tactics::Hunt;
        use std::sync::Arc;
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, MindsPlugin, FirePlugin));
        let start = crate::testing::ground(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        app.insert_resource(FireRules::new());
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(10), Health::full(30), Faction(sides.ours))).id();
        let brazier = start.offset(1, 0);
        app.world_mut().spawn((Position(brazier), Burning::forever()));
        let hunter = app
            .world_mut()
            .spawn((
                Actor,
                Blocks,
                Position(start.offset(2, 0)),
                Health::full(10),
                Faction(sides.theirs),
                Perception(8),
                MeleeAttack::new(sides.kind, atglyph_core::DiceRoll::flat(1)),
                Mind(Arc::new(Brain::new().then(Hunt))),
            ))
            .id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        for _ in 0..6 {
            if app.world().get::<crate::components::MyTurn>(player).is_some() {
                app.world_mut().write_message(Intent::new(player, Wait));
            }
            app.update();
            assert!(app.world().resource::<Fire>().is_burning(brazier), "the brazier burns from the first frame it stands");
            assert_ne!(app.world().get::<Position>(hunter).map(|p| p.0), Some(brazier), "it went round the fire, not through it");
        }
    }
}
