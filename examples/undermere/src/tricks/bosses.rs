//! The bosses' own clocks and nests: the Widow's egg sacs, the Mother
//! Below's spawnlings and the Drowned King's pool.

use super::*;

/// The prop the Widow's cave scatters: `assets/props.ron`'s own name.
const EGG_SAC: &str = "egg sac";

/// What an egg sac hatches into, and how many: the content sheet's own
/// two cave spiders, shared by both ways a sac hatches so one number says
/// it once.
const EGG_SAC_HATCH: &str = "cave spider";

/// How many hatch at once.
const EGG_SAC_HATCH_COUNT: u32 = 2;

/// Hatches an egg sac into `EGG_SAC_HATCH_COUNT` of `EGG_SAC_HATCH`
/// the moment something steps beside it, despawning the sac so the same
/// step cannot hatch it twice: the Widow's "struck or stepped beside",
/// the half a blow does not cover. Read by `PropKind` against the
/// registry each pass rather than a marker component, since nothing hands
/// this game the entity a prefab's own slot spawned when it stood the sac
/// up. A blow that spends the sac's one health hatches it too, through
/// `props.ron`'s own `destroyed` trigger and
/// [`crate::effects::bodies::Hatch`] instead of here, since combat
/// already knows where and when it broke.
///
/// Only a foe of what hatches sets one off, the adventurer or anyone else
/// at odds with the brood: the Widow walking her own cave, and her
/// hatchlings stepping off their sac beside the next, once hatched every
/// sac in a chain. And once a pass for each sac, however many steps land
/// beside it, since the despawn waits for the pass's end and a second
/// step in it hatched the same sac twice.
///
/// In `TurnSet::React`.
pub fn hatch_beside(
    mut commands: Commands,
    mut steps: MessageReader<Stepped>,
    registries: Res<Registries>,
    map: Res<WorldMap>,
    foes: BroodFoes,
    props: Query<(Entity, &PropKind, &Position, &OnMap), With<Prop>>,
) {
    let Some(sac) = registries.props.id(EGG_SAC) else { return };
    let here = map.current();
    let mut hatched: Vec<Entity> = Vec::new();
    for step in steps.read() {
        if !foes.is_foe(step.actor) {
            continue;
        }
        for (prop, kind, pos, on) in &props {
            if kind.0 == sac && on.0 == here && geometry::chebyshev(pos.0, step.to) <= 1 && !hatched.contains(&prop) {
                hatched.push(prop);
                commands.entity(prop).despawn();
                let at = pos.0;
                commands.queue(move |world: &mut World| crate::effects::bodies::hatch(world, EGG_SAC_HATCH, EGG_SAC_HATCH_COUNT, at, here));
            }
        }
    }
}

/// Whose step beside an egg sac is a foe's of what hatches from it: the
/// side `EGG_SAC_HATCH` is written with, against the side of whoever
/// stepped, by the faction matrix every blow reads.
#[derive(bevy::ecs::system::SystemParam)]
pub struct BroodFoes<'w, 's> {
    bestiary: Res<'w, Bestiary>,
    rules: Option<Res<'w, CombatRules>>,
    sides: Query<'w, 's, &'static Faction>,
}

impl BroodFoes<'_, '_> {
    /// Whether `actor` is at odds with the brood: nothing with no side, and
    /// nothing in a game with no faction matrix to say.
    fn is_foe(&self, actor: Entity) -> bool {
        let (Some(rules), Some(brood)) = (self.rules.as_deref(), self.bestiary.defs.id(EGG_SAC_HATCH)) else { return false };
        let brood = self.bestiary.defs.get(brood).faction;
        self.sides.get(actor).is_ok_and(|side| rules.factions.relation(side.0, brood) == Relation::Hostile)
    }
}

/// How many of a [`Spawns`] kind's own turns pass between each spawnling:
/// the content sheet's own three.
const SPAWN_INTERVAL: u32 = 3;

/// What a [`Spawns`] kind spawns: the content sheet's own spawnling.
const SPAWNLING: &str = "spawnling";

/// Marks a kind that spawns young from the walls near it, a counter
/// toward `SPAWN_INTERVAL` of its own turns: the Mother Below's
/// `spawning`. Marked at spawn off [`Tricks::spawns`].
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Spawns {
    /// Its own turns since the last one spawned.
    elapsed: u32,
}

/// Links a spawnling to whoever's own `spawning` made it, so
/// [`die_with_mother`] takes it out of play the moment its maker dies:
/// the Mother Below's "spawnlings die with her".
#[derive(Component, Debug, Clone, Copy)]
pub struct Spawned(Entity);

