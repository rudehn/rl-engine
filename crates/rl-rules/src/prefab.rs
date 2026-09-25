//! Prefabs: a piece of a place, drawn as rows of glyphs and what each
//! glyph stands for.
//!
//! A prefab file is [`PrefabDef`], read once by [`load`], which resolves
//! every name the legend carries against whatever registries the game
//! lends the load: a prop from `props.ron`, a monster or a role from the
//! game's own, and a tag drawn at a band. A slot's fixed item stays a
//! name, the same as a container's fixed contents, because a game's
//! items are its own registry and only the game can say whether one
//! exists; that check waits for play to begin. A cell holds exactly one
//! thing, a tile or a [`Slot`], never both, because a legend that let a
//! glyph be a wall and a monster's post at once could put a monster
//! where nothing can stand, and the place would only find out when it
//! tried to draw it.
//!
//! A slot is one of four things: a prop, an item row (the same
//! [`ContentRoll`] a container's contents are, so the two can never
//! disagree about what one means), a monster, fixed or drawn for a role at
//! a band, or a [`Slot::Mark`], a position the file leaves to the game,
//! found by its glyph among a place's spots once stamped.
//!
//! What this module does not hold is the terrain half. Turning [`rows`
//! ](PrefabDef::rows), [`tile`](PrefabDef::tile) and [`slot`
//! ](PrefabDef::slot) into a grid of tiles and entities, with rotation and
//! mirroring, is `rl-mapgen`'s, a tier above this one; this crate names no
//! mapgen type and keeps no dependency on it, so a prefab file loads and
//! every one of its mistakes is reported with no map, no `App` and no grid
//! anywhere in the test.
//!
//! ```text
//! // A prefab: a piece of a place, drawn as rows of glyphs, and what each
//! // glyph stands for.
//! //
//! // Every field:
//! //   name:   the prefab's name, which a mapgen chain asks for
//! //   ground: the tile painted under every slot; required when the legend
//! //           has any slot, and it must be a tile a monster can stand on
//! //   rows:   the piece, one string per row, all the same width; a space is
//! //           left as the map had it, and every other glyph must be in the
//! //           legend
//! //   legend: glyph to what it stands for, one of
//! //     Tile("name")                          a tile from the game's tiles
//! //     Prop("name")                          a prop from props.ron
//! //     Item(item: "item", count: 1)          that item, by the name the
//! //                                           game's own items answer to;
//! //                                           count is 1 or a range,
//! //                                           count: (fewest, most),
//! //                                           default 1
//! //     Item(tag: "tag", count: 1, band: 0)   drawn from the item table by
//! //                                           tag at the place's band plus
//! //                                           band; count is separate draws
//! //     Monster(monster: "monster")           that monster
//! //     Monster(role: "role", band: 0)        drawn from the spawn table
//! //                                           among the role's monsters, at
//! //                                           the place's band plus band
//! //     Mark                                  a position the game finds by
//! //                                           this glyph and fills itself
//! //   A monster at a slot holds that cell as its post.
//! (
//!     name: "guarded locker",
//!     ground: "floor",
//!     rows: [
//!         "#######",
//!         "#s.A.s#",
//!         "#..w..#",
//!         "##.b.##",
//!     ],
//!     legend: {
//!         '#': Tile("wall"),
//!         '.': Tile("floor"),
//!         'A': Prop("locker"),
//!         'w': Item(tag: "weapon", band: 2),
//!         's': Monster(role: "sentry", band: 1),
//!         'b': Monster(role: "brute", band: 2),
//!     },
//! )
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::ops::RangeInclusive;

use rl_core::Id;
use rl_grid::{TileId, TileRegistry};
use serde::Deserialize;

use crate::affix::TagDef;
use crate::content::{BandedTable, ContentError, Named, Registry};
use crate::loot::LootTable;
use crate::names::Names;
use crate::prop::{ContentRoll, CountRon, PropDef, PropId, Stock, read_stock};
use crate::role::{self, RoleDef, RoleId};

/// Which monster a slot holds: that one, or one drawn for a role.
pub enum Pick<M> {
    /// Always this monster, on every floor.
    Kind(Id<M>),
    /// Drawn from the spawn table among the role's members.
    Role(RoleId<M>),
}

