//! The run: the Undermere built for it, the adventurer who walks out of
//! Mereholt with what they can carry, and the same run carried on from a
//! save.
//!
//! In the library rather than the binary, as Foundry's is, so a test
//! begins and continues a run the way the window does, through
//! [`UndermerePlugin`](crate::plugin::UndermerePlugin).

use bevy::ecs::system::RunSystemOnce;
use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_rules::ai::hearing::HearingStats;
use rl_engine::rl_rules::damage::{ApplyResistance, SubtractArmor};

use crate::armory::Armory;
use crate::dungeon::{FLOORS, TOWN, Undermere, floor_of, map_of};
use crate::light::Fixtures;
use crate::lore::{Faces, Lore};
use crate::tally::Counted;
use crate::townsfolk::{GivenNames, NameSets, Townsfolk, Townsperson, name_everyone};

/// The adventurer's health at the start of a run, and the most they ever
/// have but for a potion of life: sixty, since no experience and no levels
/// raise it, nothing mends it but what they find, and part 3's armor, which
/// takes at most half a blow, no longer turns the deep floors' blows away
/// whole. Brogue's forty until part 3's soak, which won one run in a
/// hundred and fifty on it, and seven in a hundred on fifty.
pub const HEALTH: i32 = 60;

/// The lantern, by its name in `items.ron`: what the adventurer walks out
/// with lit, and what dims as they descend.
pub const LANTERN: &str = "lantern";

/// What the adventurer walks out of Mereholt with, by name and count: a
/// sword in hand, leather on their back, a lit lantern, two potions of
/// healing, a waystone for the way home and fifty gold in the pack.
/// Everything that is worn is worn.
pub const KIT: [(&str, u32); 6] =
    [("sword", 1), ("leather armor", 1), (LANTERN, 1), ("potion of healing", 2), (crate::waystone::WAYSTONE, 1), ("gold coin", 50)];

/// How the adventurer is drawn.
pub const ADVENTURER: Color = Color::WHITE;

/// The floor a run starts on, when it is not Mereholt: `main.rs` reads it
/// from `UNDERMERE_START` for a photograph of a deeper floor. Absent, or
/// nought, the run starts in Mereholt, which is every real run.
#[derive(Resource, Debug, Clone, Copy)]
pub struct StartFloor(pub u32);

/// The adventurer, as a kind the save can name.
///
/// A marker rather than the engine's `Player`, because a save names each
/// kind by its own component and restores it through that component's own
/// account of it; the engine's marker is every game's.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Adventurer;

/// Everything a run stands on that is the same however it began: the
/// places and how they look and are lit, who fights whom and how a blow is
/// taken, the dark and the fire. Shared by [`start`] and [`resume`], so a
/// continued run is built on exactly the Undermere a fresh one is.
pub fn prepare(commands: &mut Commands, seed: &Seed, registries: &Registries) {
    let undermere = Undermere::new(seed.0);
    commands.insert_resource(undermere.appearance());
    commands.insert_resource(WorldMap::new(undermere.tiles().tables()));
    commands.insert_resource(crate::content::combat_rules(registries));
    // Whether a blow lands is accuracy against evasion, and how fast a
    // body moves its speed with the speed stat on it.
    commands.insert_resource(crate::content::hit_rules());
    commands.insert_resource(crate::content::turn_rules(registries));
    // Resistance first, then armor: the traits' resistances are what make
    // fire nothing to a salamander and poison nothing to the dead, and the
    // engine's default list is armor alone, which would leave them written
    // down and never played. Then Undermere's own floor on what armor
    // took: never more than half of the blow's own roll, so plate and
    // scrolls still matter on every floor and nothing is ever immune.
    // Last of all, Mereholt's own peace: a flag kept outside the pipeline,
    // since no stage before it can ask where a blow landed, dropping
    // whatever reaches it to nothing while the town is the place loaded.
    let peace = crate::damage::MereholtPeace::new();
    commands.insert_resource(peace.clone());
    commands.insert_resource(DamageStages(vec![
        Box::new(ApplyResistance),
        Box::new(SubtractArmor),
        Box::new(crate::damage::HalveArmor),
        Box::new(crate::damage::StopsHarmInMereholt(peace)),
    ]));
    commands.insert_resource(Lighting::dark());
    // Standing in fire scorches, a point a turn for three turns.
    commands.insert_resource(FireRules::new().inflicts(registries.statuses.expect("scorched"), 3));
    commands.insert_resource(Fixtures::new(&undermere, registries));
    // The pieces the places are stamped from, shared with the engine that
    // fills their slots, so a stamp's key names the same piece in both.
    commands.insert_resource(undermere.prefabs().clone());
    commands.insert_resource(PlaceRulesRes(Box::new(undermere)));
}

