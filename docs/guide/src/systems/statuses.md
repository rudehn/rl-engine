<!-- documents:
     plugins: StatusPlugin, FactsPlugin
     files: crates/rl-bevy/src/status.rs
            crates/rl-bevy/src/combat.rs
            crates/rl-bevy/src/events.rs
            crates/rl-bevy/src/items.rs
            crates/rl-bevy/src/plugin.rs
            crates/rl-bevy/src/turn.rs
            crates/rl-rules/src/damage.rs
            crates/rl-rules/src/status.rs
            crates/rl-rules/src/stats.rs
            crates/rl-rules/src/events/fact.rs
            crates/rl-rules/src/events/ledger.rs
            crates/rl-rules/src/events/quest.rs
            crates/rl-ui/src/facet.rs
     fingerprint: 6dcb705e -->

# Statuses

A status is a registered definition an actor carries for a number of whole turns: what it does to registered stats while it lasts, and what damage it deals each turn.
Those two are all the engine acts on, because they are the two it already knows how to undo and how to resolve.
Stats are the currency underneath, one registry of ids and four operations on them, which is also how gear and a game's own traits reach a number.
Facts are the other end of the same idea: an outcome as data, tallied into counters and matched against quests, so an achievement is a definition rather than a system.

## Turning it on

`StatusPlugin` declares `needs::<Registries>`, hinting for statuses that may be empty, and its `finish` declares `depends_on::<CombatPlugin>`, because a tick's damage goes down the pipeline a sword's does.
It registers `Afflict`, `Cure` and `StatusEvent`, and chains `resolve_afflictions` ahead of `tick_statuses`, both in `ResolveSet::Effects`, which runs before `ResolveSet::Damage` so a tick lands in the pass that produced it.
Every `Actor` is given an empty `Afflicted` and an empty `StatBlock` the moment it is spawned, so a monster spawned without either still takes a status rather than shrugging it off; the stats are registered with `try_register_required_components`, since the items plugin asks for the same one and the order a game lists its plugins in must not matter.
They are required on `Actor` here rather than in the core, so a game with no statuses carries neither component.
`FactsPlugin` is the opposite shape: it registers `Happened` and `QuestChange`, runs `track_facts` in `PostUpdate` while play is on, and asks for `Quests` or `Counters`, either of which will do.
That is why it asserts on entering `Playing` rather than declaring a `needs`: a `needs` names one resource, and this plugin works with either of two.
A game that inserted neither has nothing listening and is told so, loudly, the moment play begins.

## The model

`StatusDef` is a `name`, a `stacking` rule, a list of `modifiers`, an optional `tick_damage` as a kind and an amount, and an optional `badge` of one character a panel may draw.
`status::load` reads them from RON by name, resolving every stat and damage kind through `Names` and reporting every unknown name in the file at once, so a game authors statuses in the words its other content uses.
The definition is never deserialized as it stands, because its ids index registries a content file cannot see and a number written in one would land on a different stat the day the stat list is reordered.
`Stacking` is `Refresh`, where the longer duration wins and which is the default, `Extend`, where durations add, `Stack`, where a second instance sits beside the first, and `Ignore`.
`Afflicted` is the `Statuses` an actor carries, each an `ActiveStatus` of an id, the whole turns left, and an opaque source so a tick can credit whoever applied it.
`Afflict { target, status, turns, by }` puts one on and `Cure { target, status }` takes one off, and each answers with a `StatusEvent`: `Applied` for a fresh one, a refresh or an extension, `Expired` when the time ran out, `Cured` when it was lifted.
`Statuses::apply` installs the definition's modifiers under a `Source::Status` tagged with the id and the instance, which is what makes removal exact when the same status stacks three deep.
`Statuses::tick` collects what every status deals into a `TickReport`, then takes a turn off each and strips the modifiers of whatever ran out.
`tick_statuses` runs it once per `TurnEnd` and only for actors on the current map, so a monster on a floor nobody is standing on does not burn down while the player is elsewhere.
Its damage becomes a `DamageEvent` carrying `Hit::from_status`, which names the status and credits whoever applied it but leaves `attacker` empty, so a poison tick cannot set off the riders a blow would.
That event is built with `DamageEvent::new`, so its `Reach` is `Effect` and rides through to `DamageDealt` as one: a game hanging a rule off a blow can tell a blow from a tick without reading `status` at all.
A negative `ticks` amount mends through the same pipeline, which is the whole of what makes regeneration a status like poison.
`StatBlock` is the `Stats` underneath: a per-actor base for whichever stats the game overrode, and a flat list of `Modifier`s each carrying a `Source`.
`Stats::value` is base plus every `Add`, then every `MulPct` compounded, then `AtLeast` and `AtMost`, then the definition's own `min` and `max`.
The `Source` is a tagged value rather than an opaque number because several systems fold modifiers into one `Stats` without knowing about each other: statuses remove theirs one instance at a time, the gear fold strips every item's and puts the worn ones back, and a game's own sit under `Source::Game` where nothing in the engine touches them.
A `Fact` is a registered `FactKind`, an optional subject, an optional object and an amount that is one unless the fact is about a quantity, all of it in the game's own numbering.
A `Matcher` picks facts out by kind and optionally by subject and object, and a fact with no subject is not about anyone.
`Ledger` is a value per registered `CounterDef` and a list of `Tally` rules, and feeding it a fact adds that fact's amount to every counter whose rule matches.
`Tracker` is where every quest stands: an `Objective` is a matcher and a `Need`, either a `Total` the matching amounts add up to or a `Latest` a single fact must reach.
A `QuestDef` opens when every quest in its `after` is done and is done when every objective is, and `victory` on it says the run is won.
`Tracker::feed` reports what one fact changed, in the order it happened: `Progress`, then `ObjectiveDone`, then `QuestDone`, then `QuestOpened`.
`Happened` wraps a fact for the engine, `track_facts` feeds it to whichever of the tracker and the ledger the game inserted, and each change comes back as a `QuestChange` written after the frame's systems and read by them the next frame.