/// Ticks every [`Spawns`] kind's own counter on its own `ActionDone`, and
/// stands a `SPAWNLING` at a wall cell in its sight every
/// `SPAWN_INTERVAL`th: the Mother Below's `spawning`, "a spawnling
/// appears at a wall cell in its sight".
///
/// In `TurnSet::React`, after the engine has reported this pass's own
/// `ActionDone`.
pub fn spawn_young(mut done: MessageReader<ActionDone>, mut spawners: Query<&mut Spawns>, mut commands: Commands) {
    for d in done.read() {
        let Ok(mut spawns) = spawners.get_mut(d.actor) else { continue };
        spawns.elapsed += 1;
        if spawns.elapsed < SPAWN_INTERVAL {
            continue;
        }
        spawns.elapsed = 0;
        let spawner = d.actor;
        commands.queue(move |world: &mut World| spawn_one_young(world, spawner));
    }
}

/// `spawner`'s own one spawnling, as [`spawn_young`] says: the nearest
/// wall cell in its sight, then by row and column, opened to the floor
/// tile `spawner` itself stands on rather than a tile named here, so this
/// reads nothing of `dungeon.rs`'s own and works wherever a future
/// `spawns` kind is put down; stood with a [`Spawned`] link back to
/// `spawner`, so its own death takes the spawnling with it. Nothing with
/// no such cell in `spawner`'s sight.
///
/// Never a wall on the place's own edge, one with a neighbour off the
/// map: the topmost wall in sight was the first taken, and under a real
/// throne that was the floor's outermost row, opened onto nothing.
fn spawn_one_young(world: &mut World, spawner: Entity) {
    let Some(young) = world.resource::<Bestiary>().defs.id(SPAWNLING) else { return };
    let Some(at) = world.get::<Position>(spawner).map(|p| p.0) else { return };
    let map = world.get::<OnMap>(spawner).map_or(MapId::ZERO, |m| m.0);
    let Some(view) = world.get::<Viewshed>(spawner).cloned() else { return };
    let cell = {
        let worldmap = world.resource::<WorldMap>();
        let tables = worldmap.tables();
        let inside = |p: Point| geometry::square(p, 1).all(|n| worldmap.tile(n).is_some());
        let mut walls: Vec<Point> = geometry::square(at, view.range)
            .filter(|p| view.can_see(*p) && inside(*p))
            .filter(|p| worldmap.tile(*p).is_some_and(|t| !tables.walkable[t.index()]))
            .collect();
        walls.sort_by_key(|p| (geometry::chebyshev(*p, at), p.y, p.x));
        walls.into_iter().next()
    };
    let Some(cell) = cell else { return };
    let Some(own_floor) = world.resource::<WorldMap>().tile(at) else { return };
    world.resource_mut::<WorldMap>().set_tile(cell, own_floor);
    let made = spawn_monster_now(world, young, cell, map);
    world.entity_mut(made).insert(Spawned(spawner));
    if seen_by_player(world, at) {
        if let Some(kind) = world.get::<Kind>(spawner).map(|k| k.0) {
            let name = world.resource::<Bestiary>().defs.get(kind).name.clone();
            world.resource_mut::<MonsterLore>().show(&name, "spawning");
        }
        world.write_message(Tell::new("A spawnling tears its way out of the wall!".to_string(), Tones::BAD));
    }
}

/// Takes every [`Spawned`] whose own maker just died out of play with it,
/// by `allies::vanish` rather than a death of its own, since nothing
/// struck it down: the Mother Below's spawnlings, "die with her".
///
/// In `TurnSet::React`, after the engine has written this pass's own
/// `DeathEvent`.
pub fn die_with_mother(mut deaths: MessageReader<DeathEvent>, spawned: Query<(Entity, &Spawned)>, mut commands: Commands) {
    let dead: Vec<Entity> = deaths.read().map(|d| d.entity).collect();
    if dead.is_empty() {
        return;
    }
    for (e, link) in &spawned {
        if dead.contains(&link.0) {
            commands.queue(move |world: &mut World| crate::allies::vanish(world, e));
        }
    }
}

/// How many of a [`Wells`] kind's own turns pass between each thrall:
/// twelve, where the content sheet said eight, since at eight a fight with
/// the Drowned King was a fight with his court and a thrall a turn behind
/// it, and the soak's delves came to his hall to die on the thralls.
const RISE_INTERVAL: u32 = 12;

