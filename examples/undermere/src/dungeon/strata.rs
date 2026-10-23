//! The strata: named ranges of floors, the direction the Undermere runs in.
//!
//! Depth is time. Near the top are the kingdom's streets and grounds,
//! below them its tombs and foundations, deeper still the places that
//! stood before the first stone, and at the bottom the oldest thing of
//! all, which drowned it. A stratum is Undermere's own data, a name and a
//! range of floors, and not an engine idea: the floors stay a tour, each
//! window drawing from its own biomes (`assets/biomes.ron` groups its rows
//! by stratum and [`check_within`] holds every biome to one), and no rule
//! changes a whole floor.
//!
//! The player meets them three ways. Crossing into one for the first time
//! logs [`Stratum::crossing`] ahead of the floor's arrival line, the vitals
//! name it beside the floor on one row ([`depth_line`]), and the
//! innkeeper's rumours hint at what lies deeper and older
//! (`assets/rumours.ron`).

use bevy::prelude::*;
use rl_engine::prelude::*;

use super::{FLOORS, TOWN, floor_of, map_of};

/// One stratum: a name, the floors it spans, and the line said on first
/// crossing into it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stratum {
    /// What the vitals and the rumours call it.
    pub name: &'static str,
    /// The first and last floor, both included.
    pub floors: (u32, u32),
    /// Said the first time the adventurer arrives on its first floor, ahead
    /// of the floor's own arrival line.
    pub crossing: &'static str,
}

impl Stratum {
    /// Whether `floor` is one of its floors.
    pub fn holds(&self, floor: u32) -> bool {
        (self.floors.0..=self.floors.1).contains(&floor)
    }
}

/// The strata, shallowest first, covering floors one to ten with no gap
/// and no overlap, which `check` holds.
pub const STRATA: [Stratum; 6] = [
    Stratum { name: "The Drowned Town", floors: (1, 2), crossing: "You pass through the gate, into the drowned town." },
    Stratum { name: "The Palace and its Grounds", floors: (3, 4), crossing: "You come among the palace and its grounds, sunk whole." },
    Stratum { name: "Beneath the Kingdom", floors: (5, 6), crossing: "You pass beneath the kingdom." },
    Stratum { name: "Before the Kingdom", floors: (7, 8), crossing: "You pass into a time before the kingdom." },
    Stratum { name: "The Oldest Deep", floors: (9, 9), crossing: "You come to the oldest deep." },
    Stratum { name: "The Thrones", floors: (10, 10), crossing: "You come to the thrones." },
];

/// The stratum `floor` is in, `None` for the town and for any floor the
/// Undermere does not have.
pub fn stratum_of(floor: u32) -> Option<&'static Stratum> {
    STRATA.iter().find(|s| s.holds(floor))
}

/// The stratum an arrival on `map` crosses into: the one `map` is a floor of,
/// when no other floor of it has been visited yet, `visited` saying which
/// have. Not "whichever floor is its first", so a run that comes to a
/// stratum another way than by its top stair, a photograph begun on a floor
/// or a fall, still hears it said once.
pub fn crossed_into(map: MapId, visited: impl Fn(MapId) -> bool) -> Option<&'static Stratum> {
    let floor = floor_of(map)?;
    let stratum = stratum_of(floor)?;
    (stratum.floors.0..=stratum.floors.1).filter(|f| *f != floor).all(|f| !visited(map_of(f))).then_some(stratum)
}

/// Checks that [`STRATA`] cover floors one to [`FLOORS`] in order with no
/// gap and no overlap. Panics naming the break.
pub(super) fn check() {
    let mut next = 1;
    for s in &STRATA {
        assert!(s.floors.0 == next && s.floors.0 <= s.floors.1, "strata: {:?} starts at floor {}, not {next}", s.name, s.floors.0);
        next = s.floors.1 + 1;
    }
    assert!(next == FLOORS + 1, "strata: end at floor {}, not {FLOORS}", next - 1);
}

/// Checks that the floors a biome may be drawn on lie inside one stratum,
/// so a biome belongs to exactly one. Returns what is wrong, for the
/// biome loader to name it by.
pub(super) fn check_within(floors: (u32, u32)) -> Result<(), String> {
    match stratum_of(floors.0) {
        Some(s) if s.holds(floors.1) => Ok(()),
        _ => Err(format!("floors {floors:?} are not inside one stratum")),
    }
}

/// The key the depth readout's facet is pushed under.
const READOUT: &str = "stratum";

/// What the vitals' depth row says on `map`: `3 · The Palace and its
/// Grounds` on a floor, the floor and its stratum on one row, and
/// `Mereholt` in the town, so the row is there wherever the adventurer is
/// and what is under it never moves on going down the stair. `None` for a
/// place that is neither.
pub fn depth_line(map: MapId) -> Option<String> {
    if map == TOWN {
        return Some("Mereholt".to_string());
    }
    let floor = floor_of(map)?;
    stratum_of(floor).map(|s| format!("{floor} · {}", s.name))
}

