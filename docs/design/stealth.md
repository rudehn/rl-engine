# Stealth and awareness

Status: phases A to D built 2026-09-12, on top of the abilities slice; E proposed.
Written against `main` at `5b12d0a` while that slice was in flight, and built once it had landed; section 12b records where the build differs.

## 0. Summary

`decide_minds` builds a mind's [`Snapshot`] from everyone within its `Perception` that the player has a line to, filtered by `perceives`: lit, within dark sight, or adjacent.
That is instant, certain, omniscient detection.
There is no state between "could be seen" and "has been seen", so there is nothing for a player to break and nothing for a monster to regain.

Stealth is that missing layer, and it is mostly bookkeeping: remembering who has noticed whom, and for how long that memory lasts.

Three decisions shape everything else:

1. **Noticing is a roll against two knobs, not a radius.**
   A certain distance inside which nothing helps you, and a chance per turn beyond it.
   One knob alone gives either a hard line the player learns to stand behind or a lottery with no readable edge.
2. **Light is the exposure term, and it is one number.**
   A lit subject widens the observer's certain radius by `lit_bonus`, which defaults to zero.
   A game with no lighting is unaffected without saying so, and a game with lighting gets a stealth mechanic out of the field it already casts.
3. **Awareness is per observer, per subject, and it decays.**
   Not a global alert flag and not a property of the hider.
   A monster may be unaware of the player and perfectly aware of the thief beside it, and one that loses the trail goes looking where it last saw something rather than forgetting on the same frame.

The engine never says sneaking, hiding or shadows.
It knows an observer's [`Notice`], a subject's [`Stealth`], and whether a tile is lit.

## 1. What stealth buys

Roughly in the order it becomes cheap.

- **The lighting slice pays off twice.**
  Dousing a lantern already shrinks what you can see.
  With this it also shrinks what can see you, and the two are the same field, so a dark corridor becomes a decision rather than a penalty.
- **Breaking contact, which is the move a roguelike has no other way to offer.**
  Retreat today is a race the faster actor wins.
  With an awareness that goes stale, retreat becomes "get out of sight and stay out", and the monster searching the corner you left is the whole reason it reads as an escape.
- **An opening move.**
  Crossing a room unnoticed to reach the stair, or to reach the one thing in it worth reaching, is a plan.
  Phase E turns it into damage; it is worth playing before it is worth multiplying.
- **Monsters that are asleep until you are careless.**
  An unaware monster spends its turns on whatever its lowest-priority tactic is, which is already `Wander`.
  A floor that is quiet until you are seen is a floor with pacing, and it costs one component.
- **A reason for gear to have a downside.**
  Armor that widens the radius that notices you is the first piece of equipment in the engine with a real trade, and the modifier stack already carries it.
- **Later: distraction, and things that hunt by other means.**
  A thrown object as an alert position is the `Noticed` message pointed somewhere the player is not.
  A monster with `Notice { lit_bonus: 0 }` is one that does not care about light, which is how a game says "it hunts by smell" without the engine learning the word.

## 2. The model

```rust
/// How well an observer notices what is trying not to be seen.
pub struct Notice {
    /// Always noticed at or within this many tiles, however quiet.
    pub certain: i32,
    /// Chance per turn of noticing beyond that, in percent.
    pub chance_pct: u32,
    /// Added to `certain` while the subject stands in light. Zero, the
    /// default, is a game where light has nothing to do with being seen.
    pub lit_bonus: i32,
}

/// How hard a subject is to notice.
pub struct Stealth {
    /// Taken off the observer's certain radius, floored at one, so no
    /// stack of gear makes somebody standing next to you invisible.
    pub quiet: i32,
    /// Taken off the observer's chance, in percentage points.
    pub subtlety: u32,
}

/// Whether `notice` spots `stealth` this turn.
pub fn notices(distance: i32, notice: &Notice, stealth: &Stealth, lit: bool, roll: u32) -> bool;
```

