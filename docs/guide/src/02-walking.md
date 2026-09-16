# Walking

> Run it: `cargo run -p tutorial --bin step02_walking`
>
> Source: [`step02_walking.rs`](https://github.com/rudehn/rl-engine/blob/main/examples/tutorial/src/bin/step02_walking.rs)

## An intent, not a move

<!-- include: ../../../examples/tutorial/src/bin/step02_walking.rs:input -->
```rust,no_run
/// The player, but only while it is holding the turn.
type PlayerTurn<'w, 's> = Query<'w, 's, Entity, (With<Player>, With<MyTurn>)>;

/// Keys to intents. Writing an intent is the whole of asking to act: the
/// engine claims the turn, charges it, and refuses what cannot be done.
fn player_input(
    keys: Res<ButtonInput<KeyCode>>,
    dirs: Res<DirectionKeys>,
    player: PlayerTurn,
    mut steps: MessageWriter<Intent<Step>>,
    mut waits: MessageWriter<Intent<Wait>>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::KeyQ) {
        exit.write(AppExit::Success);
        return;
    }
    // No turn in hand means it is somebody else's move; the key is dropped.
    let Ok(entity) = player.single() else { return };
    if let Some(dir) = dirs.just_pressed(&keys) {
        steps.write(Intent::new(entity, Step(dir)));
    } else if keys.just_pressed(KeyCode::Period) || keys.just_pressed(KeyCode::Numpad5) {
        waits.write(Intent::new(entity, Wait));
    }
}
```

Input never moves anybody.
It writes an `Intent<Step>` and stops.
The engine decides whether the actor may act, whether the move is legal, what it costs and what to do when it is not.

`With<MyTurn>` makes the query empty unless the player is holding a turn, so a key pressed while rats are still moving is dropped.

`DirectionKeys` is the engine's binding of the arrows, `hjklyubn` and the numpad to the eight directions, and `just_pressed` answers which one was struck.
It is a resource, so a game that wants other keys replaces it and writes no match statement of its own.

## Where the system runs

```rust,no_run
    .add_systems(Update, player_input.in_set(EngineSet::Input));
```

`EngineSet` is the frame, in order:

```text
one frame
  Stream    the world streams in and out around the player
  Input     your keys become intents
  Turns     the Turn schedule, run over and over:
              Schedule -> Decide -> Resolve -> Sweep -> React -> Cleanup
            until the player holds a turn, or nothing is left to move
  Light     what every source reaches
  Fov       what every actor can see
  Present   the frame is drawn
```

Inside `EngineSet::Turns` the engine runs the `Turn` schedule repeatedly, until the player holds a turn again or nothing is left to move.
A frame next to a dozen rats runs a dozen passes.
`keys.just_pressed` would be true on every one of them, which is why input lives outside the turn schedule: one key press, one intent.

## One pass

| Stage | What happens |
|---|---|
| `Schedule` | The clock advances and one actor is dealt `MyTurn` |
| `Decide` | Minds choose for everyone who is not the player |
| `Resolve` | Intents become changes to the world |
| `Sweep` | Anything nobody resolved is refused, loudly |
| `React` | The game answers what the turn caused |
| `Cleanup` | The actor is charged and requeued |

One actor holds a turn at a time and is out of the queue while it does.
`Cleanup` puts it back at `now + cost`.

## Time is an integer

Costs are hundredths of a normal step, and `BASE_ACTION_COST` is 100.
`Speed(200)` is twice as fast and the scheduler scales cost by it.
No floats in the clock, so a seed replays.

A diagonal costs 1.414 times a straight step, and a tile registered with `move_cost(200)` costs twice as much.
Neither is something you write.

## Refusals

Walk into a wall and nothing happens: no time passes, you keep the turn.
The step resolver reports it with `Resolution::failed`, which writes an `ActionRefused` for the player and nobody else.
A monster handed a free retry would spin forever, so a blocked monster is charged for a wait instead, and a resolver you write gets the same rule by calling the same method.

## Try it

- Give the player `Speed(200)` and watch the turn counter climb half as fast.
- Scatter a `move_cost(250)` tile and walk through it.
- Delete the `With<MyTurn>` filter and hold a direction key. The resolver still claims the actor, so one turn is still one action.

Next: [what the player knows](03-what-the-player-knows.md).
