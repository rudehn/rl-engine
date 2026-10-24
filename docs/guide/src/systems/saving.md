<!-- documents:
     plugins: SavePlugin, EndingViewPlugin
     files: crates/atglyph-save/src/run.rs
            crates/atglyph-save/src/engine.rs
            crates/atglyph-save/src/backend.rs
            crates/atglyph-save/src/unload.rs
            crates/atglyph-save/src/versioned.rs
            crates/atglyph-save/src/remap.rs
            crates/atglyph-ui/src/view/ending.rs
            crates/atglyph-ui/src/game_menu.rs
            crates/atglyph-bevy/src/state.rs
            crates/atglyph-bevy/src/world.rs
            crates/atglyph-bevy/src/plugin.rs
            crates/atglyph-bevy/src/cue.rs
            crates/atglyph-bevy/src/minds.rs
            crates/atglyph-bevy/src/title.rs
            crates/atglyph-save/src/lib.rs
     fingerprint: 2b07ed69 -->

# Saving and the ending screen

A save is a run written down as text, and the engine writes most of it.
A game says only what each kind of thing it spawns is, in its own words; where that thing stands, what it carries, what it wears and what is on it are the engine's, and so are the clock, the queue, the map's edits and what the player has seen.
The end of a run is the other half, and the one state a save is never continued into: the slot is deleted, and what the run was about is drawn on the screen the run ends on, and kept only as a record where a game keeps them, which [Run records](records.md) covers.
What is saved reaches storage through one trait, so a game in a browser and a game on a desktop differ in how one resource was built and nowhere else.

## Turning it on

`SavePlugin::new(slot)` is the loop around a save, and `.version(n)` is the number every save is matched against.
It inserts `Saves`, the backend, unless the game inserted its own: `Saves::platform_default` under the name `SavePlugin::new` is given, or memory when `atglyph::ecs::replay::scripted` says the run is a replay, a recording or a capture, so a scripted run neither starts on the player's save nor writes over it.
A game played by several people on one machine, as over SSH, inserts `Saves::per_user(name, user)` first, which keeps each user's run, settings and records in a directory of their own and refuses a user name that is not a plain name.
It registers `PropKind` and `Stair` as saved kinds itself, inserts `SaveSlot` and the `Stash`, refreshes the stash in `Last`'s `EndOfFrame::Save` stage, and deletes the slot on entering `EngineState::Over` and in the `EndRun` schedule, unless the run is being left for a title.
It continues a run itself: while `ContinueRun` stands, the run's start runs `ContinueSaved` in place of `NewRun`, and there the plugin loads the save, sets its seed, runs the game's `PrepareRun`, restores the run and runs the game's `RunResumed`.
A save that cannot be read, or restores without exactly one player, is torn down and a fresh run begun on the seed that stood before.
So a game puts what every run stands on, its rules, its places and its world, in `PrepareRun`, what only a fresh run does in `NewRun`, and what a continued run does after its restore in `RunResumed`, and writes no load or restore of its own.
For a title it does three things more: it inserts `SavedRun`, the title's sign that there is a slot to continue from, and reads the slot into it in its `finish`, before the first frame, forgets the slot the moment `AbandonRun` is triggered, and writes a run left with `LeaveRun` while still in play down in the save stage ahead of the teardown; [The title screen](title.md) is the page for those.
`.on_arrival()` also writes the slot on any frame the player arrived somewhere while playing, so each place entered is a save point; it runs in the same stage, which comes before the dead are buried and a restart or a leaving tears the run down, so it never writes a world with no run in it.
It registers no key: saving reads the whole world, so a game's save key is its own exclusive system and calls `save_run`.
`SavePlugin` is also the other half: it writes whatever is stashed on the frame the app is told to exit, after the stash was refreshed, which is the frame a native window's close button produces, and in a browser it also installs a listener for the page being hidden or unloaded.
`EndingViewPlugin` is the second plugin, and it is not a save at all: it holds what a game wants said on the screen a run ends on, needs `UiPlugin`, and is added by `GameMenuPanel` itself, so a game drawing its ending with the engine's menu adds nothing.
A game that wants the sections under a screen of its own adds the plugin alone and reads the view.
Nothing here is on by default, and a game that adds no `SavePlugin` never reaches storage at all.

## The model

