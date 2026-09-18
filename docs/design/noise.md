# Noise and hearing

Status: proposed 2026-09-17, against `main` at `56a60eb`.
Built in phases A to E below.

## 0. Summary

Stealth gave monsters one sense, and `stealth.md` §9 deferred the second: "a second sense with its own propagation is a bigger idea than this one and should not be smuggled in as a third knob on `Notice`."
This is that sense.

Today a monster knows about nothing it cannot see.
The heist fakes hearing by hand: `hear_pebbles` walks every watcher, measures a Chebyshev radius through walls, and enters the thrown pebble in each one's `Aware` as a subject disguised with `quiet: 99`, with a `Fading` countdown to despawn it.
That is the pattern this engine exists to end: the engine left the behaviour out, so the game built it, and built it wrong.

Four decisions shape the rest:

1. **The engine makes noise, and so does the game.**
   A step, a blow, a door and a thrown thing landing are the engine's own actions, so the engine writes their noise, with the loudness as data.
   A game writes the same message for anything else: a shout, a spell, a collapsing shelf.
2. **Sound floods once, when it is made.**
   Each noise is one bounded Dijkstra flood from the cell it was made on.
   Walls stop it, a closed door muffles it, and open ground costs one step of loudness per step.
   Nothing persists between turns, so there is nothing to save and nothing to replay.
3. **A listener hears a place, not a who.**
   Hearing is not noticing.
   A monster that hears something goes to look, and whether it then sees anyone is the ordinary sight and notice roll.
   The engine never tells a listener who made a sound or what was going on there; it only spares a listener its own.
4. **Hearing stands apart from stealth.**
   What was heard is kept in its own component, not in `Aware`, so a game with noise and no stealth has monsters that come round the corner to see what the clatter was.

The engine never says footsteps, shouts or alarms.
It knows a `MakeNoise`, a listener's `Hearing`, and what it last `Heard`.

## 1. What noise buys

- **Fights draw a crowd.**
  A monster fighting out of sight of its friends is heard, and they come.
  That is the support behaviour packs never had, without a line of squad code.
- **Doors mean something besides sight.**
  A closed door muffles, so fighting behind one is quieter than fighting in the open, and opening one is itself a sound.
- **Distraction is a mechanic, not a hack.**
  A thrown thing lands with a sound, and the listeners who hear it go to where it landed and not to where it was thrown from.
- **Loud and quiet become a trade.**
  An actor's `Footfall` is how heavy its steps are, so heavy armor or a clattering crab can be heard coming, and a padded foot cannot.
- **The player hears too.**
  A player with `Hearing` gets the same message a monster does, and a game logs "you hear a door open to the east".

## 2. The model

### The pure half, in `rl-rules::ai::hearing` (tier 1)

```rust
/// How keenly a listener hears.
pub struct HearingStats {
    /// Loudness must arrive with at least this much left to be heard.
    pub threshold: i32,
    /// Turns it goes on looking for a sound before it forgets.
    pub memory: u32,
}

/// What a sound can travel through: open ground at one step, a door that
/// opens at one step and `muffle` more, and nothing else.
pub fn carries(blocks_projectiles: bool, opens: bool, muffle: i32) -> Option<u32>;

/// How much of `loudness` is left after travelling `travelled` hundredths
/// of a step, in hundredths.
pub fn left_after(loudness: i32, travelled: i32) -> i32;

/// Whether what is left reaches a listener of `threshold`.
pub fn heard(left: i32, threshold: i32) -> bool;
```

Loudness and threshold are authored in whole steps, like `Perception`, because "heard eight tiles off" is how a designer thinks.
The flood runs in hundredths of a step, the engine's one unit for costs, so a diagonal spends 141 of a sound's loudness and not 100.

What sound passes through is read from what already exists rather than a new tile field.
A tile a thrown thing passes, sound passes.
A tile that stops a thrown thing and opens is a closed door, and sound passes it at a cost.
Anything else that stops a thrown thing is a wall, and stops sound too.
Gas already reads the same flag, so a content file needs nothing new.

What a listener remembers is an `Awareness`, the type stealth already has.
`Alert { at, stale_turns }` is exactly "heard something at this cell, this many turns ago", `alerted_to` is hearing it, and `lost(memory)` is forgetting it.
A second type with the same two variants would be a second thing to keep true.

### The Bevy half, in `rl-bevy::noise` (tier 2)

