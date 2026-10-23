//! What lies about on a floor the first time the adventurer arrives on it:
//! a budget of things drawn by kind, a scroll of enchanting besides, and
//! some gold.
//!
//! [`plan`] and [`laid_out`] are the pure half, tested without an `App`:
//! [`things_on`] a floor's number of things, each a kind drawn by its
//! weight in [`KINDS`] among the kinds with anything to find at the floor's
//! depth, then a thing of that kind as `item_spawns.ron` weights it there,
//! as many together as its row says; and where each goes, the [`LAID`]
//! scroll's cell drawn before anything else's, so a floor too crowded for
//! its whole budget drops some of the budget and never the scroll. [`lay`]
//! is the Bevy over it: on the arrival that builds a floor it lays the
//! floor's things, its scroll and a few piles of gold, made by the
//! [`Armory`] as anything found is, so a weapon found deep is the more
//! often better and a ring may be cursed.
//!
//! Undermere's own rather than the engine's scatter, which draws every
//! thing from the one table at once: a floor meant to hold potions and
//! scrolls three times as often as weapons would hold them only as often
//! as the table happened to list them, and every potion added to the table
//! would make weapons rarer. Drawing the kind first keeps each kind's share
//! the content sheet's however many members it has, as Brogue draws a
//! category and then a thing of it. What the dead leave and what a chest
//! holds are still the engine's to make.
//!
//! The town holds nothing: what is worth finding is under the lake.

use std::ops::RangeInclusive;

use atglyph::prelude::*;
use atglyph::rand::Rng;
use bevy::prelude::*;

use crate::armory::{Armory, ItemDef};
use crate::dungeon::floor_of;
use crate::stairs::Gate;

/// The kinds a floor's things are drawn from, by the tag each kind's
/// members carry, and how often each is drawn against the others: potions
/// and scrolls most, weapons and armor a third as often, staffs, wands,
/// rings and thrown things less, and a waystone, the way home, rarest of
/// all, and only where `item_spawns.ron` lays one, from floor five: about
/// one run in two finds one.
pub const KINDS: [(&str, u32); 9] =
    [("potion", 30), ("scroll", 30), ("weapon", 10), ("armor", 10), ("staff", 6), ("wand", 5), ("ring", 5), ("thrown", 4), ("waystone", 1)];

/// The things every floor holds before its depth adds any.
pub const BASE_THINGS: u32 = 6;

/// What every floor holds one of besides its things and its gold, by name:
/// the scroll of enchanting, so a run is sure of one a floor whatever the
/// draws give, and enchanting is what a run is built with.
pub const LAID: &str = "scroll of enchanting";

/// The gold a floor holds is piles of this, by name, each its own roll
/// times the floor.
pub const GOLD: &str = "gold coin";

/// Why the armory cannot stand for what a floor lays, every problem at
/// once: the [`LAID`] scroll and the [`GOLD`] a floor holds are named here
/// and must be items of it, and every kind of [`KINDS`] must be a tag with
/// something to find on some floor, or its weight is a share of every
/// floor's draw given to nothing.
pub fn problems(armory: &Armory, registries: &Registries) -> Vec<String> {
    let mut wrong: Vec<String> =
        [LAID, GOLD].into_iter().filter(|name| armory.defs.id(name).is_none()).map(|name| format!("{name:?}, which every floor lays, is no item")).collect();
    for (name, _) in KINDS {
        let Some(tag) = registries.tags.id(name) else {
            wrong.push(format!("kind {name:?} is no tag"));
            continue;
        };
        if !armory.table.rows().iter().any(|r| r.weight > 0 && r.tags.contains(&tag) && r.bands.0 <= crate::dungeon::FLOORS as i32 && r.bands.1 >= 1) {
            wrong.push(format!("kind {name:?} has nothing to find on any floor"));
        }
    }
    wrong
}

/// How many piles of gold a floor holds besides its things: one in the
/// middle, as the table that drew gold among everything else used to come
/// to, and none or two as often.
pub const GOLD_PILES: RangeInclusive<u32> = 0..=2;

/// How many things floor `floor` holds besides its gold and the [`LAID`]
/// scroll: [`BASE_THINGS`], and one more for every two floors down, so
/// floor one holds six and the throne's eleven.
pub fn things_on(floor: u32) -> u32 {
    BASE_THINGS + floor / 2
}

