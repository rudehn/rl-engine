<!-- documents:
     plugins: ControlsPanel, GameMenuPanel, KeyScriptPlugin, PlayerKeysPlugin, ReplayPlugin
     files: crates/atglyph-ui/src/lib.rs
            crates/atglyph-ui/src/controls.rs
            crates/atglyph-ui/src/player_keys.rs
            crates/atglyph-ui/src/keys.rs
            crates/atglyph-ui/src/modal.rs
            crates/atglyph-ui/src/cursor.rs
            crates/atglyph-ui/src/panel/controls.rs
            crates/atglyph-ui/src/game_menu.rs
            crates/atglyph-ui/src/interact.rs
            crates/atglyph-ui/src/replay.rs
            crates/atglyph-bevy/src/replay.rs
            crates/atglyph-bevy/src/seed.rs
            crates/atglyph-bevy/src/testing.rs
     fingerprint: 81a60f86 -->

# Controls, modals and cursors

A game that checks `KeyCode::KeyG` in one system and prints "g get" in another holds two copies of one fact, and the copy on screen is the one nobody updates.
So a key is declared once, as a group, an action and the chords that ask for it, and both the game and the controls screen read that one declaration.
Around it sit the two things every game with a screen gets wrong on its own: which screen owns the keyboard, and what a cursor does when you push it at a wall.

## Turning it on

`UiPlugin` is the base, and it holds all of the bindings: the `Controls` registry, `Modals`, `DirectionKeys`, `CursorKeys`, `ControlsKeys`, the `RepeatPace` and the `Repeats` state.
It initialises `ButtonInput<KeyCode>` itself, so a headless game with no input plugin still has the resource every reader here takes and reads nothing pressed.
It clears `Modals`'s frame flags in `First`, runs `forget_keys_on_focus_change` before `advance_repeats`, and `advance_repeats` before `EngineSet::Input` so the frame's repeat is decided before anything reads a key.
`close_on_escape` runs after `EngineSet::Input` and before `EngineSet::Turns`, which is the promise for a screen a game declared and never taught to close.
`ControlsPanel` takes the `Rect` it draws in, declares the `controls` modal, reads its keys in `EngineSet::Input`, draws the hint in `PresentSet::Chrome` and the screen in `PresentSet::Overlay`.
Its `finish` declares `depends_on::<UiPlugin>` and adds its own control last, so a game's groups are listed before the engine's.
`GameMenuPanel` takes a `Rect` that is the most it may occupy rather than the size it will be, declares the `menu` modal, and draws in `PresentSet::Overlay`.
Its keys run before `EngineSet::Input` and outside the engine's sets, because those sets stop once the run is over and the menu is the screen the run ends on; `OnEnter(EngineState::Over)` is where it opens itself, with no way back into the run.
It adds the view that screen's words are drawn from behind it, the way any presenter adds its own, so a game has somewhere to push what it wants the ending to say; [Saving and the ending screen](saving.md) is the page for that.
`PlayerKeysPlugin` declares the standard verbs a game names, each under the group and the action it gives, and reads them in `PlayerKeysSet` inside `EngineSet::Input`, never while a screen is up; its `finish` declares `depends_on::<UiPlugin>`, and `depends_on::<ItemsPlugin>` only when `pick_up` was named.
The interact key is `UiPlugin`'s in a game with props: its `finish` adds it when `PropsPlugin` was added, whichever came first, and declares it under the screens heading, so a game with no props has no such key.
`ReplayPlugin` adds a recorder when it is given a path and a player when it is given a recording, and `ReplayPlugin::from_env` reads `ATGLYPH_RECORD` and `ATGLYPH_REPLAY` for both.
`KeyScriptPlugin` is a test's keyboard and lives in `atglyph-bevy`: it presses in `PreUpdate` after Bevy's input has been cleared, and adds Bevy's `InputPlugin` if the app has none.

## The model

