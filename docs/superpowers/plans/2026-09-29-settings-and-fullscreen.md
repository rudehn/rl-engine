# Settings and Fullscreen Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The terminal is drawn sharp at any window size, and the engine has a settings registry, a settings screen and an opt-in plugin that remembers them, with Fullscreen as the one row the engine declares.

**Architecture:** `rl-render` lays the cell grid out again whenever its window changes, from one pure function, and stops stretching the camera.
A plain-data `Settings` registry lives in `rl-bevy`, the crate `rl-render`, `rl-ui` and `rl-save` all already depend on: `rl-render` declares and applies Fullscreen, `rl-ui` draws and edits the rows, `rl-save` remembers them.
No new dependency edge between crates.

**Tech Stack:** Rust, Bevy 0.19 (`bevy_window`, `bevy_sprite_render`, `bevy_text`), `ron`, `serde`.

**Spec:** `docs/superpowers/specs/2026-09-29-settings-and-fullscreen-design.md`

## Global Constraints

- Work in the worktree `.claude/worktrees/settings` on branch `settings`; never `cd` to the main checkout.
- `CARGO_TARGET_DIR` stays unset; before the first build run `df -h /` and stop if under 60 GB free.
- Scoped runs while iterating: `cargo test -p <crate> <filter>`. The whole workspace once, in Task 7.
- Each commit runs the fast gate first: `cargo fmt --all --check`, `cargo clippy -p <crates touched> --all-targets -- -D warnings`, and the tests of what it touched.
- Each commit adds its line to `CHANGELOG.md` under `Unreleased` when a game author would see the change.
- Commit messages are one lowercase sentence in the repo's style, with no co-author line.
- `#![deny(missing_docs)]`: every public item gets a doc comment that says why, at the density of `crates/rl-core/src/turn.rs`.
- No `TODO` comments, no em dash anywhere, no `HashMap` or `HashSet`, no `#[non_exhaustive]`, no theme words in engine crates.
- `rustfmt.toml` pins the width; do not hand-wrap.
- A test's name reads as a sentence describing the property.
- In Markdown, one sentence per line.
- Never order a system after another crate's system function; order against sets.
- The native-size picture must not change: at a window exactly `cols * cell` by `rows * cell`, the layout is the declared cell, the declared font and no margin.
- `rl-ui` must not gain a dependency on `rl-save`.

## Review Focus

1. A minimized or zero-sized window: the layout must not panic or divide by zero, and must recover when the window returns. Pinned in Task 1.
2. The window moving to a display with another scale factor at the same physical size: the grid must be laid out again. Pinned in Task 1.
3. The key that opens the settings screen read again by the screen in the same frame, so Enter on the menu row also changes the first setting. Pinned in Task 4.
4. A setting's key pressed while another screen, such as the bag, is on top: nothing changes. Pinned in Task 4.
5. A remembered choice that no longer exists after an upgrade, or a file that is not RON at all: defaults stand, nothing panics, and the file is not rewritten on load. Pinned in Task 5.

---

## File structure

| File | Responsibility |
| --- | --- |
| `crates/rl-render/src/layout.rs` (new) | `Fit` and `fit`: the pure arithmetic of where cells go. No Bevy systems. |
| `crates/rl-render/src/terminal.rs` (modify) | Spawns cells, and the `relayout` system that applies a `Fit` when the window changes. |
| `crates/rl-bevy/src/settings.rs` (new) | `Setting`, `SettingId`, `Settings`, `AddSettings`. Plain data. |
| `crates/rl-render/src/fullscreen.rs` (new) | `FullscreenPlugin`: declares the row and drives the window's mode. |
| `crates/rl-ui/src/panel/settings.rs` (new) | `SettingsPanel`: the modal, its keys, its drawing, and keyed settings as controls. |
| `crates/rl-ui/src/game_menu.rs` (modify) | The `Settings` row. |
| `crates/rl-save/src/settings.rs` (new) | `SettingsSavePlugin`: load in `finish`, write on change. |
| `crates/rl-engine/src/lib.rs` (modify) | `RoguelikePlugins` adds `FullscreenPlugin`. |
| `examples/foundry/src/{main.rs,title.rs}` (modify) | Wiring and the title screen's row. |

---

### Task 1: The layout, and a terminal that follows its window

**Files:**
- Create: `crates/rl-render/src/layout.rs`
- Modify: `crates/rl-render/src/lib.rs` (add `pub mod layout;` and `pub use layout::{Fit, fit};`)
- Modify: `crates/rl-render/src/terminal.rs` (`TerminalPlugin::build`, `spawn_grid`, new `relayout`)
- Modify: `CHANGELOG.md`

**Interfaces:**
- Produces: `rl_render::layout::fit(cols: i32, rows: i32, base_cell: Vec2, window: UVec2, scale_factor: f32) -> Fit`
- Produces: `Fit { pub cell: UVec2, pub margin: UVec2 }`, both in physical pixels, with `Fit::cell_logical(&self, scale_factor: f32) -> Vec2`, `Fit::center(&self, x: i32, y: i32, window: UVec2, scale_factor: f32) -> Vec2` (world position of a cell's centre, camera at the origin, y up) and `Fit::font(&self, base_font: f32, base_cell: Vec2, scale_factor: f32) -> f32`.

- [ ] **Step 1: Write the failing tests for `fit`**

Create `crates/rl-render/src/layout.rs` with the module doc, the types with `unimplemented!()` bodies, and these tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const BASE: Vec2 = Vec2::new(10.0, 16.0);

    /// Window sizes from far smaller than the grid to far larger, at the
    /// scale factors displays come in.
    fn cases() -> impl Iterator<Item = (i32, i32, UVec2, f32)> {
        let grids = [(80, 40), (138, 55), (40, 12)];
        let factors = [1.0_f32, 1.25, 1.5, 2.0, 3.0];
        grids.into_iter().flat_map(move |(cols, rows)| {
            factors.into_iter().flat_map(move |sf| (0..60).map(move |i| (cols, rows, UVec2::new(97 + i * 61, 53 + i * 37), sf)))
        })
    }

    #[test]
    fn the_grid_always_fits_inside_the_window_once_it_can_hold_a_pixel_per_cell() {
        for (cols, rows, window, sf) in cases() {
            let f = fit(cols, rows, BASE, window, sf);
            if window.x >= cols as u32 && window.y >= rows as u32 {
                assert!(f.margin.x + f.cell.x * cols as u32 <= window.x, "{cols}x{rows} {window} {sf}: {f:?}");
                assert!(f.margin.y + f.cell.y * rows as u32 <= window.y, "{cols}x{rows} {window} {sf}: {f:?}");
            }
        }
    }

    #[test]
    fn the_grid_is_centred_to_within_one_pixel() {
        for (cols, rows, window, sf) in cases() {
            let f = fit(cols, rows, BASE, window, sf);
            let (used_x, used_y) = (f.cell.x * cols as u32, f.cell.y * rows as u32);
            if used_x <= window.x && used_y <= window.y {
                assert!((window.x - used_x) - 2 * f.margin.x <= 1, "{window} {sf}: {f:?}");
                assert!((window.y - used_y) - 2 * f.margin.y <= 1, "{window} {sf}: {f:?}");
            }
        }
    }

    #[test]
    fn a_cell_is_never_less_than_one_pixel_even_in_a_window_of_none() {
        let f = fit(80, 40, BASE, UVec2::ZERO, 2.0);
        assert_eq!(f.cell, UVec2::ONE);
        assert_eq!(f.margin, UVec2::ZERO);
    }

    #[test]
    fn the_native_window_reproduces_the_declared_cell_with_no_margin() {
        // The factors at which a cell of ten by sixteen is a whole number of
        // pixels; at 1.25 the display itself cannot show the declared cell.
        for sf in [1.0_f32, 1.5, 2.0, 3.0] {
            for (cols, rows) in [(80, 40), (138, 55), (100, 40)] {
                let window = UVec2::new((cols as f32 * BASE.x * sf) as u32, (rows as f32 * BASE.y * sf) as u32);
                let f = fit(cols, rows, BASE, window, sf);
                assert_eq!(f.cell, UVec2::new((BASE.x * sf) as u32, (BASE.y * sf) as u32), "{cols}x{rows} at {sf}");
                assert_eq!(f.margin, UVec2::ZERO, "{cols}x{rows} at {sf}");
                assert_eq!(f.font(14.0, BASE, sf), 14.0);
                assert_eq!(f.cell_logical(sf), BASE);
            }
        }
    }

    #[test]
    fn a_larger_window_never_gives_a_smaller_cell() {
        for sf in [1.0_f32, 2.0] {
            let mut last = UVec2::ZERO;
            for i in 0..200 {
                let f = fit(80, 40, BASE, UVec2::new(200 + i * 16, 160 + i * 13), sf);
                assert!(f.cell.x >= last.x && f.cell.y >= last.y, "step {i} at {sf}");
                last = f.cell;
            }
        }
    }

    #[test]
    fn the_first_cell_is_centred_where_it_always_was_at_the_native_size() {
        // Eight by four cells of ten by twenty: the same numbers
        // `terminal::tests` pins for `cell_center`.
        let (base, window) = (Vec2::new(10.0, 20.0), UVec2::new(80, 80));
        let f = fit(8, 4, base, window, 1.0);
        assert_eq!(f.center(0, 0, window, 1.0), Vec2::new(-35.0, 30.0));
        assert_eq!(f.center(7, 3, window, 1.0), Vec2::new(35.0, -30.0));
    }
}
```

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p rl-render layout`
Expected: every test panics with `not implemented`.

- [ ] **Step 3: Implement `fit`**