`Saveable` is the one trait a game writes, implemented on the component that marks a kind: `capture` writes an entity down as `Self::Saved`, and `restore` spawns one again from that, nowhere, carrying nothing, at full health.
Neither says anything about position, health, bags, slots, stacks, statuses, remains, where a transition leads, where an actor is posted, the work it is in the middle of, how long something alight has left to burn, or whom it follows as a leader, because that is `EntityState`, the engine's half of every saved entity, and every field of it is optional, so a kind that gains a bag later still reads an old save.
`EntityState::leader` is by `SaveId` rather than by the brain it rides on, since a kind's `Brain` is content made again from its definition and not a thing to save: a follower whose leader was saved too follows it again, by the entity map, and one whose leader was not follows nobody rather than an entity that no longer exists.
A status a worn thing holds is saved apart from the rest, with the thing's save id, and put back held by the thing; one whose thing did not come back is left out rather than put back loose, since it lasted only while that thing was worn.
Remains are saved as whatever they were, and a prop kind a game laid on a body afterwards is part of what `EntityState` records of the remains, which is why `PropKind` answers false to `Saveable::TAKES_REMAINS`; a capture that finds one entity claimed by two kinds is refused with both named, since restoring it would spawn the thing twice.
`SaveableState` is the same bargain for a resource a game keeps of a run, with `capture` and `restore` on the resource itself, which must already exist when the save is restored.
`SaveableItem` and `SaveableActor` are the shorter bargain for an item or an actor the game makes through its `ItemMaker` or `ActorMaker`: the engine writes which definition it is, makes it again through the maker, and puts back only the game's `Extra`.
`SaveApp::save_kind::<K>`, `save_items::<K>`, `save_actors::<K>` and `save_state::<R>` register them, filing each under the last segment of its type name and panicking at build time when two would share one.
`PropKind`, the engine's `Stair`, `Quests` and `Counters` are the four the engine implements for itself, since it read those definitions out of a file or built them from facts and loot it owns; `SavePlugin` registers the first two, and a game with a quest tracker or a fact ledger adds `save_state::<Quests>()` or `save_state::<Counters>()`.
`RunSave` is the result: a `format`, the `EngineSave`, a `KindSave` per kind holding each entry's own RON, the `EntityState` of each, and the game's resources by name.
Capturing walks the kinds in registration order and each kind's living entities still in play in spawn order, leaving out the dead and the spent the frame keeps only so the log can name them, so one run writes the same bytes whatever order the archetypes are in; `restore` spawns each kind, binds the ids, puts the engine's state back on them, restores the game's resources and then the engine's own, into a world whose content resources the game's `PrepareRun` has already inserted; `RunSave::turn` is the whole turn it was saved on.
`EngineSave` is the engine's half: the run's `seed`, the clock, the queue as ids and readings, the map's edits, its built places and which is current, what the player has explored and the still things it last saw there, each saved entity's pools and cooldowns, its charges, the progress towards a worn thing's next `WornTimer` tick, and how many more times each of its triggers may fire, and every burning or gassed cell on every map.
`EngineSave::restore` puts every one of those back except the `seed`, which it only carries: continuing sets the `Seed` from it before `PrepareRun` runs, so the places a game prepares are built on the run's own seed.
Whoever held the turn is out of the queue when a save is taken, so it is put back at the front of the present and dealt first on the run that continues.
What is in the air when a save is taken, a shot, an ability or a thing's aimed use waiting to be seen to land, is not saved: what it cost is spent in the save and what it would have landed is lost, since every `Airborne` is emptied when its run ends.
An `Entity` means nothing in another process, so a save numbers entities as `SaveId`, handed out densely by `EntityRemap` while capturing and bound to fresh entities while restoring; a queue entry or a bag slot naming an id nobody bound is dropped rather than guessed at.
`Saves` is the backend as a resource, wrapping an `Arc<dyn SaveBackend>` of four synchronous methods over named slots, where a missing save is never an error and only real storage trouble is.
`FileBackend` writes one file per slot under a directory, `MemoryBackend` keeps them in a map for tests, `WebBackend` keys them into `localStorage`, and `Saves::platform_default(name)` picks the last on wasm and the first everywhere else.
`WebBackend` and the page-unload listener are the only surface behind `#[cfg(target_arch = "wasm32")]`, beyond the wasm arm inside each `platform_default`, so a desktop build has no `WebBackend` to name and a browser build still has a `FileBackend` with no filesystem under it.
`Versioned` is the envelope and `encode` and `decode` the pair that reads the version before it trusts the rest.
Two numbers are matched, both exactly: `SaveSlot::version`, which a game bumps when its own kinds change shape, and `RunSave::FORMAT`, which the engine bumps when the run's shape does.
`save_run` encodes the world, writes it to the slot and stashes it; `load_run` reads the slot back, answering `None` when nothing was saved there and a `SaveError` when what is there is not something this build reads.
`Stash` is the last encoding the game made, behind an `Arc<Mutex<_>>` so a handler running outside the app holds the same one, and a flush writes it and keeps it, since a browser may send both of its unload events.
The stash is re-encoded once per whole turn while playing rather than once a frame, so a window closed on a run loses at most the turn in hand, and forgetting the save deletes the slot and clears the stash together, and says in `SavedRun` that the slot is empty once it is.
`Ending` is what the engine knows of an end, inserted on the frame the run is over and gone when the next one begins: the `outcome`, the run's seed, the whole turn it ended on, and the game's `epitaph`.
`EndingView` is what the engine cannot know: `sections`, a `Vec` of an `EndingSection`'s `heading` and `body`, pushed with `EndingView::section` in `ViewSet::Annotate` the way a facet is pushed onto a row, and `summary`, the run in one line.
The ending view's collector empties it in `ViewSet::Collect`, so a game pushes on every frame it wants the rows rather than once, and a run begun again shows nothing until something pushes again.
Once the run is over it then fills it from the record `RecordsPlugin` filed for the run, when there is one, the summary and the record's sections ahead of anything a game pushes.
The game menu reads both: the epitaph and the summary wrapped, each only when there is one, the seed and the turn on one line, then each section's heading with the lines of its body under it, clipped to the rectangle the menu was given, so a section longer than the screen is cut rather than scrolled.

