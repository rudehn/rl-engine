<!-- documents:
     plugins: TitlePanel
     files: crates/rl-ui/src/panel/title.rs
            crates/rl-bevy/src/title.rs
            crates/rl-bevy/src/plugin.rs
            crates/rl-save/src/run.rs
            crates/rl-save/src/unload.rs
            crates/rl-ui/src/game_menu.rs
     fingerprint: 224bdc4f -->

# The title screen

A title is the one screen before there is a world: the run is held back until the player picks Continue, New Game, Records, Settings or Exit.
Every game with a title writes the same menu, and a second game would copy the first one's, so the menu is the engine's and the picture behind it is the game's.
The menu knows nothing of storage: what the run's save slot holds, and the asking to continue it or forget it, are words in `rl-bevy` that the save crate answers, so the screen and the save never depend on each other.

## Turning it on

`TitlePanel::new(rect)` takes the rectangle its rows are drawn in, from its top down and centred across it, and its `finish` checks for `UiPlugin`.
It inserts `HoldStart`, so the engine begins no run at startup, and a `TitleMenu`, whose presence is also how `GameMenuPanel` knows to offer "Main menu" above Quit, while playing and on the screen a run ends on.
A game paints its picture in `TitleSet::Backdrop`, and the panel draws its rows over it in `TitleSet::Menu`; both run in `Update` ahead of `EngineSet::Input`, and only while `title_is_up`, the start held and the engine idle.
The panel reads its keys ahead of the picture and after a settings screen's and the pause menu's own, then clears the whole terminal to the `surface` tone, so a picture paints only what it draws and a game with none has a blank screen under its rows.
`SavePlugin` is what gives Continue a run to take: it reads its slot into `SavedRun` in its `finish`, before the first frame, keeps that current as the slot is written and deleted, answers `AbandonRun`, and writes a run left for the title down before its teardown.
Without it `SavedRun` stays the `Empty` that `CorePlugin` initialises, and Continue is never taken.
A game's start takes one step of its own: it stands aside while `ContinueRun` is present, for whichever of the game's systems restores the saved run.

## The model

`HoldStart` is a resource: while it is present the engine's startup runs no `NewRun`, no map is built and the state stays `Idle`.
`begin_run(world)` is the one door a run comes through: it takes the hold away and runs `NewRun`, as the engine does at startup with no title and after every `Restart`, so no game's start asks whether a title is up.
`LeaveRun` is a message answered in `EndOfFrame::Restart`: the run is torn down as a restart tears it down, `EndRun` and then `clear_run`, and `HoldStart` goes back in place of a new run, so the title is up again once the state is idle.
It wins over a restart asked in the same frame, and with no run standing it changes nothing, so a title already up never runs a game's forgetting over the save it offers.
`SavedRun` is what the slot holds, `Empty`, `Readable` or `Unreadable`, plain data a screen reads knowing nothing of storage.
`ContinueRun` is a marker resource asking the next `NewRun` to restore the saved run rather than begin one, and whichever system restores it removes it, so the run after it, a restart's, begins afresh.
`AbandonRun` is an event triggered on the world rather than a message written for later, so whoever keeps the slot forgets it before the trigger returns, and a run begun in the same command never sees it.
`TitleRow` is `Continue`, `NewGame`, `Records`, `Settings` or `Exit`, drawn in that order.
Continue is always drawn and taken only over a `Readable` slot; Records is drawn once a screen declares the `records` modal and taken once `RunRecords` holds a record; Settings is drawn only when a `SettingsPanel` has a setting to list.
`TitleMenu` is the menu as it stands: `picked` counts the rows that can be taken, `confirm` is the question while it is asked, `covered` says a screen stands over the menu, and `rows` lists the takeable rows as the keys last saw them.
`title_keys` refills it once a frame before anything draws: the direction keys walk the takeable rows and wrap, confirm takes the one picked out, and the close key exits, each read through `DirectionKeys` and `CursorKeys`, so a rebound key is rebound here too.
Continue inserts `ContinueRun` and begins the run.
New Game over anything saved, even a slot this build cannot read, asks "Abandon the run in progress?" with No picked out, any direction key moving between the answers and the close key answering No, and Yes triggers `AbandonRun` and begins.
Each begins through a queued command, so the run comes up once the frame's commands are applied, and the menu forgets its place for the next time the title is up.
Records and Settings open their screens over the menu, which reads no keys and draws nothing until the screen closes.
The settings screen is moved to stand on the menu while the title is up, as wide and as tall as the game made it, and is put back where the game put it in `NewRun`, which also blanks the terminal, so every run begins on an empty screen.
`TitleWords` is what each row and the question say, changed with `TitlePanel::words`.
`MenuItem::MainMenu` is the pause and end menus' row back, and it writes `LeaveRun`.
With `SavePlugin`, a run left while still in play is written to the slot as it stands by `save_on_leaving` in `EndOfFrame::Save`, ahead of the teardown, which then keeps the slot rather than deleting it; a run that is over, or ends in the frame it is left, is only left, and its save forgotten.
`save_run`, `forget_save` and `UnloadPlugin`'s `flush_on_exit` each write what they did to the slot into `SavedRun`.

