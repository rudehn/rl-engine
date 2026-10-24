<!-- documents:
     plugins: TitlePanel
     files: crates/atglyph-ui/src/panel/title.rs
            crates/atglyph-bevy/src/title.rs
            crates/atglyph-bevy/src/plugin.rs
            crates/atglyph-save/src/run.rs
            crates/atglyph-save/src/unload.rs
            crates/atglyph-ui/src/game_menu.rs
     fingerprint: 0f277b6a -->

# The title screen

A title is the one screen before there is a world: the run is held back until the player picks Continue, New Game, Records, Settings or Exit.
Every game with a title writes the same menu, and a second game would copy the first one's, so the menu is the engine's and the picture behind it is the game's.
The menu knows nothing of storage: what the run's save slot holds, and the asking to continue it or forget it, are words in `atglyph-bevy` that the save crate answers, so the screen and the save never depend on each other.

## Turning it on

`TitlePanel::new(rect)` takes the rectangle its rows are drawn in, from its top down and centred across it, and its `finish` checks for `UiPlugin`.
It inserts `HoldStart`, so the engine begins no run at startup, and a `TitleMenu`, whose presence is also how `GameMenuPanel` knows to offer "Main menu" above Quit, while playing and on the screen a run ends on.
A game paints its picture in `TitleSet::Backdrop`, and the panel draws its rows over it in `TitleSet::Menu`; both run in `Update` ahead of `EngineSet::Input`, and only while `title_is_up`, the start held and the engine idle.
The panel reads its keys ahead of the picture and after a settings screen's and the pause menu's own, then clears the whole terminal to the `surface` tone, so a picture paints only what it draws and a game with none has a blank screen under its rows.
`SavePlugin` is what gives Continue a run to take: it inserts `SavedRun`, reads its slot into it in its `finish`, before the first frame, keeps that current as the slot is written and deleted, answers `AbandonRun`, and writes a run left for the title down before its teardown.
Without it there is no `SavedRun`, and the title draws no Continue at all.
A game's start takes no step of its own: the engine runs no `NewRun` while the title is up, and `SavePlugin` continues a saved run itself, so a start begins a fresh run and asks nothing about a title or a save.

## The model

`HoldStart` is a resource: while it is present the engine's startup runs no `NewRun`, no map is built and the state stays `Idle`.
`begin_run(world)` is the one door a run comes through: it takes the hold away and runs `PrepareRun` then `NewRun`, or `ContinueSaved` while `ContinueRun` stands, as the engine does at startup with no title and after every `Restart`, so no game's start asks whether a title is up.
It begins play once the run stands in a `WorldMap`, building that world over the places' tiles when the game named its places and made none, so no start sets the engine playing.
`LeaveRun` is a message answered in `EndOfFrame::Restart`: the run is torn down as a restart tears it down, `EndRun` and then `clear_run`, and `HoldStart` goes back in place of a new run, so the title is up again once the state is idle.
It wins over a restart asked in the same frame, and with no run standing it changes nothing, so a title already up never runs a game's forgetting over the save it offers.
The run left takes its seed with it: a fresh `Seed` is set for the next run as a restart that names none sets one, so New Game never replays the run that was left, while Continue builds the world from the seed the save holds.
`SavedRun` is what the slot holds, `Empty`, `Readable` or `Unreadable`, plain data a screen reads knowing nothing of storage, and its absence says the game keeps no slot.
`ContinueRun` is a marker resource asking `begin_run` to continue the saved run rather than begin one, and it is gone afterwards either way, so the run after it, a restart's, begins afresh.
`AbandonRun` is an event triggered on the world rather than a message written for later, so whoever keeps the slot forgets it before the trigger returns, and a run begun in the same command never sees it.
`TitleRow` is `Continue`, `NewGame`, `Records`, `Settings` or `Exit`, drawn in that order.
Continue is drawn wherever there is a `SavedRun` and taken only over a `Readable` one; Records is drawn once a screen declares the `records` modal and taken once `RunRecords` holds a record; Settings is drawn only when a `SettingsPanel` has a setting to list.
`TitleMenu` is the menu as it stands: `selected` counts the rows that can be taken, `confirm` is the question while it is asked, `covered` says a screen stands over the menu, and `rows` lists the takeable rows as the keys last saw them.
The keys refill it once a frame before anything draws: the direction keys walk the takeable rows and wrap, confirm takes the one picked out, and the close key exits, each read through `DirectionKeys` and `CursorKeys`, so a rebound key is rebound here too.
Continue inserts `ContinueRun` and begins the run.
New Game over anything saved, even a slot this build cannot read, asks "Abandon the run in progress?" with No picked out, any direction key moving between the answers and the close key answering No, and Yes triggers `AbandonRun` and begins.
Each begins through a queued command, so the run comes up once the frame's commands are applied, and the menu forgets its place for the next time the title is up.
Records and Settings open their screens over the menu, which reads no keys and draws nothing until the screen closes.
The settings screen is moved to stand on the menu while the title is up, as wide as the game made it and with room for every row its own rectangle shows, rising from the menu's top only as far as those rows need to end on the terminal, since it does not scroll, and is put back where the game put it in `PrepareRun`, which also blanks the terminal, so every run begins on an empty screen.
`TitleWords` is what each row and the question say, changed with `TitlePanel::words`.
`MenuItem::MainMenu` is the pause and end menus' row back, and it writes `LeaveRun`.
With `SavePlugin`, a run left while still in play is written to the slot as it stands in `EndOfFrame::Save`, ahead of the teardown, which then keeps the slot rather than deleting it; a run that is over, or ends in the frame it is left, is only left, and its save forgotten.
`save_run`, forgetting the save and the flush on exit each write what they did to the slot into `SavedRun`.

