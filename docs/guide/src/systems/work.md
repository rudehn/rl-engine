<!-- documents:
     plugins: none
     files: crates/atglyph-bevy/src/work.rs
            crates/atglyph-bevy/src/plugin.rs
            crates/atglyph-rules/src/work.rs
            crates/atglyph-rules/src/ai/brain.rs
            crates/atglyph-bevy/src/minds.rs
            crates/atglyph-bevy/src/status.rs
            crates/atglyph-ui/src/view/mod.rs
            crates/atglyph-ui/src/panel/nearby.rs
            crates/atglyph-ui/src/panel/inspect.rs
            crates/atglyph-save/src/run.rs
            crates/atglyph-ui/src/controls.rs
     fingerprint: dbbb9612 -->

# Work

An actor doing one thing across many turns: rebuilding a wreck, charging a device, digging through a wall.
Each of those turns is an ordinary turn, spent as a wait costs; the work breaks off when the worker is hurt, dies, or can no longer reach what it is working on; and while it lasts the actor's row says what it is doing, so a player never takes a busy actor for a stuck one.
The engine owns the loop of it, and the game owns what starting it and finishing it mean.

## Turning it on

There is no plugin to add: work is part of `CorePlugin`, which [The turn loop](turn-loop.md) covers, since a game that declares no kind of work has none to do.
It claims a worker's decision in `DecideSet::Sense`, begins and spends work in `ResolveSet::Act`, breaks it off in `ReactSet::Settle`, spends the player's turns of it after every resolver and before the sweep, and registers `BeginWork`, `WorkBegan`, `WorkDone` and `WorkBroken`.
`UiPlugin` adds the key that leaves the player's work off: any key but a modifier alone, or a screen opening, and the controls screen lists it in a game that declares a kind of work.
Every kind of work is declared with `app.add_work("word")`, where the word is what a panel shows while an actor does it, and looked up again with `WorkKinds::get`.
Harm and death are read from combat, and a game without combat has empty queues and nothing ever hurt.

## The model

`Work` is in `atglyph-rules`: a `kind`, an optional `target`, `done` and `needed`, counted in the worker's own turns and never in clock time, so a worker twice as fast finishes in half the clock without the model knowing speed exists.
`Work::new(kind, needed).on(target)` builds one, zero turns taken as one; `advance` counts a turn and answers `Progress::Left(n)` or `Progress::Finished`.
`in_reach` is the one test of whether work can still be done: the worker within `REACH`, one cell Chebyshev, of its target, asked of where both are now.
A mind begins work by deciding `Decision::Work(work)`, which becomes a `BeginWork` intent; anything else begins it through `Works::begin`.
Either way the turn it is begun on is its first, `WorkBegan` is sent, and the actor carries `Working` until it is done; work of one turn is done at once and leaves nothing working.
On every turn after, the engine claims the actor's decision before any mind is asked, which keeps its perceive stage shut, and spends the turn on the work as a wait costs.
A status that holds its holder's turns, or sleeps it, takes a worker's as well, spending each before its `Toil` is resolved, so the work waits out the hold with nothing done and nothing broken.
On the last turn `Working` comes off and `WorkDone { actor, kind, target }` is sent.
Work is broken off with `WorkBroken { actor, kind, target, done, reason }`, the gravest `BreakReason` first: `Died` on a `DeathEvent`, `Hurt` on any `DamageDealt` for the worker that dealt anything, even a hit healed in the same pass, `DoneByAnother` when another worker's `WorkDone` names the same target, and `OutOfReach` when the target is gone or out of reach.
`Works::leave_off` breaks it as `LeftOff`, the actor leaving it of its own accord, which is what a key pressed while the player works comes to; `Works::stop`, or a `StopWork` message answered at the head of the next pass, breaks it as `Stopped`, a rule of the game's own.
Everything done is lost with it; `WorkBroken::done` says how far it got.
The player works by the same `Working`, begun by `Works::begin` from whatever the player did, and its turns are spent for it with no key pressed, at most one each `WorkPace` of the wall clock, twenty a second by default and `WorkPace::every_frame()` for a test, so the world is drawn moving between them.
Spending one of those turns on anything else leaves the work off as `LeftOff`, the player's own act standing, while a direction key toward the work's own target presses on with it; a foe coming into the player's sight breaks it as `FoeSighted` before another turn is spent, and the player keeps the turn to answer it.
`Row::work` carries a `WorkRow`, the word, the target's name and the turns left, filled by the nearby and inspect collectors through `Workings`.
The nearby rail writes the word where it would write the alert, and inspect adds a line from `InspectPanel::working`'s templates.
`EntityState::working` keeps the work in a save, the kind by its word and the target by save id, and drops it on load when the target was not saved.

