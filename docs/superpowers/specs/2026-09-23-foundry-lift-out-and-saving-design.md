# Foundry: the lift out, and saving a run

Status: design, agreed in conversation on 2026-09-23 against `main` at `0b57f99`.
Nothing here is built yet.

## 1. What this is for

Foundry has no ending but death.
Its mission already says what winning is, in `assets/quests.ron`: set the last charge, "then climb all ten decks back to the lift you came down on", and nothing in the game reads that sentence.
And its title screen offers Continue, drawn dim, because Foundry keeps no save.

This slice gives a run both ends: a way out that wins it, and a way to leave and come back to it.

Done when:

- a run can be won by charging the core on deck ten and riding the lift out on deck one;
- a run left in the middle, by quitting or by closing the window, can be picked up again from the title screen's Continue, as it was;
- both are pinned by tests and checked in the running game.

## 2. What was decided

These were settled in conversation and are not reopened here.

1. **The lift out stands where the commando came in**, on deck one's arrival cell, and answers the same keys every other lift does.
2. **It wins only after the core is charged**; before that it refuses with a line and costs no turn.
3. **The win is a quest.** A fifth quest, after the core, is the climb, and finishing it is `victory: true`, so the mission screen shows the way out as the last objective.
4. **A run is saved on the way out and on every deck arrival.** Closing the window or quitting writes it; arriving on a deck writes it; there is no save key, so there is nothing to save-scum with. Death and a win delete it, which the engine already does.
5. **Saving on arrival is the engine's**, an option on `SavePlugin`, since when a save is written is the engine's loop and every game with places wants it.
6. **The title screen stays Foundry's.** Continue and New Game are Foundry's rows; the engine gains nothing for a title screen until a second game has one.
7. **New Game over a save asks first**: one line, "Abandon the run in progress?", defaulting to No.
8. **Cheats are not saved.** Godmode and a revealed deck are a debugging aid, not part of the run.

### The approaches weighed

- **Foundry does all of it**, the arrival save included, following `examples/corsair/src/save.rs`. Contained, but the arrival save would be written again by the next game with places.
- **The arrival save in the engine**, chosen. One option on `SavePlugin`, one engine test; Corsair's cave mouths and Delve's stairs get it for one line.
- **A generic Continue in the engine as well.** Rejected: a title screen is a game's own screen, and one game with one is not yet the signal to move it.

## 3. The lift out

On deck one's first arrival, `lifts::link_decks` puts a `LiftOut` on the arrival cell: an entity with a `Position`, an `OnMap`, a `Name` of "lift out", the amber `<` the other lifts are drawn in, and no `Transition`.

The engine refuses a `GoThrough` that finds no `Transition` under the actor, and leaves the player its turn at no cost.
That refusal is the seam: `lifts::ride_out`, in `TurnSet::React`, reads `ActionRefused` for the player together with the pass's `Intent<GoThrough>`, and answers only when the player stands on the `LiftOut`.

- **The core is not charged**: a `Tell`, "The lift will not move until the core is charged.", and nothing else; the engine has already given the turn back.
- **The core is charged**: a `Happened` of a new fact kind, `lift_out`, which the tracker counts toward the fifth quest.

`quests.ron` gains that quest:

```ron
(
    name: "lift_out",
    title: "The way out",
    text: "The core is charged. Climb back to the lift you came down on, on the first deck, and ride it out.",
    after: ["core_charge"],
    objectives: [(text: "Ride the lift out on deck one", on: LiftOut, need: Total(1))],
    victory: true,
),
```

`mission::On` gains `LiftOut`, and `Facts` its fact kind.
A system reading `QuestChange` answers `QuestDone { victory: true }` with `RunOver::won().saying("The lift climbs out of the foundry, and the core goes up under it.")`, the shape Corsair's `quests.rs` already has.
`offer_the_pick` is unchanged: the lift out is not one of `CHARGE_QUESTS`, so finishing it opens no pick.

What else moves:

- `climb.rs`'s header, `quests.ron`'s schema comment and `examples/foundry/TODO.md` stop saying the run cannot be won.
- `examples/foundry/DESIGN.md` marks the lift out as existing.
- The game menu's `won` line already reads "The core is charged, and you are on the lift." and stays.

## 4. Saving on arrival, in the engine

`SavePlugin` gains `on_arrival()`:

```rust
app.add_plugins(SavePlugin::new("foundry").version(1).on_arrival());
```

With it, an exclusive system in `Last` writes the run through `save_run` once for a frame in which any `PlaceEntered` was written, and only while `EngineState::Playing`.
In `Last` rather than in the turn, because saving reads the whole world and the frame's warp and the deck's population have all landed by then; and once per frame however many arrivals it held.
Without it, nothing changes for Corsair, Delve or Heist.

A save that fails logs the error and the run goes on, as `refresh_stash` does.

Tests: an app with `on_arrival()` that warps the player into a place has a save in its `MemoryBackend` afterwards, and one without it does not.

`rl-save` also gains `SaveableState` for `Counters`, beside the one it has for `Quests`: the fact ledger is the engine's, so its saving is too, and a game with a `Counters` adds `save_state::<Counters>()` and nothing more.

## 5. What Foundry saves