## Using it

A status is a registry entry, and these are the two Delve's caves have.

<!-- include: ../../../../examples/delve/src/main.rs:statuses -->
```rust,no_run
    let fire = damage_kinds.expect("fire");
    let statuses = Registry::from_defs(vec![
        StatusDef { badge: Some('s'), ..StatusDef::new("scorched").ticks(fire, 1) },
        StatusDef { badge: Some('z'), ..StatusDef::new("dazed") },
    ])
    .unwrap();
```

A fact is the game's reading of an engine message, which is the only translation quests and counters need.

<!-- include: ../../../../examples/corsair/src/quests.rs:killed -->
```rust,no_run
    let mut report = |f: Fact| happened.write(Happened(f));
    for d in out.deaths.read() {
        if let Ok((kind, faction)) = out.monsters.get(d.entity) {
            report(Fact::new(facts.killed).about(kind.0.raw() as u64));
            report(Fact::new(facts.killed_faction).about(faction.0.raw() as u64));
        }
    }
```

## The line

The engine acts on two things a status says and nothing else: it installs and removes the stat modifiers, and it turns the tick into a hit.
Everything richer is the game's, keyed by the id: a status that silences an ability, one that walls a door, one that turns a body to stone is a system reading `StatusEvent` or `Afflicted` and doing the rest.
Whether anything is inflicted at all is the game's too, and the shape the randomness rule points at is a system reading `DamageDealt` and writing `Afflict` with a chance drawn from the game's own stream, never the engine's, so a rule a game adds cannot shift the dice of the blows the engine has yet to throw.
The engine also never decides that a status is worth saying out loud: it writes the three events and a game turns the ones about its player into words, which is why every phrase about an affliction lives in a game or in the narrator's table.
What a panel shows of a status is the same division: `badge` is the one character the engine offers, and anything else is a `Facet` the game pushes onto a row in `ViewSet::Annotate`, a key interned in `Facets`, the words, and a tone.
A facet is an escape hatch and its use is a signal, so two games pushing the same key is the argument for putting that field in the view instead.
A stat is content, which is the reason nothing in the engine names one: `CombatRules` is handed the stat it should read as armor, an affix names the stat it sharpens, and a status names the stat it moves.
Facts are the same refusal one level up: nothing in the engine names a fact, a counter or a quest, and the subject and object are opaque numbers the game chose, usually a definition id.
A game therefore writes one system that turns the messages it cares about into facts, and every achievement, generated objective and run summary after that is data over the same stream.
The one thing the engine insists on is when: `track_facts` runs after the frame's systems, so a change is read on the next frame and a quest cannot finish halfway through the turn that finished it.

## Where it lives

`rl-rules` is tier 1 and has no Bevy in it, which is what lets the awkward parts be proved without an `App`.
`status.rs` holds the stacking rules and the tick, so a status applied three times, extended, and expiring one instance at a time is a test over a `Statuses` and a `Stats` built by hand.
`stats.rs` holds the order of operations and the source tagging, and the test that matters is the one where an item's modifier, a status's and a game's share a number and only the item's is stripped.
`events/` is `fact.rs`, `ledger.rs` and `quest.rs`, and none of the three knows what a fact is about, so a quest chain is tested by feeding facts made of integers.
`rl-bevy` is tier 2 and thin over both: `status.rs` is the two components, the two requests and the system that ticks the current map, and `events.rs` is a message, two optional resources and one system that feeds them.
Thin on purpose, because the parts worth testing are the parts that never needed a world.
