//! The overworld screen: the world graph drawn as a map, with a portal
//! picker over discovered sites.
//!
//! Opt-in. It reads the [`WorldRes`] and [`Knowledge`] and writes a
//! [`PortalRequest`]; nothing else depends on it, so a game can drop it
//! without touching anything else. The overworld never owns the player's
//! position: the marker is derived from the tile position, and choosing a
//! site asks the portal mechanic to move the player.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_core::{Point, Rect};
use rl_render::{Cell, Terminal};
use rl_ui::{ModalId, Modals};
use rl_world::BandId;

/// The name the overworld's modal is declared under.
pub const OVERWORLD_MODAL: &str = "overworld";

/// Where the cursor is on the discovered-site list.
///
/// Whether the screen is open is not here: it is on the shared
/// [`Modals`] stack, so this screen and a game's own cannot both think
/// they own the arrow keys.
#[derive(Resource, Debug, Default)]
pub struct OverworldScreen {
    /// Index into the discovered-site list the cursor is on.
    pub selected: usize,
}

/// What each band looks like on the overworld, indexed by [`BandId`].
#[derive(Resource, Debug, Clone, Default)]
pub struct BandAppearance {
    cells: Vec<Cell>,
}

impl BandAppearance {
    /// An empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the appearance of `band`.
    pub fn set(&mut self, band: BandId, cell: Cell) {
        let i = band.0 as usize;
        if self.cells.len() <= i {
            self.cells.resize(i + 1, Cell::default());
        }
        self.cells[i] = cell;
    }

    /// The appearance of `band`.
    pub fn get(&self, band: BandId) -> Cell {
        self.cells.get(band.0 as usize).copied().unwrap_or(Cell::new('?', Color::srgb(1.0, 0.0, 1.0)))
    }
}

/// Colours for the overlays the screen draws itself.
#[derive(Resource, Debug, Clone, Copy)]
pub struct OverworldStyle {
    /// Rivers.
    pub river: Color,
    /// Roads.
    pub road: Color,
    /// A discovered site.
    pub site: Color,
    /// The selected site.
    pub selected: Color,
    /// The player's marker.
    pub player: Color,
    /// Regions never seen.
    pub unknown: Color,
    /// Whether unexplored regions are drawn at all.
    pub show_unexplored: bool,
}

impl Default for OverworldStyle {
    fn default() -> Self {
        Self {
            river: Color::srgb(0.3, 0.6, 1.0),
            road: Color::srgb(0.8, 0.7, 0.5),
            site: Color::srgb(1.0, 0.9, 0.4),
            selected: Color::srgb(1.0, 1.0, 1.0),
            player: Color::srgb(1.0, 1.0, 1.0),
            unknown: Color::srgb(0.12, 0.12, 0.14),
            show_unexplored: true,
        }
    }
}

/// Where the overworld is drawn.
#[derive(Resource, Debug, Clone, Copy)]
pub struct OverworldLayout {
    /// The terminal cells the map occupies.
    pub viewport: Rect,
}

/// A request to move the player to a discovered site, for the game's
/// portal mechanic to honour.
#[derive(Message, Debug, Clone, Copy)]
pub struct PortalRequest {
    /// Index into the world's sites.
    pub site: usize,
}

/// Keys the screen listens to.
#[derive(Resource, Debug, Clone, Copy)]
pub struct OverworldKeys {
    /// Toggles the screen.
    pub toggle: KeyCode,
    /// Closes the screen.
    pub close: KeyCode,
    /// Moves the selection back.
    pub prev: KeyCode,
    /// Moves the selection forward.
    pub next: KeyCode,
    /// Portals to the selected site.
    pub go: KeyCode,
}

impl Default for OverworldKeys {
    fn default() -> Self {
        Self { toggle: KeyCode::KeyM, close: KeyCode::Escape, prev: KeyCode::ArrowLeft, next: KeyCode::ArrowRight, go: KeyCode::Enter }
    }
}

/// Adds the overworld screen.
pub struct OverworldPlugin;

impl Plugin for OverworldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Modals>().world_mut().resource_mut::<Modals>().declare(OVERWORLD_MODAL);
        app.needs::<OverworldLayout>("OverworldPlugin", "`OverworldLayout { viewport }`, the terminal cells the overworld is drawn in")
            .init_resource::<OverworldScreen>()
            .init_resource::<BandAppearance>()
            .init_resource::<OverworldStyle>()
            .init_resource::<OverworldKeys>()
            .add_message::<PortalRequest>()
            .add_systems(Update, handle_keys.in_set(EngineSet::Input))
            .add_systems(Update, draw_overworld.in_set(PresentSet::Overlay));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<rl_bevy::CorePlugin>(app, "OverworldPlugin");
        rl_bevy::depends_on::<rl_ui::UiPlugin>(app, "OverworldPlugin");
    }
}

