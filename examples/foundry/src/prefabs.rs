//! Foundry's pieces and roles, as data: the armories, the stores, the
//! reactor and the core, and the guard post, each a file under
//! `assets/prefabs/`, and `assets/roles.ron` naming who may stand in a
//! guard's slot.
//!
//! The engine fills them. `decks` stamps each piece by name through
//! [`Prefabs::piece`], and on the arrival that built a deck the engine's
//! `PrefabPlugin` puts down what every slot names: a locker or a crate, a
//! weapon drawn by tag, a guard drawn by role, each at the deck's own band
//! plus what the slot adds. What is left to Foundry is the one mark it
//! gives meaning to itself, the reactor's `R`, where `mission` stands the
//! console.
//!
//! [`coverage_report`] is the other half of loading: whether each drawn
//! slot finds anything on every deck, which no load can tell, printed by
//! `foundry --prefabs`.

use rl_engine::prelude::*;
use rl_engine::rl_core::RunSeed;
use rl_engine::rl_grid::TileRegistry;
use rl_engine::rl_rules::prefab::{self, Coverage, Sources};
use rl_engine::rl_rules::role;

use crate::decks::{DECKS, Foundry};
use crate::droids::{MonsterDef, Roster};

/// Every piece a deck is built from, compiled in so the binary runs from
/// anywhere.
const PIECES: [&str; 7] = [
    include_str!("../assets/prefabs/armory_wide.ron"),
    include_str!("../assets/prefabs/armory_tall.ron"),
    include_str!("../assets/prefabs/store_wide.ron"),
    include_str!("../assets/prefabs/store_tall.ron"),
    include_str!("../assets/prefabs/reactor.ron"),
    include_str!("../assets/prefabs/core.ron"),
    include_str!("../assets/prefabs/guard_post.ron"),
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
    Prefabs::new(Registry::from_defs(defs).expect("every piece has its own name"), roles)
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
    let armory = crate::gear::Armory::load(&registries, &crate::gear::effect_kinds(), &Moments::default());
    let prefabs = Foundry::new(RunSeed(0)).prefabs().clone();
    let sources = Sources { roles: prefabs.roles(), monsters: &roster.table, items: &armory.table, tags: &registries.tags };
    prefab::coverage(prefabs.defs().iter().map(|(_, def)| def), &sources, 1..=DECKS as i32)
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;
    use rl_engine::rl_bevy::places::Spot;
    use rl_engine::rl_rules::prefab::{Pick, Slot};

    use super::*;

    /// Every piece and the roles file load against the real tiles, props,
    /// tags and roster, and nothing is lost on the way: seven pieces in,
    /// seven out, and both roles the guard post asks for.
    #[test]
    fn every_piece_and_role_file_loads() {
        let prefabs = load(Foundry::new(RunSeed(0)).tiles());
        assert_eq!(prefabs.defs().len(), PIECES.len());
        assert!(prefabs.roles().id("sentry").is_some() && prefabs.roles().id("brute").is_some());
    }

    /// A slot that draws by tag or role finds something on every deck from
    /// one to ten, exactly or by falling back, so no guard post ever
    /// stands unmanned or holds no weapon because a table ran out.
    #[test]
    fn every_drawn_slot_of_every_piece_finds_something_on_every_deck() {
        let report = coverage_report();
        assert!(!report.rows.is_empty(), "the guard post draws, so the report has rows");
        assert!(report.empties().is_empty(), "{}", report.render());
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

    /// The guard post's spots on the current deck, `'s'` and `'b'`, as
    /// the stamp recorded them.
    fn guard_spots(app: &App) -> Vec<Spot> {
        let map = app.world().resource::<WorldMap>();
        let prefabs = app.world().resource::<Prefabs<MonsterDef>>();
        let place = map.place(map.current()).expect("the current deck is built");
        place
            .spots
            .iter()
            .filter(|s| {
                s.prefab.is_some_and(|key| prefabs.slot(key, char::from_u32(s.tag).unwrap_or(' ')).is_some_and(|slot| matches!(slot, Slot::Monster { .. })))
            })
            .cloned()
            .collect()
    }

    /// Every guard the post's slots name stands at its slot, is a kind its
    /// role admits, and holds that cell as its post; the one exception is
    /// a slot on the cell the commando arrived on, which the engine skips.
    #[test]
    fn a_guard_post_is_manned_and_its_guards_hold_their_cells() {
        let mut checked = 0;
        for seed in 1..=5u64 {
            let mut app = crate::testing::headless(RunSeed(seed));
            crate::testing::arrive_on(&mut app, 2);
            let entry = {
                let world = app.world_mut();
                world.query_filtered::<&Position, With<Player>>().single(world).expect("the commando stands somewhere").0
            };
            let spots = guard_spots(&app);
            assert_eq!(spots.len(), 3, "seed {seed}: two sentries and a brute on deck two");
            for spot in spots {
                if spot.at == entry {
                    continue;
                }
                let world = app.world_mut();
                let held: Vec<(Id<MonsterDef>, Option<Post>)> = world
                    .query::<(&Position, &crate::droids::Kind, Option<&Post>, &OnMap)>()
                    .iter(world)
                    .filter(|(p, _, _, on)| p.0 == spot.at && on.0 == crate::decks::map_of(2))
                    .map(|(_, kind, post, _)| (kind.0, post.copied()))
                    .collect();
                let prefabs = app.world().resource::<Prefabs<MonsterDef>>();
                let glyph = char::from_u32(spot.tag).unwrap();
                let Some(Slot::Monster { pick: Pick::Role(role), .. }) = prefabs.slot(spot.prefab.unwrap(), glyph) else {
                    panic!("seed {seed}: '{glyph}' is a role slot");
                };
                let role = prefabs.roles().get(*role);
                assert_eq!(held.len(), 1, "seed {seed}: one monster at '{glyph}' {:?}", spot.at);
                let (kind, post) = held[0];
                assert!(role.fits(kind), "seed {seed}: '{glyph}' holds a monster outside {:?}", role.name);
                assert_eq!(post, Some(Post(spot.at)), "seed {seed}: the guard at '{glyph}' holds its own cell");
                checked += 1;
            }
        }
        assert!(checked >= 12, "most guards were checked, not skipped: {checked}");
    }
}
