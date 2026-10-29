//! Tile identity and semantics, kept apart.
//!
//! A [`TileId`] is two bytes of identity. Everything the engine needs to know
//! about a tile is a [`TileProps`] record in a [`TileRegistry`] the game
//! fills at startup, usually from RON. Lookup is one array index. The
//! engine ships a [`TileRegistry::standard`] set so a game gets walls,
//! floors and doors for free; a game that wants vacuum, force fields or
//! lava registers them and they are first-class from the first frame.
//!
//! A tile carries its own look, a [`TileLook`] of plain data: a glyph, or
//! several a cell picks among by where it is, and its colours. Keeping it
//! on the record means a tile is defined in one
//! place, a line of a file or a builder in code, and read once; the map's
//! hot tables never hold it, and a renderer turns it into colour. The
//! light a tile sheds is a [`Glow`] on the record too, which the Bevy
//! layer's lighting hangs on every cell of the kind.
//!
//! A tiles file is a list of [`TileProps`], one a line, the look's fields
//! written beside the rest: `(tile: "roots", walkable: true, opaque: true,
//! glyph: '+', fg: (0.55, 0.74, 0.45))`, or `glyph: ['"', '"', ',']` for a
//! tile drawn in several.

use std::collections::BTreeMap;

use atglyph_core::Point;
use atglyph_core::seed::position_hash;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::light::Glow;

/// A dense index into a [`TileRegistry`].
///
/// Two bytes rather than four because a large world stores millions of
/// these; 65,535 tile kinds is plenty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct TileId(pub u16);

impl TileId {
    /// The raw index, for indexing a parallel table.
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// The normal movement cost, in the same hundredths the turn clock uses.
pub const NORMAL_MOVE_COST: u16 = 100;

/// What the engine needs to know about a kind of tile.
///
/// The distinction between `walkable` and `passable` is load-bearing for
/// connectivity: a closed door is not walkable this turn but is passable in
/// the sense that a corridor through it still connects two rooms.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TileLine", into = "TileLine")]
pub struct TileProps {
    /// The name content files refer to it by.
    pub name: String,
    /// Whether an actor can stand on it right now.
    pub walkable: bool,
    /// Whether a route may go through it eventually (doors, shallow water).
    /// Defaults to `walkable` when omitted.
    pub passable: Option<bool>,
    /// Whether it blocks line of sight.
    pub opaque: bool,
    /// Whether it stops projectiles and thrown things.
    pub blocks_projectiles: bool,
    /// Cost to enter, in hundredths of a normal step. `0` is treated as normal.
    pub move_cost: u16,
    /// The tile this becomes when it is opened, by name: a closed door names
    /// its open self. Stepping into it opens it, for an actor able to, and a
    /// mind that opens doors paths through it.
    pub opens_to: Option<String>,
    /// The tile this becomes when it is closed, by name: an open door names
    /// its closed self.
    pub closes_to: Option<String>,
    /// How it burns, when it does. Nothing burns unless it says so.
    pub burn: Option<Burn>,
    /// The light every cell of it sheds by itself: lava, a lit window, a
    /// wall lamp. `None`, as most tiles are, sheds nothing.
    pub light: Option<Glow>,
    /// How it looks in full light. `None` draws as the renderer's
    /// placeholder, and a renderer building its table from the registry
    /// names every tile left without one.
    pub look: Option<TileLook>,
}

/// How a tile looks in full light: a glyph and its colours, plain data a
/// renderer turns into colour. Colours are `(r, g, b)` in `0..=1`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TileLook {
    /// The character drawn, or the characters a cell picks among by where
    /// it is.
    pub glyph: TileGlyphs,
    /// The glyph's colour.
    pub fg: (f32, f32, f32),
    /// The cell's fill; black when `None`.
    pub bg: Option<(f32, f32, f32)>,
    /// How far each cell strays from the authored colour, `(brightness,
    /// hue)` in `0..=1` each; none when `None`.
    pub vary: Option<(f32, f32)>,
    /// Brightness drifting over time, `0..=1`, for water and anything else
    /// that should not sit still.
    pub shimmer: f32,
}

