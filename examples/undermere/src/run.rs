//! The run: the Undermere built for it, the adventurer who walks out of
//! Mereholt with what they can carry, and the same run carried on from a
//! save.
//!
//! In the library rather than the binary, as Foundry's is, so a test
//! begins and continues a run the way the window does, through
//! [`UndermerePlugin`](crate::plugin::UndermerePlugin).

use atglyph::prelude::*;
use atglyph::rules::ai::hearing::HearingStats;
use atglyph::rules::damage::{ApplyResistance, SubtractArmor};
use bevy::ecs::system::RunSystemOnce;
use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use crate::armory::Armory;
use crate::dungeon::{FLOORS, TOWN, Undermere, floor_of, map_of};
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
    // Mereholt's own peace is the town's, as its build marks it.
    commands.insert_resource(DamageStages(vec![Box::new(ApplyResistance), Box::new(SubtractArmor), Box::new(crate::damage::HalveArmor)]));
    commands.insert_resource(Lighting::dark());
    // Standing in fire scorches, a point a turn for three turns.
    commands.insert_resource(FireRules::new().inflicts(registries.statuses.expect("scorched"), 3));
    // Every floor's drawn biome, read off `undermere` once: the engine
    // boxes it behind `PlaceRules` below, so nothing after this can ask
    // it directly.
    commands.insert_resource(crate::light::BiomeDraws::new(&undermere));
    // The pieces the places are stamped from, shared with the engine that
    // fills their slots, so a stamp's key names the same piece in both.
    commands.insert_resource(undermere.prefabs().clone());
    // The risk-and-reward pieces paint a few tiles by name in play, which
    // only `undermere.tiles()` can still answer once it is boxed away
    // below.
    commands.insert_resource(crate::vaults::VaultTiles::new(undermere.tiles()));
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
/// `UNDERMERE_PACK`, written `potion of extra healing*2, ring of light:-2`,
/// each a name from `items.ron` with a count after `*` and a level after
/// `:`. Absent, nothing, which is every real run.
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

/// What a photograph of a deeper floor wants in sight on arriving there, or
/// of Mereholt the person it wants to stand beside: `main.rs` reads it from
/// `UNDERMERE_NEAR`, beside `UNDERMERE_START`. `camp` is a goblin camp's
/// fire, `captive` a captive's shackles, `lair` the floor's mini-boss,
/// `throne` the floor's boss, `secret` the first thing the floor hides in
/// its walls, a [`role`] of `townsfolk.ron` that person in Mereholt
/// (`smith`, `priest`, `child`, ...),
/// [`candidates`](crate::mercenaries::NEAR_CANDIDATES) the swords for hire
/// in its tavern, and anything else a prop of that name, hidden or not (`dart
/// plate`, `lever`), a tile of that name (`bridge`, `vault door`), or a
/// monster of that name. `:N` after the name stands them `N` steps short
/// of it rather than [`NEAR_STEPS`], `trapdoor:1` a step from it. Absent,
/// nothing, which is every real run.
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
    let Some(asked) = world.get_resource::<StartNear>().map(|n| n.0.clone()) else { return };
    let (near, short) = match asked.rsplit_once(':').map(|(name, n)| (name, n.parse::<usize>())) {
        Some((name, Ok(n))) => (name.to_string(), Some(n.max(1))),
        _ => (asked.clone(), None),
    };
    let Some((me, at, here)) = world.query_filtered::<(Entity, &Position, &OnMap), With<Adventurer>>().iter(world).next().map(|(e, p, on)| (e, p.0, on.0))
    else {
        return;
    };
    let candidates = near == crate::mercenaries::NEAR_CANDIDATES;
    let person = candidates || world.resource::<Townsfolk>().role(&near).is_some();
    if (here == TOWN) != person {
        return;
    }
    let target = if candidates {
        world.query_filtered::<&Position, With<crate::mercenaries::Candidate>>().iter(world).map(|p| p.0).min()
    } else if person {
        world.query::<(&Position, &Townsperson)>().iter(world).find(|(_, p)| p.0 == near).map(|(p, _)| p.0)
    } else {
        sought(world, &near, here)
    };
    let Some(target) = target else { return };
    let steps = short.unwrap_or(if person { 1 } else { NEAR_STEPS });
    world.remove_resource::<StartNear>();
    let taken: Vec<Point> = world.query_filtered::<(&Position, &OnMap), With<Actor>>().iter(world).filter(|(_, on)| on.0 == here).map(|(p, _)| p.0).collect();
    let map = world.resource::<WorldMap>();
    // What stands in a wall, a secret, is walked up to by the cells
    // beside it, and the way ends on the one reached; anything else is
    // walked up to and not onto.
    let open = |p: Point| map.is_walkable(p) || map.opens(p).is_some();
    let walled = !open(target);
    let goals: Vec<Point> = if walled {
        Direction::ALL.iter().map(|d| target + d.offset()).filter(|p| open(*p)).filter_map(|p| map.to_local(p)).collect()
    } else {
        map.to_local(target).into_iter().collect()
    };
    let Some(from) = map.to_local(at) else { return };
    let view = map.opening_view();
    let mut flood = DijkstraMap::covering(&view);
    flood.build(&view, goals.iter().copied(), PathRules::default());
    let mut way = vec![from];
    while !goals.contains(way.last().expect("never empty")) {
        let Some(next) = flood.descend(*way.last().expect("never empty")) else { break };
        way.push(next);
    }
    if !walled && way.len() > 1 {
        way.pop();
    }
    let stand = way.iter().rev().skip(steps.saturating_sub(1)).map(|p| map.to_world(*p)).find(|p| !taken.contains(p));
    if let Some(stand) = stand {
        info!("standing the adventurer at {stand:?}, short of the {near} at {target:?}");
        world.write_message(WarpRequest { actor: me, to: Destination::Place { map: here, arrive: Arrive::At(stand) }, after: None });
    }
}