```rust
//! Where the cells go in a window of any size.
//!
//! The grid was once stretched to fit by the camera, which stretches a
//! picture drawn for another size: glyphs rasterized at fourteen pixels
//! and shown at twenty have stems of uneven width. So the grid is laid out
//! for the window it is in instead, and the glyphs are drawn at the size
//! they are shown.
//!
//! Pure arithmetic, with no `App`, so every property of it is a test over
//! a range of windows rather than something to look at.

use bevy::math::{UVec2, Vec2};

/// How a grid sits in a window, in physical pixels.
///
/// Physical, not logical, because the point is that every cell edge lies
/// on the display's own pixel grid: a cell of a fractional width leaves a
/// seam between two backgrounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fit {
    /// One cell's width and height.
    pub cell: UVec2,
    /// The blank border to the left of and above the grid.
    pub margin: UVec2,
}

/// What a float's rounding may cost before a floor, in pixels. A window of
/// exactly the native size must give exactly the declared cell, and
/// `19.999998` floors to nineteen. Far smaller than one pixel spread over
/// any grid a terminal has, so it never makes the grid overflow.
const ROUNDING: f32 = 1e-3;

/// The largest whole-pixel cells that fit `cols` by `rows` of them in
/// `window`, keeping `base_cell`'s shape as nearly as whole pixels allow,
/// and the margin that centres them.
///
/// Width and height are floored separately, so a cell's shape drifts from
/// the declared one by at most a pixel. A cell is never under one pixel,
/// which is what a window of no size at all, a minimized one, gets.
pub fn fit(cols: i32, rows: i32, base_cell: Vec2, window: UVec2, scale_factor: f32) -> Fit {
    let (cols, rows) = (cols.max(1) as f32, rows.max(1) as f32);
    let native = Vec2::new(cols * base_cell.x, rows * base_cell.y) * scale_factor;
    let zoom = (window.x as f32 / native.x).min(window.y as f32 / native.y);
    let cell = (base_cell * scale_factor * zoom + ROUNDING).floor().max(Vec2::ONE).as_uvec2();
    let used = UVec2::new(cell.x * cols as u32, cell.y * rows as u32);
    Fit { cell, margin: window.saturating_sub(used) / 2 }
}

impl Fit {
    /// A cell's size in logical pixels, the unit sprites are sized in.
    pub fn cell_logical(&self, scale_factor: f32) -> Vec2 {
        self.cell.as_vec2() / scale_factor
    }

    /// The centre of cell `(x, y)` in world space: logical pixels, the
    /// window's centre at the origin, row 0 at the top.
    pub fn center(&self, x: i32, y: i32, window: UVec2, scale_factor: f32) -> Vec2 {
        let half = window.as_vec2() * 0.5;
        let cell = self.cell.as_vec2();
        let px = Vec2::new(-half.x + self.margin.x as f32 + (x as f32 + 0.5) * cell.x, half.y - self.margin.y as f32 - (y as f32 + 0.5) * cell.y);
        px / scale_factor
    }

    /// The font size for these cells, in logical pixels: the declared one
    /// scaled as the cell's height was.
    pub fn font(&self, base_font: f32, base_cell: Vec2, scale_factor: f32) -> f32 {
        base_font * self.cell.y as f32 / (base_cell.y * scale_factor)
    }
}
```

- [ ] **Step 4: Run the tests and see them pass**

Run: `cargo test -p rl-render layout`
Expected: PASS.

- [ ] **Step 5: Write the failing tests for `relayout`**

In `crates/rl-render/src/terminal.rs`'s test module:

```rust
    use bevy::window::{PrimaryWindow, WindowResolution};

    /// A headless app with a terminal of eight by four cells of ten by
    /// twenty, and a primary window of `width` by `height` physical pixels.
    fn windowed(width: u32, height: u32, scale: f32) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(TerminalPlugin { width: 8, height: 4, cell_size: Vec2::new(10.0, 20.0), font_size: 16.0 });
        let mut resolution = WindowResolution::new(width, height);
        resolution.set_scale_factor_override(Some(scale));
        resolution.set_physical_resolution(width, height);
        let window = app.world_mut().spawn((Window { resolution, ..default() }, PrimaryWindow)).id();
        app.update();
        (app, window)
    }

    fn first_cell(app: &mut App) -> (Vec2, Vec2, f32) {
        let entities = app.world().resource::<CellEntities>();
        let (background, glyph) = (entities.background[0], entities.glyph[0]);
        let size = app.world().get::<Sprite>(background).unwrap().custom_size.unwrap();
        let at = app.world().get::<Transform>(background).unwrap().translation.truncate();
        let FontSize::Px(font) = app.world().get::<TextFont>(glyph).unwrap().font_size else { panic!("a pixel font size") };
        (size, at, font)
    }

    #[test]
    fn a_window_of_the_native_size_leaves_the_cells_as_declared() {
        let (mut app, _) = windowed(80, 80, 1.0);
        assert_eq!(first_cell(&mut app), (Vec2::new(10.0, 20.0), Vec2::new(-35.0, 30.0), 16.0));
    }

    #[test]
    fn a_window_twice_the_size_doubles_the_cells_and_the_font() {
        let (mut app, _) = windowed(160, 160, 1.0);
        assert_eq!(first_cell(&mut app), (Vec2::new(20.0, 40.0), Vec2::new(-70.0, 60.0), 32.0));
    }

    #[test]
    fn resizing_the_window_lays_the_grid_out_again() {
        let (mut app, window) = windowed(80, 80, 1.0);
        app.world_mut().get_mut::<Window>(window).unwrap().resolution.set_physical_resolution(240, 240);
        app.update();
        assert_eq!(first_cell(&mut app).0, Vec2::new(30.0, 60.0));
    }

    #[test]
    fn a_change_of_scale_factor_alone_lays_the_grid_out_again() {
        let (mut app, window) = windowed(160, 160, 1.0);
        app.world_mut().get_mut::<Window>(window).unwrap().resolution.set_scale_factor_override(Some(2.0));
        app.update();
        // The same physical pixels, half as many logical ones.
        assert_eq!(first_cell(&mut app).0, Vec2::new(10.0, 20.0));
    }

    #[test]
    fn a_window_of_no_size_is_left_alone_and_recovers() {
        let (mut app, window) = windowed(160, 160, 1.0);
        app.world_mut().get_mut::<Window>(window).unwrap().resolution.set_physical_resolution(0, 0);
        app.update();
        assert_eq!(first_cell(&mut app).0, Vec2::new(20.0, 40.0), "the last layout stands while minimized");
        app.world_mut().get_mut::<Window>(window).unwrap().resolution.set_physical_resolution(80, 80);
        app.update();
        assert_eq!(first_cell(&mut app).0, Vec2::new(10.0, 20.0));
    }

    #[test]
    fn with_no_window_the_grid_is_spawned_as_declared_and_nothing_fails() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(TerminalPlugin { width: 8, height: 4, cell_size: Vec2::new(10.0, 20.0), font_size: 16.0 });
        app.update();
        app.update();
        assert_eq!(first_cell(&mut app), (Vec2::new(10.0, 20.0), Vec2::new(-35.0, 30.0), 16.0));
    }
```

`WindowResolution`'s setters are named as in Bevy 0.19; if a name differs, read `bevy_window::WindowResolution` in `~/.cargo/registry` and use the one that sets the physical size and the scale-factor override.
If spawning `Text2d` under `MinimalPlugins` needs a resource the text plugin provides, add only the plugin that provides it to `windowed`, not `DefaultPlugins`.

- [ ] **Step 6: Run them and see them fail**

Run: `cargo test -p rl-render terminal`
Expected: the doubling, resizing and scale-factor tests fail on the cell size; the native ones may pass already.

- [ ] **Step 7: Implement `relayout` and stop the camera stretching**

In `spawn_grid`, spawn the camera with no projection override, so one world unit is one logical pixel:

```rust
    commands.spawn((Camera2d, Camera { clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() }));
```

Add the system and register it in `TerminalPlugin::build` with `.add_systems(PostUpdate, relayout)`:

```rust
/// The window the grid was last laid out for: its physical size and its
/// scale factor's bits, compared exactly.
#[derive(Default)]
struct LaidOutFor(Option<(UVec2, u32)>);

/// Lays the grid out again when its window is another size or on another
/// display.
///
/// Sizes and positions come from [`fit`], and the glyphs take a font size
/// scaled with the cell, so Bevy rasterizes them at the size they are
/// shown rather than stretching what it drew for the declared one. Once
/// per change, not per frame.
///
/// With no window, as in a headless test, or a window of no size, as when
/// minimized, the last layout stands.
fn relayout(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    terminal: Res<Terminal>,
    font: Res<TerminalFont>,
    entities: Res<CellEntities>,
    mut laid: Local<LaidOutFor>,
    mut backgrounds: Query<(&mut Sprite, &mut Transform), Without<Text2d>>,
    mut glyphs: Query<(&mut TextFont, &mut Transform), With<Text2d>>,
) {
    let Ok(window) = windows.single() else { return };
    let size = UVec2::new(window.physical_width(), window.physical_height());
    let scale = window.scale_factor();
    if size.x == 0 || size.y == 0 || entities.background.is_empty() || laid.0 == Some((size, scale.to_bits())) {
        return;
    }
    laid.0 = Some((size, scale.to_bits()));
    let fit = fit(terminal.width, terminal.height, terminal.cell_size, size, scale);
    let (cell, font_size) = (fit.cell_logical(scale), fit.font(font.size, terminal.cell_size, scale));
    for y in 0..terminal.height {
        for x in 0..terminal.width {
            let i = (y * terminal.width + x) as usize;
            let c = fit.center(x, y, size, scale);
            if let Ok((mut sprite, mut transform)) = backgrounds.get_mut(entities.background[i]) {
                sprite.custom_size = Some(cell);
                transform.translation = c.extend(BACKGROUND_Z);
            }
            if let Ok((mut text, mut transform)) = glyphs.get_mut(entities.glyph[i]) {
                text.font_size = FontSize::Px(font_size);
                transform.translation = c.extend(GLYPH_Z);
            }
        }
    }
}
```