impl<M> std::fmt::Debug for Pick<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Pick::Kind(id) => f.debug_tuple("Kind").field(id).finish(),
            Pick::Role(id) => f.debug_tuple("Role").field(id).finish(),
        }
    }
}

/// What a glyph that is not a tile stands for.
pub enum Slot<M> {
    /// A prop of this kind.
    Prop(PropId),
    /// Items, as a container row says them: a named item in a count, or
    /// that many draws of a tag at the place's band plus the offset.
    Item(ContentRoll),
    /// A monster, which holds the slot's cell as its post.
    Monster {
        /// Which.
        pick: Pick<M>,
        /// Bands deeper than the place a role is drawn at; nought for a
        /// named monster.
        band: i32,
    },
    /// A position left to the game, which finds it among the place's
    /// spots by its glyph and puts there what only it knows how to.
    Mark,
}

impl<M> std::fmt::Debug for Slot<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Slot::Prop(id) => f.debug_tuple("Prop").field(id).finish(),
            Slot::Item(roll) => f.debug_tuple("Item").field(roll).finish(),
            Slot::Monster { pick, band } => f.debug_struct("Monster").field("pick", pick).field("band", band).finish(),
            Slot::Mark => f.write_str("Mark"),
        }
    }
}

/// A prefab, as its file says it.
pub struct PrefabDef<M> {
    /// The name a mapgen chain asks for.
    pub name: String,
    /// The tile painted under every slot. Always present when the legend
    /// has a slot, and always a tile a monster can stand on.
    pub ground: Option<TileId>,
    rows: Vec<String>,
    tiles: BTreeMap<char, TileId>,
    slots: BTreeMap<char, Slot<M>>,
}

impl<M> std::fmt::Debug for PrefabDef<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrefabDef")
            .field("name", &self.name)
            .field("ground", &self.ground)
            .field("rows", &self.rows)
            .field("tiles", &self.tiles)
            .field("slots", &self.slots)
            .finish()
    }
}

impl<M> Named for PrefabDef<M> {
    fn name(&self) -> &str {
        &self.name
    }
}

impl<M> PrefabDef<M> {
    /// The piece, one string per row, all the same width.
    pub fn rows(&self) -> Vec<&str> {
        self.rows.iter().map(String::as_str).collect()
    }

    /// The tile `glyph` paints, if it names one.
    pub fn tile(&self, glyph: char) -> Option<TileId> {
        self.tiles.get(&glyph).copied()
    }

    /// The slot `glyph` names, if it names one.
    pub fn slot(&self, glyph: char) -> Option<&Slot<M>> {
        self.slots.get(&glyph)
    }

    /// Every slot, by glyph.
    pub fn slots(&self) -> impl Iterator<Item = (char, &Slot<M>)> {
        self.slots.iter().map(|(&c, s)| (c, s))
    }
}

/// A prefab as authored, before its names are resolved.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PrefabRon {
    name: String,
    #[serde(default)]
    ground: Option<String>,
    rows: Vec<String>,
    legend: BTreeMap<char, LegendRon>,
}

/// One legend entry as authored: what a glyph paints or stands for.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
enum LegendRon {
    Tile(String),
    Prop(String),
    Item {
        #[serde(default)]
        item: Option<String>,
        #[serde(default)]
        tag: Option<String>,
        #[serde(default)]
        count: Option<CountRon>,
        #[serde(default)]
        band: i32,
    },
    Monster {
        #[serde(default)]
        monster: Option<String>,
        #[serde(default)]
        role: Option<String>,
        #[serde(default)]
        band: i32,
    },
    Mark,
}

