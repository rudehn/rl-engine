//! Loot: where items turn up, how many, and how often.
//!
//! Which items exist is a game's own registry, and the engine never learns
//! what one is. What is here is the part every game with loot writes the
//! same way, keyed by whatever id the game's registry hands out: a table of
//! what can be found at each band, a drop table for what the dead leave,
//! and a plan for scattering a place's loot on its floor.
//!
//! A band is how deep, how far or how dangerous somewhere is, in the game's
//! own numbering: a deck, a distance from town, a floor.
//!
//! [`LootTable`] draws by weight among the rows that apply at a band. Its
//! rows are sorted by item name when it is built, so the order a spawn file
//! is written in never changes what a seed draws; a row added or removed
//! still does, as it must. A row may give a group, so a slug on the floor
//! lies as a handful of them. [`LootTable::pick_tagged`] draws among the
//! rows whose item carries a tag, which is how a container asks for "a
//! weapon" and gets a better one the deeper it stands: when nothing with
//! the tag is found as deep as asked, the draw falls back to the nearest
//! band above that has something, so a request past the deepest row still
//! gets the deepest thing rather than nothing.
//!
//! [`DropTable`] rolls each of its rows on its own, so one death can leave
//! both, one or neither of two things. [`plan_scatter`] places a place's
//! loot: a count beside each kind of mark a [`ScatterRules`] names, and a
//! number more on free floor.
//!
//! Everything draws from a generator the caller hands in: which stream it
//! is, and so what a kill or a place's loot may never nudge, is the Bevy
//! layer's to decide.

use rand::Rng;
use rl_core::{Id, Point, Rect, geometry};
use serde::Deserialize;

use crate::TagId;
use crate::content::{ContentError, Named, Registry};
use crate::names::{NameRef, Names};

/// One row of a loot table: an item, the bands it turns up at, how often,
/// and how many at once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LootRow<K> {
    /// What is found.
    pub item: K,
    /// The shallowest and deepest band it applies at, both included.
    pub bands: (i32, i32),
    /// How often, against every other row that applies at a band; only
    /// the ratio matters.
    pub weight: u32,
    /// The fewest and most found together, on the floor.
    pub group: (u32, u32),
    /// The tags the item carries, for a draw that asks for one.
    pub tags: Vec<TagId>,
}

impl<K> LootRow<K> {
    /// A row for `item` at every band, weight one, found alone.
    pub fn new(item: K) -> Self {
        Self { item, bands: (i32::MIN, i32::MAX), weight: 1, group: (1, 1), tags: Vec::new() }
    }

    /// Restricts it to `min..=max`.
    pub fn bands(mut self, min: i32, max: i32) -> Self {
        self.bands = (min, max);
        self
    }

    /// Sets the weight.
    pub fn weight(mut self, weight: u32) -> Self {
        self.weight = weight;
        self
    }

    /// Sets how many are found together.
    pub fn group(mut self, min: u32, max: u32) -> Self {
        self.group = (min, max.max(min));
        self
    }

    /// Sets the tags the item carries.
    pub fn tagged(mut self, tags: Vec<TagId>) -> Self {
        self.tags = tags;
        self
    }

    /// Whether it applies at `band`.
    pub fn applies(&self, band: i32) -> bool {
        (self.bands.0..=self.bands.1).contains(&band)
    }
}

/// What can be found at each band, drawn by weight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LootTable<K> {
    rows: Vec<LootRow<K>>,
}

impl<K> Default for LootTable<K> {
    fn default() -> Self {
        Self { rows: Vec::new() }
    }
}

impl<K: Copy> LootTable<K> {
    /// A table of `rows`, sorted by `name` of each row's item and then by
    /// everything else, so the order they were written in draws nothing.
    pub fn new(mut rows: Vec<LootRow<K>>, name: impl Fn(K) -> String) -> Self {
        rows.sort_by_cached_key(|r| (name(r.item), r.bands, r.weight, r.group));
        Self { rows }
    }

    /// Every row, in drawing order.
    pub fn rows(&self) -> &[LootRow<K>] {
        &self.rows
    }