/// The armory's table a kind at a time: each of [`KINDS`] with its weight
/// and the rows of the table whose item carries its tag.
pub struct Kinds(Vec<(u32, LootTable<Id<ItemDef>>)>);

impl Kinds {
    /// The [`KINDS`] of `armory`'s table, each tag read in `registries`.
    /// Panics on a kind naming a tag the registries have not, which is a
    /// typo in [`KINDS`] rather than a content file's.
    pub fn of(armory: &Armory, registries: &Registries) -> Self {
        let kind = |(name, weight): &(&str, u32)| {
            let tag = registries.tags.expect(name);
            let rows = armory.table.rows().iter().filter(|row| row.tags.contains(&tag)).cloned().collect();
            (*weight, LootTable::new(rows, |id| armory.defs.get(id).name.clone()))
        };
        Self(KINDS.iter().map(kind).collect())
    }
}

/// Plans `count` things at `band`, each with how many lie together: a kind
/// drawn by its weight among the kinds with something to find at `band`,
/// then a thing of that kind as its rows weight it there.
///
/// A kind with nothing at `band` is left out of the draw rather than drawn
/// and found empty, so its share goes to the others: a floor before any
/// staff is written holds its full count of other things. Nothing at all
/// to find comes to nothing.
pub fn plan(kinds: &Kinds, band: i32, count: u32, rng: &mut impl Rng) -> Vec<(Id<ItemDef>, u32)> {
    let open: Vec<&(u32, LootTable<Id<ItemDef>>)> = kinds.0.iter().filter(|(weight, table)| *weight > 0 && table.gaps(band..=band).is_empty()).collect();
    let total: u32 = open.iter().map(|(weight, _)| weight).sum();
    let mut things = Vec::new();
    if total == 0 {
        return things;
    }
    for _ in 0..count {
        let mut roll = rng.random_range(0..total);
        for (weight, table) in &open {
            if roll < *weight {
                things.extend(table.pick(band, rng));
                break;
            }
            roll -= weight;
        }
    }
    things
}

/// What stands on a cell for good, which nothing is laid on: a prop, or a
/// way through.
type Standing = Or<(With<Prop>, With<Stair>, With<Gate>)>;

/// What laying a floor's things reads.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Ground<'w, 's> {
    armory: Res<'w, Armory>,
    registries: Res<'w, Registries>,
    map: Res<'w, WorldMap>,
    seed: Res<'w, Seed>,
    standing: Query<'w, 's, (&'static Position, &'static OnMap), Standing>,
}

/// What floor `floor` holds and where, among the `open` cells: the `laid`
/// scroll on a cell drawn before anything else, so however crowded the
/// floor it holds its scroll; then its [`things_on`] planned at its own
/// band, each with how many lie together, and [`GOLD_PILES`] piles of
/// `gold`, each on a cell of its own while cells last, so a floor with too
/// few drops the last of its budget and never the scroll.
///
/// The pure half of [`lay`], tested without an `App`: the scroll's cell,
/// the plan, the gold, then each cell in turn, from `rng`.
pub fn laid_out(kinds: &Kinds, laid: Id<ItemDef>, gold: Id<ItemDef>, floor: u32, mut open: Vec<Point>, rng: &mut impl Rng) -> Vec<(Id<ItemDef>, u32, Point)> {
    let draw = |open: &mut Vec<Point>, rng: &mut dyn atglyph::rand::RngCore| (!open.is_empty()).then(|| open.swap_remove(rng.random_range(0..open.len())));
    let mut out: Vec<(Id<ItemDef>, u32, Point)> = draw(&mut open, rng).map(|at| (laid, 1, at)).into_iter().collect();
    let mut things = plan(kinds, floor as i32, things_on(floor), rng);
    things.extend((0..rng.random_range(GOLD_PILES)).map(|_| (gold, 1)));
    for (def, count) in things {
        let Some(at) = draw(&mut open, rng) else { break };
        out.push((def, count, at));
    }
    out
}

