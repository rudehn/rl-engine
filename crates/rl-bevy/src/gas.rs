//! Gas over the map.
//!
//! Which gases there are is [`Registries::gases`]; how each spreads is
//! [`rl_rules::gas`]. This keeps one field per gas per map in [`Gases`],
//! gives off what [`Release`] asks for and what a [`Vents`] entity puts out,
//! steps every gas a whole turn at a time, writes the gas thick enough to hide
//! what is behind it into the map's veil, and reports whoever breathes it.
//!
//! What spills where and which way a cloud swirls are hashed from the run's
//! seed, the turn and the cell, as fire's rolls are, so a cloud spreads the
//! same whichever order it is stepped in and a save holds no generator for it.
//!
//! What a gas does beyond its [`Breath`](rl_rules::gas::Breath) is the game's,
//! answered from [`Breathed`], which is sent every turn an actor stands in some.

use bevy::prelude::*;
use rl_core::seed::position_hash;
use rl_core::{Point, SeedDomain};
use rl_rules::gas::{self, GasId};

use crate::components::{Actor, Position};
use crate::fields::{MapFields, SavedField};
use crate::places::{MapId, OnMap};
use crate::registries::Registries;
use crate::seed::Seed;
use crate::status::Afflict;
use crate::turn::{TurnEnd, Turns};
use crate::world::WorldMap;

/// Every gas on every map.
#[derive(Resource, Debug, Default)]
pub struct Gases {
    /// One per registered gas, by id.
    layers: Vec<MapFields<u8>>,
}

impl Gases {
    /// How much of `gas` is at world cell `p` on the current map.
    pub fn at(&self, gas: GasId, p: Point) -> u8 {
        self.layers.get(gas.index()).map_or(0, |l| l.get(p))
    }

    /// The densest gas at `p` on the current map, and how dense.
    pub fn densest(&self, p: Point) -> Option<(GasId, u8)> {
        self.layers.iter().enumerate().map(|(i, l)| (GasId::from_raw(i as u32), l.get(p))).filter(|(_, c)| *c > 0).max_by_key(|(_, c)| *c)
    }

    /// Every cell on the current map holding some of `gas`, in world
    /// coordinates.
    pub fn cells(&self, gas: GasId) -> impl Iterator<Item = (Point, u8)> + '_ {
        self.layers.get(gas.index()).into_iter().flat_map(|l| l.cells())
    }

    /// Gives off `amount` of `gas` at `p` on the current map at once: what
    /// the cell has room for stays, and the rest spills to the nearest cells
    /// `map` lets gas into, as [`gas::release`] says. `salt` is what the
    /// ragged edge is hashed from; [`spill_salt`] is the turn's.
    pub fn release(&mut self, gas: GasId, p: Point, amount: u16, map: &WorldMap, salt: u64) {
        let Some(layer) = self.layers.get_mut(gas.index()) else { return };
        let origin = layer.origin();
        gas::release(layer.field_mut(), p - origin, amount, |q| !map.blocks_projectiles(q + origin), |q| roll(salt, q + origin));
    }

    /// Takes every bit of `gas` out of `p`.
    pub fn clear_at(&mut self, gas: GasId, p: Point) {
        if let Some(layer) = self.layers.get_mut(gas.index()) {
            layer.update(p, |_| 0);
        }
    }

    /// Makes a field for each of `kinds` gases and fits them all to the map
    /// readers read.
    pub fn fit(&mut self, kinds: usize, map: &WorldMap) {
        if self.layers.len() < kinds {
            self.layers.resize_with(kinds, MapFields::default);
        }
        for layer in &mut self.layers {
            layer.follow(map);
        }
    }

    /// Every gas on every map, for saving.
    pub fn export(&self) -> Vec<(GasId, Vec<SavedField<u8>>)> {
        self.layers.iter().enumerate().map(|(i, l)| (GasId::from_raw(i as u32), l.export())).filter(|(_, maps)| !maps.is_empty()).collect()
    }

    /// Replaces every gas with what a save held.
    pub fn import(&mut self, saved: impl IntoIterator<Item = (GasId, Vec<SavedField<u8>>)>) {
        self.layers.clear();
        for (gas, maps) in saved {
            if self.layers.len() <= gas.index() {
                self.layers.resize_with(gas.index() + 1, MapFields::default);
            }
            self.layers[gas.index()].import(maps);
        }
    }
}