    /// Whether any row's item carries `tag`.
    pub fn carries(&self, tag: TagId) -> bool {
        self.rows.iter().any(|r| r.tags.contains(&tag))
    }

    /// Draws one row that applies at `band` by weight, and how many of it
    /// are found together; `None` when nothing applies.
    pub fn pick(&self, band: i32, rng: &mut impl Rng) -> Option<(K, u32)> {
        let row = draw(self.rows.iter().filter(|r| r.applies(band)), rng)?;
        let (min, max) = row.group;
        let count = if max > min { rng.random_range(min..=max) } else { min };
        Some((row.item, count))
    }

    /// Draws one item carrying `tag` by weight, at `band` or, when nothing
    /// carrying it applies there, at [`band_for`](Self::band_for) it.
    /// `None` only when no row carries the tag at all.
    pub fn pick_tagged(&self, tag: TagId, band: i32, rng: &mut impl Rng) -> Option<K> {
        let band = self.band_for(tag, band)?;
        draw(self.rows.iter().filter(|r| r.tags.contains(&tag) && r.applies(band)), rng).map(|r| r.item)
    }

    /// The band a draw for `tag` at `band` is made at: `band` itself when
    /// something carrying the tag applies there, else the nearest band
    /// above it that has something, so a request deeper than the table goes
    /// gets its deepest, and else the nearest below, for a request
    /// shallower than anything carrying the tag. `None` when no row carries
    /// it.
    pub fn band_for(&self, tag: TagId, band: i32) -> Option<i32> {
        let tagged: Vec<&LootRow<K>> = self.rows.iter().filter(|r| r.tags.contains(&tag) && r.weight > 0).collect();
        if tagged.is_empty() {
            return None;
        }
        if tagged.iter().any(|r| r.applies(band)) {
            return Some(band);
        }
        let shallower = tagged.iter().map(|r| r.bands.1).filter(|deepest| *deepest < band).max();
        shallower.or_else(|| tagged.iter().map(|r| r.bands.0).filter(|shallowest| *shallowest > band).min())
    }

    /// The bands in `range` where nothing at all would be found, for a
    /// guard test.
    pub fn gaps(&self, range: std::ops::RangeInclusive<i32>) -> Vec<i32> {
        range.filter(|b| !self.rows.iter().any(|r| r.applies(*b) && r.weight > 0)).collect()
    }
}

/// One of `rows` by weight, or `None` when their weights sum to nothing.
fn draw<'a, K>(rows: impl Iterator<Item = &'a LootRow<K>> + Clone, rng: &mut impl Rng) -> Option<&'a LootRow<K>>
where
    K: 'a,
{
    let total: u64 = rows.clone().map(|r| u64::from(r.weight)).sum();
    if total == 0 {
        return None;
    }
    let mut roll = rng.random_range(0..total);
    for row in rows {
        if u64::from(row.weight) > roll {
            return Some(row);
        }
        roll -= u64::from(row.weight);
    }
    None
}

/// A spawn file's row as authored, for [`load`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
struct Authored<D: 'static> {
    item: NameRef<D>,
    bands: (i32, i32),
    weight: u32,
    #[serde(default)]
    group: Option<(u32, u32)>,
}

