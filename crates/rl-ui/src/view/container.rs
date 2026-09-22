//! What is inside the thing you just opened.
//!
//! The view holds what a container holds, as plain rows, and nothing
//! about whether a screen is up: which container is open is the screen's
//! own state, in [`OpenContainer`], set when the engine says one was
//! opened and cleared when the screen closes or the container goes out of
//! reach.
//!
//! Rows are the shared [`Row`], the way the nearby rail's and the gear
//! panel's are, so a game that annotates one annotates all of them the
//! same way.

use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_render::Glyph;

use crate::view::Row;

/// The container whose contents are being shown, if any.
///
/// The screen's own state rather than the engine's: a container is not
/// "open" in the world, it is being looked into by whoever opened it.
#[derive(Resource, Debug, Default)]
pub struct OpenContainer(pub Option<Entity>);

/// What the open container holds.
#[derive(Resource, Debug, Default)]
pub struct ContainerView {
    /// Which container, while one is open.
    pub entity: Option<Entity>,
    /// What it is called, from its `Name`.
    pub title: String,
    /// What is inside, in the order it went in.
    pub rows: Vec<Row>,
}

impl ContainerView {
    /// The row at `index`, for a menu cursor.
    pub fn row(&self, index: usize) -> Option<&Row> {
        self.rows.get(index)
    }

    /// Every row, for a game annotating what it recognises.
    pub fn rows_mut(&mut self) -> impl Iterator<Item = &mut Row> {
        self.rows.iter_mut()
    }
}

/// Keeps [`ContainerView`] current.
pub struct ContainerViewPlugin;

impl Plugin for ContainerViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ContainerView>().init_resource::<OpenContainer>().add_systems(Update, collect_container.in_set(crate::ViewSet::Collect));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::UiPlugin>(app, "ContainerViewPlugin");
    }
}

/// Fills [`ContainerView`] from whatever [`OpenContainer`] names.
///
/// A container that was emptied is still shown, empty, until the screen
/// is closed: what a player did is worth seeing, and a screen that
/// vanished as the last thing came out would read as a bug.
pub fn collect_container(
    mut view: ResMut<ContainerView>,
    open: Res<OpenContainer>,
    containers: Query<(Option<&Name>, &Inventory), With<Container>>,
    items: Query<(Option<&Name>, Option<&Glyph>, Option<&Stack>)>,
) {
    view.rows.clear();
    view.entity = None;
    view.title.clear();
    let Some(prop) = open.0 else { return };
    let Ok((name, contents)) = containers.get(prop) else { return };
    view.entity = Some(prop);
    view.title = name.map(|n| n.as_str().to_string()).unwrap_or_default();
    for item in &contents.items {
        let (name, glyph, stack) = items.get(*item).unwrap_or((None, None, None));
        let count = stack.map_or(1, |s| s.count);
        let label = name.map(|n| rl_core::noun::listed(n.as_str(), count)).unwrap_or_default();
        view.rows.push(Row::new(*item, label, glyph.copied().unwrap_or(Glyph::new('?', Color::WHITE))));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;

    /// The view is what the container holds, named and counted as the bag
    /// names and counts what it carries.
    #[test]
    fn the_view_lists_what_the_open_container_holds_and_nothing_when_none_is_open() {
        let mut stage = Stage::new(ContainerViewPlugin);
        let slugs = stage.app.world_mut().spawn((Item, Name::new("slug"), Stack { key: 1, count: 4 })).id();
        let kit = stage.app.world_mut().spawn((Item, Name::new("medkit"))).id();
        let crate_entity = stage.app.world_mut().spawn((Prop, Container, Name::new("supply crate"), Inventory { items: vec![slugs, kit] })).id();
        stage.tick();
        assert!(stage.app.world().resource::<ContainerView>().rows.is_empty(), "nothing is open, so nothing is listed");

        stage.app.world_mut().resource_mut::<OpenContainer>().0 = Some(crate_entity);
        stage.tick();
        let view = stage.app.world().resource::<ContainerView>();
        assert_eq!(view.title, "supply crate");
        assert_eq!(view.rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), vec!["4 slugs", "medkit"], "counted as the bag counts");
    }
}