impl TileLook {
    /// `glyph` in `fg` on black, the same in every cell: one character, or
    /// an array of them a cell picks among, as [`TileGlyphs`] says.
    pub fn new(glyph: impl Into<TileGlyphs>, fg: (f32, f32, f32)) -> Self {
        Self { glyph: glyph.into(), fg, bg: None, vary: None, shimmer: 0.0 }
    }

    /// Builder: the cell's fill.
    pub const fn on(mut self, bg: (f32, f32, f32)) -> Self {
        self.bg = Some(bg);
        self
    }

    /// Builder: how far each cell strays in brightness and hue.
    pub const fn vary(mut self, brightness: f32, hue: f32) -> Self {
        self.vary = Some((brightness, hue));
        self
    }

    /// Builder: brightness drifting over time.
    pub const fn shimmer(mut self, shimmer: f32) -> Self {
        self.shimmer = shimmer;
        self
    }
}

/// The glyphs a tile is drawn in: one, or several interchangeable ones each
/// cell picks among, so a meadow reads as grass rather than as one
/// character repeated to the horizon.
///
/// A cell's pick is a hash of its world position, never the time or a
/// draw, so a tile looks the same every frame, in memory as in sight, and
/// does not shimmer as the view scrolls over it; nor does it spend a draw
/// from any stream, since how a tile looks is cosmetic. A glyph listed
/// twice is picked twice as often. Never empty.
///
/// Written in a file as one character, `glyph: '.'`, or a list of them,
/// `glyph: ['"', '"', ',']`, and read back the same way; in code, a `char`
/// or an array of them converts into one.
///
/// ```
/// use atglyph_core::Point;
/// use atglyph_grid::TileGlyphs;
///
/// let grass = TileGlyphs::from(['"', '"', ',']);
/// let here = Point::new(12, 7);
/// assert_eq!(grass.at(here), grass.at(here), "the same cell, the same glyph");
/// assert!(grass.as_slice().contains(&grass.at(here)));
/// assert_eq!(TileGlyphs::from('.').at(here), '.');
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileGlyphs(Vec<char>);

/// Salt for the pick, so which glyph a cell draws owes nothing to any
/// other hash of its position, a renderer's jitter among them.
const GLYPH_SALT: u64 = 0x6c79_7068_7069_636b;

impl TileGlyphs {
    /// The glyphs of `glyphs`, in order, repeats kept as weight; `None`
    /// when there are none.
    pub fn new(glyphs: impl IntoIterator<Item = char>) -> Option<Self> {
        let glyphs: Vec<char> = glyphs.into_iter().collect();
        (!glyphs.is_empty()).then_some(Self(glyphs))
    }

    /// The glyph drawn where a cell's place does not matter: a key, a
    /// legend, a tile named rather than shown. The only one, for a tile of
    /// one.
    pub fn first(&self) -> char {
        self.0[0]
    }

    /// The glyph the cell at world position `p` draws.
    pub fn at(&self, p: Point) -> char {
        match self.0.as_slice() {
            [one] => *one,
            many => many[(position_hash(GLYPH_SALT, p.x, p.y) % many.len() as u64) as usize],
        }
    }

    /// Every glyph, in the order written and with its repeats: for a check
    /// that each can be drawn, say that the font has it.
    pub fn as_slice(&self) -> &[char] {
        &self.0
    }
}

impl From<char> for TileGlyphs {
    fn from(glyph: char) -> Self {
        Self(vec![glyph])
    }
}

impl<const N: usize> From<[char; N]> for TileGlyphs {
    fn from(glyphs: [char; N]) -> Self {
        // An empty array fails to compile where it is converted, rather than
        // at the first cell drawn.
        const { assert!(N > 0, "a tile is drawn in at least one glyph") };
        Self(glyphs.to_vec())
    }
}