/// Spawns the adventurer with empty hands and an empty pack, nowhere yet:
/// the warp into Mereholt puts them somewhere, [`equip_kit`] gives a fresh
/// run's adventurer what they set out with, and a continued run's save
/// gives back what they carried.
pub fn spawn_adventurer(commands: &mut Commands, registries: &Registries) -> Entity {
    let side = registries.factions.expect("adventurer");
    let crush = registries.damage_kinds.expect("crush");
    commands
        .spawn((
            (Actor, Player, Adventurer, Blocks, Position(Point::ZERO), Viewshed::new(20), RevealsMap),
            // Bare hands: what is left when the sword is gone.
            (Health::full(HEALTH), Armor(0), Faction(side), MeleeAttack::new(crush, DiceRoll::new(1, 2))),
            (Name::new("you"), Glyph::new('@', ADVENTURER).on_layer(10)),
            // Noticeable, not sneaky: a subject a monster has to notice
            // rather than one it sees the moment it comes into view, so
            // whether the lantern is lit decides how soon it does.
            Stealth(crate::stats::OWN_STEALTH),
            // Ears, so the floor's noise reads on the vitals: the
            // adventurer hears what a keen-eared monster hears.
            Hearing(HearingStats { threshold: 0, memory: 6 }),
            (Inventory::default(), Equipped(Equipment::with_slot_count(registries.slots.len()))),
        ))
        .id()
}

/// Things put in the adventurer's pack as if they had been found, beside
/// the kit, for a photograph of the pack: `main.rs` reads them from
/// `UNDERMERE_PACK`, written `potion of extra healing*2, ring of light:-2`, each
/// a name from `items.ron` with a count after `*` and a level after `:`.
/// Absent, nothing, which is every real run.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct ExtraPack(pub Vec<(String, i32, u32)>);

impl ExtraPack {
    /// Reads `text`, comma by comma: a name, then `:level` and `*count` in
    /// either order, each optional, level nought and one of it otherwise.
    /// A count or a level that is not a number is read as none.
    pub fn parse(text: &str) -> Self {
        let mut things = Vec::new();
        for thing in text.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            let (mut name, mut level, mut count) = (thing, 0, 1);
            // Each mark peeled off the end, whichever order they came in.
            while let Some(at) = name.rfind([':', '*']) {
                let number = name[at + 1..].trim();
                match &name[at..at + 1] {
                    ":" => level = number.parse().unwrap_or(0),
                    _ => count = number.parse().unwrap_or(1),
                }
                name = &name[..at];
            }
            things.push((name.trim().to_string(), level, count));
        }
        Self(things)
    }
}

/// What a photograph of a deeper floor wants in sight on arriving there,
/// or of Mereholt the person it wants to stand beside: `main.rs` reads it
/// from `UNDERMERE_NEAR`, beside `UNDERMERE_START`. `camp` is a goblin
/// camp's fire, `captive` a captive's shackles, `lair` the floor's
/// mini-boss, `throne` the floor's boss, a [`role`] of `townsfolk.ron`
/// that person in Mereholt (`smith`, `priest`, `child`, ...), and anything
/// else a monster of that name. Absent, nothing, which is every real run.
///
/// The adventurer is warped along the way from where they arrived to it,
/// [`NEAR_STEPS`] short of it, so what they see is what a player walking
/// up to it would see, not a cell picked from above that might look at it
/// through a wall; beside a person, so one step into them opens the shop
/// or talks.
///
/// [`role`]: crate::townsfolk::Role::role
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct StartNear(pub String);

