//! A pool: the pieces a run may hold, how often, where and how many.
//!
//! A pool file is [`Pool`], read once by [`load`], which resolves every
//! name in it and reports every mistake at once, as a layout file is. An
//! [`Entry`] is one thing a place may be given: a prefab grown as an area
//! of its own, a prefab stamped inside an area, or a dressing; the floors
//! and place tags it may stand on; a named weight; how many of it one place
//! and one run may hold; an exclusive group; and, for one stamped inside,
//! the kind of area it prefers. A [`Budget`] says how many picks of a kind
//! a floor gets, and whether a pick of it the place cannot fit fails the
//! place or is dropped.
//!
//! [`Pool::plan_run`] draws a whole run's [`Pick`]s at once, one list for
//! each [`Place`] the run will build, from a stream the game derives from
//! the run's seed, so every limit and group is held in one place and a
//! place's picks never depend on which place is built first.
//!
//! A kind is a string the file names in its budgets and nothing else: no
//! kind means anything to the engine, which only counts them, and the
//! piece itself says nothing about what kind of room it is
//! (`docs/design/prefabs.md` section 8 refused categories on a prefab).
//!
//! What this module does not hold is the laying. Growing a piece is
//! `rl-mapgen`'s `Accrete` and stamping one inside an area its
//! `StampPicks`; `rl-bevy` turns a place's picks into both. This crate
//! names no mapgen type, so a pool loads and every mistake in it is
//! reported with no map and no `App`.
//!
//! ```text
//! // A pool: the pieces a run may hold, how often, where and how many.
//! //
//! // Every field:
//! //   weights: [(name, weight)], how often an entry is drawn, by a name
//! //            the entries use, so a retune is one line; nought is never
//! //   tags:    every place tag an entry may name in where or not
//! //   budgets: how many picks of a kind a floor gets
//! //     floors:   (first, last), the floors it covers; two budgets of one
//! //               kind never cover the same floor, and a floor no budget
//! //               of a kind covers gets none of that kind
//! //     kind:     the kind it counts; any string, read only here
//! //     count:    (fewest, most), drawn evenly for each floor
//! //     required: optional, false if left out; true fails a place that
//! //               cannot fit a pick of this kind, false drops the pick
//! //   entries: what a place may be given
//! //     name:      the entry's name
//! //     kind:      a kind some budget names
//! //     piece:     the prefab it lays, for Grown and Inside
//! //     dressing:  the dressing it lays, for Dressed; one of piece and
//! //                dressing, never both
//! //     floors:    (first, last), the floors it may stand on
//! //     where:     optional, any if left out: tags of which a place must
//! //                carry one
//! //     not:       optional, none if left out: tags a place must carry
//! //                none of
//! //     weight:    a name from weights
//! //     per_place: optional, 1 if left out: the most of it on one floor
//! //     per_run:   optional, 1 if left out: the most of it in one run,
//! //                no fewer than per_place
//! //     group:     optional: entries sharing a group never share a floor,
//! //                and are all of one kind
//! //     placed:    how it is laid, one of
//! //       Grown(join)       grown as an area of its own through the entries
//! //                         its prefab draws; only of a kind every budget
//! //                         covering its floors marks required, since
//! //                         growth fails a place that cannot lay a piece
//! //                         it owes rather than dropping it; joined by one of
//! //                           Door, Arch, Mouth(n) with n at least 1,
//! //                           Locked("tag") opened by a carrier of the tag,
//! //                           Sealed, a gate the game lifts
//! //       Inside([shape])   stamped inside an area one of the layout's
//! //                         named shapes drew, at least one of them a
//! //                         shape that draws rooms, not a Passage; any
//! //                         area if the list is empty
//! //       Dressed           a dressing laid over an area of any size
//! //     prefer:    optional, Anywhere if left out, and only Anywhere for a
//! //                grown piece: the area an Inside or Dressed pick goes
//! //                in, one of
//! //       Secluded   the area farthest from the route it fits
//! //       DeadEnd    an area with one way in
//! //       Hidden     an area reached only through a secret
//! //       Junction   an area three ways or more lead out of
//! //       Anywhere   any area it fits
//! (
//!     weights: [("rare", 5), ("common", 50)],
//!     tags: ["wet", "dry"],
//!     budgets: [
//!         (floors: (1, 5), kind: "flavor", count: (2, 3)),
//!         (floors: (1, 5), kind: "reward", count: (1, 1), required: true),
//!     ],
//!     entries: [
//!         (name: "locked store", kind: "reward", piece: "store", floors: (1, 5), weight: "common",
//!          group: "stores", placed: Grown(Locked("key"))),
//!         (name: "side stash", kind: "reward", piece: "stash", floors: (2, 5), not: ["wet"], weight: "rare",
//!          placed: Inside(["room"]), prefer: Secluded),
//!         (name: "puddles", kind: "flavor", dressing: "puddles", floors: (1, 5), where: ["wet"],
//!          weight: "common", per_place: 2, per_run: 4, placed: Dressed, prefer: DeadEnd),
//!     ],
//! )
//! ```

use std::ops::RangeInclusive;

use rand::Rng;
use rand::seq::SliceRandom;
use serde::Deserialize;

use crate::content::{ContentError, Registry};
use crate::dressing::DressingDef;
use crate::layout::LayoutDef;
use crate::names::Names;
use crate::prefab::{Coverage, Legended, PrefabDef, Sources};
use crate::tag::TagId;

/// How a grown piece is joined to what it is attached to: a layout
/// shape's joins, and the two only a piece the pool places may carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Joined {
    /// One cell of the door tile.
    Door,
    /// One cell of floor.
    Arch,
    /// Floor up to this many cells wide, at least one.
    Mouth(u8),
    /// One cell of the locked tile, opened by whoever carries the tag.
    Locked(TagId),
    /// One cell of the sealed tile, a gate the game lifts.
    Sealed,
}

/// How an entry is laid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placed {
    /// A prefab grown as an area of its own, through the entries it draws.
    Grown {
        /// The prefab's name.
        piece: String,
        /// How it is joined.
        join: Joined,
    },
    /// A prefab stamped inside an area.
    Inside {
        /// The prefab's name.
        piece: String,
        /// The layout shapes whose areas it may go in; any if empty.
        shapes: Vec<String>,
    },
    /// A dressing laid over an area of any size.
    Dressed {
        /// The dressing's name, which the dressings' own load checks.
        dressing: String,
    },
}

/// The area an entry stamped inside or dressed prefers. A grown piece has
/// none, since growth decides where it attaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub enum Prefer {
    /// The area farthest from the route that it fits: what a player went
    /// out of their way to find.
    Secluded,
    /// An area with one way in.
    DeadEnd,
    /// An area reached only through a secret.
    Hidden,
    /// An area three ways or more lead out of.
    Junction,
    /// Any area it fits.
    #[default]
    Anywhere,
}

