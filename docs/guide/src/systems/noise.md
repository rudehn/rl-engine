<!-- documents:
     plugins: NoisePlugin
     files: crates/rl-rules/src/ai/hearing.rs
            crates/rl-rules/src/ai/awareness.rs
            crates/rl-rules/src/ai/tactics.rs
            crates/rl-bevy/src/noise.rs
            crates/rl-bevy/src/plugin.rs
            crates/rl-bevy/src/minds.rs
            crates/rl-bevy/src/stealth.rs
            crates/rl-bevy/src/turn.rs
            crates/rl-bevy/src/combat.rs
            crates/rl-bevy/src/doors.rs
            crates/rl-bevy/src/items.rs
            crates/rl-ui/src/view/nearby.rs
            crates/rl-ui/src/panel/nearby.rs
     fingerprint: a157a707 -->

# Noise

A sound is made on a cell, floods once from it, and is over.
Walls stop it, a closed door muffles it, open ground spends a step of its loudness per step walked, and every listener it still reaches with enough left goes to look at the place it came from.
A listener hears a place, never a who: it cannot tell a friend's footsteps from an enemy's, and whether it sees anything when it arrives is the ordinary sight and notice roll.
The engine writes the noise of its own actions and a game writes the rest, as one message, heard one way.

## Turning it on

`NoisePlugin::new(NoiseRules { step, strike, door, landing, door_muffle })` is the whole of turning it on, and the rules have no defaults, because how loud a step is, is balance.
A loudness of zero makes no noise at all, which is how a game leaves one of the engine's four sources out without leaving the plugin out.
It chains `make_engine_noise` and `resolve_noise` in `TurnSet::Listen`, which sits after `TurnSet::React`: a noise a game writes answering the pass is heard in the same pass whichever way the executor ran the two, so a replay cannot tell a game's sound from the engine's.
`age_heard` goes in `DecideSet::Notice`, beside stealth's own aging, so a sound and a sighting grow stale at one point in a turn, and `follow_heard` in `PerceiveSet::Annotate`, where a mind's knowledge is filled in.
It declares `depends_on::<CorePlugin>` and nothing else: the `Thinking` it annotates, the `Stepped` the move resolver writes and the `DoorEvent` a door writes are all `CorePlugin`'s.
It registers `DamageEvent` and `ItemEvent` itself, because a game may have added neither combat nor items, and then those queues stay empty rather than panicking a reader.
Without the plugin nothing is heard however close it is made, and a `Hearing` on an actor sits there doing nothing.

## The model

`Hearing(HearingStats)` is the component, and an actor without one is deaf, which is how every actor behaved before noise existed.
`HearingStats` is two numbers: `threshold`, the whole steps of loudness that must still be on a sound when it arrives, zero hearing it to the last step it carries, and `memory`, the turns it goes on looking before it forgets, six when a content file leaves it out.
`Hearing` requires `Heard(Awareness)`, so a listener is ready to remember the moment it is spawned, and what it remembers is stealth's own `Awareness`: `Alert { at, stale_turns }` is exactly "heard something there, this many turns ago", and a second type with those two variants would be a second thing to keep true.
`Footfall(i32)` is one actor's steps in place of `NoiseRules::step`, heavier for something that clatters and zero for something that pads.
`MakeNoise { at, loudness, sound, maker }` is a sound made this pass, `loudness` in whole steps of how far it carries over open ground.
The engine reads `maker` for one thing only, that its maker does not hear it: its own step is the one sound a listener knows the source of, and without that a listener walking toward a fight hears its own foot louder than the fight and forgets the fight for it.
`NoiseHeard { listener, at, sound, maker, left }` is written for every listener a sound reached, the player included, and `left` is what was still on it when it arrived, in hundredths of a step, so the same shout arrives louder next door than across a deck.
`SoundId` is interned by `Sounds`, the engine's four first as `Sounds::STEP`, `STRIKE`, `DOOR` and `LANDING`; a game declares its own with `app.add_sound("shout")` and finds it again by name with `Sounds::get`, so there is no closed list of what can make a noise.
`make_engine_noise` writes those four: every `Stepped` the move resolver let through, at the cell stepped to; one sound per attacker per pass however many strikes its `DamageEvent`s carried, at the attacker's cell, with a mend and damage that has no attacker making none; a `DoorEvent` opened or closed, at the door; and an `ItemEvent::Thrown` where the thing came to rest.
`resolve_noise` then floods each one.
It skips a noise no listener on this map is within `loudness` Chebyshev tiles of, since no flood carries further than that, and otherwise builds `Earshot`, a single `DijkstraMap` reused by every flood so hearing allocates nothing once it has grown, over a square of side `2 * loudness + 1` clipped to the loaded window.
What a cell costs a sound is `hearing::carries`, read off flags a tile already has rather than a field of its own: what a thrown thing passes, sound passes at one step; what stops one and opens is a closed door, passed at one step and `door_muffle` more; anything else that stops one is a wall and stops sound.
`left_after` takes the walk off the loudness and `heard` asks whether what is left reaches the listener's threshold.
A listener that heard several in a pass keeps the one that arrived loudest, ties to the lower cell in `Point` order, so the order they were written in cannot change where it goes.
Every listener is told, but only a listener that is not the player has its `Heard` set: the player's turn is the game's, so the engine says what was heard and decides nothing about it.
`follow_heard` offers what the mind holding the turn heard to `Thinking::offer_trail`, unless that mind can see the cell, in which case it forgets it, because seeing the place is having looked and that is what ends a search that arrived.
Stealth offers its lost trails to the same place, the freshest offer becomes `Snapshot::last_known` with ties to the lower cell, and so which contributor ran first cannot reach a tactic.
`NoiseRunning` answers whether the plugin was added at all, asked of its message rather than of the components, since `Hearing` brings a `Heard` with it whether or not anything will ever fill it.

