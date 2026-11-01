<!-- documents:
     plugins: StealthPlugin
     files: crates/atglyph-rules/src/ai/awareness.rs
            crates/atglyph-rules/src/ai/awareness/per_observer.rs
            crates/atglyph-rules/src/ai/awareness/by_range.rs
            crates/atglyph-rules/src/ai/tactics.rs
            crates/atglyph-rules/src/faction.rs
            crates/atglyph-bevy/src/stealth.rs
            crates/atglyph-bevy/src/minds.rs
            crates/atglyph-bevy/src/noise.rs
            crates/atglyph-bevy/src/combat.rs
            crates/atglyph-bevy/src/components.rs
            crates/atglyph-bevy/src/status.rs
            crates/atglyph-bevy/src/items.rs
            crates/atglyph-bevy/src/ability.rs
            crates/atglyph-bevy/src/remains.rs
            crates/atglyph-rules/src/status.rs
            crates/atglyph-bevy/src/lighting.rs
            crates/atglyph-grid/src/light.rs
            crates/atglyph-ui/src/view/nearby.rs
            crates/atglyph-ui/src/view/vitals.rs
     fingerprint: a2a9ec43 -->

# Stealth

Without this a mind acts on everything its own sight reaches the turn it first reaches it, which leaves a player nothing to break and a monster nothing to regain.
Stealth is the layer between "could be seen" and "has been seen": a roll to notice, and a memory that decays.
What an observer knows is per observer and per subject, so a monster may be unaware of you and perfectly aware of the thief beside it, and one that loses you searches where it last saw you before it forgets.
Who may notice whom is decided by one of two rules a game picks: by the observer, two knobs on whoever notices against two on whoever hides, or by the subject, a range of its own within which anything may notice it.
A status can go further and make its holder unseen, hidden from every mind at any distance until it runs out or its holder attacks, or, for one a worn thing holds, until the thing comes off.

## Turning it on

A game adds the plugin with the rule it wants: `StealthPlugin::per_observer()` or `StealthPlugin::by_range(rules)`, where `rules` is a `RangeRules` of the game's own numbers, since the engine has none.
Either way the plugin adds the same six systems (by range, two more keep each range current), and the messages and the stream they need: the noticing roll in `DecideSet::Notice`, before a mind decides; taking out the unseen, then the unnoticed, in `PerceiveSet::Filter`, after the roster stage put everyone in sight into a snapshot; revealing attackers and marking the unseen in `ResolveSet::Effects`, either side of the statuses being applied and ticked; and waking the struck in `ReactSet::Settle`, inside `TurnSet::React`, where a turn's consequences land, so a game that reads `Noticed` or writes an `Aware` in its own reactions orders itself after that set.
It registers `RemoveStatus` and declares that it reads `Struck`, `ItemEvent` and `LandedOn`, so a game with stealth and no items or abilities has nothing to reveal on rather than a panic; with no status registered as unseen nothing is ever unseen.
It declares `depends_on::<MindsPlugin>`, since noticing is a thing minds act on, and the stream is `StealthRng`, so a tactic added to a brain or a blow struck elsewhere cannot shift which turn a guard spots you on.

Per observer, both sides have to be authored before anything changes: a `Notice` absent means the observer sees on sight, which is the behaviour before stealth existed, and a `Stealth` absent means the subject never hides.
That is the right way round, and it is why "I added the plugin and nothing happened" is the likely first report.
The plugin is opt-in per game and the components are opt-in per spawn, so a game may carry it and still have places where nothing hides.
Lighting is optional under it: with no `Lighting` resource every cell counts as lit, and with one, a subject standing in the lit band widens the observer's certain radius by `lit_bonus`, which is zero unless a game says otherwise.
A subject in a lamp's dim ring is seen and gives no bonus, which is what makes the edge of the light a place to stand.

By range, nothing is authored per spawn: the plugin gives every `Actor` that appears without a `StealthRange` one at `rules.base`, and every `Mind` that appears without an `Aware` one, the moment each appears, whether a game, a prefab or a save made it.
So adding the plugin changes everything at once: every actor hides, and every mind notices.

Combat is optional under either rule: with no `CombatRules` nobody has a side, so everyone is at odds with everyone and every observer judges every hider.

## The model

`Hides` is the marker every subject of either rule carries, brought by a `Stealth` or a `StealthRange` and gone with the last of them, and the one thing asked wherever the question is whether something can hide: the noticing roll, the unnoticed filter, waking on a blow, `Watchers`, a sleeper's waking and the vitals strip.
`Notice` requires `Aware(BTreeMap<Entity, Awareness>)`, keyed only by the subjects the observer has an opinion of: a map for correctness, and a `BTreeMap` because it is read on the decision path and walked in a fixed order.
`Awareness` is `Unaware` or `Alert { at, stale_turns }`, and the third state is derivable rather than stored: alert and in sight is hunting, alert and out of sight is searching.
`saw(at)` resets the staleness as well as the position, which is what stops a monster giving up on the turn it catches you; `lost(memory)` returns to `Unaware` once the count passes `memory`; `alerted_to(at)` is anything that tells it without its looking, and is the same call.

