//! Tile identity and semantics, kept apart.
//!
//! A [`TileId`] is two bytes of identity. Everything the engine needs to know
//! about a tile is a [`TileProps`] record in a [`TileRegistry`] the game
//! fills at startup, usually from RON. Lookup is one array index. The
//! engine ships a [`TileRegistry::standard`] set so a game gets walls,
//! floors and doors for free; a game that wants vacuum, force fields or
//! lava registers them and they are first-class from the first frame.
//!
//! Anything only the game cares about (glyph, colour, flavour text) belongs
//! in a game-side table keyed by the same id, not here.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileProps {
    /// The name content files refer to it by.
    pub name: String,
    /// Whether an actor can stand on it right now.
    #[serde(default)]
    pub walkable: bool,
    /// Whether a route may go through it eventually (doors, shallow water).
    /// Defaults to `walkable` when omitted.
    #[serde(default)]
    pub passable: Option<bool>,
    /// Whether it blocks line of sight.
    #[serde(default)]
    pub opaque: bool,
    /// Whether it stops projectiles and thrown things.
    #[serde(default)]
    pub blocks_projectiles: bool,
    /// Cost to enter, in hundredths of a normal step. `0` is treated as normal.
    #[serde(default)]
    pub move_cost: u16,
    /// The tile this becomes when it is opened, by name: a closed door names
    /// its open self. Stepping into it opens it, for an actor able to, and a
    /// mind that opens doors paths through it.
    #[serde(default)]
    pub opens_to: Option<String>,
    /// The tile this becomes when it is closed, by name: an open door names
    /// its closed self.
    #[serde(default)]
    pub closes_to: Option<String>,
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

    /// Whether a route may go through it, defaulting to `walkable`.
    pub fn is_passable(&self) -> bool {
        self.passable.unwrap_or(self.walkable)
    }

    /// The effective entry cost, never zero.
    pub fn effective_move_cost(&self) -> u32 {
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
        let mut r = Self::new();
        r.register(TileProps::wall("void")).expect("empty registry");
        r.register(TileProps::wall("wall")).expect("fresh name");
        r.register(TileProps::floor("floor")).expect("fresh name");
        r.register(TileProps::named("door_closed").passable(true).opaque(true).blocks_projectiles(true).opens_to("door_open")).expect("fresh name");
        r.register(TileProps::floor("door_open").closes_to("door_closed")).expect("fresh name");
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

    /// The properties of `id`, if it exists.
    pub fn try_get(&self, id: TileId) -> Option<&TileProps> {
        self.props.get(id.index())
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
    /// Panics naming the tile if one opens or closes into a name nobody
    /// registered. Names are resolved here rather than at registration so a
    /// door may be registered before the tile it opens into.
    pub fn tables(&self) -> TileTables {
        let resolve = |props: &TileProps, into: &Option<String>, verb: &str| {
            into.as_ref().map(|name| self.id(name).unwrap_or_else(|| panic!("tile {:?} {verb} {name:?}, which is not registered", props.name)))
        };
        TileTables {
            walkable: self.props.iter().map(|p| p.walkable).collect(),
            passable: self.props.iter().map(|p| p.is_passable()).collect(),
            opaque: self.props.iter().map(|p| p.opaque).collect(),
            blocks_projectiles: self.props.iter().map(|p| p.blocks_projectiles).collect(),
            move_cost: self.props.iter().map(|p| p.effective_move_cost()).collect(),
            opens: self.props.iter().map(|p| resolve(p, &p.opens_to, "opens to")).collect(),
            closes: self.props.iter().map(|p| resolve(p, &p.closes_to, "closes to")).collect(),
        }
    }
}

/// Dense per-id flag tables, see [`TileRegistry::tables`].
#[derive(Debug, Clone, Default)]
pub struct TileTables {
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
}