## Using it

A title is a panel, added with the others, beside the picture the game paints under it: `app.add_plugins(TitlePanel::new(rect))` and the game's painter in `Update`, `.in_set(TitleSet::Backdrop)`.

## The line

The engine decides what a title offers and when: the rows, which of them can be taken, the question asked before a run is given up, the one door every run comes through, and that a run begun after one was left is on a seed of its own.
The game decides how it looks and what it says: its picture, painted under the rows on a cleared terminal, the rectangle the rows stand in, and the words of each row and of the question.
Whether the slot holds a run is the save's to say and the menu's to read, never the menu's to find out, so a game that saves nothing has a title with no Continue and a screen that never touches storage.
Restoring a run is the engine's: Continue inserts `ContinueRun`, and `SavePlugin` sets the saved seed, runs the game's `PrepareRun` to build the world on it, restores the run and runs the game's `RunResumed`; what a continued run says or gives back is the game's, in `RunResumed`.
A row that cannot be taken now but could be later, Continue before anything is saved and Records before any run has ended, is drawn dim, while one that never can, Continue in a game that saves nothing and Settings with nothing to set, is not drawn at all, since whether a game keeps a slot or has a settings screen cannot change while it runs.
The hold is on the whole of a run's start, `PrepareRun` and `NewRun` alike, not only on a game's own systems, so an `Update` system that reads a resource a run inserts must take it as optional, or the first frame of a launch panics.
Leaving a run is not ending it: in a game that saves, a run still in play is written down to be continued, and only a run that is over has nothing to keep.
A headless test has no title unless it adds one, so its run begins at startup as it always did.

## Where it lives

The words a title and a save share are plain data in `atglyph-bevy`, the one crate below both, which is what keeps `atglyph-ui` free of `atglyph-save`, the same split settings make.
The hold and the door are `atglyph-bevy`'s because the run's life is: `begin_run` is the door the engine's own startup and its restarts go through, so a title is one more caller rather than a second way in.
The menu is a presenter over state in a resource, so every row, the question and what each key does to them are tested by pressing keys at a headless app, with no window and no save behind it.
A game's own title once held all of this as some three hundred lines of menu over eight hundred of picture; the menu moved here, and the picture stayed with the game.
