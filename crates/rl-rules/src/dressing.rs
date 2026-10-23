//! Dressings: what fills an area of any size, as a legend and the rules
//! for where its things go rather than rows.
//!
//! A prefab is drawn at one size, so a room twice its size around it is a
//! bare ring with a cluster in the middle. A dressing is the prefab's
//! [`Legend`] and, for each glyph, a [`Spread`] and a density: along the
//! walls, in the corners, in the center, in rows with aisles between, or
//! scattered, at a share of the cells that rule offers, with a share of
//! the floor repainted. One dressing then dresses a small room, a long
//! hall and a cave alike.
//!
//! A dressing file is a list of [`DressingDef`], read once by [`load`],
//! which resolves every name its legends carry exactly as a prefab's load
//! does, through the same code, and reports every mistake in the file at
//! once. A slot means what it means in a prefab, so a dressing's props,
//! items and monsters fill through the same slots, streams and checks.
//!
//! [`DistrictDressings`] is the weighted list an area no stamp took is
//! dressed from, by the district it was grown in, with a weight of nothing
//! at all, since an empty room is a room too and leaves floor clear to
//! fight on. [`check_districts`] holds such lists to the dressing file and
//! a layout, and [`check_pool`] a pool's dressed entries to the file.
//!
//! What this module does not hold is the dressing itself: which cells a
//! rule offers in an area, keeping off its openings and never cutting it
//! in two, is `rl-mapgen`'s `Dress`, and [`grow::dress`](crate::grow::dress)
//! hands the file to it.
//! A rule loads as the mapgen [`Thing`] that pass lays, its [`Spread`]
//! read straight from the file and what it stands as, a tile or a slot,
//! from the legend, and a district's list is the mapgen
//! [`DistrictDressings`]; a dressing loads and every mistake in it is
//! reported with no map grown and no `App`.
//!
//! ```text
//! // Dressings: what fills an area of any size, a legend and where its
//! // things go.
//! //
//! // A list of dressings, each with every field:
//! //   name:    the dressing's name, which a pool's Dressed entry and a
//! //            district's list name it by
//! //   legend:  glyph to what it stands for, as a prefab's legend says it:
//! //     Tile("name")                          a tile from the game's tiles
//! //     Prop("name")                          a prop from props.ron
//! //     Item(item: "item", count: 1)          that item; count is 1 or
//! //                                           (fewest, most)
//! //     Item(tag: "tag", count: 1, band: 0)   drawn by tag at the place's
//! //                                           band plus band
//! //     Monster(monster: "monster")           that monster
//! //     Monster(role: "role", band: 0)        drawn for the role at the
//! //                                           place's band plus band
//! //     Mark                                  a position the game fills
//! //   An Entry is refused: a dressing has no wall of its own to be joined
//! //   through. Every glyph of the legend is placed by some rule, and
//! //   every rule places a glyph of the legend.
//! //   place:   the rules, laid in this order, each
//! //     (glyph: 'c', at: Walls, density_pct: 30)
//! //   with at one of
//! //     Walls              a cell with a wall straight beside it, not a
//! //                        corner
//! //     Corners            a cell with walls on two sides at a right angle
//! //     Center             the cells farthest from every wall
//! //     Rows(aisle: 1)     lines along the area's longer side, a cell in
//! //                        from its walls, with aisle cells clear between
//! //                        two lines; aisle is 1 or more
//! //     Scatter            any cell of the area
//! //   and density_pct the share in a hundred, 1 to 100, of the cells the
//! //   rule offers that it fills
//! //   repaint: optional; (tile, pct): this share in a hundred, 1 to 100,
//! //            of the area's floor left clear is repainted with the tile,
//! //            which must be one a body can stand on
//! // A dressing places something or repaints, or both.
//! [
//!     (
//!         name: "barracks",
//!         legend: {
//!             'b': Prop("cot"),
//!             'w': Item(tag: "weapon", band: 1),
//!         },
//!         place: [
//!             (glyph: 'b', at: Walls, density_pct: 40),
//!             (glyph: 'w', at: Corners, density_pct: 50),
//!         ],
//!         repaint: ("rubble", 10),
//!     ),
//! ]
//! ```
//!
//! A district's list, as a game writes it inside its own file:
//!
//! ```text
//! // (district: "halls", nothing: 2, dressings: [("barracks", 3), ("store", 1)])
//! //   district:  a district of the layout
//! //   nothing:   the weight of leaving an area bare; default 0
//! //   dressings: each dressing by name and its weight
//! ```

