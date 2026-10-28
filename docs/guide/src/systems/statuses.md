<!-- documents:
     plugins: FactsPlugin
     files: crates/atglyph-bevy/src/status.rs
            crates/atglyph-bevy/src/plugin.rs
            crates/atglyph-bevy/src/combat.rs
            crates/atglyph-bevy/src/events.rs
            crates/atglyph-bevy/src/items.rs
            crates/atglyph-bevy/src/effects/engine.rs
            crates/atglyph-bevy/src/parts.rs
            crates/atglyph-bevy/src/stealth.rs
            crates/atglyph-bevy/src/minds.rs
            crates/atglyph-bevy/src/turn.rs
            crates/atglyph-bevy/src/noise.rs
            crates/atglyph-rules/src/damage.rs
            crates/atglyph-rules/src/status.rs
            crates/atglyph-rules/src/stats.rs
            crates/atglyph-rules/src/events/fact.rs
            crates/atglyph-rules/src/events/ledger.rs
            crates/atglyph-rules/src/events/quest.rs
            crates/atglyph-ui/src/facet.rs
            crates/atglyph-ui/src/narrate.rs
     fingerprint: 8cd9dda2 -->

# Statuses

A status is a registered definition an actor carries for a number of whole turns: what it does to registered stats while it lasts, what damage it deals or mends each turn, whether it takes its holder's turns or puts it to sleep, what it resists, which side it fights for and how often its steps go astray.
Those are what the statuses act on, because each is something the engine already knows how to undo, resolve or spend; one more, whether nothing sees whoever holds it, is stealth's to act on.
Stats are the currency underneath, one registry of ids and four operations on them, which is also how gear and a game's own traits reach a number.
Facts are the other end of the same idea: an outcome as data, tallied into counters and matched against quests, so an achievement is a definition rather than a system.

## Turning it on

Statuses are part of `CorePlugin`: a game that registers none has nothing to tick, and without `Registries` at all nobody is afflicted. A tick's damage goes down the pipeline a sword's does, so it lands only in a game with `CombatPlugin`.
It registers `ApplyStatus`, `RemoveStatus` and `StatusEvent`, and applies, removes and ticks statuses in that order in `ResolveSet::Effects`, which runs before `ResolveSet::Damage` so a tick lands in the pass that produced it.
It takes the turns of a holder whose status skips them in `DecideSet::Sense`, where work claims a worker's turns, before any mind or player is asked, and turns a stumbling holder's chosen step in `DecideSet::Statuses`, after every mind and the game have decided and before anything resolves.
It wakes a sleeper to harm in `ReactSet::Settle`, and to a foe it notices in `DecideSet::Notice`, and keeps what a status lends its holder, its side, its resistances and its kind's changes while it holds one, in `ReactSet::Statuses`, read off the statuses every pass so a status a save put back does it all again with nothing else saved.
Every `Actor` is given an empty `ActiveStatuses` and an empty `StatBlock` the moment it is spawned, so a monster spawned without either still takes a status rather than shrugging it off.
`FactsPlugin` is the opposite shape: it registers `Happened` and `QuestChange`, feeds facts in `FactSet`, straight after `TurnSet::React` and before `TurnSet::Listen`, and again in `PostUpdate` for a fact written outside the turns, and asks for `Quests` or `Counters`, either of which will do.
Both are a run's, so in `EndRun` it puts every counter back to nought and every quest back where a fresh tracker over the same quests puts it, and a game may insert either once for every run rather than again in `NewRun`.
That is why it asserts on entering `Playing` rather than declaring a `needs`: a `needs` names one resource, and this plugin works with either of two.
A game that inserted neither has nothing listening and is told so, loudly, the moment play begins.

## The model

