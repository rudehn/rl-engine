# Testing without a window

> Run it: `cargo test -p tutorial`
> Source: [`step09_shove.rs`](https://github.com/rudehn/rl-engine/blob/main/examples/tutorial/src/bin/step09_shove.rs)

Warren's tests run the real game: the same `start` system, the same map generation, the same turn loop, with no window.

## A headless app

```rust,no_run
{{#include ../../../examples/tutorial/src/bin/step09_shove.rs:headless}}
```

`headless_app` is `MinimalPlugins`, states and `CorePlugin`.
You add the engine plugins your game uses and your own systems, exactly as `main` does, minus the three that draw.

Two `update` calls start a run: the first runs `Startup` and the warp that builds floor one, the second deals the player its first turn.
After that, one per action.

Writing an intent is how the game plays itself.
The suite runs in about forty milliseconds.

## What goes into one

`rl_engine::rl_bevy::testing` is the engine's own test kit, and a game's tests use the same copy:

- `KeyScriptPlugin` and `press(&mut app, key)` play a key the way a keyboard does, so a test can drive the real input system rather than writing intents by hand. Bevy clears `just_pressed` at the top of every frame, so a key pressed on `ButtonInput` from inside a test is gone before any system sees it; `press` lands it after the clearing.
- `surface(&mut app)` stands an open test world up and hands back ground to start on, for a test that needs a map and not the game's own.
- `two_sides(&mut app)` inserts combat rules for two sides at war, for a test that fights.

The targeting cursor's own tests open it on an ability, step it with the arrow keys and press Enter to fire, which is the whole flow a player uses, with no window.

## Test a property

Where a property exists, assert it over a range of seeds.

```rust,no_run
{{#include ../../../examples/tutorial/src/bin/step09_shove.rs:property}}
```

Forty-eight generated floors, and the assertion is what actually has to be true: you can stand where you arrive, and there is somewhere to go on to.
It does not care where the rooms are, so tuning `min_size` does not break it, and it does catch a cave generator that walls off the stairs on seed 9.

It also never builds an `App`, which is why forty-eight floors cost milliseconds.
Generation is tier 1; test it there.

Where no property exists, use a fingerprint test and say so in the name, so a change reads as a change rather than a failure.

Names read as sentences:

```
every_floor_of_every_seed_has_a_walkable_way_in_and_a_way_on
a_shove_at_nobody_is_refused_costs_nothing_and_leaves_the_turn_in_hand
```

## Testing an action

```rust,no_run
{{#include ../../../examples/tutorial/src/bin/step09_shove.rs:shove_tests}}
```

`room_to_shove` is worth copying as a habit.
On generated maps, a test that assumes there is space to the east fails on seed 12 for reasons unrelated to the code under test.

The second test is the one people leave out.
It asserts the refusal: no time passed, the player still holds the turn.
A wrong refusal crashes nothing; it silently eats a turn or freezes the loop.
