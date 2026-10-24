//! Foundry's pieces and roles, as data: the armories, the stores, the
//! reactor and the core, the guard post, and the rooms that say what the
//! rest of a deck was for, each a file under
//! `assets/prefabs/`, and `assets/roles.ron` naming who may stand in a
//! guard's slot.
//!
//! The engine fills them. `decks` stamps each piece by name through
//! [`PrefabLibrary::piece`](atglyph::rules::PrefabLibrary::piece), and on the arrival that built a deck the engine's
//! `PrefabPlugin` puts down what every slot names: a locker or a crate, a
//! weapon drawn by tag, a guard drawn by role, each at the deck's own band
//! plus what the slot adds. What is left to Foundry is the one mark it
//! gives meaning to itself, the reactor's `R`, where `mission` stands the
//! console.
//!
//! [`coverage_report`] is the other half of loading: whether each drawn
//! slot finds anything on every deck, which no load can tell, printed by
//! `foundry --prefabs`.

use atglyph::grid::TileRegistry;
use atglyph::prelude::all::*;
use atglyph::primitives::RunSeed;
use atglyph::rules::prefab::{self, Coverage, Sources};
use atglyph::rules::role;

use crate::decks::{DECKS, Foundry};
use crate::droids::{MonsterDef, Roster};

/// Every piece a deck is built from, compiled in so the binary runs from
/// anywhere.
const PIECES: [&str; 22] = [
    include_str!("../assets/prefabs/armory_wide.ron"),
    include_str!("../assets/prefabs/armory_tall.ron"),
    include_str!("../assets/prefabs/store_wide.ron"),
    include_str!("../assets/prefabs/store_tall.ron"),
    include_str!("../assets/prefabs/reactor.ron"),
    include_str!("../assets/prefabs/core.ron"),
    include_str!("../assets/prefabs/guard_post.ron"),
    include_str!("../assets/prefabs/machine_shop.ron"),
    include_str!("../assets/prefabs/machine_shop_large.ron"),
    include_str!("../assets/prefabs/charging_bay.ron"),
    include_str!("../assets/prefabs/charging_bay_large.ron"),
    include_str!("../assets/prefabs/parts_store.ron"),
    include_str!("../assets/prefabs/parts_store_large.ron"),
    include_str!("../assets/prefabs/pump_room.ron"),
    include_str!("../assets/prefabs/pump_room_large.ron"),
    include_str!("../assets/prefabs/control_room.ron"),
    include_str!("../assets/prefabs/control_room_large.ron"),
    include_str!("../assets/prefabs/scrap_sorting.ron"),
    include_str!("../assets/prefabs/scrap_sorting_large.ron"),
    include_str!("../assets/prefabs/loading_dock.ron"),
    include_str!("../assets/prefabs/loading_dock_large.ron"),
    include_str!("../assets/prefabs/maintenance_closet.ron"),
];

/// Who may stand in a guard's slot, compiled in beside the pieces.
const ROLES_RON: &str = include_str!("../assets/roles.ron");

/// Every piece and role, resolved against Foundry's tiles, props, tags
/// and roster. Panics with every problem in every file, since a broken
/// piece is a deck that cannot be built.
pub fn load(tiles: &TileRegistry) -> Prefabs<MonsterDef> {
    let registries = crate::content::registries();
    let roster = Roster::load(&registries);
    let base = registries.names();
    let monsters = base.clone().with("monster", &roster.defs);
    let roles = role::load(ROLES_RON, &monsters).unwrap_or_else(|e| panic!("assets/roles.ron: {e}"));
    let names = monsters.with("prop", &registries.props).with("role", &roles);
    let defs = PIECES.iter().map(|text| prefab::load(text, tiles, &names).unwrap_or_else(|e| panic!("assets/prefabs: {e}"))).collect();
    let prefabs = Prefabs::new(Registry::from_defs(defs).expect("every piece has its own name"), roles);
    // Every slot that draws by tag or role finds something on every deck,
    // exactly or by falling back, so no guard post ever stands unmanned or
    // holds no weapon because a table ran out: read against the roster
    // and the item table a run draws through, which is the one question a
    // piece's own load cannot ask.
    let items = crate::gear::load_table(&registries, &crate::gear::load_defs(&registries));
    let sources = Sources { roles: prefabs.roles(), monsters: &roster.table, items: &items, tags: &registries.tags };
    let report = prefab::coverage(prefabs.defs().iter().map(|(_, def)| def), &sources, 1..=DECKS as i32);
    assert!(report.empties().is_empty(), "assets/prefabs: a drawn slot finds nothing on some deck:\n{}", report.render());
    prefabs
}

/// The coverage of every piece's drawn slots on every deck, for
/// `--prefabs` and for the test that holds it free of gaps.
///
/// Drawn through the same roster and armory a run draws through, so the
/// report answers no question a run would answer differently.
pub fn coverage_report() -> Coverage {
    let registries = crate::content::registries();
    let roster = Roster::load(&registries);
    // The armory as the running game loads it; the moments are the
    // engine's, since Foundry registers none of its own.
    let armory = crate::gear::Armory::load(&registries, &crate::gear::effect_kinds(), &TriggerKinds::default(), &crate::gear::sounds());
    let prefabs = Foundry::new(RunSeed(0)).prefabs().clone();
    let sources = Sources { roles: prefabs.roles(), monsters: &roster.table, items: &armory.table, tags: &registries.tags };
    prefab::coverage(prefabs.defs().iter().map(|(_, def)| def), &sources, 1..=DECKS as i32)
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use super::*;

    /// Every piece and the roles file load against the real tiles, props,
    /// tags and roster, and nothing is lost on the way: every piece in is
    /// a piece out, and both roles the guard post asks for.
    #[test]
    fn every_piece_and_role_file_loads() {
        let prefabs = load(Foundry::new(RunSeed(0)).tiles());
        assert_eq!(prefabs.defs().len(), PIECES.len());
        assert!(prefabs.roles().id("sentry").is_some() && prefabs.roles().id("brute").is_some());
    }

    /// Guards drawn for one post fight side by side only if they are on
    /// one side. Droids and vermin are hostile to each other, so a role
    /// mixing the two puts a post's guards at each other's throats the
    /// moment any of them wakes: a scrap crab once drawn as a brute beside
    /// two droid sentries was shot dead by them in two turns.
    #[test]
    fn every_roles_members_all_belong_to_one_faction() {
        let registries = crate::content::registries();
        let roster = Roster::load(&registries);
        let prefabs = load(Foundry::new(RunSeed(0)).tiles());
        for (_, role) in prefabs.roles().iter() {
            let mut factions: Vec<&str> = role.members.iter().map(|m| registries.factions.name(roster.defs.get(*m).faction.id())).collect();
            factions.sort_unstable();
            factions.dedup();
            assert_eq!(factions.len(), 1, "{} mixes factions: {factions:?}", role.name);
        }
    }
}
