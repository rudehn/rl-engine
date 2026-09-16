# Blows

> Run it: `cargo run -p tutorial --bin step05_combat`
>
> Source: [`step05_combat.rs`](https://github.com/rudehn/rl-engine/blob/main/examples/tutorial/src/bin/step05_combat.rs)

![Two rats in the lit room closing on the player, the log counting their bites in red, HP down to 20](images/05-blows.png)

## One key, three actions

<!-- include: ../../../examples/tutorial/src/bin/step05_combat.rs:input -->
```rust,no_run
/// The player, but only while it is holding the turn.
type PlayerTurn<'w, 's> = Query<'w, 's, (Entity, &'static Position), (With<Player>, With<MyTurn>)>;

/// Keys to intents. Writing an intent is the whole of asking to act: the
/// engine claims the turn, charges it, and refuses what cannot be done.
///
/// The walk keys write a [`Bump`], which the engine resolves to a step, a
/// blow at a foe, or opening a door, whichever is in the way.
fn player_input(
    keys: Res<ButtonInput<KeyCode>>,
    dirs: Res<DirectionKeys>,
    player: PlayerTurn,
    mut bumps: MessageWriter<Intent<Bump>>,
    mut waits: MessageWriter<Intent<Wait>>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::KeyQ) {
        exit.write(AppExit::Success);
        return;
    }
    // No turn in hand means it is somebody else's move; the key is dropped.
    let Ok((entity, _)) = player.single() else { return };
    if let Some(dir) = dirs.just_pressed(&keys) {
        bumps.write(Intent::new(entity, Bump(dir)));
    } else if keys.just_pressed(KeyCode::Period) || keys.just_pressed(KeyCode::Numpad5) {
        waits.write(Intent::new(entity, Wait));
    }
}
```

The walk keys no longer write a `Step`.
They write a `Bump`, and the engine decides what a bump comes to: a step onto open ground, a blow at a foe standing there, or the door in the way opened.
That is an alternate action, an intent that resolves to another, and `Bump` is the engine's own example of one.

It works in `ResolveSet::Redirect`, the stage before any resolver claims the turn.
The bump is read, the intent it stands for is written, and the resolver that owns that intent spends the turn as if the player had written it.
A bump into someone who is not a foe is refused for free, the way a step into a wall is, and reported as `Bumped`.
A game with a companion inserts `BumpRules::new().swap_allies()` and a bump into an ally changes places with them instead.

Bump to attack is still a decision: a game whose walk key should only ever step writes `Step`, as Warren did in [chapter 2](02-walking.md).
An alternate of your own follows the same shape: read your intent in `Redirect`, write the engine's, claim nothing.

## What a hit passes through

```rust
    commands.insert_resource(DamageStages(vec![Box::new(SubtractArmor)]));
```

A `Hit` carries the kind, the dice and who threw it.
Each stage in turn may change the number before it lands.

Warren has one stage.
Resistances by damage kind, a shield that eats the first hit each turn, a critical rule reading the attacker's stats: each is another entry in that list, in the order you put them.

```rust
            (Health::full(24), Armor(1), MeleeAttack { kind: kinds.expect("kick"), dice: DiceRoll::new(1, 6) }),
```

`DiceRoll::new(1, 6)` is `1d6`, and it parses from `"1d6+2"` when it comes out of a content file.

## Two events

- `DamageEvent` is a hit on its way in, before the stages run. Write one to hurt somebody: poison, a fall, a trap.
- `DamageDealt` is what landed, after the stages, with the final number.

`DeathEvent` carries `was_player` and who gets the credit.
The dead linger until the end of the frame, so anything that wanted to react to a death still finds the entity.

## The narrator

Nothing in Warren turns those events into words.
The engine does:

<!-- include: ../../../examples/tutorial/src/bin/step05_combat.rs:narrator -->
```rust,no_run
        // The engine narrates blows, deaths and pickups into the log, naming
        // things in their own colours. Warren changes one phrase: what a rat
        // does to you is a bite.
        .add_plugins(NarratorPlugin::default().phrase(Phrase::HitsYou, "{Who} bites you for {n}.", Tones::BAD))
        // Escape opens the menu. The run's end opens it by itself, under these
        // words, offering a new run or the same seed again; the morgue writes
        // the run down beside the executable.
        .add_plugins(GameMenuPanel::new(Rect::new(COLS / 2 - 20, 8, 40, 12)).died("The warren keeps you."))
        .insert_resource(Morgue::platform_default("warren", "Warren"))
```

`NarratorPlugin` reads every event the engine raises, inside the turn, one pass at a time, so a frame in which three rats act reads in the order they acted.
Each event becomes a `Phrase`, split by who did what to whom: `YouHit`, `HitsYou`, `OthersFight`, `YouKill`, `Dies`, `YouPickUp`, and so on.
The `Phrasebook` holds one template and one tone per phrase, and a game changes any of them.
Warren changes one: a rat bites.

A name in a template is drawn in the colour of the thing it names, so the rat in `The rat bites you for 2.` is the rat's own brown, lifted just enough to read against the log.
Your own blows are in the `hit` tone and your kills in the `kill` tone, both brighter than text, so what you did stands out from what was done to you.

The rats need a `Name` for any of this to say more than `something`.
That is the same `Name` the rail will use in [chapter 10](10-panels.md).

What the engine cannot know stays yours.
Warren will narrate what eating a crust means in the next chapter, by pushing its own line to the log, as it always could.

## Ending the run

When the player dies the engine writes `RunOver`, leaves `EngineState::Playing` for `EngineState::Over`, and the `GameMenuPanel` opens by itself under the words Warren gave it.
The world stays on screen and nothing in it moves.
Escape opens the same menu during a run, with a way back in.

Choosing a new run writes `Restart`.
The engine tears the run down, everything that stood or lay on a map with it, and runs your start again in `NewRun`, on a fresh seed or the same one.
That is why `start` lives in `NewRun` rather than `Startup`: the same system starts the first run and the tenth.

The `Morgue` writes each run down as a text file when it ends: the seed, the turn, how it ended, what you were, and the last lines of the log.
A game adds sections of its own by pushing them when it reads `RunOver`.

## Try it

- Add a stage that halves every hit and read the log to confirm the order.
- Give the rats `Resists` and a damage kind they shrug off.
- Write a `DamageEvent` from a key press. The pipeline does not care where a hit came from.
- Change `Phrase::YouKill` to something of your own and give it the `notice` tone.
- Take the rats' `Name` off and watch the log say `something`.

Next: [things to pick up](06-items.md).
