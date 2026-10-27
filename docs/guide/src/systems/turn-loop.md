<!-- documents:
     plugins: CorePlugin
     files: crates/atglyph-core/src/turn.rs
            crates/atglyph-bevy/src/turn.rs
            crates/atglyph-bevy/src/cue.rs
            crates/atglyph-bevy/src/plugin.rs
            crates/atglyph-bevy/src/components.rs
            crates/atglyph-bevy/src/world.rs
            crates/atglyph-bevy/src/title.rs
            crates/atglyph-ui/src/player_keys.rs
     fingerprint: 61b5044f -->

# The turn loop

A turn is dealt to one actor at a time, from a queue keyed by an integer clock.
One pass of the `Turn` schedule deals that turn, lets a mind decide what to do with it, resolves the decision, and puts the actor back at the reading it is next due at.
The loop runs the pass again and again inside one frame until the player holds a turn or nothing moved, to a ceiling of 512 passes, so a player's step costs one frame however many monsters are awake between.
A run nobody plays has no player to stop for, and is dealt at a `Pace` instead: so much game time for each second it is watched.
What a turn caused is answered inside the same pass, and what is worth watching can stop the loop until it has been seen.

## Turning it on

`CorePlugin` is the loop, and it is the one plugin every game adds.
It creates the `Turn` schedule and the run's own, `PrepareRun`, `NewRun`, `ContinueSaved`, `RunResumed` and `EndRun`, chains `EngineSet` across `Update` and `TurnSet` across a pass, and runs the passes in `EngineSet::Turns`.
It begins the first run at startup unless a title holds it, through the same `begin_run` a title and a restart use, and play begins there once the run stands in a world, so no game's start sets the engine playing.
It registers the actions that need nothing else: `Step`, `Wait`, `Bump`, a swap with an ally, `GoThrough`, `Open` and `Close`.
Statuses and work are part of it rather than plugins of their own, since a game that registers no status and declares no kind of work has nothing to tick or do.
It sets `Turn` to run on one thread, because a pass is dozens of small systems dealt once per actor turn rather than once per frame, and the multi-threaded executor's per-system handoff cost more than the systems it was handing off.
It declares `needs::<WorldMap>`, so a game that sets play going with no map is told so, by name, rather than by an empty screen; a run whose start builds no map does not begin play, and is not yet reported.
A game's own action is registered with `app.add_action::<A>()`, which adds `Intent<A>` as a message and one sweeper that refuses an intent no resolver claimed.
Cues and the wait for them are in `CorePlugin` too, and stay inert until a plugin that draws them calls `CueWait::watch`.
A landing is not: `AbilitiesPlugin`, `CombatPlugin`, `ThrowingPlugin` and `EffectsPlugin`, for a thing's aimed use, each register their own, so an `Airborne<L>` arrives with the subsystem that flies something rather than with the loop.
`TurnRules` is optional data: absent, or naming no stat, every actor is as fast as its own `Speed`.

## The model