/// How many steps short of what [`StartNear`] names the adventurer is
/// stood: near enough that it is in the lantern's reach on every floor,
/// far enough that what stands round it is in sight too.
pub const NEAR_STEPS: usize = 4;

/// Stands the adventurer [`NEAR_STEPS`] short of what [`StartNear`] names,
/// or beside a person of Mereholt it names, once it stands on the place
/// they arrived on, and forgets the ask; does nothing while they are
/// somewhere it cannot be, a person below or anything else in Mereholt, or
/// while the place holds nothing by that name.
pub fn stand_near(world: &mut World) {
    let Some(near) = world.get_resource::<StartNear>().map(|n| n.0.clone()) else { return };
    let Some((me, at, here)) = world.query_filtered::<(Entity, &Position, &OnMap), With<Adventurer>>().iter(world).next().map(|(e, p, on)| (e, p.0, on.0))
    else {
        return;
    };
    let person = world.resource::<Townsfolk>().role(&near).is_some();
    if (here == TOWN) != person {
        return;
    }
    let target =
        if person { world.query::<(&Position, &Townsperson)>().iter(world).find(|(_, p)| p.0 == near).map(|(p, _)| p.0) } else { sought(world, &near, here) };
    let Some(target) = target else { return };
    let steps = if person { 1 } else { NEAR_STEPS };
    world.remove_resource::<StartNear>();
    let taken: Vec<Point> = world.query_filtered::<(&Position, &OnMap), With<Actor>>().iter(world).filter(|(_, on)| on.0 == here).map(|(p, _)| p.0).collect();
    let map = world.resource::<WorldMap>();
    let (Some(from), Some(goal)) = (map.to_local(at), map.to_local(target)) else { return };
    let view = map.opening_view();
    let mut flood = DijkstraMap::covering(&view);
    flood.build(&view, [goal], PathRules::default());
    let mut way = vec![from];
    while let Some(next) = flood.descend(*way.last().expect("never empty")).filter(|n| *n != goal) {
        way.push(next);
    }
    let stand = way.iter().rev().skip(steps.saturating_sub(1)).map(|p| map.to_world(*p)).find(|p| !taken.contains(p));
    if let Some(stand) = stand {
        info!("standing the adventurer at {stand:?}, short of the {near} at {target:?}");
        world.write_message(WarpRequest { actor: me, to: Destination::Place { map: here, arrive: Arrive::At(stand) } });
    }
}

/// Where on `here` what [`StartNear`] names as `near` stands, if it does.
fn sought(world: &mut World, near: &str, here: MapId) -> Option<Point> {
    let registries = world.resource::<Registries>().clone();
    let prop = |name: &str| registries.props.id(name);
    let props: Vec<(Point, PropKind)> =
        world.query::<(&Position, &OnMap, &PropKind)>().iter(world).filter(|(_, on, _)| on.0 == here).map(|(p, _, k)| (p.0, *k)).collect();
    let find_prop = |name: &str| prop(name).and_then(|id| props.iter().find(|(_, k)| k.0 == id).map(|(p, _)| *p));
    match near {
        "camp" => return find_prop("fire"),
        "captive" => return find_prop("shackles"),
        _ => {}
    }
    let living: Vec<(Point, crate::bestiary::Kind)> = world
        .query_filtered::<(&Position, &OnMap, &crate::bestiary::Kind), With<Actor>>()
        .iter(world)
        .filter(|(_, on, _)| on.0 == here)
        .map(|(p, _, k)| (p.0, *k))
        .collect();
    let bestiary = world.resource::<crate::bestiary::Bestiary>();
    living
        .into_iter()
        .find(|(_, kind)| {
            let def = bestiary.defs.get(kind.0);
            match near {
                "lair" => def.miniboss,
                "throne" => def.boss,
                name => def.name == name,
            }
        })
        .map(|(p, _)| p)
}

