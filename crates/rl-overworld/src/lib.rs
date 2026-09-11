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
use rl_world::BandId;

/// Whether the overworld screen is open.
#[derive(Resource, Debug, Default)]
pub struct OverworldScreen {
    /// Open or not.
    pub open: bool,
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
        app.init_resource::<OverworldScreen>()
            .init_resource::<BandAppearance>()
            .init_resource::<OverworldStyle>()
            .init_resource::<OverworldKeys>()
            .add_message::<PortalRequest>()
            .add_systems(Update, handle_keys.in_set(EngineSet::Input))
            .add_systems(Update, draw_overworld.in_set(EngineSet::Present).after(rl_render::map_view::draw_map));
    }
}

/// Whether the overworld screen is open, for gating game input.
pub fn overworld_open(screen: Res<OverworldScreen>) -> bool {
    screen.open
}

fn handle_keys(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<OverworldKeys>,
    mut screen: ResMut<OverworldScreen>,
    knowledge: Res<Knowledge>,
    mut portals: MessageWriter<PortalRequest>,
) {
    if keys.just_pressed(binds.toggle) {
        screen.open = !screen.open;
        return;
    }
    if !screen.open {
        return;
    }
    if keys.just_pressed(binds.close) {
        screen.open = false;
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
        screen.open = false;
    }
}

/// The two tables the overworld is drawn from.
#[derive(bevy::ecs::system::SystemParam)]
struct Look<'w> {
    bands: Res<'w, BandAppearance>,
    style: Res<'w, OverworldStyle>,
}

fn draw_overworld(
    screen: Res<OverworldScreen>,
    layout: Option<Res<OverworldLayout>>,
    world: Res<WorldRes>,
    knowledge: Res<Knowledge>,
    look: Look,
    mut terminal: ResMut<Terminal>,
    player: Query<&Position, With<Player>>,
) {
    let style = *look.style;
    let look = &look.bands;
    if !screen.open {
        return;
    }
    let Some(layout) = layout else { return };
    let vp = layout.viewport;
    let player_region = player.single().ok().map(|p| world.region_of_tile(p.0));
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