The rules differ in two questions and in nothing else: whether an unaware observer spots a subject it has a line to this turn (`spots`), and whether an alert one keeps hold of a subject it has a line to (`holds`).
Each rule answers both as pure functions in `atglyph-rules`, over a `Glimpse { distance, in_view, lit, asleep }` the pipeline fills in, `lit` only for the per-observer rule, the one that reads it, and `observe` is the one function that turns an answer into what the observer knows afterward.

Per observer, `NoticeStats` is the observer's half: `certain`, the tiles inside which it spots you whatever the roll; `chance_pct`, its chance a turn beyond that; `lit_bonus`, added to `certain` while you stand in the lit band; and `memory`, the turns it keeps looking after losing you, six when a content file leaves it out.
`StealthStats` is the subject's: `quiet` off the certain radius and `subtlety` off the chance, both defaulting to nothing.
`certain_radius` is `certain` plus the light bonus less `quiet`, floored at one, so no stack of gear hides you from somebody standing next to you; `notice_chance` is the chance less `subtlety`; `notices` is either of them answering yes.
`Notice(NoticeStats)` and `Stealth(StealthStats)` are the components, one name per tier so that globbing both crates into a prelude does not put two types called `Notice` in it.
An alert observer holds whatever it can see, and forgets after its own `memory`.
It judges only what it is at odds with, so a neutral costs it no roll.

By range, `StealthRange(i32)` is the subject's: the distance within which an observer may notice it, the same for every observer.
`RangeRules` is the game's, one set for the whole game: `base`, the range in the open; `dark_divisor` and `cover_divisor`, dividing it down in the dark and in cover; `still_divisor`, dividing it, rounding up, while the subject stands still; `attack_multiplier`, on the turn it attacks; `floor`, below which it never falls; `chance_pct`, the chance a turn of being spotted within it; `hold`, the times the range within which an alert observer keeps hold of it in view; and `memory`, the turns an observer keeps looking after losing hold.
Each divisor is at least one and each multiplier, `attack_multiplier` and `hold`, at least nought: `RangeRules::check` names any that are not, `StealthPlugin::by_range` refuses rules that fail it, naming every one, and a multiplier too large for a range saturates rather than overflowing.
`range_of` works a range out from them and a `Standing`, in that order of steps.
The engine keeps each actor's range current itself: `LastDeed`, brought by `StealthRange`, is what the actor did last, `Deed::Waited` when its action was a wait, `Deed::Attacked` when it struck, missed or threw at someone, and `Deed::Acted` for any other action.
At the head of every pass, before the roll, the range of each actor on the current map that moved or finished an action since the last pass is worked out afresh from its `LastDeed` and where it stands, and of every actor when the light or the map changed under them: the dark band alone is dark, so a lit tile or a lamp's dim ring is not, and a tile is cover when the tile tables say so.
So a range a game gives an actor at spawn lasts only until the first turn, and a game changes ranges by changing its `RangeRules`, its tiles and its light.
An unaware observer spots a subject within its range by the chance alone, never beyond it however long it looks, and never for certain however near.
It judges everything it is not allied with, a neutral included, so a deer must notice the player before it steps away; a subject with no `StealthRange`, such as a body left where an actor fell, is never spotted.

The roll is made for the actor holding the turn and no other, and never for the player, so noticing costs at most one roll per subject in view per monster-turn and nothing per frame: per observer, the actor must carry a `Notice`; by range, it must be a mind.
It looks only at the subjects on its map within its reach, and at those it already knows of wherever they went, which age out of its memory as ever, so a turn costs a look per subject nearby, not per subject loaded.
It walks its subjects in the order of where they stand, by map, row and column, rather than the order of the archetypes or the entities' numbers, since which subject gets which roll must not depend on how the world happens to be laid out or numbered.
A roll decides only whether an unaware observer becomes aware: one already alert keeps its subject while its rule says it holds, so a monster in plain view does not lose you to a bad number, and losing takes `memory` turns without holding while noticing takes one.
What "could be seen" means is the observer's own `Viewshed` and `within_reach` of its `Perception`, `DEFAULT_PERCEPTION` without one, so the minds, the roll and the panels can never disagree about who could be seen.
An ally is never judged by either rule, and a subject the observer does not judge is dropped from its `Aware`.
`Noticed { observer, subject, at }` is written once, on the flip from unaware, and never again while the awareness holds.

