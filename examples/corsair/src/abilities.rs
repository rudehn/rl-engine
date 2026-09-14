//! What a pirate calls on besides a cutlass.
//!
//! The abilities are data in `assets/abilities.ron` and the engine resolves
//! them: the aim, the footprint, the cost, the cooldown and the turn. What is
//! here is Corsair's side of that boundary, and it is small on purpose. The
//! registries an ability file names, handed to the engine's loader, so an
//! ability names a tag or a status the way the rest of the content does.
//! `Plunder`, the one effect Corsair adds to the engine's seven. And the
//! keys, which write [`AimAt`] and stop there.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_rules::ability::RawValue;

use crate::items::Armory;
use crate::monsters::Bestiary;

/// The abilities, compiled in so the binary runs from anywhere.
const ABILITIES_RON: &str = include_str!("../assets/abilities.ron");

/// What the player knows, in the order `1` to `4` aim them.
pub const PLAYER_KNOWS: [&str; 4] = ["broadside", "grapnel", "swig", "plunder"];

/// The player, and only while it holds the turn.
type PlayerHolding = (With<Player>, With<MyTurn>);

/// Doubloons a monster carries, which `plunder` spills at its feet.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Purse(pub u32);

/// Loads and builds the abilities; panics naming every problem, at startup
/// rather than the first time a key is pressed.
///
/// No new tables: the stats, tags and slots are the armory's, the damage
/// kinds the bestiary's, the statuses the ones `statuses.ron` defines.
pub fn load(armory: &Armory, bestiary: &Bestiary, statuses: &StatusRules, kinds: &EffectKinds) -> Abilities {
    let names = Names::new().stats(&armory.stats).tags(&armory.tags).slots(&armory.slots).damage_kinds(&bestiary.kinds).statuses(&statuses.defs);
    Abilities::load(ABILITIES_RON, kinds, &names).unwrap_or_else(|e| panic!("assets/abilities.ron: {e}"))
}

/// The player's grants.
pub fn player_grants(abilities: &Abilities) -> Grants {
    Grants(PLAYER_KNOWS.iter().map(|n| abilities.expect(n)).collect())
}

/// Shake a foe down: whatever is in its purse spills onto the ground at its
/// feet, to be picked up like any other loot.
///
/// A purse is Corsair's, not the engine's, so no engine effect could reach
/// it; this one reaches it through `commands`, which is the whole of the
/// escape hatch. It spills rather than pockets because a stack on the ground
/// merges into the bag through the engine's own pick-up, and an effect that
/// merged stacks itself would be a second copy of that rule.
#[derive(Debug, Clone, Copy, Default)]
pub struct Plunder;

impl Effect for Plunder {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        for target in landing.targets.clone() {
            world.commands.queue(move |w: &mut World| {
                let Some(coin) = w.get::<Purse>(target).map(|p| p.0).filter(|n| *n > 0) else { return };
                let Some(at) = w.get::<Position>(target).map(|p| p.0) else { return };
                w.entity_mut(target).insert(Purse(0));
                w.resource_scope(|w: &mut World, armory: Mut<Armory>| {
                    let mut queue = bevy::ecs::world::CommandQueue::default();
                    let mut commands = Commands::new(&mut queue, w);
                    armory.spawn(&mut commands, armory.defs.expect("doubloons"), coin, Some(at));
                    queue.apply(w);
                });
            });
        }
    }
}

impl FromArgs for Plunder {
    const KIND: &'static str = "Plunder";

    fn from_args(_: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        Ok(Plunder)
    }
}

/// `1` to `4` aim the abilities the player knows, in order; `a` lists them
/// with the reasons any is out of reach.
///
/// A key writes [`AimAt`] and stops. Whether a cursor opens, where, how it
/// is steered and what the use costs are the engine's, which is why nothing
/// here knows what a broadside does.
pub fn ability_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut modals: ResMut<Modals>,
    player: Query<(Entity, &Known), PlayerHolding>,
    mut aims: MessageWriter<AimAt>,
) {
    const SLOTS: [KeyCode; 4] = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4];
    let list = ability_modal(&modals);
    if keys.just_pressed(KeyCode::KeyA) && (modals.is_top(list) || !modals.any_open()) {
        modals.toggle(list);
        return;
    }
    if modals.any_open() {
        return;
    }
    let Ok((user, known)) = player.single() else { return };
    if let Some(slot) = SLOTS.iter().position(|k| keys.just_pressed(*k))
        && let Some((ability, _)) = known.iter().nth(slot)
    {
        aims.write(AimAt { user, ability });
    }
}