`examples/foundry/src/save.rs`, modelled on Corsair's, registers the plugin and what only Foundry knows.

```rust
app.add_plugins((SavePlugin::new(SLOT).version(VERSION).on_arrival(), UnloadPlugin))
    .save_kind::<ItemKind>()
    .save_kind::<Kind>()
    .save_kind::<Commando>()
    .save_kind::<Lift>()
    .save_state::<Taken>()
    .save_state::<Deepest>()
    .save_state::<Quests>()
    .save_state::<Counters>();
```

The engine already saves, for every saved entity, where it stands, its health, its bag, what it wears, its statuses, its stack, a thing's charges and its triggers' firings, a transition's destination, and the dead left as remains; and the props, the clock, the queue, every built deck with its edits, and what the commando has seen.
So each kind below saves only what Foundry spawned it from and what Foundry changed on it since.

- **`Commando`**, a new marker on the player. Restored by spawning a fresh commando the way `run::start` does, then putting back what the run did to it: the lamp on or off, and every upgrade in `Taken` applied again through `upgrades::apply` to a commando that has none yet.
The uplink's extra tile of reach lives on a worn gun as `Reached`, so a restored gun is given it again by the same path that gave it the first time, rather than saved on the item.
- **`Kind`**, the monster definition droids and critters already carry. Restored by spawning that definition the way `droids::populate_deck` does.
- **`ItemKind`**, a new component naming the item definition, put on every item `gear::spawn_item` makes. Saved with the weapon's `Heat`, if it has one. Restored by `spawn_item`, then the heat put back.
- **`Lift`**, a new marker on the lifts `link_decks` lays and on the lift out. Saved as its glyph, and whether it is the lift out; the engine restores where it leads.

Resources: `Taken`, `Deepest`, and the mission's `Quests` and `Counters`.

`Drops`, the loot stream, is not saved: it is derived again from the saved seed when a run is continued, the way the engine derives its own streams, so a continued run is the same run in every fact it saved and rolls fresh from there.
A continued run that rolled exactly what the uninterrupted one would have is a larger change, to every stream in the engine, and is not this slice.

Not saved: `Cheats`, the title, and whatever is rebuilt from content at every start, the roster, the armory and the mission's definitions.

`VERSION` starts at 1, with the comment Corsair's carries saying what bumps it.

## 6. The title screen

- **Continue** is taken when the `foundry` slot holds a save this build can read: `load_run` answers `Ok(Some(_))`. Otherwise it stays dim and the cursor steps over it, as today.
- **Picking Continue** lowers the title and runs `NewRun`, as New Game does, with a `Resume` resource set. `run::start` sees it, takes the seed from the save, builds the same `Foundry` world, and restores the run into it with `RunSave::restore` instead of spawning a fresh commando on deck one. The log opens with "Continuing on deck N." in place of the run's opening lines.
- **New Game with a save present** opens a confirm over the menu: "Abandon the run in progress?" with Yes and No, No picked out. Yes deletes the save and starts a fresh run; No, or the close key, returns to the menu with New Game picked out. With no save, New Game starts at once, as today.
- **A save this build cannot read**, a wrong version or a damaged file, leaves Continue dim, and New Game asks the same question, so it can always be cleared from the menu.

The confirm is drawn in the title's own style, one line and two words, and its keys are the menu's: up and down, enter, escape.

## 7. Testing

Each test is written to fail before the code it covers.

- **The lift out**: on deck one's first arrival there is a `LiftOut` on the arrival cell; going through it before the core is charged says the line and spends no turn; after the core, it finishes "The way out" and the run is won; going through it anywhere else, or on another deck, is the engine's refusal alone.
- **The engine's arrival save**: saved on a warp with `on_arrival()`, not without it; not while the run is over.
- **The round trip**: a run on deck three with a gun half hot, a spent console on deck three, two upgrades taken, the lamp off, slugs in the pack, a droid wounded, `Deepest` at three and the first charge counted, saved and restored into a fresh app, comes back as it was in every one of those, and plays on.
- **`Counters`**: an `rl-save` test that a ledger's counts come back.
- **The ends**: death deletes the save; a win deletes the save.
- **The title**: Continue is taken only with a readable save; New Game over a save asks, and No keeps it while Yes deletes it; a damaged save leaves Continue dim.
- **In the game**: screenshots of the refused lift, the won ending, the confirm, and a continued run on the deck it was left on.

## 8. Documentation

- `CHANGELOG.md`: `SavePlugin::on_arrival()` and the saved `Counters`; Foundry's lift out, its fifth quest, and its saving.
- `docs/guide/src/systems/saving.md`: `on_arrival()`, re-blessed.
- `docs/OVERVIEW.md`: the saving line gains the arrival save, and Foundry's section its ending and its save.
- `examples/foundry/DESIGN.md` and `TODO.md`: the lift out and saving marked as existing, and the TODO lines for both gone.
- `docs/PLAN.md`: a progress-log entry.

## 9. Out of scope

- A save key or a save anywhere but on arrival and on the way out.
- Several save slots.
- A Continue or a title screen in the engine.
- Saving the cheats.
- Anything on the ending screen beyond the words `RunOver::won()` carries.