The unnoticed filter takes the hiders the mind holding the turn has not noticed back out of its enemies, and by range out of its others too, so a neutral is hidden from a mind until it notices it just as an enemy is; its allies it never touches, and a mind that keeps no `Aware` it leaves alone.
It then offers every subject the mind is alert to, at odds with, and cannot see as a trail through `Thinking::offer_trail`; one it is not at odds with, as a deer the player, it keeps in mind and forgets the same way, but never walks back to look for.
Noise offers its own to the same place, the freshest becomes `Snapshot::last_known`, and `SearchLastKnown` walks to it.
Whoever takes a blow from something that hides is woken and pointed at the attacker's cell: a mend is a `Heal` and never a blow, a blow armor stopped at zero still wakes it, and so does a rolled attack that missed, read from `Missed`, so a hider cannot fire at a sleeper until one lands.
Whether stealth is running is asked of the rule the plugin inserts rather than of the components, because `Notice` brings an `Aware` with it and a game that authored observers without the plugin would otherwise have monsters that notice nothing forever; `Watchers::running` answers it.
`Watchers` answers who is watching whom by the rule the minds act on: an observer that keeps an `Aware` watches what it knows about, one that does not watches whatever its own sight reaches, and neither watches anything it is not at odds with or anything unseen.
`Watchers::remembers` answers whether an observer still has a subject in mind, watching it or not, and the two part only for the unseen.
`Watchers::unaware` answers whether an observer has not noticed a subject at all, which combat asks before a blow by the subject lands harder for it: only while the plugin runs, for an observer keeping an `Aware`, at odds with a subject that hides, that remembers nothing of it, so one searching where it lost the trail is not caught unaware and the player never is.
`StatusDef::unseen`, `unseen: true` in a status file, is a status whose holder nothing sees while it lasts, and `Unseen` is the marker the engine keeps on whoever holds one.
It is kept from `Changed<ActiveStatuses>` after the statuses are applied, ticked and cured, so it lasts exactly as long as the status, a continued run gets it back from the statuses it saved, and the pass a status goes on is the pass the next mind to decide cannot see its holder.
An unseen status a worn thing holds ends by the items plugin's `RemoveStatus`, written in the same stage before the statuses resolve, so the marker comes off in the pass the thing does and the next mind to decide sees its wearer.
The unseen filter takes the unseen out of the enemies, allies and others of every mind, noticing or not, and the roll counts one out of view, so an observer that was alert to it searches where it last saw it for its memory and forgets.
Revealing reads `Struck`, written for every blow or shot before the accuracy roll, `ItemEvent::Thrown`, and `LandedOn` for an ability or a thing's aimed use that came down on anyone but its user, and writes a `RemoveStatus` for every unseen status each attacker holds.
It runs before the statuses resolve in the same pass, so the marker is gone before the damage lands and the one struck is pointed at an attacker it can see.
Nobody is woken to an attacker still unseen, which is harm from something revealing does not read: a thing used whose `use` trigger hurt those around it, a worn thing's timer, or a trap its holder set off; a status ticking carries no attacker and wakes nobody at all.
A status that sleeps (`docs/guide/src/systems/statuses.md`) leans on the same answers this page gives: `Watchers::unaware` says yes for a sleeper to whoever strikes it, stealth running or not, so the blow that wakes one lands as on the unaware first; and a sleeper's own noticing is the ordinary roll, run only on its turn, so stealth's roll and sleep's wake are one mechanism asked twice rather than two.
Per observer, a sleeper notices only within `SLEEPER_REACH`, by its chance alone, and wakes to a foe it notices there; by range it notices by the subject's range as it would awake, and wakes to anything it notices.
A sleeper that notices by neither rule, the player or one with no mind, wakes on sight to a foe within `SLEEPER_REACH`, whichever rule runs.

## Using it

A per-observer observer is authored per kind, so the template's goblin is spawned with a `Notice` of its own, sure of what is two steps off, further while its subject stands in light, and given a chance beyond that; a kind given none would notice nobody.

<!-- include: ../../../../templates/starter/src/main.rs:goblin -->
```rust,no_run
impl Goblins {
    /// One goblin, standing at `at`.
    fn goblin(&self, at: Point) -> impl Bundle + use<> {
        (
            (
                Actor,
                Blocks,
                Position(at),
                Name::new("goblin"),
                Faction(self.side),
            ),
            (
                Health::full(8),
                Armor(0),
                MeleeAttack::new(self.stab, DiceRoll::new(1, 4)),
                Perception(10),
                Mind(self.mind.clone()),
                Moves(Movement::Wander),
                // Sapient: it searches where it lost you, and opens doors.
                // `Wits::ANIMAL` would stop at a door, and `Wits::MINDLESS`
                // would forget you the moment it lost sight of you.
                Intelligence(Wits::SAPIENT),
            ),
            // It sees three steps without light, always notices you two
            // steps away (further while you stand in light), sometimes
            // beyond that, and remembers where you were for six turns.
            (
                DarkSight(3),
                Notice(NoticeStats {
                    certain: 2,
                    chance_pct: 20,
                    lit_bonus: 4,
                    memory: 6,
                }),
                Glyph::new('g', Color::srgb(0.55, 0.80, 0.35)).on_layer(5),
            ),
        )
    }
}
```

