//! What lies about on a floor the first time the adventurer arrives on it:
//! a budget of things drawn by kind, a scroll of enchanting besides, and
//! some gold.
//!
//! [`plan`] is the pure half, tested without an `App`: [`things_on`] a
//! floor's number of things, each a kind drawn by its weight in [`KINDS`]
//! among the kinds with anything to find at the floor's depth, then a thing
//! of that kind as `item_spawns.ron` weights it there, as many together as
//! its row says. [`lay`] is the Bevy over it: on the arrival that builds a
//! floor it plans the floor's things, adds the [`LAID`] scroll and a few
//! piles of gold, and puts each on a cell of its own, made by the
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

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;

use crate::armory::{Armory, ItemDef};
use crate::dungeon::floor_of;
use crate::stairs::Way;

/// The kinds a floor's things are drawn from, by the tag each kind's
/// members carry, and how often each is drawn against the others: potions
/// and scrolls most, weapons and armor a third as often, and staffs,
/// wands, rings and thrown things least.
pub const KINDS: [(&str, u32); 8] = [("potion", 30), ("scroll", 30), ("weapon", 10), ("armor", 10), ("staff", 6), ("wand", 5), ("ring", 5), ("thrown", 4)];

/// The things every floor holds before its depth adds any.
pub const BASE_THINGS: u32 = 6;

/// What every floor holds one of besides its things and its gold, by name:
/// the scroll of enchanting, so a run is sure of one a floor whatever the
/// draws give, and enchanting is what a run is built with.
pub const LAID: &str = "scroll of enchanting";

/// The gold a floor holds is piles of this, by name, each its own roll
/// times the floor.
pub const GOLD: &str = "gold coin";

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
type Standing = Or<(With<Prop>, With<Way>)>;

/// What laying a floor's things reads.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Ground<'w, 's> {
    armory: Res<'w, Armory>,
    registries: Res<'w, Registries>,
    map: Res<'w, WorldMap>,
    seed: Res<'w, Seed>,
    standing: Query<'w, 's, (&'static Position, &'static OnMap), Standing>,
}