/// One thing a place may be given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The entry's name.
    pub name: String,
    /// An index into [`Pool::kinds`].
    pub kind: usize,
    /// The first and last floor it may stand on.
    pub floors: (i32, i32),
    /// Tags of which a place must carry one; any place if empty.
    pub within: Vec<String>,
    /// Tags a place must carry none of.
    pub not: Vec<String>,
    /// How often it is drawn, the weight its name resolved to.
    pub weight: u32,
    /// The most of it on one floor.
    pub per_place: u32,
    /// The most of it in one run.
    pub per_run: u32,
    /// Entries sharing a group never share a floor.
    pub group: Option<String>,
    /// How it is laid.
    pub placed: Placed,
    /// The area it prefers.
    pub prefer: Prefer,
}

impl Entry {
    /// Whether it may stand on a floor numbered `band` carrying `tags`.
    pub fn fits(&self, band: i32, tags: &[String]) -> bool {
        (self.floors.0..=self.floors.1).contains(&band)
            && (self.within.is_empty() || self.within.iter().any(|t| tags.contains(t)))
            && !self.not.iter().any(|t| tags.contains(t))
    }

    /// The prefab it lays, for a grown piece or one stamped inside.
    pub fn piece(&self) -> Option<&str> {
        match &self.placed {
            Placed::Grown { piece, .. } | Placed::Inside { piece, .. } => Some(piece),
            Placed::Dressed { .. } => None,
        }
    }
}

/// How many picks of a kind each of a run of floors gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    /// The first and last floor it covers.
    pub floors: (i32, i32),
    /// An index into [`Pool::kinds`].
    pub kind: usize,
    /// The fewest and most picks, drawn evenly for each floor.
    pub count: (u32, u32),
    /// Whether a pick of it a place cannot fit fails the place. A run
    /// never quietly loses what a required kind promised; a pick of a kind
    /// that is not required is dropped instead.
    pub required: bool,
}

/// A loaded pool file, every name resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pool {
    /// Every kind a budget names, in the order first named.
    pub kinds: Vec<String>,
    /// Every budget.
    pub budgets: Vec<Budget>,
    /// Every entry.
    pub entries: Vec<Entry>,
}

impl Pool {
    /// The budget of `kind` covering floor `band`, if one does.
    pub fn budget(&self, kind: usize, band: i32) -> Option<&Budget> {
        self.budgets.iter().find(|b| b.kind == kind && (b.floors.0..=b.floors.1).contains(&band))
    }

    /// Every dressing an entry names, for the dressings' load to check.
    pub fn dressings(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().filter_map(|e| match &e.placed {
            Placed::Dressed { dressing } => Some(dressing.as_str()),
            _ => None,
        })
    }
}