use std::collections::BTreeMap;

use rl_grid::{TileId, TileRegistry};
use serde::Deserialize;

use crate::content::{ContentError, Named, Registry};
use crate::layout::LayoutDef;
use crate::names::Names;
use crate::pool::Pool;
use crate::prefab::{Legend, LegendRon, Legended, read_legend};
use rl_mapgen::DistrictDressings;
use rl_mapgen::dress::{Spread, Stand, Thing};

/// A dressing, as its file says it.
pub struct DressingDef<M> {
    /// The name a pool and a district's list ask for.
    pub name: String,
    legend: Legend<M>,
    /// The rules, in the order they are laid: an earlier rule's things
    /// are already standing when a later one looks for room. Each stands
    /// as a tile where its legend paints one and as a slot otherwise.
    pub things: Vec<Thing>,
    /// The tile and the share in a hundred of the floor left clear that is
    /// repainted with it, after every thing stands.
    pub repaint: Option<(TileId, u32)>,
}

impl<M> std::fmt::Debug for DressingDef<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DressingDef")
            .field("name", &self.name)
            .field("legend", &self.legend)
            .field("things", &self.things)
            .field("repaint", &self.repaint)
            .finish()
    }
}

impl<M> Named for DressingDef<M> {
    fn name(&self) -> &str {
        &self.name
    }
}

impl<M> Legended<M> for DressingDef<M> {
    fn legend(&self) -> &Legend<M> {
        &self.legend
    }
}

/// A dressing as authored, before its names are resolved.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DressingRon {
    name: String,
    legend: BTreeMap<char, LegendRon>,
    #[serde(default)]
    place: Vec<ThingRon>,
    #[serde(default)]
    repaint: Option<(String, u32)>,
}

/// One rule as authored: a glyph, where it goes, and how much of it, the
/// share in a hundred of the cells `at` offers, so a hall gets more than a
/// closet from one rule. What the glyph stands as is its legend's.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThingRon {
    glyph: char,
    at: Spread,
    density_pct: u32,
}

/// Loads a dressing file, resolving every name in every legend through
/// `names` and every tile from `tiles`, as [`prefab::load`
/// ](crate::prefab::load) resolves a prefab's.
///
/// Reports every problem in the file at once: whatever a prefab's legend
/// refuses, an entry, a glyph placed that the legend does not hold or
/// held and never placed, a density outside 1 to 100, rows with no aisle,
/// a repaint tile unknown or one nothing can stand on or a share outside
/// 1 to 100, a dressing that lays nothing, and two dressings of one name.
pub fn load<M: 'static>(text: &str, tiles: &TileRegistry, names: &Names<'_>) -> Result<Registry<DressingDef<M>>, ContentError> {
    let options = ron::options::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
    let all: Vec<DressingRon> = options.from_str(text).map_err(|e| ContentError::Parse(e.to_string()))?;
    let mut errors = Vec::new();
    let mut defs = Vec::new();
    for d in all {
        let name = &d.name;
        if defs.iter().any(|def: &DressingDef<M>| def.name == *name) {
            errors.push(format!("two dressings are named {name:?}"));
        }
        let legend = read_legend(name, &d.legend, tiles, names, &mut errors);
        for (&c, entry) in &d.legend {
            if matches!(entry, LegendRon::Entry) {
                errors.push(format!("{name}: '{c}' is an entry, and a dressing has no wall of its own to be joined through"));
            }
            if !d.place.iter().any(|t| t.glyph == c) {
                errors.push(format!("{name}: '{c}' is in the legend and placed by no rule"));
            }
        }
        for t in &d.place {
            let c = t.glyph;
            if !d.legend.contains_key(&c) {
                errors.push(format!("{name}: '{c}' is placed and is not in the legend"));
            }
            if !(1..=100).contains(&t.density_pct) {
                errors.push(format!("{name}: '{c}' at {:?} fills {}% of its cells, and a density is a share in a hundred from 1 to 100", t.at, t.density_pct));
            }
            if t.at == (Spread::Rows { aisle: 0 }) {
                errors.push(format!("{name}: '{c}' is laid in rows with no aisle between them, which is a block"));
            }
        }
        let repaint = match &d.repaint {
            Some((tile, pct)) => {
                if !(1..=100).contains(pct) {
                    errors.push(format!("{name}: repaints {pct}% of its floor, and a share in a hundred runs from 1 to 100"));
                }
                match tiles.id(tile) {
                    Some(id) if tiles.get(id).walkable => Some((id, *pct)),
                    Some(_) => {
                        errors.push(format!("{name}: repaints with {tile:?}, a tile nothing can stand on; a repaint dresses the floor and never takes it"));
                        None
                    }
                    None => {
                        errors.push(format!("{name}: repaints with unknown tile {tile:?}"));
                        None
                    }
                }
            }
            None => None,
        };
        if d.place.is_empty() && d.repaint.is_none() {
            errors.push(format!("{name}: lays nothing; an area is left bare by a district's `nothing`, not by an empty dressing"));
        }
        let things = d
            .place
            .iter()
            .map(|t| Thing { glyph: t.glyph, stands: legend.tile(t.glyph).map_or(Stand::Slot, Stand::Tile), at: t.at, density_pct: t.density_pct })
            .collect();
        defs.push(DressingDef { name: d.name, legend, things, repaint });
    }
    if !errors.is_empty() {
        return Err(ContentError::Invalid(errors));
    }
    Registry::from_defs(defs)
}