Add `use crate::layout::fit;`.
`spawn_grid` keeps placing cells with `Terminal::cell_center`, so a headless grid is the declared one.
Update `TerminalPlugin`'s doc comment: "Sets up the terminal grid and keeps it filling its window", with one sentence on why the camera does not scale.

- [ ] **Step 8: Run the tests and see them pass**

Run: `cargo test -p rl-render`
Expected: PASS, including the six new terminal tests.

- [ ] **Step 9: See it in the running game**

Take a native-size capture before and after, and compare:

```bash
RL_CAPTURE=/tmp/claude-settings-after.png cargo run -p foundry
(cd /Users/nathanrude/Development/rl-engine && RL_CAPTURE=/tmp/claude-settings-before.png ./target/debug/foundry)
cmp /tmp/claude-settings-before.png /tmp/claude-settings-after.png && echo identical
```

Expected: `identical`.
If `cmp` differs, open both with the Read tool; a difference of anything but the title screen's animation frame is a bug in `fit` at the native size and is fixed before going on.
The title screen animates, so if the two differ only there, capture a frame in a run instead using `RL_KEYS` as `crates/rl-render/src/capture.rs` documents.

- [ ] **Step 10: Gate and commit**

Add to `CHANGELOG.md` under `Unreleased`: the terminal fills its window at any size with glyphs drawn at the size shown; nothing changes at the native size; `rl_render::layout::fit` is public.

```bash
cargo fmt --all --check && cargo clippy -p rl-render --all-targets -- -D warnings && cargo test -p rl-render
git add crates/rl-render CHANGELOG.md
git commit -m "the terminal is laid out for the window it is in, so glyphs are drawn at the size they are shown and stay sharp maximized or fullscreen"
```

---

### Task 2: The settings registry

**Files:**
- Create: `crates/rl-bevy/src/settings.rs`
- Modify: `crates/rl-bevy/src/lib.rs` (`pub mod settings;`, a `pub use settings::{AddSettings, Setting, SettingId, Settings};` beside the others, and the same four names in `prelude`)
- Modify: `CHANGELOG.md`

**Interfaces:**
- Produces:
  - `Setting::new(name: impl Into<String>, group: impl Into<String>, label: impl Into<String>, choices: impl IntoIterator<Item = impl Into<String>>) -> Setting`, `.default_choice(index: usize) -> Setting`, `.key(key: KeyCode) -> Setting`; public fields `name`, `group`, `label`, `choices: Vec<String>`, `default: usize`, `key: Option<KeyCode>`.
  - `SettingId` (`Copy`, `Eq`, `Ord`, `Debug`).
  - `Settings` (`Resource`, `Default`): `add(&mut self, Setting) -> SettingId`, `get(&self, SettingId) -> &Setting`, `find(&self, name: &str) -> Option<SettingId>`, `chosen(&self, SettingId) -> usize`, `choice(&self, SettingId) -> &str`, `choose(&mut self, SettingId, usize)`, `cycle(&mut self, SettingId, step: i32)`, `iter(&self) -> impl Iterator<Item = (SettingId, &Setting, usize)>`, `is_empty(&self) -> bool`, `remembered(&self) -> BTreeMap<String, String>`, `recall(&mut self, &BTreeMap<String, String>)`.
  - `AddSettings for App`: `add_setting(&mut self, Setting) -> SettingId`.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn two() -> (Settings, SettingId, SettingId) {
        let mut settings = Settings::default();
        let glow = settings.add(Setting::new("glow", "Display", "Glow", ["Off", "On"]));
        let pace = settings.add(Setting::new("pace", "Play", "Pace", ["Slow", "Even", "Fast"]).default_choice(1));
        (settings, glow, pace)
    }

    #[test]
    fn a_setting_starts_on_its_default_and_the_first_choice_is_the_default_default() {
        let (settings, glow, pace) = two();
        assert_eq!((settings.chosen(glow), settings.choice(glow)), (0, "Off"));
        assert_eq!((settings.chosen(pace), settings.choice(pace)), (1, "Even"));
    }

    #[test]
    fn cycling_wraps_round_at_both_ends() {
        let (mut settings, _, pace) = two();
        settings.cycle(pace, 1);
        assert_eq!(settings.choice(pace), "Fast");
        settings.cycle(pace, 1);
        assert_eq!(settings.choice(pace), "Slow");
        settings.cycle(pace, -1);
        assert_eq!(settings.choice(pace), "Fast");
    }

    #[test]
    fn settings_are_listed_in_the_order_they_were_declared() {
        let (settings, ..) = two();
        assert_eq!(settings.iter().map(|(_, s, _)| s.name.as_str()).collect::<Vec<_>>(), ["glow", "pace"]);
    }

    #[test]
    #[should_panic(expected = "the setting `glow` is declared twice")]
    fn declaring_one_name_twice_panics_and_names_it() {
        let (mut settings, ..) = two();
        settings.add(Setting::new("glow", "Other", "Glow again", ["A", "B"]));
    }

    #[test]
    #[should_panic(expected = "the setting `bare` has no choices")]
    fn a_setting_with_no_choices_panics_when_declared() {
        Settings::default().add(Setting::new("bare", "Display", "Bare", Vec::<String>::new()));
    }

    #[test]
    fn what_is_remembered_is_each_name_and_the_words_of_its_choice() {
        let (mut settings, _, pace) = two();
        settings.choose(pace, 2);
        let kept = settings.remembered();
        assert_eq!(kept.get("glow").map(String::as_str), Some("Off"));
        assert_eq!(kept.get("pace").map(String::as_str), Some("Fast"));
    }

    #[test]
    fn recalling_skips_a_name_nobody_declared_and_a_choice_that_is_gone() {
        let (mut settings, glow, pace) = two();
        let kept = BTreeMap::from([("glow".to_string(), "On".to_string()), ("pace".to_string(), "Breakneck".to_string()), ("gone".to_string(), "On".to_string())]);
        settings.recall(&kept);
        assert_eq!(settings.choice(glow), "On");
        assert_eq!(settings.choice(pace), "Even", "an unknown choice leaves the default");
    }

    #[test]
    fn an_app_declares_a_setting_without_being_told_to_make_the_registry_first() {
        let mut app = App::new();
        let id = app.add_setting(Setting::new("glow", "Display", "Glow", ["Off", "On"]).key(KeyCode::F11));
        assert_eq!(app.world().resource::<Settings>().get(id).key, Some(KeyCode::F11));
    }
}
```

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p rl-bevy settings::`
Expected: does not compile, `Settings` not found.

- [ ] **Step 3: Implement**

```rust
//! What a player chooses once and expects to find again: declared once,
//! read everywhere, and listed.
//!
//! A setting is a name, where it is listed, and the choices it has. It is
//! plain data in the one crate the renderer, the UI and saving all depend
//! on, so each does its own part without knowing the others: whoever owns
//! the thing a setting changes declares it and reads it, the settings
//! screen lists and changes whatever was declared, and saving remembers
//! names and words without knowing what any of them mean.
//!
//! A registry, not an enum: a game's settings are declared the same way
//! as the engine's, and neither needs a variant anywhere.
//!
//! Declare in a plugin's `build`. What was remembered is recalled once
//! every plugin has built, and a setting declared after that starts on
//! its default.
//!
//! ```
//! # use bevy::prelude::*;
//! # use rl_bevy::settings::{AddSettings, Setting, Settings};
//! let mut app = App::new();
//! let trails = app.add_setting(Setting::new("trails", "Display", "Trails", ["On", "Off"]));
//! assert_eq!(app.world().resource::<Settings>().choice(trails), "On");
//! ```

use std::collections::BTreeMap;

use bevy::prelude::*;

/// One thing a player may choose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setting {
    /// The word it is remembered under. Stable: renaming it forgets what
    /// every player chose.
    pub name: String,
    /// The heading it is listed under.
    pub group: String,
    /// What it is called on the screen.
    pub label: String,
    /// What it may be set to, in the order a player cycles through.
    pub choices: Vec<String>,
    /// The choice it starts on, by its place in `choices`.
    pub default: usize,
    /// A key that cycles it from anywhere, if it has one.
    pub key: Option<KeyCode>,
}

impl Setting {
    /// A setting remembered as `name`, listed under `group` as `label`,
    /// starting on the first of `choices`.
    pub fn new(name: impl Into<String>, group: impl Into<String>, label: impl Into<String>, choices: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self { name: name.into(), group: group.into(), label: label.into(), choices: choices.into_iter().map(Into::into).collect(), default: 0, key: None }
    }

    /// Starts on the choice at `index` instead of the first.
    pub fn default_choice(mut self, index: usize) -> Self {
        self.default = index;
        self
    }

    /// Cycled by `key` as well as from the settings screen.
    pub fn key(mut self, key: KeyCode) -> Self {
        self.key = Some(key);
        self
    }
}

/// A declared setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SettingId(u16);

/// Every setting declared, in the order it was declared, and what each is
/// set to.
///
/// A reader that acts on a change watches the resource with Bevy's change
/// detection and compares against what it last applied; the registry does
/// not say which row moved, because the readers are few and each knows
/// its own.
#[derive(Resource, Debug, Clone, Default)]
pub struct Settings {
    rows: Vec<(Setting, usize)>,
}

impl Settings {
    /// Declares a setting and returns its id.
    ///
    /// # Panics
    /// Panics if the name is already declared, or if it has no choices or
    /// a default past the end of them: each is a mistake in a game's
    /// setup, not something to carry on from.
    pub fn add(&mut self, setting: Setting) -> SettingId {
        assert!(self.find(&setting.name).is_none(), "the setting `{}` is declared twice", setting.name);
        assert!(!setting.choices.is_empty(), "the setting `{}` has no choices", setting.name);
        assert!(setting.default < setting.choices.len(), "the setting `{}` defaults to a choice it does not have", setting.name);
        let chosen = setting.default;
        self.rows.push((setting, chosen));
        SettingId((self.rows.len() - 1) as u16)
    }