/// A pool as authored, before its names are resolved.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PoolRon {
    weights: Vec<(String, u32)>,
    tags: Vec<String>,
    budgets: Vec<BudgetRon>,
    entries: Vec<EntryRon>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BudgetRon {
    floors: (i32, i32),
    kind: String,
    count: (u32, u32),
    #[serde(default)]
    required: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryRon {
    name: String,
    kind: String,
    #[serde(default)]
    piece: Option<String>,
    #[serde(default)]
    dressing: Option<String>,
    floors: (i32, i32),
    #[serde(default, rename = "where")]
    within: Vec<String>,
    #[serde(default)]
    not: Vec<String>,
    weight: String,
    #[serde(default = "one")]
    per_place: u32,
    #[serde(default = "one")]
    per_run: u32,
    #[serde(default)]
    group: Option<String>,
    placed: PlacedRon,
    #[serde(default)]
    prefer: Prefer,
}

fn one() -> u32 {
    1
}

#[derive(Deserialize)]
enum PlacedRon {
    Grown(JoinRon),
    Inside(Vec<String>),
    Dressed,
}

#[derive(Deserialize)]
enum JoinRon {
    Door,
    Arch,
    Mouth(u8),
    Locked(String),
    Sealed,
}

/// Reads a pool file, checking every prefab against `pieces`, every
/// shape against `layout` and every lock's tag against the tags in
/// `names`.
///
/// Reports every problem in the file at once rather than the first, each
/// naming the weight, budget or entry it is about: a name used twice, a
/// weight, tag, kind, prefab, shape or lock tag that resolves to nothing,
/// floors or a count that run backwards, two budgets of one kind covering
/// one floor, an entry whose floors no budget of its kind covers, a limit
/// of nought or a per-place limit past the per-run one, a piece and a
/// dressing both or neither or the wrong one for how it is placed, a grown
/// prefab with no entry to join it by, a mouth of no width, a preference
/// on a grown piece, a grown piece of a kind some budget on its floors
/// does not require, an inside piece whose shapes all draw no room, and a
/// group whose entries are of more than one kind.
pub fn load<M>(text: &str, layout: &LayoutDef, pieces: &Registry<PrefabDef<M>>, names: &Names<'_>) -> Result<Pool, ContentError> {
    let options = ron::options::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
    let a: PoolRon = options.from_str(text).map_err(|e| ContentError::Parse(e.to_string()))?;
    let mut errors = Vec::new();

    let mut twice = |what: &str, names: Vec<&str>| {
        for (i, name) in names.iter().enumerate() {
            if names[..i].contains(name) {
                errors.push(format!("{name}: two {what} share this name"));
            }
        }
    };
    twice("weights", a.weights.iter().map(|(n, _)| n.as_str()).collect());
    twice("tags", a.tags.iter().map(String::as_str).collect());
    twice("entries", a.entries.iter().map(|e| e.name.as_str()).collect());

    let backwards = |(lo, hi): (i32, i32)| lo > hi;
    let mut kinds: Vec<String> = Vec::new();
    let mut budgets: Vec<Budget> = Vec::new();
    for (i, b) in a.budgets.iter().enumerate() {
        let what = format!("budget {i} ({} on {} to {})", b.kind, b.floors.0, b.floors.1);
        if backwards(b.floors) {
            errors.push(format!("{what}: its floors run backwards"));
        }
        if b.count.0 > b.count.1 {
            errors.push(format!("{what}: its count runs backwards, from {} to {}", b.count.0, b.count.1));
        }
        let kind = kinds.iter().position(|k| *k == b.kind).unwrap_or_else(|| {
            kinds.push(b.kind.clone());
            kinds.len() - 1
        });
        for (j, other) in budgets.iter().enumerate() {
            if other.kind == kind && other.floors.0.max(b.floors.0) <= other.floors.1.min(b.floors.1) {
                errors.push(format!("{what}: covers a floor budget {j} of the same kind covers"));
            }
        }
        budgets.push(Budget { floors: b.floors, kind, count: b.count, required: b.required });
    }

    let shape_names: Vec<&str> = layout.shapes.iter().map(|s| s.name.as_str()).collect();
    let mut entries = Vec::new();
    for e in &a.entries {
        let name = &e.name;
        let kind = kinds.iter().position(|k| *k == e.kind);
        match kind {
            None => errors.push(format!("{name}: no budget counts its kind {:?}, so it is never drawn", e.kind)),
            Some(k) if !backwards(e.floors) && !budgets.iter().any(|b| b.kind == k && b.floors.0.max(e.floors.0) <= b.floors.1.min(e.floors.1)) => {
                errors.push(format!("{name}: no budget of its kind {:?} covers any of floors {} to {}, so it is never drawn", e.kind, e.floors.0, e.floors.1));
            }
            Some(_) => {}
        }
        if backwards(e.floors) {
            errors.push(format!("{name}: its floors run backwards, from {} to {}", e.floors.0, e.floors.1));
        }
        let weight = a.weights.iter().find(|(w, _)| *w == e.weight).map(|(_, w)| *w);
        if weight.is_none() {
            errors.push(format!("{name}: names unknown weight {:?}", e.weight));
        }
        for tag in e.within.iter().chain(&e.not) {
            if !a.tags.contains(tag) {
                errors.push(format!("{name}: names unknown tag {tag:?}"));
            }
        }
        if e.per_place == 0 || e.per_run == 0 {
            errors.push(format!("{name}: a limit of nought; an entry kept out of play takes a weight of nought instead"));
        } else if e.per_place > e.per_run {
            errors.push(format!("{name}: per_place is {}, past its per_run of {}", e.per_place, e.per_run));
        }
        let piece = |errors: &mut Vec<String>| -> String {
            let Some(piece) = &e.piece else {
                errors.push(format!("{name}: a grown or inside entry names a piece"));
                return String::new();
            };
            if e.dressing.is_some() {
                errors.push(format!("{name}: names both a piece and a dressing"));
            }
            if pieces.id(piece).is_none() {
                errors.push(format!("{name}: names unknown prefab {piece:?}"));
            }
            piece.clone()
        };
        let placed = match &e.placed {
            PlacedRon::Grown(join) => {
                let piece = piece(&mut errors);
                if let Some(def) = pieces.id(&piece).map(|id| pieces.get(id))
                    && !def.rows().iter().any(|row| row.chars().any(|c| def.entry(c)))
                {
                    errors.push(format!("{name}: grows {piece:?}, which draws no entry to join it by"));
                }
                if e.prefer != Prefer::Anywhere {
                    errors.push(format!("{name}: prefers {:?}, and a grown piece goes where growth attaches it", e.prefer));
                }
                let loose = |b: &&Budget| Some(b.kind) == kind && !b.required && b.floors.0.max(e.floors.0) <= b.floors.1.min(e.floors.1);
                if let Some(b) = budgets.iter().find(loose) {
                    errors.push(format!(
                        "{name}: is grown, and its kind {:?} is not required on floors {} to {}; growth cannot drop a piece it owes",
                        e.kind, b.floors.0, b.floors.1
                    ));
                }
                let join = match join {
                    JoinRon::Door => Joined::Door,
                    JoinRon::Arch => Joined::Arch,
                    JoinRon::Mouth(0) => {
                        errors.push(format!("{name}: Mouth(0) is no way through; a mouth is at least 1 wide"));
                        Joined::Mouth(1)
                    }
                    JoinRon::Mouth(n) => Joined::Mouth(*n),
                    JoinRon::Locked(tag) => match names.tag(tag) {
                        Ok(tag) => Joined::Locked(tag),
                        Err(why) => {
                            errors.push(format!("{name}: {why}"));
                            Joined::Sealed
                        }
                    },
                    JoinRon::Sealed => Joined::Sealed,
                };
                Placed::Grown { piece, join }
            }
            PlacedRon::Inside(shapes) => {
                let piece = piece(&mut errors);
                for shape in shapes.iter().filter(|s| !shape_names.contains(&s.as_str())) {
                    errors.push(format!("{name}: names unknown shape {shape:?}"));
                }
                // A passage is no room, and nothing is stamped in one.
                let roomy = |n: &String| layout.shapes.iter().any(|s| s.name == *n && !matches!(s.kind, crate::layout::ShapeKind::Passage { .. }));
                if !shapes.is_empty() && shapes.iter().all(|n| shape_names.contains(&n.as_str())) && !shapes.iter().any(roomy) {
                    errors.push(format!("{name}: names only shapes that draw no room, so it can never be stamped"));
                }
                Placed::Inside { piece, shapes: shapes.clone() }
            }
            PlacedRon::Dressed => {
                if e.piece.is_some() {
                    errors.push(format!("{name}: is dressed, and names a piece rather than a dressing"));
                }
                let dressing = e.dressing.clone().unwrap_or_else(|| {
                    errors.push(format!("{name}: is dressed and names no dressing"));
                    String::new()
                });
                Placed::Dressed { dressing }
            }
        };
        entries.push(Entry {
            name: name.clone(),
            kind: kind.unwrap_or(0),
            floors: e.floors,
            within: e.within.clone(),
            not: e.not.clone(),
            weight: weight.unwrap_or(0),
            per_place: e.per_place,
            per_run: e.per_run,
            group: e.group.clone(),
            placed,
            prefer: e.prefer,
        });
    }

    // A group's entries share a floor's one place in it; one of a single
    // kind keeps the draw a flow that counts each kind apart.
    for (i, e) in a.entries.iter().enumerate() {
        if let Some(group) = &e.group
            && let Some(other) = a.entries[..i].iter().find(|o| o.group.as_ref() == Some(group) && o.kind != e.kind)
        {
            errors.push(format!("{}: is a {} in group {group:?}, which {} holds as a {}; a group is of one kind", e.name, e.kind, other.name, other.kind));
        }
    }

    if errors.is_empty() { Ok(Pool { kinds, budgets, entries }) } else { Err(ContentError::Invalid(errors)) }
}

/// [`prefab::coverage`](crate::prefab::coverage) over every piece the pool
/// lays, each at the floors its entries may stand on and no others, so a
/// slot is never reported empty on a floor its piece is never laid on.
///
/// A piece two entries name is reported once, at the floors of both. The
/// report's bands run from the first floor any piece stands on to the
/// last, and a row leaves blank each band its piece is not laid at.
pub fn coverage<M, I: Copy>(pool: &Pool, pieces: &Registry<PrefabDef<M>>, sources: &Sources<'_, M, I>) -> Coverage {
    laid_coverage(pool.entries.iter().filter_map(|e| e.piece().map(|p| (p, e.floors))), pieces, sources)
}

/// [`coverage`] over every dressing the pool's dressed entries lay, from
/// the dressing file, at the floors those entries stand on, read the same
/// way. A dressing a district's list lays on every floor is reported by
/// [`prefab::coverage`](crate::prefab::coverage) over the bands it may be
/// laid at.
pub fn dressing_coverage<M, I: Copy>(pool: &Pool, dressings: &Registry<DressingDef<M>>, sources: &Sources<'_, M, I>) -> Coverage {
    let dressed = pool.entries.iter().filter_map(|e| match &e.placed {
        Placed::Dressed { dressing } => Some((dressing.as_str(), e.floors)),
        _ => None,
    });
    laid_coverage(dressed, dressings, sources)
}

/// The coverage of each file `laid` names, by name and the floors an entry
/// lays it at, reported once at the floors of every entry naming it.
fn laid_coverage<'a, M, I: Copy, D: Legended<M>>(
    laid: impl Iterator<Item = (&'a str, (i32, i32))>,
    defs: &Registry<D>,
    sources: &Sources<'_, M, I>,
) -> Coverage {
    // Each file in the order first named, and every floor it stands on.
    let mut at_floors: Vec<(&str, Vec<i32>)> = Vec::new();
    for (name, floors) in laid {
        let at = match at_floors.iter().position(|(p, _)| *p == name) {
            Some(at) => at,
            None => {
                at_floors.push((name, Vec::new()));
                at_floors.len() - 1
            }
        };
        at_floors[at].1.extend(floors.0..=floors.1);
    }
    let first = at_floors.iter().flat_map(|(_, f)| f.iter().copied()).min().unwrap_or(1);
    let last = at_floors.iter().flat_map(|(_, f)| f.iter().copied()).max().unwrap_or(0);
    let mut rows = Vec::new();
    for (name, floors) in at_floors {
        let Some(id) = defs.id(name) else { continue };
        let report = crate::prefab::coverage([defs.get(id)], sources, first..=last);
        rows.extend(report.rows.into_iter().map(|mut row| {
            row.reach.retain(|(band, _)| floors.contains(band));
            row
        }));
    }
    Coverage { bands: RangeInclusive::new(first, last), rows }
}

/// One place a run will build, as the plan reads it: its floor and the
/// tags an entry's `where` and `not` are read against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    /// Its floor, in the numbering the pool's floors are written in.
    pub band: i32,
    /// Its tags, such as the region it stands in.
    pub tags: Vec<String>,
}