/// Loads a loot table from a spawn file, a RON list of rows, each item
/// named in the game's own `items`, which `names` must resolve through
/// `Names::with`, and `tags` saying which tags each carries.
///
/// Every field of a row:
///
/// - `item`: a name in the game's own item registry.
/// - `bands`: `(shallowest, deepest)`, both included.
/// - `weight`: how often against every other row that applies at a band;
///   only the ratio matters.
/// - `group`: optional; `(fewest, most)` found together on the floor; one
///   when left out.
///
/// An item may have several rows, and one with none is never found by a
/// table. Names that resolve to nothing are reported together first; once
/// every name resolves, every other problem in the file is reported at
/// once.
///
/// ```
/// use rl_rules::{Named, Names, Registry, TagDef, loot};
///
/// struct Thing(String, bool);
/// impl Named for Thing {
///     fn name(&self) -> &str {
///         &self.0
///     }
/// }
///
/// let tags = Registry::from_defs(vec![TagDef::new("weapon")]).unwrap();
/// let things = Registry::from_defs(vec![Thing("blade".into(), true), Thing("slug".into(), false)]).unwrap();
/// let names = Names::new().tags(&tags).with("item", &things);
/// let weapon = tags.expect("weapon");
/// let table = loot::load(
///     r#"[(item: "slug", bands: (1, 10), weight: 4, group: (2, 5)), (item: "blade", bands: (1, 10), weight: 1)]"#,
///     &names,
///     &things,
///     |t: &Thing| if t.1 { vec![weapon] } else { vec![] },
/// )
/// .unwrap();
/// assert_eq!(table.rows()[0].item, things.expect("blade"), "sorted by name, whatever order they were written in");
/// assert_eq!(table.rows()[1].group, (2, 5));
/// assert!(table.carries(weapon));
/// ```
pub fn load<D: Named + 'static>(text: &str, names: &Names<'_>, items: &Registry<D>, tags: impl Fn(&D) -> Vec<TagId>) -> Result<LootTable<Id<D>>, ContentError> {
    let authored: Vec<Authored<D>> = names.load_list(text)?;
    let mut errors = Vec::new();
    let mut rows = Vec::new();
    for (i, a) in authored.iter().enumerate() {
        let id = a.item.id();
        let at = format!("entry {} ({})", i + 1, items.get(id).name());
        if a.bands.0 > a.bands.1 {
            errors.push(format!("{at}: bands {} to {} is no range at all", a.bands.0, a.bands.1));
        }
        if a.weight == 0 {
            errors.push(format!("{at}: a weight of nothing is never drawn; leave the row out instead"));
        }
        let group = a.group.unwrap_or((1, 1));
        if group.0 == 0 || group.0 > group.1 {
            errors.push(format!("{at}: a group of {} to {} is no group at all", group.0, group.1));
        }
        rows.push(LootRow { item: id, bands: a.bands, weight: a.weight, group, tags: tags(items.get(id)) });
    }
    if !errors.is_empty() {
        return Err(ContentError::Invalid(errors));
    }
    Ok(LootTable::new(rows, |id| items.get(id).name().to_string()))
}

/// One thing the dead may leave: how likely, and how many.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropRow<K> {
    /// What.
    pub item: K,
    /// Percent chance, rolled on its own.
    pub pct: u32,
    /// The fewest and most left, when it is left at all.
    pub count: (u32, u32),
}

impl<K> DropRow<K> {
    /// `item`, one of it, `pct` percent of the time.
    pub fn new(item: K, pct: u32) -> Self {
        Self { item, pct, count: (1, 1) }
    }

    /// Leaves `min..=max` of it when it is left.
    pub fn count(mut self, min: u32, max: u32) -> Self {
        self.count = (min, max.max(min));
        self
    }
}

/// What the dead leave, each row rolled on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropTable<K>(pub Vec<DropRow<K>>);

impl<K> Default for DropTable<K> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<K: Copy> DropTable<K> {
    /// Rolls every row in order, once each: one roll under a hundred
    /// against its chance, and a count for each that is left. A row missed
    /// is not retried, so two rows may give both, one or neither.
    pub fn roll(&self, rng: &mut impl Rng) -> Vec<(K, u32)> {
        let mut left = Vec::new();
        for row in &self.0 {
            if rng.random_range(0..100) < row.pct {
                let (min, max) = row.count;
                left.push((row.item, if max > min { rng.random_range(min..=max) } else { min }));
            }
        }
        left
    }

    /// Whether it leaves nothing, ever.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// How a place's loot is laid out when it is first built: how many items
/// beside each kind of mark, and how many more loose on the floor.
///
/// A mark is a place's own, a spot a prefab or a generator tagged, in the
/// game's numbering; what goes beside one is drawn from the same table as
/// everything else, at the place's band.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScatterRules {
    /// A mark's tag, and how many items lie beside each mark with it.
    pub spots: Vec<(u32, u32)>,
    /// The fewest and most loose items a place gets before its band adds.
    pub loose: (u32, u32),
    /// How many more loose items each band adds.
    pub per_band: u32,
}