/// Gives `adventurer` the [`KIT`], made by the `armory`: every piece in the
/// pack, and each that is worn in its slot. And `extra`, as if found, into
/// the pack alone.
///
/// Put on straight into the slots rather than asked for with `Equip`,
/// which would spend the adventurer's first turns dressing: a run begins
/// with them dressed. Every piece is [`Counted`] whole, since what the
/// adventurer set out with is nothing they found, however often they put
/// it down and take it up again. Whatever they set out with they know,
/// kind and level: [`start`] makes its kinds known.
pub fn equip_kit(commands: &mut Commands, adventurer: Entity, armory: &Armory, registries: &Registries, extra: &ExtraPack) {
    let mut pack = Vec::new();
    let mut worn = Equipment::with_slot_count(registries.slots.len());
    for (name, count) in KIT {
        let id = armory.defs.expect(name);
        let each = if armory.defs.get(id).stack { count } else { 1 };
        for item in crate::armory::spawn_items(commands, armory, id, count, registries) {
            commands.entity(item).insert(Counted(each));
            if let Some(shape) = crate::armory::shape_of(armory.defs.get(id), registries) {
                // Each piece of the kit has a slot of its own, so none
                // displaces another.
                worn.equip(item, &shape).expect("the kit fits the adventurer's slots");
            }
            pack.push(item);
        }
    }
    for (name, level, count) in &extra.0 {
        let Some(id) = armory.defs.id(name) else {
            warn!("UNDERMERE_PACK names {name:?}, which items.ron has no thing called");
            continue;
        };
        match armory.defs.get(id).stack {
            true => pack.extend(crate::armory::spawn_items(commands, armory, id, *count, registries)),
            false => pack.extend((0..*count).map(|_| crate::armory::spawn_found(commands, armory, id, *level, registries))),
        }
    }
    commands.entity(adventurer).insert((Inventory { items: pack }, Equipped(worn)));
}

// ANCHOR: start
/// Builds the Undermere, spawns the adventurer with their kit and warps
/// them into Mereholt, or onto [`StartFloor`]'s floor. Runs in [`NewRun`],
/// for every run but one the title asked to continue with `ContinueRun`,
/// which is [`resume`]'s.
///
/// Asks nothing about a title: while one is up the engine runs no
/// `NewRun` at all, and runs it once the player picks.
pub fn start(
    mut commands: Commands,
    seed: Res<Seed>,
    content: Content,
    first: Option<Res<StartFloor>>,
    continuing: Option<Res<ContinueRun>>,
    mut begin: Begin,
) {
    if continuing.is_some() {
        return;
    }
    prepare(&mut commands, &seed, &content.registries);
    // The faces this run's things wear, and the kinds of what the
    // adventurer sets out with, which they know.
    let mut lore = Lore::draw(&content.faces, &content.armory.defs, &seed);
    for (name, _) in KIT {
        lore.learn(name);
    }
    commands.insert_resource(lore);
    // Who Mereholt's people are this run, each named apart from everybody
    // else the run names.
    let mut given = GivenNames::default();
    commands.insert_resource(name_everyone(&content.townsfolk, &content.names, &seed, &mut given));
    commands.insert_resource(given);
    let adventurer = spawn_adventurer(&mut commands, &content.registries);
    let extra = content.extra.as_deref().cloned().unwrap_or_default();
    equip_kit(&mut commands, adventurer, &content.armory, &content.registries, &extra);
    begin.log.notice(format!("Seed {}. Ten floors under the lake, something waits on a drowned throne.", seed.0.0), 0);
    let place = match first.map_or(0, |f| f.0.min(FLOORS)) {
        0 => TOWN,
        floor => map_of(floor),
    };
    begin.warps.write(WarpRequest::into_place(adventurer, place));
    begin.next.set(EngineState::Playing);
}
// ANCHOR_END: start

