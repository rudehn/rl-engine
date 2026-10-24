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
//! An [`entry`](PrefabDef::entry) is neither: a cell of the piece's outer
//! wall where it may be joined when it is grown as an area of its own, and
//! the map as it was until it is. One that is not on the outer wall, or
//! does not have the inside of the piece straight across from the outside,
//! is refused, since a way in that opens onto the piece's own wall or onto
//! nothing is no way in. Whether a piece has any is asked where it is
//! grown, since a piece only ever stamped needs none.
//!
//! A slot is one of four things: a prop, an item row (the same
//! [`ContentRoll`] a container's contents are, so the two can never
//! disagree about what one means), a monster, fixed or drawn for a role at
//! a band, or a [`Slot::Mark`], a position the file leaves to the game,
//! found by its glyph among a place's spots once stamped.
//!
//! A prefab also says how far its drawing may be turned and mirrored, as
//! a [`Rotation`] and a [`Mirror`]: the drawing is the one that knows an
//! inscription reads backwards mirrored, where the chain that stamps it
//! only knows where its door must meet a corridor. Neither is required,
//! and a prefab that says nothing may be laid any way its stamp allows.
//!
//! [`Reached`] gathers everything a set of prefabs or dressings names
//! through its slots, with the contents of every prop it stands: what a
//! game reads, beside its own sources, to find the content nothing lays.
//!
//! What this module does not hold is the terrain half. Turning [`rows`
//! ](PrefabDef::rows), [`tile`](PrefabDef::tile) and [`slot`
//! ](PrefabDef::slot) into a grid of tiles and entities, with rotation and
//! mirroring, is `atglyph-mapgen`'s, a tier 1 crate this one depends on, and
//! the [`Rotation`] and [`Mirror`] a file writes are its values, read straight
//! from the file; a prefab file loads and every one of its mistakes is
//! reported with no map grown and no `App`. [`PrefabLibrary`] is where the
//! two meet: it holds a game's prefabs, dressings and roles and cuts each
//! into the keyed piece or dressing a chain lays, and finds a stamped
//! mark's slot again from its key.
//!
//! ```text
//! // A prefab: a piece of a place, drawn as rows of glyphs, and what each
//! // glyph stands for.
//! //
//! // Every field:
//! //   name:   the prefab's name, which a mapgen chain asks for
//! //   ground: the tile painted under every slot; required when the legend
//! //           has any slot, and whenever it is given, slots or none, it
//! //           must be a tile a monster can stand on
//! //   turn:   optional, Quarter if left out: how far the piece may be
//! //           turned wherever it is laid, one of
//! //             None      never
//! //             Half      as drawn or a half-turn
//! //             Quarter   any of the four quarter-turns
//! //   mirror: optional, Both if left out: how it may be mirrored, one of
//! //             None      never
//! //             LeftRight left to right
//! //             UpDown    top to bottom
//! //             Both      either, and both together, the half-turn
//! //           A chain that stamps it may hold it to fewer facings, never
//! //           more; a piece meant for the edge of a map is drawn with its
//! //           back to the south
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
//! //     Entry                                 a cell of the piece's outer wall
//! //                                           where a piece grown as its own
//! //                                           area may be joined; left as the
//! //                                           map had it until it is, and on
//! //                                           the outer wall, with the inside
//! //                                           of the piece straight across
//! //   A monster at a slot holds that cell as its post.
//! (
//!     name: "guarded locker",
//!     ground: "floor",
//!     turn: Quarter,
//!     mirror: LeftRight,
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

use atglyph_core::Id;
use atglyph_grid::{TileId, TileRegistry};
use atglyph_mapgen::BuildError;
use atglyph_mapgen::accrete::Join;
use atglyph_mapgen::dress::Dressing;
use atglyph_mapgen::prefab::{Mirror, Prefab, PrefabCell, PrefabShape, Rotation};
use serde::Deserialize;

