<!-- documents:
     plugins: StealthPlugin
     files: crates/atglyph-rules/src/ai/awareness.rs
            crates/atglyph-rules/src/ai/tactics.rs
            crates/atglyph-bevy/src/stealth.rs
            crates/atglyph-bevy/src/minds.rs
            crates/atglyph-bevy/src/noise.rs
            crates/atglyph-bevy/src/combat.rs
            crates/atglyph-bevy/src/components.rs
            crates/atglyph-bevy/src/status.rs
            crates/atglyph-bevy/src/items.rs
            crates/atglyph-bevy/src/ability.rs
            crates/atglyph-rules/src/status.rs
            crates/atglyph-bevy/src/lighting.rs
            crates/atglyph-grid/src/light.rs
            crates/atglyph-ui/src/view/nearby.rs
     fingerprint: c09e5c36 -->

# Stealth

Without this a mind acts on everything its own sight reaches the turn it first reaches it, which leaves a player nothing to break and a monster nothing to regain.
Stealth is the layer between "could be seen" and "has been seen": a roll to notice, and a memory that decays.
Noticing is two knobs rather than a radius, because a radius alone is a hard line the player learns to stand behind and a chance alone is a lottery with no readable edge.
What an observer knows is per observer and per subject, so a monster may be unaware of you and perfectly aware of the thief beside it, and one that loses you searches where it last saw you before it forgets.
A status can go further and make its holder unseen, hidden from every mind at any distance until it runs out or its holder attacks, or, for one a worn thing holds, until the thing comes off.

## Turning it on

`StealthPlugin` adds six systems, and the messages and the stream they need, and nothing else: the noticing roll in `DecideSet::Notice`, before a mind decides; taking out the unseen, then the unnoticed, in `PerceiveSet::Filter`, after the roster stage put everyone in sight into a snapshot; revealing attackers and marking the unseen in `ResolveSet::Effects`, either side of the statuses being applied and ticked; and waking the struck in `TurnSet::React`, where a turn's consequences land.
It registers `RemoveStatus` and declares that it reads `Struck`, `ItemEvent` and `LandedOn`, so a game with stealth and no items or abilities has nothing to reveal on rather than a panic; with no status registered as unseen nothing is ever unseen.
It declares `depends_on::<MindsPlugin>`, since noticing is a thing minds act on, and the stream is `StealthRng`, so a tactic added to a brain or a blow struck elsewhere cannot shift which turn a guard spots you on.
Both sides have to be authored before anything changes: a `Notice` absent means the observer sees on sight, which is the behaviour before stealth existed, and a `Stealth` absent means the subject never hides.
That is the right way round, and it is why "I added the plugin and nothing happened" is the likely first report.
The plugin is opt-in per game and the components are opt-in per spawn, so a game may carry it and still have places where nothing hides.
Lighting is optional under it: with no `Lighting` resource every cell counts as lit, and with one, a subject standing in the lit band widens the observer's certain radius by `lit_bonus`, which is zero unless a game says otherwise.
A subject in a lamp's dim ring is seen and gives no bonus, which is what makes the edge of the light a place to stand.
Combat is optional too: with no `CombatRules` nobody has a side, so everyone is at odds with everyone and every observer rolls against every hider.

## The model

