//! What lies under the player, for the screen that asks which of it to
//! pick up.
//!
//! A view of the player's own cell rather than of everything in sight: the
//! nearby rail lists what can be seen, and this what can be taken without
//! a step, each thing with its count, so a pile of one kind can be asked
//! how many of it.

use atglyph_bevy::prelude::*;
use atglyph_render::Glyph;
use bevy::prelude::*;

use crate::view::Row;

/// One thing lying under the player.
#[derive(Debug, Clone, PartialEq)]
pub struct GroundRow {
    /// What it is, by the name the game gave it and its count.
    pub row: Row,
    /// How many of it lie there: a stack's count, or one.
    pub count: u32,
    /// What one of it is called, as the game named it.
    pub name: String,
}

/// Everything lying on the player's cell, oldest first.
#[derive(Resource, Debug, Default)]
pub struct GroundView {
    /// What lies there.
    pub rows: Vec<GroundRow>,
}

/// Keeps [`GroundView`] current.
pub struct GroundViewPlugin;

impl Plugin for GroundViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GroundView>().add_systems(Update, collect_ground.in_set(crate::ViewSet::Collect));
    }

    fn finish(&self, app: &mut App) {
        app.depends_on::<crate::UiPlugin>("GroundViewPlugin");
    }
}

/// A thing lying somewhere, as the ground screen lists it.
type Lying = (Entity, &'static Position, Option<&'static OnMap>, Option<&'static Name>, Option<&'static Glyph>, Option<&'static Stack>);

/// What lies under the player, by entity order, which is the order things
/// were made in, so a pile reads the same from one frame to the next.
pub(crate) fn collect_ground(mut view: ResMut<GroundView>, map: Res<WorldMap>, player: Query<&Position, With<Player>>, items: Query<Lying, With<Item>>) {
    view.rows.clear();
    let Ok(at) = player.single() else { return };
    let here = map.current();
    let mut lying: Vec<_> = items.iter().filter(|(_, pos, on, ..)| pos.0 == at.0 && on.map_or(MapId::ZERO, |m| m.0) == here).collect();
    lying.sort_by_key(|(e, ..)| e.index());
    for (item, _, _, name, glyph, stack) in lying {
        let count = stack.map_or(1, |s| s.count);
        let name = name.map(|n| n.as_str().to_string()).unwrap_or_default();
        let label = atglyph_core::noun::listed(&name, count);
        view.rows.push(GroundRow { row: Row::new(item, label, glyph.copied().unwrap_or(Glyph::new('?', Color::WHITE))), count, name });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;

    #[test]
    fn the_view_lists_what_lies_under_the_player_and_nothing_beside() {
        let mut stage = Stage::new(GroundViewPlugin);
        let at = stage.at;
        let pile = stage.app.world_mut().spawn((Item, Name::new("pellet"), Stack { key: 1, count: 20 }, Position(at))).id();
        let kit = stage.app.world_mut().spawn((Item, Name::new("medkit"), Position(at))).id();
        stage.app.world_mut().spawn((Item, Name::new("bolt"), Position(at.offset(1, 0))));
        stage.tick();
        let view = stage.app.world().resource::<GroundView>();
        let rows: Vec<(Entity, &str, u32)> = view.rows.iter().map(|r| (r.row.entity, r.row.label.as_str(), r.count)).collect();
        assert_eq!(rows, vec![(pile, "20 pellets", 20), (kit, "medkit", 1)]);
    }
}
