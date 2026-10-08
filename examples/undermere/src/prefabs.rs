//! Undermere's pieces, as data: Mereholt, the whole town in one piece; the
//! throne, with the king and his court at their posts; and the one-cell
//! pieces the floors are furnished with, a chest, a brazier and a
//! candelabra. Each is a file under `assets/prefabs/`.
//!
//! The engine fills them. `dungeon` stamps each piece by name through
//! [`Prefabs::piece`], and on the arrival that built a place the engine's
//! `PrefabPlugin` puts down what every slot names: a prop from
//! `props.ron`, or a monster from the bestiary, made by
//! [`Bestiary`](crate::bestiary::Bestiary) exactly as a floor's own
//! population would make it. The one mark Undermere gives a meaning to
//! itself is the town's `@`, where a run arrives.
//!
//! Task 8a's five mini-bosses add a lair apiece, each its own piece, and
//! the one role a slot draws for yet, `grubnak-honor-guard`, in
//! `assets/roles.ron`.

use rl_engine::prelude::*;
use rl_engine::rl_rules::prefab;
use rl_engine::rl_rules::role;

use crate::bestiary::MonsterDef;
use crate::content::Library;

/// Every piece, compiled in so the binary runs from anywhere.
const PIECES: [&str; 10] = [
    include_str!("../assets/prefabs/mereholt.ron"),
    include_str!("../assets/prefabs/throne.ron"),
    include_str!("../assets/prefabs/chest.ron"),
    include_str!("../assets/prefabs/brazier.ron"),
    include_str!("../assets/prefabs/candelabra.ron"),
    include_str!("../assets/prefabs/grubnak.ron"),
    include_str!("../assets/prefabs/widow.ron"),
    include_str!("../assets/prefabs/choirmaster.ron"),
    include_str!("../assets/prefabs/drowned_abbess.ron"),
    include_str!("../assets/prefabs/forgemaster.ron"),
];

/// Every role a prefab's slot may draw for, compiled in the same way.
const ROLES: &str = include_str!("../assets/roles.ron");

/// Every piece, resolved against `tiles`, the props and the bestiary the
/// run loads, so a piece's ids are the running game's. Panics with every
/// problem in every file, since a broken piece is a place that cannot be
/// built.
pub fn load(tiles: &TileRegistry) -> Prefabs<MonsterDef> {
    // The whole library, in its one order, since the monsters a piece
    // stands are read against the traits and the items they name.
    let library = Library::load(&crate::content::effect_kinds(), &Moments::default());
    let names = library.registries.names().with("monster", &library.bestiary.defs).with("prop", &library.registries.props);
    // Roles read before the pieces, which name one by a `Monster(role:)`
    // slot and need it in `names` to resolve against.
    let roles = role::load::<MonsterDef>(ROLES, &names).unwrap_or_else(|e| panic!("assets/roles.ron: {e}"));
    let names = names.with("role", &roles);
    let defs = PIECES.iter().map(|text| prefab::load(text, tiles, &names).unwrap_or_else(|e| panic!("assets/prefabs: {e}"))).collect();
    Prefabs::new(Registry::from_defs(defs).expect("every piece has a name of its own"), roles)
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_core::RunSeed;

    use super::*;
    use crate::dungeon::Undermere;

    /// Every piece loads against the real tiles, props and bestiary, and
    /// nothing is lost on the way; Mereholt is the town as it was drawn.
    #[test]
    fn every_piece_loads_against_the_tiles_the_props_and_the_bestiary() {
        let undermere = Undermere::new(RunSeed(0));
        let prefabs = load(undermere.tiles());
        assert_eq!(prefabs.defs().len(), PIECES.len());
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
        ] {
            assert!(prefabs.defs().id(name).is_some(), "no piece called {name:?}");
        }
        assert!(prefabs.roles().id("grubnak-honor-guard").is_some(), "Grubnak's own honor guard role");
        let town = prefabs.defs().get(prefabs.defs().expect("Mereholt"));
        assert_eq!(town.rows().len(), 36, "thirty-six rows");
        assert!(town.rows().iter().all(|row| row.chars().count() == 64), "each sixty-four across");
    }
}