use crate::content::{BandedTable, ContentError, Named, Registry};
use crate::dressing::DressingDef;
use crate::loot::LootTable;
use crate::names::Names;
use crate::prop::{ContentRoll, CountRon, PropDef, PropId, Stock, read_stock};
use crate::role::{self, RoleDef, RoleId};
use crate::tag::{TagDef, TagId};

/// Which monster a slot holds: that one, or one drawn for a role.
pub enum SlotPick<M> {
    /// Always this monster, on every floor.
    Kind(Id<M>),
    /// Drawn from the spawn table among the role's members.
    Role(RoleId<M>),
}

impl<M> std::fmt::Debug for SlotPick<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SlotPick::Kind(id) => f.debug_tuple("Kind").field(id).finish(),
            SlotPick::Role(id) => f.debug_tuple("Role").field(id).finish(),
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
        pick: SlotPick<M>,
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

/// What each glyph of a file stands for, resolved: a tile, a slot or an
/// entry. A prefab's rows and a dressing's rules both read one, so a
/// glyph means the same in either and both fill through the same slots.
pub struct Legend<M> {
    tiles: BTreeMap<char, TileId>,
    slots: BTreeMap<char, Slot<M>>,
    entries: BTreeSet<char>,
}

impl<M> std::fmt::Debug for Legend<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Legend").field("tiles", &self.tiles).field("slots", &self.slots).field("entries", &self.entries).finish()
    }
}

impl<M> Legend<M> {
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

    /// Whether `glyph` marks an entry: a cell of the outer wall where the
    /// piece may be joined when it is grown as an area of its own.
    pub fn entry(&self, glyph: char) -> bool {
        self.entries.contains(&glyph)
    }
}

/// A file whose glyphs a [`Legend`] gives a meaning: a prefab, or a
/// dressing. What reads only the legend, a [`coverage`] report or a check
/// of what a slot names, takes either.
pub trait Legended<M>: Named {
    /// What each glyph stands for.
    fn legend(&self) -> &Legend<M>;
}

/// A prefab, as its file says it.
pub struct PrefabDef<M> {
    /// The name a mapgen chain asks for.
    pub name: String,
    /// The tile painted under every slot. Always present when the legend
    /// has a slot, and always a tile a monster can stand on.
    pub ground: Option<TileId>,
    /// How far its drawing may be turned.
    pub turn: Rotation,
    /// How its drawing may be mirrored.
    pub mirror: Mirror,
    rows: Vec<String>,
    legend: Legend<M>,
}

impl<M> std::fmt::Debug for PrefabDef<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrefabDef")
            .field("name", &self.name)
            .field("ground", &self.ground)
            .field("turn", &self.turn)
            .field("mirror", &self.mirror)
            .field("rows", &self.rows)
            .field("legend", &self.legend)
            .finish()
    }
}

impl<M> Named for PrefabDef<M> {
    fn name(&self) -> &str {
        &self.name
    }
}

impl<M> Legended<M> for PrefabDef<M> {
    fn legend(&self) -> &Legend<M> {
        &self.legend
    }
}

impl<M> PrefabDef<M> {
    /// The piece, one string per row, all the same width.
    pub fn rows(&self) -> Vec<&str> {
        self.rows.iter().map(String::as_str).collect()
    }

    /// The tile `glyph` paints, if it names one.
    pub fn tile(&self, glyph: char) -> Option<TileId> {
        self.legend.tile(glyph)
    }

    /// The slot `glyph` names, if it names one.
    pub fn slot(&self, glyph: char) -> Option<&Slot<M>> {
        self.legend.slot(glyph)
    }

    /// Every slot, by glyph.
    pub fn slots(&self) -> impl Iterator<Item = (char, &Slot<M>)> {
        self.legend.slots()
    }

    /// Whether `glyph` marks an entry: a cell of the outer wall where the
    /// piece may be joined when it is grown as an area of its own.
    pub fn entry(&self, glyph: char) -> bool {
        self.legend.entry(glyph)
    }
}

/// A prefab as authored, before its names are resolved.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PrefabRon {
    name: String,
    #[serde(default)]
    ground: Option<String>,
    #[serde(default)]
    turn: Rotation,
    #[serde(default)]
    mirror: Mirror,
    rows: Vec<String>,
    legend: BTreeMap<char, LegendRon>,
}