/// Asks for `amount` more of `gas` at `at` on the current map.
///
/// Given off as soon as fields are stepped in the pass it is sent in, so a
/// cloud an ability releases hangs there before anyone else acts.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Release {
    /// Which gas.
    pub gas: GasId,
    /// Where.
    pub at: Point,
    /// How much, a full cell being [`gas::FULL`]: what the cell has no room
    /// for spills to the nearest cells, so a grenade's worth is a cloud.
    pub amount: u16,
}

/// Gives off `amount` of `gas` wherever this entity is, every whole turn: a
/// vent, a pool, a censer.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vents {
    /// Which gas.
    pub gas: GasId,
    /// How much a turn, a full cell being [`gas::FULL`].
    pub amount: u16,
}

/// An actor spent a whole turn standing in gas.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Breathed {
    /// Who.
    pub actor: Entity,
    /// What.
    pub gas: GasId,
    /// How thick.
    pub amount: u8,
}

/// What a spill on `turn` is hashed from: which of two nearly as near cells
/// it fills first.
pub fn spill_salt(seed: &Seed, turn: u32) -> u64 {
    seed.0.derive(SeedDomain::new(b"gas spill"), u64::from(turn))
}

/// What the swirls of `turn` are hashed from.
fn swirl_salt(seed: &Seed, turn: u32) -> u64 {
    seed.0.derive(SeedDomain::new(b"gas swirl"), u64::from(turn))
}

/// A cell's roll under `salt`, by its world coordinates, so a cloud on the
/// streamed surface swirls alike wherever the window stands.
fn roll(salt: u64, p: Point) -> u32 {
    position_hash(salt, p.x, p.y) as u32
}

/// Gives off what was asked for this pass.
pub fn release_gas(
    mut requests: MessageReader<Release>,
    mut gases: ResMut<Gases>,
    registries: Res<Registries>,
    map: Res<WorldMap>,
    seed: Res<Seed>,
    clock: Res<Turns>,
) {
    gases.fit(registries.gases.len(), &map);
    let salt = spill_salt(&seed, clock.turn_number());
    for request in requests.read() {
        gases.release(request.gas, request.at, request.amount, &map, salt);
    }
}

/// What stepping gas reads and writes besides the gases themselves.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Air<'w, 's> {
    registries: Res<'w, Registries>,
    map: ResMut<'w, WorldMap>,
    seed: Res<'w, Seed>,
    clock: Res<'w, Turns>,
    vents: Query<'w, 's, (&'static Position, &'static Vents, Option<&'static OnMap>)>,
    breathers: Query<'w, 's, (Entity, &'static Position, Option<&'static OnMap>), With<Actor>>,
    afflict: MessageWriter<'w, Afflict>,
    breathed: MessageWriter<'w, Breathed>,
}

