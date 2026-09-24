<!-- documents:
     plugins: NarratorPlugin, NarrationViewPlugin
     files: crates/rl-ui/src/narrate.rs
            crates/rl-ui/src/log.rs
            crates/rl-ui/src/tone.rs
            crates/rl-ui/src/lib.rs
            crates/rl-bevy/src/combat.rs
            crates/rl-bevy/src/plugin.rs
     fingerprint: b5d9a49e -->

# Narration

Every game with a log writes the same system: read the blows and the deaths, branch on whether the player did it or had it done to it, pick a tone, push a line.
Ten copies of that, each getting the order wrong when two monsters act in one frame, and each narrating blows nobody saw.
So narration is a view, a collector and a presenter, the split every panel has, with one twist in where the collector runs: inside the turn rather than inside the frame, because that is the only place the order is knowable.

## Turning it on

`NarrationViewPlugin` keeps the view current and speaks nothing: it registers every message it reads, so a game without the plugin that raises one still has an empty buffer rather than a missing resource, and adds `collect_narration` to the `Turn` schedule in `TurnSet::Record`.
That set is its own phase rather than a reader in `TurnSet::React`, because a reader there races the game's own reactions and whichever the executor ran first would decide whether a game's line landed in this pass or trailed into the next; it is before `TurnSet::Cleanup`, so the dead still stand where they fell when the row is made.
`NarratorPlugin` is the presenter, and it adds `NarrationViewPlugin` if the game has not, so a game that wants the engine's words adds one plugin and a game that wants the rows adds the other.
It inserts its `Phrasebook` and runs `speak` in `ViewSet::Speak`, the last of the four view phases, which is after `ViewSet::Annotate` and therefore after the game has had its say about the rows.
Both declare `depends_on::<UiPlugin>`, which is where the `MessageLog` the narrator writes into lives.
A game changes a phrase, silences one or asks for the unseen to be spoken by building the plugin: `NarratorPlugin::default().phrase(..)`, `.silence(..)` and `.speaking_the_unseen()`, or through the `Phrasebook` resource afterwards.

## The model

`NarrationView` is the rows a pass produced, oldest first, spoken and cleared once a frame.
A `Said` is one thing that happened as the narrator reads it: its `words`, a `who`, a `whom` and a `what`, a `named` for a registry's word, a `detail`, an `amount`, an `at`, whether it was `seen`, whether the doer was `who_seen`, and the `turn`.
`Words` is either a `Phrase`, one of the engine's own events, or `Own { text, tone }`, a game's line already worded.
Two kinds rather than a phrase a game may add to, because `Phrase` enumerates what the engine raises and nothing else, and a game's line needs no entry in a table, only a place in the order.
`Phrase` is closed for the same reason, and it is split by perspective and by how the damage arrived: `YouHit`, `HitsYou` and `OthersFight` for a blow, `YouShoot`, `ShootsYou` and `OthersShoot` for a shot, each of the six with a twin for the one that got through nothing, so no grammar and no branch on who did it lives in the engine.
What tells a shot from a blow is `Reach`, which rides `DamageEvent` down the pipeline to `DamageDealt` untouched: only `Shot` is worded as one, and `Melee`, `Thrown` and `Effect` keep the blow's words, since a bolt or a poison is already narrated by whatever cast or inflicted it.
`called` is what `who`, `whom` and `what` were called when the row was made, filled in by the collector, because a row is made inside the pass and spoken after it and things change in between: what dies becomes remains and is renamed, what is thrown merges into a stack.
A use is named as one of the thing, `You use a stim.`, since the stack already counts one fewer, and only a use of a thing with a `use` trigger is said at all: what using anything else means is the game's to say, the way a crust of bread is.
`seen` is whether the player saw it, which is either that it happened to the player or that it happened where the player can see, and `Phrasebook::speak_unseen` decides whether an unseen row is spoken at all.
`who_seen` is the narrower fact beside it, whether the doer's own cell was in the player's sight, and `render` takes as an argument whether to honour it, since what to do about an unseen doer is the presenter's setting rather than the row's business.
A doer that may not be named is `UNSEEN`, the one word `something`, and `speak` names it anyway while `speak_unseen` is on, because a game that asked for the unseen to be narrated asked for it named.
The two were one field until a shot out of an unlit room named the shooter, a thing the player had never laid eyes on: being shot from the dark is always worth telling you, which is what `seen` answers, and that answer is not permission to say who fired.
`Tell` is a game's own line told in its place among what the turns did: a template, a tone, and up to three entities for its placeholders, with `by`, `to` and `about` to name them.
It is written from inside a pass, usually in `TurnSet::React`, and the collector reads it with that pass's events and after them, so it lands in the log below what it answers and above whatever the next actor does.
A line written outside the turns, the one a run opens with or a key refused before any turn is spent, has no pass to wait for and goes to the `MessageLog` directly.
`Phrasebook` is the words for every phrase, one entry per phrase with a tone of its own, and `set`, `silence` and `get` are all of it.
The placeholders are `{who}` and `{whom}`, which are `you` or `the <Name>`, `{what}`, which is a thing's name said with its stack's count, `{n}` for the amount, `{named}` for a registry's word and `{detail}` for whatever more there is to say, each capitalised by writing the key capitalised.
`render` fills a template and answers with the text and a `Span` per name that has a colour, and an unknown key is left in the text as it was written rather than dropped.
`speak` walks the view, skips what was not seen unless the book says otherwise, looks each phrase up, renders it, and pushes it to the log with its tone and its turn.
The log is a bounded ring of `LogEntry`, each a text, a tone, a turn, a count and its spans, and a line repeated folds into the one above it with a count rather than filling the panel, because a four-line log filled by one event is a log that has stopped reporting.
A `Span` is a run of a line's characters in a colour of its own, which is how a name in the log wears the colour of the thing it names; that colour is content the way a glyph's is, not a tone, and a presenter keeps it legible against its surface with `readable`.
`ToneId` is the other half: a semantic role interned in `Tones`, coloured by the `Palette`, so a game recolours every line the narrator will ever speak by replacing one resource.

