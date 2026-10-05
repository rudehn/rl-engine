<!-- documents:
     plugins: RecordsPlugin, RecordsSavePlugin, RecordsPanel
     files: crates/rl-bevy/src/records.rs
            crates/rl-save/src/records.rs
            crates/rl-ui/src/panel/records.rs
            crates/rl-ui/src/view/ending.rs
            crates/rl-ui/src/game_menu.rs
            crates/rl-ui/src/panel/title.rs
     fingerprint: b6db979e -->

# Run records

A record is what a run came to, kept once the run is over: the game's score, its one line about how the run went and its own account in sections, beside the outcome, the seed and the turn the engine already knows.
The engine files every record in one list, best score first, keeps the list between launches in a slot of its own, and shows it on a screen the title opens.
The screen a run ends on reads the same record, so what it says of a run and what the records list says of it are one text.

## Turning it on

`RecordsPlugin` is the record itself: it registers the `RecordRun` message, initialises `RunRecords`, completes and files each record in `Update` ahead of `EngineSet::Present`, so whatever draws a record has it the frame it is filed, and forgets which record is this run's in `EndRun`.
A game writes one `RecordRun` when its run is over, and a game that never writes one keeps no records.
`RecordsSavePlugin`, in `rl-save`, keeps the list between launches: it needs `Saves`, says so through `app.needs`, reads the `records` slot in its `finish`, before the first frame, and writes it in `Last` on a frame the list changed.
Without it the list lasts as long as the process.
`RecordsPanel::new(rect)` is the screen: it declares the `records` modal, checks for `UiPlugin` in its `finish`, reads its keys ahead of `EngineSet::Input` and draws after `EngineSet::Present`, outside the engine's sets, because a title opens it before there is a world.
The screen a run ends on takes nothing more: the `EndingViewPlugin` that `GameMenuPanel` adds reads this run's record whenever there is a `RunRecords` to read it from.

## The model

`RecordRun` is the game's half: a `score`, a `summary` and `sections`, each section a `RecordSection` of a `heading` and a `body`.
`RunRecord` is the whole: the game's three, the `outcome` as a `RecordOutcome` of `Won`, `Died` or `Abandoned`, the run's `seed`, the whole `turn` it ended on, and `when`, seconds since the Unix epoch, or `None` on wasm, where the engine has no wall clock to read.
`record_runs` completes each `RecordRun` once there is an `Ending`: the outcome and the turn from the `Ending`, the seed from the run's `Seed`, and the time from the clock.
A record written in the same pass as the `RunOver` it answers arrives before the `Ending` does, so it waits for one rather than being dropped, however many frames that takes, as long as its run stands.
A record still waiting when its run is torn down goes with it in `EndRun`: filed if the run had ended, since the `Ending` is still there, and otherwise dropped with a warning, never filed against the next run's ending.
`RunRecords` holds `records`, best score first, and `this_run`, the record filed for the run that is ending.
`RunRecords::file` puts a record in its place, a tie keeping the newer ahead, and drops whatever falls past `MAX_RECORDS`, a hundred, so the list keeps the hundred best rather than the first hundred.
`this_run` is cleared in `EndRun`, so the record stands for every frame the ended run's screen is up and is gone before the next run begins, while `records` is never cleared by a run.
`collect_ending` reads it in `ViewSet::Collect`: `EndingView::summary` is its summary, and its sections are pushed ahead of any a game pushes in `ViewSet::Annotate`.
So the end screen says the run's line, when a record was filed, under its title and any epitaph, then the seed and the turn, then the sections.
`RecordsSavePlugin` writes the list as RON in the `records` slot, apart from the run's slot and never deleted with it, and only when the list differs from what the slot is known to hold.
Every field of `RunRecord` is `#[serde(default)]` and a field it does not know is passed over, so a record an earlier or a later build wrote reads as far as it can.
A slot that does not parse is warned about and left as it is with the list starting empty, and the next record filed writes a fresh list over it.
`RecordsPanel` lists the records in its rectangle, one row each as `rank. score  summary  date`, the date `YYYY-MM-DD`, or nothing for a record with no time, scrolled to keep the row picked out in view.
The direction keys walk the list and wrap, and confirm opens the record picked out: its summary in the frame's title, the seed and the turn under it, then its sections drawn the way the end screen draws them, scrolled by the direction keys once they run past the rectangle.
Close goes back from a record to the list, and from the list closes the screen.
`RecordsScreen` is which record is picked out, whether it is open and how far it is scrolled, reset each time the screen opens, so the key that opened it is never read as its first move.
`RecordsPanel::title` and `empty` change the list's border and what it says with no records, and `records_modal` is the id a game's own menu opens it by.
`TitlePanel` draws a Records row once the `records` modal is declared, and lets it be taken once the list holds a record.