`notices` takes `lit: bool` rather than a `Lighting`, so it stays pure, is tested without a world, and lets a game decide exposure means something else entirely: standing in water, or on open ground, or having just shouted.

`Perception` stays exactly as it is and keeps its meaning: the hard cap on how far an actor notices anything at all.
`Notice` is the curve inside it.
Keeping them apart means a game can give a monster long sight and poor attention, which is most guards.

## 3. Where each piece lives

### rl-rules: `ai/awareness.rs` (tier 1, no Bevy)

`Notice`, `Stealth`, `notices`, and the [`Awareness`] state machine of section 4.
Pure, so the roll is the caller's and two runs of the same seed agree.

### rl-bevy: `stealth.rs` (tier 2)

`Notice` and `Stealth` as components wrapping the tier-1 types.

`Aware(BTreeMap<Entity, Awareness>)` on each observer, keyed by the subjects it has an opinion about.
Only entities carrying `Stealth` are ever keyed, which in most games is one, so the map is a map for correctness and a pair of entries in practice.
A `BTreeMap` rather than a `HashMap` because it is read on a gameplay path and iterated in a fixed order.

`update_awareness` in a new `DecideSet::Notice`, ordered before `DecideSet::Minds`.
It runs for the actor holding the turn, so awareness ticks once per actor-turn: the right cadence, and it costs one roll per subject rather than one per frame.

`StealthPlugin` is opt-in and adds nothing else.
Without it there is no `Aware` component, `decide_minds` reads `Option<&Aware>` and finds none, and every game built so far behaves exactly as it does now.
That is the same shape `Lighting` uses and the reason a delve that wants none of this pays nothing for it.

### rl-bevy: `decide_minds`

One change: a candidate carrying `Stealth` enters `snapshot.enemies` only if the thinker is aware of it.
Everything without a `Stealth` component is seen the way it is today.

The oracle stays what it is.
Lines are symmetric and non-players carry no viewshed, so the player's is still what decides who has a line to whom; stealth layers on top of that and does not touch it.

## 4. The awareness state machine

```rust
pub enum Awareness {
    /// Has not noticed.
    Unaware,
    /// Knows where it was, and how long ago.
    Alert { at: Point, stale_turns: u32 },
}
```

Two variants, because the third is derivable and a third stored variant would be a third thing to keep true.
`Alert` and visible this turn is hunting; `Alert` and not visible is searching; that is the whole taxonomy and neither the engine nor a game has to write it down.

Transitions are pure methods: `saw(at)` on a sighting, `lost(forget_after)` on a turn without one, `alerted_to(at)` for anything that wakes it.
`lost` returns to `Unaware` once `stale_turns` passes `forget_after`, which is what ends a chase.

A sighting resets the staleness rather than only updating the position.
Without that, a monster that has been chasing for longer than the leash gives up on the turn it catches you, which is the bug `fantasy-rogue`'s own `alert_to_position` carries a paragraph of comment about.

## 5. The snapshot and the tactic

[`Snapshot`] gains one field:

```rust
/// Where the freshest thing it knows about but cannot see was last
/// seen. What a search walks toward.
pub last_known: Option<Point>,
```

And `rl-rules::ai::tactics` gains one tactic:

```rust
/// Walk to where an enemy was last seen. Nothing to do when it is in
/// sight, since hunting outranks searching.
pub struct SearchLastKnown;
```

It fires when `enemies` is empty and `last_known` is `Some`, descends toward the remembered tile, and stops mattering when the awareness goes stale and the field clears.
In the brain it sits between `Hunt` and `Wander`: look for what you lost before you drift.

That ordering is the whole behaviour.
A monster with `MeleeAdjacent`, `Hunt`, `SearchLastKnown`, `Wander` hunts what it sees, searches what it lost, and drifts when it has nothing, and a game reorders it by writing a different list.

## 6. Waking up