impl ScatterRules {
    /// Nothing beside any mark, and nothing loose.
    pub fn new() -> Self {
        Self::default()
    }

    /// Puts `items` beside every mark tagged `tag`.
    pub fn at_spot(mut self, tag: u32, items: u32) -> Self {
        self.spots.push((tag, items));
        self
    }

    /// Scatters `min..=max` loose items before the band adds any.
    pub fn loose(mut self, min: u32, max: u32) -> Self {
        self.loose = (min, max.max(min));
        self
    }

    /// Adds `n` loose items for each band.
    pub fn per_band(mut self, n: u32) -> Self {
        self.per_band = n;
        self
    }

    /// How many loose items a place at `band` gets. A negative band adds
    /// nothing.
    pub fn loose_count(&self, band: i32, rng: &mut impl Rng) -> u32 {
        let (min, max) = self.loose;
        let base = if max > min { rng.random_range(min..=max) } else { min };
        base + self.per_band * band.max(0) as u32
    }
}

/// One item a scatter plan puts down: which, how many together, and where.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed<K> {
    /// What.
    pub item: K,
    /// How many, together.
    pub count: u32,
    /// Where.
    pub at: Point,
}

/// Where a place's loot may go: its marks, each a tag and a cell, and the
/// bounds of its floor.
#[derive(Debug, Clone, Copy)]
pub struct Layout<'a> {
    /// The place's marks, in the game's own numbering.
    pub marks: &'a [(u32, Point)],
    /// The floor a loose item may be put anywhere on.
    pub bounds: Rect,
}