/// The id of the overworld's modal, for a game gating its own systems.
///
/// # Panics
/// Panics if [`OverworldPlugin`] was not added.
pub fn overworld_modal(modals: &Modals) -> ModalId {
    modals.get(OVERWORLD_MODAL).expect("OverworldPlugin declares the overworld modal")
}

/// Whether the overworld screen is open, for gating game input.
pub fn overworld_open(modals: Res<Modals>) -> bool {
    modals.is_open(overworld_modal(&modals))
}

/// Opens and closes the screen, moves the cursor over discovered sites,
/// and asks for a portal to the one chosen.
pub fn handle_keys(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<OverworldKeys>,
    mut screen: ResMut<OverworldScreen>,
    mut modals: ResMut<Modals>,
    knowledge: Res<Knowledge>,
    mut portals: MessageWriter<PortalRequest>,
) {
    let modal = overworld_modal(&modals);
    if keys.just_pressed(binds.toggle) && (modals.is_top(modal) || !modals.any_open()) {
        modals.toggle(modal);
        return;
    }
    if !modals.is_top(modal) {
        return;
    }
    if keys.just_pressed(binds.close) {
        modals.close_one(modal);
        return;
    }
    let discovered: Vec<usize> = knowledge.discovered_sites().collect();
    if discovered.is_empty() {
        return;
    }
    if keys.just_pressed(binds.next) {
        screen.selected = (screen.selected + 1) % discovered.len();
    }
    if keys.just_pressed(binds.prev) {
        screen.selected = (screen.selected + discovered.len() - 1) % discovered.len();
    }
    if keys.just_pressed(binds.go) {
        let site = discovered[screen.selected.min(discovered.len() - 1)];
        portals.write(PortalRequest { site });
        modals.close_one(modal);
    }
}

/// The two tables the overworld is drawn from.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Look<'w> {
    bands: Res<'w, BandAppearance>,
    style: Res<'w, OverworldStyle>,
}

/// Where the player is and what it knows.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Whereabouts<'w, 's> {
    world: Res<'w, WorldRes>,
    map: Res<'w, WorldMap>,
    knowledge: Res<'w, Knowledge>,
    player: Query<'w, 's, &'static Position, With<Player>>,
}

/// Draws the world's bands, rivers, roads, discovered sites and the
/// player's own region, over whatever the map view drew.
pub fn draw_overworld(
    screen: Res<OverworldScreen>,
    modals: Res<Modals>,
    layout: Option<Res<OverworldLayout>>,
    whereabouts: Whereabouts,
    look: Look,
    mut terminal: ResMut<Terminal>,
) {
    let style = *look.style;
    let look = &look.bands;
    if !modals.is_open(overworld_modal(&modals)) {
        return;
    }
    let Whereabouts { world, map, knowledge, player } = whereabouts;
    let Some(layout) = layout else { return };
    let vp = layout.viewport;
    // Below the surface the player's tile position means nothing here.
    let player_region = if map.current().is_surface() { player.single().ok().map(|p| world.region_of_tile(p.0)) } else { None };
    // Centre the view on the player's region when the world is bigger than
    // the viewport.
    let origin = player_region
        .map(|r| {
            Point::new((r.x - vp.width / 2).clamp(0, (world.width() - vp.width).max(0)), (r.y - vp.height / 2).clamp(0, (world.height() - vp.height).max(0)))
        })
        .unwrap_or(Point::ZERO);
    let layers = world.layers();
    let discovered: Vec<usize> = knowledge.discovered_sites().collect();
    let selected = discovered.get(screen.selected.min(discovered.len().saturating_sub(1))).copied();
    for s in vp.cells() {
        let region = s - vp.origin() + origin;
        let Some(band) = layers.band(region) else {
            terminal.set(s.x, s.y, Cell::default());
            continue;
        };
        let seen = knowledge.region_touched(region);
        let mut cell = if seen || style.show_unexplored { look.get(band) } else { Cell::new(' ', style.unknown).on(style.unknown) };
        if !seen && style.show_unexplored {
            cell = cell.dimmed(0.5);
        }
        if layers.hydrology.is_river(region) {
            cell = Cell::new('~', style.river).on(cell.bg);
        }
        if !world.roads().at(region).is_empty() {
            cell = Cell::new('+', style.road).on(cell.bg);
        }
        if let Some(site) = world.site_index_at(region)
            && knowledge.site_discovered(site)
        {
            let fg = if Some(site) == selected { style.selected } else { style.site };
            cell = Cell::new('O', fg).on(cell.bg);
        }
        if Some(region) == player_region {
            cell = Cell::new('@', style.player).on(cell.bg);
        }
        terminal.set(s.x, s.y, cell);
    }
}