`NoticeStats` is the observer's half: `certain`, the tiles inside which it spots you whatever the roll; `chance_pct`, its chance a turn beyond that; `lit_bonus`, added to `certain` while you stand in the lit band; and `memory`, the turns it keeps looking after losing you, six when a content file leaves it out.
`StealthStats` is the subject's: `quiet` off the certain radius and `subtlety` off the chance, both defaulting to nothing.
`certain_radius` is `certain` plus the light bonus less `quiet`, floored at one, so no stack of gear hides you from somebody standing next to you; `notice_chance` is the chance less `subtlety`; `notices` is either of them answering yes.
`Notice(NoticeStats)` and `Stealth(StealthStats)` are the components, one name per tier so that globbing both crates into a prelude does not put two types called `Notice` in it.
`Notice` requires `Aware(BTreeMap<Entity, Awareness>)`, keyed only by the subjects the observer has an opinion of, which in most games is the player alone: a map for correctness and one entry in practice, and a `BTreeMap` because it is read on the decision path and walked in a fixed order.
`Awareness` is `Unaware` or `Alert { at, stale_turns }`, and the third state is derivable rather than stored: alert and in sight is hunting, alert and out of sight is searching.
`saw(at)` resets the staleness as well as the position, which is what stops a monster giving up on the turn it catches you; `lost(memory)` returns to `Unaware` once the count passes `memory`; `alerted_to(at)` is anything that tells it without its looking, and is the same call.
The roll is made for the actor holding the turn and no other, and never for the player, which carries no `Aware` worth filling and never rolls to notice whatever `Notice` is put on it, so noticing costs one roll per subject per monster-turn and nothing per frame.
It walks its subjects in spawn order rather than archetype order, since which subject gets which roll must not depend on how the world happens to be laid out.
A roll decides only whether an unaware observer becomes aware: one already alert keeps its subject for as long as it can perceive it, so a monster in plain view does not lose you to a bad number, and losing takes `memory` turns out of sight while noticing takes one.
What "could be seen" means is the observer's own `Viewshed` and `within_reach` of its `Perception`, `DEFAULT_PERCEPTION` without one, so the minds, the roll and the panels can never disagree about who could be seen.
`Noticed { observer, subject, at }` is written once, on the flip from unaware, and never again while the awareness holds.
The unnoticed filter takes the hiders the mind holding the turn has not noticed back out of its enemies and leaves a mind that keeps no `Aware` alone, then offers every subject it is alert to but cannot see as a trail through `Thinking::offer_trail`.
Noise offers its own to the same place, the freshest becomes `Snapshot::last_known`, and `SearchLastKnown` walks to it.
Whoever takes a blow from something carrying `Stealth` is woken and and points it at the attacker's cell: a mend is a `Heal` and never a blow, a blow armor stopped at zero still wakes it, and so does a rolled attack that missed, read from `Missed`, so a hider cannot fire at a sleeper until one lands.
Whether stealth is running is asked of a marker only the plugin inserts rather than of the components, because `Notice` brings an `Aware` with it and a game that authored observers without the plugin would otherwise have monsters that notice nothing forever; `Watchers::running` answers it.
`Watchers` answers who is watching whom by the rule the minds act on: an observer that keeps an `Aware` watches what it knows about, one that does not watches whatever its own sight reaches, and neither watches anything it is not at odds with or anything unseen.
`Watchers::remembers` answers whether an observer still has a subject in mind, watching it or not, and the two part only for the unseen.
`Watchers::unaware` answers whether an observer has not noticed a subject at all, which combat asks before a blow by the subject lands harder for it: only while the plugin runs, for an observer keeping an `Aware`, at odds with a subject carrying `Stealth`, that remembers nothing of it, so one searching where it lost the trail is not caught unaware and the player never is.
`StatusDef::unseen`, `unseen: true` in a status file, is a status whose holder nothing sees while it lasts, and `Unseen` is the marker the engine keeps on whoever holds one.
It is kept from `Changed<ActiveStatuses>` after the statuses are applied, ticked and cured, so it lasts exactly as long as the status, a continued run gets it back from the statuses it saved, and the pass a status goes on is the pass the next mind to decide cannot see its holder.
An unseen status a worn thing holds ends by the items plugin's `RemoveStatus`, written in the same stage before the statuses resolve, so the marker comes off in the pass the thing does and the next mind to decide sees its wearer.
The unseen filter takes the unseen out of the enemies, allies and others of every mind, noticing or not, and the roll counts one out of view, so an observer that was alert to it searches where it last saw it for its memory and forgets.
Revealing reads `Struck`, written for every blow or shot before the accuracy roll, `ItemEvent::Thrown`, and `LandedOn` for an ability or a thing's aimed use that came down on anyone but its user, and writes a `RemoveStatus` for every unseen status each attacker holds.
It runs before the statuses resolve in the same pass, so the marker is gone before the damage lands and the one struck is pointed at an attacker it can see.
Nobody is woken to an attacker still unseen, which is harm from something revealing does not read: a thing used whose `use` trigger hurt those around it, a worn thing's timer, or a trap its holder set off; a status ticking carries no attacker and wakes nobody at all.
A status that sleeps (`docs/guide/src/systems/statuses.md`) leans on the same two answers this page gives: `Watchers::unaware` says yes for a sleeper to whoever strikes it, stealth running or not, so the blow that wakes one lands as on the unaware first; and a sleeper's own noticing is the ordinary roll, narrowed to `SLEEPER_REACH` and run only on its turn, so stealth's roll and sleep's wake are one mechanism asked twice rather than two.

