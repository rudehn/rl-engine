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

`Resolution` is the engine's side of every resolver, its own and yours.

**Claim the turn.** `resolution.claim` returns false if the actor holds no turn, so the intent is stale, or if something already spent this one. One turn is one action, across resolvers that have never heard of each other.

**Say how it went.** `resolution.done` charges what the action cost and requeues the actor. `resolution.failed` is for one that could not be done: the player keeps the turn at no cost, and anyone else is charged, because a monster handed a free retry asks again forever. That rule lives in `Resolution`, so no resolver has to remember it.

**Keep the indexes straight.** Moving something means `occupancy.relocate` as well as writing `Position`, and marking a moved viewshed dirty.

`SHOVE_COST` is `BASE_ACTION_COST / 2`: fifty against a step's hundred, both scaled by `Speed`.

Reporting through a `Shoved` message rather than logging from inside the resolver keeps narration out of it and lets anything else react later.

## Deciding for monsters

Warren's shove is player-only, so it is written in `EngineSet::Input`.

For monsters, decide in `TurnSet::Decide`: `DecideSet::Minds` is where the engine's brains run, and `DecideSet::Game` is after them.
Claim with `acting.claim_decision` before writing the intent, so nothing chooses twice.

A tactic of your own sits in the brain beside the engine's, and what it needs to know that the engine does not, a scent or a post, you push onto the snapshot in `PerceiveSet::Annotate` with `add_sense` and read back in the tactic with `sense::<T>()`.

## Try it

- Make a shove fail against a heavier monster with `resolution.failed`.
- Charge a full turn instead of half. The clock is the balance knob.
- Delete `resolve_shoves` and press the key. Read the warning.
