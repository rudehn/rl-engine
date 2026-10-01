<!-- documents:
     plugins: FullscreenPlugin, SettingsPanel, SettingsSavePlugin
     files: crates/rl-bevy/src/settings.rs
            crates/rl-render/src/fullscreen.rs
            crates/rl-ui/src/panel/settings.rs
            crates/rl-ui/src/game_menu.rs
            crates/rl-save/src/settings.rs
            crates/rl-engine/src/lib.rs
     fingerprint: d11660b5 -->

# Settings

A setting is something a player chooses once and expects to find again: whether the game fills the screen, and whatever else a game thinks is the player's to decide.
It is declared once, as a name, where it is listed and the choices it has, and three things read that one declaration: whoever owns what the setting changes, the screen that lists it, and the plugin that remembers it.
None of the three knows the others.

## Turning it on

`RoguelikePlugins` adds `FullscreenPlugin`, so every game on the terminal has one setting declared before it adds anything: `fullscreen`, under `Display`, `Off` or `On`, with F11 as its key.
Nothing is fullscreen until something sets it, and the key is the settings screen's to read, so a game without `SettingsPanel` has the setting and not the key.
`SettingsPanel` takes the `Rect` it may draw in, declares the `settings` modal, and in its `finish` turns every setting that has a key into a control, so the controls screen lists it.
`SettingsSavePlugin` is the opt-in third: it needs `Saves`, says so through `app.needs`, recalls what was remembered in its `finish` and writes in `Last` when a setting has changed.
Without it every setting works and starts on its default each launch.
On the web `FullscreenPlugin` declares nothing, because a browser grants fullscreen only from inside the handler of a key or a click and Bevy reads keys a frame later; the canvas follows the page, and the browser's own fullscreen fills the screen.

## The model

`Setting` is a `name`, a `group`, a `label`, its `choices`, the `default` among them and an optional `key`, built with `Setting::new` and then `default_choice` and `key`.
The name is the word it is remembered under, so renaming one forgets what every player chose; the group and the label are what the screen shows.
`app.add_setting` declares one and returns a `SettingId`, and panics on a name declared twice, a setting with no choices or a default it does not have, since each is a mistake in a game's setup.
`Settings` is the registry, a resource: `chosen` is a setting's choice by its place and `choice` the same in words, `choose` sets it, `cycle` moves it along and wraps at both ends, `find` looks one up by name and `iter` lists them in the order they were declared.
It does not say which setting moved; a reader watches the resource with Bevy's change detection and compares against what it last applied, the way `FullscreenPlugin` compares against the window's mode before writing it.
`remembered` is every name with the words of its choice, and `recall` sets each declared setting a map names, passing over a name nobody declared and a choice a setting no longer has.
Words and not places in the list, so a choice added in the middle of a setting does not turn everyone's into another.
`FullscreenPlugin` puts the primary window in borderless fullscreen or back in a window when the setting and the window disagree, and once more in its `cleanup`, after every plugin's `finish`, so a choice already recalled is on the window before the first frame.
`SettingsPanel` lists the settings under their groups, a group standing where its first setting was declared: up and down pick a row, left and right change it, confirm changes it forward and the close key goes back to whatever was under the screen.
A setting's own key changes it when no screen is open or this one is on top, in any engine state.
The game menu offers a `Settings` row above `Quit` when the panel was added and at least one setting is declared, and is not drawn while the screen it opened is up.
`SettingsSavePlugin` keeps one slot, `settings`, a RON map of names to words; text that does not parse is a warning and the defaults stand, and loading never writes.

## Using it

A game adds the screen as it adds any panel, and the menu finds it.

<!-- include: ../../../../examples/foundry/src/main.rs:settings -->
```rust,no_run
    // What the player may set, opened from the menu and from the title
    // screen: fullscreen is the engine's, declared by `RoguelikePlugins`.
    app.add_plugins(SettingsPanel::new(screen.settings));
```

Remembering takes a line of its own beside the backend a run is saved through.

<!-- include: ../../../../examples/foundry/src/main.rs:remember -->
```rust,no_run
        // Remembered beside the save and apart from it. A scripted run's
        // backend is in memory, so a capture or a replay starts on the
        // defaults whatever the player chose.
        .add_plugins(SettingsSavePlugin)
```

## The line

The engine decides how a setting is listed, changed and remembered, and it owns the one setting that is about the engine's own window.
The game decides what else is the player's to choose: it declares the setting, keeps the id, and acts on the choice in a system of its own.
So there is no enum of settings and no typed value per setting; a registry is the extension point, as it is for controls, and a game's settings sit beside the engine's on one screen with nothing to tell them apart.
A setting is a choice among a few named things and nothing finer: a slider, a key to rebind and a line of text are not settings in this sense, and a game that wants one writes its own screen.
The settings screen is the one screen read and drawn outside the engine's sets, after the whole of `EngineSet::Present` and in any engine state, because it is the one a game opens before there is a world, from a title screen of its own where no presenter runs.
A game's own screen under it is the game's to put away: the engine's menu steps aside by itself, and a title screen stops reading keys while any modal is open and stops drawing its menu under it.
What is remembered is apart from the run and outlives it: a new run, a restart and an ending never touch the slot, and it is not the versioned envelope a run is saved in, which refuses a save whole when the version differs.
A preference should survive an upgrade, so the file is read for what it still means.
The engine does not read the window back into the setting: a window sent fullscreen by the operating system's own button is fullscreen with the setting still `Off`.

## Where it lives

The registry is plain data in `rl-bevy`, the one crate the renderer, the UI and saving all depend on, which is what lets each do its part with no edge between them: `rl-ui` still does not depend on `rl-save`.
Being plain data it is tested with no window and no `App`, and so is what saving writes, which is a map built by `remembered` and compared before anything touches a backend.
The window's half is in `rl-render` because the window is the renderer's, beside the layout that draws the grid for whatever size the window turns out to be.