/// Plans a place's loot at `band`: `rules`' count beside each of the
/// layout's marks with a tag it names, then `loose` more on free floor in
/// its bounds, each drawn from `table`, no two on one cell.
///
/// Beside a mark rather than on it, because a mark is where a prop goes: a
/// locker stands on its mark, and an item under it is one nothing can pick
/// up. `free` says where one may lie. A loose item gives up after forty
/// tries at a free cell, so a place with nowhere left stops rather than
/// spinning, and a mark with no free cell beside it is left bare.
pub fn plan_scatter<K: Copy>(
    table: &LootTable<K>,
    band: i32,
    rules: &ScatterRules,
    layout: Layout<'_>,
    loose: u32,
    free: &mut impl FnMut(Point) -> bool,
    rng: &mut impl Rng,
) -> Vec<Placed<K>> {
    let Layout { marks, bounds } = layout;
    let mut placed: Vec<Placed<K>> = Vec::new();
    let taken = |placed: &[Placed<K>], p: Point| placed.iter().any(|q| q.at == p);
    for &(tag, items) in &rules.spots {
        for &(_, mark) in marks.iter().filter(|(t, _)| *t == tag) {
            for _ in 0..items {
                let Some(at) = geometry::square(mark, 1).find(|&p| p != mark && free(p) && !taken(&placed, p)) else { break };
                let Some((item, count)) = table.pick(band, rng) else { break };
                placed.push(Placed { item, count, at });
            }
        }
    }
    for _ in 0..loose {
        for _ in 0..40 {
            let at = Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom()));
            if !free(at) || taken(&placed, at) {
                continue;
            }
            if let Some((item, count)) = table.pick(band, rng) {
                placed.push(Placed { item, count, at });
            }
            break;
        }
    }
    placed
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use rand::seq::SliceRandom;

    const WEAPON: TagId = TagId::from_raw(0);
    const ARMOR: TagId = TagId::from_raw(1);

    /// Blades that get better with depth, plate, and slugs found in
    /// handfuls, named by the letters they are keyed by.
    fn rows() -> Vec<LootRow<char>> {
        vec![
            LootRow::new('a').bands(1, 3).weight(3).tagged(vec![WEAPON]),
            LootRow::new('b').bands(3, 6).weight(2).tagged(vec![WEAPON]),
            LootRow::new('c').bands(6, 8).weight(1).tagged(vec![WEAPON]),
            LootRow::new('p').bands(2, 8).weight(2).tagged(vec![ARMOR]),
            LootRow::new('s').bands(1, 8).weight(4).group(2, 5),
        ]
    }

    fn table(rows: Vec<LootRow<char>>) -> LootTable<char> {
        LootTable::new(rows, |c| c.to_string())
    }

    /// The order a spawn file is written in is not part of the seed: the
    /// same rows shuffled any way draw the same things from the same
    /// generator.
    #[test]
    fn the_order_rows_are_written_in_draws_nothing() {
        let draws = |t: &LootTable<char>, seed: u64| {
            let mut rng = StdRng::seed_from_u64(seed);
            (0..50).map(|i| t.pick(1 + i % 8, &mut rng)).collect::<Vec<_>>()
        };
        let written = table(rows());
        for seed in 0..16u64 {
            let mut shuffled = rows();
            shuffled.shuffle(&mut StdRng::seed_from_u64(seed ^ 0xABCD));
            assert_eq!(draws(&table(shuffled), seed), draws(&written, seed), "seed {seed}");
        }
    }

    #[test]
    fn a_pick_applies_at_its_band_and_its_group_is_honoured() {
        let t = table(rows());
        let mut rng = StdRng::seed_from_u64(3);
        for band in 1..=8 {
            for _ in 0..200 {
                let (item, count) = t.pick(band, &mut rng).expect("something at every band");
                let row = t.rows().iter().find(|r| r.item == item).unwrap();
                assert!(row.applies(band), "{item} drawn at {band}");
                assert!((row.group.0..=row.group.1).contains(&count), "{item} in a group of {count}");
            }
        }
        assert_eq!(t.pick(20, &mut rng), None, "nothing past the table");
    }

    /// A draw for a tag gives only what carries it, from as deep as asked:
    /// a weapon at band 7 is the best one, never the one found at the top.
    #[test]
    fn a_tagged_draw_gives_only_what_carries_the_tag_from_as_deep_as_asked() {
        let t = table(rows());
        let mut rng = StdRng::seed_from_u64(5);
        for band in 1..=8 {
            for _ in 0..100 {
                let item = t.pick_tagged(WEAPON, band, &mut rng).expect("a weapon at every band");
                assert!(['a', 'b', 'c'].contains(&item), "{item} at {band}");
                assert!(t.rows().iter().any(|r| r.item == item && r.applies(band)));
            }
        }
        assert!((0..100).all(|_| t.pick_tagged(WEAPON, 7, &mut rng) == Some('c')), "deep, the deep one");
    }

    /// Past the deepest row, the deepest thing; above the shallowest, the
    /// shallowest; and a tag nothing carries, nothing.
    #[test]
    fn a_band_past_the_table_falls_back_to_the_nearest_band_that_has_something() {
        let t = table(rows());
        assert_eq!(t.band_for(WEAPON, 12), Some(8), "deeper than the table goes: its deepest");
        assert_eq!(t.band_for(ARMOR, 1), Some(2), "shallower than any plate: the shallowest plate");
        assert_eq!(t.band_for(ARMOR, 5), Some(5), "where something applies, there");
        let mut rng = StdRng::seed_from_u64(9);
        assert!((0..100).all(|_| t.pick_tagged(WEAPON, 30, &mut rng) == Some('c')));
        assert_eq!(t.pick_tagged(TagId::from_raw(7), 3, &mut rng), None, "nothing carries it");

        let gapped = table(vec![LootRow::new('x').bands(1, 2).tagged(vec![WEAPON]), LootRow::new('y').bands(6, 9).tagged(vec![WEAPON])]);
        assert_eq!(gapped.band_for(WEAPON, 4), Some(2), "in a gap, the nearest band above, never deeper than asked");
    }

    /// Deeper never draws from shallower than a shallower request would:
    /// the band a tagged draw is made at only ever grows with the band
    /// asked for.
    #[test]
    fn a_deeper_request_is_never_drawn_from_a_shallower_band() {
        let t = table(rows());
        for tag in [WEAPON, ARMOR] {
            let bands: Vec<i32> = (-5..=20).map(|b| t.band_for(tag, b).unwrap()).collect();
            assert!(bands.windows(2).all(|w| w[0] <= w[1]), "{bands:?}");
        }
    }

    #[test]
    fn each_drop_is_rolled_on_its_own_near_its_rate() {
        let drops = DropTable(vec![DropRow::new('s', 10), DropRow::new('k', 50).count(2, 4)]);
        let mut rng = StdRng::seed_from_u64(4);
        let (mut slugs, mut keys, mut both) = (0, 0, 0);
        for _ in 0..10_000 {
            let left = drops.roll(&mut rng);
            let s = left.iter().any(|(i, _)| *i == 's');
            let k = left.iter().find(|(i, _)| *i == 'k');
            if let Some((_, n)) = k {
                assert!((2..=4).contains(n));
            }
            slugs += usize::from(s);
            keys += usize::from(k.is_some());
            both += usize::from(s && k.is_some());
        }
        assert!((800..1200).contains(&slugs), "ten percent: {slugs}");
        assert!((4700..5300).contains(&keys), "half: {keys}");
        assert!((350..650).contains(&both), "rolled apart, so both about one time in twenty: {both}");
    }

    /// Over many seeds and a floor with walls in it: nothing lands on a
    /// cell that is not free, no two land on one cell, a mark gets its own
    /// count beside it and never on it, and the loose ones are all there
    /// when there is room.
    #[test]
    fn a_scatter_lands_only_on_free_cells_one_to_a_cell_beside_its_marks() {
        let t = table(rows());
        let rules = ScatterRules::new().at_spot('A' as u32, 1).at_spot('L' as u32, 2).loose(3, 3).per_band(1);
        let bounds = Rect::new(0, 0, 30, 20);
        let marks = [('A' as u32, Point::new(5, 5)), ('L' as u32, Point::new(20, 10)), ('L' as u32, Point::new(10, 15))];
        for seed in 0..32u64 {
            let wall = |p: Point| p.x == 15 && p.y != 10;
            let mut free = |p: Point| bounds.contains(p) && !wall(p) && !marks.iter().any(|(_, m)| *m == p);
            let mut rng = StdRng::seed_from_u64(seed);
            let band = (seed % 8) as i32 + 1;
            let loose = rules.loose_count(band, &mut rng);
            assert_eq!(loose, 3 + band as u32);
            let plan = plan_scatter(&t, band, &rules, Layout { marks: &marks, bounds }, loose, &mut free, &mut rng);
            let mut cells: Vec<Point> = plan.iter().map(|p| p.at).collect();
            cells.sort();
            cells.dedup();
            assert_eq!(cells.len(), plan.len(), "seed {seed}: two on one cell");
            assert!(plan.iter().all(|p| !wall(p.at) && bounds.contains(p.at)), "seed {seed}: on a wall or off the floor");
            for (tag, mark) in marks {
                let beside = plan.iter().filter(|p| geometry::chebyshev(p.at, mark) == 1).count();
                let owed = if tag == 'A' as u32 { 1 } else { 2 };
                assert!(beside >= owed, "seed {seed}: {owed} owed beside {mark:?}, {beside} there");
                assert!(plan.iter().all(|p| p.at != mark), "seed {seed}: on the mark itself");
            }
            assert_eq!(plan.len() as u32, 1 + 2 * 2 + loose, "seed {seed}: every item placed on a floor with room");
        }
    }

    #[test]
    fn a_spawn_file_names_every_problem_at_once() {
        struct Thing(String);
        impl Named for Thing {
            fn name(&self) -> &str {
                &self.0
            }
        }
        let things = Registry::from_defs(vec![Thing("slug".into())]).unwrap();
        let names = Names::new().with("item", &things);
        let err = load(r#"[(item: "slug", bands: (5, 1), weight: 0, group: (3, 1))]"#, &names, &things, |_| Vec::new())
            .expect_err("a file this wrong does not load")
            .to_string();
        for said in ["no range", "weight of nothing", "no group"] {
            assert!(err.contains(said), "{said:?} in {err}");
        }
        let err = load(r#"[(item: "slugg", bands: (1, 2), weight: 1)]"#, &names, &things, |_| Vec::new()).expect_err("an unknown item").to_string();
        assert!(err.contains("slugg"), "{err}");
    }
}