/// One entry drawn for one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pick {
    /// An index into [`Pool::entries`].
    pub entry: usize,
    /// Whether the budget it was drawn for is required, so a place that
    /// cannot fit it fails rather than dropping it.
    pub required: bool,
}

impl Pool {
    /// Every place's picks for a whole run, drawn at once: one list per
    /// place, in the order `places` gives them.
    ///
    /// Drawn at once so that every per-run limit and group is checked in
    /// one place and no place's picks depend on which is built first, and
    /// a place's attempts then vary its layout and never its picks. Pure:
    /// the same `rng` state and places plan the same picks. The game passes
    /// a stream derived from the run's seed.
    ///
    /// Each place draws a count for each kind a budget covers it for,
    /// evenly from the budget's range. Then every pick of a required kind
    /// is drawn, in an order drawn from `rng`, and after them every other,
    /// each an entry that fits the place, by weight, under every limit and
    /// group. A draw is kept only when the picks still to make can all be
    /// made as fully as before it, worked out exactly as two flows from the
    /// picks owed through the entries that fit to each entry's per-run
    /// limit: the most of the required picks still owed that can be made,
    /// and the most of all of them. One that lowers either by more than
    /// its own pick is drawn again without that entry. A flow that has
    /// made the most required picks can always go on to make the most of
    /// all, since making more never unmakes one, so the two are kept
    /// together and a required draw never spends an entry a later floor
    /// of the same kind, required there or not, could not do without. So
    /// a budget is met whenever the pool can meet it, a scarce entry is
    /// not spent on a floor others could have filled, and a pick no entry
    /// can fill is left out, one not required before any required one.
    /// Within a place, required picks come first.
    pub fn plan_run(&self, rng: &mut impl Rng, places: &[Place]) -> Vec<Vec<Pick>> {
        let mut plan = Plan::new(self, places);
        // Each place's count of each kind, in place order and then kind
        // order, so a count never moves for a draw made after it.
        let mut owed: [Vec<(usize, usize)>; 2] = [Vec::new(), Vec::new()];
        for (p, place) in places.iter().enumerate() {
            for k in 0..self.kinds.len() {
                if let Some(b) = self.budget(k, place.band) {
                    let count = rng.random_range(b.count.0..=b.count.1);
                    plan.owed[p][k] = count;
                    owed[usize::from(!b.required)].extend(std::iter::repeat_n((p, k), count as usize));
                }
            }
        }
        let mut picks = vec![Vec::new(); places.len()];
        for (tier, slots) in owed.iter_mut().enumerate() {
            let required = tier == 0;
            slots.shuffle(rng);
            for &(p, k) in slots.iter() {
                // The required flow is held only in the required tier;
                // past it, every required pick is made or never can be.
                let most_required = if required { plan.most(true) } else { 0 };
                let most = plan.most(false);
                let mut candidates: Vec<usize> = plan.fitting[p].iter().copied().filter(|&e| self.entries[e].kind == k && plan.open(p, e)).collect();
                plan.owed[p][k] -= 1;
                while !candidates.is_empty() {
                    let total: u64 = candidates.iter().map(|&e| u64::from(self.entries[e].weight)).sum();
                    let mut roll = rng.random_range(0..total);
                    let at = candidates
                        .iter()
                        .position(|&e| {
                            let w = u64::from(self.entries[e].weight);
                            roll < w || {
                                roll -= w;
                                false
                            }
                        })
                        .expect("the roll is below the total");
                    let e = candidates.swap_remove(at);
                    plan.take(p, e, true);
                    if plan.most(false) + 1 == most && (!required || plan.most(true) + 1 == most_required) {
                        picks[p].push(Pick { entry: e, required });
                        break;
                    }
                    plan.take(p, e, false);
                }
                // A pick with no entry left that keeps the rest whole is
                // one no plan could make, and is left out.
            }
        }
        picks
    }
}

/// What a run's plan has drawn so far, and the flow that says whether a
/// draw leaves the rest as fully made as before.
struct Plan<'a> {
    pool: &'a Pool,
    /// Each place's entries that fit it and carry weight.
    fitting: Vec<Vec<usize>>,
    /// Each place's picks of each kind still owed.
    owed: Vec<Vec<u32>>,
    /// Whether each place's budget of each kind is required.
    required: Vec<Vec<bool>>,
    /// Each place's picks of each entry.
    here: Vec<Vec<u32>>,
    /// Each entry's picks in the run.
    run: Vec<u32>,
    /// Every group, by name.
    groups: Vec<&'a str>,
    /// Each entry's group, as an index into `groups`.
    group: Vec<Option<usize>>,
    /// Each place's groups already drawn from.
    shut: Vec<Vec<bool>>,
}