`StatusDef` is a `name`, a `stacking` rule, a `max` it stacks to, a list of `modifiers`, an optional `tick_damage` as a kind and an amount or a `tick_mend`, an optional `badge` of one character a panel may draw, the `resists` its holder has while it lasts, the `side` it fights for, the percent of its steps that `stumbles`, and four flags, each false unless a status file writes it `true` or its builder sets it: `unseen`, `boon`, `skips_turns` and `sleeps`.
A status that skips turns takes its holder's turns while it lasts: the engine claims the decision of an actor holding the turn while `Statuses::taking_turns` names a status on it, and spends the turn as a wait costs, so paralysis and a snare are each one line of a game's statuses.
Its mind is not asked what to do, though its sight is still recast and its noticing rolled; a held player's keys come to nothing, since the turn is spent before any intent resolves; and a worker caught at its work loses those turns rather than working them.
It ends when the status does, by its time or by a cure, so a status a blow breaks is the game's `RemoveStatus` on the blow.
A status that sleeps takes its holder's turns the same way, written `sleeps: true`, and ends itself the moment it is woken rather than waiting for the game's own `RemoveStatus`: `Sleepers::asleep` answers who is asleep by reading `Statuses::sleeping` off its current statuses, and `Statuses::wake` cures every one that sleeps and reports each with `StatusEvent::Cured`.
A sleeper senses less than a held actor still does: its sight is recast and its noticing rolled, but only within `SLEEPER_REACH`, two cells, and only by its chance there, never by its certain radius, so a hider can stand over one without waking it.
Harm wakes whoever it landed on this pass, armor stopped at nothing included, since it was felt; the blow that woke it was rolled on it as on the unaware.
On the sleeper's own turn, after noticing runs, sight wakes it to a foe it notices within that reach, by the same roll stealth's own noticing uses when stealth runs, or on sight alone when the foe does not hide or sees on sight regardless; a sleeper watches nobody outside the reach.
`NoisePlugin` adds a third, in `TurnSet::Listen`, which wakes whoever heard a noise this pass with its `Hearing` threshold left on it, so a sleeper with no hearing sleeps through any noise and a game with statuses and no noise has nothing to wake one from it.
An unseen status hides whoever holds it from every mind while it lasts and ends when its holder attacks, which `StealthPlugin` decides and the statuses know nothing of; without stealth the flag is carried and read by nobody.
A status naming a `side` turns its holder to that side when it comes on and puts it back the pass the last such status ends, keeping the side to go back to as `SideWas`, which is saved; one that `resists` is kept as the holder's `StatusResists`, added to its own and its gear's where a blow is weighed; and one that `stumbles` turns that percent of its holder's steps and bumps a random open way, drawn from a stream of its own.
`WhileHolding`, put on from `ActorParts::while_holding`, is what an actor's own kind changes about its stats while it holds a status, under `ModifierSource::Holding`, so burning halts one kind's mending and nobody else's.
`status::load` reads them from RON by name, resolving every stat and damage kind through `Names` and reporting every unknown name in the file at once, so a game authors statuses in the words its other content uses.
The definition is never deserialized as it stands, because its ids index registries a content file cannot see and a number written in one would land on a different stat the day the stat list is reordered.
`Stacking` is `Refresh`, where the longer duration wins and which is the default, `Extend`, where durations add, `Stack`, where a second instance sits beside the first up to the status's `max`, and `Ignore`.
`ActiveStatuses` is the `Statuses` an actor carries, each an `ActiveStatus` of an id, the whole turns left or `UNTIL_CURED`, an opaque source so a tick can credit whoever applied it, and an opaque `held_by`, the worn thing it lasts only while, if one holds it.
`ApplyStatus { target, status, turns, by, held_by }` puts one on and `RemoveStatus { target, status }` takes one off, and each answers with a `StatusEvent`: `Applied` for one that began, `Renewed` for one put on again while still on, its time refreshed or extended or another instance stacked beside it, `Expired` when the time ran out, `Cured` when it was lifted.
A put-on that changed nothing, a refresh no longer than what is left or an ignored second, writes neither, and a reader that tells only `Applied`, as the narrator does, tells a cloud's status once, when it begins, and says nothing of a stack's count rising.
`Statuses::apply` installs the definition's modifiers under a `ModifierSource::Status` tagged with the id and the instance, which is what makes removal exact when the same status stacks three deep.
`Statuses::apply_held` does the same held by a thing, and a refresh or an extension it makes is held by that thing from then on, while one by `apply` leaves the holder as it was.
The statuses never act on the holder, only read it to choose `apply_held` over `apply`: the items plugin writes a `RemoveStatus` for every held status whose thing its holder is not wearing, in `ResolveSet::Effects` before statuses are applied, so it ends in the pass the thing comes off, and one landed on somebody not wearing the thing ends on the next pass.
`Statuses::tick` collects what every status deals into a `TickReport`, then takes a turn off each and strips the modifiers of whatever ran out.
A status put on for `UNTIL_CURED` turns bites and mends like any other and is never counted down, so only a cure ends it.
The engine ticks once per `TurnEnd` and only for actors on the current map, so a monster on a floor nobody is standing on does not burn down while the player is elsewhere.
Its damage becomes a `DamageEvent` carrying `Hit::from_status`, which names the status and credits whoever applied it but leaves `attacker` empty, so a poison tick cannot set off the riders a blow would.
That event is built with `DamageEvent::new`, so its `DamageRoute` is `Effect` and rides through to `DamageDealt` as one: a game hanging a rule off a blow can tell a blow from a tick without reading `status` at all.
A status that mends says `mends`, its `tick_mend`, and its tick becomes a `Heal` by whoever applied it, which is the whole of what makes regeneration a status like poison; a negative `ticks` is refused at load, since mending is not damage of any kind.
`StatBlock` is the `Stats` underneath: a per-actor base for whichever stats the game overrode, and a flat list of `Modifier`s each carrying a `ModifierSource`.
`Stats::value` is base plus every `Add`, then every `MulPct` compounded, then `AtLeast` and `AtMost`, then the definition's own `min` and `max`.
The `ModifierSource` is a tagged value rather than an opaque number because several systems fold modifiers into one `Stats` without knowing about each other: statuses remove theirs one instance at a time, the gear fold strips every item's and puts the worn ones back, a kind's changes while holding sit under `Holding`, and a game's own sit under `ModifierSource::Game` where nothing in the engine touches them.
A `Fact` is a registered `FactKind`, an optional subject, an optional object and an amount that is one unless the fact is about a quantity, all of it in the game's own numbering.
A `Matcher` picks facts out by kind and optionally by subject and object, and a fact with no subject is not about anyone.
`Ledger` is a value per registered `CounterDef` and a list of `Tally` rules, and feeding it a fact adds that fact's amount to every counter whose rule matches; `reset` puts every value back to nought and keeps the rules.
`Tracker` is where every quest stands: an `Objective` is a matcher and a `Need`, either a `Total` the matching amounts add up to or a `Latest` a single fact must reach.
A `QuestDef` opens when every quest in its `after` is done and is done when every objective is, and `victory` on it says the run is won: the engine ends the run with a `RunOver` in the pass whose fact finished it, saying the quest's `ending`.
`Tracker::feed` reports what one fact changed, in the order it happened: `Progress`, then `ObjectiveDone`, then `QuestDone`, then `QuestOpened`.
`Happened` wraps a fact for the engine, which feeds it to whichever of the tracker and the ledger the game inserted, and each change comes back as a `QuestChange` written in the pass whose fact moved it, so a system in `TurnSet::Record` reads it there and one in `Update` once the frame's turns are done.

