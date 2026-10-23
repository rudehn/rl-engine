//! Which doors the adventurer has been through.
//!
//! A floor of halls and cells behind doors is explored by opening them, and
//! a door seen from across a room and never walked through is the part of
//! the floor most likely still unseen. Nothing in the engine records that
//! a cell was walked on, only that it was seen, so [`Passed`] does: every
//! door cell the adventurer has stepped onto, per map, kept with the run
//! (it is [`SaveableState`], so a continued run still knows).
//!
//! The mark key, declared in [`input`](crate::input) under "Look", shows
//! every door the map remembers and the record lacks, as the door's own
//! cell washed in a blue that pulses, its glyph left on top, until the next
//! key of any kind is pressed. A door never seen is not marked, since the
//! mark would give away where it is; a door seen and not passed is exactly what the player wants to be
//! reminded of. An open door counts as a door: a door shuts again behind
//! whoever opens it, so which of the two tiles it is says nothing about
//! whether anyone went through.
//!
//! The vault doors and carved doors are tiles of their own and are not
//! these: they are opened by a key or a lever and no one forgets them.

use std::collections::{BTreeMap, BTreeSet};

use atglyph::grid::TileId;
use atglyph::prelude::*;
use atglyph::primitives::Point;
use atglyph::save::SaveableState;
use atglyph::ui::{CursorMap, CursorStyle, Tones};
use bevy::prelude::*;

use crate::input::Binds;

/// Every door cell the adventurer has stepped onto, per map.
///
/// Ordered, so a save writes it the same way every time.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct Passed(BTreeMap<MapId, BTreeSet<Point>>);

impl Passed {
    /// Whether the adventurer has stepped onto the door at `at` of `map`.
    pub fn has(&self, map: MapId, at: Point) -> bool {
        self.0.get(&map).is_some_and(|cells| cells.contains(&at))
    }

    /// Records that the adventurer stepped onto the door at `at` of `map`.
    pub fn record(&mut self, map: MapId, at: Point) {
        self.0.entry(map).or_default().insert(at);
    }
}

impl SaveableState for Passed {
    type Saved = Vec<(u32, Vec<Point>)>;

    fn capture(&self) -> Self::Saved {
        self.0.iter().map(|(map, cells)| (map.0, cells.iter().copied().collect())).collect()
    }

    fn restore(&mut self, saved: Self::Saved) {
        self.0 = saved.into_iter().map(|(map, cells)| (MapId(map), cells.into_iter().collect())).collect();
    }
}

/// The two tiles a door is, shut and open.
#[derive(Resource, Debug, Clone, Copy)]
pub struct DoorTiles {
    shut: TileId,
    open: TileId,
}

impl DoorTiles {
    /// The ids the dungeon's tile registry gave them.
    pub fn new() -> Self {
        let tiles = crate::dungeon::registry();
        Self { shut: tiles.expect("door"), open: tiles.expect("open door") }
    }

    /// The tile of a shut door.
    pub fn shut(&self) -> TileId {
        self.shut
    }

    /// Whether `tile` is a door in either state.
    pub fn is_door(&self, tile: TileId) -> bool {
        tile == self.shut || tile == self.open
    }
}

impl Default for DoorTiles {
    fn default() -> Self {
        Self::new()
    }
}

/// The doors the mark key is showing, empty when it is not.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct DoorMarks(pub Vec<Point>);

/// Notes a door the adventurer stepped onto. In `TurnSet::React`.
///
/// A step is what is recorded, not a bump: walking into a shut door only
/// opens it, and the player may turn back from the doorway.
pub fn note_doors_passed(
    mut stepped: MessageReader<Stepped>,
    player: Query<(), With<Player>>,
    map: Res<WorldMap>,
    doors: Res<DoorTiles>,
    mut passed: ResMut<Passed>,
) {
    for step in stepped.read() {
        if player.get(step.actor).is_ok() && map.tile(step.to).is_some_and(|t| doors.is_door(t)) {
            passed.record(map.current(), step.to);
        }
    }
}

/// The doors of the current map that were seen and not walked through.
pub fn unpassed(map: &WorldMap, knowledge: &Knowledge, doors: &DoorTiles, passed: &Passed) -> Vec<Point> {
    let here = map.current();
    map.window_tiles().cells().filter(|p| knowledge.is_explored(*p) && map.tile(*p).is_some_and(|t| doors.is_door(t)) && !passed.has(here, *p)).collect()
}

/// What the mark key reads to say which doors are not yet passed.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Remembered<'w> {
    map: Res<'w, WorldMap>,
    knowledge: Res<'w, Knowledge>,
    doors: Res<'w, DoorTiles>,
    passed: Res<'w, Passed>,
}

/// The mark key shows the doors, and the next key of any kind puts them
/// away: the same key again hides them, and any other does what it does.
/// Silent while a screen is up, which has its own use for keys.
pub fn mark_doors(
    input: Res<ButtonInput<KeyCode>>,
    keys: ControlInput,
    binds: Res<Binds>,
    modals: Res<Modals>,
    seen: Remembered,
    mut marks: ResMut<DoorMarks>,
) {
    let pressed = input.get_just_pressed().any(|k| !is_modifier(*k));
    if !marks.0.is_empty() && pressed {
        marks.0.clear();
        if keys.just_pressed(binds.mark_doors) {
            return;
        }
    }
    if modals.any_open() || !keys.just_pressed(binds.mark_doors) {
        return;
    }
    marks.0 = unpassed(&seen.map, &seen.knowledge, &seen.doors, &seen.passed);
}