## Using it

The engine's words are one plugin, and changing any of them is one call on it.

<!-- include: ../../../../examples/tutorial/src/bin/step10_panels.rs:narrator -->
```rust,no_run
        // The engine narrates blows, deaths and pickups into the log, naming
        // things in their own colours. Warren changes one phrase: what a rat
        // does to you is a bite.
        .add_plugins(NarratorPlugin::default().phrase(Phrase::HitsYou, "{Who} bites you for {n}.", Tones::BAD))
```

A line the engine has no phrase for is a `Tell`, written from inside the pass it answers.

<!-- include: ../../../../examples/foundry/src/droids/alarm.rs:tell -->
```rust,no_run
/// Says in the log that a probe sounds the alarm, for every [`Noticed`]
/// whose observer carries [`Alarm`]: once on the flip from unaware, which
/// is when the engine writes one, and not on every shout after.
pub fn sound_alarm(mut noticed: MessageReader<Noticed>, alarmed: Query<(), With<Alarm>>, mut tell: MessageWriter<Tell>) {
    for ev in noticed.read() {
        if alarmed.contains(ev.observer) {
            tell.write(Tell::new("{Who} sounds an alarm.", Tones::BAD).by(ev.observer));
        }
    }
}
```

## The line

The engine has no vocabulary of its own, so a line gets its words from exactly two places and neither is a type in an engine crate.
The first is the `Phrasebook`, a table of English a game may overwrite entry by entry; the default is the English the games spoke before the narrator existed, and nothing reads it but the presenter.
The second is a `Name` on an entity, which is the game's word for the thing, reaching the line through a placeholder and wearing the thing's own colour.
That is why `Phrase` can be a closed enum without breaking the no-theme-words rule: it enumerates the events the engine raises, `YouHit`, `NoticesYou`, `YouAreAfflicted`, and every one of those is a shape rather than a subject.
A game's own events are the game's to narrate, and the engine will not guess: it offers a `Tell` and the order to put it in, and asks for the words.
The order is the part worth taking, and it is the part a game cannot easily get right: one pass is one actor's action, so reading a pass's events in a fixed order gives the true order across a frame of many turns.
A collector in the drawing phase sees a whole frame's buffers at once and cannot know which blow followed which cast, which is the bug this exists to make impossible.
Within a pass the order is by kind and it is deliberate: a use before the blows it landed, blows before the deaths they caused, and a game's `Tell`s last, since they answer what the pass did.
The one exception is a notice by whoever holds the turn, which is read first, because that was rolled as the actor looked round before it did anything; a notice by anyone else came of what the turn did and keeps its place after the blows.
What the engine decides about a row is what it can know: who, to whom, with what, how much, where, whether the player was in a position to see it, and whether whoever did it was in sight to be named.
What it cannot know is a game's line, so `Tell` carries no tone the engine picked and is always spoken: the game chose to say it, so whether the player saw who it names is the game's to have weighed.
Between the two sits `ViewSet::Annotate`, where a game may edit or remove rows before they are spoken, and behind both sits the view itself, which a game may read and speak in its own words with no presenter at all.
What the engine does not get is grammar: no pluralisation of a verb, no agreement, no articles worked out from a name, and no second language.
A template with a placeholder in it is the whole mechanism, because the alternative is a grammar engine that every game would fight.

## Where it lives

Both halves are in `rl-ui` and tier 2, because a row names an `Entity` and the words are read off components.
The collector and the presenter are one module rather than two, since the twist that makes narration different from a panel is exactly the relation between them, and splitting the file would hide it.
The log is a module of its own, and it is on `UiPlugin` rather than on either plugin here, because a game writes to it from its own systems whether or not anything narrates or draws: a headless test adds the base plugin and has a log.
`MessageLog` holds no `Color` except inside a `Span`, and a `Span`'s colour comes off a `Glyph`, which is content; everything else is a `ToneId`, which is why the same rows read correctly in a palette the narrator never heard of.
Nothing here is in a tier 1 crate, and nothing needs to be: there is no arithmetic in narration, only order, and order is tested by running a pass.