/// Continues the saved run, when the title's Continue asked for it with
/// `ContinueRun`: the Undermere built again from the saved seed, the run
/// restored into it, and every place it had built lit again. Takes the
/// marker away, so the run after this one, a restart's, begins afresh.
///
/// After [`start`] in `NewRun`, so the resources a restore writes into
/// exist. A save that cannot be read is logged and a fresh run begins
/// instead: the title offers Continue only for a save it could read, so
/// this is a save that went bad in between.
pub fn resume(world: &mut World) {
    if world.remove_resource::<ContinueRun>().is_none() {
        return;
    }
    let saved = match rl_engine::rl_save::load_run(world) {
        Ok(Some(saved)) => saved,
        Ok(None) | Err(_) => {
            error!("there is no save that can be continued; beginning a new run");
            begin_afresh(world);
            return;
        }
    };
    let seed = Seed(saved.engine.seed);
    world.insert_resource(seed);
    let registries = world.resource::<Registries>().clone();
    let mut queue = CommandQueue::default();
    prepare(&mut Commands::new(&mut queue, world), &seed, &registries);
    queue.apply(world);
    // A save that restores only in part is worse than none: the run it
    // leaves is neither the one saved nor a fresh one. So a failure tears
    // down what it got to, and a new run begins, as for a save that could
    // not be read at all.
    let restored = saved.restore(world);
    let adventurer = world.query_filtered::<Entity, With<Adventurer>>().single(world).ok();
    let (Ok(()), Some(adventurer)) = (restored, adventurer) else {
        error!("the save could not be restored; beginning a new run");
        rl_engine::rl_bevy::plugin::clear_run(world);
        begin_afresh(world);
        return;
    };
    // The fixtures' light is the places' own and not the run's, lit again
    // on every place built, as each first arrival lit it.
    let mut queue = CommandQueue::default();
    {
        let (map, fixtures) = (world.resource::<WorldMap>(), world.resource::<Fixtures>());
        let mut commands = Commands::new(&mut queue, world);
        for place in std::iter::once(TOWN).chain((1..=FLOORS).map(map_of)) {
            crate::light::light_place(&mut commands, map, place, fixtures);
        }
    }
    queue.apply(world);
    let line = match world.get::<OnMap>(adventurer).and_then(|on| floor_of(on.0)) {
        Some(floor) => format!("Continuing on floor {floor}."),
        None => "Continuing in Mereholt.".to_string(),
    };
    world.resource_mut::<MessageLog>().notice(line, saved.turn());
    world.resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
}

/// Begins a fresh run where a continued one could not be.
fn begin_afresh(world: &mut World) {
    if let Err(e) = world.run_system_once(start) {
        error!("the new run could not begin: {e}");
    }
}

/// The content a run is made from, loaded once before any run begins, and
/// what a photograph asked for in the pack besides.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Content<'w> {
    registries: Res<'w, Registries>,
    armory: Res<'w, Armory>,
    faces: Res<'w, Faces>,
    townsfolk: Res<'w, Townsfolk>,
    names: Res<'w, NameSets>,
    extra: Option<Res<'w, ExtraPack>>,
}