    /// The setting `id` names.
    ///
    /// # Panics
    /// Panics on an id from another registry.
    pub fn get(&self, id: SettingId) -> &Setting {
        &self.rows[id.0 as usize].0
    }

    /// The setting remembered as `name`.
    pub fn find(&self, name: &str) -> Option<SettingId> {
        self.rows.iter().position(|(s, _)| s.name == name).map(|i| SettingId(i as u16))
    }

    /// What `id` is set to, by its place in the setting's choices.
    pub fn chosen(&self, id: SettingId) -> usize {
        self.rows[id.0 as usize].1
    }

    /// What `id` is set to, in words.
    pub fn choice(&self, id: SettingId) -> &str {
        let (setting, chosen) = &self.rows[id.0 as usize];
        &setting.choices[*chosen]
    }

    /// Sets `id` to the choice at `index`, held to the last one.
    pub fn choose(&mut self, id: SettingId, index: usize) {
        let (setting, chosen) = &mut self.rows[id.0 as usize];
        *chosen = index.min(setting.choices.len() - 1);
    }

    /// Moves `id` `step` choices along, wrapping round at both ends.
    pub fn cycle(&mut self, id: SettingId, step: i32) {
        let (setting, chosen) = &mut self.rows[id.0 as usize];
        *chosen = (*chosen as i32 + step).rem_euclid(setting.choices.len() as i32) as usize;
    }

    /// Every setting with what it is set to, in declaration order.
    pub fn iter(&self) -> impl Iterator<Item = (SettingId, &Setting, usize)> {
        self.rows.iter().enumerate().map(|(i, (s, chosen))| (SettingId(i as u16), s, *chosen))
    }

    /// Whether nothing was declared.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Each name with the words of its choice: what saving writes down.
    ///
    /// Words rather than a place in the list, so a choice added in the
    /// middle of a setting does not quietly turn everyone's into another.
    /// Ordered, so the file is the same from one write to the next.
    pub fn remembered(&self) -> BTreeMap<String, String> {
        self.rows.iter().map(|(s, chosen)| (s.name.clone(), s.choices[*chosen].clone())).collect()
    }

    /// Sets every declared setting `kept` names to the choice it names.
    ///
    /// A name nobody declared and a choice the setting no longer has are
    /// passed over: what a player chose in an older build must not stop a
    /// newer one starting.
    pub fn recall(&mut self, kept: &BTreeMap<String, String>) {
        for (setting, chosen) in &mut self.rows {
            if let Some(index) = kept.get(&setting.name).and_then(|words| setting.choices.iter().position(|c| c == words)) {
                *chosen = index;
            }
        }
    }
}

/// Declaring settings while the app is built.
pub trait AddSettings {
    /// Declares a setting and returns its id. See [`Settings::add`].
    fn add_setting(&mut self, setting: Setting) -> SettingId;
}

impl AddSettings for App {
    fn add_setting(&mut self, setting: Setting) -> SettingId {
        self.init_resource::<Settings>();
        self.world_mut().resource_mut::<Settings>().add(setting)
    }
}
```

- [ ] **Step 4: Run the tests and see them pass**

Run: `cargo test -p rl-bevy settings`
Expected: PASS, the doc-test included.

- [ ] **Step 5: Check the facade's prelude still has no clash with Bevy's**

Run: `cargo test -p rl-engine --doc`
Expected: PASS.
If `Settings` or `Setting` clashes with a name in `bevy::prelude`, leave both out of `rl_bevy::prelude` and say why in the comment the prelude already carries for `Rect`.

- [ ] **Step 6: Gate and commit**

`CHANGELOG.md`: a settings registry, `Settings` and `app.add_setting`, for what a player chooses once.

```bash
cargo fmt --all --check && cargo clippy -p rl-bevy --all-targets -- -D warnings && cargo test -p rl-bevy settings
git add crates/rl-bevy CHANGELOG.md
git commit -m "a settings registry: what a player chooses is declared once as a name and its choices, and read by whoever owns what it changes"
```

---

### Task 3: Fullscreen

**Files:**
- Create: `crates/rl-render/src/fullscreen.rs`
- Modify: `crates/rl-render/src/lib.rs` (`pub mod fullscreen;`, `pub use fullscreen::{FULLSCREEN, FullscreenPlugin};`, and `FullscreenPlugin` in `prelude`)
- Modify: `crates/rl-engine/src/lib.rs` (`.add(rl_render::FullscreenPlugin)` after `TerminalPlugin`, and name it in `RoguelikePlugins`' doc comment)
- Modify: `CHANGELOG.md`

**Interfaces:**
- Consumes: `rl_bevy::settings::{AddSettings, Setting, SettingId, Settings}` from Task 2.
- Produces: `rl_render::FullscreenPlugin`, and `rl_render::FULLSCREEN: &str = "fullscreen"`, the name the setting is declared and remembered under. Choices are `["Off", "On"]`, group `"Display"`, label `"Fullscreen"`, key `KeyCode::F11`.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use bevy::window::PrimaryWindow;

    fn app() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(FullscreenPlugin);
        let window = app.world_mut().spawn((Window::default(), PrimaryWindow)).id();
        (app, window)
    }

    fn mode(app: &App, window: Entity) -> WindowMode {
        app.world().get::<Window>(window).unwrap().mode
    }

    fn set(app: &mut App, on: bool) {
        let mut settings = app.world_mut().resource_mut::<Settings>();
        let id = settings.find(FULLSCREEN).unwrap();
        settings.choose(id, on as usize);
    }

    #[test]
    fn the_setting_drives_the_window_into_fullscreen_and_back() {
        let (mut app, window) = app();
        app.finish();
        app.cleanup();
        app.update();
        assert_eq!(mode(&app, window), WindowMode::Windowed);
        set(&mut app, true);
        app.update();
        assert_eq!(mode(&app, window), WindowMode::BorderlessFullscreen(MonitorSelection::Current));
        set(&mut app, false);
        app.update();
        assert_eq!(mode(&app, window), WindowMode::Windowed);
    }

    #[test]
    fn a_choice_made_before_the_first_frame_is_on_the_window_before_it() {
        let (mut app, window) = app();
        set(&mut app, true);
        app.finish();
        app.cleanup();
        assert_eq!(mode(&app, window), WindowMode::BorderlessFullscreen(MonitorSelection::Current), "no frame has run yet");
    }

    #[test]
    fn with_no_window_nothing_fails() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(FullscreenPlugin);
        app.finish();
        app.cleanup();
        app.update();
    }
}
```

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p rl-render fullscreen`
Expected: does not compile.

- [ ] **Step 3: Implement**

```rust
//! Filling the screen, as a setting.
//!
//! The window is the renderer's, so the renderer declares the setting and
//! is the one thing that reads it. The settings screen changes it without
//! knowing what it does, and saving remembers it without knowing either.
//!
//! Borderless rather than exclusive: it takes the display at the
//! resolution it already has, so nothing flickers and the layout in
//! [`layout`](crate::layout) draws the grid for exactly those pixels.

use bevy::prelude::*;
use bevy::window::{MonitorSelection, PrimaryWindow, WindowMode};
use rl_bevy::settings::{AddSettings, Setting, Settings};

/// The name the fullscreen setting is declared and remembered under.
pub const FULLSCREEN: &str = "fullscreen";

/// Declares the fullscreen setting and keeps the primary window in the
/// mode it asks for.
///
/// On the web it declares nothing. A browser grants fullscreen only from
/// inside the handler of a key or a click, and Bevy reads keys a frame
/// later, so the setting could not be honored; the canvas follows the
/// page instead, and the browser's own fullscreen fills the screen.
pub struct FullscreenPlugin;

impl Plugin for FullscreenPlugin {
    fn build(&self, app: &mut App) {
        if cfg!(target_arch = "wasm32") {
            return;
        }
        app.add_setting(Setting::new(FULLSCREEN, "Display", "Fullscreen", ["Off", "On"]).key(KeyCode::F11));
        app.add_systems(PostUpdate, apply.run_if(resource_changed::<Settings>));
    }

    /// Once, before the first frame and after every plugin has finished,
    /// which is after what was remembered has been recalled: a game left
    /// fullscreen opens that way rather than flashing a window first.
    fn cleanup(&self, app: &mut App) {
        if cfg!(target_arch = "wasm32") {
            return;
        }
        let wanted = wanted(app.world().resource::<Settings>());
        let mut windows = app.world_mut().query_filtered::<&mut Window, With<PrimaryWindow>>();
        if let Ok(mut window) = windows.single_mut(app.world_mut())
            && window.mode != wanted
        {
            window.mode = wanted;
        }
    }
}

/// The mode the setting asks for.
fn wanted(settings: &Settings) -> WindowMode {
    let on = settings.find(FULLSCREEN).is_some_and(|id| settings.chosen(id) == 1);
    if on { WindowMode::BorderlessFullscreen(MonitorSelection::Current) } else { WindowMode::Windowed }
}

/// Puts the window in the mode the setting asks for, when it is not.
///
/// Compared before it is written, so a change to some other setting does
/// not mark the window changed.
fn apply(settings: Res<Settings>, mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    let wanted = wanted(&settings);
    if let Ok(mut window) = windows.single_mut()
        && window.mode != wanted
    {
        window.mode = wanted;
    }
}
```

- [ ] **Step 4: Run the tests and see them pass**

Run: `cargo test -p rl-render fullscreen`
Expected: PASS.

- [ ] **Step 5: Add it to `RoguelikePlugins` and build the games**

In `crates/rl-engine/src/lib.rs`, after the `TerminalPlugin` line:

```rust
            // Filling the screen, as a setting the renderer declares.
            .add(rl_render::FullscreenPlugin)