/// How many hired hands a photograph wants following the adventurer from
/// the first frame they stand anywhere, at most
/// [`PARTY`](crate::mercenaries::PARTY): `main.rs` reads it from
/// `UNDERMERE_PARTY`, beside `UNDERMERE_START` and `UNDERMERE_NEAR`, so a
/// picture of a party on a deep floor, in a fight or at the end of a run
/// needs no walk to the tavern and back. It waits on the stand
/// `UNDERMERE_NEAR` asks for, so one asking for what the floor does not
/// hold, a lair on a floor with none, gets no party either. Absent,
/// nothing, which is every real run.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartParty(pub usize);

/// Hires [`StartParty`]'s hands on the cells beside the adventurer, once
/// they stand somewhere and [`StartNear`] has stood them where it asked,
/// and forgets the ask. Each is drawn as the tavern draws a candidate, a
/// kit at the deepest floor reached and a name the run has not given,
/// made a candidate and hired the one way a player's hire is, so what the
/// photograph shows is what playing would.
pub fn hire_party(world: &mut World) {
    use crate::mercenaries::{Makers, Mercenaries, PARTY, hire, spawn_candidate};
    let Some(StartParty(wanted)) = world.get_resource::<StartParty>().copied() else { return };
    let Some((me, at, here)) = world.query_filtered::<(Entity, &Position, &OnMap), With<Adventurer>>().iter(world).next().map(|(e, p, on)| (e, p.0, on.0))
    else {
        return;
    };
    world.remove_resource::<StartParty>();
    let taken: Vec<Point> = world.query_filtered::<(&Position, &OnMap), With<Blocks>>().iter(world).filter(|(_, on)| on.0 == here).map(|(p, _)| p.0).collect();
    let map = world.resource::<WorldMap>();
    let cells: Vec<Point> =
        (0..8).map(|i| at + Direction::from_index(i).offset()).filter(|p| map.is_walkable_on(here, *p) && !taken.contains(p)).take(wanted.min(PARTY)).collect();
    let mut given = world.resource::<GivenNames>().clone();
    let mut rng = world.resource::<Seed>().rng(crate::seeds::PARTY, 0);
    let mut queue = CommandQueue::default();
    let made: Vec<Entity> = {
        let mut commands = Commands::new(&mut queue, world);
        let (mercenaries, armory, names) = (world.resource::<Mercenaries>(), world.resource::<Armory>(), world.resource::<NameSets>());
        let deepest = world.resource::<crate::tally::Tally>().deepest;
        let makers = Makers::of(world);
        cells
            .into_iter()
            .map(|cell| {
                let kit = mercenaries.kit(deepest, armory, &mut rng);
                let name = names.mercenary().draw_distinct(&mut rng, |n| given.has(n)).expect("the mercenaries' names never run dry");
                given.give(&name);
                let candidate = spawn_candidate(&mut commands, &makers, &name, &kit, mercenaries.fee(&kit, armory), cell);
                commands.entity(candidate).insert(OnMap(here));
                candidate
            })
            .collect()
    };
    queue.apply(world);
    world.insert_resource(given);
    for candidate in made {
        hire(world, candidate, me);
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
        "secret" => return world.resource::<WorldMap>().secrets(here).first().map(|s| s.at),
        _ => {}
    }
    if let Some(at) = find_prop(near) {
        return Some(at);
    }
    let map = world.resource::<WorldMap>();
    if let Some(tile) = map.tables().names.iter().position(|n| n == near) {
        let found = map.window_tiles().cells().find(|p| map.tile(*p).is_some_and(|t| t.index() == tile));
        if found.is_some() {
            return found;
        }
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
        for item in crate::armory::spawn_items(commands, armory, id, count) {
            commands.entity(item).insert(Counted(each));
            if let Some(shape) = &armory.parts(id).wear {
                // Each piece of the kit has a slot of its own, so none
                // displaces another.
                worn.equip(item, shape).expect("the kit fits the adventurer's slots");
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
            true => pack.extend(crate::armory::spawn_items(commands, armory, id, *count)),
            false => pack.extend((0..*count).map(|_| crate::armory::spawn_found(commands, armory, id, *level))),
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
    begin.log.notice(format!("Seed {}. Ten floors under the lake, something waits on a drowned throne.", seed.0.0));
    let place = match first.map_or(0, |f| f.0.min(FLOORS)) {
        0 => TOWN,
        floor => map_of(floor),
    };
    begin.warps.write(WarpRequest::into_place(adventurer, place));
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
    let saved = match atglyph::save::load_run(world) {
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
        atglyph::ecs::plugin::clear_run(world);
        begin_afresh(world);
        return;
    };
    let line = match world.get::<OnMap>(adventurer).and_then(|on| floor_of(on.0)) {
        Some(floor) => format!("Continuing on floor {floor}."),
        None => "Continuing in Mereholt.".to_string(),
    };
    world.resource_mut::<MessageLog>().push_at(line, Tones::NOTICE, saved.turn());
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
/// line and the warp into Mereholt; play begins on its own once the start
/// has built the world.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Begin<'w> {
    warps: MessageWriter<'w, WarpRequest>,
    log: Log<'w>,
}

#[cfg(test)]
mod tests {
    use atglyph::primitives::RunSeed;

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
        app.world_mut().write_message(DamageEvent::new(salamander, atglyph::rules::Hit::by(me, registries.damage_kinds.expect("fire"), 10)));
        settle(&mut app);
        assert_eq!(health(&app), full, "fire-blooded: fire does nothing");
        app.world_mut().write_message(DamageEvent::new(salamander, atglyph::rules::Hit::by(me, registries.damage_kinds.expect("cold"), 10)));
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
        assert_eq!(app.world().get::<ItemLevel>(ring).map(|l| l.0), Some(-2));
        assert!(app.world().get::<crate::lore::Unfamiliar>(ring).is_some(), "found, not set out with");
        assert_eq!(app.world().get::<Stack>(bag[bag.len() - 1]).map(|s| s.count), Some(2));
    }

    /// A run started on a floor begins on that floor, at its way in:
    /// what a photograph of a deeper floor starts from. The ask is a floor
    /// number read the same way for every floor, so the first and the
    /// deepest are the ends of what it can be given.
    #[test]
    fn a_run_started_on_a_floor_begins_on_that_floor() {
        for floor in [1, FLOORS] {
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

    /// A photograph's ask names a secret, a prop or a tile as well as a
    /// monster, and `:N` stands the adventurer that many steps short:
    /// one step from the floor's first secret, and a few from its first
    /// trap, hidden as it is.
    #[test]
    fn a_run_started_near_a_secret_or_a_trap_stands_the_steps_it_asks_short_of_it() {
        use crate::testing::settle;
        let mut app = headless(RunSeed(1));
        app.insert_resource(StartFloor(3)).insert_resource(StartNear("secret:1".to_string()));
        let me = adventurer(&mut app);
        settle(&mut app);
        let secret = app.world().resource::<WorldMap>().secrets(map_of(3)).first().map(|s| s.at).expect("floor three hides something");
        assert_eq!(geometry::chebyshev(at(&app, me), secret), 1, "a step from the secret");
        let mut app = headless(RunSeed(2));
        app.insert_resource(StartFloor(3)).insert_resource(StartNear("dart plate".to_string()));
        let me = adventurer(&mut app);
        settle(&mut app);
        let plate = app.world().resource::<Registries>().props.expect("dart plate");
        let world = app.world_mut();
        let plates: Vec<Point> = world.query::<(&Position, &PropKind)>().iter(world).filter(|(_, k)| k.0 == plate).map(|(p, _)| p.0).collect();
        let far = plates.iter().map(|p| geometry::chebyshev(at(&app, me), *p)).min().expect("a dart plate on floor three");
        assert!((1..=NEAR_STEPS as i32 + 1).contains(&far), "stood {far} from the plate");
    }

    /// A photograph's ask for a party hires that many, never more than the
    /// party holds, beside the adventurer where the run starts and after
    /// any stand it asked for: each a mercenary of its own name, following
    /// them, on the floor they stand on. The run's own draws are not
    /// moved: the floor's monsters stand where they would without it.
    #[test]
    fn a_run_started_with_a_party_has_it_hired_and_following_beside_the_adventurer() {
        use crate::mercenaries::{Mercenary, Order, PARTY};
        use crate::testing::settle;
        let monsters = |app: &mut App| {
            let world = app.world_mut();
            let mut all: Vec<Point> = world.query_filtered::<&Position, (With<crate::bestiary::Kind>, Without<Mercenary>)>().iter(world).map(|p| p.0).collect();
            all.sort();
            all
        };
        for (s, (floor, near)) in [(1, None), (FLOORS, Some("throne"))].into_iter().enumerate() {
            let begin = |party: Option<usize>| {
                let mut app = headless(RunSeed(s as u64));
                app.insert_resource(StartFloor(floor));
                if let Some(near) = near {
                    app.insert_resource(StartNear(near.to_string()));
                }
                if let Some(n) = party {
                    app.insert_resource(StartParty(n));
                }
                let me = adventurer(&mut app);
                settle(&mut app);
                (app, me)
            };
            let (mut plain, _) = begin(None);
            let (mut app, me) = begin(Some(PARTY + 1));
            let hired: Vec<(Entity, Mercenary)> = {
                let world = app.world_mut();
                world.query::<(Entity, &Mercenary)>().iter(world).map(|(e, m)| (e, m.clone())).collect()
            };
            assert_eq!(hired.len(), PARTY, "floor {floor}: the most the party holds");
            assert_ne!(hired[0].1.name, hired[1].1.name, "floor {floor}: each its own name");
            for (who, mercenary) in &hired {
                assert_eq!(mercenary.order, Order::Follow, "floor {floor}");
                assert_eq!(app.world().get::<Leader>(*who).map(|l| l.0), Some(me), "floor {floor}: following the adventurer");
                assert_eq!(app.world().get::<OnMap>(*who).map(|m| m.0), Some(map_of(floor)), "floor {floor}: on their floor");
                assert!(geometry::chebyshev(at(&app, *who), at(&app, me)) <= 2, "floor {floor}: beside them");
            }
            assert!(app.world().get_resource::<StartParty>().is_none(), "floor {floor}: the ask is forgotten once answered");
            assert_eq!(monsters(&mut app), monsters(&mut plain), "floor {floor}: the run's own draws unmoved");
        }
    }

    /// A photograph's ask to start by one of Mereholt's people who keeps
    /// a post stands the adventurer beside them, one step from their shop
    /// or their talk, for every one of them. Mereholt is drawn by hand and
    /// is the same on every seed, so one run is asked for each of them in
    /// turn, the stand answered by `stand_near` itself.
    #[test]
    fn a_run_started_by_a_person_who_keeps_a_post_stands_beside_them_in_mereholt() {
        use crate::testing::settle;
        use crate::townsfolk::Moves;
        let townsfolk = crate::content::Library::load(&crate::content::effect_kinds(), &TriggerKinds::default()).townsfolk;
        let mut app = headless(RunSeed(0));
        let me = adventurer(&mut app);
        settle(&mut app);
        let mut asked = 0;
        for role in townsfolk.roles().filter(|r| r.moves == Moves::Keeps) {
            app.insert_resource(StartNear(role.role.clone()));
            stand_near(app.world_mut());
            settle(&mut app);
            let person = {
                let world = app.world_mut();
                let mut q = world.query::<(&Position, &Townsperson)>();
                q.iter(world).find(|(_, p)| p.0 == role.role).map(|(p, _)| p.0).expect("the person stands")
            };
            assert_eq!(geometry::chebyshev(at(&app, me), person), 1, "beside the {} at {person:?}, stood at {:?}", role.role, at(&app, me));
            assert!(app.world().get_resource::<StartNear>().is_none(), "the ask is forgotten once answered");
            asked += 1;
        }
        assert!(asked > 0, "some of Mereholt's people keep a post");
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
