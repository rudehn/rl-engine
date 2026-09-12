# An action of your own

> Run it: `cargo run -p tutorial --bin step09_shove`
> Source: [`step09_shove.rs`](https://github.com/rudehn/rl-engine/blob/main/examples/tutorial/src/bin/step09_shove.rs)

Everything so far used actions the engine ships.
This one it has never heard of: shove a rat back a cell, for half a turn.

## An action is a type

```rust,no_run
{{#include ../../../examples/tutorial/src/bin/step09_shove.rs:action}}
```

`Action` is an empty marker trait and `Intent<Shove>` is its own message type.
There is no `enum Action` for it to be added to.

```rust,no_run
    .add_action::<Shove>()
    .add_message::<Shoved>()
    .add_systems(Turn, resolve_shoves.in_set(ResolveSet::Act))
```

`add_action` registers the message and installs a sweeper in `TurnSet::Sweep`, after everything that might have resolved the intent.
Forget the resolver and you get a warning naming the type, not a frozen game with the player holding a turn nothing will spend.

## What a resolver owes

```rust,no_run
{{#include ../../../examples/tutorial/src/bin/step09_shove.rs:resolver}}
```

**Check the actor holds the turn.** `With<MyTurn>` on the query. An intent for somebody not acting is stale.

**Claim the actor.** `acting.claim_action` returns false if something already spent this turn. One turn is one action, across resolvers that have never heard of each other.

**Report a cost or a refusal.** `ActionDone` charges and requeues. `ActionRefused` costs nothing and leaves the turn in hand. Only ever refuse the player; a monster with a free retry loops forever, which is why the engine charges a stranded one for a wait in `Cleanup`.

**Keep the indexes straight.** Moving something means `occupancy.relocate` as well as writing `Position`, and marking a moved viewshed dirty.

`SHOVE_COST` is `BASE_ACTION_COST / 2`: fifty against a step's hundred, both scaled by `Speed`.

Reporting through a `Shoved` message rather than logging from inside the resolver keeps narration out of it and lets anything else react later.

## Deciding for monsters

Warren's shove is player-only, so it is written in `EngineSet::Input`.

For monsters, decide in `TurnSet::Decide`, which has two stages: `DecideSet::Minds` is where the engine's brains run, `DecideSet::Game` is after them.
Claim with `acting.claim_decision` before writing the intent, so nothing chooses twice.

## Try it

- Make a shove fail against a heavier monster and refuse it.
- Charge a full turn instead of half. The clock is the balance knob.
- Delete `resolve_shoves` and press the key. Read the warning.