/// Loads one prefab from RON, resolving every name in its legend through
/// `names`, and painting `ground` and every `Tile` glyph from `tiles`. A
/// fixed item's name is left unchecked: a game's items are its own
/// registry, the same as a container's fixed contents, and only the
/// game can say whether one exists, so that check waits for play to
/// begin.
///
/// Reports every problem in the file at once rather than the first: a
/// mistyped tile, prop, tag, monster or role name, a row of the wrong
/// width, a glyph in a row that names nothing, a legend entry that names
/// nothing in any row, and a ground missing or unfit to stand on.
pub fn load<M: 'static>(text: &str, tiles: &TileRegistry, names: &Names<'_>) -> Result<PrefabDef<M>, ContentError> {
    let options = ron::options::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
    let a: PrefabRon = options.from_str(text).map_err(|e| ContentError::Parse(e.to_string()))?;
    let mut errors = Vec::new();

    let width = a.rows.first().map(|r| r.chars().count()).unwrap_or(0);
    if width == 0 {
        errors.push(format!("{}: has no rows", a.name));
    }
    for (y, row) in a.rows.iter().enumerate() {
        let w = row.chars().count();
        if y > 0 && w != width {
            errors.push(format!("{}: row {y} is {w} wide, the first row is {width}", a.name));
        }
    }

    let mut reported_unknown = BTreeSet::new();
    for (y, row) in a.rows.iter().enumerate() {
        for c in row.chars() {
            if c != ' ' && !a.legend.contains_key(&c) && reported_unknown.insert(c) {
                errors.push(format!("{}: '{c}' in row {y} is not in the legend", a.name));
            }
        }
    }

    for &c in a.legend.keys() {
        if c == ' ' {
            errors.push(format!("{}: a space is the map left as it was, and cannot be given a meaning", a.name));
        }
        if !a.rows.iter().any(|row| row.contains(c)) {
            errors.push(format!("{}: '{c}' is in the legend and in no row", a.name));
        }
    }

    let mut tile_map = BTreeMap::new();
    let mut slots = BTreeMap::new();
    for (&c, entry) in &a.legend {
        match entry {
            LegendRon::Tile(name) => match tiles.id(name) {
                Some(id) => {
                    tile_map.insert(c, id);
                }
                None => errors.push(format!("{}: '{c}' paints unknown tile {name:?}", a.name)),
            },
            LegendRon::Prop(name) => match names.id::<PropDef>(name) {
                Ok(id) => {
                    slots.insert(c, Slot::Prop(id));
                }
                Err(e) => errors.push(format!("{}: '{c}': {e}", a.name)),
            },
            LegendRon::Item { item, tag, count, band } => {
                // Checked whether or not the stock resolves, so a slot with
                // two mistakes names both in one load.
                let (min, max) = count.map(CountRon::range).unwrap_or((1, 1));
                if min > max {
                    errors.push(format!("{}: '{c}' is written as {min} to {max}, which is no range at all", a.name));
                }
                match read_stock(item.as_deref(), tag.as_deref(), *band, names) {
                    Ok(what) => {
                        slots.insert(c, Slot::Item(ContentRoll { what, min, max, band: *band }));
                    }
                    Err(e) => errors.push(format!("{}: '{c}': {e}", a.name)),
                }
            }
            LegendRon::Monster { monster, role, band } => {
                let pick = match (monster.as_deref(), role.as_deref()) {
                    (Some(name), None) => {
                        if *band != 0 {
                            errors.push(format!("{}: '{c}': {name:?} is a fixed monster, drawn from no band, so a band offset means nothing on it", a.name));
                        }
                        match names.id::<M>(name) {
                            Ok(id) => Some(Pick::Kind(id)),
                            Err(e) => {
                                errors.push(format!("{}: '{c}': {e}", a.name));
                                None
                            }
                        }
                    }
                    (None, Some(role)) => match names.id::<RoleDef<M>>(role) {
                        Ok(id) => Some(Pick::Role(id)),
                        Err(e) => {
                            errors.push(format!("{}: '{c}': {e}", a.name));
                            None
                        }
                    },
                    _ => {
                        errors.push(format!("{}: '{c}' names a `monster` or a `role`, and exactly one of them", a.name));
                        None
                    }
                };
                if let Some(pick) = pick {
                    slots.insert(c, Slot::Monster { pick, band: *band });
                }
            }
            LegendRon::Mark => {
                slots.insert(c, Slot::Mark);
            }
        }
    }

    let ground = match &a.ground {
        Some(name) => match tiles.id(name) {
            Some(id) => Some(id),
            None => {
                errors.push(format!("{}: its ground is unknown tile {name:?}", a.name));
                None
            }
        },
        None => None,
    };
    // Whether the legend asks for a ground is decided from what was
    // authored, not from what resolved: a legend whose one slot entry is
    // a typo still needs a ground to stand it on, and saying so beside
    // the typo is how both get fixed in one pass instead of one at a
    // time.
    let has_slots = a.legend.values().any(|entry| !matches!(entry, LegendRon::Tile(_)));
    if has_slots && a.ground.is_none() {
        errors.push(format!("{}: has slots and no `ground` to stand them on", a.name));
    }
    if let (Some(name), Some(id)) = (&a.ground, ground)
        && !tiles.get(id).walkable
    {
        errors.push(format!("{}: its ground {name:?} is a tile nothing can stand on", a.name));
    }

    if !errors.is_empty() {
        return Err(ContentError::Invalid(errors));
    }
    Ok(PrefabDef { name: a.name, ground, rows: a.rows, tiles: tile_map, slots })
}