/// Every whole turn: what vents gives off, every gas spread and faded over
/// what holds gas, the veil redrawn, and whoever stands in gas breathing it.
///
/// A tile that stops a thrown thing stops gas too, so walls and closed doors
/// hold a cloud back and open water does not.
pub fn step_gases(mut ends: MessageReader<TurnEnd>, mut gases: ResMut<Gases>, air: Air) {
    let passed = ends.read().count() as u32;
    if passed == 0 {
        return;
    }
    let Air { registries, mut map, seed, clock, vents, breathers, mut afflict, mut breathed } = air;
    gases.fit(registries.gases.len(), &map);
    let here = map.current();
    let on_here = |on: Option<&OnMap>| on.map_or(MapId::SURFACE, |m| m.0) == here;
    for step in 0..passed {
        let turn = clock.turn_number() + 1 - passed + step;
        let spill = spill_salt(&seed, turn);
        for (pos, vent, on) in &vents {
            if on_here(on) {
                gases.release(vent.gas, pos.0, vent.amount, &map, spill);
            }
        }
        let swirl = swirl_salt(&seed, turn);
        for (id, def) in registries.gases.iter() {
            let Some(layer) = gases.layers.get_mut(id.index()) else { continue };
            let origin = layer.origin();
            gas::diffuse(layer.field_mut(), def, |p| !map.blocks_projectiles(p + origin), |p| roll(swirl, p + origin));
        }
    }
    // Thick enough to hide what is behind it: into the veil the map reads
    // opacity through, so sight and light both stop there.
    let layers = &gases.layers;
    let veiled: Vec<Point> = registries
        .gases
        .iter()
        .filter(|(_, def)| def.veils_at.is_some())
        .filter_map(|(id, def)| layers.get(id.index()).map(|l| (l, def)))
        .flat_map(|(layer, def)| layer.cells().filter(move |(_, c)| def.veils(*c)).map(|(p, _)| p))
        .collect();
    map.set_veil(veiled);
    for (actor, pos, on) in &breathers {
        if !on_here(on) {
            continue;
        }
        for (id, def) in registries.gases.iter() {
            let amount = gases.at(id, pos.0);
            if amount == 0 {
                continue;
            }
            breathed.write(Breathed { actor, gas: id, amount });
            if let Some(breath) = def.breathed(amount) {
                afflict.write(Afflict { target: actor, status: breath.status, turns: breath.turns, by: None });
            }
        }
    }
}

/// Refuses play when a gas inflicts a status and nothing would land it.
fn check_breaths(registries: Option<Res<Registries>>, statuses: Option<Res<Messages<crate::status::StatusEvent>>>) {
    let Some(registries) = registries else { return };
    let biting: Vec<&str> = registries.gases.iter().filter(|(_, def)| def.inflicts.is_some()).map(|(_, def)| def.name.as_str()).collect();
    assert!(
        biting.is_empty() || statuses.is_some(),
        "GasPlugin: {biting:?} inflict a status, but StatusPlugin was not added, so breathing them would do nothing; add StatusPlugin"
    );
}

/// Gas: [`Gases`], given off by [`Release`] and [`Vents`], spread and faded
/// every whole turn, hiding what is behind it and breathed by whoever stands
/// in it.
///
/// Needs [`Registries`] with the gases in it, the run's [`Seed`], and [`StatusPlugin`](crate::status::StatusPlugin)
/// as well when a gas inflicts a status, which it checks when play begins.
/// Registers the `Emit` ability effect, so abilities may give gas off.
pub struct GasPlugin;

impl Plugin for GasPlugin {
    fn build(&self, app: &mut App) {
        use crate::effects::AddEffect;
        use crate::plugin::{FieldSet, Needs, ResetsOnNewRun, Turn};
        app.init_resource::<Gases>()
            .reset_on_new_run::<Gases>()
            .add_message::<Release>()
            .add_message::<Breathed>()
            .add_message::<Afflict>()
            .needs::<Registries>("GasPlugin", "`Registries`, with the gases in it, loaded with `gas::load`")
            .needs::<Seed>("GasPlugin", "`Seed(RunSeed(n))`, which a cloud's spill and swirls are hashed from")
            .add_effect::<crate::effects::Emit>()
            .add_systems(Turn, (release_gas, step_gases).chain().in_set(FieldSet::Gas))
            .add_systems(OnEnter(crate::state::EngineState::Playing), check_breaths);
    }

