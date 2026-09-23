<!-- documents:
     plugins: CorePlugin
     files: crates/rl-core/src/turn.rs
            crates/rl-bevy/src/turn.rs
            crates/rl-bevy/src/cue.rs
            crates/rl-bevy/src/plugin.rs
            crates/rl-bevy/src/components.rs
     fingerprint: 7c8ef5b7 -->

# The turn loop

A turn is dealt to one actor at a time, from a queue keyed by an integer clock.
One pass of the `Turn` schedule deals that turn, lets a mind decide what to do with it, resolves the decision, and puts the actor back at the reading it is next due at.
The loop runs the pass again and again inside one frame until the player holds a turn or nothing moved, to a ceiling of 512 passes, so a player's step costs one frame however many monsters are awake between.
What a turn caused is answered inside the same pass, and what is worth watching can stop the loop until it has been seen.

## Turning it on

`CorePlugin` is the loop, and it is the one plugin every game adds.
It creates the `Turn`, `NewRun` and `EndRun` schedules, chains `EngineSet` across `Update` and `TurnSet` across a pass, and puts `run_turns` in `EngineSet::Turns`.
It registers the actions that need nothing else: `Step`, `Wait`, `Bump`, `Swap`, `GoThrough`, `Open` and `Close`.
It declares `needs::<WorldMap>`, so a game that never builds a map is told so, by name, the moment play begins rather than by an empty screen.
A game's own action is registered with `app.add_action::<A>()`, which adds `Intent<A>` as a message and one sweeper that refuses an intent no resolver claimed.
Cues and the hold are in `CorePlugin` too, and stay inert until a plugin that draws them calls `TurnHold::watch`.
A landing is not: `add_airborne` is called by `AbilitiesPlugin`, `CombatPlugin` and `ThrowingPlugin`, so an `Airborne<L>` arrives with the subsystem that flies something rather than with the loop.

## The model

`Turns` wraps a `TurnQueue<Entity>` whose clock is a `u32` in hundredths of a step, with `BASE_ACTION_COST` at 100.
Entries come out earliest first, ties settling by insertion order, and `Entry` is ordered on those two fields alone so no entity's bit pattern can decide who goes first.
`scaled_cost(base, speed_percent)` divides in integers, `reschedule_at` saturates rather than wraps, and `set_now` panics on a clock asked to run backwards.
An actor holding `MyTurn` is out of the queue until something reports `ActionDone` or `ActionRefused` for it.
`schedule` deals to the first live actor that is due and standing where play is; one on another map or outside the loaded window is requeued a full step without acting, and the clock advances at most once a pass, writing `TurnEnd` when it crosses into a new whole turn.
`admit_new_actors` holds a freshly spawned actor out of the queue until it first stands where play is, and admits the player ahead of the rest so a first turn does not depend on archetype order.
A decision is an `Intent<A>` for an `A: Action`, written by the game in `EngineSet::Input` for the player and by minds in `TurnSet::Decide` for everyone else.
A resolver takes a `Resolution`: `claim` gives it the turn once, `done(actor, cost)` spends it, and `failed(actor, cost)` refuses for the player and charges anyone else, because a monster handed a free retry asks again forever.
`cleanup_turns` requeues at `scaled_cost` of what was owed against `Speed`, requeues one actor once per pass, and charges a wait to any non-player left holding a turn nobody used.
`Cued` is what a resolver writes when a turn did something worth seeing: a `Cue::Flight` between two `Anchor`s or a `Cue::Burst` on several, where an anchor that follows an entity goes where the entity goes.
`TurnHold` is the brake, and it takes only while something watches: `hold_for_cues` raises it after any pass that cued, and `run_turns` then runs no pass until the watcher releases it.
`Airborne<L>` is what a subsystem has in the air; `launched` hands the landing straight back when nothing watches, so a headless game lands everything at once.

## Using it

The tutorial's input system is the whole of asking to act: a key becomes an `Intent`, and the engine does the rest.

<!-- include: ../../../../examples/tutorial/src/bin/step01_walking.rs:input -->
```rust,no_run
/// The player, but only while it is holding the turn.
type PlayerTurn<'w, 's> = Query<'w, 's, Entity, (With<Player>, With<MyTurn>)>;

/// Keys to intents. Writing an intent is the whole of asking to act: the
/// engine claims the turn, charges it, and refuses what cannot be done.
fn player_input(
    keys: Res<ButtonInput<KeyCode>>,
    dirs: Res<DirectionKeys>,
    repeats: Res<Repeats>,
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
    // A press walks, and a key held down keeps walking: `Repeats` is the
    // engine's hold, already advanced before input is read.
    if let Some(dir) = dirs.just_pressed(&keys).or_else(|| repeats.firing_any().map(|(d, _)| d)) {
        steps.write(Intent::new(entity, Step(dir)));
    } else if keys.just_pressed(KeyCode::Period) || keys.just_pressed(KeyCode::Numpad5) {
        waits.write(Intent::new(entity, Wait));
    }
}
```

## The line

Costs and clocks are integers in hundredths of a step, the same unit everywhere, so an `f32` never gets the chance to lose a low bit and reorder two actors who were tied.
What an action costs is the game's number; what happens to the clock once that number is known is the engine's.
Anything that reacts to what a turn caused belongs in `TurnSet::React`, inside the pass and before the next actor acts, not in the drawing phase: a drink that heals, a bite that poisons, the loot the dead leave.
A system that scans the world every frame is not a reaction and belongs in `PresentSet::Narrate`, because `React` runs once a pass and one frame may hold hundreds.
Systems in a pass run several times a frame, so a system that must run once a frame says so by being in `EngineSet::Input` or a `PresentSet` layer instead.
The engine owns who is dealt a turn, when, and what is done with an action that nobody resolved; the game owns what actions exist beyond the few above, what each costs, and who is allowed to try it.
A game orders its systems into `TurnSet` and `ResolveSet`, never after another crate's system function.

## Where it lives

`rl-core` is tier 0 and has no Bevy in it: `TurnQueue` is generic over the actor id, so its whole ordering contract is tested with ids made out of thin air, and `scaled_cost` and `reschedule_at` are `const fn` tested by arithmetic alone.
That split is why the ordering rules can be proved without an `App`, and why nothing in them can quietly come to depend on an entity's index.
`rl-bevy` is tier 2 and owns the loop over it: `turn.rs` has `Turns`, `Occupancy`, `Action`, `Intent`, `Resolution` and the systems of a pass, and `cue.rs` has `Cued`, `TurnHold` and `Airborne`.
`plugin.rs` has the sets, the schedules, `run_turns` and `CorePlugin` itself, in one file because the order of a frame is one decision and not six.