/// One legend entry as authored: what a glyph paints or stands for.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) enum LegendRon {
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
    Entry,
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
/// nothing in any row, a ground missing or unfit to stand on, and an entry
/// off the outer wall or with no inside straight across from it.
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

    let legend = read_legend(&a.name, &a.legend, tiles, names, &mut errors);
    // Where the outside is can only be worked out on rows of one width.
    if width > 0 && a.rows.iter().all(|r| r.chars().count() == width) {
        entries_on_the_wall(&a.name, &a.rows, &legend.entries, &mut errors);
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
    let has_slots = a.legend.values().any(|entry| !matches!(entry, LegendRon::Tile(_) | LegendRon::Entry));
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
    Ok(PrefabDef { name: a.name, ground, turn: a.turn, mirror: a.mirror, rows: a.rows, legend })
}

/// Resolves every entry of a legend as authored, reporting each name that
/// does not resolve and each slot written wrong into `errors` against the
/// file called `file`; what resolves is kept, so a file with two mistakes
/// names both in one load.
pub(crate) fn read_legend<M: 'static>(
    file: &str,
    legend: &BTreeMap<char, LegendRon>,
    tiles: &TileRegistry,
    names: &Names<'_>,
    errors: &mut Vec<String>,
) -> Legend<M> {
    let mut tile_map = BTreeMap::new();
    let mut slots = BTreeMap::new();
    let mut entries = BTreeSet::new();
    for (&c, entry) in legend {
        match entry {
            LegendRon::Tile(name) => match tiles.id(name) {
                Some(id) => {
                    tile_map.insert(c, id);
                }
                None => errors.push(format!("{file}: '{c}' paints unknown tile {name:?}")),
            },
            LegendRon::Prop(name) => match names.id::<PropDef>(name) {
                Ok(id) => {
                    slots.insert(c, Slot::Prop(id));
                }
                Err(e) => errors.push(format!("{file}: '{c}': {e}")),
            },
            LegendRon::Item { item, tag, count, band } => {
                // Checked whether or not the stock resolves, so a slot with
                // two mistakes names both in one load.
                let (min, max) = count.map(CountRon::range).unwrap_or((1, 1));
                if min > max {
                    errors.push(format!("{file}: '{c}' is written as {min} to {max}, which is no range at all"));
                }
                match read_stock(item.as_deref(), tag.as_deref(), *band, names) {
                    Ok(what) => {
                        slots.insert(c, Slot::Item(ContentRoll { what, min, max, band: *band }));
                    }
                    Err(e) => errors.push(format!("{file}: '{c}': {e}")),
                }
            }
            LegendRon::Monster { monster, role, band } => {
                let pick = match (monster.as_deref(), role.as_deref()) {
                    (Some(name), None) => {
                        if *band != 0 {
                            errors.push(format!("{file}: '{c}': {name:?} is a fixed monster, drawn from no band, so a band offset means nothing on it"));
                        }
                        match names.id::<M>(name) {
                            Ok(id) => Some(SlotPick::Kind(id)),
                            Err(e) => {
                                errors.push(format!("{file}: '{c}': {e}"));
                                None
                            }
                        }
                    }
                    (None, Some(role)) => match names.id::<RoleDef<M>>(role) {
                        Ok(id) => Some(SlotPick::Role(id)),
                        Err(e) => {
                            errors.push(format!("{file}: '{c}': {e}"));
                            None
                        }
                    },
                    _ => {
                        errors.push(format!("{file}: '{c}' names a `monster` or a `role`, and exactly one of them"));
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
            LegendRon::Entry => {
                entries.insert(c);
            }
        }
    }
    Legend { tiles: tile_map, slots, entries }
}

/// Reports each cell of an entry glyph that is not a way in: one with no
/// side on the outside of the piece, or none with the inside straight
/// across from it.
///
/// The outside is every space reachable from beyond the rows through
/// spaces, and the inside every other cell not touching it, corners
/// included: the cells a piece grown on its own encloses within its wall.
/// `atglyph-mapgen`'s grown piece works out its body by the same rule.
fn entries_on_the_wall(name: &str, rows: &[String], entries: &BTreeSet<char>, errors: &mut Vec<String>) {
    let grid: Vec<Vec<char>> = rows.iter().map(|r| r.chars().collect()).collect();
    let (h, w) = (grid.len() as i32, grid[0].len() as i32);
    let at = |x: i32, y: i32| (0..w).contains(&x) && (0..h).contains(&y);
    let mut outside = vec![vec![false; w as usize]; h as usize];
    let mut stack: Vec<(i32, i32)> = (0..w).flat_map(|x| [(x, 0), (x, h - 1)]).chain((0..h).flat_map(|y| [(0, y), (w - 1, y)])).collect();
    while let Some((x, y)) = stack.pop() {
        if !at(x, y) || outside[y as usize][x as usize] || grid[y as usize][x as usize] != ' ' {
            continue;
        }
        outside[y as usize][x as usize] = true;
        stack.extend([(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]);
    }
    let out = |x: i32, y: i32| !at(x, y) || outside[y as usize][x as usize];
    let inside = |x: i32, y: i32| at(x, y) && !out(x, y) && (-1..=1).all(|dy| (-1..=1).all(|dx| !out(x + dx, y + dy)));
    for (y, row) in grid.iter().enumerate() {
        for (x, c) in row.iter().enumerate() {
            if !entries.contains(c) {
                continue;
            }
            let (x, y) = (x as i32, y as i32);
            let sides = [(1, 0), (-1, 0), (0, 1), (0, -1)];
            if !sides.iter().any(|(dx, dy)| out(x + dx, y + dy)) {
                errors.push(format!("{name}: '{c}' at ({x}, {y}) is not on the piece's outer wall, so nothing outside could be joined to it"));
            } else if !sides.iter().any(|(dx, dy)| out(x + dx, y + dy) && inside(x - dx, y - dy)) {
                errors.push(format!("{name}: '{c}' at ({x}, {y}) has no inside of the piece straight across from the outside, so it opens onto nothing"));
            }
        }
    }
}

/// The registries and tables a coverage report draws through: the same
/// ones a game hands the engine at play, so a report answers no question
/// a run would answer differently.
///
/// Both tables are read at the same band for a column, since a report has
/// one band per column: that assumes the game numbers its loot bands and
/// its spawn bands alike, as a game does that uses a floor's number for both.
/// A game whose `ItemMaker::band` and `ActorMaker::band` answer one place
/// differently reads each kind of slot's rows against its own numbering.
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
pub enum BandFallback {
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
    /// band the report covers, or for those its piece is laid at in a
    /// pool's report.
    pub reach: Vec<(i32, BandFallback)>,
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
            .flat_map(|r| r.reach.iter().filter(|(_, reach)| matches!(reach, BandFallback::Empty)).map(move |(band, _)| (r.prefab.as_str(), r.glyph, *band)))
            .collect()
    }

    /// A table, one column per band and one row per drawn slot: `✓` where
    /// the draw lands on the band asked, `~N` where it falls back to band
    /// `N`, `✗` where it finds nothing, and blank at a band the row was
    /// not drawn at.
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
                for band in self.bands.clone() {
                    let cell = match row.reach.iter().find(|(b, _)| *b == band).map(|(_, reach)| reach) {
                        Some(BandFallback::Exact) => "✓".to_string(),
                        Some(BandFallback::Fallback(band)) => format!("~{band}"),
                        Some(BandFallback::Empty) => "✗".to_string(),
                        None => String::new(),
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
fn reach_of(asked: i32, reached: Option<i32>) -> BandFallback {
    match reached {
        None => BandFallback::Empty,
        Some(band) if band == asked => BandFallback::Exact,
        Some(band) => BandFallback::Fallback(band),
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
/// and then glyph order. A dressing's slots are a prefab's, so a report
/// takes the [`DressingDef`]s as well. A
/// slot with a fixed item or a fixed monster draws nothing to report; only
/// what a game's tables might not cover is worth checking.
///
/// One band per column serves item and monster slots alike, which assumes
/// the game's item bands and monster bands agree; see [`Sources`].
pub fn coverage<'p, M: 'p, I: Copy, D: Legended<M> + 'p>(
    prefabs: impl IntoIterator<Item = &'p D>,
    sources: &Sources<'_, M, I>,
    bands: RangeInclusive<i32>,
) -> Coverage {
    let mut rows = Vec::new();
    for def in prefabs {
        for (glyph, slot) in def.legend().slots() {
            let (what, slot_reach): (String, Vec<(i32, BandFallback)>) = match slot {
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
                Slot::Monster { pick: SlotPick::Role(role), band: offset } => {
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
            rows.push(CoverageRow { prefab: def.name().to_string(), glyph, what, reach: slot_reach });
        }
    }
    Coverage { bands, rows }
}

/// Everything a set of files with legends names through its slots and its
/// props' contents: the props it stands, the monsters it names and the
/// roles it draws for, and the items it asks for by name and by tag. What
/// a game reads to say which of its content nothing lays, its orphans,
/// with its own sources beside it: a spawn table, a shop, a kit, a list in
/// its code. A slot drawing for a role reaches the role, not its members,
/// since which members a draw finds is the game's spawn table's to say.
pub struct Reached<M> {
    /// Every prop a slot stands.
    pub props: BTreeSet<PropId>,
    /// Every monster a slot names.
    pub monsters: BTreeSet<Id<M>>,
    /// Every role a slot draws for.
    pub roles: BTreeSet<RoleId<M>>,
    /// Every item a slot or a stood prop's contents names, by the name the
    /// game's own items answer to.
    pub items: BTreeSet<String>,
    /// Every tag a slot or a stood prop's contents draws by.
    pub tags: BTreeSet<TagId>,
}

impl<M> Default for Reached<M> {
    fn default() -> Self {
        Self { props: BTreeSet::new(), monsters: BTreeSet::new(), roles: BTreeSet::new(), items: BTreeSet::new(), tags: BTreeSet::new() }
    }
}

impl<M> Reached<M> {
    /// What every one of `files` reaches, a prefab or a dressing alike,
    /// each prop's contents read from `props`.
    pub fn by<'p, D: Legended<M> + 'p>(files: impl IntoIterator<Item = &'p D>, props: &Registry<PropDef>) -> Self
    where
        M: 'p,
    {
        let mut reached = Self::default();
        for def in files {
            for (_, slot) in def.legend().slots() {
                match slot {
                    Slot::Prop(id) => reached.prop(*id, props),
                    Slot::Item(roll) => reached.stock(&roll.what),
                    Slot::Monster { pick: SlotPick::Kind(id), .. } => {
                        reached.monsters.insert(*id);
                    }
                    Slot::Monster { pick: SlotPick::Role(id), .. } => {
                        reached.roles.insert(*id);
                    }
                    Slot::Mark => {}
                }
            }
        }
        reached
    }

    /// `id` stood, and everything its contents name.
    pub fn prop(&mut self, id: PropId, props: &Registry<PropDef>) {
        self.props.insert(id);
        for row in props.get(id).container.iter().flat_map(|c| &c.contents) {
            self.stock(&row.what);
        }
    }

    /// What one container row or item slot names.
    fn stock(&mut self, what: &Stock) {
        match what {
            Stock::Item(name) => {
                self.items.insert(name.clone());
            }
            Stock::Tag(tag) => {
                self.tags.insert(*tag);
            }
        }
    }
}

/// The bit a dressing's key carries and a prefab's never does, so one
/// number on a stamped mark names either and the two registries never
/// collide: a game has far fewer than two billion prefabs.
const DRESSING: u32 = 1 << 31;

/// Every prefab and dressing a game loaded and the roles their monster
/// slots name, cut into the pieces and dressings a mapgen chain lays.
///
/// The one place that turns a file into what `atglyph-mapgen` stamps and grows:
/// [`piece`](Self::piece) a [`Prefab`] and [`grown`](Self::grown) a
/// [`PrefabShape`], each keyed by the prefab's registry id, and
/// [`dressing`](Self::dressing) a [`Dressing`] keyed with a bit no prefab's
/// id has, so [`slot`](Self::slot) finds what any stamped mark stands for
/// from the key the mark carries. No `App` and no Bevy: a tool or a
/// terminal game grows the same places from it, and `atglyph-bevy`'s `Prefabs`
/// is this, shared, as a resource.
pub struct PrefabLibrary<M> {
    defs: Registry<PrefabDef<M>>,
    roles: Registry<RoleDef<M>>,
    dressings: Registry<DressingDef<M>>,
}

impl<M> std::fmt::Debug for PrefabLibrary<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrefabLibrary").field("defs", &self.defs.len()).field("roles", &self.roles.len()).field("dressings", &self.dressings.len()).finish()
    }
}

impl<M> PrefabLibrary<M> {
    /// Every prefab, loaded with [`load`], and every role, loaded with
    /// [`role::load`].
    pub fn new(defs: Registry<PrefabDef<M>>, roles: Registry<RoleDef<M>>) -> Self {
        Self::with_dressings(defs, roles, Registry::default())
    }

    /// As [`new`](Self::new), with the dressings loaded with
    /// [`dressing::load`](crate::dressing::load), whose slots then fill as a
    /// prefab's.
    pub fn with_dressings(defs: Registry<PrefabDef<M>>, roles: Registry<RoleDef<M>>, dressings: Registry<DressingDef<M>>) -> Self {
        Self { defs, roles, dressings }
    }

    /// Every dressing.
    pub fn dressings(&self) -> &Registry<DressingDef<M>> {
        &self.dressings
    }

    /// Every prefab.
    pub fn defs(&self) -> &Registry<PrefabDef<M>> {
        &self.defs
    }

    /// Every role.
    pub fn roles(&self) -> &Registry<RoleDef<M>> {
        &self.roles
    }

    /// The piece called `name`, keyed so its stamp's marks find their slots
    /// again: each tile as the file paints it, each slot a mark painted with
    /// the prefab's ground, and declaring the turns and mirrors its file
    /// allows, so no stamp lays it a way its drawing refuses. An error
    /// naming the piece when there is none, so a chain can `?` it.
    pub fn piece(&self, name: &str) -> Result<Prefab, BuildError> {
        let id = self.defs.id(name).ok_or_else(|| BuildError::new("prefab", format!("no prefab is called {name:?}")))?;
        let def = self.defs.get(id);
        let cell = |c: char| match (def.tile(c), def.slot(c)) {
            (Some(t), _) => PrefabCell::Tile(t),
            (None, Some(_)) => PrefabCell::Mark(def.ground),
            (None, None) if def.entry(c) => PrefabCell::Entry,
            (None, None) => PrefabCell::Clear,
        };
        Prefab::parse_cells(&def.rows(), cell)
            .map(|p| p.keyed(id.raw()).declaring(def.turn, def.mirror))
            .map_err(|e| BuildError::new("prefab", format!("{name}: {e}")))
    }

    /// The piece called `name` as a shape [`Accrete`](atglyph_mapgen::Accrete)
    /// grows as an area of its own, joined by `join` through the entries its
    /// file draws, and keyed as [`piece`](Self::piece) keys it, so its
    /// slots fill as a stamped piece's do. An error naming the piece when
    /// there is none or it has no entry.
    pub fn grown(&self, name: &str, join: Join) -> Result<PrefabShape, BuildError> {
        PrefabShape::new(name, &self.piece(name)?, join)
    }

    /// The dressing called `name`, as [`Dress`](atglyph_mapgen::Dress) lays it:
    /// its rules as its file says them, keyed so the slots it emits find
    /// their meaning again. An error naming it when there is none, so a
    /// chain can `?` it.
    pub fn dressing(&self, name: &str) -> Result<Dressing, BuildError> {
        let id = self.dressings.id(name).ok_or_else(|| BuildError::new("dressing", format!("no dressing is called {name:?}")))?;
        let def = self.dressings.get(id);
        Ok(Dressing { name: def.name.clone(), things: def.things.clone(), repaint: def.repaint, key: Some(DRESSING | id.raw()) })
    }

    /// What `glyph` stands for in the prefab or the dressing keyed `key`,
    /// if the key names one and the glyph a slot in it.
    pub fn slot(&self, key: u32, glyph: char) -> Option<&Slot<M>> {
        if key & DRESSING != 0 {
            return self.dressings.try_get(Id::from_raw(key & !DRESSING))?.legend().slot(glyph);
        }
        self.defs.try_get(Id::from_raw(key))?.slot(glyph)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::Registry;
    use crate::prop::{PropDef, Stock};
    use crate::tag::TagDef;
    use atglyph_grid::{TileProps, TileRegistry};

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
        assert!(matches!(def.slot('b'), Some(Slot::Monster { pick: SlotPick::Role(r), band: 1 }) if *r == w.roles.expect("brute")));
        assert!(matches!(def.slot('W'), Some(Slot::Monster { pick: SlotPick::Kind(k), band: 0 }) if *k == w.beasts.expect("warden")));
        assert!(matches!(def.slot('m'), Some(Slot::Mark)));
        assert_eq!(def.rows(), vec!["#####", "#bLw#", "##W##", "##m##"]);
    }

    /// What a piece reaches is every prop it stands, with what each holds,
    /// every monster it names, every role it draws for and every item and
    /// tag it asks for, and nothing of a mark or a tile.
    #[test]
    fn a_piece_reaches_its_props_and_their_contents_its_monsters_its_roles_and_its_items() {
        let w = world();
        let props = crate::prop::load(
            r#"#![enable(implicit_some)]
            [(name: "locker", glyph: 'L', color: (r: 255, g: 255, b: 255), blocks: true,
                 container: (contents: [(item: "knife", count: 1), (tag: "weapon", count: (0, 1))]), offers: [(verb: "open", time: 100)])]"#,
            &Names::new().tags(&w.tags),
        )
        .unwrap();
        let names = Names::new().tags(&w.tags).with("prop", &props).with("monster", &w.beasts).with("role", &w.roles);
        let def = load::<Beast>(GUARDED, &w.tiles, &names).unwrap();
        let reached = Reached::by([&def], &props);
        assert_eq!(reached.props, BTreeSet::from([props.expect("locker")]));
        assert_eq!(reached.monsters, BTreeSet::from([w.beasts.expect("warden")]));
        assert_eq!(reached.roles, BTreeSet::from([w.roles.expect("brute")]));
        assert_eq!(reached.items, BTreeSet::from(["knife".to_string()]));
        assert_eq!(reached.tags, BTreeSet::from([w.tags.expect("weapon")]));
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

    /// A piece with a notch cut in its corner: the wall along the notch is
    /// outer wall as much as the wall along the edge.
    fn entered(rows: &[&str]) -> Result<PrefabDef<Beast>, ContentError> {
        let rows = rows.iter().map(|r| format!("{r:?}")).collect::<Vec<_>>().join(", ");
        read(&world(), &format!("(name: \"hall\", rows: [{rows}], legend: {{ '#': Tile(\"wall\"), '.': Tile(\"floor\"), 'E': Entry }})"))
    }

    #[test]
    fn an_entry_on_the_outer_wall_with_the_inside_across_loads_and_needs_no_ground() {
        let def = entered(&["  #####", "  #...#", "###...E", "E.....#", "#######"]).unwrap();
        assert!(def.entry('E') && !def.entry('#'));
        assert!(def.slot('E').is_none() && def.tile('E').is_none(), "an entry paints nothing and fills nothing");
        assert!(entered(&["##E##", "#...#", "#####"]).is_ok(), "on the top edge");
    }

    #[test]
    fn an_entry_off_the_outer_wall_or_with_no_inside_across_is_refused() {
        let inner = entered(&["#####", "#.E.#", "#...#", "#####"]).unwrap_err().to_string();
        assert!(inner.contains("'E' at (2, 1) is not on the piece's outer wall"), "{inner}");
        let corner = entered(&["E####", "#...#", "#####"]).unwrap_err().to_string();
        assert!(corner.contains("'E' at (0, 0) has no inside"), "{corner}");
        // Beside the notch, with wall and not floor straight across.
        let thick = entered(&["  E##", "  #.#", "###.#", "#...#", "#####"]).unwrap_err().to_string();
        assert!(thick.contains("'E' at (2, 0) has no inside"), "{thick}");
    }

    #[test]
    fn a_prefab_with_slots_and_no_ground_is_refused() {
        let w = world();
        let err = read(&w, r#"(name: "bare", rows: ["m"], legend: { 'm': Mark })"#).unwrap_err().to_string();
        assert!(err.contains("no `ground`"), "{err}");
    }

    /// What a file declares about turning and mirroring is what it loads
    /// with, `None` included, which the file's implicit `Some` must not
    /// read as an absent option; and a file that declares nothing may be
    /// laid any way at all.
    #[test]
    fn a_prefab_loads_the_turn_and_mirror_it_declares_and_every_facing_when_it_declares_none() {
        let w = world();
        let tiles = |extra: &str| format!(r###"(name: "bar", {extra} rows: ["##"], legend: {{ '#': Tile("wall") }})"###);
        let plain = read(&w, &tiles("")).unwrap();
        assert_eq!((plain.turn, plain.mirror), (Rotation::Quarter, Mirror::Both));
        for (text, turn, mirror) in [
            ("turn: None, mirror: None,", Rotation::None, Mirror::None),
            ("turn: Half, mirror: UpDown,", Rotation::Half, Mirror::UpDown),
            ("mirror: LeftRight,", Rotation::Quarter, Mirror::LeftRight),
        ] {
            let def = read(&w, &tiles(text)).unwrap_or_else(|e| panic!("{text}: {e}"));
            assert_eq!((def.turn, def.mirror), (turn, mirror), "{text}");
        }
        assert!(read(&w, &tiles("turn: Sideways,")).is_err(), "a turn no file may write");
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
        assert!(matches!(brute.reach[0], (1, BandFallback::Fallback(3))), "band 1 asks at 2, nothing until 3");
        assert!(matches!(brute.reach[4], (5, BandFallback::Exact)));
        assert!(matches!(brute.reach[9], (10, BandFallback::Fallback(8))), "band 10 asks at 11, the deepest is 8");
        let weapon = report.rows.iter().find(|r| r.glyph == 'w').unwrap();
        assert!(matches!(weapon.reach[9], (10, BandFallback::Fallback(10))));
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

    /// Two prefabs are two blocks, each under its own header, with one
    /// blank line between them and none anywhere else.
    #[test]
    fn a_rendered_report_of_two_prefabs_puts_one_blank_line_between_them() {
        let w = world();
        let guarded = read(&w, GUARDED).unwrap();
        let rack = read(&w, r#"(name: "rack", ground: "floor", rows: ["w"], legend: { 'w': Item(tag: "weapon") })"#).unwrap();
        let (monsters, items) = (heavy_on(&w, 3, 8), weapons(&w, 1, 10));
        let sources = Sources { roles: &w.roles, monsters: &monsters, items: &items, tags: &w.tags };
        let text = coverage([&guarded, &rack], &sources, 1..=3).render();
        let lines: Vec<&str> = text.lines().collect();
        let rack_at = lines.iter().position(|l| l.starts_with("rack")).unwrap_or_else(|| panic!("no rack header: {text}"));
        assert!(lines[0].starts_with("guarded locker"), "{text}");
        assert_eq!(lines[rack_at - 1], "", "one blank line before the second header: {text}");
        assert_eq!(lines.iter().filter(|l| l.is_empty()).count(), 1, "and no other: {text}");
        assert!(lines[rack_at + 1].trim_start().starts_with("w weapon"), "{text}");
        assert_eq!(lines.len(), rack_at + 2, "the rack's one row ends the report: {text}");
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