/// The registries and tables a coverage report draws through: the same
/// ones a game hands the engine at play, so a report answers no question
/// a run would answer differently.
pub struct Sources<'a, M, I> {
    /// Every role a monster slot may name.
    pub roles: &'a Registry<RoleDef<M>>,
    /// The spawn table a role is drawn from.
    pub monsters: &'a BandedTable<Id<M>>,
    /// The loot table a tagged item slot is drawn from.
    pub items: &'a LootTable<I>,
    /// Every tag, so a tagged slot's tag has a name to print.
    pub tags: &'a Registry<TagDef>,
}

/// Whether a slot's draw at a band lands on the band it asked for, falls
/// back to the nearest band that has something, or finds nothing at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// The band asked for has something.
    Exact,
    /// Nothing at the band asked for; the nearest band that has something.
    Fallback(i32),
    /// No band, asked or fallen back to, has anything at all.
    Empty,
}

/// One drawn slot's reach at every band a [`Coverage`] report covers.
pub struct CoverageRow {
    /// The prefab it is drawn in.
    pub prefab: String,
    /// The glyph it stamps.
    pub glyph: char,
    /// The role's or the tag's name, with the slot's own band offset
    /// appended when it is not nought: `"brute +1"`.
    pub what: String,
    /// The band asked and what the draw there reaches, one pair for every
    /// band the report covers.
    pub reach: Vec<(i32, Reach)>,
}

/// A report of what every drawn slot in a set of prefabs finds at every
/// band, item by tag and monster by role. Loading a prefab file only
/// checks that a name resolves; whether the band it is stamped at ever has
/// anything to draw is a question only a game's own spawn and loot tables
/// answer, and no startup check can ask it before those tables exist. This
/// runs the same fallback a real draw takes, against no map and no run.
pub struct Coverage {
    /// The bands it covers.
    pub bands: RangeInclusive<i32>,
    /// One row per drawn slot, in the order its prefab was given and then
    /// glyph order.
    pub rows: Vec<CoverageRow>,
}

impl Coverage {
    /// Every band where a drawn slot finds nothing at all: `(prefab,
    /// glyph, band)`, so a content author can see what to fix before a run
    /// ever reaches it.
    pub fn empties(&self) -> Vec<(&str, char, i32)> {
        self.rows
            .iter()
            .flat_map(|r| r.reach.iter().filter(|(_, reach)| matches!(reach, Reach::Empty)).map(move |(band, _)| (r.prefab.as_str(), r.glyph, *band)))
            .collect()
    }

    /// A table, one column per band and one row per drawn slot: `✓` where
    /// the draw lands on the band asked, `~N` where it falls back to band
    /// `N`, `✗` where it finds nothing.
    pub fn render(&self) -> String {
        let mut prefabs: Vec<&str> = Vec::new();
        for row in &self.rows {
            if !prefabs.contains(&row.prefab.as_str()) {
                prefabs.push(&row.prefab);
            }
        }
        let label_width = prefabs.iter().map(|p| p.chars().count()).chain(self.rows.iter().map(|r| 4 + r.what.chars().count())).max().unwrap_or(0) + 2;
        let mut out = String::new();
        for (i, &prefab) in prefabs.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.push_str(&format!("{prefab:<label_width$}"));
            for band in self.bands.clone() {
                out.push_str(&format!("{band:>4}"));
            }
            out.push('\n');
            for row in self.rows.iter().filter(|r| r.prefab == prefab) {
                let label = format!("  {} {}", row.glyph, row.what);
                out.push_str(&format!("{label:<label_width$}"));
                for (_, reach) in &row.reach {
                    let cell = match reach {
                        Reach::Exact => "✓".to_string(),
                        Reach::Fallback(band) => format!("~{band}"),
                        Reach::Empty => "✗".to_string(),
                    };
                    out.push_str(&format!("{cell:>4}"));
                }
                out.push('\n');
            }
        }
        out
    }
}