`Turns` wraps a `TurnQueue<Entity>` whose clock is a `u32` in hundredths of a step, with `BASE_ACTION_COST` at 100.
Entries come out earliest first, ties settling by insertion order, and `Entry` is ordered on those two fields alone so no entity's bit pattern can decide who goes first.
`scaled_cost(base, speed_percent)` divides in integers, `reschedule_at` saturates rather than wraps, and `set_now` panics on a clock asked to run backwards.
An actor holding `MyTurn` is out of the queue until something reports `ActionDone` or `ActionRefused` for it.
The pass deals to the first live actor that is due and standing where play is; one on another map or outside the loaded window is requeued a full step without acting, and the clock advances at most once a pass, writing `TurnEnd` when it crosses into a new whole turn.
A freshly spawned actor is held out of the queue until it first stands where play is, and the player is admitted ahead of the rest so a first turn does not depend on archetype order.
A decision is an `Intent<A>` for an `A: Action`, written by the game in `EngineSet::Input` for the player and by minds in `TurnSet::Decide` for everyone else.
A resolver takes a `Resolution`: `claim` gives it the turn once, `done(actor, cost)` spends it, and `failed(actor, cost)` refuses for the player and charges anyone else, because a monster handed a free retry asks again forever.
The cleanup requeues at `scaled_cost` of what was owed against the actor's speed, requeues one actor once per pass, and charges a wait to any non-player left holding a turn nobody used.
That speed is what `Speeds::of` reads, the one reader, so nothing that shows a speed disagrees with the queue about who is hasted: the actor's own `Speed`, a hundred when it has none, with the value of the stat `TurnRules::new().speed_stat(stat)` names added as a percentage of it, so a hundred on the stat is twice as fast and minus fifty half as fast.
The stat wants a base of nought, so an actor with nothing on it is as fast as it was spawned, and nothing takes a speed below one, since an actor that is not to act at all is held by a status rather than slowed to nothing.
`Cued` is what a resolver writes when a turn did something worth seeing: a `Cue::Flight` between two `Anchor`s or a `Cue::Burst` on several, all at once or going out `from` one of them, where an anchor that follows an entity goes where the entity goes.
`Pace` is how fast the turns run while nobody is waited on: `Pace::unpaced()`, the default, `Pace::per_second(hundredths)` of game time for each second of the wall clock, or `Pace::stopped()`.
A frame's share is earned once a frame, capped at a quarter of a second, and spent as the clock advances, so a pass that moves no time is free and one that jumps the clock is paid off over the frames after.
Once it is gone nobody is dealt a turn, and the pass that finds it so changes nothing and ends the loop: a turn already dealt is always resolved, a player's intent is answered in its own frame at any pace, and a warp asked for while the turns are stopped is still answered.
A `Player` that is no `Actor` is an onlooker: never admitted to the queue, so never dealt a turn, while the map being read, the view and everything that asks what the player sees still follow it.
`CueWait` is the brake, and it takes only while something watches: the loop waits after any pass that cued, and runs no pass until the watcher lets go; `wait` and `is_waiting` are how a watcher holds it and asks.
`Airborne<L>` is what a subsystem has in the air, emptied when a run ends; a landing is handed straight back when nothing watches, so a headless game lands everything at once.
The move resolver refuses a `Step` into an occupied or unwalkable cell, and a diagonal one besides unless the one corner rule answers yes: a diagonal may squeeze only between two cells the map lets a diagonal past, never a wall's corner or a closed door's, which is the same question `CostSource::lets_diagonal_past` puts to a search, so a mind's own path and the move it resolves never disagree and a mover is never refused the same step twice.

## Using it

The tutorial asks to act through `PlayerKeysPlugin`, an instance of a key becoming an `Intent`, and the engine does the rest.

<!-- include: ../../../../examples/tutorial/src/bin/step01_walking.rs:input -->
```rust,no_run
/// Reads Warren's own keys once a frame.
fn player_input(keys: ControlInput, binds: Res<Binds>, mut exit: MessageWriter<AppExit>) {
    if keys.just_pressed(binds.quit) {
        exit.write(AppExit::Success);
    }
}
```

## The line

Costs and clocks are integers in hundredths of a step, the same unit everywhere, so an `f32` never gets the chance to lose a low bit and reorder two actors who were tied.
What an action costs is the game's number; what happens to the clock once that number is known is the engine's.
Anything that reacts to what a turn caused belongs in `TurnSet::React`, inside the pass and before the next actor acts, not in the drawing phase: a drink that heals, a bite that poisons, the loot the dead leave.
A system that scans the world every frame is not a reaction and belongs in `PresentSet::Narrate`, because `React` runs once a pass and one frame may hold hundreds.
Systems in a pass run several times a frame, so a system that must run once a frame says so by being in `EngineSet::Input` or a `PresentSet` layer instead.
Which executor a pass runs on is the engine's default and not its decision: a game whose own system in the pass is heavy enough to be worth a thread puts the multi-threaded one back with `app.edit_schedule(Turn, ..)`.
The pace is the game's number and its alone: it decides how many passes a frame runs and never what a pass does, so one seed plays one run at any pace, stopped and started or not.
The engine owns who is dealt a turn, when, and what is done with an action that nobody resolved; the game owns what actions exist beyond the few above, what each costs, and who is allowed to try it.
A game orders its systems into `TurnSet` and `ResolveSet`, never after another crate's system function.

## Where it lives

`atglyph-core` is tier 0 and has no Bevy in it: `TurnQueue` is generic over the actor id, so its whole ordering contract is tested with ids made out of thin air, and `scaled_cost` and `reschedule_at` are `const fn` tested by arithmetic alone.
That split is why the ordering rules can be proved without an `App`, and why nothing in them can quietly come to depend on an entity's index.
`atglyph-bevy` is tier 2 and owns the loop over it: `turn.rs` has `Turns`, `Occupancy`, `Action`, `Intent`, `Resolution` and the systems of a pass, and `cue.rs` has `Cued`, `CueWait` and `Airborne`.
`plugin.rs` has the sets, the schedules, the loop of passes and `CorePlugin` itself, in one file because the order of a frame is one decision and not six.
