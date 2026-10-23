//! Cells that hide what they are: a place's secret doors and the walled
//! mouths of its hidden rooms, kept until they are found.
//!
//! A builder hides an opening by walling it over and publishing an
//! [`atglyph_mapgen::SecretCell`] for every cell it walled; a [`PlaceBuild`]
//! carries them out, and [`WorldMap::install_place`] keeps each as a
//! [`Secret`] on the place it belongs to. Until it is found a secret is
//! the wall its terrain says it is, to everyone: a mind paths round it, a
//! projectile stops at it, and a hidden room is a pocket and never a back
//! door. [`WorldMap::find_secret`] lays the tile it really is, for its
//! whole group at once, and from then on it is that tile and nothing
//! else, so a found door opens, closes and is pathed through like any door.
//!
//! Finding one is the spotting roll's, in [`props`](crate::props), which
//! rolls for hidden props and secrets alike from the one stream: a secret
//! is rolled for while any cell of it is in the player's sight, once for
//! its whole group.
//!
//! A secret is kept by the place rather than spawned as an entity, the way
//! a [`Lock`](crate::places::Lock) is, because it is a fact about a cell of
//! a place's terrain and not a thing standing on one. Kept there it is on
//! its own map without anyone saying so, stays there through a switch to
//! another map, and goes into the save with the terrain it walls, so a
//! found one stays found and a hidden one stays hidden on a continued run.
//! An entity would have had to be spawned wherever a place is installed,
//! which is a [`WorldMap`] method with no `Commands`, and skipped by every
//! query that reads whatever has a position.
//!
//! [`PlaceBuild`]: crate::places::PlaceBuild

use atglyph_core::Point;
use atglyph_grid::TileId;

#[cfg(doc)]
use crate::world::WorldMap;

/// One cell of a place still hiding what it is: wall in the terrain, and
/// [`is`](Self::is) once found.
///
/// What [`atglyph_mapgen::SecretCell`] says, less the areas it joins: those are
/// indices into the build's own areas, which a place does not keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Secret {
    /// Where.
    pub at: Point,
    /// The tile it really is, laid when it is found.
    pub is: TileId,
    /// The tile it shows while hidden, the wall its build laid over it.
    /// Found, the cell becomes [`is`](Self::is) only while it still shows
    /// this, so a cell a game changed since, dug out or built over, keeps
    /// the change rather than having a door laid over it. A save written
    /// before this was kept reads it from the saved terrain.
    #[serde(default = "Secret::unread")]
    pub shows: TileId,
    /// Percent chance a turn of being spotted while in sight, before the
    /// roll's nearness, keenness and stillness.
    pub spot: u8,
    /// The opening it hides. Every cell of one shares it, and is found
    /// together.
    pub group: u32,
}

impl Secret {
    /// What `secret` says, kept by a place whose build shows `shows` on
    /// its cell.
    pub fn hidden(secret: atglyph_mapgen::SecretCell, shows: TileId) -> Self {
        Self { at: secret.at, is: secret.is, shows, spot: secret.spot, group: secret.group }
    }

    /// What a save written before [`shows`](Self::shows) was kept reads
    /// it as until the place it is in fills it from its terrain: no tile
    /// a registry hands out, which a registry would need every id but one
    /// to reach.
    pub(crate) const fn unread() -> TileId {
        TileId(u16::MAX)
    }
}

#[cfg(test)]
mod tests {
    use atglyph_core::{Direction, Point};
    use atglyph_grid::{Terrain, TileRegistry};
    use bevy::prelude::*;

    use crate::components::{Actor, Blocks, Player, Position, Viewshed};
    use crate::doors::Open;
    use crate::places::{MapId, OnMap, PlaceBuild};
    use crate::state::EngineState;
    use crate::testing::headless_app;
    use crate::turn::Intent;
    use crate::world::WorldMap;

    const PLACE: MapId = MapId(1);

    /// A corridor nine long, walled at x = 3 by a hidden door (group 0)
    /// and with a three-wide walled mouth at x = 6 (group 1).
    fn hiding() -> (WorldMap, TileRegistry) {
        let tiles = TileRegistry::standard();
        let (wall, floor, door) = (tiles.expect("wall"), tiles.expect("floor"), tiles.expect("door_closed"));
        let mut terrain = Terrain::filled(9, 5, floor);
        for y in 0..5 {
            terrain.set(Point::new(3, y), wall);
            terrain.set(Point::new(6, y), wall);
        }
        let secret = |at: Point, is, group| atglyph_mapgen::SecretCell { at, is, spot: 6, group, joins: (0, 1) };
        let secrets =
            vec![secret(Point::new(3, 2), door, 0), secret(Point::new(6, 1), floor, 1), secret(Point::new(6, 2), floor, 1), secret(Point::new(6, 3), floor, 1)];
        let build = PlaceBuild {
            terrain,
            entry: Point::new(2, 2),
            exit: None,
            spots: Vec::new(),
            locks: Vec::new(),
            secrets,
            ambient: atglyph_grid::Light::DARK,
            peaceful: false,
        };
        let mut map = WorldMap::new(tiles.tables());
        map.install_place(PLACE, build);
        map.switch_to(PLACE);
        (map, tiles)
    }