/// The names most callers want in scope.
pub mod prelude {
    pub use crate::{
        BandAppearance, OverworldKeys, OverworldLayout, OverworldPlugin, OverworldScreen, OverworldStyle, PortalRequest, overworld_modal, overworld_open,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use rl_bevy::plugin::headless_app;
    use rl_core::RunSeed;
    use rl_grid::TileRegistry;
    use rl_world::{BandId, CellFacts, Layers, Site, SiteKindId, WorldConfig, WorldGraph, WorldRules};

    /// Land everywhere but the sea, with one town to discover.
    struct Flat;
    impl WorldRules for Flat {
        fn classify(&self, f: &CellFacts) -> BandId {
            BandId(if f.is_sea { 0 } else { 1 })
        }
        fn road_friction(&self, _: BandId, _: &CellFacts) -> Option<f32> {
            None
        }
        fn settlements(&self, layers: &Layers, _: u64) -> Vec<Site> {
            layers.bands.iter().find(|(_, b)| b.0 == 1).map(|(p, _)| vec![Site { kind: SiteKindId(1), position: p }]).unwrap_or_default()
        }
    }

    fn app() -> App {
        let mut app = headless_app();
        app.add_plugins(rl_bevy::FovPlugin);
        app.add_plugins((bevy::input::InputPlugin, rl_render::MapViewPlugin, rl_ui::UiPlugin, OverworldPlugin))
            .init_resource::<Script>()
            .add_systems(PreUpdate, play_script.after(bevy::input::InputSystems));
        let tiles = TileRegistry::standard();
        let world = WorldGraph::generate(RunSeed(5), WorldConfig { region_size: 64, ..WorldConfig::regions(64, 64) }, &Flat);
        let (region, _) = world.layers().bands.iter().find(|(_, b)| b.0 == 1).expect("land");
        let start = world.tile_origin(region).offset(8, 8);
        let mut bands = BandAppearance::new();
        bands.set(BandId(0), Cell::new('~', Color::srgb(0.2, 0.3, 0.8)));
        bands.set(BandId(1), Cell::new('.', Color::srgb(0.3, 0.7, 0.3)));
        app.insert_resource(WorldMap::new(tiles.tables()))
            .insert_resource(WorldRes(world))
            .insert_resource(Terminal::new(100, 40, Vec2::ONE))
            .insert_resource(OverworldLayout { viewport: Rect::new(0, 1, 100, 35) })
            .insert_resource(rl_render::MapView::new(Rect::new(0, 1, 100, 35)))
            .insert_resource(bands);
        app.world_mut().spawn((Player, Position(start)));
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app
    }

    /// Keys to play, the way a real keyboard delivers them: pressed in
    /// `PreUpdate` after the input systems, which clear `just_pressed`
    /// every frame, and released on the frame after.
    #[derive(Resource, Default)]
    struct Script {
        next: Vec<KeyCode>,
        held: Vec<KeyCode>,
    }

    fn play_script(mut script: ResMut<Script>, mut keys: ResMut<ButtonInput<KeyCode>>) {
        for k in std::mem::take(&mut script.held) {
            keys.release(k);
        }
        let next = std::mem::take(&mut script.next);
        for k in &next {
            keys.press(*k);
        }
        script.held = next;
    }

    fn press(app: &mut App, key: KeyCode) {
        app.world_mut().resource_mut::<Script>().next.push(key);
        app.update();
    }

    #[test]
    fn the_map_key_opens_the_screen_and_draws_the_world_over_the_map_view() {
        let mut app = app();
        let is_open = |app: &App| {
            let modals = app.world().resource::<Modals>();
            modals.is_open(overworld_modal(modals))
        };
        assert!(!is_open(&app));

        press(&mut app, OverworldKeys::default().toggle);
        assert!(is_open(&app), "the map key opens the screen");

        let drawn: Vec<char> = {
            let t = app.world().resource::<Terminal>();
            (0..100).map(|x| t.get(x, 10).expect("in bounds").glyph).collect()
        };
        assert!(drawn.iter().any(|c| *c == '.' || *c == '~'), "the world's bands are drawn: {drawn:?}");

        press(&mut app, OverworldKeys::default().close);
        assert!(!is_open(&app), "escape closes it");
    }
}
