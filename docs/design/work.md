# Work

Status: designed 2026-09-23, not built.
The reasoning is here; `docs/OVERVIEW.md` will list what exists once it does.
Bringing remains back to life is part of the same slice of work and is written up in `docs/design/remains.md` §9, since it is a change to that subsystem rather than a new one.

## 0. Summary

Everything an actor does today takes one turn, however long it costs.
A prop offer like `(verb: "charge", time: 300)` lands its effect on the turn it is taken and then leaves the actor out of the queue for three turns.
That is right for a lever and wrong for anything that should take a while to *happen*.

Foundry wants a repair drone that walks to a droid wreck and spends ten turns rebuilding it.
Built as one action with a large cost, the wreck would stand up on the first turn, the drone would stand idle for nine, nothing could stop it, and the player would see a drone that looks stuck.

Work is an actor doing one thing across many turns: it spends each of them as an ordinary turn, can be broken off, and says what it is doing while it does it.
The engine owns the loop of it, and the game owns what starting it and finishing it mean.

## 1. The model

`Work` lives in `rl-rules`, so it is tested without an `App`:

```rust
pub struct Work<A> {
    pub kind: WorkKindId,
    pub target: Option<A>,
    pub done: u16,
    pub needed: u16,
}
```

`advance()` counts one turn and answers either the turns left or finished.
Progress counts turns worked, never clock time, so a hasted drone finishes in half the clock time without the model knowing speed exists.

`WorkKindId` is interned from a word, through `app.add_work("repairing")`, the way verbs and sounds are.
The word is the state's name as a panel shows it, and it is the only word: nothing ever shows a work's base verb, so none is asked for.
This is the shape `AlertWords` already has, where the engine holds a state and the game names it in its own words.
A save stores the word rather than the id, as statuses are stored by name, so registration order never matters to a save.

There is no registry of work definitions and no RON schema.
Whether to start work is a tactic, and how long it takes depends on what only code can see, a stronger droid's wreck taking longer to rebuild, so whoever starts work builds it in code at that moment:

```rust
Decision::Work(Work::new(REPAIRING, 6 + 2 * tier).on(wreck))
```

Rejected: a `works.ron` of kinds with turns and break rules.
It put numbers in a file that the code starting the work always knew better, and it grew per-kind knobs for interruption that §4 shows are not needed.

## 2. Starting

A mind starts work by returning `Decision::Work(work)`, a new engine decision beside `Wait` and `PickUp`.
The engine resolves it itself: it claims the turn, puts `Working(work)` on the actor, counts that turn as the first of `needed`, and writes `WorkBegan`.

A game therefore needs no action of its own to start work.
Foundry's drone is one tactic, `RepairWreck`, which finds a wreck of its own side in `Snapshot::remains`, walks there with `step_toward`, and returns the decision once it is within reach.

For a start that does not come from a mind, the `Works` system param offers `works.begin(actor, work)`.
Nothing in this slice uses it; it is there so that the player's long interactions, when they come, start work the same way.

## 3. Continuing

At the head of `TurnSet::Decide`, the engine claims the decision of any `Working` actor that holds the turn and writes `Intent<Toil>`.
A claimed decision never opens the perceive stage, which is a path `decide_minds` already has for a game that decides for its own actor, so a busy drone does not think and costs almost nothing per turn.

`Toil` is resolved in `ResolveSet::Act`.
It costs what a wait costs, scaled by the actor's `Speed`, and advances the work.
On the last turn it takes `Working` off and writes `WorkDone { actor, kind, target }`.

The brain is not asked whether to carry on.
Rejected: a `KeepWorking` tactic that continues the work from inside the brain, so that any tactic above it could take over.
What it buys is a worker distracted by something other than harm, which nothing here asks for.
What it costs is a new "was hurt" signal in the snapshot just to rebuild the one interruption that is asked for, and a full perceive stage on every busy turn.
If a game wants distraction later, the upgrade adds to this design rather than replacing it: let a `Working` actor's brain run with `Snapshot::work` filled in, and break the work as `Distracted` when the brain returns anything but carrying on.

## 4. Breaking

Work breaks for one of five reasons, and each comes from a single place rather than from watching for change.

| Reason | Where it comes from |
|---|---|
| `Hurt` | A `DamageDealt` for the worker with `dealt > 0` |
| `Died` | A `DeathEvent` for the worker |
| `OutOfReach` | The target is missing, or more than one cell from the worker |
| `DoneByAnother` | The engine's own `WorkDone` for the same target |
| `Stopped` | The game called `works.stop(actor)` |

Each takes `Working` off and writes `WorkBroken { actor, kind, target, done, reason }`.
Progress is lost with it: the next start begins from nothing.
A killing blow is both harm and death, and reads as `Died`.

The checks run in `TurnSet::React`, which the turn loop already runs after the whole resolve chain.
That puts them after the copy remains takes of a dying actor in `ResolveSet::Damage` (`docs/design/remains.md` §9.1), so the copy still holds the work it died doing, and no system names another to get that order.
A game's own `React` system may read a `WorkBroken` in the pass it was written or the next, which is already true of `RemainsLeft`, written in `CleanupSet::Remove` and answered by Foundry in `React`.
Panels draw after the loop, so either way they never show work that has broken.
Rejected: a new `ResolveSet::Work` for the checks, which bought only that same-pass guarantee and added a set every plugin author would have to learn.