## Using it

An observer is authored per kind, so the template's goblin is spawned with a `Notice` of its own, sure of what is two steps off, further while its subject stands in light, and given a chance beyond that; a kind given none would notice nobody.

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

## The line

The engine decides who has noticed whom; a game decides what that is worth.
Propagation is deliberately absent: what a shout carries, how far it goes and who it reaches are content, so a game that wants a squad writes a dozen lines over `Noticed` rather than accepting the engine's idea of a squad.
Sneak damage is the engine's only as far as the question goes, whether a target has noticed its attacker, which `Watchers::unaware` answers for combat so a blow and the minds never disagree; the multiplier is balance, a percent the game names on `CombatRules` or on the blow, and a game that names none lands a blow from hiding as any other.
Stealth hides a subject from minds and from nothing else: the drawing is untouched, so a monster is never hidden from the player, and two-way stealth would be a render change rather than another component.
The engine decides that the unseen is seen by nothing and that attacking ends it; the game decides which status is unseen, for how long, what grants it, and whether it lasts only while the thing that granted it is worn.
The unseen departs on purpose from the floor of one: that floor is about `Stealth`'s quiet, which is for good, and the unseen is a status that lasts turns and ends the moment its holder strikes.
Being hurt does not end it, and hearing is untouched, so a mind that hears the unseen still goes to look and finds nothing there to see.
A `Perception` is still the hard cap on how far an actor notices anything at all, and `Notice` is only the curve inside it, which is how a game gives a guard long sight and poor attention.
`notices` takes a `lit` flag rather than a `Lighting`, so it stays pure and a game is free to decide exposure means standing in water, or on open ground, or having shouted a moment ago.
Light is the one exposure term the engine ships, and it is one number, so a creature with `lit_bonus: 0` is one that hunts by something other than the eye without the engine learning a word for it.
Hearing is a separate lever that this never reads: it brings a monster close, and close is where the roll is likely to land.
What the player reads off it is `Alert::Hunting` on a nearby row, unless the game named what the monster is doing, which outranks it, and hunting outranks searching, since something that has seen you is not still wondering about a noise.
Something that had you and lost you to the unseen reads `Alert::Searching`, since it is on its way to where it last knew of you, and neither hunting, since nothing sees the unseen, nor idle, since it has not forgotten.
`Aware` is not saved, so a monster that had noticed you has forgotten by the time a continued run begins.

## Where it lives

`atglyph-rules` is tier 1 and has no Bevy in it: `ai/awareness.rs` is the two stat blocks, the three functions over them and the state machine, with the caller doing the rolling and the caller deciding what lit means.
That is what lets the properties be proved rather than watched: that light widens the certain radius by exactly its bonus and by nothing else, that `quiet` narrows it and the floor of one holds against any stack of gear, and that `lost` returns to `Unaware` on exactly the turn the memory passes while a sighting in between resets the count.
Both stat blocks are serde-ready, so an observer's attention and a subject's quiet are written in a bestiary file rather than in Rust.
`atglyph-bevy` is tier 2 and owns the rolling: `stealth.rs` is the components, `Aware`, `Unseen`, the stream, the six systems and the `Watchers` parameter that answers whether stealth is running and who is watching.
`StatusDef::unseen` is a flag on a tier 1 definition, so which status hides its holder is a line in a status file that loads and is checked without an `App`, and what hiding does is left to `stealth.rs`.
`Watchers` lives there rather than in a panel because the vitals strip and the nearby rail must read the same answer the minds act on, and the bug that put it there was a strip reading hidden while a cutthroat cut the player down.