```rust
/// A sound, made this turn.
pub struct MakeNoise {
    pub at: Point,
    /// In whole steps: how far it carries over open ground.
    pub loudness: i32,
    /// What it was, for a game phrasing a log line.
    pub sound: SoundId,
    /// Who made it, for a game's own reactions. The engine reads it only
    /// so its maker does not hear it.
    pub maker: Option<Entity>,
}

/// A listener heard a noise.
pub struct NoiseHeard { pub listener: Entity, pub at: Point, pub sound: SoundId, pub maker: Option<Entity> }

/// How keenly this actor hears. Absent, it is deaf.
#[require(Heard)]
pub struct Hearing(pub HearingStats);

/// What it last heard, and how long ago.
pub struct Heard(pub Awareness);

/// Heavier or lighter steps than the rules' default, in steps of loudness.
pub struct Footfall(pub i32);
```

`SoundId` is interned, the way `ToneId` is.
`Sounds` holds the engine's four, `Sounds::STEP`, `STRIKE`, `DOOR` and `LANDING`, and a game adds its own with `app.add_sound("shout")`.
There is no closed enum of what can make a noise.

`NoisePlugin::new(NoiseRules { step, strike, door, landing, door_muffle })` is how a game turns it on.
The rules have no defaults, since how loud a step is is balance.
A loudness of zero makes no noise at all, which is how a game turns one source off.

## 3. Where noise comes from

The engine writes a `MakeNoise` for each of these, from messages the turn already produces:

- **A step**, from `Stepped`, a new message `resolve_moves` writes for every step it lets through.
  Loudness is the stepper's `Footfall` if it has one, and `NoiseRules::step` otherwise.
- **A blow**, from a `DamageEvent` whose hit has an attacker and does not mend, made at the attacker's cell.
  One per attacker per pass, however many strikes the blow carried, so a flurry is one sound and not four.
  Melee, a shot, a thrown thing that hits and an ability that harms all come through here, since all of them write the same message.
  Damage over time has no attacker and makes no noise.
- **A door**, from `DoorEvent`, opened or closed, at the door.
- **A landing**, from `ItemEvent::Thrown`, where the thing came to rest.

The plugin registers each message it reads, so it works whether or not a game added doors, items or throwing.

## 4. Propagation

`resolve_noise` takes every noise made this pass and, for each one:

1. Skips it cheaply if no listener on the same map is within `loudness` in straight-line distance, since a flood cannot carry further than that.
2. Floods a `DijkstraMap` from the cell, over a square of side `2 * loudness + 1`, clipped to the loaded window.
   The map is a scratch resource reset for each flood, so a flood allocates nothing once it has grown.
3. For each listener the flood reached, works out what is left and asks `heard`.

A listener that hears more than one noise in a pass keeps the one that arrived loudest, with ties going to the lower cell in `Point` order, so a replay agrees.
A sound heard in a later pass replaces an older one: newer is what a listener goes to.

It runs in a new `TurnSet::Listen`, between `React` and `Cleanup`.
A game writes its own noises in `React`, where a turn's consequences are answered, and the engine's are written before that, in `Resolve`.
Were the resolver in `React` too, a shout written there would be heard in this pass or the next depending on which system the executor ran first, and a replay would diverge.

## 5. What a listener does with it

**Who made a sound decides nothing but that its maker does not hear it.**
A listener cannot tell an ally's footsteps from an enemy's, and does not try.
It hears a sound, and it goes to see.
It does know its own footsteps: without that, a listener walking toward a fight would hear its own step louder than the fight, and forget the fight for it.

**A sound at a cell it can see is not investigated.**
In the perceive stage, if the thinker can see the cell it heard, it forgets the sound: it has looked, and anything there is already in its snapshot, or is hiding and up to the notice roll.
This is also what ends a search that arrived.

**The trail it follows is the freshest one.**
`Thinking` gains `offer_trail(at, stale_turns)`, and stealth and noise each offer theirs.
`Thinking` puts the freshest in `Snapshot::last_known` when the snapshot is closed, ties to the lower cell, so the order the contributors ran in cannot reach a tactic.
`SearchLastKnown` is unchanged: it walks to `last_known`, and a brain of `MeleeAdjacent`, `Hunt`, `SearchLastKnown`, `Wander` now goes to what it heard as well as what it lost.
A mind whose wits do not search does not follow sounds either.

**It forgets.**
`Heard` ages once per turn the listener holds, in `DecideSet::Notice`, and is forgotten after `memory` turns, as awareness is.

## 6. Hearing and stealth

They are two separate levers.
`Stealth::quiet` is how hard you are to see; `Footfall` is how loud you are to walk.
Hearing draws a monster close, and close is where the notice roll is likely to succeed: that is the whole of how the two combine, and neither reads the other.
`wake_on_damage` is unchanged.