**Things that happen to the worker come from the engine's choke points.**
Every source of harm, a blow, a shot, fire, gas, a status tick, becomes a `DamageEvent` and lands in `apply_damage`, which writes `DamageDealt`.
Narration, on-hit reactions and stealth already rely on that message, so a harm that skipped it would already be a bug.
A hit healed in the same pass still breaks the work, because the message is about the hit and not the net.
An ability's own health cost is paid outside the pipeline and is not being hurt, which is right.

**Whether the work can still be done is a condition, not a change.**
`OutOfReach` asks where the worker and the target are now, never how they got there.
A drone shoved off breaks, a wreck carried off by a scrap crab breaks, and a drone swapped to another cell still beside the wreck carries on.
Four systems write `Position` today, a step, a swap, a relocation and a warp, and none of them needs to know work exists; a fifth added later is covered the same way.
The condition keeps no memory, so it is asked on every pass, and a panel never says "repairing" about a drone that can no longer reach its wreck.
Reach is one cell, Chebyshev, which covers standing beside the target and standing on it; work from further away is not asked for.

Rejected: comparing the worker's health and position with what they were when last looked at.
The remembered values had to be saved, a hit healed in the same pass went unseen, a change to maximum health read as harm, and "the worker moved" broke work that a swap left perfectly doable.

**The engine closes out a finished target.**
When `WorkDone` is written, every other worker on the same target breaks as `DoneByAnother`, so two drones on one wreck do not leave one of them rebuilding a droid that is already standing.

**What stays the game's.**
Anything else that should stop work is the game's rule, and `works.stop` is how it says so.
An actor that cannot be interrupted is deferred; when it comes, it is a narrow engine marker such as `Unflinching` that a game's own `Boss` requires, never an engine `Boss`.

## 5. What the player sees

For now, words; the look on the map waits (§8).

The nearby row gains `work: Option<WorkRow { kind, left }>`, filled by the collector from `Working`.
Being worked on is the engine's to know, so it is a field of the view and not a `Facet`: every game with work would push the same one.
The nearby presenter writes the work's word where it would write the alert word, so the drone reads `(repairing)` rather than `(hunting)`.
What it is busy doing is what the player needs, and "hunting" would say it is coming for them when it is not.

Inspect gains a line from a template on the presenter, `"{doing} the {target}, {left} left"`, which a game may replace as it replaces `RemainsNaming`.
The first letter is capitalised and `{left}` goes through `rl_core::noun`, giving "Repairing the line droid remains, 4 turns left" and "1 turn left".
The engine supplies no English a game cannot override.

## 6. Saving

`EntityState` gains `working`: the kind by its word, the target's `SaveId`, `done` and `needed`.
Nothing else is kept, since §4 remembers nothing.
If the target was not saved the work is dropped on load, as a remains' `credit` is.

## 7. Where it lives

- `rl-rules`: `Work`, `advance`, `WorkKindId`, `Decision::Work`, and the reach condition as a plain function over two points.
  All of it is tested without an `App`, over a range of `needed` and speeds.
- `rl-bevy`: `WorkPlugin`, opt-in, with `add_work`, the `Working` component, `Toil` and its resolver, the break checks, `WorkBegan`, `WorkDone`, `WorkBroken`, the `Works` param and the save fields.
- `rl-ui`: the row field, the nearby word and the inspect line.

## 8. Order of work

Each step green and committed on its own, paying its documentation in the same commit as `AGENTS.md` asks.

1. Taking a worn item out of a container takes it out of `Equipped` as well as `Inventory`; today a looted body goes on listing armor that is in the player's bag.
   A bug on its own, reproduced first, and what revival's rule for items (`docs/design/remains.md` §9.3) rests on.
2. Revival, `docs/design/remains.md` §9.
3. `WorkPlugin` and its display, with a guide page `docs/guide/src/systems/work.md` and the overview, changelog and README feature line.
4. Foundry's repair drone: `RepairWreck`, `add_work("repairing")`, and a `WorkDone` answer in `TurnSet::React` that revives the wreck.

## 9. Tests

- A worker finishes in exactly `needed` of its own turns, whatever its speed, and the clock it took scales with speed.
- A worker that is hurt, killed, shoved out of reach, or whose target is carried off or despawned breaks with that reason, and one swapped to a cell still in reach does not.
- A hit fully healed in the same pass still breaks the work.
- Two workers on one target: the first to finish breaks the other as `DoneByAnother`.
- A working actor's perceive stage never opens.
- Work saved is work continued, and work on an unsaved target is dropped on load.
- The row and the inspect line show the word and the turns left, and nothing once the work breaks.

## 10. What waits

- **Ability wind-ups and bosses that cannot be interrupted.** A wind-up is work whose finish is an ability use, with `windup: 3` on the ability as its only content; the wind-up state is one engine kind the game names once.
- **The player's own long actions**, a hack or a search over several turns, started through `works.begin`, continued automatically and stopped by a key or an `ExploreInterrupt`.
- **The look on the map**: a spark on the target each turn of work, which needs a cue that does not hold the turns, and the target's colour drawn toward what it will become as the work progresses.
- **Progress that outlives a break**, a half-rebuilt wreck another drone finishes.
  `WorkBroken` already carries `done`; if a game wants this it keeps the number on the target itself and starts the next worker partway, and a second game doing the same is the signal to lift it.