/// What `asked` reaches, from what `role::band_for` or `LootTable::band_for`
/// answered: nothing when it is `None`, the band asked for when it answers
/// with itself, else the band it fell back to.
fn reach_of(asked: i32, reached: Option<i32>) -> Reach {
    match reached {
        None => Reach::Empty,
        Some(band) if band == asked => Reach::Exact,
        Some(band) => Reach::Fallback(band),
    }
}

/// `name`, with a slot's own band offset appended when it is not nought:
/// `"brute +1"`, `"weapon -2"`, plain `"weapon"` at nought.
fn slot_name(name: &str, offset: i32) -> String {
    if offset == 0 { name.to_string() } else { format!("{name} {offset:+}") }
}

/// Runs every drawn slot in `prefabs`, an item slot naming a tag and a
/// monster slot naming a role, against `sources` at every band in `bands`:
/// the same fallback a real draw takes, in the order the prefabs are given
/// and then glyph order. A slot with a fixed item or a fixed monster draws
/// nothing to report; only what a game's tables might not cover is worth
/// checking.
pub fn coverage<'p, M: 'p, I: Copy>(prefabs: impl IntoIterator<Item = &'p PrefabDef<M>>, sources: &Sources<'_, M, I>, bands: RangeInclusive<i32>) -> Coverage {
    let mut rows = Vec::new();
    for def in prefabs {
        for (glyph, slot) in def.slots() {
            let (what, slot_reach): (String, Vec<(i32, Reach)>) = match slot {
                Slot::Item(ContentRoll { what: Stock::Tag(tag), band: offset, .. }) => {
                    let what = slot_name(sources.tags.name(*tag), *offset);
                    let slot_reach = bands
                        .clone()
                        .map(|b| {
                            let asked = b + offset;
                            (b, reach_of(asked, sources.items.band_for(*tag, asked)))
                        })
                        .collect();
                    (what, slot_reach)
                }
                Slot::Monster { pick: Pick::Role(role), band: offset } => {
                    let role_def = sources.roles.get(*role);
                    let what = slot_name(&role_def.name, *offset);
                    let slot_reach = bands
                        .clone()
                        .map(|b| {
                            let asked = b + offset;
                            (b, reach_of(asked, role::band_for(sources.monsters, role_def, asked)))
                        })
                        .collect();
                    (what, slot_reach)
                }
                _ => continue,
            };
            rows.push(CoverageRow { prefab: def.name.clone(), glyph, what, reach: slot_reach });
        }
    }
    Coverage { bands, rows }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::affix::TagDef;
    use crate::content::Registry;
    use crate::prop::{PropDef, Stock};
    use rl_grid::{TileProps, TileRegistry};

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
        let beasts = Registry::from_defs(vec![Beast("heavy"), Beast("warden")]).unwrap();
        let roles = crate::role::load(r#"{ "brute": ["heavy"] }"#, &Names::new().with("monster", &beasts)).unwrap();
        let tags = Registry::from_defs(vec![TagDef::new("weapon")]).unwrap();
        let props = crate::prop::load(r#"[(name: "locker", glyph: 'L', color: (r: 255, g: 255, b: 255), blocks: true)]"#, &Names::new().tags(&tags)).unwrap();
        World { tiles, beasts, roles, props, tags }
    }

    fn read(w: &World, text: &str) -> Result<PrefabDef<Beast>, ContentError> {
        let names = Names::new().tags(&w.tags).with("prop", &w.props).with("monster", &w.beasts).with("role", &w.roles);
        load(text, &w.tiles, &names)
    }

    const GUARDED: &str = r######"(
        name: "guarded locker",
        ground: "floor",
        rows: ["#####", "#bLw#", "##W##", "##m##"],
        legend: {
            '#': Tile("wall"),
            'L': Prop("locker"),
            'w': Item(tag: "weapon", band: 2),
            'b': Monster(role: "brute", band: 1),
            'W': Monster(monster: "warden"),
            'm': Mark,
        },
    )"######;

    #[test]
    fn a_prefab_file_resolves_every_tile_and_slot_its_legend_names() {
        let w = world();
        let def = read(&w, GUARDED).unwrap();
        assert_eq!(def.name, "guarded locker");
        assert_eq!(def.ground, w.tiles.id("floor"));
        assert_eq!(def.tile('#'), w.tiles.id("wall"));
        assert!(matches!(def.slot('L'), Some(Slot::Prop(p)) if *p == w.props.expect("locker")));
        assert!(matches!(def.slot('w'), Some(Slot::Item(ContentRoll { what: Stock::Tag(_), min: 1, max: 1, band: 2 }))));
        assert!(matches!(def.slot('b'), Some(Slot::Monster { pick: Pick::Role(r), band: 1 }) if *r == w.roles.expect("brute")));
        assert!(matches!(def.slot('W'), Some(Slot::Monster { pick: Pick::Kind(k), band: 0 }) if *k == w.beasts.expect("warden")));
        assert!(matches!(def.slot('m'), Some(Slot::Mark)));
        assert_eq!(def.rows(), vec!["#####", "#bLw#", "##W##", "##m##"]);
    }

    #[test]
    fn a_prefab_file_refuses_every_mistake_in_it_at_once() {
        let w = world();
        let text = r##"(
            name: "broken",
            ground: "wall",
            rows: ["#?#", "#b"],
            legend: {
                '#': Tile("wallx"),
                'b': Monster(monster: "wardn", band: 1),
                'r': Monster(role: "bruiser"),
                'x': Monster(monster: "heavy", role: "brute"),
                'i': Item(item: "knife", band: 1),
                't': Item(tag: "wepon"),
                'p': Prop("lockr"),
                ' ': Mark,
            },
        )"##;
        let err = read(&w, text).unwrap_err().to_string();
        for said in [
            "row 1 is 2 wide",
            "'?' in row 0 is not in the legend",
            "wallx",
            "a band offset means nothing",
            "unknown monster \"wardn\"",
            "bruiser",
            "exactly one of them",
            "wepon",
            "lockr",
            "a space",
            "nothing can stand on",
            "is in the legend and in no row",
        ] {
            assert!(err.contains(said), "{said:?} missing from: {err}");
        }
    }

    #[test]
    fn a_prefab_with_slots_and_no_ground_is_refused() {
        let w = world();
        let err = read(&w, r#"(name: "bare", rows: ["m"], legend: { 'm': Mark })"#).unwrap_err().to_string();
        assert!(err.contains("no `ground`"), "{err}");
    }

    #[test]
    fn a_prefab_of_tiles_alone_needs_no_ground() {
        let w = world();
        assert!(read(&w, r###"(name: "tiles only", rows: ["##"], legend: { '#': Tile("wall") })"###).is_ok());
    }

    #[test]
    fn a_prefab_whose_only_slot_entry_fails_still_demands_a_ground() {
        let w = world();
        let err = read(&w, r#"(name: "unground", rows: ["p"], legend: { 'p': Prop("lockr") })"#).unwrap_err().to_string();
        assert!(err.contains("lockr"), "{err}");
        assert!(err.contains("no `ground`"), "{err}");
    }

    #[test]
    fn a_prefab_with_no_rows_is_refused() {
        let w = world();
        let err = read(&w, r#"(name: "empty", rows: [], legend: {})"#).unwrap_err().to_string();
        assert!(err.contains("has no rows"), "{err}");
    }

    #[test]
    fn an_unknown_ground_is_refused() {
        let w = world();
        let err = read(&w, r##"(name: "bad ground", ground: "hull", rows: ["#"], legend: { '#': Tile("wall") })"##).unwrap_err().to_string();
        assert!(err.contains("its ground is unknown tile"), "{err}");
    }

    #[test]
    fn a_ground_that_cannot_be_stood_on_is_refused_even_with_no_slots() {
        let w = world();
        let err = read(&w, r##"(name: "solid", ground: "wall", rows: ["#"], legend: { '#': Tile("wall") })"##).unwrap_err().to_string();
        assert!(err.contains("nothing can stand on"), "{err}");
    }

    #[test]
    fn an_item_slots_count_written_backwards_is_refused() {
        let w = world();
        let err =
            read(&w, r#"(name: "backwards range", ground: "floor", rows: ["i"], legend: { 'i': Item(item: "rope", count: (5, 2)) })"#).unwrap_err().to_string();
        assert!(err.contains("5 to 2, which is no range at all"), "{err}");
    }

    /// An item slot refused for its tag still has its count checked, the
    /// same as a container's row: every mistake in a file at once.
    #[test]
    fn an_item_slot_refused_for_its_stock_still_has_its_count_checked() {
        let w = world();
        let err =
            read(&w, r#"(name: "two mistakes", ground: "floor", rows: ["i"], legend: { 'i': Item(tag: "wepon", count: (5, 2)) })"#).unwrap_err().to_string();
        for said in ["wepon", "5 to 2, which is no range at all"] {
            assert!(err.contains(said), "{said:?} in {err}");
        }
    }

    fn heavy_on(w: &World, lo: i32, hi: i32) -> BandedTable<Id<Beast>> {
        BandedTable::new(vec![crate::content::BandedEntry::new(w.beasts.expect("heavy")).bands(lo, hi)])
    }

    fn weapons(w: &World, lo: i32, hi: i32) -> crate::loot::LootTable<u32> {
        let weapon = w.tags.expect("weapon");
        crate::loot::LootTable::new(vec![crate::loot::LootRow { item: 1u32, bands: (lo, hi), weight: 1, group: (1, 1), tags: vec![weapon] }], |n| n.to_string())
    }

    #[test]
    fn coverage_reads_exact_where_a_slot_finds_its_band_and_fallback_where_it_does_not() {
        let w = world();
        let def = read(&w, GUARDED).unwrap();
        let (monsters, items) = (heavy_on(&w, 3, 8), weapons(&w, 1, 10));
        let sources = Sources { roles: &w.roles, monsters: &monsters, items: &items, tags: &w.tags };
        let report = coverage([&def], &sources, 1..=10);
        let brute = report.rows.iter().find(|r| r.glyph == 'b').unwrap();
        assert_eq!(brute.what, "brute +1");
        assert!(matches!(brute.reach[0], (1, Reach::Fallback(3))), "band 1 asks at 2, nothing until 3");
        assert!(matches!(brute.reach[4], (5, Reach::Exact)));
        assert!(matches!(brute.reach[9], (10, Reach::Fallback(8))), "band 10 asks at 11, the deepest is 8");
        let weapon = report.rows.iter().find(|r| r.glyph == 'w').unwrap();
        assert!(matches!(weapon.reach[9], (10, Reach::Fallback(10))));
        assert!(report.rows.iter().all(|r| r.glyph != 'W' && r.glyph != 'L' && r.glyph != 'm'), "named, prop and mark slots draw nothing");
        assert!(report.empties().is_empty());
    }

    #[test]
    fn coverage_names_every_slot_that_can_draw_nothing_at_all() {
        let w = world();
        let def = read(&w, GUARDED).unwrap();
        let (monsters, items) = (BandedTable::new(Vec::new()), weapons(&w, 1, 10));
        let sources = Sources { roles: &w.roles, monsters: &monsters, items: &items, tags: &w.tags };
        let report = coverage([&def], &sources, 1..=3);
        assert_eq!(report.empties(), vec![("guarded locker", 'b', 1), ("guarded locker", 'b', 2), ("guarded locker", 'b', 3)]);
    }

    #[test]
    fn a_rendered_report_has_a_column_per_band_and_a_row_per_drawn_slot() {
        let w = world();
        let def = read(&w, GUARDED).unwrap();
        let (monsters, items) = (heavy_on(&w, 3, 8), weapons(&w, 1, 10));
        let sources = Sources { roles: &w.roles, monsters: &monsters, items: &items, tags: &w.tags };
        let text = coverage([&def], &sources, 1..=10).render();
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("guarded locker") && lines[0].trim_end().ends_with("10"), "{text}");
        assert!(lines.iter().any(|l| l.trim_start().starts_with("b brute +1") && l.contains("~3") && l.contains("~8") && l.contains('✓')), "{text}");
        assert!(lines.iter().any(|l| l.trim_start().starts_with("w weapon +2")), "{text}");
    }
}