## Using it

A game's tactic begins work once its actor is within reach of what it wants to work on, and walks there otherwise.

<!-- include: ../../../../examples/foundry/src/droids/repair.rs:tactic -->
```rust,no_run
/// Rebuild a wreck: set to work on it if it is within reach, and walk
/// toward the nearest otherwise.
pub struct RepairWrecks;

impl Tactic<Entity> for RepairWrecks {
    fn name(&self) -> &'static str {
        "repair_wrecks"
    }

    fn evaluate(&self, ctx: &mut TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
        let me = ctx.snapshot.me.pos;
        let wrecks = ctx.snapshot.sense::<Wrecks>()?.clone();
        if let Some((wreck, _, turns)) = wrecks.found.iter().find(|(_, at, _)| in_reach(me, *at)) {
            return Some(Decision::Work(Work::new(wrecks.kind, *turns).on(*wreck)));
        }
        let cells: Vec<Point> = wrecks.found.iter().map(|(_, at, _)| *at).collect();
        ctx.step_toward(&cells).map(Decision::Step)
    }
}
```

What finishing means is the game's, answered from `WorkDone` in `TurnSet::React`.

<!-- include: ../../../../examples/foundry/src/droids/repair.rs:rebuild -->
```rust,no_run
/// A finished repair stands the wreck up as the droid it was, at half its
/// health, and says so when the commando can see it happen.
pub fn rebuild_wrecks(
    mut commands: Commands,
    mut done: MessageReader<WorkDone>,
    kinds: Res<WorkKinds>,
    roster: Res<Roster>,
    wrecks: Query<(&Kind, &Position), With<Remains>>,
    eyes: Query<&Viewshed, With<Player>>,
    mut tell: MessageWriter<Tell>,
) {
    let Some(repairing) = kinds.get(REPAIRING) else { return };
    for finished in done.read().filter(|d| d.kind == repairing) {
        let Some(wreck) = finished.target else { continue };
        let Ok((kind, pos)) = wrecks.get(wreck) else { continue };
        let def = roster.defs.get(kind.0);
        commands.revive(wreck, (def.hp + 1) / 2);
        if eyes.iter().any(|v| v.can_see(pos.0)) {
            tell.write(Tell::new(format!("The {} whirs back to life.", def.name), Tones::NOTICE));
        }
    }
}
```

## The line

The engine decides how work runs: that each turn of it is a turn, what it costs, that the brain is not asked again while it lasts, and what breaks it.
Breaking comes from the one place each cause lands rather than from watching for change: harm from the damage pipeline, death from the death message, and reach from where the worker and its target are now, so every way of moving either is covered without any of them knowing work exists.
The game decides everything work means: what kinds there are and what each is called, when an actor begins one, how long it takes, which is a number its own code computes when it begins, and what finishing does.
A rule of the game's own that should stop work calls `Works::stop` or writes a `StopWork`, so a reader of `WorkBroken` tells it from the worker walking away.
The pace of the player's work is the game's number and decides only when a turn is spent, never what it does, so a replay keyed by the turn clock plays the same at any pace.
An actor that shrugs off harm and keeps working is not here either, and when it comes it is a marker a game's own boss would require, never an engine word for a boss.

## Where it lives

`atglyph-rules` holds `Work`, `Progress`, `in_reach` and `Decision::Work`, with no Bevy in them, so how work counts and what it reaches are tested over a range of turns without an `App`.
`atglyph-bevy` holds the loop: the intents, the claim on the decision, the resolvers, the player's paced turns and the one system that breaks work, which is what the two sides of a pass can only be asked about with an `App` running turns.
`atglyph-ui` reads `Working` to fill a row and reads the key that leaves it off, and `atglyph-save` keeps it, each without `atglyph-bevy` knowing either exists.