`Chord` is a key with Shift held or not, matched exactly, so `L` and `l` mean different things without either system checking for the other, and `label` writes it the way a player reads a keycap.
`Keys` is what asks for a control: `Chords` for a list matched in order, `Directions { shift }` for every key `DirectionKeys` binds, `Engine` for one of the engine's own bindings, or `Any` for any key at all, which is what stops the player's work.
`EngineKey` names a binding rather than copying a key, and the chord is read out of `CursorKeys`, `ControlsKeys`, `MenuKeys` or a panel's own resource every time it is listed, so a game that rebinds a cursor key sees the new key on the screen without telling anyone.
A `Control` is a `group`, an `action` in the game's words and its `keys`; `Controls::add` hands back a `ControlId` and gives the same id to an identical declaration, so two plugins reading one cursor key list it once.
The registry keeps declaration order, which is the order the screen lists in, with a group placed where its first control was declared, and two controls of one group on the same keys listed as one row.
`find`, `rename` and `regroup` are for a game rewording or moving one the engine declared, and `App::add_control` is the same thing while the app is built.
`ControlInput` is what a game's input system takes in place of `ButtonInput<KeyCode>`: `just_pressed`, `pressed`, `which` for which of a control's chords went down, `direction` and `direction_held`, `label`, and `input` for the rare reading the registry has no word for.
`DirectionKeys` binds the arrows, `hjklyubn` and the numpad to the eight directions, because a player who reaches for `k` and a player who reaches for the numpad are both right; `none` and `bind` build another set.
`RepeatPace` is the wait before a held key repeats and the wait between repeats, one resource for every game since a player who holds a key expects the same walk in each.
`Repeats` is one state rather than one per control, because the direction keys are one physical set and a player holds one of them at a time, and `ControlInput::direction` reads a repeat as if the key had been pressed again.
A repeat also asks `CueWait` for a skip, since the skipper reads a fresh press and a repeat is not one; without that, walking with a key down would wait out every cue in sight.
`Bindings` borrows the binding resources together and turns a `Keys` into chords or into a label, writing the direction keys as the families they come in so a walk is one row rather than eight.
`Modals` is the stack of open screens, innermost last: `declare` interns a name, `open` raises one already on the stack rather than listing it twice, `close` returns to the one under it, `close_one` takes one out of the middle, and `close_all` is what an action that ends a turn does.
`any_open` is true while anything is open *and* for the rest of the frame a screen closed on, because the key that closed it is still down and the world must not read it as its own.
`modal_is` gates a screen's keys on being the one on top, `modal_open` on being open at all, and `no_modal` is the one gate a game puts on its own input.
`CursorKeys` is one set for both cursors, a key to look, one to step to the next thing in sight, one to close and two to confirm, because a player reaching for Enter and a player reaching for Space are both right.
`steer` is the one reading of a frame's keys onto a cursor, answering `Steer::Close`, `Confirm`, `Moved` or `Stay`: close wins over confirm and both over moving, so a frame with several keys down never acts and moves at once.
It takes the candidates as a closure and asks for them only when the cursor moves, since working out what is in sight is the expensive part and most frames press nothing.
Stepping stops at the edge of the window rather than sliding along it, which would read as the cursor moving on its own, and every step or cycle drags the shared focus with it so the row a panel highlights is what the cursor is on.
`CursorStyle` is `Glow` or `Ticks`, each taking a `ToneId` and an optional second tone to breathe toward, and `mark` is the one drawing of them, so the two cannot drift apart.
A glow washes every cell of the tile it marks; the ticks stand on the four tiles round it, each put down as the tile's own glyph is, so on a map of tiles wider than a cell they are centred on their tiles and stand as far off on every side.
`ControlsScreen` is which page is showing and `ControlsLayout` is where the screen and its one hand-typed hint are drawn.
The menu's rows are resume, a new run, the same seed again, settings, the main menu and quit, chosen by whether the run is in play, whether there is a settings screen and whether there is a title.
Resume is offered only while playing, `Settings` only when a `SettingsPanel` has a setting to list, which it opens over the menu, and `MainMenu` only when a `TitlePanel` was added; each of the rest is one message, a `Restart`, a `LeaveRun` or an `AppExit`, and nothing in the menu knows how a game starts.
With nothing open and something in sight picked out, the close key lets go of it rather than opening the menu, so backing out of the nearby list takes the same key as backing out of a cursor, and the next press opens the menu.
`Recording` is a run written down: the seed, the command line it was run with, and every frame that pressed something as a `Pressed` of a turn clock, the keys and whether Shift was with them.
`KeyScript` is the test keyboard: `press` for one frame, `hold` until `release`, which is what a finger resting on a key does.
`PlayerVerb` is the six standard verbs, `Walk` writing a `Bump`, `GoThrough`, `Wait`, `PickUp`, `Fire` opening the targeting cursor when the player has a shot, and `Throw` opening the pack on the first thing that can be thrown, each on the keys every roguelike here chose unless `keys` binds others.
Fire or throw with nothing to act on spends no turn, says the game's `Otherwise` line if it gave one, and writes a `Declined { actor, verb }` for a game that says why in its own words.

## Using it

A game names the standard verbs it wants to `PlayerKeysPlugin`, and declares its own keys once beside them, under the headings the controls screen groups them by, keeping the ids.