/// Lays every floor's things on the arrival that builds it, as
/// [`laid_out`] places them, each made by the [`Armory`] as a thing found
/// on that floor.
///
/// A cell one could walk to with nothing standing on it: no prop, since a
/// thing under a chest is one nothing can pick up, and no way through,
/// since a thing lying on the stairs would be drawn over them. Draws from
/// `seed.place(map).rng(FINDS, 0)`, the floor's own stream, so what lies on floor two never depends on what floor one drew
/// or which was entered first; where everything goes, then what is made
/// there, in that order.
pub fn lay(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, ground: Ground) {
    let Ground { armory, registries, map, seed, standing } = &ground;
    for ev in entered.read().filter(|ev| ev.first) {
        let (Some(floor), Some(place)) = (floor_of(ev.map), map.place(ev.map)) else { continue };
        let mut rng = seed.place(ev.map).rng(crate::seeds::FINDS, 0);
        let taken: Vec<Point> = standing.iter().filter(|(_, on)| on.0 == ev.map).map(|(at, _)| at.0).collect();
        let open: Vec<Point> = place.terrain.bounds().cells().filter(|p| map.is_walkable_on(ev.map, *p) && !taken.contains(p)).collect();
        let (laid, gold) = (armory.defs.expect(LAID), armory.defs.expect(GOLD));
        let found = Provenance { found: Found::Scatter, band: floor as i32 };
        for (def, count, at) in laid_out(&Kinds::of(armory, registries), laid, gold, floor, open, &mut rng) {
            for thing in armory.make(&mut commands, registries, def, count, found, &mut rng) {
                commands.entity(thing).insert((Position(at), OnMap(ev.map)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use atglyph::rand::SeedableRng;

    use super::*;
    use crate::content::Library;
    use crate::dungeon::FLOORS;

    fn library() -> Library {
        Library::load(&crate::content::effect_kinds(), &Moments::default())
    }

    /// Six things on floor one, and one more for every two floors down, to
    /// the throne's eleven.
    #[test]
    fn every_floor_holds_six_things_and_one_more_for_every_two_floors_down() {
        assert_eq!((1..=FLOORS).map(things_on).collect::<Vec<_>>(), [6, 7, 7, 8, 8, 9, 9, 10, 10, 11]);
    }

    /// What a floor's things are drawn as, on made-up kinds: a kind by its
    /// weight among the kinds with anything to find at the band, three
    /// times as often for a kind weighed three times another, never a kind
    /// with nothing at the band or of nought weight, then one of its
    /// members found at the band, as many together as its row says, over a
    /// span of seeds. That the shipped [`KINDS`] weigh potions and scrolls
    /// at three times weapons is their own numbers, read where they are
    /// written.
    #[test]
    fn a_floors_things_are_drawn_by_kind_each_as_often_as_its_weight_says_over_a_span_of_seeds() {
        let item = Id::<ItemDef>::from_raw;
        let table = |rows: Vec<LootRow<Id<ItemDef>>>| LootTable::new(rows, |id| format!("{}", id.index()));
        let kinds = Kinds(vec![
            (30, table(vec![LootRow::new(item(0)).bands(1, 9).group(2, 3), LootRow::new(item(1)).bands(1, 9)])),
            (10, table(vec![LootRow::new(item(2)).bands(1, 9)])),
            (50, table(vec![LootRow::new(item(3)).bands(7, 9)])),
            (0, table(vec![LootRow::new(item(4)).bands(1, 9)])),
        ]);
        let mut drawn = [0u32; 5];
        for seed in 0..400 {
            let things = plan(&kinds, 5, 10, &mut atglyph::rand::rngs::StdRng::seed_from_u64(seed));
            assert_eq!(things.len(), 10, "seed {seed}: as many as asked for");
            for (def, count) in things {
                let together = if def == item(0) { 2..=3 } else { 1..=1 };
                assert!(together.contains(&count), "seed {seed}: {count} of item {} together", def.index());
                drawn[def.index()] += 1;
            }
        }
        assert_eq!((drawn[3], drawn[4]), (0, 0), "nothing of a kind empty at the band or weighed nought");
        let (first, second) = (drawn[0] + drawn[1], drawn[2]);
        assert!((first * 10).abs_diff(second * 30) * 5 <= second * 30, "a kind three times the weight is drawn about three times as often: {drawn:?}");
        assert!(plan(&Kinds(vec![(10, table(vec![LootRow::new(item(0)).bands(1, 1)]))]), 5, 10, &mut atglyph::rand::rngs::StdRng::seed_from_u64(0)).is_empty());
    }
    /// A floor with room for one thing holds its scroll of enchanting and
    /// nothing of its budget; one with room for all of it holds the scroll,
    /// its things and its gold, each on a cell of its own, over a span of
    /// seeds.
    #[test]
    fn a_crowded_floor_drops_from_its_budget_and_never_its_scroll_of_enchanting_over_a_span_of_seeds() {
        let library = library();
        let (armory, registries) = (&library.armory, &library.registries);
        let kinds = Kinds::of(armory, registries);
        let (laid, gold) = (armory.defs.expect(LAID), armory.defs.expect(GOLD));
        let cells = |n: i32| (0..n).map(|x| Point::new(x, 0)).collect::<Vec<_>>();
        for seed in 0..40 {
            let mut rng = atglyph::rand::rngs::StdRng::seed_from_u64(seed);
            let crowded = laid_out(&kinds, laid, gold, 5, cells(1), &mut rng);
            assert_eq!(crowded, [(laid, 1, Point::new(0, 0))], "seed {seed}: the one cell holds the scroll");
            let roomy = laid_out(&kinds, laid, gold, 5, cells(100), &mut rng);
            assert_eq!(roomy[0].0, laid, "seed {seed}: the scroll first");
            assert!(roomy.len() as u32 > things_on(5), "seed {seed}: and its budget besides: {}", roomy.len());
            let mut at: Vec<Point> = roomy.iter().map(|(_, _, p)| *p).collect();
            at.sort_by_key(|p| p.x);
            at.dedup();
            assert_eq!(at.len(), roomy.len(), "seed {seed}: each on a cell of its own");
        }
    }

    /// About four potions of strength a run, read from the weights with no
    /// floor built and no seed drawn: on every floor, its [`things_on`],
    /// each a potion as often as [`KINDS`] weighs potions among the kinds
    /// open there and then a potion of strength as often as its row weighs
    /// among the floor's potions, as many together as its row's mean; and
    /// on every floor but the throne's, its [`CHESTS`](crate::dungeon::CHESTS),
    /// each holding the potions `assets/props.ron` stocks a chest with,
    /// drawn the same way one at a time. Enough that what heavy gear asks
    /// for can be come by, and few enough that it is still asked. What a
    /// piece lays or the dead drop comes on top, and is a share too small
    /// and too rare to count.
    #[test]
    fn about_four_potions_of_strength_are_laid_over_a_run_by_the_weights() {
        let library = library();
        let (armory, registries) = (&library.armory, &library.registries);
        let tag = |name: &str| registries.tags.expect(name);
        let strength = armory.defs.expect("potion of strength");
        let rows = |t: TagId, band: i32| armory.table.rows().iter().filter(move |r| r.weight > 0 && r.tags.contains(&t) && r.applies(band));
        // Potions of strength for one draw of a potion at `band`, its row's
        // mean group counted or each draw one alone.
        let per_potion = |band: i32, grouped: bool| -> f64 {
            let total: u32 = rows(tag("potion"), band).map(|r| r.weight).sum();
            let found = rows(tag("potion"), band).filter(|r| r.item == strength);
            let weighted: f64 = found.map(|r| f64::from(r.weight) * if grouped { f64::from(r.group.0 + r.group.1) / 2.0 } else { 1.0 }).sum();
            if total == 0 { 0.0 } else { weighted / f64::from(total) }
        };
        let chest = registries.props.get(registries.props.expect("chest"));
        let potions_a_chest: f64 = chest
            .container
            .iter()
            .flat_map(|c| &c.contents)
            .filter(|row| row.what == atglyph::rules::Stock::Tag(tag("potion")))
            .map(|row| f64::from(row.min + row.max) / 2.0)
            .sum();
        let chests = f64::from(crate::dungeon::CHESTS.0 + crate::dungeon::CHESTS.1) / 2.0;
        let mut expected = 0.0;
        for floor in 1..=FLOORS {
            let band = floor as i32;
            let open: u32 = KINDS.iter().filter(|(name, _)| rows(tag(name), band).next().is_some()).map(|(_, w)| w).sum();
            let potion = KINDS.iter().find(|(name, _)| *name == "potion").map_or(0, |(_, w)| *w);
            expected += f64::from(things_on(floor)) * f64::from(potion) / f64::from(open) * per_potion(band, true);
            if floor < FLOORS {
                expected += chests * potions_a_chest * per_potion(band, false);
            }
        }
        assert!((3.0..=5.0).contains(&expected), "{expected:.2} potions of strength a run");
    }
}