/// Says on the vitals where the adventurer is, by [`depth_line`], in the
/// tone of the other readings, so the depth is always on the screen and
/// says what age it is. In `ViewSet::Annotate`, after the engine's
/// collector and ahead of the gold carried; absent vitals, as in a test
/// that draws none, says nothing.
pub fn name_the_stratum(vitals: Option<ResMut<VitalsView>>, map: Option<Res<WorldMap>>) {
    let (Some(mut vitals), Some(world_map)) = (vitals, map) else { return };
    let Some(line) = depth_line(world_map.current()) else { return };
    vitals.facets.push(Facet::new(READOUT, line).toned(Tones::TEXT));
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_core::RunSeed;

    use super::*;
    use crate::stairs::place_line;
    use crate::testing::{adventurer, arrive, headless, said, settle};

    /// The strata run from floor one to floor ten with no gap and no overlap,
    /// and name every floor.
    #[test]
    fn the_strata_cover_floors_one_to_ten_in_order_and_name_every_floor() {
        check();
        for floor in 1..=FLOORS {
            let holding: Vec<&str> = STRATA.iter().filter(|s| s.holds(floor)).map(|s| s.name).collect();
            assert_eq!(holding.len(), 1, "floor {floor} is in exactly one stratum: {holding:?}");
        }
        assert!(stratum_of(0).is_none() && stratum_of(FLOORS + 1).is_none(), "Mereholt is no stratum's");
        assert_eq!(stratum_of(3).map(|s| s.name), Some("The Palace and its Grounds"));
        assert_eq!(stratum_of(5).map(|s| s.name), Some("Beneath the Kingdom"));
    }

    /// A biome that straddles two strata, or runs past the last floor, is
    /// refused, so a biome belongs to exactly one.
    #[test]
    fn a_biome_across_two_strata_is_refused() {
        assert!(check_within((3, 4)).is_ok() && check_within((9, 9)).is_ok() && check_within((10, 10)).is_ok());
        assert!(check_within((2, 3)).is_err(), "floors two and three are two strata");
        assert!(check_within((7, 9)).is_err());
        assert!(check_within((10, 11)).is_err());
    }

    /// Crossing into a stratum for the first time logs its line, ahead of the
    /// floor's own, once: the next floor of it says nothing of the kind, nor
    /// does coming back to the first, and the next stratum says its own.
    #[test]
    fn crossing_into_a_stratum_says_its_line_once_ahead_of_the_floors_own() {
        let mut app = headless(RunSeed(1));
        arrive(&mut app, map_of(3));
        let draws = app.world().resource::<crate::light::BiomeDraws>().clone();
        let lines = said(&app);
        let palace = lines.iter().position(|l| l == STRATA[1].crossing).unwrap_or_else(|| panic!("the crossing is said: {lines:#?}"));
        let floor = lines.iter().position(|l| *l == place_line(map_of(3), &draws)).expect("the floor's own line");
        assert_eq!(palace + 1, floor, "the crossing directly ahead of the arrival line: {lines:#?}");
        arrive(&mut app, map_of(4));
        arrive(&mut app, map_of(3));
        arrive(&mut app, map_of(5));
        let lines = said(&app);
        for stratum in &STRATA[1..3] {
            assert_eq!(lines.iter().filter(|l| *l == stratum.crossing).count(), 1, "{:?} said once: {lines:#?}", stratum.name);
        }
        assert!(lines.iter().any(|l| l == "You pass beneath the kingdom."), "the line the design names: {lines:#?}");
        assert!(!lines.iter().any(|l| l == STRATA[3].crossing), "floor seven is not reached: {lines:#?}");
    }

    /// The vitals name the floor and its stratum on one row, and Mereholt
    /// in the town, so the row is always there.
    #[test]
    fn the_vitals_name_the_floor_and_its_stratum_on_one_row_and_mereholt_in_the_town() {
        let mut app = headless(RunSeed(1));
        app.add_plugins(VitalsViewPlugin);
        adventurer(&mut app);
        settle(&mut app);
        let readout =
            |app: &App| -> Vec<String> { app.world().resource::<VitalsView>().facets.iter().filter(|f| f.key == READOUT).map(|f| f.text.clone()).collect() };
        assert_eq!(readout(&app), ["Mereholt"], "the town by its name");
        for (floor, line) in [(1, "1 · The Drowned Town"), (3, "3 · The Palace and its Grounds"), (10, "10 · The Thrones")] {
            arrive(&mut app, map_of(floor));
            settle(&mut app);
            assert_eq!(readout(&app), [line], "floor {floor}");
        }
    }
}