## Using it

A title is a panel, added with the others, beside the picture the game paints under it.

<!-- include: ../../../../examples/undermere/src/main.rs:title -->
```rust,no_run
    // The screen before there is a run: the engine's menu, which holds the
    // run back until the player picks, over the night Undermere paints
    // under it. With it up, the pause and end menus offer the way back.
    app.add_plugins(TitlePanel::new(screen.title_menu)).add_systems(Update, undermere::title::paint_backdrop.in_set(TitleSet::Backdrop));
```

The step a panel cannot take for a game is in its start, which begins a run as it always did unless the title asked to continue one.

<!-- include: ../../../../examples/undermere/src/run.rs:start -->
```rust,no_run
/// Builds the Undermere, spawns the adventurer with their kit and warps
/// them into Mereholt, or onto [`StartFloor`]'s floor. Runs in [`NewRun`],
/// for every run but one the title asked to continue with `ContinueRun`,
/// which is [`resume`]'s.
///
/// Asks nothing about a title: while one is up the engine runs no
/// `NewRun` at all, and runs it once the player picks.
pub fn start(
    mut commands: Commands,
    seed: Res<Seed>,
    content: Content,
    first: Option<Res<StartFloor>>,
    continuing: Option<Res<ContinueRun>>,
    mut begin: Begin,
) {
    if continuing.is_some() {
        return;
    }
    prepare(&mut commands, &seed, &content.registries);
    let adventurer = spawn_adventurer(&mut commands, &content.registries);
    equip_kit(&mut commands, adventurer, &content.armory, &content.registries);
    begin.log.notice(format!("Seed {}. Ten floors under the lake, the Drowned King waits.", seed.0.0), 0);
    let place = match first.map_or(0, |f| f.0.min(FLOORS)) {
        0 => TOWN,
        floor => map_of(floor),
    };
    begin.warps.write(WarpRequest::into_place(adventurer, place));
    begin.next.set(EngineState::Playing);
}
```

## The line

The engine decides what a title offers and when: the rows, which of them can be taken, the question asked before a run is given up, and the one door every run comes through.
The game decides how it looks and what it says: its picture, painted under the rows on a cleared terminal, the rectangle the rows stand in, and the words of each row and of the question.
Whether the slot holds a run is the save's to say and the menu's to read, never the menu's to find out, so a game that saves nothing has a title whose Continue is never taken and a screen that never touches storage.
What restoring a run means is the game's: the engine says Continue was picked with `ContinueRun`, and the game's own `NewRun` systems build the world from the saved seed and restore the run into it, as they would from any save.
A row that cannot be taken now but could be later, Continue before anything is saved and Records before any run has ended, is drawn dim, while one that never can, Settings with nothing to set, is not drawn at all, since whether a game has a settings screen cannot change while it runs.
The hold is on the whole of `NewRun`, not only on a game's start, so an `Update` system that reads a resource a run inserts must take it as optional, or the first frame of a launch panics.
Leaving a run is not ending it: a run still in play is written down to be continued, and only a run that is over has nothing to keep.
A headless test has no title unless it adds one, so its run begins at startup as it always did.

## Where it lives

The words a title and a save share are plain data in `rl-bevy`, the one crate below both, which is what keeps `rl-ui` free of `rl-save`, the same split settings make.
The hold and the door are `rl-bevy`'s because the run's life is: `begin_run` is the door the engine's own startup and its restarts go through, so a title is one more caller rather than a second way in.
The menu is a presenter over state in a resource, so every row, the question and what each key does to them are tested by pressing keys at a headless app, with no window and no save behind it.
Foundry's own title held all of this as some three hundred lines of menu over eight hundred of picture; the menu moved here, and the picture stayed with the game.