## Using it

A kind is a component and its account of itself, and the smallest is a marker on something the engine already keeps: a gate whose cell and destination the engine saves is `impl Saveable for Gate` with `type Saved = ()`, a `capture` that writes nothing and a `restore` that spawns the marker and its name again.

Registering is the step an implementation does not show, and it is the whole of a game's setup: the plugin at its version, `SavePlugin::new(slot).version(version)` with `.on_arrival()` for a save on every arrival, then `save_items`, `save_actors` and `save_kind` for every kind and `save_state` for every resource of the run.

A game that saves splits its start across the three schedules a run begins in: what every run stands on in `PrepareRun`, a fresh run's start in `NewRun`, and what a continued run gives back after the engine restored it in `RunResumed`, such as an upgrade's effect put back on the player it was taken by.

What a run was about is a section on the end screen, and a game with `RecordsPlugin` gets it from the `RecordRun { score, summary, sections }` it writes once the run is over, whose sections the screen shows ahead of anything a game pushes.

## The line

With the plugin added and nothing else registered, the engine saves the clock and the queue, the run's seed, every built place, what the player has explored and the still things it last saw there, the fire and the gas on every map, and what each actor has spent on abilities.
It saves every prop too, because it spawned them from definitions it can read again.
Everything else is the game's to register, since the engine cannot know what a monster or a sword is made of: what a kind is written down as, and what spawning one again means, are `Saveable`, or for an item or an actor the game makes through its maker, `SaveableItem` and `SaveableActor`, which write which definition it is and make it again through the maker, and what is not an entity is `SaveableState`.
The engine then puts its own half back on whatever the game spawned, so giving a kind a bag, a status or a stack later changes nothing about how that game saves it.
What a save guarantees is that the run goes on as the same run: the same things in the same places with the same health, bags, gear and statuses, the same clock reading and the same actor holding the turn.
The seed is the one thing the save writes down and does not restore as state: continuing sets it before the game's `PrepareRun`, so the places the run has not reached yet are built from the run's own seed, and a game that built its places in `NewRun` instead would build them for a fresh run only.
What it does not guarantee is that a run replayed from its start would arrive at the saved state, because a save is a position and not a record of the moves: every stream [Seeds and determinism](seeds.md) describes has been drawn from by the time the save is written, and a continued run draws from those streams afresh.
A save that does not fit is refused rather than repaired: either version failing to match is an error, and so is a save holding a kind or a resource this build does not register.
The one thing the load repairs on its own is content that has gone missing, and only for what the engine makes again from a definition, a prop, or an item or an actor through its maker: one whose definition this build has lost comes back as an empty entity with a warning rather than failing the whole save.
What is left of a run that cannot be continued is a screen, and a record in a slot of its own where a game keeps records: the run's slot is deleted when the run ends, and the ending is drawn for as long as the player looks at it.
The outcome, the seed and the turn are the engine's, since it kept them; every heading and every body is the game's, because only the game knows whether a run is measured in coin, in decks cleared or in what the player was made of.
Nothing in `atglyph-save` decides when a run is over, and nothing in `atglyph-ui` writes a byte to storage.
When the stash is refreshed is the engine's, and so are when the slot is deleted, when a run left for a title is written down and how a saved run is continued, because `SavePlugin` schedules all of it itself; what a game decides is when else a run is written down: on arrival, by asking for it, and whenever the `save_run` behind a key of its own is pressed.
A death deletes the slot and clears the stash together, so the way out has nothing left to write, which is what makes a death final rather than a suggestion.

## Where it lives

`atglyph-save` is tier 2 and sits on top of `atglyph-bevy` rather than beside it, because what a save is made of is the engine's own components: an `Inventory`, an `Equipped`, an `ActiveStatuses`, a `Transition`, a `Remains`.
What that buys is that each game's save walk was deleted rather than shared out: what every game used to write by hand is `EntityState` and `EngineSave`, and a game's own save file is its kinds and nothing else.
`backend.rs` is the only file that knows where bytes go, which is why a browser is one implementation of a four-method trait rather than a second path through the crate.
`versioned.rs` and `remap.rs` are plain functions over plain data, `remap.rs`'s `Entity` aside, so the version policy and the density and stability of a `SaveId` are tested as properties with no `App` anywhere near them.
This is also the one tier 2 crate `scripts/check-tiers.sh --wasm` builds, because the two wasm-only pieces are invisible to a native build and would otherwise rot unseen.
The ending is in `atglyph-ui` because it is a view, a collector and a presenter like every other screen, and nowhere near `atglyph-save`.
It was a `Morgue` there until 2026-09-23: the same sections, rendered and written to a file that nothing read while the game ran, which is how a presenter came to do file I/O and how a UI crate came to depend on the save crate for one screen.
Showing them instead of storing them cost the page a paragraph and `atglyph-ui` a dependency.
