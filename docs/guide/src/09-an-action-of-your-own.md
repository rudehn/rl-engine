# An action of your own

> Run it: `cargo run -p tutorial --bin step09_shove`
>
> Source: [`step09_shove.rs`](https://github.com/rudehn/rl-engine/blob/main/examples/tutorial/src/bin/step09_shove.rs)

Everything so far used actions the engine ships.
This one it has never heard of: shove a rat back a cell, for half a turn.

## An action is a type

<!-- include: ../../../examples/tutorial/src/bin/step09_shove.rs:action -->
```rust,no_run
/// Shove whoever stands one cell away in this direction back another cell.
///
/// An action is a type. There is no list in the engine for it to be added
/// to; registering it makes `Intent<Shove>` a message, and the sweep
/// refuses any that no resolver claims. It is a `Choice` as well, so a
/// hog's brain can decide it and the engine routes the decision to the
/// same intent the player's key writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shove(Direction);
impl Action for Shove {}
impl Choice for Shove {
    fn name(&self) -> &'static str {
        "shove"
    }
}

/// A shove that landed, for the log to read.
#[derive(Message, Debug, Clone, Copy)]
struct Shoved {
    target: Entity,
}
```

`Action` is an empty marker trait and `Intent<Shove>` is its own message type.
There is no `enum Action` for it to be added to.

```rust,no_run
        .add_choice::<Shove>()
        .add_message::<Shoved>()
        .add_systems(Turn, resolve_shoves.in_set(ResolveSet::Act))
```

`add_action` registers the message and installs a sweeper in `TurnSet::Sweep`, after everything that might have resolved the intent.
Warren says `add_choice`, which does everything `add_action` does and one thing more, for the hog at the end of this chapter.
Forget the resolver and you get a warning naming the type, not a frozen game with the player holding a turn nothing will spend.

## What a resolver owes

<!-- include: ../../../examples/tutorial/src/bin/step09_shove.rs:resolver -->
```rust,no_run
/// What the shove costs. A shove is quicker than a swing.
const SHOVE_COST: u32 = BASE_ACTION_COST / 2;

/// Everything the resolver moves.
#[derive(bevy::ecs::system::SystemParam)]
struct Shoving<'w, 's> {
    occupancy: ResMut<'w, Occupancy>,
    map: Res<'w, WorldMap>,
    holders: Query<'w, 's, &'static Position, With<MyTurn>>,
    targets: Query<'w, 's, (&'static mut Position, Option<&'static mut Viewshed>), Without<MyTurn>>,
}

/// Resolves a shove.
///
/// The shape every resolver has: claim the turn so nothing else spends it,
/// do the thing, and say how it went. `done` charges what it cost; `failed`
/// leaves the player holding the turn and charges anyone else, so a
/// monster cannot try the same impossible shove forever.
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
            // Nobody there, or nowhere for them to go: nothing happens, and
            // the turn is still the player's to spend on something else.
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

`Resolution` is the engine's side of every resolver, its own and yours.

**Claim the turn.** `resolution.claim` returns false if the actor holds no turn, so the intent is stale, or if something already spent this one. One turn is one action, across resolvers that have never heard of each other.

**Say how it went.** `resolution.done` charges what the action cost and requeues the actor. `resolution.failed` is for one that could not be done: the player keeps the turn at no cost, and anyone else is charged, because a monster handed a free retry asks again forever. That rule lives in `Resolution`, so no resolver has to remember it.

**Keep the indexes straight.** Moving something means `occupancy.relocate` as well as writing `Position`, and marking a moved viewshed dirty.

`SHOVE_COST` is `BASE_ACTION_COST / 2`: fifty against a step's hundred, both scaled by `Speed`.

Reporting through a `Shoved` message rather than logging from inside the resolver keeps narration out of it and lets anything else react later.

## A monster that shoves

The hog shoves too, and it takes three lines more than the player did.

<!-- include: ../../../examples/tutorial/src/bin/step09_shove.rs:tactic -->
```rust,no_run
/// The hog's move: shove whoever stands beside it rather than bite. A
/// tactic of Warren's own, in the brain beside the engine's, that decides
/// Warren's own action.
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

A tactic is a type that reads the snapshot and answers with a `Decision`.
The engine's decisions are its own actions, a step, a blow, a use; yours is `Decision::own(Shove(d))`, a box holding your type.
`Shove` implements `Choice` as well as `Action`, one `name` for the trace, and the brain puts `ShoveAdjacent` in front of `MeleeAdjacent` for any rat whose file says `shoves: true`.

```rust,no_run
        .add_choice::<Shove>()
```

`add_choice` replaces `add_action`.
It registers the action and its sweeper as before, and it routes every choice of that type a mind makes to the same `Intent<Shove>` the key writes, so `resolve_shoves` never learns whether a hog or a hand asked.
A choice nobody routed is refused by the sweeper, not lost.

What a tactic needs to know that the engine does not, a scent or a post to return to, you push onto the snapshot in `PerceiveSet::Annotate` with `add_sense` and read back in the tactic with `sense::<T>()`.
A game that decides a monster's whole turn itself claims it with `acting.claim_decision` in `TurnSet::Decide`, and the engine's brains leave that monster alone.

## Try it

- Make a shove fail against a heavier monster with `resolution.failed`.
- Charge a full turn instead of half. The clock is the balance knob.
- Delete `resolve_shoves` and press the key. Read the warning.
- Give the rat king `shoves: true` and stand beside it.

Next: [panels](10-panels.md).