/// A tile of one glyph is that glyph, so a check written against one
/// character reads as it did; a tile of several is no single character.
impl PartialEq<char> for TileGlyphs {
    fn eq(&self, other: &char) -> bool {
        self.0 == [*other]
    }
}

impl Serialize for TileGlyphs {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0.as_slice() {
            [one] => serializer.serialize_char(*one),
            many => many.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for TileGlyphs {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Glyphs;
        impl<'de> serde::de::Visitor<'de> for Glyphs {
            type Value = TileGlyphs;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a glyph, or a list of at least one")
            }

            fn visit_char<E: serde::de::Error>(self, glyph: char) -> Result<TileGlyphs, E> {
                Ok(TileGlyphs::from(glyph))
            }

            // A one-character string, for a format with no characters of its own.
            fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<TileGlyphs, E> {
                let mut chars = text.chars();
                match (chars.next(), chars.next()) {
                    (Some(glyph), None) => Ok(TileGlyphs::from(glyph)),
                    _ => Err(E::invalid_value(serde::de::Unexpected::Str(text), &self)),
                }
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<TileGlyphs, A::Error> {
                let mut glyphs = Vec::new();
                while let Some(glyph) = seq.next_element::<char>()? {
                    glyphs.push(glyph);
                }
                TileGlyphs::new(glyphs).ok_or_else(|| serde::de::Error::invalid_length(0, &self))
            }
        }
        deserializer.deserialize_any(Glyphs)
    }
}

/// One line of a tiles file: a tile with its look's fields written beside
/// the rest, which is how [`TileProps`] reads and writes itself.
///
/// The full option space: `tile` always; `walkable`, `opaque` and
/// `blocks_projectiles`, false when left out; `passable`, as `walkable`;
/// `move_cost`, normal; `opens_to`, `closes_to`, `burn` and `light`, none;
/// and the look, `glyph` and `fg` together or neither, with `bg`, `vary`
/// and `shimmer` only beside them. `glyph` is one character, `'.'`, or a
/// list of at least one a cell picks among, `['"', '"', ',']`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TileLine {
    #[serde(alias = "name")]
    tile: String,
    #[serde(default)]
    walkable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    passable: Option<bool>,
    #[serde(default)]
    opaque: bool,
    #[serde(default)]
    blocks_projectiles: bool,
    #[serde(default = "normal_move_cost")]
    move_cost: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    opens_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    closes_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    burn: Option<Burn>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    light: Option<Glow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    glyph: Option<TileGlyphs>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fg: Option<(f32, f32, f32)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bg: Option<(f32, f32, f32)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vary: Option<(f32, f32)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shimmer: Option<f32>,
}

impl TryFrom<TileLine> for TileProps {
    type Error = String;

    fn try_from(line: TileLine) -> Result<Self, String> {
        let look = match (line.glyph, line.fg) {
            (Some(glyph), Some(fg)) => Some(TileLook { glyph, fg, bg: line.bg, vary: line.vary, shimmer: line.shimmer.unwrap_or(0.0) }),
            (None, None) if line.bg.is_none() && line.vary.is_none() && line.shimmer.is_none() => None,
            _ => return Err(format!("{}: a look needs a glyph and an fg, both", line.tile)),
        };
        Ok(Self {
            name: line.tile,
            walkable: line.walkable,
            passable: line.passable,
            opaque: line.opaque,
            blocks_projectiles: line.blocks_projectiles,
            move_cost: line.move_cost,
            opens_to: line.opens_to,
            closes_to: line.closes_to,
            burn: line.burn,
            light: line.light,
            look,
        })
    }
}

impl From<TileProps> for TileLine {
    fn from(t: TileProps) -> Self {
        let (glyph, fg, bg, vary, shimmer) = match t.look {
            Some(l) => (Some(l.glyph), Some(l.fg), l.bg, l.vary, Some(l.shimmer).filter(|s| *s != 0.0)),
            None => (None, None, None, None, None),
        };
        Self {
            tile: t.name,
            walkable: t.walkable,
            passable: t.passable,
            opaque: t.opaque,
            blocks_projectiles: t.blocks_projectiles,
            move_cost: t.move_cost,
            opens_to: t.opens_to,
            closes_to: t.closes_to,
            burn: t.burn,
            light: t.light,
            glyph,
            fg,
            bg,
            vary,
            shimmer,
        }
    }
}