/// Abilities used and refused, in words.
pub fn narrate_abilities(
    mut events: MessageReader<AbilityEvent>,
    abilities: Res<Abilities>,
    turns: Res<Turns>,
    mut log: ResMut<MessageLog>,
    names: Query<&Name>,
    players: Query<(), With<Player>>,
) {
    let turn = turns.turn_number();
    let named =
        |e: Entity| if players.contains(e) { "you".to_string() } else { names.get(e).map(|n| n.as_str().to_string()).unwrap_or_else(|_| "something".into()) };
    for ev in events.read() {
        match ev {
            AbilityEvent::Used { user, ability, targets, .. } => {
                let what = &abilities.get(*ability).name;
                let mine = players.contains(*user);
                let (who, verb) = if mine { ("You".to_string(), "use") } else { (upper_first(&named(*user)), "uses") };
                let line = match targets.as_slice() {
                    [] => format!("{who} {verb} {what}."),
                    [one] => format!("{who} {verb} {what} on {}.", named(*one)),
                    many => format!("{who} {verb} {what}, catching {}.", many.len()),
                };
                log.push(line, if mine { Tones::TEXT } else { Tones::BAD }, turn);
            }
            AbilityEvent::Refused { user, ability, .. } if players.contains(*user) => {
                log.bad(format!("You cannot use {} right now; `a` says why.", abilities.get(*ability).name), turn);
            }
            AbilityEvent::Refused { .. } => {}
        }
    }
}

fn upper_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::ItemKind;
    use rl_engine::rl_core::RunSeed;

    /// A headless Corsair holding the player's first turn, and where its
    /// saves go.
    fn settled(tag: &str) -> (App, Entity, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("corsair-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = crate::testing::headless(RunSeed(7), false, &dir);
        app.update();
        app.update();
        let me = {
            let w = app.world_mut();
            let mut q = w.query_filtered::<Entity, With<Player>>();
            q.single(w).unwrap()
        };
        (app, me, dir)
    }

    /// A surface cutthroat `dx` tiles east of the player.
    fn cutthroat(app: &mut App, me: Entity, dx: i32) -> Entity {
        let at = app.world().get::<Position>(me).expect("a position").0.offset(dx, 0);
        app.world_mut().resource_scope(|world: &mut World, bestiary: Mut<Bestiary>| {
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let mut commands = Commands::new(&mut queue, world);
            let e = bestiary.spawn(&mut commands, bestiary.defs.expect("cutthroat"), at);
            queue.apply(world);
            e
        })
    }

    fn use_it(app: &mut App, me: Entity, name: &str, aim: Point) {
        let ability = app.world().resource::<Abilities>().expect(name);
        app.world_mut().write_message(Intent::new(me, Use { ability, aim }));
        app.update();
    }

    /// The bag's powder stack, as a count.
    fn powder(app: &mut App, me: Entity) -> Option<(Entity, u32)> {
        let kind = app.world().resource::<Armory>().defs.expect("powder");
        let bag = app.world().get::<Inventory>(me).expect("a bag").items.clone();
        bag.into_iter()
            .find(|i| app.world().get::<ItemKind>(*i).is_some_and(|k| k.0 == kind))
            .map(|i| (i, app.world().get::<Stack>(i).map(|s| s.count).unwrap_or(1)))
    }

    #[test]
    fn plunder_spills_a_foes_purse_on_the_ground_at_its_feet() {
        let (mut app, me, dir) = settled("plunder");
        let foe = cutthroat(&mut app, me, 1);
        app.world_mut().entity_mut(foe).insert(Purse(9));
        let at = app.world().get::<Position>(foe).unwrap().0;
        app.update();

        use_it(&mut app, me, "plunder", at);
        app.update();
        assert_eq!(app.world().get::<Purse>(foe), Some(&Purse(0)), "the purse is empty");
        let doubloons = app.world().resource::<Armory>().defs.expect("doubloons");
        let spilled: u32 = {
            let w = app.world_mut();
            let mut q = w.query::<(&ItemKind, &Position, Option<&Stack>)>();
            q.iter(w).filter(|(k, p, _)| k.0 == doubloons && p.0 == at).map(|(_, _, s)| s.map(|s| s.count).unwrap_or(1)).sum()
        };
        assert_eq!(spilled, 9, "and all of it lies where the foe stood");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_broadside_spends_one_powder_from_the_bag() {
        let (mut app, me, dir) = settled("broadside");
        let foe = cutthroat(&mut app, me, 2);
        let at = app.world().get::<Position>(foe).unwrap().0;
        app.update();
        let (_, before) = powder(&mut app, me).expect("the player starts with powder");

        use_it(&mut app, me, "broadside", at);
        let (_, after) = powder(&mut app, me).expect("powder left");
        assert_eq!(after, before - 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn with_no_powder_a_broadside_is_refused_and_costs_no_time() {
        let (mut app, me, dir) = settled("dry");
        let foe = cutthroat(&mut app, me, 2);
        let at = app.world().get::<Position>(foe).unwrap().0;
        app.update();
        let (stack, _) = powder(&mut app, me).expect("the player starts with powder");
        app.world_mut().get_mut::<Inventory>(me).expect("a bag").items.retain(|i| *i != stack);
        app.world_mut().despawn(stack);
        let clock = app.world().resource::<Turns>().now();

        use_it(&mut app, me, "broadside", at);
        assert_eq!(app.world().resource::<Turns>().now(), clock, "refused, so the turn is still the player's");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