## Using it

A game's own noise is the same message the engine writes, so Foundry's probe sounds a klaxon that the deck's droids hear by the engine's rules and nothing else.

<!-- include: ../../../../examples/foundry/src/droids/alarm.rs:shout -->
```rust,no_run
/// Shouts the alarm for every action an [`Alarm`] carrier finishes while it
/// knows where the player is: a [`MakeNoise`] of [`ALARM_SOUND`] where it
/// stands, as loud as [`ALARM_LOUDNESS`], and a [`PULSE`] on it when the
/// player can see it there. Whoever hears it comes to look; the engine's
/// hearing decides who that is.
///
/// On the probe's own actions rather than on the clock, so a probe frozen
/// on a deck the commando left says nothing, and one that notices and acts
/// in the same pass shouts in that pass. The pulse is a cue like any
/// other, so it holds the turns while it plays and a key skips it; one out
/// of sight would give the probe away and hold the turns for nothing to
/// see, so the noise goes out and the pulse does not.
pub fn shout_alarm(
    mut done: MessageReader<ActionDone>,
    alarmed: Query<(&Position, &Aware), With<Alarm>>,
    players: Query<(Entity, &Viewshed), With<Player>>,
    sounds: Res<Sounds>,
    mut noise: MessageWriter<MakeNoise>,
    mut cues: MessageWriter<Cued>,
) {
    let alarm = sounds.get(ALARM_SOUND).expect("FoundryPlugin declares the alarm's sound");
    for ev in done.read() {
        let Ok((at, aware)) = alarmed.get(ev.actor) else { continue };
        if !players.iter().any(|(p, _)| aware.knows(p)) {
            continue;
        }
        noise.write(MakeNoise { at: at.0, loudness: ALARM_LOUDNESS, sound: alarm, maker: Some(ev.actor) });
        if !players.iter().any(|(_, sight)| sight.can_see(at.0)) {
            continue;
        }
        cues.write(Cued { actor: ev.actor, cue: Cue::Burst { on: vec![Anchor::on(ev.actor, at.0)], look: LookOf::Given(PULSE), from: None } });
    }
}
```

And what the engine's own actions cost is one constant, handed to the plugin as it is added.

<!-- include: ../../../../examples/foundry/src/droids/alarm.rs:noise_rules -->
```rust,no_run
/// How loud the engine's own actions are on a deck. A blow or a shot
/// carries ten steps, so a firefight draws the droids in earshot; steps,
/// doors and a thrown thing landing make no sound worth hearing over the
/// machinery, and a shut bulkhead takes three steps off anything that
/// passes it.
pub const NOISE: NoiseRules = NoiseRules { step: 0, strike: 10, door: 0, landing: 0, door_muffle: 3 };
```

## The line

The engine decides how far a sound carries and who it reaches; a game decides what is worth making a sound about, and how loud.
A listener is told a place, and the engine tells it nothing about what happened there: `sound` and `maker` ride along for a game's own reactions and the engine reads neither, except to spare a listener its own.
Hearing and stealth are two levers that never read each other: `Stealth::quiet` is how hard you are to see and `Footfall` is how loud you are to walk, and all hearing does for noticing is bring a monster close, where the notice roll is likely to land.
What follows a sound is a tactic reading `last_known`: `SearchLastKnown` walks to the place, `Keep::enemies` keeps station on it once nothing is in sight and `Hover` holds while it is remembered, each of them only for a mind whose `Wits` hold `SEARCHES`, so a mind without the wit, or with none of those tactics in its brain, hears the sound and does nothing with it.
Nothing here persists between turns: a noise never outlives the pass it was made in, so there is no field to step and nothing to save, and `Heard` is lost on load the way awareness is, which is a monster on its way to look at a sound forgetting it.
A game that adds the plugin and authors no `Hearing` anywhere hears nothing at all, which is the right way round and the likely first report.
What the player reads off it is `Alert::Searching` on a nearby row, which is exactly this: something on its way to a noise that has not seen you, named in the game's own words through `AlertWords`.
Per-weapon loudness and per-tile deadening are not here: a knife and a pistol are both `strike`, and a thick carpet is a `TileProps` field on the day a game asks for one.

## Where it lives

`rl-rules` is tier 1 and has no Bevy in it: `ai/hearing.rs` is four items and no world, the stats, what a cell costs a sound, what is left after a walk and whether that reaches a threshold, so the property the whole subsystem rests on, that a sound of loudness `n` reaches a threshold of zero at exactly `n` steps and not one more, is proved as arithmetic with no `App` under it.
`ai/awareness.rs` is where `Awareness` lives, shared with stealth, so a heard place and a lost subject go stale by one state machine and a panel reading either reads one type.
`rl-bevy` is tier 2 and owns where a sound goes: `noise.rs` is the plugin, the components, the messages, the flood and the two systems that reach into a mind's turn, and it asks `hearing::carries` what each cell costs rather than deciding that itself.
`plugin.rs` fixes `TurnSet::Listen` between the pass's reactions and its record, which is the one ordering decision that makes a game's noises and the engine's indistinguishable in a replay.