Being hit alerts you regardless of how quiet the attacker was.
`wake_on_damage` reads `DamageDealt` in `TurnSet::React`, which is where a turn's consequences land, and calls `alerted_to` with the attacker's position.

When awareness flips `Unaware` to `Alert`, the engine writes one message:

```rust
pub struct Noticed { pub observer: Entity, pub subject: Entity, pub at: Point }
```

A game reads it to shout, to log a line, to change the music, or to wake a squad.
Propagation is deliberately not here: what a shout carries, how far it goes and who it reaches are content, and a game that wants squads writes a dozen lines over this message rather than accepting the engine's idea of a squad.

## 7. The examples

The delve turns it on, because the delve is the dark game and stealth means nothing in daylight.
Beasts get a `Notice` from RON, the player gets a `Stealth`, and dousing the brand becomes a way past the salt ghosts rather than only a way to see less.
Corsair's caves get the same, and its surface deliberately does not: a game may carry the plugin and still have places where nothing hides.

This is the phase to stop at and play.
Everything before it can be right and still not be a game.

## 8. The UI payoff

[`Row`] gains `aware: Option<bool>`: whether this actor knows about the subject, `None` when no stealth system is running.
It goes in the view rather than arriving as a facet because it is engine-knowable and reads the same in every game, which is the test `ui.md` sets for what belongs in a view rather than on a row.

The nearby rail then shows which of the things in sight have noticed you, which is the readout that makes the mechanic playable rather than mysterious.
The vitals strip reads hidden or seen off the same data.

## 9. What is not in this slice

- **Two-way stealth: monsters hiding from the player.**
  The model is symmetric and the components would work, but the player has no mind to consult an `Aware`, and a hidden monster has to be kept off the map view as well as out of the panel.
  That is a render change and a separate slice.
- **Squad alerting.**
  The `Noticed` message is the seam; the propagation is the game's.
- **Sneak attack damage.**
  Phase E, and the one breaking change to a tier-1 type, so it is last and separable.
- **Noise.**
  A second sense with its own propagation is a bigger idea than this one and should not be smuggled in as a third knob on `Notice`.

## 10. Phases

- **A. The pure half.** Built.
  `Notice`, `Stealth`, `notices`, `Awareness` and its transitions, in `rl-rules::ai::awareness`, with the property tests of section 11.
  Nothing behaves differently.
- **B. The Bevy half.** Built.
  The components, `Aware`, `StealthPlugin`, `DecideSet::Notice`, `update_awareness`, `wake_on_damage`, `Noticed`, the `Snapshot::last_known` field and the `SearchLastKnown` tactic.
  Waits on the abilities slice, which is in flight in the same four files.
- **C. The delve, and Corsair's caves.** Built.
  Play it before going on.
- **D. The panels.** Built.
  `Row::aware`, the rail showing who has noticed you, the vitals reading.
- **E. Sneak attacks.**
  `Defender` gains `unaware`, so a game can write its own damage stage for it.
  The engine ships no multiplier: that is balance.

## 11. Tests

Pure, in `rl-rules`:

- Inside `certain` a subject is noticed whatever the roll, and outside `Perception` it is never noticed whatever the roll.
- `lit_bonus` widens the certain radius by exactly itself and by nothing else.
- `quiet` narrows it, and the floor of one holds against any stack of gear.
- Over a seed range: a subject sitting still at a distance with a non-zero chance is noticed within a bounded number of turns, and with a zero chance outside `certain` it is never noticed at all.
- `lost` returns to `Unaware` exactly on the turn `forget_after` passes, and a sighting in between resets the count rather than only the position.

Headless, in `rl-bevy`:

- A monster with `Notice` and a player with `Stealth` in the dark does not hunt; light the tile and the next turn it does.
- A monster that is hit wakes even when it could not possibly have seen the blow.
- A player who leaves sight is followed to the last known tile and no further, and the monster is wandering again after `forget_after`.
- `Noticed` is written once on the flip and not again while the awareness holds.
- Without `StealthPlugin` every existing example test passes unchanged, which is the assertion that the opt-in is real.