/// What a tile left without a cost costs, for serde.
fn normal_move_cost() -> u16 {
    NORMAL_MOVE_COST
}

/// How a tile burns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Burn {
    /// Percent chance each turn that it catches from each burning neighbour.
    pub catch_pct: u8,
    /// Turns it burns once alight.
    pub turns: u8,
    /// The tile it leaves once burnt out, by name.
    ///
    /// Required rather than defaulting to the tile itself: a tile that stayed
    /// itself could catch again from the neighbour it had just lit, and two
    /// such tiles would pass one fire between them forever. Burning uses the
    /// fuel up, which is what makes every fire end.
    pub leaves: String,
}

/// How a tile burns, with the tile it leaves resolved to an id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kindling {
    /// Percent chance each turn that it catches from each burning neighbour.
    pub catch_pct: u8,
    /// Turns it burns once alight, at least one.
    pub turns: u8,
    /// What it leaves.
    pub leaves: TileId,
}

impl TileProps {
    /// A tile with the given name and every flag off.
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            walkable: false,
            passable: None,
            opaque: false,
            blocks_projectiles: false,
            move_cost: NORMAL_MOVE_COST,
            opens_to: None,
            closes_to: None,
            burn: None,
            light: None,
            look: None,
        }
    }

    /// A walkable, transparent tile.
    pub fn floor(name: impl Into<String>) -> Self {
        Self { walkable: true, ..Self::named(name) }
    }

    /// An impassable, opaque, projectile-stopping tile.
    pub fn wall(name: impl Into<String>) -> Self {
        Self { opaque: true, blocks_projectiles: true, ..Self::named(name) }
    }

    /// Builder: sets `walkable`.
    pub fn walkable(mut self, v: bool) -> Self {
        self.walkable = v;
        self
    }

    /// Builder: sets `passable`.
    pub fn passable(mut self, v: bool) -> Self {
        self.passable = Some(v);
        self
    }

    /// Builder: sets `opaque`.
    pub fn opaque(mut self, v: bool) -> Self {
        self.opaque = v;
        self
    }

    /// Builder: sets `blocks_projectiles`.
    pub fn blocks_projectiles(mut self, v: bool) -> Self {
        self.blocks_projectiles = v;
        self
    }

    /// Builder: sets `move_cost`.
    pub fn move_cost(mut self, v: u16) -> Self {
        self.move_cost = v;
        self
    }

    /// Builder: opening this makes it the tile called `name`.
    pub fn opens_to(mut self, name: impl Into<String>) -> Self {
        self.opens_to = Some(name.into());
        self
    }

    /// Builder: closing this makes it the tile called `name`.
    pub fn closes_to(mut self, name: impl Into<String>) -> Self {
        self.closes_to = Some(name.into());
        self
    }

    /// Builder: it catches from each burning neighbour with `catch_pct`
    /// percent a turn, burns `turns` turns, and leaves the tile called
    /// `leaves`.
    pub fn burns(mut self, catch_pct: u8, turns: u8, leaves: impl Into<String>) -> Self {
        self.burn = Some(Burn { catch_pct, turns, leaves: leaves.into() });
        self
    }

    /// Builder: every cell of it sheds `light`.
    pub fn light(mut self, light: Glow) -> Self {
        self.light = Some(light);
        self
    }

    /// Builder: how it looks in full light.
    pub fn look(mut self, look: TileLook) -> Self {
        self.look = Some(look);
        self
    }

    /// Whether a route may go through it, defaulting to `walkable`.
    pub(crate) fn is_passable(&self) -> bool {
        self.passable.unwrap_or(self.walkable)
    }

    /// The effective entry cost, never zero.
    pub(crate) fn effective_move_cost(&self) -> u32 {
        if self.move_cost == 0 { NORMAL_MOVE_COST as u32 } else { self.move_cost as u32 }
    }
}