## 7. Saving

Nothing new.
`MakeNoise` never outlives the pass it was made in.
`Heard` is what a listener remembers, and like `Aware`, which is not saved either, it is lost on load: a monster that was going to look at a sound forgets it.
Should either become worth saving, the two go into `EngineSave` together.

## 8. The examples

**The heist moves onto the engine's noise.**
`Hears`, `alert_listeners`, `hear_pebbles`, `Fading`, and the pebble's `Stealth { quiet: 99 }` all go.
A pebble is the engine's landing sound.
`raise_alarm` becomes one `MakeNoise` of the game's own "shout" at the watchman who noticed.
The watch get `hearing` in `watch.ron` in place of `hears`, and the thief's steps are quiet enough that only a hound close by hears them.
`a_thrown_pebble_draws_a_watchman_to_where_it_clattered` keeps its meaning and reads `Heard` where it read `Aware`.
One thing changes for the better: sound no longer goes through the counting house walls.

**The delve hears.**
The whale's beasts get `hearing` in `beasts.ron`, and a fight in the gullet draws what is near.

**Corsair and the tutorial are left alone**, and their passing tests are the check that the plugin is opt-in.

## 9. The UI payoff

`Row` gains `heard: Option<bool>`: whether this actor is going to look at a sound, `None` without `NoisePlugin`.
It is engine-knowable and reads the same in every game, so it is a field of the view and not a facet.
The nearby rail marks a monster coming to look apart from one that has seen you, which is what makes noise readable rather than a monster wandering over for no reason.

The heist logs `NoiseHeard` for the player: a door, a landing, a shout, with a direction.

## 10. What is not in this slice

- **Per-weapon loudness.**
  A pistol and a knife make the same `strike`; a weapon's own loudness is a field on the weapon once a game asks.
- **Per-tile sound properties.**
  A thick carpet or deep water deadening sound is a `TileProps` field once a game asks; derived costs cover walls and doors.
- **Sound that lingers.**
  An echo, or an overlay of what was heard, is a `TileField` rule, which `fields.md` already names for sound.
- **Knowing what was heard.**
  A listener that tells a fight from footsteps and cares more about one is a game's tactic over `NoiseHeard`.

## 11. Phases

- **A. The pure half.**
  `HearingStats`, `carries`, `left_after` and `heard` in `rl-rules::ai::hearing`, with the tests of §12.
  Nothing behaves differently.
- **B. The Bevy half.**
  `NoisePlugin`, `NoiseRules`, `MakeNoise`, `NoiseHeard`, `Sounds` and `add_sound`, `Hearing`, `Heard`, `TurnSet::Listen`, `resolve_noise`, aging and forgetting, and `Thinking::offer_trail` with stealth moved onto it.
- **C. The engine's sources.**
  `Stepped`, and the step, blow, door and landing noises, with `Footfall`.
- **D. The heist and the delve.**
  Play it.
- **E. The panel.**
  `Row::heard` and the heist's log line.

## 12. Tests

Pure, in `rl-rules`:

- A wall carries nothing, however loud; a closed door carries at exactly `muffle` more than the same doorway open.
- What is left never grows with distance.
- A sound of loudness `n` over open ground reaches a threshold of zero at exactly `n` steps and not `n + 1`.

Headless, in `rl-bevy`:

- A listener hears a sound behind a door but not behind a wall.
- A listener walks to a fight it cannot see, and forgets it once `memory` turns pass.
- A sound at a cell the listener can see is not followed.
- Two noises in one pass: the loudest to arrive wins, whatever order they were written in.
- A deaf actor, one with no `Hearing`, hears nothing.
- Without `NoisePlugin`, a `Hearing` listener hears nothing, and every example's tests pass unchanged.

A benchmark: a crowded floor with every monster stepping every turn, to measure what the floods cost rather than assume it.

## 13. Risks

- **Monsters that cluster on each other's footsteps.**
  A listener cannot tell its friends' steps from anyone else's, so an idle monster drifts toward one walking behind a wall.
  That reads as restless and is honest, and `step` and `Footfall` are how a game tunes it: steps quieter than monsters' thresholds are heard by nothing but a keen ear close by.
- **Adding the plugin and hearing nothing.**
  With no `Hearing` on anyone, nothing listens.
  That is the same trap `stealth.md` §13 names, and the right way round.
- **Cost.**
  With no filter by side, every step within earshot of a listener that cannot see it is a flood.
  A flood is bounded by its loudness, 81 cells at four steps, and quiet steps are the tuning; the benchmark keeps it honest.