```

Run: `cargo build --workspace`
Expected: builds.

- [ ] **Step 6: Gate and commit**

`CHANGELOG.md`: `RoguelikePlugins` adds `FullscreenPlugin`, which declares a `fullscreen` setting; nothing is fullscreen until something sets it.

```bash
cargo fmt --all --check && cargo clippy -p rl-render -p rl-engine --all-targets -- -D warnings && cargo test -p rl-render && cargo test -p rl-engine
git add crates/rl-render crates/rl-engine CHANGELOG.md
git commit -m "fullscreen is a setting the renderer declares and applies to its window, borderless, and before the first frame when it was already chosen"
```

---

### Task 4: The settings screen, and the menu's row

**Files:**
- Create: `crates/rl-ui/src/panel/settings.rs`
- Modify: `crates/rl-ui/src/panel/mod.rs` (`pub mod settings;`)
- Modify: `crates/rl-ui/src/lib.rs` (export `SETTINGS_MODAL`, `SettingsPanel`, `settings_modal` beside the other panels, and `SettingsPanel` in `prelude`; add the screen to the crate docs' list of panels)
- Modify: `crates/rl-ui/src/game_menu.rs` (`MenuItem::Settings`)
- Modify: `CHANGELOG.md`

**Interfaces:**
- Consumes: `Settings`, `SettingId` from Task 2; `Modals`, `AddModal`, `ControlInput`, `AddControls`, `ControlId`, `clear`, `frame`, `clip`, `Palette`, `Tones` from `rl-ui`.
- Produces: `SettingsPanel::new(rect: Rect) -> SettingsPanel` (a `Plugin`), `SETTINGS_MODAL: &str = "settings"`, `settings_modal(modals: &Modals) -> Option<ModalId>` (`None` when the panel was not added), `MenuItem::Settings`, and `MenuItem::offered(playing: bool, settings: bool) -> Vec<MenuItem>`.

- [ ] **Step 1: Write the failing tests**

In `crates/rl-ui/src/panel/settings.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_menu::GameMenuPanel;
    use crate::harness::Stage;
    use rl_bevy::settings::{AddSettings, Setting};

    fn staged() -> Stage {
        let rect = Rect::new(0, 0, 40, 12);
        let mut stage = Stage::new_with((GameMenuPanel::new(rect), SettingsPanel::new(rect), crate::InventoryPanel::new(rect)), |app| {
            app.add_setting(Setting::new("glow", "Display", "Glow", ["Off", "On"]).key(KeyCode::F11));
            app.add_setting(Setting::new("pace", "Play", "Pace", ["Slow", "Even", "Fast"]).default_choice(1));
        })
        .screen(40, 12);
        stage.tick();
        stage
    }

    fn choice(stage: &Stage, name: &str) -> String {
        let settings = stage.app.world().resource::<Settings>();
        settings.choice(settings.find(name).unwrap()).to_string()
    }

    /// Opens the menu and takes its Settings row.
    fn open(stage: &mut Stage) {
        stage.press(KeyCode::Escape);
        for _ in 0..3 {
            stage.press(KeyCode::ArrowDown);
        }
        stage.press(KeyCode::Enter);
    }

    #[test]
    fn the_menu_row_opens_the_screen_with_every_setting_under_its_heading() {
        let mut stage = staged();
        open(&mut stage);
        assert!(stage.row(0).contains(" Settings "), "{:?}", stage.row(0));
        let rows = stage.rows().join("\n");
        assert!(rows.contains("Display") && rows.contains("Glow") && rows.contains("Off"), "{rows}");
        assert!(rows.contains("Play") && rows.contains("Pace") && rows.contains("Even"), "{rows}");
    }

    #[test]
    fn the_key_that_opened_the_screen_does_not_also_change_the_first_setting() {
        let mut stage = staged();
        open(&mut stage);
        assert_eq!(choice(&stage, "glow"), "Off");
    }

    #[test]
    fn left_and_right_change_the_picked_row_and_down_picks_the_next() {
        let mut stage = staged();
        open(&mut stage);
        stage.press(KeyCode::ArrowRight);
        assert_eq!(choice(&stage, "glow"), "On");
        stage.press(KeyCode::ArrowDown);
        stage.press(KeyCode::ArrowLeft);
        assert_eq!(choice(&stage, "pace"), "Slow");
        stage.press(KeyCode::Enter);
        assert_eq!(choice(&stage, "pace"), "Even", "confirm cycles forward");
        assert_eq!(choice(&stage, "glow"), "On", "the other row is untouched");
    }

    #[test]
    fn the_close_key_goes_back_to_the_menu_and_not_past_it() {
        let mut stage = staged();
        open(&mut stage);
        stage.press(KeyCode::Escape);
        assert!(stage.row(0).contains(" Menu "), "{:?}", stage.row(0));
        assert!(stage.app.world().resource::<Modals>().any_open());
    }

    #[test]
    fn a_settings_key_changes_it_with_nothing_open_and_is_listed_as_a_control() {
        let mut stage = staged();
        stage.press(KeyCode::F11);
        assert_eq!(choice(&stage, "glow"), "On");
        let controls = stage.app.world().resource::<crate::Controls>();
        assert!(controls.find("Display", "switch glow").is_some());
    }

    #[test]
    fn a_settings_key_does_nothing_while_another_screen_is_on_top() {
        let mut stage = staged();
        let bag = stage.app.world().resource::<Modals>().get(crate::INVENTORY_MODAL).unwrap();
        stage.app.world_mut().resource_mut::<Modals>().open(bag);
        stage.tick();
        stage.press(KeyCode::F11);
        assert_eq!(choice(&stage, "glow"), "Off");
    }

    #[test]
    fn the_menu_offers_no_settings_row_when_nothing_is_declared() {
        let rect = Rect::new(0, 0, 40, 12);
        let mut stage = Stage::new((GameMenuPanel::new(rect), SettingsPanel::new(rect))).screen(40, 12);
        stage.press(KeyCode::Escape);
        assert!(!stage.rows().join("\n").contains("Settings"));
    }
}
```

The existing menu test `escape_opens_the_menu_over_the_run_and_the_first_row_takes_you_back` adds no `SettingsPanel`, and must keep passing untouched: `Quit` stays on row 4 there.

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p rl-ui settings`
Expected: does not compile.

- [ ] **Step 3: Implement the screen**

```rust
//! The settings screen: every declared setting under its heading, and the
//! keys that change them.
//!
//! It lists [`Settings`](rl_bevy::settings::Settings) and nothing else, so
//! what it shows is what the engine and the game read. It does not know
//! what any setting does: whoever declared one watches the registry.
//!
//! Unlike every other screen it is drawn after the whole of
//! `EngineSet::Present` and read in any engine state, because it is the
//! one screen a game opens before there is a world, from its own title
//! screen, where no presenter runs; and because it must cover the menu
//! that opened it.

use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_bevy::settings::{SettingId, Settings};
use rl_core::{Direction, Rect};
use rl_render::{Cell, Terminal};

use crate::controls::{AddControls, ControlId, ControlInput, key_name};
use crate::modal::{AddModal, ModalId, Modals};
use crate::panel::{clear, clip, frame};
use crate::tone::{Palette, Tones};

/// The name the settings screen's modal is declared under.
pub const SETTINGS_MODAL: &str = "settings";

/// Where the screen is drawn.
#[derive(Resource, Debug, Clone)]
pub struct SettingsLayout {
    /// The most of the terminal it may occupy; drawn only as far down as
    /// its rows need.
    pub rect: Rect,
}

/// Which row is picked out, counting settings and not headings.
#[derive(Resource, Debug, Default)]
pub struct SettingsScreen {
    /// The row picked out.
    pub selected: usize,
}

/// The control each keyed setting was declared as, so the controls screen
/// lists the key and this screen reads it through the registry.
#[derive(Resource, Debug, Default)]
struct SettingKeys(Vec<(SettingId, ControlId)>);

/// The settings screen.
pub struct SettingsPanel(SettingsLayout);

impl SettingsPanel {
    /// The screen in `rect`, from its top-left corner down as far as its
    /// rows need.
    pub fn new(rect: Rect) -> Self {
        Self(SettingsLayout { rect })
    }
}

impl Plugin for SettingsPanel {
    fn build(&self, app: &mut App) {
        app.add_modal(SETTINGS_MODAL);
        app.init_resource::<Settings>().init_resource::<SettingsScreen>().init_resource::<SettingKeys>().insert_resource(self.0.clone());
        // After the menu's keys, which are this crate's own: the key that
        // closes this screen must not be read by the menu under it in the
        // same frame. Outside the engine's sets, since those do not run
        // before a run exists.
        app.add_systems(Update, settings_keys.after(crate::game_menu::menu_keys).before(EngineSet::Input))
            .add_systems(Update, draw_settings.after(EngineSet::Present));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::UiPlugin>(app, "SettingsPanel");
        let keyed: Vec<_> = app.world().resource::<Settings>().iter().filter_map(|(id, s, _)| s.key.map(|key| (id, s.group.clone(), s.label.to_lowercase(), key))).collect();
        let mut keys = Vec::new();
        for (id, group, label, key) in keyed {
            keys.push((id, app.add_control(&group, &format!("switch {label}"), key)));
        }
        app.insert_resource(SettingKeys(keys));
    }
}

/// The id of the settings screen's modal, or `None` when the panel was
/// not added: a menu or a title screen offers the row only when it is.
pub fn settings_modal(modals: &Modals) -> Option<ModalId> {
    modals.get(SETTINGS_MODAL)
}

/// Reads a setting's own key, and the screen's keys while it is on top.
///
/// A frame that opened the screen reads nothing more: the key that opened
/// it is the confirm key, still down, and it would change the first row.
fn settings_keys(keys: ControlInput, bound: Res<SettingKeys>, mut screen: ResMut<SettingsScreen>, mut modals: ResMut<Modals>, mut settings: ResMut<Settings>) {
    let Some(modal) = settings_modal(&modals) else { return };
    let top = modals.is_top(modal);
    if top && modals.just_opened() {
        screen.selected = 0;
        return;
    }
    if top || !modals.any_open() {
        for (id, control) in &bound.0 {
            if keys.just_pressed(*control) {
                settings.cycle(*id, 1);
            }
        }
    }
    if !top || modals.closing() {
        return;
    }
    let (input, bindings) = (keys.input(), keys.bindings());
    if input.just_pressed(bindings.cursor.close) {
        modals.close_one(modal);
        return;
    }
    let ids: Vec<SettingId> = settings.iter().map(|(id, ..)| id).collect();
    if ids.is_empty() {
        return;
    }
    let picked = screen.selected.min(ids.len() - 1);
    let confirm = input.just_pressed(bindings.cursor.confirm) || input.just_pressed(bindings.cursor.also_confirm);
    match bindings.directions.just_pressed(input) {
        Some(Direction::North) => screen.selected = (picked + ids.len() - 1) % ids.len(),
        Some(Direction::South) => screen.selected = (picked + 1) % ids.len(),
        Some(Direction::West) => settings.cycle(ids[picked], -1),
        Some(Direction::East) => settings.cycle(ids[picked], 1),
        _ if confirm => settings.cycle(ids[picked], 1),
        _ => {}
    }
}

/// Paints the screen while it is open: each group's heading, and under it
/// each setting with its choice at the right edge.
fn draw_settings(terminal: Option<ResMut<Terminal>>, layout: Res<SettingsLayout>, screen: Res<SettingsScreen>, modals: Res<Modals>, settings: Res<Settings>, keys: ControlInput, palette: Res<Palette>) {
    let Some(mut terminal) = terminal else { return };
    if !settings_modal(&modals).is_some_and(|modal| modals.is_open(modal)) {
        return;
    }
    let rect = layout.rect;
    if rect.width < 16 || rect.height < 4 {
        return;
    }
    let mut groups: Vec<&str> = Vec::new();
    for (_, setting, _) in settings.iter() {
        if !groups.contains(&setting.group.as_str()) {
            groups.push(&setting.group);
        }
    }
    let count = settings.iter().count();
    let height = ((count + groups.len()) as i32 + 2).min(rect.height);
    let rect = Rect::new(rect.x, rect.y, rect.width, height);
    let bindings = keys.bindings();
    let hints = format!("\u{2191}\u{2193} pick \u{2022} \u{2190}\u{2192} change \u{2022} {} back", key_name(bindings.cursor.close));
    clear(&mut terminal, rect, &palette);
    frame(&mut terminal, rect, "Settings", &hints, &palette);
    let inner = Rect::new(rect.x + 2, rect.y + 1, rect.width - 4, rect.height - 2);
    let surface = palette.get(Tones::SURFACE);
    let picked = screen.selected.min(count.saturating_sub(1));
    let (mut y, mut row) = (inner.y, 0);
    for group in groups {
        if y >= inner.bottom() {
            break;
        }
        terminal.print_on(inner.x, y, &clip(group, inner.width as usize), palette.get(Tones::TITLE), surface);
        y += 1;
        for (_, setting, chosen) in settings.iter().filter(|(_, s, _)| s.group == group) {
            if y >= inner.bottom() {
                break;
            }
            let bg = if row == picked { palette.get(Tones::SELECT) } else { surface };
            terminal.fill(Rect::new(inner.x, y, inner.width, 1), Cell::new(' ', palette.get(Tones::TEXT)).on(bg));
            let choice = clip(&setting.choices[chosen], (inner.width as usize).saturating_sub(4));
            let room = (inner.width as usize).saturating_sub(choice.chars().count() + 3);
            terminal.print_on(inner.x + 1, y, &clip(&setting.label, room), palette.get(Tones::TEXT), bg);
            terminal.print_on(inner.right() - 1 - choice.chars().count() as i32, y, &choice, palette.get(Tones::TEXT), bg);
            row += 1;
            y += 1;
        }
    }
}
```

