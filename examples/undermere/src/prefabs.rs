//! Undermere's pieces, as data: Mereholt, the whole town in one piece;
//! the floor ten boss's own throne, one of three a run draws between
//! (`dungeon::FinalBoss`), and the five mini-bosses' lairs; the one-cell
//! pieces a floor is furnished with, a chest, and the brazier, candelabra,
//! hanging lantern, witchlight and topiary a biome's decor in
//! `assets/biomes.ron` names, the body `decor::BloodTrail` stands at the end of its own trail
//! and the wall `decor::WallInscriptions` carves a line into; and the
//! pool's pieces, flavor, risk, reward and landmark, under
//! `assets/prefabs/flavor/`, `risk/`, `reward/` and `landmark/`. Each is a file under `assets/prefabs/`, and the
//! pool's dressings and the pool itself are loaded beside them.
//!
//! The engine fills them. `dungeon` stamps each piece by name through
//! [`PrefabLibrary::piece`](atglyph::rules::PrefabLibrary::piece), and on the arrival that built a place the engine's
//! `PrefabPlugin` puts down what every slot names: a prop from
//! `props.ron`, or a monster from the bestiary, made by
//! [`Bestiary`] exactly as a floor's own
//! population would make it. The marks Undermere gives a meaning to
//! itself are the town's `@`, where a run arrives, and a vault piece's,
//! which [`vaults`](crate::vaults) fills.
//!
//! The five mini-bosses add a lair apiece, each its own piece, and
//! the one role a slot draws for yet, `grubnak-honor-guard`, in
//! `assets/roles.ron`. Two more thrones, the Mother Below's cave and the
//! Ember Wyrm's hall, stand beside the Drowned King's own.

use atglyph::prelude::all::*;
use atglyph::rules::dressing;
use atglyph::rules::layout::LayoutDef;
use atglyph::rules::pool::{self, Pool};
use atglyph::rules::prefab;
use atglyph::rules::role;

use crate::bestiary::{Bestiary, MonsterDef};
use crate::content::Library;