/// Every kind of tile a game knows about.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TileRegistry {
    props: Vec<TileProps>,
    by_name: BTreeMap<String, TileId>,
}

/// Why a registration was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterError {
    /// A tile with this name already exists.
    Duplicate(String),
    /// The registry is full.
    Full,
}

impl std::fmt::Display for RegisterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegisterError::Duplicate(n) => write!(f, "tile {n:?} is already registered"),
            RegisterError::Full => write!(f, "tile registry is full"),
        }
    }
}

impl std::error::Error for RegisterError {}

impl TileRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// The engine's conventional starting set: `void`, `wall`, `floor`,
    /// `door_closed`, `door_open`.
    ///
    /// `void` is id 0 and is what an unwritten cell holds: not walkable,
    /// opaque, so an unfinished map is a solid block rather than an open
    /// field. The two doors open and close into each other.
    pub fn standard() -> Self {
        // Plain greys, so they read as placeholders a game replaces, and
        // doors in a warm brown so a way through stands out.
        let floor_bg = (0.05, 0.05, 0.05);
        let door = (0.70, 0.50, 0.30);
        let mut r = Self::new();
        r.register(TileProps::wall("void").look(TileLook::new(' ', (0.0, 0.0, 0.0)).on((0.0, 0.0, 0.0)))).expect("empty registry");
        r.register(TileProps::wall("wall").look(TileLook::new('#', (0.60, 0.60, 0.60)).on((0.15, 0.15, 0.15)))).expect("fresh name");
        r.register(TileProps::floor("floor").look(TileLook::new('.', (0.45, 0.45, 0.45)).on(floor_bg))).expect("fresh name");
        let shut = TileProps::named("door_closed").passable(true).opaque(true).blocks_projectiles(true).opens_to("door_open");
        r.register(shut.look(TileLook::new('+', door).on(floor_bg))).expect("fresh name");
        r.register(TileProps::floor("door_open").closes_to("door_closed").look(TileLook::new('\'', door).on(floor_bg))).expect("fresh name");
        r
    }

    /// Adds a tile kind. Names must be unique.
    pub fn register(&mut self, props: TileProps) -> Result<TileId, RegisterError> {
        if self.by_name.contains_key(&props.name) {
            return Err(RegisterError::Duplicate(props.name));
        }
        if self.props.len() > u16::MAX as usize {
            return Err(RegisterError::Full);
        }
        let id = TileId(self.props.len() as u16);
        self.by_name.insert(props.name.clone(), id);
        self.props.push(props);
        Ok(id)
    }

    /// Adds every tile in `defs`, stopping at the first error.
    pub fn register_all(&mut self, defs: impl IntoIterator<Item = TileProps>) -> Result<(), RegisterError> {
        for d in defs {
            self.register(d)?;
        }
        Ok(())
    }

    /// The properties of `id`.
    ///
    /// # Panics
    /// Panics if `id` was not issued by this registry.
    pub fn get(&self, id: TileId) -> &TileProps {
        &self.props[id.index()]
    }

    /// The id registered under `name`.
    pub fn id(&self, name: &str) -> Option<TileId> {
        self.by_name.get(name).copied()
    }

    /// The id registered under `name`.
    ///
    /// # Panics
    /// Panics if no such tile exists. For content the game knows it
    /// registered; use [`id`](Self::id) for user input.
    pub fn expect(&self, name: &str) -> TileId {
        self.id(name).unwrap_or_else(|| panic!("no tile named {name:?}"))
    }

    /// Sets the light every cell of `id` sheds, or with `None` takes it
    /// away: for a game that registers its tiles in code and writes their
    /// light in a file beside their looks, read once the names exist.
    ///
    /// # Panics
    /// Panics if `id` was not issued by this registry.
    pub fn set_light(&mut self, id: TileId, light: Option<Glow>) {
        self.props[id.index()].light = light;
    }

    /// Number of kinds registered.
    pub fn len(&self) -> usize {
        self.props.len()
    }

    /// Whether nothing is registered.
    pub fn is_empty(&self) -> bool {
        self.props.is_empty()
    }

    /// Every id in registration order with its properties.
    pub fn iter(&self) -> impl Iterator<Item = (TileId, &TileProps)> {
        self.props.iter().enumerate().map(|(i, p)| (TileId(i as u16), p))
    }

    /// The flags every algorithm consults, as parallel tables indexed by id.
    ///
    /// Built once per registry and cached by the callers that sit in hot
    /// loops, so a FOV or a flood never chases a `Vec<TileProps>` pointer.
    ///
    /// # Panics
    /// Panics naming the tile if one opens, closes or burns into a name
    /// nobody registered. Names are resolved here rather than at registration
    /// so a door may be registered before the tile it opens into.
    pub fn tables(&self) -> TileTables {
        let named = |props: &TileProps, name: &str, verb: &str| {
            self.id(name).unwrap_or_else(|| panic!("tile {:?} {verb} {name:?}, which is not registered", props.name))
        };
        let resolve = |props: &TileProps, into: &Option<String>, verb: &str| into.as_ref().map(|name| named(props, name, verb));
        let kindling = |props: &TileProps| {
            props.burn.as_ref().map(|b| Kindling { catch_pct: b.catch_pct.min(100), turns: b.turns.max(1), leaves: named(props, &b.leaves, "burns to") })
        };
        TileTables {
            names: self.props.iter().map(|p| p.name.clone()).collect(),
            walkable: self.props.iter().map(|p| p.walkable).collect(),
            passable: self.props.iter().map(|p| p.is_passable()).collect(),
            opaque: self.props.iter().map(|p| p.opaque).collect(),
            blocks_projectiles: self.props.iter().map(|p| p.blocks_projectiles).collect(),
            move_cost: self.props.iter().map(|p| p.effective_move_cost()).collect(),
            opens: self.props.iter().map(|p| resolve(p, &p.opens_to, "opens to")).collect(),
            closes: self.props.iter().map(|p| resolve(p, &p.closes_to, "closes to")).collect(),
            burns: self.props.iter().map(kindling).collect(),
            lights: self.props.iter().map(|p| p.light).collect(),
        }
    }
}