The picked row is counted in the order drawn, groups together; `settings_keys` walks declaration order.
Those differ when two settings of one group are declared with another group's between them, so make both walk the drawn order: collect the ids as `groups` order then declaration order within a group, in one private function `fn ordered(settings: &Settings) -> Vec<(SettingId, &str /* group */)>` that both systems call, and add this test:

```rust
    #[test]
    fn rows_are_picked_in_the_order_they_are_drawn_when_a_group_is_declared_in_two_parts() {
        let rect = Rect::new(0, 0, 40, 12);
        let mut stage = Stage::new_with((GameMenuPanel::new(rect), SettingsPanel::new(rect)), |app| {
            app.add_setting(Setting::new("a", "Display", "A", ["Off", "On"]));
            app.add_setting(Setting::new("b", "Play", "B", ["Off", "On"]));
            app.add_setting(Setting::new("c", "Display", "C", ["Off", "On"]));
        })
        .screen(40, 12);
        open(&mut stage);
        stage.press(KeyCode::ArrowDown);
        stage.press(KeyCode::ArrowRight);
        assert_eq!(choice(&stage, "c"), "On", "the second row drawn is C, under Display");
        assert_eq!(choice(&stage, "b"), "Off");
    }
```

`Rect::right` and `Rect::bottom` are used as `game_menu.rs` uses `bottom`; if `right` does not exist, use `inner.x + inner.width`.

- [ ] **Step 4: Give the menu its row**

In `crates/rl-ui/src/game_menu.rs`:

- Add the variant, before `Quit`:

```rust
    /// The settings screen, when a game has one with something on it.
    Settings,
```

- `offered` takes whether to offer it:

```rust
    /// What is offered while `playing`, or once the run is over, with the
    /// settings screen when there is one with a row on it.
    pub fn offered(playing: bool, settings: bool) -> Vec<MenuItem> {
        let mut items = Vec::new();
        if playing {
            items.push(MenuItem::Resume);
        }
        items.extend([MenuItem::NewRun, MenuItem::SameSeed]);
        if settings {
            items.push(MenuItem::Settings);
        }
        items.push(MenuItem::Quit);
        items
    }
```

- `label`: `MenuItem::Settings => "Settings",`.
- A helper both `menu_keys` and `draw_game_menu` call, so they cannot disagree:

```rust
/// The settings screen's modal, when the menu should offer it: the panel
/// was added, and something is declared for it to list. A screen with no
/// rows is not offered, which is what a build on the web gets.
fn settings_offered(modals: &Modals, settings: Option<&rl_bevy::settings::Settings>) -> Option<ModalId> {
    crate::panel::settings::settings_modal(modals).filter(|_| settings.is_some_and(|s| !s.is_empty()))
}
```

- Add `settings: Option<Res<'w, rl_bevy::settings::Settings>>` to the `Opening` system param and to `MenuScreen`, call `MenuItem::offered(playing, settings_offered(&modals, ...).is_some())` in both systems, and in `menu_keys`' match:

```rust
        MenuItem::Settings => {
            if let Some(settings) = settings_offered(&modals, opening.settings.as_deref()) {
                modals.open(settings);
            }
        }
```

- Make `menu_keys` leave the keys alone in a frame where a screen closed over it, as its first check after `if !modals.is_top(modal) { return; }`:

```rust
    // A screen that closed over the menu this frame did so on a key that
    // is still down; it is not the menu's.
    if modals.closing() {
        return;
    }
```

- [ ] **Step 5: Run the tests and see them pass**

Run: `cargo test -p rl-ui settings` then `cargo test -p rl-ui game_menu`
Expected: PASS, the existing menu tests untouched.

- [ ] **Step 6: Gate and commit**

`CHANGELOG.md`: `SettingsPanel`, a screen listing every declared setting; the menu offers it when it was added and a setting is declared; `MenuItem::offered` takes a second argument and `MenuItem` has a `Settings` variant, which a game matching on it must handle.

```bash
cargo fmt --all --check && cargo clippy -p rl-ui --all-targets -- -D warnings && cargo test -p rl-ui
git add crates/rl-ui CHANGELOG.md
git commit -m "a settings screen lists every declared setting under its heading, the menu offers it when there is one, and a setting's own key is a control"
```

---

### Task 5: Remembering

**Files:**
- Create: `crates/rl-save/src/settings.rs`
- Modify: `crates/rl-save/src/lib.rs` (`pub mod settings;`, `pub use settings::{SETTINGS_SLOT, SettingsSavePlugin};`, `SettingsSavePlugin` in `prelude`, and one sentence in the crate docs)
- Modify: `CHANGELOG.md`