/// Every piece, compiled in so the binary runs from anywhere.
const PIECES: [&str; 66] = [
    include_str!("../assets/prefabs/mereholt.ron"),
    include_str!("../assets/prefabs/throne.ron"),
    include_str!("../assets/prefabs/chest.ron"),
    include_str!("../assets/prefabs/brazier.ron"),
    include_str!("../assets/prefabs/candelabra.ron"),
    include_str!("../assets/prefabs/hanging_lantern.ron"),
    include_str!("../assets/prefabs/witchlight.ron"),
    include_str!("../assets/prefabs/topiary.ron"),
    include_str!("../assets/prefabs/fallen_body.ron"),
    include_str!("../assets/prefabs/wall_inscription.ron"),
    include_str!("../assets/prefabs/grubnak.ron"),
    include_str!("../assets/prefabs/widow.ron"),
    include_str!("../assets/prefabs/choirmaster.ron"),
    include_str!("../assets/prefabs/drowned_abbess.ron"),
    include_str!("../assets/prefabs/forgemaster.ron"),
    // The floor ten boss, drawn from three.
    include_str!("../assets/prefabs/mother_below.ron"),
    include_str!("../assets/prefabs/ember_wyrm.ron"),
    // The flavor pool's thirteen fixed pieces: nothing to take, each a
    // place that says what it was.
    include_str!("../assets/prefabs/flavor/cold_campsite.ron"),
    include_str!("../assets/prefabs/flavor/last_words.ron"),
    include_str!("../assets/prefabs/flavor/fallen_statue.ron"),
    include_str!("../assets/prefabs/flavor/wayside_shrine.ron"),
    include_str!("../assets/prefabs/flavor/stewards_audience.ron"),
    include_str!("../assets/prefabs/flavor/coal_store.ron"),
    include_str!("../assets/prefabs/flavor/glowcap_ring.ron"),
    include_str!("../assets/prefabs/flavor/cart_track.ron"),
    include_str!("../assets/prefabs/flavor/embalmers_room.ron"),
    include_str!("../assets/prefabs/flavor/orrery.ron"),
    include_str!("../assets/prefabs/flavor/baptistery.ron"),
    include_str!("../assets/prefabs/flavor/fallen_bell.ron"),
    include_str!("../assets/prefabs/flavor/stilt_hut.ron"),
    // The risk-and-reward pool's twenty-two pieces: what each does that
    // no prop or effect says is `src/vaults.rs`'s.
    include_str!("../assets/prefabs/risk/caged_altars.ron"),
    include_str!("../assets/prefabs/risk/the_locked_vault.ron"),
    include_str!("../assets/prefabs/risk/the_trapped_corridor.ron"),
    include_str!("../assets/prefabs/risk/the_rat_trap.ron"),
    include_str!("../assets/prefabs/risk/the_flooding_vault.ron"),
    include_str!("../assets/prefabs/risk/the_rotten_floor.ron"),
    include_str!("../assets/prefabs/risk/the_gas_chamber.ron"),
    include_str!("../assets/prefabs/risk/gallery_of_darts.ron"),
    include_str!("../assets/prefabs/risk/the_drowned_reliquary.ron"),
    include_str!("../assets/prefabs/risk/the_sealed_vault.ron"),
    include_str!("../assets/prefabs/risk/the_warded_hall.ron"),
    include_str!("../assets/prefabs/risk/the_sleeping_giant.ron"),
    include_str!("../assets/prefabs/risk/the_paralysis_vestibule.ron"),
    include_str!("../assets/prefabs/risk/the_collapsing_gallery.ron"),
    include_str!("../assets/prefabs/risk/the_guarded_hoard.ron"),
    include_str!("../assets/prefabs/risk/captives_in_chains.ron"),
    include_str!("../assets/prefabs/risk/the_tinder_room.ron"),
    include_str!("../assets/prefabs/risk/the_spiders_larder.ron"),
    include_str!("../assets/prefabs/risk/the_goblin_stockade.ron"),
    include_str!("../assets/prefabs/risk/the_bone_circle.ron"),
    include_str!("../assets/prefabs/risk/the_brood_chamber.ron"),
    include_str!("../assets/prefabs/risk/the_last_stand.ron"),
    // The reward pool's twelve pieces (the other four are dressed, in
    // `assets/dressings.ron`).
    include_str!("../assets/prefabs/reward/jewellers_niche.ron"),
    include_str!("../assets/prefabs/reward/quiet_reliquary.ron"),
    include_str!("../assets/prefabs/reward/treasure_room.ron"),
    include_str!("../assets/prefabs/reward/hidden_cache.ron"),
    include_str!("../assets/prefabs/reward/a_fallen_adventurer.ron"),
    include_str!("../assets/prefabs/reward/wizards_study.ron"),
    include_str!("../assets/prefabs/reward/healing_spring.ron"),
    include_str!("../assets/prefabs/reward/enchanters_altar.ron"),
    include_str!("../assets/prefabs/reward/the_cold_forge.ron"),
    include_str!("../assets/prefabs/reward/overturned_wagon.ron"),
    include_str!("../assets/prefabs/reward/library_nook.ron"),
    include_str!("../assets/prefabs/reward/moonwell.ron"),
    // The landmarks, each a floor's first piece that the floor is grown
    // round, one to a biome that has one.
    include_str!("../assets/prefabs/landmark/the_hunting_lodge.ron"),
    include_str!("../assets/prefabs/landmark/the_hedge_maze.ron"),
];

/// The Drowned Town's first wave's pieces, compiled in beside the rest and
/// kept in a list of their own, so a later wave's pieces land in theirs:
/// the Old Town's and the Harbour's lights, their two flavor pieces, and
/// the two landmarks of floors one and two, the Watch Barracks' gatehouse
/// and the Harbour's sunken ship, which `assets/pool.ron` seeds.
const TOWN_WAVE: [&str; 6] = [
    include_str!("../assets/prefabs/street_lamp.ron"),
    include_str!("../assets/prefabs/harbour_lantern.ron"),
    include_str!("../assets/prefabs/flavor/town_well.ron"),
    include_str!("../assets/prefabs/flavor/upturned_boat.ron"),
    include_str!("../assets/prefabs/landmark/the_gatehouse.ron"),
    include_str!("../assets/prefabs/landmark/the_sunken_ship.ron"),
];

/// Every role a prefab's slot may draw for, compiled in the same way.
const ROLES: &str = include_str!("../assets/roles.ron");

/// Every dressing, compiled in beside the pieces: thirty-five of the flavor
/// pool's own, laid by the biome lists `dungeon/biomes.rs` builds from
/// `assets/biomes.ron`'s own `dressings`, and four of the reward pool's.
const DRESSINGS_RON: &str = include_str!("../assets/dressings.ron");

/// The run's pool, compiled in beside the pieces and the dressings it
/// draws from both.
const POOL_RON: &str = include_str!("../assets/pool.ron");

