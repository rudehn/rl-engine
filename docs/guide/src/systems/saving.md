<!-- documents:
     plugins: SavePlugin, UnloadPlugin
     files: crates/rl-save/src/run.rs
            crates/rl-save/src/engine.rs
            crates/rl-save/src/backend.rs
            crates/rl-save/src/unload.rs
            crates/rl-save/src/versioned.rs
            crates/rl-save/src/remap.rs
            crates/rl-save/src/morgue.rs
            crates/rl-ui/src/game_menu.rs
            crates/rl-bevy/src/state.rs
            crates/rl-bevy/src/plugin.rs
            crates/rl-bevy/src/world.rs
     fingerprint: 7b0de961 -->

# Saving and the morgue

A save is a run written down as text, and the engine writes most of it.
A game says only what each kind of thing it spawns is, in its own words; where that thing stands, what it carries, what it wears and what is on it are the engine's, and so are the clock, the queue, the map's edits and what the player has seen.
The morgue is the other half of an ending: the file a finished run leaves behind, written once and never read back into a game.
Both go to storage through one trait, so a game in a browser and a game on a desktop differ in how two resources were built and nowhere else.

## Turning it on

`SavePlugin::new(slot)` is the loop around a save, and `.version(n)` is the number every save is matched against.
It declares `needs::<Saves>`, the backend, with a hint naming `Saves::platform_default("my-game")`; it registers `PropKind` as a saved kind itself, inserts `SaveSlot` and the `Stash`, refreshes the stash in `Last`, and deletes the slot in the `EndRun` schedule and on entering `EngineState::Over`.
It registers no key: saving reads the whole world, so a game's save key is its own exclusive system and calls `save_run`.
`UnloadPlugin` is the other half and is added on its own, needing the same `Saves`: it writes whatever is stashed on the frame the app is told to exit, which is the frame a native window's close button produces, and in a browser it also installs a listener for the page being hidden or unloaded.
A game that wants a morgue inserts a `Morgue` and adds no plugin, because `GameMenuPanel` is what files one and a game with no menu files what it likes when it likes.
Nothing here is on by default, and a game that registers no kinds and adds neither plugin never reaches storage at all.

## The model

`Saveable` is the one trait a game writes, implemented on the component that marks a kind: `capture` writes an entity down as `Self::Saved`, and `restore` spawns one again from that, nowhere, carrying nothing, at full health.
Neither says anything about position, health, bags, slots, stacks, statuses, remains or where a transition leads, because that is `EntityState`, the engine's half of every saved entity, and every field of it is optional, so a kind that gains a bag later still reads an old save.
`SaveableState` is the same bargain for a resource a game keeps of a run, with `capture` and `restore` on the resource itself, which must already exist when the save is restored.
`AddSaveable::save_kind::<K>` and `save_state::<R>` register both into `SaveRegistry`, filing each under the last segment of its type name and panicking at build time when two would share one.
`PropKind` and `Quests` are the two the engine implements for itself, since it read those definitions out of a file and can read them again; `SavePlugin` registers the first, and a game with a quest tracker adds `save_state::<Quests>()`.
`RunSave` is the result: a `format`, the `EngineSave`, a `KindSave` per kind holding each entry's own RON, the `EntityState` of each, and the game's resources by name.
`RunSave::capture` walks the kinds in registration order and each kind's living entities in spawn order, so one run writes the same bytes whatever order the archetypes are in; `restore` spawns each kind, binds the ids, puts the engine's state back on them, restores the game's resources and then the engine's own, into a world whose content resources and whose `Seed` the game has already inserted.
`RunSave::state::<R>` reads one resource out of a save before anything is restored, for the part of a start that runs before the world exists, and `count_of` and `turn` are for a line in the log.
`EngineSave` is the engine's half: the run's `seed`, the clock, the queue as ids and readings, the map's edits and its built places, what the player has explored and which sites it has found, each saved entity's pools, cooldowns and charges, and every burning or gassed cell on every map.
`EngineSave::restore` puts every one of those back except the `seed`, which it only carries: a game reads `save.engine.seed` itself and inserts the `Seed` before it builds the world the continued run stands in.
Whoever held the turn is out of the queue when a save is taken, so it is put back at the front of the present and dealt first on the run that continues.
An `Entity` means nothing in another process, so a save numbers entities as `SaveId`, handed out densely by `EntityRemap` while capturing and bound to fresh entities while restoring; a queue entry or a bag slot naming an id nobody bound is dropped rather than guessed at.
`Saves` is the backend as a resource, wrapping an `Arc<dyn SaveBackend>` of four synchronous methods over named slots, where a missing save is never an error and only real storage trouble is.
`FileBackend` writes one file per slot under a directory, `MemoryBackend` keeps them in a map for tests, `WebBackend` keys them into `localStorage`, and `Saves::platform_default(name)` picks the last on wasm and the first everywhere else.
`WebBackend` and the page-unload listener are the only surface behind `#[cfg(target_arch = "wasm32")]`, beyond the wasm arm inside each `platform_default`, so a desktop build has no `WebBackend` to name and a browser build still has a `FileBackend` with no filesystem under it.
`Versioned` is the envelope and `encode` and `decode` the pair that reads the version before it trusts the rest.
Two numbers are matched, both exactly: `SaveSlot::version`, which a game bumps when its own kinds change shape, and `RunSave::FORMAT`, which the engine bumps when the run's shape does.
`save_run` encodes the world, writes it to the slot and stashes it; `load_run` reads the slot back, answering `None` when nothing was saved there and a `SaveError` when what is there is not something this build reads.
`Stash` is the last encoding the game made, behind an `Arc<Mutex<_>>` so a handler running outside the app holds the same one: `stash` replaces it, `clear` forgets it, `pending` says which slot is waiting, and `flush` writes it and keeps it, since a browser may send both of its unload events.
`refresh_stash` re-encodes once per whole turn while playing rather than once a frame, so a window closed on a run loses at most the turn in hand, and `forget_save` deletes the slot and clears the stash together.
`Morgue` is where a run's record is filed, over a `SaveBackend` of its own, holding the game's `title` and the sections a game pushes with `section` and the filing takes with `take_sections`.
`Morgue::platform_default(name, title)` chooses that backend the way `Saves` chooses its own, a `morgue` folder of text files beside the executable or browser storage keyed under `name:morgue`, which is the second of the two resources a platform decides.
`Obituary` is the file: a title, the seed, the turn, a one-line `outcome`, the game's `epitaph` and headed sections in order, with `slot` naming the file after the title, the seed and the turn so two runs never share one, and `render` giving the plain text.

