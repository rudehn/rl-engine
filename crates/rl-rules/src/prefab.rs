//! Prefabs: a piece of a place, drawn as rows of glyphs and what each
//! glyph stands for.
//!
//! A prefab file is [`PrefabDef`], read once by [`load`], which resolves
//! every name the legend carries against whatever registries the game
//! lends the load: a prop from `props.ron`, a monster or a role from the
//! game's own, an item by name or a tag drawn at a band. A cell holds
//! exactly one thing, a tile or a [`Slot`], never both, because a legend
//! that let a glyph be a wall and a monster's post at once could put a
//! guard where nothing can stand, and the place would only find out when
//! it tried to draw it.
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
//! //     Item(item: "item", count: 1)          that item; count is a number
//! //                                           or a range, default 1
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
//!     ground: "deck",
//!     rows: [
//!         "#######",
//!         "#s.A.s#",
//!         "#..w..#",
//!         "##.b.##",
//!     ],
//!     legend: {
//!         '#': Tile("bulkhead"),
//!         '.': Tile("deck"),
//!         'A': Prop("armory locker"),
//!         'w': Item(tag: "weapon", band: 2),
//!         's': Monster(role: "sentry", band: 1),
//!         'b': Monster(role: "brute", band: 2),
//!     },
//! )
//! ```

use std::collections::BTreeMap;

use rl_core::Id;
use rl_grid::{TileId, TileRegistry};
use serde::Deserialize;

use crate::content::{ContentError, Named};
use crate::names::Names;
use crate::prop::{ContentRoll, CountRon, PropDef, PropId, read_stock};
use crate::role::{RoleDef, RoleId};

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
/// `names`, and painting `ground` and every `Tile` glyph from `tiles`.
///
/// Reports every problem in the file at once rather than the first: a
/// mistyped tile, prop, item, tag, monster or role name, a row of the
/// wrong width, a glyph in a row that names nothing, a legend entry that
/// names nothing in any row, and a ground missing or unfit to stand on.
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

    let mut reported_unknown = std::collections::BTreeSet::new();
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
            LegendRon::Item { item, tag, count, band } => match read_stock(item.as_deref(), tag.as_deref(), *band, names) {
                Ok(what) => {
                    let (min, max) = count.map(CountRon::range).unwrap_or((1, 1));
                    if min > max {
                        errors.push(format!("{}: '{c}' is written as {min} to {max}, which is no range at all", a.name));
                    }
                    slots.insert(c, Slot::Item(ContentRoll { what, min, max, band: *band }));
                }
                Err(e) => errors.push(format!("{}: '{c}': {e}", a.name)),
            },
            LegendRon::Monster { monster, role, band } => {
                let pick = match (monster.as_deref(), role.as_deref()) {
                    (Some(name), None) if *band != 0 => {
                        errors.push(format!("{}: '{c}': {name:?} is a fixed monster, drawn from no band, so a band offset means nothing on it", a.name));
                        None
                    }
                    (Some(name), None) => match names.id::<M>(name) {
                        Ok(id) => Some(Pick::Kind(id)),
                        Err(e) => {
                            errors.push(format!("{}: '{c}': {e}", a.name));
                            None
                        }
                    },
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
    if !slots.is_empty() {
        match (&a.ground, ground) {
            (None, _) => errors.push(format!("{}: has slots and no `ground` to stand them on", a.name)),
            (Some(name), Some(id)) if !tiles.get(id).walkable => {
                errors.push(format!("{}: its ground {name:?} is a tile nothing can stand on", a.name));
            }
            _ => {}
        }
    }

    if !errors.is_empty() {
        return Err(ContentError::Invalid(errors));
    }
    Ok(PrefabDef { name: a.name, ground, rows: a.rows, tiles: tile_map, slots })
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
        tiles.register(TileProps::wall("bulkhead")).unwrap();
        tiles.register(TileProps::floor("deck")).unwrap();
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
        ground: "deck",
        rows: ["#####", "#bLw#", "##W##", "##m##"],
        legend: {
            '#': Tile("bulkhead"),
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
        assert_eq!(def.ground, w.tiles.id("deck"));
        assert_eq!(def.tile('#'), w.tiles.id("bulkhead"));
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
            ground: "bulkhead",
            rows: ["#?#", "#b"],
            legend: {
                '#': Tile("bulkhed"),
                'b': Monster(monster: "warden", band: 1),
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
            "bulkhed",
            "a band offset means nothing",
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
        assert!(read(&w, r###"(name: "wall", rows: ["##"], legend: { '#': Tile("bulkhead") })"###).is_ok());
    }
}