/// Refuses a pool whose dressed entry names a dressing the file lacks, so
/// a typo is a load failure and not a place that fails to build when the
/// pick is first drawn, which may be deep into a run.
pub fn check_pool<M>(pool: &Pool, dressings: &Registry<DressingDef<M>>) -> Result<(), ContentError> {
    let errors: Vec<String> = pool
        .dressings()
        .filter(|d| dressings.id(d).is_none())
        .map(|d| format!("the pool lays dressing {d:?}, which the dressing file does not hold"))
        .collect();
    if errors.is_empty() { Ok(()) } else { Err(ContentError::Invalid(errors)) }
}

/// Refuses lists that name a dressing the file lacks or a district the
/// layout lacks, give one district two lists, or weigh nothing at all.
/// Every mistake is reported at once.
pub fn check_districts<M>(lists: &[DistrictDressings], dressings: &Registry<DressingDef<M>>, layout: &LayoutDef) -> Result<(), ContentError> {
    let mut errors = Vec::new();
    for (i, list) in lists.iter().enumerate() {
        let district = &list.district;
        if !layout.districts.iter().any(|d| d.name == *district) {
            errors.push(format!("dressings for district {district:?}, which the layout does not have"));
        }
        if lists[..i].iter().any(|l| l.district == *district) {
            errors.push(format!("district {district:?} is given two lists of dressings"));
        }
        for (name, _) in &list.dressings {
            if dressings.id(name).is_none() {
                errors.push(format!("district {district:?} is dressed with {name:?}, which the dressing file does not hold"));
            }
        }
        if u64::from(list.nothing) + list.dressings.iter().map(|(_, w)| u64::from(*w)).sum::<u64>() == 0 {
            errors.push(format!("district {district:?} weighs every choice at nought, so nothing can be drawn"));
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(ContentError::Invalid(errors)) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prefab::{Pick, Reach, Slot, Sources};
    use crate::prop::{PropDef, Stock};
    use crate::role::RoleDef;
    use crate::tag::TagDef;
    use rl_grid::TileProps;

    struct Beast(&'static str);
    impl Named for Beast {
        fn name(&self) -> &str {
            self.0
        }
    }

    struct World {
        tiles: TileRegistry,
        beasts: Registry<Beast>,
        roles: Registry<RoleDef<Beast>>,
        props: Registry<PropDef>,
        tags: Registry<TagDef>,
    }

    fn world() -> World {
        let mut tiles = TileRegistry::new();
        tiles.register(TileProps::wall("wall")).unwrap();
        tiles.register(TileProps::floor("floor")).unwrap();
        tiles.register(TileProps::floor("rubble")).unwrap();
        let beasts = Registry::from_defs(vec![Beast("heavy")]).unwrap();
        let roles = crate::role::load(r#"{ "brute": ["heavy"] }"#, &Names::new().with("monster", &beasts)).unwrap();
        let tags = Registry::from_defs(vec![TagDef::new("weapon")]).unwrap();
        let props = crate::prop::load(r#"[(name: "cot", glyph: 'c', color: (r: 255, g: 255, b: 255), blocks: true)]"#, &Names::new().tags(&tags)).unwrap();
        World { tiles, beasts, roles, props, tags }
    }

    fn read(w: &World, text: &str) -> Result<Registry<DressingDef<Beast>>, ContentError> {
        let names = Names::new().tags(&w.tags).with("prop", &w.props).with("monster", &w.beasts).with("role", &w.roles);
        load(text, &w.tiles, &names)
    }

    const FILE: &str = r#"[
        (
            name: "barracks",
            legend: {
                'c': Prop("cot"),
                'w': Item(tag: "weapon", band: 1),
                'b': Monster(role: "brute"),
                'p': Tile("wall"),
                'm': Mark,
            },
            place: [
                (glyph: 'c', at: Walls, density_pct: 40),
                (glyph: 'w', at: Corners, density_pct: 50),
                (glyph: 'p', at: Rows(aisle: 2), density_pct: 100),
                (glyph: 'b', at: Center, density_pct: 100),
                (glyph: 'm', at: Scatter, density_pct: 5),
            ],
            repaint: ("rubble", 10),
        ),
        (name: "dust", legend: {}, repaint: ("rubble", 30)),
    ]"#;

    #[test]
    fn a_dressing_file_resolves_its_legend_as_a_prefab_s_and_keeps_its_rules_in_order() {
        let w = world();
        let all = read(&w, FILE).unwrap();
        let barracks = all.get(all.expect("barracks"));
        assert!(matches!(barracks.legend().slot('c'), Some(Slot::Prop(_))));
        assert!(matches!(barracks.legend().slot('w'), Some(Slot::Item(r)) if matches!(r.what, Stock::Tag(_)) && r.band == 1));
        assert!(matches!(barracks.legend().slot('b'), Some(Slot::Monster { pick: Pick::Role(_), band: 0 })));
        assert!(matches!(barracks.legend().slot('m'), Some(Slot::Mark)));
        assert_eq!(barracks.legend().tile('p'), w.tiles.id("wall"));
        let order: Vec<(char, Spread)> = barracks.things.iter().map(|t| (t.glyph, t.at)).collect();
        assert_eq!(order, vec![('c', Spread::Walls), ('w', Spread::Corners), ('p', Spread::Rows { aisle: 2 }), ('b', Spread::Center), ('m', Spread::Scatter)]);
        let stands: Vec<Stand> = barracks.things.iter().map(|t| t.stands).collect();
        let wall = Stand::Tile(w.tiles.id("wall").unwrap());
        assert_eq!(stands, vec![Stand::Slot, Stand::Slot, wall, Stand::Slot, Stand::Slot], "a glyph its legend paints stands as the tile, any other as a slot");
        assert_eq!(barracks.repaint, Some((w.tiles.id("rubble").unwrap(), 10)));
        let dust = all.get(all.expect("dust"));
        assert!(dust.things.is_empty() && dust.repaint.is_some(), "a dressing may only repaint");
    }

    #[test]
    fn a_dressing_file_reports_every_mistake_at_once() {
        let w = world();
        let text = r#"[
            (
                name: "bad",
                legend: { 'c': Prop("cto"), 'e': Entry, 'x': Tile("floor"), 'r': Tile("wal") },
                place: [
                    (glyph: 'c', at: Walls, density_pct: 0),
                    (glyph: 'e', at: Scatter, density_pct: 101),
                    (glyph: 'q', at: Center, density_pct: 10),
                    (glyph: 'r', at: Rows(aisle: 0), density_pct: 10),
                ],
                repaint: ("wall", 0),
            ),
            (name: "empty", legend: {}),
            (name: "empty", legend: {}, repaint: ("nowhere", 5)),
        ]"#;
        let Err(ContentError::Invalid(errors)) = read(&w, text) else { panic!("refused") };
        let says = |what: &str| assert!(errors.iter().any(|e| e.contains(what)), "{what:?} is not among {errors:#?}");
        says("bad: 'c': ");
        says("bad: 'r' paints unknown tile \"wal\"");
        says("bad: 'e' is an entry");
        says("bad: 'x' is in the legend and placed by no rule");
        says("bad: 'q' is placed and is not in the legend");
        says("bad: 'c' at Walls fills 0%");
        says("bad: 'e' at Scatter fills 101%");
        says("bad: 'r' is laid in rows with no aisle");
        says("bad: repaints 0% of its floor");
        says("bad: repaints with \"wall\", a tile nothing can stand on");
        says("empty: lays nothing");
        says("two dressings are named \"empty\"");
        says("empty: repaints with unknown tile \"nowhere\"");
    }

    const LAYOUT: &str = r#"(
        shapes: [(name: "room", kind: Rectangle(w: (5, 9), h: (5, 8)), join: Door)],
        districts: [(name: "rooms", shapes: [("room", 1)]), (name: "halls", shapes: [("room", 1)])],
        recipes: [],
    )"#;

    fn pool(dressing: &str) -> Pool {
        let text = format!(
            r#"(
                weights: [("common", 50)],
                tags: [],
                budgets: [(floors: (1, 3), kind: "flavor", count: (1, 1))],
                entries: [(name: "it", kind: "flavor", dressing: "{dressing}", floors: (2, 3), weight: "common", placed: Dressed)],
            )"#
        );
        crate::pool::load::<()>(&text, &crate::layout::load(LAYOUT).unwrap(), &Registry::default(), &Names::new()).unwrap()
    }

    #[test]
    fn a_pool_laying_a_dressing_the_file_lacks_is_refused_and_one_it_holds_is_not() {
        let w = world();
        let all = read(&w, FILE).unwrap();
        assert_eq!(check_pool(&pool("barracks"), &all), Ok(()));
        let Err(ContentError::Invalid(errors)) = check_pool(&pool("barack"), &all) else { panic!("refused") };
        assert_eq!(errors, vec!["the pool lays dressing \"barack\", which the dressing file does not hold"]);
    }

    #[test]
    fn district_lists_are_held_to_the_dressings_and_the_layout() {
        let w = world();
        let all = read(&w, FILE).unwrap();
        let layout = crate::layout::load(LAYOUT).unwrap();
        let list = |district: &str, nothing: u32, dressings: &[(&str, u32)]| DistrictDressings {
            district: district.into(),
            nothing,
            dressings: dressings.iter().map(|(n, w)| (n.to_string(), *w)).collect(),
        };
        let good = [list("rooms", 2, &[("barracks", 3), ("dust", 1)]), list("halls", 0, &[("dust", 1)])];
        assert_eq!(check_districts(&good, &all, &layout), Ok(()));
        let bad = [list("rooms", 1, &[("barack", 1)]), list("rooms", 1, &[]), list("caves", 0, &[("dust", 0)])];
        let Err(ContentError::Invalid(errors)) = check_districts(&bad, &all, &layout) else { panic!("refused") };
        assert_eq!(
            errors,
            vec![
                "district \"rooms\" is dressed with \"barack\", which the dressing file does not hold",
                "district \"rooms\" is given two lists of dressings",
                "dressings for district \"caves\", which the layout does not have",
                "district \"caves\" weighs every choice at nought, so nothing can be drawn",
            ]
        );
        let parsed: DistrictDressings = ron::from_str(r#"(district: "halls", nothing: 2, dressings: [("dust", 1)])"#).unwrap();
        assert_eq!(parsed, list("halls", 2, &[("dust", 1)]));
    }

    /// A dressing's drawn slots are reported as a prefab's are, on their
    /// own and at the floors a pool lays them.
    #[test]
    fn coverage_reports_a_dressing_s_drawn_slots_as_a_prefab_s() {
        let w = world();
        let all = read(&w, FILE).unwrap();
        let monsters = crate::content::BandedTable::new(vec![crate::content::BandedEntry::new(w.beasts.expect("heavy")).bands(3, 8)]);
        let weapon = w.tags.expect("weapon");
        let items = crate::loot::LootTable::new(vec![crate::loot::LootRow { item: 1u32, bands: (1, 10), weight: 1, group: (1, 1), tags: vec![weapon] }], |n| {
            n.to_string()
        });
        let sources = Sources { roles: &w.roles, monsters: &monsters, items: &items, tags: &w.tags };
        let report = crate::prefab::coverage(all.iter().map(|(_, d)| d), &sources, 1..=3);
        let glyphs: Vec<(&str, char)> = report.rows.iter().map(|r| (r.prefab.as_str(), r.glyph)).collect();
        assert_eq!(glyphs, vec![("barracks", 'b'), ("barracks", 'w')], "the role and the tag; the prop, the mark and the tile draw nothing");
        assert_eq!(report.empties(), Vec::new());
        assert!(matches!(report.rows[0].reach[0], (1, Reach::Fallback(3))));

        let pool = pool("barracks");
        let report = crate::pool::dressing_coverage(&pool, &all, &sources);
        assert_eq!(report.bands, 2..=3, "the floors the pool lays it at");
        assert_eq!(report.rows.len(), 2);
    }
}