/// Dense per-id flag tables, see [`TileRegistry::tables`].
#[derive(Debug, Clone, Default)]
pub struct TileTables {
    /// What each tile is called, for whatever describes one to a player.
    pub names: Vec<String>,
    /// Walkable now.
    pub walkable: Vec<bool>,
    /// Passable eventually.
    pub passable: Vec<bool>,
    /// Blocks sight.
    pub opaque: Vec<bool>,
    /// Blocks projectiles.
    pub blocks_projectiles: Vec<bool>,
    /// Entry cost in hundredths of a step.
    pub move_cost: Vec<u32>,
    /// What each tile becomes when opened, if it opens.
    pub opens: Vec<Option<TileId>>,
    /// What each tile becomes when closed, if it closes.
    pub closes: Vec<Option<TileId>>,
    /// How each tile burns, if it does.
    pub burns: Vec<Option<Kindling>>,
    /// The light each tile sheds by itself, if it sheds any.
    pub lights: Vec<Option<Glow>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_set_has_void_first_and_sane_flags() {
        let r = TileRegistry::standard();
        assert_eq!(r.expect("void"), TileId(0));
        assert!(!r.get(TileId(0)).walkable);
        assert!(r.get(TileId(0)).opaque);
        assert!(r.get(r.expect("floor")).walkable);
        let door = r.get(r.expect("door_closed"));
        assert!(!door.walkable && door.is_passable() && door.opaque);
        assert_eq!(r.len(), 5);
    }

