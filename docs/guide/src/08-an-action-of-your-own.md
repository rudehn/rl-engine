# An action of your own

> Run it: `cargo run -p tutorial --bin step08_shove`
>
> Source: [`step08_shove.rs`](https://github.com/rudehn/rl-engine/blob/main/examples/tutorial/src/bin/step08_shove.rs)

The engine knows walking, attacking, using and throwing.
This chapter adds an action it has never heard of: a shove, for the player and for a monster.

## An action is a type

<!-- include: ../../../examples/tutorial/src/bin/step08_shove.rs:action -->
```rust,no_run
/// Shove whoever stands next to you, in this direction, back one cell.
///
/// An action is a type. Registering it as a choice makes `Intent<Shove>` a
/// message, and lets a mind choose it too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shove(Direction);
impl Action for Shove {}
impl Choice for Shove {
    fn name(&self) -> &'static str {
        "shove"
    }
}

/// A shove that landed, for the log.
#[derive(Message, Debug, Clone, Copy)]
struct Shoved {
    target: Entity,
}
```

`Action` makes `Shove` something an actor can be asked to do.
`Choice` lets a mind choose it too.
`Shoved` is a message for the log.

<!-- include: ../../../examples/tutorial/src/bin/step08_shove.rs:register -->
```rust,no_run
        // The shove: something the player may do and a mind may choose,
        // and the system that resolves it.
        app.add_choice::<Shove>().add_message::<Shoved>().add_systems(Turn, resolve_shoves.in_set(ResolveSet::Act));
```

`add_choice` registers both, and `resolve_shoves` runs in `ResolveSet::Act` with the engine's own resolvers.

## The resolver

<!-- include: ../../../examples/tutorial/src/bin/step08_shove.rs:resolver -->
```rust,no_run
/// A shove costs half a turn.
const SHOVE_COST: u32 = BASE_ACTION_COST / 2;

/// What the resolver reads and moves.
#[derive(bevy::ecs::system::SystemParam)]
struct Shoving<'w, 's> {
    occupancy: ResMut<'w, Occupancy>,
    map: Res<'w, WorldMap>,
    holders: Query<'w, 's, &'static Position, With<MyTurn>>,
    targets: Query<'w, 's, (&'static mut Position, Option<&'static mut Viewshed>), Without<MyTurn>>,
}

/// Resolves a shove, the shape every resolver has: claim the turn, do the
/// thing, and say how it went. `failed` leaves the player holding the turn.
fn resolve_shoves(mut intents: MessageReader<Intent<Shove>>, mut resolution: Resolution, mut shoved: MessageWriter<Shoved>, mut world: Shoving) {
    for intent in intents.read() {
        if !resolution.claim(intent.actor) {
            continue;
        }
        let Ok(from) = world.holders.get(intent.actor) else {
            resolution.failed(intent.actor, SHOVE_COST);
            continue;
        };
        let offset = intent.action.0.offset();
        let behind = from.0 + offset + offset;
        let room = world.map.is_walkable(behind) && !world.occupancy.is_occupied(behind);
        let pushed = world.occupancy.first_at(from.0 + offset).filter(|_| room).and_then(|t| world.targets.get_mut(t).ok().map(|found| (t, found)));
        let Some((target, (mut pos, viewshed))) = pushed else {
            // Nobody there, or nowhere for them to go.
            resolution.failed(intent.actor, SHOVE_COST);
            continue;
        };
        world.occupancy.relocate(target, pos.0, behind);
        pos.0 = behind;
        if let Some(mut v) = viewshed {
            v.dirty = true;
        }
        shoved.write(Shoved { target });
        resolution.done(intent.actor, SHOVE_COST);
    }
}
```

Every resolver has the same shape.
`claim` the turn so nothing else spends it, do the thing, then report.
`done` charges the cost.
`failed` leaves the player holding the turn, and charges a monster, so a monster never retries the impossible forever.

## The key

<!-- include: ../../../examples/tutorial/src/bin/step08_shove.rs:keys -->
```rust,no_run
/// Warren's own keys.
#[derive(Resource, Clone, Copy)]
struct Binds {
    shove: ControlId,
    screech: ControlId,
    eat: ControlId,
    throw: ControlId,
    lantern: ControlId,
    quit: ControlId,
}

/// Declares the keys. Walking and waiting are the engine's standard keys.
fn declare_controls(app: &mut App) {
    app.add_plugins(
        PlayerKeysPlugin::new()
            .walk("Move", "walk or attack")
            .go_through("Move", "take the stairs")
            .pick_up("Act", "pick up what is here", Otherwise::say("There is nothing here."))
            .wait("Act", "wait a turn"),
    );
    let binds = Binds {
        // The same direction keys with Shift held. A chord matches exactly,
        // so Shift and `l` never reads as a step.
        shove: app.add_control("Move", "shove", Keys::Directions { shift: true }),
        screech: app.add_control("Act", "screech", KeyCode::KeyA),
        eat: app.add_control("Act", "eat a crust", KeyCode::KeyE),
        throw: app.add_control("Act", "throw a rock", KeyCode::KeyR),
        lantern: app.add_control("Act", "lantern on or off", KeyCode::KeyT),
        quit: app.add_control("Game", "quit", KeyCode::KeyQ),
    };
    app.insert_resource(binds);
}
```

`Keys::Directions { shift: true }` binds Shift with every direction key, and `keys.direction(binds.shove)` reads which:

<!-- include: ../../../examples/tutorial/src/bin/step08_shove.rs:input -->
```rust,no_run
/// The player, while it holds the turn, and its bag.
type PlayerTurn<'w, 's> = Query<'w, 's, (Entity, &'static Inventory), (With<Player>, With<MyTurn>)>;

/// Reads Warren's own keys once a frame. Asking to act is writing an
/// intent: the engine spends the turn, or refuses what cannot be done.
fn player_input(
    keys: ControlInput,
    binds: Res<Binds>,
    player: PlayerTurn,
    carried: Carried,
    knacks: Res<Knacks>,
    mut intents: PlayerIntents,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(binds.quit) {
        exit.write(AppExit::Success);
    }
    // Not the player's turn: drop the key.
    let Ok((entity, bag)) = player.single() else { return };
    if let Some(dir) = keys.direction(binds.shove) {
        intents.shoves.write(Intent::new(entity, Shove(dir)));
    } else if keys.just_pressed(binds.screech) {
        // Opens the aiming cursor. The engine uses the knack on Enter.
        intents.knacks.write(AimAt { user: entity, ability: knacks.screech });
    } else if keys.just_pressed(binds.eat) {
        if let Some(crust) = bag.items.iter().copied().find(|i| carried.crusts.contains(*i)) {
            intents.uses.write(Intent::new(entity, UseItem(crust)));
        }
    } else if keys.just_pressed(binds.throw)
        && let Some(rock) = bag.items.iter().copied().find(|i| carried.rocks.contains(*i))
    {
        // Opens the aiming cursor. The engine writes the throw on Enter.
        intents.aims.write(AimThrow { user: entity, item: rock });
    }
}

/// Every intent Warren's keys write, in one parameter.
#[derive(bevy::ecs::system::SystemParam)]
struct PlayerIntents<'w> {
    uses: MessageWriter<'w, Intent<UseItem>>,
    aims: MessageWriter<'w, AimThrow>,
    knacks: MessageWriter<'w, AimAt>,
    shoves: MessageWriter<'w, Intent<Shove>>,
}

/// What the bag might hold, so a key can pick the right thing.
#[derive(bevy::ecs::system::SystemParam)]
struct Carried<'w, 's> {
    crusts: Query<'w, 's, (), With<Crust>>,
    rocks: Query<'w, 's, (), With<Throwable>>,
}
```

## A monster that shoves

<!-- include: ../../../examples/tutorial/src/bin/step08_shove.rs:tactic -->
```rust,no_run
/// The hog's move: shove whoever is beside it instead of biting.
struct ShoveAdjacent;

impl Tactic<Entity> for ShoveAdjacent {
    fn name(&self) -> &'static str {
        "shove_adjacent"
    }

    fn evaluate(&self, ctx: &mut TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
        let me = ctx.snapshot.me.pos;
        let foe = ctx.snapshot.adjacent_enemies().next()?;
        Direction::between(me, foe.pos).map(|d| Decision::own(Shove(d)))
    }
}
```

A tactic looks at the snapshot of the world its mind sees and returns a decision, or nothing.
`Decision::own` wraps Warren's own action.

The warren hog has `shoves: true` in `rats.ron`, and the bestiary puts the tactic first in its brain:

<!-- include: ../../../examples/tutorial/src/bin/step08_shove.rs:brain -->
```rust,no_run
            let wits = if def.clever { Wits::SAPIENT } else { Wits::ANIMAL };
            let can = Repertoire { flee_at: def.flee_at, melee: true, abilities: def.clever, wits, ..Repertoire::default() };
            let mut brain = if def.clever { CLEVER.brain(can) } else { WARREN.brain(can) };
            // A shover leads with its shove: Warren's own choice, routed to
            // Warren's own action.
            if def.shoves {
                brain = brain.first(ShoveAdjacent);
            }
            minds.push(Arc::new(brain.build()));
```

## Try it

- Make a shove cost a full turn.
- Give the rat king `shoves: true`.

Next: [panels, and where to go next](09-where-to-go-next.md).