The engine carries `Noticed` no further than writing it, and a game decides what being seen sets off, such as a sentry that knows where the player is raising an alarm, in a system of its own that reads `Aware`.
A game on the by-range rule authors no part per kind: it names its `RangeRules` once, in the plugin's constructor.

## The line

The engine decides who has noticed whom; a game decides what that is worth.
The engine owns the pipeline, the memory, the roll, the trail and the waking, and both rules; the game picks a rule and supplies its numbers, an observer's attention and a subject's quiet per observer, the one set of `RangeRules` by range.
A game using one rule never names the other's types, and cannot add a third: each rule is a pair of pure questions the engine answers, and a third would be written beside them in the engine.
Propagation is deliberately absent: what a shout carries, how far it goes and who it reaches are content, so a game that wants a squad writes a dozen lines over `Noticed` rather than accepting the engine's idea of a squad.
Sneak damage is the engine's only as far as the question goes, whether a target has noticed its attacker, which `Watchers::unaware` answers for combat so a blow and the minds never disagree; the multiplier is balance, a percent the game names on `CombatRules` or on the blow, and a game that names none lands a blow from hiding as any other.
Stealth hides a subject from minds and from nothing else: the drawing is untouched, so a monster is never hidden from the player, and two-way stealth would be a render change rather than another component.
The engine decides that the unseen is seen by nothing and that attacking ends it; the game decides which status is unseen, for how long, what grants it, and whether it lasts only while the thing that granted it is worn.
The unseen departs on purpose from the floor of one: that floor is about `Stealth`'s quiet, which is for good, and the unseen is a status that lasts turns and ends the moment its holder strikes.
Being hurt does not end it, and hearing is untouched, so a mind that hears the unseen still goes to look and finds nothing there to see.
A `Perception` is still the hard cap on how far an actor notices anything at all, and a rule is only the curve inside it, which is how a game gives a guard long sight and poor attention.
`notices` takes a `lit` flag rather than a `Lighting`, so it stays pure and a game is free to decide exposure means standing in water, or on open ground, or having shouted a moment ago.
Light is the one exposure term the per-observer rule ships, and it is one number, so a creature with `lit_bonus: 0` is one that hunts by something other than the eye without the engine learning a word for it.
Hearing is a separate lever that this never reads: it brings a monster close, and close is where the roll is likely to land.
What the player reads off it is `Alert::Hunting` on a nearby row, unless the game named what the monster is doing, which outranks it, and hunting outranks searching, since something that has seen you is not still wondering about a noise.
Something that had you and lost you to the unseen reads `Alert::Searching`, since it is on its way to where it last knew of you, and neither hunting, since nothing sees the unseen, nor idle, since it has not forgotten.
`Aware` is not saved, so a monster that had noticed you has forgotten by the time a continued run begins.

## Where it lives

`atglyph-rules` is tier 1 and has no Bevy in it: `ai/awareness.rs` is the state machine, the glimpse a rule judges and `observe`; `ai/awareness/per_observer.rs` is the two stat blocks, the three functions over them and that rule's `spots` and `holds`; and `ai/awareness/by_range.rs` is `RangeRules` and its `check`, `range_of` and that rule's `spots` and `holds`, with the caller doing the rolling and deciding what lit, dark and cover mean.
That is what lets the properties be proved rather than watched: that light widens the certain radius by exactly its bonus, that the floor of one holds against any stack of gear, that a range takes each step in its order and never falls below its floor, that no roll spots beyond the range, and that `lost` returns to `Unaware` on exactly the turn the memory passes.
The stat blocks and the range rules are serde-ready, so an observer's attention, a subject's quiet and a game's ranges are written in a content file rather than in Rust.
`atglyph-bevy` is tier 2 and owns the rolling: `stealth.rs` is the components, `Aware`, `Unseen`, the stream, the six systems, the one place the two rules differ, `LastDeed` and the two systems that keep a range current, `record_deeds` and `refresh_ranges`, and the `Watchers` parameter that answers whether stealth is running and who is watching.
`StatusDef::unseen` is a flag on a tier 1 definition, so which status hides its holder is a line in a status file that loads and is checked without an `App`, and what hiding does is left to `stealth.rs`.
`Watchers` lives there rather than in a panel because the vitals strip and the nearby rail must read the same answer the minds act on, and the bug that put it there was a strip reading hidden while a cutthroat cut the player down.