impl<'a> Plan<'a> {
    fn new(pool: &'a Pool, places: &[Place]) -> Self {
        let mut groups: Vec<&str> = Vec::new();
        let group = pool
            .entries
            .iter()
            .map(|e| {
                e.group.as_deref().map(|g| {
                    groups.iter().position(|n| *n == g).unwrap_or_else(|| {
                        groups.push(g);
                        groups.len() - 1
                    })
                })
            })
            .collect();
        let fitting = places
            .iter()
            .map(|pl| (0..pool.entries.len()).filter(|&e| pool.entries[e].weight > 0 && pool.entries[e].fits(pl.band, &pl.tags)).collect())
            .collect();
        let required = places.iter().map(|pl| (0..pool.kinds.len()).map(|k| pool.budget(k, pl.band).is_some_and(|b| b.required)).collect()).collect();
        Self {
            pool,
            fitting,
            owed: vec![vec![0; pool.kinds.len()]; places.len()],
            required,
            here: vec![vec![0; pool.entries.len()]; places.len()],
            run: vec![0; pool.entries.len()],
            shut: vec![vec![false; groups.len()]; places.len()],
            groups,
            group,
        }
    }

    /// Whether place `p` may take one more of entry `e`.
    fn open(&self, p: usize, e: usize) -> bool {
        let entry = &self.pool.entries[e];
        self.here[p][e] < entry.per_place && self.run[e] < entry.per_run && self.group[e].is_none_or(|g| !self.shut[p][g])
    }

    /// Gives place `p` one of entry `e`, or takes it back.
    fn take(&mut self, p: usize, e: usize, give: bool) {
        let step = |n: &mut u32| if give { *n += 1 } else { *n -= 1 };
        step(&mut self.here[p][e]);
        step(&mut self.run[e]);
        if let Some(g) = self.group[e] {
            self.shut[p][g] = give;
        }
    }

    /// The most of the picks still owed, to required budgets alone when
    /// `only_required`, that can all be made together: a maximum flow from
    /// each place's owed picks of a kind, through a gate of one for each
    /// group at that place, to each entry that fits and is open, each
    /// carrying at most its per-place limit less what the place holds,
    /// and on to the end through each entry's per-run limit less what the
    /// run holds. A group is of one kind, so a gate never carries one kind
    /// into another's entries.
    fn most(&self, only_required: bool) -> u32 {
        let mut net = Net::default();
        let (source, sink) = (net.node(), net.node());
        let entries: Vec<usize> = (0..self.pool.entries.len()).map(|_| net.node()).collect();
        for (e, &node) in entries.iter().enumerate() {
            net.edge(node, sink, self.pool.entries[e].per_run - self.run[e]);
        }
        for (p, fitting) in self.fitting.iter().enumerate() {
            let mut gates: Vec<Option<usize>> = vec![None; self.groups.len()];
            for (k, &owed) in self.owed[p].iter().enumerate() {
                if owed == 0 || (only_required && !self.required[p][k]) {
                    continue;
                }
                let slot = net.node();
                net.edge(source, slot, owed);
                for &e in fitting.iter().filter(|&&e| self.pool.entries[e].kind == k && self.open(p, e)) {
                    let room = self.pool.entries[e].per_place - self.here[p][e];
                    match self.group[e] {
                        None => net.edge(slot, entries[e], room),
                        Some(g) => {
                            let gate = *gates[g].get_or_insert_with(|| {
                                let (inn, out) = (net.node(), net.node());
                                net.edge(slot, inn, u32::MAX);
                                net.edge(inn, out, 1);
                                out
                            });
                            net.edge(gate, entries[e], room);
                        }
                    }
                }
            }
        }
        net.flow(source, sink)
    }
}

/// A flow network: each edge stored beside its reverse, at an index one
/// apart, so pushing along one gives the other back.
#[derive(Default)]
struct Net {
    out: Vec<Vec<usize>>,
    to: Vec<usize>,
    cap: Vec<u32>,
}

impl Net {
    fn node(&mut self) -> usize {
        self.out.push(Vec::new());
        self.out.len() - 1
    }

    fn edge(&mut self, from: usize, to: usize, cap: u32) {
        for (a, b, c) in [(from, to, cap), (to, from, 0)] {
            self.out[a].push(self.to.len());
            self.to.push(b);
            self.cap.push(c);
        }
    }

