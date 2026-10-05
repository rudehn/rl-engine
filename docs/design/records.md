# Run records

What a run came to, kept once it is over, and the list of every run that has ended.
Built for Undermere, whose records screen needs its runs kept between launches, and in the engine because no game kept its runs and the second one to want it would have copied the first.

The reference page is `docs/guide/src/systems/records.md`; this is why it is shaped that way.

## What was there

The screen a run ends on drew the engine's `Ending`, the outcome, the seed and the turn, and under it whatever sections a game pushed onto `EndingView` every frame.
Nothing outlived that screen: a new run cleared the view, and a closed window took the run's account with it.
Heist pushed the coin carried out and no other game pushed anything; Undermere wanted an itemized score on the end screen and the same score in a list of runs a title opens, kept between launches.

## A record is the game's words

`RunRecord` is the game's score, its one line about the run and its sections, beside the outcome, the seed, the turn and the time the engine fills in.

- **Not the engine's counters.**
  The engine could have kept a record of its own: kills from `DeathEvent`, depth from `PlaceEntered`, turns from the clock, all in `Counters` or a table of its own.
  It was rejected because what a run is worth is a game's design, not a tally: Undermere scores depth, the threat of what was slain, treasure, mini-bosses and victory, a heist the coin carried out, and a game of decks cleared would score neither.
  A record built from counters would make every game's records the same table, and a game would register counters it never reads only so its records said something.
- **Not an opaque blob.**
  A record could have been bytes the game serializes and draws itself, which would let a game keep anything.
  It was rejected because the records screen would then be the game's to write, which is the screen every game would copy; a summary, a score and sections of heading and body are what both screens draw, and anything finer goes in a section's words.
- **The engine's half is not asked twice.**
  The outcome and the turn are read off `Ending` and the seed off the run's `Seed`, so a game writes `RecordRun { score, summary, sections }` and never a number the engine already holds.
  The engine says the seed and the turn on both screens, in its own words, so a game's sections leave them out and a run's seed is said once.
- **The score is an `i64`, and only the score sorts.**
  A game whose score is a running total writes the total; one with no score writes nothing better than another and the list falls back to the newest first among ties.

## Written once, at the end

`RecordRun` is a message a game writes when its run is over, not a resource kept current through the run.
How a game counts its run on the way is its own business, saved with its run if it wants Continue to keep it, and the record is what the count came to.
A game writes it from whatever ends its run, often the same pass that writes `RunOver`, before `end_runs` in `PostUpdate` has turned that into an `Ending`.
So `record_runs` keeps what arrives early and completes it the first frame there is an `Ending`, rather than dropping a record because it came first, or making every game order its system after the engine's.

## A slot apart from the run's

`RecordsSavePlugin` keeps the list in a `records` slot of its own, as `SettingsSavePlugin` keeps a player's choices.

- **Not in the run's save.**
  The run's slot is deleted by the death or the win a record is filed for, and given up by New Game; the list has to outlive every run, so it cannot live in a slot every run ends by deleting.
- **Not in the settings slot.**
  That is a map of names to words, read name by name; a list of records is another shape, and a damaged one should cost the records and never the player's choices.
- **Not the versioned envelope.**
  A run's save is refused whole when its version differs, which is right for entities saved in a game's own shapes.
  A hundred runs outlast one build, so every field of a record defaults and a field a build does not know is passed over: a record an earlier or a later build wrote reads as far as it can.
- **A list that will not parse is replaced, not merged.**
  A map of names can be recalled name by name whatever else is wrong with it; a list that will not parse cannot be added to entry by entry.
  So the slot is warned about and left alone, the list starts empty, and the next record filed writes a fresh list over it, losing what could not be read rather than refusing to record again.
- **Written when it changes**, compared against what the slot is known to hold, so a frame that filed nothing writes nothing.

## The end screen reads it

`RunRecords::this_run` is the record filed for the run that is ending, and `collect_ending` reads it: the end screen says the record's summary under its title and draws the record's sections, ahead of anything a game still pushes.

- **One text, two screens.**
  A game that pushed its sections onto the end screen and wrote them into a record as well would have two accounts of one run, and nothing would keep them alike.
  Reading the record is what makes the end screen and the records screen agree by construction.
- **Kept until the teardown.**
  `this_run` is cleared in `EndRun`, not with the whole resource and not in `NewRun`, so the record stands through every frame the ended run's screen reads it and is gone before the next run begins, while the list itself is never cleared by a run.
- **A game with no records is unchanged.**
  `collect_ending` reads `RunRecords` as optional, so Foundry, which records nothing, draws the end screen it always drew.

## The list

Best score first, a tie keeping the newer record ahead, and a hundred at most, so the list is the hundred best rather than the first hundred.
Best first because a list of runs is read for the best of them; the date on each row is what says when.
A hundred is long enough that filling it means something and short enough to read in a screen or two; the screen scrolls inside the rectangle a game gave it rather than growing to fit.
The date is a few lines of civil-calendar arithmetic in the panel, `YYYY-MM-DD` from seconds since the epoch, since one date a row does not earn a calendar dependency, and a record made on wasm, where the engine reads no wall clock, has none.

## Split three ways

The record is plain data in `rl-bevy`, the slot is `rl-save`'s and the screen `rl-ui`'s, with no edge between the last two, the split settings and the title make.
The title offers its Records row knowing only that a screen declared the `records` modal and that the list holds something.

## What waits

- **A run given up leaves no record.**
  New Game over a save forgets the run through `AbandonRun`, and the menu's new run is a `Restart`; neither ends the run with a `RunOver`, so there is no `Ending` to complete a record against, and `RecordOutcome::Abandoned` is filed only for a game that ends its own run as given up.
- **The list is read one way.**
  No sorting by date or by outcome, no record deleted by the player, and no filter per game mode; each waits on a game that wants it.
- **A record holds words, not a replay.**
  The seed is kept, so a run can be played again on it, but the keys are not; a run written down with `RL_RECORD` is the replay, and a record does not point at one.