    #[test]
    fn names_are_unique_and_looked_up() {
        let mut r = TileRegistry::new();
        let lava = r.register(TileProps::floor("lava").move_cost(300)).unwrap();
        assert_eq!(r.id("lava"), Some(lava));
        assert_eq!(r.id("ice"), None);
        assert_eq!(r.register(TileProps::floor("lava")), Err(RegisterError::Duplicate("lava".into())));
        assert_eq!(r.get(lava).effective_move_cost(), 300);
        assert_eq!(TileProps::floor("x").move_cost(0).effective_move_cost(), 100);
    }

    #[test]
    fn tables_mirror_the_registry() {
        let r = TileRegistry::standard();
        let t = r.tables();
        for (id, props) in r.iter() {
            assert_eq!(t.walkable[id.index()], props.walkable);
            assert_eq!(t.opaque[id.index()], props.opaque);
            assert_eq!(t.passable[id.index()], props.is_passable());
        }
    }

    #[test]
    fn a_door_opens_into_the_tile_it_names_and_closes_back() {
        let r = TileRegistry::standard();
        let t = r.tables();
        let (closed, open) = (r.expect("door_closed"), r.expect("door_open"));
        assert_eq!(t.opens[closed.index()], Some(open));
        assert_eq!(t.closes[open.index()], Some(closed));
        assert_eq!(t.opens[r.expect("wall").index()], None, "a wall opens into nothing");
        let gate: TileProps = ron::from_str(r#"(name: "gate", passable: Some(true), opens_to: Some("gate_open"))"#).unwrap();
        assert_eq!(gate.opens_to.as_deref(), Some("gate_open"));
    }

    #[test]
    fn a_tile_that_burns_leaves_the_tile_it_names() {
        let mut r = TileRegistry::standard();
        let ash = r.register(TileProps::floor("ash")).unwrap();
        let grass = r.register(TileProps::floor("grass").burns(60, 3, "ash")).unwrap();
        let t = r.tables();
        assert_eq!(t.burns[grass.index()], Some(Kindling { catch_pct: 60, turns: 3, leaves: ash }));
        assert_eq!(t.burns[ash.index()], None, "ash does not burn, so the fire ends");
        let hay: TileProps = ron::from_str(r#"(name: "hay", walkable: true, burn: Some((catch_pct: 90, turns: 2, leaves: "ash")))"#).unwrap();
        assert_eq!(hay.burn.map(|b| b.leaves), Some("ash".to_string()));
    }

    #[test]
    #[should_panic(expected = "tile \"thatch\" burns to \"ahs\", which is not registered")]
    fn a_tile_that_burns_into_a_tile_nobody_registered_is_refused_by_name() {
        let mut r = TileRegistry::standard();
        r.register(TileProps::floor("thatch").burns(50, 2, "ahs")).unwrap();
        r.tables();
    }

    #[test]
    #[should_panic(expected = "tile \"hatch\" opens to \"hatch_opne\", which is not registered")]
    fn a_door_into_a_tile_nobody_registered_is_refused_by_name() {
        let mut r = TileRegistry::standard();
        r.register(TileProps::named("hatch").opens_to("hatch_opne")).unwrap();
        r.tables();
    }

    #[test]
    fn props_load_from_ron_with_defaults() {
        let p: TileProps = ron::from_str(r#"(name: "grass", walkable: true)"#).unwrap();
        assert!(p.walkable && !p.opaque && p.is_passable());
        assert_eq!(p.effective_move_cost(), 100);
        let list: Vec<TileProps> = ron::from_str(r#"[(name: "a", walkable: true), (name: "b", opaque: true, move_cost: 250)]"#).unwrap();
        let mut r = TileRegistry::new();
        r.register_all(list).unwrap();
        assert_eq!(r.get(r.expect("b")).effective_move_cost(), 250);
    }

    /// A tile says what light it sheds in the same file that says what it
    /// is, and the tables carry it beside the flags; a tile that says
    /// nothing sheds nothing.
    #[test]
    fn a_tiles_light_reaches_the_tables_and_a_tile_without_one_sheds_none() {
        let list: Vec<TileProps> = ron::from_str(
            r#"[(name: "rock", opaque: true), (name: "lava", walkable: true, light: Some((intensity: 200, radius: 3, color: (r: 255, g: 133, b: 38), flicker: 77)))]"#,
        )
        .unwrap();
        let mut r = TileRegistry::new();
        r.register_all(list).unwrap();
        let lava = Glow::new(200, 3, crate::light::Rgb::new(255, 133, 38)).flickering(77);
        let tables = r.tables();
        assert_eq!(tables.lights[r.expect("lava").index()], Some(lava));
        assert_eq!(tables.lights[r.expect("rock").index()], None);
        r.set_light(r.expect("rock"), Some(Glow::darkness(255, 2)));
        assert_eq!(r.tables().lights[r.expect("rock").index()], Some(Glow::darkness(255, 2)), "and a light set after registering lands too");
    }

    /// A tile of one glyph draws it everywhere and reads and writes as the
    /// one character it always was.
    #[test]
    fn a_tile_of_one_glyph_draws_it_in_every_cell_and_round_trips_as_one_character() {
        let p: TileProps = ron::from_str(r#"(name: "floor", walkable: true, glyph: Some('.'), fg: Some((0.5, 0.5, 0.5)))"#).unwrap();
        let look = p.look.clone().expect("a look");
        assert_eq!(look, TileLook::new('.', (0.5, 0.5, 0.5)));
        assert!(look.glyph == '.', "a single glyph compares as its character");
        assert!((-20..20).all(|x| look.glyph.at(Point::new(x, x * 3 - 7)) == '.'));
        let written = ron::to_string(&p).unwrap();
        assert!(written.contains("glyph:Some('.')"), "written as one character, not a list: {written}");
        assert_eq!(ron::from_str::<TileProps>(&written).unwrap(), p);
    }

    /// A list is picked from by position alone, so the same cell draws the
    /// same glyph however often it is asked, every glyph turns up across a
    /// patch of ground, and one listed twice turns up about twice as often.
    #[test]
    fn a_tile_of_several_glyphs_picks_one_by_position_and_shows_every_one_in_its_weight() {
        let p: TileProps = ron::from_str(r#"(name: "grass", walkable: true, glyph: Some(['"', '"', ',']), fg: Some((0.3, 0.6, 0.2)))"#).unwrap();
        let glyphs = p.look.as_ref().expect("a look").glyph.clone();
        assert_eq!(glyphs.as_slice(), ['"', '"', ','], "in the order written, repeats kept");
        assert_eq!(glyphs.first(), '"');
        assert!(glyphs != '"', "several glyphs are no one character");
        let patch: Vec<Point> = (-16..16).flat_map(|y| (-16..16).map(move |x| Point::new(x, y))).collect();
        assert!(patch.iter().all(|&at| glyphs.at(at) == glyphs.at(at)));
        let (quotes, commas) = (patch.iter().filter(|&&at| glyphs.at(at) == '"').count(), patch.iter().filter(|&&at| glyphs.at(at) == ',').count());
        assert_eq!(quotes + commas, patch.len(), "only listed glyphs are drawn");
        assert!(commas > 0, "every glyph turns up");
        assert!((1.6..2.4).contains(&(quotes as f64 / commas as f64)), "twice listed, about twice as often: {quotes} to {commas}");
        let written = ron::to_string(&p).unwrap();
        assert_eq!(ron::from_str::<TileProps>(&written).unwrap(), p, "and it reads back as written: {written}");
        assert_eq!(TileLook::new(['"', '"', ','], (0.3, 0.6, 0.2)).glyph, glyphs, "in code, an array of them");
    }

    #[test]
    fn a_tile_drawn_in_no_glyph_at_all_is_refused() {
        let err = ron::from_str::<TileProps>(r#"(name: "nothing", glyph: Some([]), fg: Some((1.0, 1.0, 1.0)))"#).unwrap_err();
        assert!(err.to_string().contains("at least one"), "{err}");
        assert_eq!(TileGlyphs::new([]), None);
        assert_eq!(TileGlyphs::new("ab".chars()).map(|g| g.as_slice().to_vec()), Some(vec!['a', 'b']));
    }
}