**Interfaces:**
- Consumes: `Settings::remembered`, `Settings::recall` from Task 2; `Saves`, `SaveBackend`, `MemoryBackend` from `rl-save`; `Needs` from `rl-bevy`.
- Produces: `rl_save::SettingsSavePlugin` (unit struct, a `Plugin`) and `rl_save::SETTINGS_SLOT: &str = "settings"`.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MemoryBackend;
    use rl_bevy::settings::{AddSettings, Setting};
    use std::sync::Arc;

    /// An app with two settings declared and `backend` to remember them
    /// in, finished the way `App::run` finishes one.
    fn game(backend: &Arc<MemoryBackend>) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(Saves(backend.clone()));
        app.add_setting(Setting::new("glow", "Display", "Glow", ["Off", "On"]));
        app.add_setting(Setting::new("pace", "Play", "Pace", ["Slow", "Even", "Fast"]).default_choice(1));
        app.add_plugins(SettingsSavePlugin);
        app.finish();
        app.cleanup();
        app
    }

    fn choice(app: &App, name: &str) -> String {
        let settings = app.world().resource::<Settings>();
        settings.choice(settings.find(name).unwrap()).to_string()
    }

    fn choose(app: &mut App, name: &str, index: usize) {
        let mut settings = app.world_mut().resource_mut::<Settings>();
        let id = settings.find(name).unwrap();
        settings.choose(id, index);
    }

    #[test]
    fn what_a_player_chose_is_there_the_next_time_the_game_is_started() {
        let backend = Arc::new(MemoryBackend::default());
        let mut app = game(&backend);
        app.update();
        choose(&mut app, "glow", 1);
        app.update();
        let again = game(&backend);
        assert_eq!(choice(&again, "glow"), "On", "recalled before any frame has run");
        assert_eq!(choice(&again, "pace"), "Even");
    }

    #[test]
    fn nothing_is_written_until_a_setting_changes() {
        let backend = Arc::new(MemoryBackend::default());
        let mut app = game(&backend);
        app.update();
        app.update();
        assert!(!backend.exists(SETTINGS_SLOT));
    }

    #[test]
    fn loading_what_was_remembered_does_not_write_it_back() {
        let backend = Arc::new(MemoryBackend::default());
        let text = "{\"glow\": \"On\", \"gone\": \"Yes\"}";
        backend.persist(SETTINGS_SLOT, text).unwrap();
        let mut app = game(&backend);
        app.update();
        app.update();
        assert_eq!(backend.load(SETTINGS_SLOT).unwrap().as_deref(), Some(text), "byte for byte what was there");
    }

    #[test]
    fn a_name_nobody_declares_and_a_choice_that_is_gone_are_passed_over() {
        let backend = Arc::new(MemoryBackend::default());
        backend.persist(SETTINGS_SLOT, "{\"glow\": \"Blinding\", \"gone\": \"Yes\", \"pace\": \"Fast\"}").unwrap();
        let app = game(&backend);
        assert_eq!(choice(&app, "glow"), "Off");
        assert_eq!(choice(&app, "pace"), "Fast");
    }

    #[test]
    fn a_file_that_is_not_ron_at_all_leaves_the_defaults_and_is_not_touched() {
        let backend = Arc::new(MemoryBackend::default());
        backend.persist(SETTINGS_SLOT, "this is not a map").unwrap();
        let mut app = game(&backend);
        app.update();
        assert_eq!(choice(&app, "glow"), "Off");
        assert_eq!(backend.load(SETTINGS_SLOT).unwrap().as_deref(), Some("this is not a map"));
    }

    #[test]
    fn a_change_is_written_as_names_and_the_words_of_their_choices() {
        let backend = Arc::new(MemoryBackend::default());
        let mut app = game(&backend);
        app.update();
        choose(&mut app, "pace", 2);
        app.update();
        let text = backend.load(SETTINGS_SLOT).unwrap().expect("written");
        let kept: BTreeMap<String, String> = ron::from_str(&text).unwrap();
        assert_eq!(kept, BTreeMap::from([("glow".to_string(), "Off".to_string()), ("pace".to_string(), "Fast".to_string())]));
    }

    #[test]
    fn the_plugin_says_it_needs_a_backend() {
        let mut app = rl_bevy::plugin::headless_app();
        app.add_plugins(SettingsSavePlugin);
        let missing = app.world().resource::<rl_bevy::plugin::Requirements>().missing(app.world());
        assert!(missing.iter().any(|m| m.contains("SettingsSavePlugin") && m.contains("Saves")), "{missing:?}");
    }
}
```

Check how `SavePlugin`'s own tests read `Requirements` and copy that; if `missing` is private or named otherwise, assert the way they do.

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p rl-save settings`
Expected: does not compile.

- [ ] **Step 3: Implement**

```rust
//! Remembering what a player chose.
//!
//! [`Settings`](rl_bevy::settings::Settings) is plain data that knows
//! nothing of storage, and the settings screen draws it without knowing
//! either. This is the third part: one slot, read once before the first
//! frame and written whenever a setting changes.
//!
//! Not a [`Versioned`](crate::Versioned) envelope. That refuses a save
//! whole when its version differs, which is right for a run and wrong for
//! a preference: a player who chose fullscreen should still have it after
//! an upgrade. So the file is a map of names to the words of their
//! choices, and anything in it a build does not recognize is passed over.
//!
//! Apart from the run's slot, and never forgotten with it: a new run, a
//! restart and the end of a run leave it alone.

use std::collections::BTreeMap;

use bevy::prelude::*;
use rl_bevy::plugin::Needs;
use rl_bevy::settings::Settings;

use crate::backend::{SaveBackend, Saves};

/// The slot settings are remembered in.
pub const SETTINGS_SLOT: &str = "settings";

/// Recalls what was remembered before the first frame, and writes it down
/// when a setting changes.
///
/// Opt-in: without it the settings work and start on their defaults every
/// launch. Add it after the plugins that declare settings in their
/// `build`; what is declared later starts on its default.
pub struct SettingsSavePlugin;

/// What the slot is known to hold, so a frame that changed nothing, and
/// the load itself, write nothing.
#[derive(Resource, Debug, Default)]
struct Remembered(BTreeMap<String, String>);

impl Plugin for SettingsSavePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Settings>()
            .init_resource::<Remembered>()
            .needs::<Saves>("SettingsSavePlugin", "`Saves`, the backend settings are remembered through, such as `Saves::platform_default(\"my-game\")`")
            .add_systems(Last, remember.run_if(resource_changed::<Settings>).run_if(resource_exists::<Saves>));
    }

    /// In `finish`, when every plugin has built and so declared its
    /// settings, and before any plugin's `cleanup`, where the renderer
    /// puts the window in the mode it finds chosen.
    fn finish(&self, app: &mut App) {
        let Some(saves) = app.world().get_resource::<Saves>() else { return };
        let kept = match saves.load(SETTINGS_SLOT) {
            Ok(Some(text)) => match ron::from_str::<BTreeMap<String, String>>(&text) {
                Ok(kept) => kept,
                Err(e) => {
                    warn!("the remembered settings could not be read, so the defaults stand: {e}");
                    BTreeMap::new()
                }
            },
            Ok(None) => BTreeMap::new(),
            Err(e) => {
                warn!("the remembered settings could not be loaded, so the defaults stand: {e}");
                BTreeMap::new()
            }
        };
        let mut settings = app.world_mut().resource_mut::<Settings>();
        settings.recall(&kept);
        let now = settings.remembered();
        app.world_mut().resource_mut::<Remembered>().0 = now;
    }
}

/// Writes the settings down when they are not what the slot holds.
fn remember(settings: Res<Settings>, saves: Res<Saves>, mut remembered: ResMut<Remembered>) {
    let now = settings.remembered();
    if now == remembered.0 {
        return;
    }
    match ron::ser::to_string_pretty(&now, ron::ser::PrettyConfig::default()) {
        Ok(text) => match saves.persist(SETTINGS_SLOT, &text) {
            Ok(()) => remembered.0 = now,
            Err(e) => warn!("the settings could not be remembered: {e}"),
        },
        Err(e) => warn!("the settings could not be written down: {e}"),
    }
}
```

If `Last` already has an `EndOfFrame` set that `SavePlugin` orders its exclusive systems in, and Bevy's ambiguity checker in this workspace's tests reports `remember` against them, put it `.before(EndOfFrame::Save)` in that set's schedule; it reads only `Settings` and `Saves`.

- [ ] **Step 4: Run the tests and see them pass**

Run: `cargo test -p rl-save settings`
Expected: PASS.

- [ ] **Step 5: Gate and commit**

`CHANGELOG.md`: `SettingsSavePlugin` remembers settings in a `settings` slot through `Saves`; opt-in.

```bash
cargo fmt --all --check && cargo clippy -p rl-save --all-targets -- -D warnings && cargo test -p rl-save
git add crates/rl-save CHANGELOG.md
git commit -m "settings are remembered in a slot of their own, recalled before the first frame and written when one changes, and a file from another build is read for what it still means"
```

---

### Task 6: Foundry

**Files:**
- Modify: `examples/foundry/src/main.rs` (`add_panels`, the plugin list, `Screen`)
- Modify: `examples/foundry/src/title.rs` (`Choice`, `Title`, `read_title_keys`, the menu's rows, and its tests)
- Modify: `examples/foundry/DESIGN.md` if it lists the title screen's rows
- Modify: `CHANGELOG.md` only if a game author is affected; Foundry's own changes are not

**Interfaces:**
- Consumes: `SettingsPanel::new(rect)`, `settings_modal(&Modals) -> Option<ModalId>`, `Settings::is_empty`, `SettingsSavePlugin`.

- [ ] **Step 1: Write the failing title tests**

Read the existing tests at the bottom of `examples/foundry/src/title.rs` and add, in their style and with their helpers:

```rust
    #[test]
    fn the_settings_row_sits_between_new_game_and_exit() {
        assert_eq!(Choice::all(), [Choice::Continue, Choice::NewGame, Choice::Settings, Choice::Exit]);
    }

    #[test]
    fn the_settings_row_cannot_be_picked_when_there_is_no_settings_screen() {
        assert!(!Choice::Settings.available(SaveOnDisk::None, false));
        assert!(Choice::Settings.available(SaveOnDisk::None, true));
        // Stepping down from New Game passes over it to Exit.
        assert_eq!(step(1, 1, SaveOnDisk::None, false), 3);
        assert_eq!(step(1, 1, SaveOnDisk::None, true), 2);
    }
```

And one driving the keys, built the way the file's existing key tests build their app, with `UiPlugin`, `SettingsPanel::new(Rect::new(0, 0, 40, 12))` and one declared setting added:

- pressing Down to `Settings` and Enter opens the settings modal, and the title stays up;
- while it is open, Down and Enter do not move `title.picked` or start a run;
- Escape closes it, the app does not exit, and `title.up` is still true.

Name it `the_title_gives_its_keys_to_the_settings_screen_while_it_is_open_and_takes_them_back_when_it_closes`.

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p foundry title`
Expected: does not compile.

- [ ] **Step 3: Implement the title's row**

In `title.rs`:

- `Choice` gains `Settings` (doc: "What the player may set: the engine's settings screen, over this one."), `all()` returns `[Choice; 4]` in the order Continue, NewGame, Settings, Exit, and `label` returns `"Settings"`.
- `Title` gains `pub settings: bool` ("Whether there is a settings screen with something on it to open."), `false` in `Default`.
- `available(self, save: SaveOnDisk, settings: bool)`:

```rust
    fn available(self, save: SaveOnDisk, settings: bool) -> bool {
        match self {
            Choice::Continue => save == SaveOnDisk::Readable,
            Choice::Settings => settings,
            Choice::NewGame | Choice::Exit => true,
        }
    }
```

- `step` and `first_available` take the same `settings: bool` and pass it through; every caller passes `title.settings`.
- A `Startup` system, added in `TitlePlugin::build`, that sets it once every plugin has finished:

```rust
/// Whether there is a settings screen to open: the panel was added and a
/// setting is declared. Looked at once, since both are fixed by then.
fn look_for_settings(mut title: ResMut<Title>, modals: Option<Res<Modals>>, settings: Option<Res<Settings>>) {
    title.settings = modals.is_some_and(|m| settings_modal(&m).is_some()) && settings.is_some_and(|s| !s.is_empty());
}
```

- `read_title_keys` takes `mut modals: Option<ResMut<Modals>>` and starts with:

```rust
    // A screen over the title has the keys, and the key that closed one
    // this frame is still down and is not the title's.
    if modals.as_deref().is_some_and(|m| m.any_open() || m.closing()) {
        return;
    }
```

  and its match gains:

```rust
        Choice::Settings => {
            if let Some(modals) = modals.as_deref_mut()
                && let Some(settings) = settings_modal(modals)
            {
                modals.open(settings);
            }
        }
```

- The menu's drawing loops over `Choice::all()` already; read `paint_menu` and the `rows` module and make sure four rows at the existing spacing end above the terminal's last row. If they do not, move `rows::MENU` up by the two rows the new entry takes and check the rule above it and the tagline still clear the picture.
- A row that cannot be picked is drawn the way `Continue` is drawn with no save.

- [ ] **Step 4: Wire the plugins**

In `main.rs`:

- `Screen` gains a `settings: Rect`; give it the same rectangle as `screen.menu`, so the screen stands where the menu that opened it stood.
- In `add_panels`, after `GameMenuPanel`:

```rust
        // What the player may set, opened from the menu and from the title
        // screen: fullscreen is the engine's, declared by `RoguelikePlugins`.
        SettingsPanel::new(screen.settings),
```

- In `main`, after `.insert_resource(saves_for(scripted()))`:

```rust
        // Remembered beside the save and apart from it. A scripted run's
        // backend is in memory, so a capture or a replay starts on the
        // defaults whatever the player chose.
        .add_plugins(rl_engine::rl_save::SettingsSavePlugin)
```

  Wrap the two lines `SettingsPanel::new(...)` in `// ANCHOR: settings` and `// ANCHOR_END: settings`, and the `SettingsSavePlugin` line in `// ANCHOR: remember` and `// ANCHOR_END: remember`; the guide's page quotes them in Task 7.

- [ ] **Step 5: Run Foundry's tests**

Run: `cargo test -p foundry`
Expected: PASS. A test that counted three title rows is updated to four, and nothing else changes.

- [ ] **Step 6: Play it, and look hard**

This is the end-to-end check, done as a player would, with the Read tool on every screenshot.

```bash
cargo build -p foundry
rm -f target/debug/settings.save.ron
```

1. Run `./target/debug/foundry` in the background, wait for the title, `screencapture -x <scratchpad>/title.png`. The four rows are evenly spaced and nothing overlaps the picture.
2. The window cannot be driven from the shell on this machine (no accessibility access), so drive the game with a small temporary key script: read `crates/rl-render/src/capture.rs` for `RL_KEYS`, and note a capture run saves in memory. For the remembered path, write the file by hand instead: `printf '{"fullscreen": "On"}' > target/debug/settings.save.ron`, run the game, `screencapture -x <scratchpad>/fullscreen.png`.
3. Read `fullscreen.png`: the game fills the display from its first frame. Crop the tagline at full resolution, enlarge it four times with nearest sampling as the investigation did, and compare with the stretched capture: stems are even, and no seam shows between cell backgrounds anywhere in the picture.
4. Watch the launch for a windowed flash: capture at 0.5 second intervals for the first three seconds and check no frame shows a window with a title bar. If one does, the window is created before `cleanup`; move the first application of the mode to where the window is still unmade, and say what was found in the commit.
5. Delete the file, run again, and confirm it opens windowed.
6. With `RL_KEYS`, open the settings screen from the title and capture it; then from the Esc menu in a run and capture it. In both, the frame is closed, the row is highlighted, `Fullscreen` and `Off` are on one line, and the hint line reads correctly.
7. `RL_CAPTURE` at the native size, compared with one from `main`'s build as in Task 1 Step 9: identical.

Anything that looks off is fixed here, including things this work did not cause.

- [ ] **Step 7: Gate and commit**

```bash
cargo fmt --all --check && cargo clippy -p foundry --all-targets -- -D warnings && cargo test -p foundry
git add examples/foundry
git commit -m "Foundry has a settings screen, on its title and in its menu, and opens the way it was left"
```

---

### Task 7: The documentation pass, and everything CI runs

**Files:**
- Create: `docs/guide/src/systems/settings.md`, `docs/design/settings.md`
- Modify: `docs/guide/src/systems/rendering.md`, `docs/guide/src/systems/panels.md` or `controls.md` where the menu's rows are listed, `docs/guide/src/SUMMARY.md`
- Modify: `docs/OVERVIEW.md`, `docs/README.md`, `README.md`, `AGENTS.md` (the layout section's list of design docs), `CHANGELOG.md`

- [ ] **Step 1: See what the checks say first**

```bash
python3 scripts/check-systems.py 2>&1 | tail -40
scripts/check-overview.sh 2>&1 | tail -20
```

Expected: `check-systems.py` reports the pages whose guarded files moved (`rendering.md` for `terminal.rs`, the page guarding `game_menu.rs`, and any guarding `rl-save/src/lib.rs`) and that `FullscreenPlugin`, `SettingsPanel` and `SettingsSavePlugin` have no page; `check-overview.sh` reports the three plugins missing from the overview.

- [ ] **Step 2: Write `docs/design/settings.md`**

The reasoning, from the spec's sections 2 and 3, one sentence per line: why a registry in `rl-bevy`; why words and not indices are remembered; why not `Versioned`; why whole-pixel cells under a fractional zoom and not whole-number zoom; why no resize limit; why not on the web; why the screen draws after `EngineSet::Present`; the three future decisions.
Add it to `docs/README.md`'s index and to the `docs/design/` list in `AGENTS.md`'s layout section, in alphabetical place.

- [ ] **Step 3: Write `docs/guide/src/systems/settings.md`**

The six fixed parts in order, under eighty lines of prose.

- Manifest: plugins `FullscreenPlugin`, `SettingsPanel`, `SettingsSavePlugin`; files `crates/rl-bevy/src/settings.rs`, `crates/rl-render/src/fullscreen.rs`, `crates/rl-ui/src/panel/settings.rs`, `crates/rl-save/src/settings.rs`. Copy the manifest's exact syntax from `controls.md`.
- One paragraph: what a setting is.
- `Turning it on`: `RoguelikePlugins` brings Fullscreen; a game adds `SettingsPanel` and, to remember, `SettingsSavePlugin`.
- `The model`: `Setting`, `Settings`, `add_setting`, `chosen` and `choice`, change detection; written from the type definitions.
- `Using it`: one sentence, then the `settings` anchor from `examples/foundry/src/main.rs`; a second, `remember`, because remembering takes a step the first does not show.
- `The line`: the engine decides how settings are listed, changed and remembered, and owns Fullscreen; the game decides what else is a setting, declares it, and acts on it.
- `Where it lives`: the registry is plain data in `rl-bevy` and tested with no window; the split keeps `rl-ui` free of `rl-save`.

Add it to `SUMMARY.md` beside the other system pages.

- [ ] **Step 4: Bring the moved pages up to date**

For each page `check-systems.py` named, read the diff it prints against the page and fix what became untrue.
`rendering.md` certainly: it must say the grid is laid out for its window and no longer say or imply the camera scales it; add `crates/rl-render/src/layout.rs` to its manifest.

- [ ] **Step 5: The inventory, the changelog and the README**

- `docs/OVERVIEW.md`: `Settings` in the `rl-bevy` section; `FullscreenPlugin` and the layout in `rl-render`'s; `SettingsPanel` in `rl-ui`'s presenter table; `SettingsSavePlugin` in `rl-save`'s; `FullscreenPlugin` in the list of what `RoguelikePlugins` adds.
- `CHANGELOG.md`: read the `Unreleased` lines the six commits added as one entry and merge any that repeat.
- `README.md`'s feature list: one line, settings a player chooses and the game remembers, fullscreen among them, with the terminal sharp at any window size.

- [ ] **Step 6: Bless and run every check**

```bash
scripts/check-systems-style.sh docs/guide/src/systems/settings.md
python3 scripts/check-systems.py --bless settings
python3 scripts/check-systems.py --bless rendering   # and each other page confirmed in Step 4
python3 scripts/check-systems.py
scripts/check-overview.sh
scripts/check-guide.sh
scripts/check-tiers.sh
scripts/check-tiers.sh --wasm
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace > /tmp/claude-settings-test.log 2>&1; tail -30 /tmp/claude-settings-test.log
```

Expected: every script exits 0 and every test passes.
`check-tiers.sh --wasm` matters here: `rl-save` builds for wasm and now holds `settings.rs`.
Read `.github/workflows/ci.yml` and run anything it runs that is not above.
A failure or a flaky test is fixed, whoever caused it.

- [ ] **Step 7: Commit**

```bash
git add docs README.md AGENTS.md CHANGELOG.md
git commit -m "the reference, the design note and the inventory say what settings are, how the terminal fills its window, and why"
```

---

## After the last task

Use superpowers:requesting-code-review over the branch against the spec's section 11, then superpowers:finishing-a-development-branch.
The worktree is removed and the branch deleted once it is merged.