    fn finish(&self, app: &mut App) {
        crate::plugin::depends_on::<crate::plugin::CorePlugin>(app, "GasPlugin");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{CombatPlugin, Health};
    use crate::components::{Blocks, Player, RevealsMap, Viewshed};
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use crate::status::{Afflicted, StatusPlugin};
    use crate::turn::{Intent, Wait};
    use rl_grid::TileId;
    use rl_rules::gas::GasDef;
    use rl_rules::{Registry, StatusDef, StatusId};

    struct Rig {
        app: App,
        player: Entity,
        start: Point,
        smoke: GasId,
        fumes: GasId,
        choking: StatusId,
    }

    impl Rig {
        /// Open ground, a player, smoke thick enough to hide behind, and fumes
        /// that choke whoever breathes enough of them.
        fn new() -> Self {
            let mut app = headless_app();
            app.add_plugins((crate::fov::FovPlugin, CombatPlugin, StatusPlugin, GasPlugin, crate::world::StreamingPlugin));
            let start = crate::testing::surface(&mut app);
            crate::testing::two_sides(&mut app);
            let statuses = Registry::from_defs(vec![StatusDef::new("choking")]).unwrap();
            let choking = statuses.expect("choking");
            let gases = Registry::from_defs(vec![
                GasDef::new("smoke").spread(60).fade(5).veils_at(60),
                GasDef::new("fumes").spread(40).fade(10).inflicts(50, choking, 2),
            ])
            .unwrap();
            let (smoke, fumes) = (gases.expect("smoke"), gases.expect("fumes"));
            {
                let mut registries = app.world_mut().resource_mut::<Registries>();
                registries.statuses = statuses;
                registries.gases = gases;
            }
            let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(10), RevealsMap, Health::full(30))).id();
            app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
            app.update();
            app.update();
            Self { app, player, start, smoke, fumes, choking }
        }

        fn wait(&mut self, turns: usize) {
            for _ in 0..turns {
                self.app.world_mut().write_message(Intent::new(self.player, Wait));
                self.app.update();
            }
        }

        fn gas(&self, gas: GasId, p: Point) -> u8 {
            self.app.world().resource::<Gases>().at(gas, p)
        }

        fn sees(&self, p: Point) -> bool {
            self.app.world().get::<Viewshed>(self.player).unwrap().can_see(p)
        }
    }

    #[test]
    fn a_vent_fills_the_cells_around_it_and_a_wall_keeps_it_out() {
        let mut rig = Rig::new();
        let fumes = rig.fumes;
        for y in -4..=4 {
            let wall = rig.start.offset(5, y);
            rig.app.world_mut().resource_mut::<WorldMap>().set_tile(wall, TileId(1));
        }
        let vent = rig.start.offset(3, 0);
        rig.app.world_mut().spawn((Position(vent), Vents { gas: fumes, amount: 60 }));
        rig.wait(8);
        assert!(rig.gas(fumes, vent) > 0 && rig.gas(fumes, vent.offset(-1, 0)) > 0, "it filled the cells around the vent");
        assert_eq!(rig.gas(fumes, rig.start.offset(5, 0)), 0, "not the wall");
        assert_eq!(rig.gas(fumes, rig.start.offset(6, 0)), 0, "nor past it");
    }

    #[test]
    fn thick_smoke_hides_what_is_behind_it_until_it_thins() {
        let mut rig = Rig::new();
        let (smoke, behind) = (rig.smoke, rig.start.offset(6, 0));
        assert!(rig.sees(behind));
        for y in -2..=2 {
            let at = rig.start.offset(3, y);
            rig.app.world_mut().write_message(Release { gas: smoke, at, amount: 255 });
        }
        rig.wait(1);
        assert!(!rig.sees(behind), "hidden behind the smoke");
        assert!(rig.sees(rig.start.offset(2, 0)), "while this side of it is still in sight");
        rig.wait(80);
        assert!(rig.sees(behind), "and in sight again once it thinned");
    }

    #[test]
    fn breathing_enough_of_a_gas_that_bites_inflicts_its_status() {
        let mut rig = Rig::new();
        let (fumes, choking, player, at) = (rig.fumes, rig.choking, rig.player, rig.start);
        rig.app.world_mut().write_message(Release { gas: fumes, at, amount: 30 });
        rig.wait(1);
        assert!(!rig.app.world().get::<Afflicted>(player).unwrap().has(choking), "too thin to choke on");
        rig.app.world_mut().write_message(Release { gas: fumes, at, amount: 200 });
        rig.wait(1);
        assert!(rig.app.world().get::<Afflicted>(player).unwrap().has(choking), "thick enough to");
    }
}