## Using it

A kind is a component and its account of itself, and a stairway is the smallest one there is: the engine knows where it leads, so the game writes down the glyph and nothing more.

<!-- include: ../../../../examples/corsair/src/save.rs:kind -->
```rust,no_run
/// A stairway or a cave mouth: the engine knows where it leads, Corsair
/// only how it is drawn.
#[derive(Component, Debug, Clone, Copy)]
pub struct Stairway;

impl Saveable for Stairway {
    type Saved = char;

    fn capture(world: &World, entity: Entity) -> char {
        world.get::<rl_engine::rl_render::Glyph>(entity).map_or('>', |g| g.ch)
    }

    fn restore(world: &mut World, glyph: &char) -> Entity {
        world.spawn((Stairway, crate::places::stair_glyph(*glyph))).id()
    }
}
```

Registering is the step an implementation does not show, and it is the whole of a game's setup: the plugin at its version, then every kind and every resource.

<!-- include: ../../../../examples/corsair/src/save.rs:register -->
```rust,no_run
/// What the save is made of: the plugin that keeps it, the four kinds, and
/// the four resources.
pub fn register(app: &mut App) {
    app.add_plugins(SavePlugin::new(SLOT).version(VERSION))
        // Items before those who carry them is not required, since every
        // kind is spawned before any bag is filled, but it reads better.
        .save_kind::<ItemKind>()
        .save_kind::<MonsterKind>()
        .save_kind::<Captain>()
        .save_kind::<Stairway>()
        .save_state::<StartOptions>()
        .save_state::<Armory>()
        .save_state::<Bestiary>()
        .save_state::<Entrances>()
        .save_state::<Quests>();
}
```

## The line

With the plugin added and nothing else registered, the engine saves the clock and the queue, the run's seed, the surface's edits and every built place, what the player has explored and the sites it has found, the fire and the gas on every map, and what each actor has spent on abilities.
It saves every prop too, because it spawned them from definitions it can read again.
Everything else is the game's to register, since the engine cannot know what a monster or a sword is made of: what a kind is written down as, and what spawning one again means, are `Saveable`, and what is not an entity is `SaveableState`.
The engine then puts its own half back on whatever the game spawned, so giving a kind a bag, a status or a stack later changes nothing about how that game saves it.
What a save guarantees is that the run goes on as the same run: the same things in the same places with the same health, bags, gear and statuses, the same clock reading and the same actor holding the turn.
The seed is the one thing the save writes down and does not put back, and it is the game's to read: `EngineSave::restore` carries `seed` past without inserting a `Seed`, so a game that builds its world before reading `save.engine.seed` replays the saved edits onto regions generated from another seed, silently, and `places.md`'s account of how a region regenerates is what makes that wrong rather than merely different.
What it does not guarantee is that a run replayed from its start would arrive at the saved state, because a save is a position and not a record of the moves: every stream `seeds.md` describes has been drawn from by the time the save is written, and a continued run draws from those streams afresh.
A save that does not fit is refused rather than repaired: either version failing to match is an error, and so is a save holding a kind or a resource this build does not register.
The one thing the load repairs on its own is content that has gone missing, and only for props: a prop whose definition this build has lost comes back as an empty entity with a warning rather than failing the whole save.
The morgue is for the run that cannot be continued: the slot is deleted when the run ends, so the obituary is the only thing left of it, written for the player to read, keep or paste into a bug report.
Nothing in `rl-save` decides when a run is over or reads the log; `GameMenuPanel` composes the obituary on the frame the run ends, out of the `Ending`, the message log and the sheet, and files it with whatever a game pushed through `Morgue::section`.
When the stash is refreshed is the engine's, and so is when the slot is deleted, because `SavePlugin` schedules `forget_save` itself; what a game decides is when a run is written down, which is the `save_run` behind its own key.
So a game that clears the stash has cleared what the way out would have written, which is what makes a death final rather than a suggestion.

## Where it lives

`rl-save` is tier 2 and sits on top of `rl-bevy` rather than beside it, because what a save is made of is the engine's own components: an `Inventory`, an `Equipped`, an `Afflicted`, a `Transition`, a `Remains`.
What that buys is that each game's save walk was deleted rather than shared out: what every game used to write by hand is `EntityState` and `EngineSave`, and a game's own save file is its kinds and nothing else.
`backend.rs` is the only file that knows where bytes go, which is why a browser is one implementation of a four-method trait rather than a second path through the crate.
`versioned.rs` and `remap.rs` are plain functions over plain data, `remap.rs`'s `Entity` aside, so the version policy and the density and stability of a `SaveId` are tested as properties with no `App` anywhere near them.
This is also the one tier 2 crate `scripts/check-tiers.sh --wasm` builds, because the two wasm-only pieces are invisible to a native build and would otherwise rot unseen.
`Morgue` is here rather than in `rl-ui` because filing is storage; what an ending says is `rl-ui`'s, and the two meet at a single `file` call.