/// Lays every floor's things on the arrival that builds it: its
/// [`things_on`] planned at its own band, the [`LAID`] scroll and
/// [`GOLD_PILES`] piles of [`GOLD`], each made by the [`Armory`] as a
/// thing found on that floor and put on a cell of its own.
///
/// A cell one could walk to with nothing standing on it: no prop, since a
/// thing under a chest is one nothing can pick up, and no way through,
/// since a thing lying on the stairs would be drawn over them. Draws from
/// `Seed::stream(b"undermere.finds", floor)`, a stream for each floor
/// alone, so what lies on floor two never depends on what floor one drew
/// or which was entered first; the plan, then the gold, then each cell and
/// what is made there, in that order.
pub fn lay(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, ground: Ground) {
    let Ground { armory, registries, map, seed, standing } = &ground;
    for ev in entered.read().filter(|ev| ev.first) {
        let (Some(floor), Some(place)) = (floor_of(ev.map), map.place(ev.map)) else { continue };
        let mut rng = seed.stream(b"undermere.finds", u64::from(floor));
        let band = floor as i32;
        let mut things = plan(&Kinds::of(armory, registries), band, things_on(floor), &mut rng);
        things.push((armory.defs.expect(LAID), 1));
        let gold = armory.defs.expect(GOLD);
        things.extend((0..rng.random_range(GOLD_PILES)).map(|_| (gold, 1)));
        let taken: Vec<Point> = standing.iter().filter(|(_, on)| on.0 == ev.map).map(|(at, _)| at.0).collect();
        let mut open: Vec<Point> = place.terrain.bounds().cells().filter(|p| map.is_walkable_on(ev.map, *p) && !taken.contains(p)).collect();
        let found = Provenance { found: Found::Scatter, band };
        for (def, count) in things {
            if open.is_empty() {
                break;
            }
            let at = open.swap_remove(rng.random_range(0..open.len()));
            for thing in armory.make(&mut commands, registries, def, count, found, &mut rng) {
                commands.entity(thing).insert((Position(at), OnMap(ev.map)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rl_engine::rl_core::RunSeed;

    use super::*;
    use crate::bestiary::Kind;
    use crate::content::Library;
    use crate::dungeon::{FLOORS, map_of};
    use crate::testing::{adventurer, arrive, headless, sturdy};

    fn library() -> Library {
        Library::load(&crate::content::effect_kinds(), &Moments::default())
    }

    /// Six things on floor one, and one more for every two floors down, to
    /// the throne's eleven.
    #[test]
    fn every_floor_holds_six_things_and_one_more_for_every_two_floors_down() {
        assert_eq!((1..=FLOORS).map(things_on).collect::<Vec<_>>(), [6, 7, 7, 8, 8, 9, 9, 10, 10, 11]);
    }

    /// What a floor's things are drawn as: a kind by its weight among the
    /// kinds with anything to find at the floor's depth, potions and
    /// scrolls three times as often as weapons or armor, then one of its
    /// members as the spawn table weights them there, each found at that
    /// depth and as many together as its row says, over a span of seeds.
    #[test]
    fn a_floors_things_are_drawn_by_kind_potions_and_scrolls_three_times_as_often_as_weapons_over_a_span_of_seeds() {
        let library = library();
        let (armory, registries) = (&library.armory, &library.registries);
        let kinds = Kinds::of(armory, registries);
        let tag = |name: &str| registries.tags.expect(name);
        let open: Vec<(&str, u32)> =
            KINDS.into_iter().filter(|(name, _)| armory.table.rows().iter().any(|r| r.tags.contains(&tag(name)) && r.applies(5))).collect();
        let total: u32 = open.iter().map(|(_, w)| w).sum();
        let mut drawn = vec![0u32; open.len()];
        for seed in 0..400 {
            let things = plan(&kinds, 5, 10, &mut rand::rngs::StdRng::seed_from_u64(seed));
            assert_eq!(things.len(), 10, "seed {seed}: as many as asked for");
            for (def, count) in things {
                let rows: Vec<_> = armory.table.rows().iter().filter(|r| r.item == def && r.applies(5)).collect();
                assert!(!rows.is_empty(), "seed {seed}: {} is found on floor five", armory.defs.get(def).name);
                assert!(rows.iter().any(|r| (r.group.0..=r.group.1).contains(&count)), "seed {seed}: {count} together");
                let at = open.iter().position(|(name, _)| armory.defs.get(def).tags.iter().any(|t| t.id() == tag(name)));
                drawn[at.expect("of a kind that is drawn")] += 1;
            }
        }
        // Within a fifth of its share of four thousand.
        for ((name, weight), got) in open.iter().zip(&drawn) {
            let share = 4000 * weight / total;
            assert!(got.abs_diff(share) * 5 <= share, "{name}: {got} drawn where {share} was its share, {drawn:?}");
        }
        assert!(open.iter().any(|(name, _)| *name == "potion") && open.iter().any(|(name, _)| *name == "weapon"), "{open:?}");
    }

    /// A world where nobody fights, so nothing a floor holds is a thing the
    /// dead left on it.
    fn at_peace(app: &mut App) {
        let rules = CombatRules::new(&app.world().resource::<Registries>().factions);
        app.insert_resource(rules);
    }

    /// The names of the things on `floor`, its gold or everything but its
    /// gold as `gold` says, lying on it or carried by whoever stands on it,
    /// and none in a chest: what the floor laid.
    fn on_floor(app: &mut App, floor: u32, gold: bool) -> Vec<String> {
        let map = map_of(floor);
        let world = app.world_mut();
        let armory = world.resource::<Armory>().clone();
        let mut things: Vec<Entity> =
            world.query_filtered::<(Entity, &OnMap), (With<Item>, With<Position>)>().iter(world).filter(|(_, on)| on.0 == map).map(|(e, _)| e).collect();
        let carried: Vec<Entity> = world
            .query_filtered::<(&Inventory, &OnMap), With<Kind>>()
            .iter(world)
            .filter(|(_, on)| on.0 == map)
            .flat_map(|(bag, _)| bag.items.clone())
            .collect();
        things.extend(carried);
        let gold_tag = world.resource::<Registries>().tags.expect("gold");
        let def = |e: Entity| world.get::<crate::armory::ItemKind>(e).map(|k| armory.defs.get(k.0));
        things.iter().filter_map(|e| def(*e)).filter(|d| d.tags.iter().any(|t| t.id() == gold_tag) == gold).map(|d| d.name.clone()).collect()
    }

    /// Every floor holds its things and a scroll of enchanting besides,
    /// and none, one or two piles of gold, the first time it is entered,
    /// over a span of seeds.
    #[test]
    fn every_floor_holds_a_scroll_of_enchanting_and_its_things_besides_gold_over_a_span_of_seeds() {
        for seed in 0..3 {
            let mut app = headless(RunSeed(seed));
            let me = adventurer(&mut app);
            sturdy(&mut app, me);
            at_peace(&mut app);
            for floor in 1..=FLOORS {
                arrive(&mut app, map_of(floor));
                let things = on_floor(&mut app, floor, false);
                assert_eq!(things.len() as u32, things_on(floor) + 1, "seed {seed}, floor {floor}: {things:?}");
                assert!(things.iter().any(|t| t == LAID), "seed {seed}, floor {floor}: no scroll of enchanting in {things:?}");
                let gold = on_floor(&mut app, floor, true);
                assert!(GOLD_PILES.contains(&(gold.len() as u32)), "seed {seed}, floor {floor}: {} piles of gold", gold.len());
            }
        }
    }

    /// About four potions of strength turn up over a whole run, lying about
    /// or in a chest, over a span of seeds: enough that what heavy gear
    /// asks for can be come by, and few enough that it is still asked.
    #[test]
    fn about_four_potions_of_strength_turn_up_over_a_run_over_a_span_of_seeds() {
        let runs = 24;
        let mut found = 0;
        for seed in 0..runs {
            let mut app = headless(RunSeed(seed));
            let me = adventurer(&mut app);
            sturdy(&mut app, me);
            for floor in 1..=FLOORS {
                arrive(&mut app, map_of(floor));
            }
            let world = app.world_mut();
            let strength = world.resource::<Armory>().defs.expect("potion of strength");
            let mut things = world.query::<(&crate::armory::ItemKind, Option<&Stack>)>();
            found += things.iter(world).filter(|(kind, _)| kind.0 == strength).map(|(_, stack)| stack.map_or(1, |s| s.count)).sum::<u32>();
        }
        assert!((3 * runs as u32..=5 * runs as u32).contains(&found), "{found} over {runs} runs");
    }
}