## 12. Cost

One roll per aware-able subject per actor-turn, which is one roll per monster-turn in a game with a player and no other hiders.
`Aware` is a `BTreeMap` whose realistic length is one.
Nothing here runs per frame, and nothing here allocates per turn beyond the map's single entry.

## 12b. What the build changed

- **Two names per idea, one per tier.**
  The tier-1 data is `NoticeStats` and `StealthStats`; the components are `Notice` and `Stealth`.
  One name for both would have put two types called `Notice` into the facade's prelude, which globs `rl-rules` and `rl-bevy` together.
- **`memory` lives on `NoticeStats`.**
  Section 4 had `forget_after` as an argument.
  It is authored per kind of observer, so a salt ghost that searches for eight turns and a crab that gives up after three are data rather than code.
- **`notices` does not take perception.**
  The cap is `within_reach`, applied by the caller beside the line-of-sight check, so the pure function answers only whether the observer paid attention.
- **An alert observer keeps its subject for as long as it can perceive it.**
  The roll decides only whether an unaware observer becomes aware.
  Rolling every turn would let a monster in plain view lose you to a bad number, which is the flicker section 13 warned of; this way losing takes `memory` turns out of sight and noticing takes one.
- **Whether stealth runs is asked of the plugin, not of the components.**
  `Notice` brings an `Aware` with it, so a game that authors observers and never adds `StealthPlugin` would have got monsters that notice nothing, forever, rather than the old behaviour.
  `StealthRunning` is true only when the plugin was added, and the delve's own test harness, which does not add it, is what caught this.
- **The line-of-sight oracle is one function.**
  `perceivable` in `minds.rs` (it began in `combat.rs`, before the minds had a module of their own) is shared by `decide_minds` and by noticing, so the two can never disagree about who could be seen.

Found on the way, both fixed: a mind that had not noticed the player could still descend the shared flow fields, which are built toward the player, and so walk straight to someone it never saw; and the delve had no way to put the brand out, so a quiet player carrying a lit brand was never quiet at all. Shift and `L` now smothers it and spends the turn.

Found in play afterwards: Corsair's vitals strip read hidden while a surface cutthroat cut the player down.
"Seen" asked only the observers that keep an `Aware`, and a monster with no `Notice` sees on sight without ever keeping one, so the observers that were actually attacking were the ones never asked.
`Watchers` in `rl-bevy` now answers "who is watching whom" by the rule the minds act on - an observer that keeps track watches what it knows about, one that sees on sight watches what it can perceive - and both the vitals strip and the nearby rail read it.
Reproduced first as a Corsair test through the real wiring, with a surface cutthroat at the player's elbow.

## 13. Risks

- **A monster that oscillates at the edge of `certain`.**
  Noticing and losing on alternate turns would read as a broken monster rather than a tense one.
  `forget_after` is the damper: losing takes several turns and noticing takes one, so the hysteresis is built into the state machine rather than bolted on.
- **Stealth that is invisible to the player.**
  A mechanic the player cannot read is a mechanic that feels like the game cheating, which is why phase D is in this slice and not deferred.
  If the panels slip, the mechanic should slip with them.
- **The oracle.**
  Everything a mind sees is still routed through the player's viewshed.
  That is a pre-existing simplification, it is documented in `decide_minds`, and stealth neither fixes nor worsens it, but a reader meeting stealth first will expect otherwise.
- **Games that turn it on and author nothing.**
  A `Notice` absent means noticed on sight, which is today's behaviour and the safe default.
  A `Stealth` absent means the subject is never hidden.
  A game has to author both sides before anything changes, which is the right way round but worth saying, since "I added the plugin and nothing happened" is the likely first report.
- **The abilities slice.**
  It is rewriting `decide_minds`, `Snapshot`, `TacticCtx` and the tactic list right now.
  Phase B should be written against that code once it has landed, not merged into it afterwards.