fn is_modifier(key: KeyCode) -> bool {
    matches!(
        key,
        KeyCode::ShiftLeft
            | KeyCode::ShiftRight
            | KeyCode::ControlLeft
            | KeyCode::ControlRight
            | KeyCode::AltLeft
            | KeyCode::AltRight
            | KeyCode::SuperLeft
            | KeyCode::SuperRight
    )
}

/// The tone a door not passed is washed in, declared in a bright azure by
/// [`UndermerePlugin`](crate::plugin::UndermerePlugin): no door, wall or
/// floor wears a blue like it, so the mark cannot be read as part of the map.
pub const MARK_TONE: &str = "door mark";
/// The tone the wash breathes toward, a paler azure, so the pulse is seen
/// on a still frame as well as in motion.
pub const MARK_PULSE_TONE: &str = "door mark glow";

/// The colours of the two tones, in `0..=1`.
pub const MARK_COLORS: [(f32, f32, f32); 2] = [(0.1, 0.38, 0.78), (0.2, 0.62, 1.0)];

/// How a door not passed is marked: its own cell washed in azure that
/// breathes, the door's gold glyph left on top. Not ticks on the cells
/// beside it, which a still frame reads as map glyphs.
fn style(tones: Option<&Tones>) -> CursorStyle {
    let find = |name: &str, fallback| tones.and_then(|t| t.get(name)).unwrap_or(fallback);
    CursorStyle::glow(find(MARK_TONE, Tones::SELECT)).breathing(Some(find(MARK_PULSE_TONE, Tones::NOTICE)))
}

/// Draws the marks over the map. In `PresentSet::Overlay`.
pub fn draw_door_marks(
    marks: Res<DoorMarks>,
    terminal: Option<ResMut<Terminal>>,
    palette: Option<Res<Palette>>,
    tones: Option<Res<Tones>>,
    map: CursorMap,
    time: Res<Time>,
) {
    let (Some(mut terminal), Some(palette)) = (terminal, palette) else { return };
    for cell in &marks.0 {
        map.mark(&mut terminal, *cell, style(tones.as_deref()), &palette, time.elapsed_secs());
    }
}

#[cfg(test)]
mod tests {
    use atglyph::testing::{KeyScriptPlugin, press};

    use super::*;
    use crate::testing::{arena_on, continued};

    /// Two doors beside the adventurer, east and south, both seen, and a
    /// third far to the north, walled in, so it is out of sight whatever
    /// the floor round it is: on open ground a door twelve steps off is in
    /// sight, as it came to be when the floor's layout moved.
    fn doors_about() -> (App, Point, [Point; 3]) {
        let (mut app, _me, here) = arena_on(6, 1);
        app.add_plugins(KeyScriptPlugin);
        let cells = [here.offset(1, 0), here.offset(0, 1), here.offset(0, -12)];
        crate::testing::lay_floor(&mut app, &cells);
        let walled: Vec<Point> = atglyph::primitives::geometry::square(cells[2], 1).filter(|p| *p != cells[2]).collect();
        crate::testing::wall_off(&mut app, &walled);
        let shut = app.world().resource::<DoorTiles>().shut;
        let mut map = app.world_mut().resource_mut::<WorldMap>();
        for cell in cells {
            map.set_tile(cell, shut);
        }
        let mut knowledge = app.world_mut().resource_mut::<Knowledge>();
        knowledge.mark(cells[0]);
        knowledge.mark(cells[1]);
        crate::testing::settle(&mut app);
        (app, here, cells)
    }

    fn here(app: &App) -> MapId {
        app.world().resource::<WorldMap>().current()
    }

    #[test]
    fn a_door_stepped_through_is_recorded_and_a_door_only_opened_is_not() {
        let (mut app, _, [east, south, _]) = doors_about();
        press(&mut app, KeyCode::KeyL);
        assert!(!app.world().resource::<Passed>().has(here(&app), east), "walking into a shut door only opens it");
        press(&mut app, KeyCode::KeyL);
        let map = here(&app);
        let passed = app.world().resource::<Passed>();
        assert!(passed.has(map, east), "the step onto it is what counts");
        assert!(!passed.has(map, south), "and the one beside it was not walked");
    }

    #[test]
    fn a_door_passed_is_still_passed_after_a_continued_run() {
        let (mut app, _, [east, ..]) = doors_about();
        press(&mut app, KeyCode::KeyL);
        press(&mut app, KeyCode::KeyL);
        let map = here(&app);
        atglyph::save::save_run(app.world_mut()).expect("the run is written down");
        let text = app.world().resource::<Saves>().load(crate::save::SLOT).expect("the slot reads").expect("a save");
        let again = continued(&text);
        assert!(again.world().resource::<Passed>().has(map, east), "the record came back with the run");
    }

    #[test]
    fn the_mark_key_shows_only_doors_seen_and_not_passed_until_the_next_key() {
        let (mut app, _, [east, south, north]) = doors_about();
        press(&mut app, KeyCode::KeyL);
        press(&mut app, KeyCode::KeyL);
        press(&mut app, KeyCode::KeyM);
        let marks = app.world().resource::<DoorMarks>().0.clone();
        assert!(marks.contains(&south), "seen and not passed: {marks:?}");
        assert!(!marks.contains(&east), "passed: {marks:?}");
        assert!(!marks.contains(&north), "never seen, so not given away: {marks:?}");
        press(&mut app, KeyCode::Period);
        assert!(app.world().resource::<DoorMarks>().0.is_empty(), "the next key puts them away");
    }
}