## Using it

A game writes its record once, when its run is over, in its own words.

<!-- include: ../../../../examples/undermere/src/tally.rs:record -->
```rust,no_run
/// Writes the run down once it is over: one [`RecordRun`] with its
/// score, its summary and its two sections, which the engine completes
/// with the outcome, the seed and the clock, files, and shows on the end
/// screen.
///
/// On entering `EngineState::Over`, the one door every ending comes
/// through, the boss's death and the adventurer's alike, and the frame the
/// engine's `Ending` is first there to read. Nothing in the run moves once
/// it is over, so the tally read here is the run's last word, and the
/// adventurer still stands where it ended.
pub fn record(ending: Option<Res<Ending>>, tally: Res<Tally>, map: Option<Res<WorldMap>>, credit: Credit, mut records: MessageWriter<RecordRun>) {
    let (Some(ending), Some(map)) = (ending, map) else { return };
    let by = match ending.outcome {
        Outcome::Died { by } => credit.of(by),
        _ => None,
    };
    let lines = score(&tally, ending.outcome == Outcome::Won);
    records.write(RecordRun {
        score: total(&lines),
        summary: summary(ending.outcome, by.as_deref(), map.current(), &tally),
        sections: sections(&lines, &tally, ending.turn),
    });
}
```

Keeping the list between launches is one more plugin beside the ones a game already saves its run with.

<!-- include: ../../../../examples/undermere/src/save.rs:register -->
```rust,no_run
/// What the save is made of: the engine's plugins, the four kinds, and
/// the two resources; and the records, in their own slot.
pub fn register(app: &mut App) {
    app.add_plugins((SavePlugin::new(SLOT).version(VERSION).on_arrival(), UnloadPlugin, RecordsSavePlugin))
        .save_kind::<ItemKind>()
        .save_kind::<Kind>()
        .save_kind::<Adventurer>()
        .save_kind::<Way>()
        .save_state::<Tally>()
        .save_state::<Quests>();
}
```

## The line

The engine decides what is the same in every game's records: that a run ends in one, how the list is ordered and how long it grows, where it is kept, and the screen that lists it.
The game decides what a run is worth and what it is said to have been: the score, the summary and every section are its own words, because only the game knows whether a run is measured in floors, in coin or in a king slain.
So a record is the game's account rather than a dump of the engine's counters: the engine fills in only what it already holds, the outcome, the seed and the turn, and never adds a line to a game's sections.
A record is written once, when the run is over, and how the game tallies its score on the way is the game's own business.
The list lives apart from the run's slot because it outlives every run: the death or the win a record is filed for is also what deletes the run's slot.
It is not the versioned envelope a run is saved in, which refuses a save whole when the version differs; a hundred runs outlast one build, so a record is read for what it still says.
The end screen reads the record rather than having the game push the same sections a second time, so the two screens cannot disagree about what a run came to.
A game may still push sections of its own onto `EndingView`, which the end screen shows after the record's, for anything it wants said and not kept.

## Where it lives

The record is plain data in `rl-bevy`, the crate the save and the screen both sit on, so `rl-save` keeps it and `rl-ui` shows it with no edge between the two, the split settings make.
Ordering and the cap are `RunRecords::file` over a `Vec`, tested with no `App`, and completing a record is one system, tested on a headless app with no window.
The slot is plain RON through a `Saves` backend, so a test round-trips a record through a `MemoryBackend` and writes a damaged slot as a string.
The date on a row is a few lines of calendar arithmetic in the panel rather than a calendar dependency.
`docs/design/records.md` is why it is shaped this way.