<!-- include: ../../../../examples/tutorial/src/bin/step09_panels.rs:keys -->
```rust,no_run
/// Warren's own keys.
#[derive(Resource, Clone, Copy)]
struct Binds {
    shove: ControlId,
    screech: ControlId,
    eat: ControlId,
    throw: ControlId,
    lantern: ControlId,
    quit: ControlId,
}

/// Declares the keys. Walking and waiting are the engine's standard keys.
fn declare_controls(app: &mut App) {
    app.add_plugins(
        PlayerKeysPlugin::new()
            .walk("Move", "walk or attack")
            .go_through("Move", "take the stairs")
            .pick_up("Act", "pick up what is here")
            .wait("Act", "wait a turn"),
    );
    let binds = Binds {
        // The same direction keys with Shift held. A chord matches exactly,
        // so Shift and `l` never reads as a step.
        shove: app.add_control("Move", "shove", Keys::Directions { shift: true }),
        screech: app.add_control("Act", "screech", KeyCode::KeyA),
        eat: app.add_control("Act", "eat a crust", KeyCode::KeyE),
        throw: app.add_control("Act", "throw a rock", KeyCode::KeyR),
        lantern: app.add_control("Act", "lantern on or off", KeyCode::KeyT),
        quit: app.add_control("Game", "quit", KeyCode::KeyQ),
    };
    app.insert_resource(binds);
}
```

The other half is one run condition, which is the whole of what a game has to remember about screens it has not written yet.

<!-- include: ../../../../examples/tutorial/src/bin/step09_panels.rs:gate -->
```rust,no_run
        app.add_systems(Update, (player_input, tend_lantern).in_set(EngineSet::Input).run_if(no_modal));
```

## The line

The engine decides the shape of input and the game decides the meaning.
Which chords exist, that a chord matches Shift exactly, that a held key repeats at one pace, that a control has a group and an action and can be listed: all of that is the engine's, and none of it says what any key does.
What a key asks for is the game's, and the engine never reads a `KeyCode` the game bound: it reads a `ControlId` the game declared, so a game that rebinds a key changes one line and the screen changes with it.
The standard verbs are the engine's because every game wrote the same six branches line for line, and each only writes the intent a hand-written key would, so the turn loop still decides what it comes to and nothing in them is a rule.
The engine's own keys go through the same registry, which is what makes the controls screen a list of what the game actually reads rather than a second copy of it to keep in step.
A modal takes the keys by being on top of the stack, not by anything the engine does: the engine never opens or closes a screen, a key handler does, and every system that should not fire while a screen is up asks the stack instead of asking each screen.
A key that only means something inside a screen is that screen's own, read while it is the top one and written along its bottom border, and it is not declared in the registry, because two `a`s are no clash when one of them can only be pressed inside a modal.
The frame a screen closed on still counts as a frame with a screen up, which is the difference between an Enter that confirms an aim and an Enter that also takes the stairs and spends the turn the aim was for.
`ReplayPlugin` is what makes "sometimes it gets stuck" into a bug with a reproduction: the loop and every roll are deterministic given the seed, so a run is its seed plus the player's keys.
What is recorded is what the game reads, after the repeat has been turned into presses, each key stamped with the turn clock it was read at, so nothing in the file depends on how fast the frames came.
A replay never presses while the turns are held for something to be seen, because a key then would skip it and what it skipped decides which clock the next key is read at.
A recording whose clock the game has already passed is a run that drifted, and the replay stops and says which key it reached rather than pressing on into a world that is not the one recorded; that is determinism reporting itself, not enforcing itself.
The half the plugin does not do is the seed: it presses keys and nothing else, so a game that sets `ATGLYPH_REPLAY` and leaves its own seed alone replays a recorded run against a fresh world and drifts on the first key.
Restoring it is the game's, because the seed is inserted before play begins and the plugin has no say in when that happens: `LaunchArgs::from_env` reads the recording's seed and flags while one is being played, and `--seed` and the command line otherwise, which is what a game with flags of its own inserts from, and `Seed::from_args` is the one line that wraps it for a game with no flags of its own, falling back to a fresh seed.
`KeyScriptPlugin` is the same idea at the other end: a key pressed on `ButtonInput` from outside the schedule is wiped before any system sees it, so a test that wants to press one needs a plugin, and a test's keys then run the same path a player's do.
What the engine does not get is a rebinding screen or text entry, because those belong to an application rather than to a roguelike, and nothing here reads a click: what one means is the game's.

## Where it lives

All of it is tier 2, because a `ModalId` gates systems and a control is read out of a Bevy resource, and none of it has arithmetic worth pulling down a tier.
The split that matters is the one inside the crate: the registry, the stack and the cursor reading are each a module with no panel in them, so a game that draws its own controls screen or its own menu still declares its keys once and still gates on the same stack.
`steer` is a free function over a point, a focus, a frame's keys and a bounds, which is why a cursor's behaviour at the edge of a map is a test rather than a playthrough.
The recording's file lives in `atglyph-bevy` and the plugin that writes and reads it in `atglyph-ui`, because the file is a seed and a list of keys and the plugin is the one thing that knows what a held key means.
That split is also what lets a game ask `LaunchArgs::from_env` before there is an app, and so start a replay on the seed and the flags it was recorded with.
`KeyScriptPlugin` is in `atglyph-bevy` rather than beside the tests that use it, because the engine's crates and a game's tests want the same keyboard and nine copies of one was how it started.