    #[test]
    fn one_cell_of_a_group_found_finds_the_whole_group_and_nothing_else() {
        let (mut map, tiles) = hiding();
        assert_eq!(map.tile(Point::new(6, 1)), Some(tiles.expect("wall")), "hidden, it is the wall it looks like");
        let found = map.find_secret(PLACE, Point::new(6, 3));
        assert_eq!(found.iter().map(|s| s.at).collect::<Vec<_>>(), [Point::new(6, 1), Point::new(6, 2), Point::new(6, 3)]);
        for y in 1..4 {
            assert_eq!(map.tile(Point::new(6, y)), Some(tiles.expect("floor")), "the whole mouth is open");
        }
        assert_eq!(map.tile(Point::new(3, 2)), Some(tiles.expect("wall")), "and the door in the other wall is still hidden");
        assert_eq!(map.secrets(PLACE).len(), 1);
        assert!(map.find_secret(PLACE, Point::new(6, 2)).is_empty(), "a found secret is found once");
        assert!(map.find_secret(PLACE, Point::new(4, 2)).is_empty(), "and a plain cell hides nothing");
    }

    /// A game that dug through a hidden mouth's middle keeps its dug
    /// cell when the mouth is found; the cells still wall open as ever.
    #[test]
    fn a_found_secret_leaves_a_cell_a_game_changed_since_as_the_game_left_it() {
        let (mut map, tiles) = hiding();
        let dug = tiles.expect("door_open");
        assert!(map.set_tile(Point::new(6, 2), dug), "the game changes the middle of the mouth");
        let found = map.find_secret(PLACE, Point::new(6, 1));
        assert_eq!(found.len(), 3, "the whole group is found");
        assert_eq!(map.tile(Point::new(6, 2)), Some(dug), "the changed cell keeps the change");
        assert_eq!((map.tile(Point::new(6, 1)), map.tile(Point::new(6, 3))), (Some(tiles.expect("floor")), Some(tiles.expect("floor"))));
    }

    #[test]
    fn a_found_secret_door_opens_like_any_door_and_moves_the_cost_epoch_minds_path_by() {
        let (map, tiles) = hiding();
        let mut app = headless_app();
        app.insert_resource(map);
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(Point::new(2, 2)), OnMap(PLACE), Viewshed::new(8))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        let door = Point::new(3, 2);
        let tile = |app: &App| app.world().resource::<WorldMap>().tile(door);

        app.world_mut().write_message(Intent::new(player, Open(Direction::East)));
        app.update();
        assert_eq!(tile(&app), Some(tiles.expect("wall")), "hidden, there is nothing there to open");

        let epoch = app.world().resource::<WorldMap>().cost_epoch();
        app.world_mut().resource_mut::<WorldMap>().find_secret(PLACE, door);
        assert!(app.world().resource::<WorldMap>().cost_epoch() > epoch, "a mind's flow fields know a way opened");
        assert_eq!(tile(&app), Some(tiles.expect("door_closed")), "found, it is the door it was");

        app.world_mut().write_message(Intent::new(player, Open(Direction::East)));
        app.update();
        assert_eq!(tile(&app), Some(tiles.expect("door_open")), "and it opens like any door");
    }

    #[test]
    fn a_place_s_secrets_stay_as_they_were_through_a_map_switch_and_a_save_and_an_older_save_has_none() {
        let (mut map, tiles) = hiding();
        map.find_secret(PLACE, Point::new(6, 2));
        map.install_place(
            MapId(2),
            PlaceBuild {
                terrain: Terrain::filled(3, 3, tiles.expect("floor")),
                entry: Point::new(1, 1),
                exit: None,
                spots: Vec::new(),
                locks: Vec::new(),
                secrets: Vec::new(),
                ambient: atglyph_grid::Light::DARK,
                peaceful: false,
            },
        );
        map.switch_to(MapId(2));
        map.switch_to(PLACE);
        let text = ron::to_string(&map.export()).unwrap();
        let mut back = WorldMap::new(tiles.tables());
        back.import(ron::from_str(&text).unwrap());
        assert_eq!(back.secrets(PLACE), map.secrets(PLACE), "the hidden door is still hidden");
        assert_eq!(back.tile(Point::new(3, 2)), Some(tiles.expect("wall")));
        assert_eq!(back.tile(Point::new(6, 2)), Some(tiles.expect("floor")), "and the found mouth still found");
        assert_eq!(back.find_secret(PLACE, Point::new(3, 2)).len(), 1, "and still there to find");

        // A save written before places had secrets has no word on them.
        let mut old = text.clone();
        while let Some(start) = old.find("secrets:[") {
            let end = start + old[start..].find(']').expect("the list ends") + 1;
            old = format!("{}{}", &old[..start], old[end..].trim_start_matches(','));
        }
        assert!(!old.contains("secrets"), "{old}");
        let mut older = WorldMap::new(tiles.tables());
        older.import(ron::from_str(&old).unwrap());
        assert!(older.secrets(PLACE).is_empty(), "it loads, hiding nothing");

        // One written before a secret kept the tile it shows reads it
        // from the terrain saved with it, and is found as ever.
        let unshown = text.replace(&format!("shows:{},", ron::to_string(&tiles.expect("wall")).unwrap()), "");
        assert!(!unshown.contains("shows"), "{unshown}");
        let mut before = WorldMap::new(tiles.tables());
        before.import(ron::from_str(&unshown).unwrap());
        assert_eq!(before.secrets(PLACE), map.secrets(PLACE), "the shown wall is read back from the terrain");
        before.find_secret(PLACE, Point::new(3, 2));
        assert_eq!(before.tile(Point::new(3, 2)), Some(tiles.expect("door_closed")));
    }
}