## Using it

A status is a registry entry, `StatusDef::new(name)` and a builder for each thing it does: `.ticks(kind, 1)` burns its holder a point of `kind` a turn, `.mends(n).boon()` heals one that is glad of it, `.unseen().boon()` hides one until it strikes, and a `badge` is the glyph a health bar shows for it.

A fact is the game's reading of an engine message, which is the only translation quests and counters need: a system reading each `DeathEvent` writes `Happened(Fact::new(boss_slain).about(kind))` for one whose kind the game calls a boss, and passes the rest over.

## The line

The status plugin acts on two things a status says: it installs and removes the stat modifiers, and it turns the tick into a hit; `unseen` is read by stealth, because who can see whom is stealth's question.
Whether a status outlasts the thing that gave it is the game's to write, as `while_worn` on the `Inflict` that lands it, and the engine's to keep: which way it came off never matters, only whether the thing is still worn.
A status that lasts for what a body is, or for as long as a worn thing is worn, is put on for `UNTIL_CURED` turns rather than flagged beside its count, so a save keeps it as it keeps any count and a refresh or an extension leaves it as it is.
A status that holds is the engine's to carry out and the game's to write, because a turn is the turn loop's: a game skipping a paralysed monster's mind itself would still leave the player's keys working and a worker's toil going on, and an actor that is not to act at all is held this way rather than slowed to nothing.
Everything richer is the game's, keyed by the id: a status that silences an ability, one that walls a door, one that turns a body to stone is a system reading `StatusEvent` or `ActiveStatuses` and doing the rest.
Which sides there are and who is hostile to whom stay the game's even for a status that turns a side: the status only names which.
Whether anything is inflicted at all is the game's too, and the shape the randomness rule points at is a system reading `DamageDealt` and writing `ApplyStatus` with a chance drawn from the game's own stream, never the engine's, so a rule a game adds cannot shift the dice of the blows the engine has yet to throw.
The engine also never decides that a status is worth saying out loud: it writes the three events and a game turns the ones about its player into words, which is why every phrase about an affliction lives in a game or in the narrator's table.
`boon` is the one thing a status says for the words alone: the rules never read it, and the narrator tells a boon landing as good news and its end as a note, so a cloak is not announced in the tone of a wound.
What a panel shows of a status is the same division: `badge` is the one character the engine offers, and anything else is a `Facet` the game pushes onto a row in `ViewSet::Annotate`, `Facet::new("key", text)`, toned as the game likes.
A facet is an escape hatch and its use is a signal, so two games pushing the same key is the argument for putting that field in the view instead.
A stat is content, which is the reason nothing in the engine names one: `CombatRules` is handed the stat it should read as armor, `TurnRules` the one it should read as speed, a worn thing names the stat it lends, and a status names the stat it moves, so haste is a status on a stat and the queue never learns what haste is.
Facts are the same refusal one level up: nothing in the engine names a fact, a counter or a quest, and the subject and object are opaque numbers the game chose, usually a definition id.
A game therefore writes one system that turns the messages it cares about into facts, and every achievement, generated objective and run summary after that is data over the same stream.
The one thing the engine insists on is when: facts are fed after the pass's reactions have all landed, so a quest finishes in the pass that raised its fact, and a victory is the run's first word on its end.

## Where it lives

`atglyph-rules` is tier 1 and has no Bevy in it, which is what lets the awkward parts be proved without an `App`.
`status.rs` holds the stacking rules and the tick, so a status applied three times, extended, and expiring one instance at a time is a test over a `Statuses` and a `Stats` built by hand.
`stats.rs` holds the order of operations and the source tagging, and the test that matters is the one where an item's modifier, a status's and a game's share a number and only the item's is stripped.
`events/` is `fact.rs`, `ledger.rs` and `quest.rs`, and none of the three knows what a fact is about, so a quest chain is tested by feeding facts made of integers.
`atglyph-bevy` is tier 2 and thin over both: `status.rs` is the components, the two requests and the systems that apply, tick, wake and keep what statuses lend, and `events.rs` is a message, two optional resources, one system that feeds them and one that forgets them when a run ends.
Thin on purpose, because the parts worth testing are the parts that never needed a world.