    /// The most that flows from `source` to `sink`, by shortest paths
    /// with room left, each found by a walk out from the source. Flows
    /// here are a run's picks, a few dozen at most.
    fn flow(&mut self, source: usize, sink: usize) -> u32 {
        let mut total = 0;
        loop {
            let mut via: Vec<Option<usize>> = vec![None; self.out.len()];
            let mut queue = std::collections::VecDeque::from([source]);
            while let Some(n) = queue.pop_front() {
                if n == sink {
                    break;
                }
                for &i in &self.out[n] {
                    let m = self.to[i];
                    if self.cap[i] > 0 && m != source && via[m].is_none() {
                        via[m] = Some(i);
                        queue.push_back(m);
                    }
                }
            }
            if via[sink].is_none() {
                return total;
            }
            let mut push = u32::MAX;
            let mut n = sink;
            while let Some(i) = via[n] {
                push = push.min(self.cap[i]);
                n = self.to[i ^ 1];
            }
            let mut n = sink;
            while let Some(i) = via[n] {
                self.cap[i] -= push;
                self.cap[i ^ 1] += push;
                n = self.to[i ^ 1];
            }
            total += push;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tag::TagDef;
    use rl_grid::{TileProps, TileRegistry};

    struct World {
        layout: LayoutDef,
        pieces: Registry<PrefabDef<()>>,
        tags: Registry<TagDef>,
    }

    const LAYOUT: &str = r#"(
        shapes: [
            (name: "room", kind: Rectangle(w: (5, 9), h: (5, 8)), join: Door),
            (name: "run", kind: Passage(width: (1, 2), length: (4, 12), bend_pct: 30), join: Arch),
        ],
        districts: [(name: "rooms", shapes: [("room", 4), ("run", 1)])],
        recipes: [],
    )"#;

    fn world() -> World {
        let mut tiles = TileRegistry::new();
        tiles.register(TileProps::wall("wall")).unwrap();
        tiles.register(TileProps::floor("floor")).unwrap();
        let piece = |name: &str, rows: &str| {
            let entry = if rows.contains('E') { ", 'E': Entry" } else { "" };
            let text = format!(r#"(name: "{name}", rows: [{rows}], legend: {{'#': Tile("wall"), '.': Tile("floor"){entry}}})"#);
            crate::prefab::load::<()>(&text, &tiles, &Names::new()).unwrap()
        };
        let tags = Registry::from_defs(vec![TagDef::new("key")]).unwrap();
        let pieces = Registry::from_defs(vec![
            piece("store", r######""#####", "#...#", "#...#", "##E##""######),
            piece("stash", r#"".#.""#),
            piece("shut", r####""###", "#.#", "###""####),
            crate::prefab::load::<()>(r#"(name: "rack", ground: "floor", rows: ["w"], legend: { 'w': Item(tag: "key") })"#, &tiles, &Names::new().tags(&tags))
                .unwrap(),
        ])
        .unwrap();
        World { layout: crate::layout::load(LAYOUT).unwrap(), pieces, tags }
    }

    fn read(w: &World, text: &str) -> Result<Pool, ContentError> {
        load(text, &w.layout, &w.pieces, &Names::new().tags(&w.tags))
    }

    const POOL: &str = r#"(
        weights: [("rare", 5), ("common", 50)],
        tags: ["wet", "dry"],
        budgets: [
            (floors: (1, 5), kind: "flavor", count: (2, 3)),
            (floors: (1, 5), kind: "reward", count: (1, 1), required: true),
        ],
        entries: [
            (name: "locked store", kind: "reward", piece: "store", floors: (1, 5), weight: "common", group: "stores", placed: Grown(Locked("key"))),
            (name: "side stash", kind: "reward", piece: "stash", floors: (2, 5), not: ["wet"], weight: "rare", placed: Inside(["room"]), prefer: Secluded),
            (name: "puddles", kind: "flavor", dressing: "puddles", floors: (1, 5), where: ["wet"], weight: "common", per_place: 2, per_run: 4, placed: Dressed, prefer: DeadEnd),
        ],
    )"#;

    #[test]
    fn a_pool_file_resolves_every_name_it_carries() {
        let w = world();
        let pool = read(&w, POOL).unwrap();
        assert_eq!(pool.kinds, vec!["flavor", "reward"]);
        assert_eq!(pool.budgets[1], Budget { floors: (1, 5), kind: 1, count: (1, 1), required: true });
        assert!(!pool.budgets[0].required, "a budget is not required unless it says so");
        let store = &pool.entries[0];
        assert_eq!((store.kind, store.weight, store.per_place, store.per_run), (1, 50, 1, 1), "limits default to one");
        assert_eq!(store.placed, Placed::Grown { piece: "store".into(), join: Joined::Locked(w.tags.expect("key")) });
        assert_eq!(store.prefer, Prefer::Anywhere);
        let stash = &pool.entries[1];
        assert_eq!((stash.weight, stash.prefer, &stash.not), (5, Prefer::Secluded, &vec!["wet".to_string()]));
        assert_eq!(stash.placed, Placed::Inside { piece: "stash".into(), shapes: vec!["room".into()] });
        let puddles = &pool.entries[2];
        assert_eq!((puddles.per_place, puddles.per_run, &puddles.within), (2, 4, &vec!["wet".to_string()]));
        assert_eq!(pool.dressings().collect::<Vec<_>>(), vec!["puddles"]);
        let tags = |t: &[&str]| t.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(stash.fits(3, &tags(&["dry"])) && !stash.fits(3, &tags(&["wet", "dry"])) && !stash.fits(1, &tags(&[])));
        assert!(puddles.fits(1, &tags(&["wet"])) && !puddles.fits(1, &tags(&["dry"])));
    }

    #[test]
    fn the_load_refuses_every_mistake_in_a_pool_at_once() {
        let w = world();
        let text = r#"(
            weights: [("rare", 5), ("rare", 6)],
            tags: ["wet"],
            budgets: [
                (floors: (5, 1), kind: "flavor", count: (3, 2)),
                (floors: (1, 5), kind: "reward", count: (1, 1)),
                (floors: (4, 9), kind: "reward", count: (1, 1)),
            ],
            entries: [
                (name: "a", kind: "flavor", piece: "stash", floors: (1, 2), weight: "uncommon", placed: Inside(["hall"])),
                (name: "b", kind: "risk", piece: "nowhere", floors: (1, 2), weight: "rare", placed: Grown(Mouth(0)), prefer: Secluded),
                (name: "c", kind: "reward", piece: "stash", floors: (6, 3), where: ["damp"], weight: "rare", per_place: 2, placed: Inside([])),
                (name: "d", kind: "reward", piece: "shut", dressing: "x", floors: (1, 2), weight: "rare", per_run: 0, group: "g", placed: Grown(Locked("lock"))),
                (name: "e", kind: "flavor", floors: (1, 2), weight: "rare", group: "g", placed: Dressed),
                (name: "a", kind: "reward", piece: "stash", floors: (12, 14), weight: "rare", placed: Dressed),
                (name: "f", kind: "reward", piece: "stash", floors: (1, 2), weight: "rare", placed: Inside(["run"])),
            ],
        )"#;
        let Err(ContentError::Invalid(errors)) = read(&w, text) else { panic!("the pool is refused") };
        let says = |what: &str| assert!(errors.iter().any(|e| e.contains(what)), "no error says {what:?} among {errors:#?}");
        says("rare: two weights share this name");
        says("a: two entries share this name");
        says("budget 0 (flavor on 5 to 1): its floors run backwards");
        says("its count runs backwards, from 3 to 2");
        says("budget 2 (reward on 4 to 9): covers a floor budget 1 of the same kind covers");
        says("a: names unknown weight \"uncommon\"");
        says("a: names unknown shape \"hall\"");
        says("b: no budget counts its kind \"risk\"");
        says("b: names unknown prefab \"nowhere\"");
        says("b: Mouth(0) is no way through");
        says("b: prefers Secluded, and a grown piece");
        says("c: its floors run backwards, from 6 to 3");
        says("c: names unknown tag \"damp\"");
        says("c: per_place is 2, past its per_run of 1");
        says("d: names both a piece and a dressing");
        says("d: a limit of nought");
        says("d: grows \"shut\", which draws no entry");
        says("d: unknown tag \"lock\"");
        says("d: is grown, and its kind \"reward\" is not required on floors 1 to 5");
        says("f: names only shapes that draw no room");
        says("e: is dressed and names no dressing");
        says("e: is a flavor in group \"g\", which d holds as a reward");
        says("a: is dressed, and names a piece rather than a dressing");
        says("a: no budget of its kind \"reward\" covers any of floors 12 to 14");
        assert_eq!(errors.len(), 27, "{errors:#?}");
    }

    /// A piece's slots are reported at the floors its entries stand on,
    /// so a piece laid only deep is never called empty on a shallow floor.
    #[test]
    fn a_pool_s_coverage_reports_each_piece_at_its_own_floors_and_no_others() {
        let w = world();
        let text = POOL.replace(
            "entries: [",
            r#"entries: [
            (name: "rack low", kind: "flavor", piece: "rack", floors: (2, 3), weight: "rare", placed: Inside([])),
            (name: "rack deep", kind: "flavor", piece: "rack", floors: (5, 5), weight: "rare", placed: Inside([])),"#,
        );
        let pool = read(&w, &text).unwrap();
        let roles = Registry::<crate::role::RoleDef<()>>::from_defs(Vec::new()).unwrap();
        let monsters = crate::content::BandedTable::new(Vec::new());
        let row = |lo, hi| vec![crate::loot::LootRow { item: 1u32, bands: (lo, hi), weight: 1, group: (1, 1), tags: vec![w.tags.expect("key")] }];
        let items = crate::loot::LootTable::new(row(1, 9), |n| n.to_string());
        let report = coverage(&pool, &w.pieces, &Sources { roles: &roles, monsters: &monsters, items: &items, tags: &w.tags });
        assert_eq!(report.bands, 1..=5, "from the first floor a piece stands on to the last");
        assert_eq!(report.rows.len(), 1, "a piece two entries name is one row");
        assert_eq!(report.rows[0].reach.iter().map(|(b, _)| *b).collect::<Vec<_>>(), vec![2, 3, 5]);
        assert!(report.render().lines().nth(1).unwrap().ends_with("       ✓   ✓       ✓"), "{}", report.render());

        let items = crate::loot::LootTable::new(Vec::new(), |n: &u32| n.to_string());
        let report = coverage(&pool, &w.pieces, &Sources { roles: &roles, monsters: &monsters, items: &items, tags: &w.tags });
        assert_eq!(report.empties(), vec![("rack", 'w', 2), ("rack", 'w', 3), ("rack", 'w', 5)]);
    }

    /// A pool of dressings alone, since what is laid is no business of
    /// the plan: three kinds, floors and tags that leave some places few
    /// entries, two groups, and for each kind a plain entry that fits
    /// everywhere and may fill a whole budget, so every budget can always
    /// be met.
    const RUN: &str = r#"(
        weights: [("rare", 5), ("uncommon", 20), ("common", 50), ("frequent", 100)],
        tags: ["wet", "dry", "deep"],
        budgets: [
            (floors: (1, 6), kind: "flavor", count: (2, 4)),
            (floors: (7, 10), kind: "flavor", count: (1, 2)),
            (floors: (1, 9), kind: "risk", count: (1, 2), required: true),
            (floors: (1, 10), kind: "reward", count: (0, 2), required: true),
        ],
        entries: [
            (name: "f any", kind: "flavor", dressing: "x", floors: (1, 10), weight: "rare", per_place: 4, per_run: 40, placed: Dressed),
            (name: "f wet", kind: "flavor", dressing: "x", floors: (1, 10), where: ["wet"], weight: "frequent", per_place: 2, per_run: 3, placed: Dressed),
            (name: "f dry", kind: "flavor", dressing: "x", floors: (2, 8), where: ["dry"], weight: "common", per_run: 2, placed: Dressed),
            (name: "f shallow", kind: "flavor", dressing: "x", floors: (1, 4), not: ["wet"], weight: "uncommon", per_run: 2, placed: Dressed),
            (name: "f deep", kind: "flavor", dressing: "x", floors: (5, 10), where: ["deep"], weight: "common", per_run: 2, placed: Dressed),
            (name: "f odd", kind: "flavor", dressing: "x", floors: (3, 3), weight: "rare", placed: Dressed),
            (name: "r any", kind: "risk", dressing: "x", floors: (1, 9), weight: "rare", per_place: 2, per_run: 40, placed: Dressed),
            (name: "r one", kind: "risk", dressing: "x", floors: (1, 9), weight: "common", group: "pair", placed: Dressed),
            (name: "r two", kind: "risk", dressing: "x", floors: (2, 9), weight: "common", group: "pair", per_run: 2, placed: Dressed),
            (name: "r wet", kind: "risk", dressing: "x", floors: (3, 6), where: ["wet"], weight: "uncommon", per_run: 2, placed: Dressed),
            (name: "r deep", kind: "risk", dressing: "x", floors: (6, 9), weight: "uncommon", placed: Dressed),
            (name: "w any", kind: "reward", dressing: "x", floors: (1, 10), weight: "rare", per_place: 2, per_run: 40, placed: Dressed),
            (name: "w a", kind: "reward", dressing: "x", floors: (1, 5), weight: "frequent", group: "shelf", per_run: 2, placed: Dressed),
            (name: "w b", kind: "reward", dressing: "x", floors: (1, 10), weight: "common", group: "shelf", per_run: 2, placed: Dressed),
            (name: "w dry", kind: "reward", dressing: "x", floors: (4, 10), where: ["dry"], weight: "uncommon", placed: Dressed),
        ],
    )"#;

    /// Ten floors, wet and dry by turns, the last four deep.
    fn floors() -> Vec<Place> {
        (1..=10)
            .map(|band| {
                let mut tags = vec![if band % 2 == 0 { "wet" } else { "dry" }.to_string()];
                if band >= 7 {
                    tags.push("deep".to_string());
                }
                Place { band, tags }
            })
            .collect()
    }

    fn plan(pool: &Pool, places: &[Place], seed: u64) -> Vec<Vec<Pick>> {
        use rand::SeedableRng;
        pool.plan_run(&mut rand::rngs::StdRng::seed_from_u64(seed), places)
    }

    const SEEDS: std::ops::Range<u64> = 0..500;

    #[test]
    fn no_limit_is_exceeded_and_no_group_shares_a_floor() {
        let pool = read(&world(), RUN).unwrap();
        let places = floors();
        for seed in SEEDS {
            let picks = plan(&pool, &places, seed);
            let mut run = vec![0; pool.entries.len()];
            for (p, here) in picks.iter().enumerate() {
                let mut groups: Vec<&str> = Vec::new();
                for (i, pick) in here.iter().enumerate() {
                    let e = &pool.entries[pick.entry];
                    run[pick.entry] += 1;
                    assert!(
                        here.iter().filter(|o| o.entry == pick.entry).count() as u32 <= e.per_place,
                        "seed {seed}: {} past its per-place limit on floor {}",
                        e.name,
                        p + 1
                    );
                    if let Some(g) = &e.group {
                        assert!(!groups.contains(&g.as_str()), "seed {seed}: group {g} twice on floor {}", p + 1);
                        groups.push(g);
                    }
                    assert!(here[..i].iter().all(|o| o.required) || !pick.required, "seed {seed}: a required pick comes before any other");
                }
            }
            for (e, n) in run.iter().enumerate() {
                assert!(*n <= pool.entries[e].per_run, "seed {seed}: {} drawn {n} times", pool.entries[e].name);
            }
        }
    }

    #[test]
    fn an_entry_is_never_drawn_outside_its_floors_or_its_tags() {
        let pool = read(&world(), RUN).unwrap();
        let places = floors();
        for seed in SEEDS {
            for (place, here) in places.iter().zip(plan(&pool, &places, seed)) {
                for pick in here {
                    let e = &pool.entries[pick.entry];
                    assert!((e.floors.0..=e.floors.1).contains(&place.band), "seed {seed}: {} on floor {}", e.name, place.band);
                    assert!(e.within.is_empty() || e.within.iter().any(|t| place.tags.contains(t)), "seed {seed}: {} in {:?}", e.name, place.tags);
                    assert!(!e.not.iter().any(|t| place.tags.contains(t)), "seed {seed}: {} in {:?}", e.name, place.tags);
                }
            }
        }
    }

    /// One kind required on the first three floors and not on the next
    /// three, a frequent entry standing on all six that only three may
    /// hold, and a rare one standing on the first three alone: a draw
    /// floor by floor, or the required floors drawn with no thought for
    /// the rest, spends the frequent one where the rare one would do and
    /// leaves the last floors empty.
    const TIGHT: &str = r#"(
        weights: [("rare", 1), ("frequent", 1000)],
        tags: [],
        budgets: [
            (floors: (1, 3), kind: "find", count: (1, 1), required: true),
            (floors: (4, 6), kind: "find", count: (1, 1)),
            (floors: (1, 6), kind: "sight", count: (1, 2)),
        ],
        entries: [
            (name: "everywhere", kind: "find", dressing: "x", floors: (1, 6), weight: "frequent", per_run: 3, placed: Dressed),
            (name: "early", kind: "find", dressing: "x", floors: (1, 3), weight: "rare", per_run: 3, placed: Dressed),
            (name: "sight one", kind: "sight", dressing: "x", floors: (1, 6), weight: "frequent", group: "sights", per_run: 6, placed: Dressed),
            (name: "sight two", kind: "sight", dressing: "x", floors: (1, 6), weight: "rare", group: "sights", per_run: 6, placed: Dressed),
            (name: "sight more", kind: "sight", dressing: "x", floors: (1, 6), weight: "rare", per_run: 2, placed: Dressed),
        ],
    )"#;

    #[test]
    fn every_budget_is_met_whenever_the_pool_can_meet_it() {
        let tight = read(&world(), TIGHT).unwrap();
        let six: Vec<Place> = (1..=6).map(|band| Place { band, tags: Vec::new() }).collect();
        for (pool, places) in [(read(&world(), RUN).unwrap(), floors()), (tight, six)] {
            for seed in SEEDS {
                budgets_met(&pool, &places, seed);
            }
        }
    }

    fn budgets_met(pool: &Pool, places: &[Place], seed: u64) {
        for (place, here) in places.iter().zip(plan(pool, places, seed)) {
            for (k, kind) in pool.kinds.iter().enumerate() {
                let n = here.iter().filter(|p| pool.entries[p.entry].kind == k).count() as u32;
                let (lo, hi) = pool.budget(k, place.band).map_or((0, 0), |b| b.count);
                assert!((lo..=hi).contains(&n), "seed {seed}: floor {} holds {n} {kind}, its budget {lo} to {hi}", place.band);
                assert!(here.iter().filter(|p| pool.entries[p.entry].kind == k).all(|p| p.required == pool.budget(k, place.band).unwrap().required));
            }
        }
    }

    /// A kind required on one floor and not on the next: the required
    /// floor takes the rare entry that stands only there, since the
    /// frequent one is the only one the next floor can hold.
    #[test]
    fn a_required_draw_never_spends_what_a_later_floor_of_the_kind_needs() {
        let pool = read(
            &world(),
            r#"(
                weights: [("rare", 1), ("frequent", 1000)],
                tags: [],
                budgets: [
                    (floors: (1, 1), kind: "reward", count: (1, 1), required: true),
                    (floors: (2, 2), kind: "reward", count: (1, 1)),
                ],
                entries: [
                    (name: "x", kind: "reward", dressing: "x", floors: (1, 2), weight: "frequent", placed: Dressed),
                    (name: "y", kind: "reward", dressing: "x", floors: (1, 1), weight: "rare", placed: Dressed),
                ],
            )"#,
        )
        .unwrap();
        let places: Vec<Place> = (1..=2).map(|band| Place { band, tags: Vec::new() }).collect();
        for seed in SEEDS {
            let picks = plan(&pool, &places, seed);
            assert_eq!(picks, vec![vec![Pick { entry: 1, required: true }], vec![Pick { entry: 0, required: false }]], "seed {seed}");
        }
    }

    /// A draw that would spend the only entry a later floor can take on a
    /// floor another entry could fill is drawn again, so the plan meets
    /// what a draw floor by floor would miss; and where the pool cannot
    /// meet every budget, it meets as many as can be met.
    #[test]
    fn a_scarce_entry_is_kept_for_the_floor_only_it_fits_and_a_short_pool_gives_all_it_has() {
        let w = world();
        let pool = read(
            &w,
            r#"(
                weights: [("rare", 1), ("frequent", 1000)],
                tags: [],
                budgets: [(floors: (1, 3), kind: "reward", count: (1, 1), required: true)],
                entries: [
                    (name: "wide", kind: "reward", dressing: "x", floors: (1, 2), weight: "frequent", placed: Dressed),
                    (name: "narrow", kind: "reward", dressing: "x", floors: (1, 1), weight: "rare", placed: Dressed),
                    (name: "deep", kind: "reward", dressing: "x", floors: (3, 3), weight: "rare", per_place: 1, per_run: 1, placed: Dressed),
                ],
            )"#,
        )
        .unwrap();
        let places: Vec<Place> = (1..=3).map(|band| Place { band, tags: Vec::new() }).collect();
        for seed in 0..100 {
            let names: Vec<Vec<&str>> = plan(&pool, &places, seed).iter().map(|h| h.iter().map(|p| pool.entries[p.entry].name.as_str()).collect()).collect();
            assert_eq!(names, vec![vec!["narrow"], vec!["wide"], vec!["deep"]], "seed {seed}");
        }

        let mut short = pool.clone();
        short.budgets[0].floors = (1, 4);
        let places: Vec<Place> = (1..=4).map(|band| Place { band, tags: Vec::new() }).collect();
        for seed in 0..100 {
            let picks = plan(&short, &places, seed);
            assert_eq!(picks.iter().map(Vec::len).sum::<usize>(), 3, "seed {seed}: three entries of one each fill three of four floors");
            assert!(picks[3].is_empty(), "seed {seed}: nothing fits the fourth");
        }
    }

    #[test]
    fn every_entry_is_drawn_somewhere_over_the_seeds() {
        let pool = read(&world(), RUN).unwrap();
        let places = floors();
        let mut drawn = vec![0; pool.entries.len()];
        for seed in SEEDS {
            for pick in plan(&pool, &places, seed).into_iter().flatten() {
                drawn[pick.entry] += 1;
            }
        }
        for (e, n) in drawn.iter().enumerate() {
            assert!(*n > 0, "{} never drawn", pool.entries[e].name);
        }
    }

    #[test]
    fn the_same_seed_and_places_plan_the_same_picks() {
        let pool = read(&world(), RUN).unwrap();
        let places = floors();
        let mut differ = 0;
        for seed in 0..50 {
            assert_eq!(plan(&pool, &places, seed), plan(&pool, &places, seed), "seed {seed}");
            differ += usize::from(plan(&pool, &places, seed) != plan(&pool, &places, seed + 1));
        }
        assert!(differ > 45, "another seed plans other picks: {differ} of 50");
    }
}