/// Every piece, dressing and pool entry, resolved against `tiles`, `layout`,
/// the props and the bestiary the run loads, so a piece's ids are the
/// running game's. Panics with every problem in every file, since a
/// broken piece, dressing or pool entry is a place that cannot be built.
/// The bestiary it read them against comes back too, for the loader of the
/// biomes to hold their favored kinds against.
pub fn load(tiles: &TileRegistry, layout: &LayoutDef) -> (Prefabs<MonsterDef>, Pool, Bestiary) {
    // The whole library, in its one order, since the monsters a piece
    // stands are read against the traits and the items they name.
    let library = Library::load(&crate::content::effect_kinds(), &TriggerKinds::default());
    let names = library.registries.names().with("monster", &library.bestiary.defs).with("prop", &library.registries.props);
    // Roles read before the pieces, which name one by a `Monster(role:)`
    // slot and need it in `names` to resolve against.
    let roles = role::load::<MonsterDef>(ROLES, &names).unwrap_or_else(|e| panic!("assets/roles.ron: {e}"));
    let names = names.with("role", &roles);
    let defs = Registry::from_defs(
        PIECES.iter().chain(&TOWN_WAVE).map(|text| prefab::load(text, tiles, &names).unwrap_or_else(|e| panic!("assets/prefabs: {e}"))).collect(),
    )
    .expect("every piece has a name of its own");
    let dressings = dressing::load::<MonsterDef>(DRESSINGS_RON, tiles, &names).unwrap_or_else(|e| panic!("assets/dressings.ron: {e}"));
    let pool = pool::load(POOL_RON, layout, &defs, &names).unwrap_or_else(|e| panic!("assets/pool.ron: {e}"));
    dressing::check_pool(&pool, &dressings).unwrap_or_else(|e| panic!("assets/pool.ron: {e}"));
    let prefabs = Prefabs::with_dressings(defs, roles, dressings);
    (prefabs, pool, library.bestiary)
}

#[cfg(test)]
mod tests {
    use atglyph::primitives::RunSeed;

    use super::*;
    use crate::dungeon::Undermere;

    /// Every piece loads against the real tiles, props and bestiary, and
    /// nothing is lost on the way; Mereholt is the town as it was drawn.
    #[test]
    fn every_piece_loads_against_the_tiles_the_props_and_the_bestiary() {
        let undermere = Undermere::new(RunSeed(0));
        let (prefabs, _pool, _) = load(undermere.tiles(), &crate::dungeon::layout());
        assert_eq!(prefabs.defs().len(), PIECES.len() + TOWN_WAVE.len());
        for name in [
            "Mereholt",
            "the throne",
            "chest",
            "brazier",
            "candelabra",
            "Grubnak the goblin king",
            "the Widow",
            "the Choirmaster",
            "the Drowned Abbess",
            "the Forgemaster",
            "the Mother Below",
            "the Ember Wyrm",
        ] {
            assert!(prefabs.defs().id(name).is_some(), "no piece called {name:?}");
        }
        assert!(prefabs.roles().id("grubnak-honor-guard").is_some(), "Grubnak's own honor guard role");
        let town = prefabs.defs().get(prefabs.defs().expect("Mereholt"));
        assert_eq!(town.rows().len(), 36, "thirty-six rows");
        assert!(town.rows().iter().all(|row| row.chars().count() == 64), "each sixty-four across");
    }

    /// The pool's own report: every slot a piece or a dressing it lays
    /// draws finds something, item by tag and monster by role, at every
    /// band it may stand on, so a run never stands at a band with nothing
    /// to draw.
    #[test]
    fn the_coverage_report_finds_every_slot_at_every_band_it_may_stand_at() {
        let undermere = Undermere::new(RunSeed(0));
        let (prefabs, pool, _) = load(undermere.tiles(), &crate::dungeon::layout());
        let library = Library::load(&crate::content::effect_kinds(), &TriggerKinds::default());
        let sources = atglyph::rules::prefab::Sources {
            roles: prefabs.roles(),
            monsters: &library.bestiary.spawns,
            items: &library.armory.table,
            tags: &library.registries.tags,
        };
        let pieces = atglyph::rules::pool::coverage(&pool, prefabs.defs(), &sources);
        assert_eq!(pieces.empties(), Vec::new(), "a pool piece's slot finds nothing at some band it stands on");
        let dressed = atglyph::rules::pool::dressing_coverage(&pool, prefabs.dressings(), &sources);
        assert_eq!(dressed.empties(), Vec::new(), "a dressing's slot finds nothing at some band it stands on");
    }
}