/// What starting a run writes beyond the world itself: the log's first
/// line, the warp into Mereholt, and the state that begins play.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Begin<'w> {
    warps: MessageWriter<'w, WarpRequest>,
    next: ResMut<'w, NextState<EngineState>>,
    log: ResMut<'w, MessageLog>,
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_core::RunSeed;

    use super::*;
    use crate::testing::{adventurer, arrive, at, headless, here, pack, settle, worn};

    /// A new run, over a span of seeds: the one adventurer in Mereholt,
    /// where its piece marks the arrival, at full health, the sword in
    /// hand, the leather on and the lantern lit in its place, and two
    /// potions of healing and fifty gold in the pack.
    #[test]
    fn a_new_run_puts_the_adventurer_in_mereholt_with_their_kit() {
        for s in 0..4 {
            let mut app = headless(RunSeed(s));
            let me = adventurer(&mut app);
            let entry = app.world().resource::<WorldMap>().place(TOWN).expect("Mereholt was built").entry;
            assert_eq!((here(&app), at(&app, me)), (TOWN, entry), "seed {s}: in Mereholt, where it marks the arrival");
            assert_eq!(app.world_mut().query_filtered::<(), With<Player>>().iter(app.world()).count(), 1, "seed {s}: one adventurer");
            assert_eq!(app.world().get::<Health>(me).map(|h| (h.current, h.max)), Some((HEALTH, HEALTH)), "seed {s}");
            let named = |names: [Option<&str>; 7]| names.map(|n| n.map(String::from)).to_vec();
            assert_eq!(
                worn(&app, me),
                named([Some("sword"), None, None, Some("leather armor"), None, None, Some(LANTERN)]),
                "seed {s}: hands, head, body, fingers and light"
            );
            let mut carried = pack(&app, me);
            carried.sort();
            let kit: Vec<(String, u32)> =
                [("gold coin", 50), (LANTERN, 1), ("leather armor", 1), ("potion of healing", 2), ("sword", 1), (crate::waystone::WAYSTONE, 1)]
                    .map(|(n, c)| (n.to_string(), c))
                    .to_vec();
            assert_eq!(carried, kit, "seed {s}: everything worn is carried, and the potions are one stack of two");
            let bag = &app.world().get::<Inventory>(me).expect("a pack").items;
            let lantern = bag.iter().copied().find(|i| app.world().get::<Name>(*i).is_some_and(|n| n.as_str() == LANTERN));
            assert!(lantern.is_some_and(|l| app.world().get::<LightSource>(l).is_some()), "seed {s}: and the lantern is lit");
        }
    }

    /// A blow meets resistance before armor: fire does nothing to a
    /// salamander, whatever its plate, and cold, which goes through plate,
    /// bites it half as hard again.
    #[test]
    fn a_resistance_is_taken_before_armor_so_fire_does_nothing_to_a_salamander() {
        use crate::testing::{clear_monsters, monster, settle};
        let mut app = headless(RunSeed(4));
        let me = arrive(&mut app, map_of(1));
        clear_monsters(&mut app);
        let beside = at(&app, me).offset(0, 1);
        let salamander = monster(&mut app, "salamander", beside);
        let registries = app.world().resource::<Registries>().clone();
        let health = |app: &App| app.world().get::<Health>(salamander).map(|h| h.current);
        let full = health(&app);
        app.world_mut().write_message(DamageEvent::new(salamander, rl_engine::rl_rules::Hit::by(me, registries.damage_kinds.expect("fire"), 10)));
        settle(&mut app);
        assert_eq!(health(&app), full, "fire-blooded: fire does nothing");
        app.world_mut().write_message(DamageEvent::new(salamander, rl_engine::rl_rules::Hit::by(me, registries.damage_kinds.expect("cold"), 10)));
        settle(&mut app);
        assert_eq!(health(&app), full.map(|h| h - 15), "and cold bites half as hard again");
    }

    /// What a photograph asks for in the pack is read a thing to a comma,
    /// a level after a colon and a count after a star in either order, and
    /// it goes into the pack as found: a ring not known to be any level, a
    /// stack of potions counted.
    #[test]
    fn what_a_photograph_asks_for_goes_into_the_pack_as_found() {
        let asked = ExtraPack::parse(" ring of light:-2, potion of extra healing*2 ,chain mail*2:1, scroll of identify ");
        let want = [("ring of light", -2, 1), ("potion of extra healing", 0, 2), ("chain mail", 1, 2), ("scroll of identify", 0, 1)];
        assert_eq!(asked, ExtraPack(want.map(|(n, l, c)| (n.to_string(), l, c)).to_vec()));
        let mut app = headless(RunSeed(1));
        app.insert_resource(ExtraPack::parse("ring of light:-2, potion of extra healing*2"));
        let me = adventurer(&mut app);
        let bag = app.world().get::<Inventory>(me).expect("a pack").items.clone();
        let ring = bag[bag.len() - 2];
        assert_eq!(app.world().get::<Enchant>(ring).map(|e| e.level), Some(-2));
        assert!(app.world().get::<crate::lore::Unfamiliar>(ring).is_some(), "found, not set out with");
        assert_eq!(app.world().get::<Stack>(bag[bag.len() - 1]).map(|s| s.count), Some(2));
    }

    /// A run started on a floor begins on that floor, at its way in, for
    /// every floor there is: what a photograph of a deeper floor starts
    /// from.
    #[test]
    fn a_run_started_on_a_floor_begins_on_that_floor() {
        for floor in 1..=FLOORS {
            let mut app = headless(RunSeed(7));
            app.insert_resource(StartFloor(floor));
            let me = adventurer(&mut app);
            let entry = app.world().resource::<WorldMap>().place(map_of(floor)).map(|p| p.entry);
            assert_eq!((here(&app), Some(at(&app, me))), (map_of(floor), entry), "floor {floor}: on it, at its way in");
        }
    }

    /// A photograph's ask to start near the throne's boss stands the
    /// adventurer on the way to it, a few steps short, for every seed
    /// tried and so for each of the three bosses a seed may draw.
    #[test]
    fn a_run_started_near_the_throne_stands_a_few_steps_short_of_its_boss() {
        use crate::testing::settle;
        for s in 0..6 {
            let mut app = headless(RunSeed(s));
            app.insert_resource(StartFloor(FLOORS)).insert_resource(StartNear("throne".to_string()));
            let me = adventurer(&mut app);
            settle(&mut app);
            let boss = {
                let world = app.world_mut();
                let bosses: Vec<(Point, crate::bestiary::Kind)> =
                    world.query::<(&Position, &crate::bestiary::Kind)>().iter(world).map(|(p, k)| (p.0, *k)).collect();
                let bestiary = app.world().resource::<crate::bestiary::Bestiary>();
                bosses.into_iter().find(|(_, k)| bestiary.defs.get(k.0).boss).map(|(p, _)| p).expect("a boss on the throne")
            };
            let far = geometry::chebyshev(at(&app, me), boss);
            assert!((1..=NEAR_STEPS as i32 + 1).contains(&far), "seed {s}: stood {far} from the boss");
            assert!(app.world().get_resource::<StartNear>().is_none(), "seed {s}: the ask is forgotten once answered");
        }
    }

    /// A photograph's ask to start by one of Mereholt's people who keeps
    /// a post stands the adventurer beside them, one step from their shop
    /// or their talk, for every one of them and every seed tried.
    #[test]
    fn a_run_started_by_a_person_who_keeps_a_post_stands_beside_them_in_mereholt() {
        use crate::testing::settle;
        use crate::townsfolk::Moves;
        let townsfolk = crate::content::Library::load(&crate::content::effect_kinds(), &Moments::default()).townsfolk;
        for s in [0, 1, 7] {
            for role in townsfolk.roles().filter(|r| r.moves == Moves::Keeps) {
                let mut app = headless(RunSeed(s));
                app.insert_resource(StartNear(role.role.clone()));
                let me = adventurer(&mut app);
                settle(&mut app);
                let person = {
                    let world = app.world_mut();
                    let mut q = world.query::<(&Position, &Townsperson)>();
                    q.iter(world).find(|(_, p)| p.0 == role.role).map(|(p, _)| p.0).expect("the person stands")
                };
                assert_eq!(geometry::chebyshev(at(&app, me), person), 1, "seed {s}: beside the {} at {person:?}, stood at {:?}", role.role, at(&app, me));
                assert!(app.world().get_resource::<StartNear>().is_none(), "seed {s}: the ask is forgotten once answered");
            }
        }
    }

    /// A run begun after another in the same launch has been nowhere and
    /// done nothing: the deepest floor the last one reached and the kill
    /// it made are not the new one's.
    #[test]
    fn a_run_begun_after_another_has_reached_no_floor_and_killed_nothing() {
        use crate::tally::Tally;
        use crate::testing::{clear_monsters, kill, lay_floor, monster};
        let mut app = headless(RunSeed(2));
        let me = arrive(&mut app, map_of(3));
        clear_monsters(&mut app);
        let beside = at(&app, me).offset(1, 0);
        lay_floor(&mut app, &[beside]);
        let rat = monster(&mut app, "rat", beside);
        kill(&mut app, rat, Some(me));
        let tally = app.world().resource::<Tally>();
        assert_eq!((tally.deepest, tally.kills), (3, 1), "the run went down to floor three and killed a rat");
        app.world_mut().write_message(Restart::fresh());
        settle(&mut app);
        settle(&mut app);
        assert_eq!(*app.world().resource::<Tally>(), Tally::default(), "the next run has been nowhere and done nothing");
        let me = adventurer(&mut app);
        assert_eq!((here(&app), app.world().get::<OnMap>(me).map(|m| m.0)), (TOWN, Some(TOWN)), "and sets out from Mereholt");
    }
}