/// How far from a [`Wells`] kind a cell of water may lie and still rise a
/// thrall: wide enough to cover the one hall a `welling` kind's own
/// water, a black pool, ever stands in.
const RISE_RANGE: i32 = 12;

/// Marks a kind whose own water rises a drowned thrall every
/// `RISE_INTERVAL` of its own turns: the Drowned King's `welling`.
/// Marked at spawn off [`Tricks::wells`].
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Wells {
    /// Its own turns since the last one rose.
    elapsed: u32,
}

/// Ticks every [`Wells`] kind's own counter on its own `ActionDone`, and
/// calls [`crate::effects::bodies::call_drowned`] for one thrall every
/// `RISE_INTERVAL`th: the Drowned King's `welling`, forced rather than
/// chosen, unlike the Drowned Abbess's own `calling the drowned` ability.
/// Learned only where the adventurer sees the king or the water a thrall
/// rose from, as `spawning` is only where they see its maker: a thrall
/// risen in a hall they never looked into teaches nothing.
///
/// In `TurnSet::React`, after the engine has reported this pass's own
/// `ActionDone`.
pub fn rise_from_the_pool(mut done: MessageReader<ActionDone>, mut wells: Query<&mut Wells>, mut commands: Commands) {
    for d in done.read() {
        let Ok(mut w) = wells.get_mut(d.actor) else { continue };
        w.elapsed += 1;
        if w.elapsed < RISE_INTERVAL {
            continue;
        }
        w.elapsed = 0;
        let king = d.actor;
        commands.queue(move |world: &mut World| {
            if let (_, true) = crate::effects::bodies::call_drowned(world, king, 1, RISE_RANGE)
                && let Some(kind) = world.get::<Kind>(king).map(|k| k.0)
            {
                let name = world.resource::<Bestiary>().defs.get(kind).name.clone();
                world.resource_mut::<MonsterLore>().show(&name, "welling");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Widow's egg sac, stepped beside: two cave spiders hatch and
    /// the sac itself is gone, so the same step cannot hatch it twice.
    #[test]
    fn an_egg_sac_hatches_two_cave_spiders_when_stepped_beside_and_then_is_gone() {
        use crate::bestiary::Bestiary;
        use crate::testing::{arena, pass_turns};
        let (mut app, me, here) = arena(31);
        let sac_id = app.world().resource::<Registries>().props.expect(EGG_SAC);
        let map = app.world().resource::<WorldMap>().current();
        let at = here.offset(3, 0);
        let sac = app.world_mut().spawn((Prop, PropKind(sac_id), Position(at), OnMap(map))).id();
        app.world_mut().write_message(Stepped { actor: me, from: here, to: at.offset(-1, 0) });
        pass_turns(&mut app, 1);
        assert!(app.world().get::<Prop>(sac).is_none(), "the sac is gone, hatched");
        let spider_id = app.world().resource::<Bestiary>().defs.expect(EGG_SAC_HATCH);
        let world = app.world_mut();
        let spiders = world.query::<&Kind>().iter(world).filter(|k| k.0 == spider_id).count();
        assert_eq!(spiders, EGG_SAC_HATCH_COUNT as usize, "two cave spiders hatched");
    }

    /// The Widow walking past her own sac hatches nothing; two of the
    /// adventurer's steps beside one sac in the one pass hatch it once.
    #[test]
    fn an_egg_sac_hatches_once_a_pass_and_only_for_a_foe_of_the_brood() {
        use crate::bestiary::Bestiary;
        use crate::testing::{arena, monster, pass_turns};
        let (mut app, me, here) = arena(34);
        let sac_id = app.world().resource::<Registries>().props.expect(EGG_SAC);
        let map = app.world().resource::<WorldMap>().current();
        let at = here.offset(3, 0);
        let sac = app.world_mut().spawn((Prop, PropKind(sac_id), Position(at), OnMap(map))).id();
        let widow = monster(&mut app, "the Widow", here.offset(4, 2));
        app.world_mut().entity_mut(widow).remove::<Mind>();
        app.world_mut().write_message(Stepped { actor: widow, from: at.offset(1, 2), to: at.offset(1, 1) });
        pass_turns(&mut app, 1);
        assert!(app.world().get::<Prop>(sac).is_some(), "her own step beside it hatches nothing");
        app.world_mut().write_message(Stepped { actor: me, from: here, to: at.offset(-1, 0) });
        app.world_mut().write_message(Stepped { actor: me, from: at.offset(-1, 0), to: at.offset(-1, 1) });
        pass_turns(&mut app, 1);
        let spider_id = app.world().resource::<Bestiary>().defs.expect(EGG_SAC_HATCH);
        let world = app.world_mut();
        let spiders = world.query::<&Kind>().iter(world).filter(|k| k.0 == spider_id).count();
        assert_eq!(spiders, EGG_SAC_HATCH_COUNT as usize, "two steps of the adventurer's in one pass hatch it once");
    }

    /// The same sac, struck rather than stepped beside: its one health
    /// spent lands `props.ron`'s own `destroyed` trigger, `Hatch`, the
    /// other half of "struck or stepped beside".
    #[test]
    fn an_egg_sac_also_hatches_when_struck() {
        use crate::bestiary::Bestiary;
        use crate::testing::{arena, kill};
        let (mut app, _, here) = arena(32);
        let sac_id = app.world().resource::<Registries>().props.expect(EGG_SAC);
        let map = app.world().resource::<WorldMap>().current();
        let at = here.offset(2, 0);
        let sac = app.world_mut().spawn((Prop, PropKind(sac_id), Position(at), OnMap(map), Health::full(1))).id();
        // A pass so `arm_props` gives the freshly spawned sac its own
        // `Triggers`, built from `props.ron`'s `destroyed` the moment
        // the app was built; only then does breaking it land `Hatch`.
        app.update();
        kill(&mut app, sac, None);
        let spider_id = app.world().resource::<Bestiary>().defs.expect(EGG_SAC_HATCH);
        let world = app.world_mut();
        let spiders = world.query::<&Kind>().iter(world).filter(|k| k.0 == spider_id).count();
        assert_eq!(spiders, EGG_SAC_HATCH_COUNT as usize, "two cave spiders hatched when the sac was struck down");
    }

    /// The Mother Below's `spawning`: left alone near a wall, she stands a
    /// spawnling at it given enough of her own turns, as the content
    /// sheet's "every 3 of its turns while alive" says.
    #[test]
    fn a_spawns_kind_stands_a_spawnling_at_a_wall_cell_in_its_sight_given_enough_of_its_own_turns() {
        use crate::bestiary::Bestiary;
        use crate::testing::{arena, monster, pass_turns, sturdy, wall_off};
        let (mut app, me, here) = arena(60);
        sturdy(&mut app, me);
        monster(&mut app, "the Mother Below", here.offset(4, 0));
        wall_off(&mut app, &[here.offset(4, 2)]);
        pass_turns(&mut app, 40);
        let spawnling = app.world().resource::<Bestiary>().defs.expect(SPAWNLING);
        let world = app.world_mut();
        let count = world.query::<&Kind>().iter(world).filter(|k| k.0 == spawnling).count();
        assert!(count >= 1, "no spawnling stood after forty of the adventurer's own turns");
    }

    /// The Drowned King's `welling`: left alone near a black pool, he
    /// wells a drowned thrall up out of it given enough of his own turns,
    /// every twelve of his own turns, as `RISE_INTERVAL` says.
    #[test]
    fn a_wells_kind_wells_a_drowned_thrall_up_from_water_in_reach_given_enough_of_its_own_turns() {
        use crate::bestiary::Bestiary;
        use crate::testing::{arena, monster, pass_turns, sturdy};
        let (mut app, me, here) = arena(61);
        sturdy(&mut app, me);
        monster(&mut app, "the Drowned King", here.offset(4, 0));
        let pool_at = here.offset(4, 2);
        {
            let mut map = app.world_mut().resource_mut::<WorldMap>();
            let pool = TileId(map.tables().names.iter().position(|n| n == "black pool").expect("a black pool is a tile") as u16);
            map.set_tile(pool_at, pool);
        }
        pass_turns(&mut app, 70);
        let thrall = app.world().resource::<Bestiary>().defs.expect("drowned thrall");
        let world = app.world_mut();
        let count = world.query::<&Kind>().iter(world).filter(|k| k.0 == thrall).count();
        assert!(count >= 1, "no drowned thrall rose after seventy of the adventurer's own turns");
        assert!(app.world().resource::<MonsterLore>().has_shown("the Drowned King", "welling"), "risen in the adventurer's sight, so learned");
    }

    /// The same king and pool walled off from the adventurer's sight: a
    /// thrall still rises, and nothing of `welling` is learned, since
    /// neither the king nor the water it rose from was seen. The ground
    /// round the hall, as far as a thrall may rise from anywhere the king
    /// can stand inside it, is laid plain first, so the walled pool is the
    /// only water and no thrall rises from a floor's own shallows in the
    /// open.
    #[test]
    fn a_thrall_welling_up_out_of_the_adventurers_sight_teaches_nothing() {
        use crate::bestiary::Bestiary;
        use crate::testing::{arena, lay_floor, monster, pass_turns, sturdy, wall_off};
        let (mut app, me, here) = arena(62);
        sturdy(&mut app, me);
        let hall = here.offset(3, 0);
        lay_floor(&mut app, &geometry::square(hall, RISE_RANGE + 1).collect::<Vec<_>>());
        let ring: Vec<Point> = geometry::square(hall, 2).filter(|p| geometry::chebyshev(*p, hall) == 2).collect();
        wall_off(&mut app, &ring);
        monster(&mut app, "the Drowned King", hall);
        {
            let mut map = app.world_mut().resource_mut::<WorldMap>();
            let pool = TileId(map.tables().names.iter().position(|n| n == "black pool").expect("a black pool is a tile") as u16);
            map.set_tile(hall.offset(0, 1), pool);
        }
        pass_turns(&mut app, 70);
        let thrall = app.world().resource::<Bestiary>().defs.expect("drowned thrall");
        let world = app.world_mut();
        let count = world.query::<&Kind>().iter(world).filter(|k| k.0 == thrall).count();
        assert!(count >= 1, "a thrall rose behind the walls all the same");
        assert!(!app.world().resource::<MonsterLore>().has_shown("the Drowned King", "welling"), "never seen, so never learned");
    }

    /// On a real floor ten, the Mother Below's spawnlings and the Drowned
    /// King's thralls come out of the throne's own geometry: each stands on
    /// open ground inside the floor and off its outermost ring, so a wall
    /// torn open for a spawnling never opens the floor onto nothing. The
    /// adventurer is at peace with everyone, so what the boss does in forty
    /// turns is its own tricks and nothing it was provoked into.
    #[test]
    fn on_a_real_throne_floor_spawnlings_and_thralls_stand_on_open_ground_inside_it() {
        use crate::bestiary::Bestiary;
        use crate::dungeon::{FLOORS, Undermere, map_of};
        use crate::testing::{adventurer, arrive, headless, pass_turns, sturdy};
        use rl_engine::rl_core::RunSeed;
        for (piece, young) in [("the Mother Below", SPAWNLING), ("the throne", "drowned thrall")] {
            let seed = (0..100).map(RunSeed).find(|s| Undermere::new(*s).final_boss().piece == piece).expect("some seed draws each boss");
            let mut app = headless(seed);
            let me = adventurer(&mut app);
            sturdy(&mut app, me);
            let peace = CombatRules::new();
            app.insert_resource(peace);
            let floor = map_of(FLOORS);
            arrive(&mut app, floor);
            pass_turns(&mut app, 40);
            let young = app.world().resource::<Bestiary>().defs.expect(young);
            let bounds = app.world().resource::<WorldMap>().place(floor).expect("floor ten is built").terrain.bounds();
            let world = app.world_mut();
            let stood: Vec<Point> =
                world.query::<(&Kind, &Position, &OnMap)>().iter(world).filter(|(k, _, on)| k.0 == young && on.0 == floor).map(|(_, p, _)| p.0).collect();
            assert!(!stood.is_empty(), "{seed:?}: under {piece}, nothing came in forty turns");
            let map = app.world().resource::<WorldMap>();
            for at in stood {
                assert!(
                    bounds.contains(at) && !bounds.is_border(at),
                    "{seed:?}: under {piece}, one stands at {at:?}, on or past the floor's own edge {bounds:?}"
                );
                assert!(map.is_walkable_on(floor, at), "{seed:?}: under {piece}, one stands at {at:?}, which is not open ground");
            }
        }
    }

    /// The Mother Below's own spawnlings stop the moment she dies: killing
    /// her takes every `Spawned` link of hers out of play with her, as
    /// `allies::vanish` does, since nothing struck them down.
    #[test]
    fn the_mother_belows_spawnlings_are_taken_out_of_play_the_moment_she_dies() {
        use crate::testing::{arena, kill, monster, settle};
        let (mut app, _, here) = arena(70);
        let mother = monster(&mut app, "the Mother Below", here.offset(3, 0));
        let young = monster(&mut app, "spawnling", here.offset(3, 1));
        app.world_mut().entity_mut(young).insert(Spawned(mother));
        kill(&mut app, mother, None);
        settle(&mut app);
        assert!(app.world().get_entity(young).is_err(), "the spawnling is gone with her");
    }
}
