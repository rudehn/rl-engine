<!-- documents:
     plugins: NoisePlugin
     files: crates/atglyph-rules/src/ai/hearing.rs
            crates/atglyph-rules/src/ai/awareness.rs
            crates/atglyph-rules/src/ai/tactics.rs
            crates/atglyph-bevy/src/noise.rs
            crates/atglyph-bevy/src/plugin.rs
            crates/atglyph-bevy/src/minds.rs
            crates/atglyph-bevy/src/stealth.rs
            crates/atglyph-bevy/src/turn.rs
            crates/atglyph-bevy/src/combat.rs
            crates/atglyph-bevy/src/doors.rs
            crates/atglyph-bevy/src/items.rs
            crates/atglyph-bevy/src/effects/engine.rs
            crates/atglyph-bevy/src/props.rs
            crates/atglyph-bevy/src/status.rs
            crates/atglyph-ui/src/view/nearby.rs
            crates/atglyph-ui/src/panel/nearby.rs
     fingerprint: 6932947e -->

# Noise

A sound is made on a cell, floods once from it, and is over.
Walls stop it, a closed door muffles it, open ground spends a step of its loudness per step walked, and every listener it still reaches with enough left goes to look at the place it came from.
A listener hears a place, never a who: it cannot tell a friend's footsteps from an enemy's, and whether it sees anything when it arrives is the ordinary sight and notice roll.
The engine writes the noise of its own actions and a game writes the rest, as one message, heard one way.

## Turning it on

`NoisePlugin::new(NoiseRules { step, strike, door, landing, door_muffle })` is the whole of turning it on, and the rules have no defaults, because how loud a step is, is balance.
A loudness of zero makes no noise at all, which is how a game leaves one of the engine's four sources out without leaving the plugin out.
It chains `make_engine_noise`, `resolve_noise` and `wake_on_noise` in `TurnSet::Listen`, which sits after `TurnSet::React`: a noise a game writes answering the pass is heard in the same pass whichever way the executor ran the two, so a replay cannot tell a game's sound from the engine's.
`age_heard` goes in `DecideSet::Notice`, beside stealth's own aging, so a sound and a sighting grow stale at one point in a turn, and `follow_heard` in `PerceiveSet::Annotate`, where a mind's knowledge is filled in.
It declares `depends_on::<CorePlugin>` and nothing else: the `Thinking` it annotates, the `Stepped` the move resolver writes and the `DoorEvent` a door writes are all `CorePlugin`'s.
It registers `DamageEvent` and `ItemEvent` itself, because a game may have added neither combat nor items, and then those queues stay empty rather than panicking a reader.
It registers the `Noise` effect as well, so content that makes a sound loads exactly when the game can hear one.
`wake_on_noise`, last in that chain, cures a sleeping status on whoever heard a noise with its `Hearing` threshold left on it; the system lives beside the other two that wake a sleeper, in `status.rs`, since only this plugin knows when a noise reached someone.
Without the plugin nothing is heard however close it is made, a `Hearing` on an actor sits there doing nothing, and a sleeper wakes to harm and sight alone.

## The model

`Hearing(HearingStats)` is the component, and an actor without one is deaf, which is how every actor behaved before noise existed.
`HearingStats` is two numbers: `threshold`, the whole steps of loudness that must still be on a sound when it arrives, zero hearing it to the last step it carries, and `memory`, the turns it goes on looking before it forgets, six when a content file leaves it out.
`Hearing` requires `Heard(Awareness)`, so a listener is ready to remember the moment it is spawned, and what it remembers is stealth's own `Awareness`: `Alert { at, stale_turns }` is exactly "heard something there, this many turns ago", and a second type with those two variants would be a second thing to keep true.
`Footfall(i32)` is one actor's steps in place of `NoiseRules::step`, heavier for something that clatters and zero for something that pads.
`MakeNoise { at, loudness, sound, maker }` is a sound made this pass, `loudness` in whole steps of how far it carries over open ground.
The engine reads `maker` for one thing only, that its maker does not hear it: its own step is the one sound a listener knows the source of, and without that a listener walking toward a fight hears its own foot louder than the fight and forgets the fight for it.
`NoiseHeard { listener, at, sound, maker, left }` is written for every listener a sound reached, the player included, and `left` is what was still on it when it arrived, in hundredths of a step, so the same shout arrives louder next door than across a deck.
`SoundId` is interned by `Sounds`, the engine's four first as `Sounds::STEP`, `STRIKE`, `DOOR` and `LANDING`; a game declares its own with `app.add_sound("shout")` and finds it again by name with `Sounds::get`, so there is no closed list of what can make a noise.
`SoundNames` puts every sound declared so far in scope where content is loaded, `registries.names().sounds(&sounds)`, since a sound is interned while the app is built and is not in `Registries`; the engine's prop loader chains it on itself, and a game with noise chains it wherever it builds its items' or abilities' effects.
`Noise { sound, loudness }` is the effect content makes a sound with, `(kind: "Noise", args: (sound: "blast", loudness: 14))`: one `MakeNoise` by the landing's user where a projectile stopped, else where it was aimed, one sound however wide the burst, and a sound nobody declared or a `loudness` below one refused at load.
`make_engine_noise` writes those four: every `Stepped` the move resolver let through, at the cell stepped to; one sound per attacker per pass however many strikes its `DamageEvent`s carried, at the attacker's cell, with damage that has no attacker making none; a `DoorEvent` opened or closed, at the door; and an `ItemEvent::Thrown` where the thing came to rest.
`resolve_noise` then floods each one.
It skips a noise no listener on this map is within `loudness` Chebyshev tiles of, since no flood carries further than that, and otherwise builds `Earshot`, a single `DijkstraMap` reused by every flood so hearing allocates nothing once it has grown, over a square of side `2 * loudness + 1` clipped to the loaded window.
What a cell costs a sound is `hearing::carries`, read off flags a tile already has rather than a field of its own: what a thrown thing passes, sound passes at one step; what stops one and opens is a closed door, passed at one step and `door_muffle` more; anything else that stops one is a wall and stops sound.
`left_after` takes the walk off the loudness and `heard` asks whether what is left reaches the listener's threshold.
Listeners are told in row and column order, and one that heard several in a pass keeps the one `hearing::loudest` picks, the most left with a tie to the earlier cell in row-major order, so neither the order they were written in nor an entity's number can change where it goes.
Every listener is told, but only a listener that is not the player has its `Heard` set: the player's turn is the game's, so the engine says what was heard and decides nothing about it.
`follow_heard` offers what the mind holding the turn heard to `Thinking::offer_trail`, unless that mind can see the cell, in which case it forgets it, because seeing the place is having looked and that is what ends a search that arrived.
Stealth offers its lost trails to the same place, the freshest offer becomes `Snapshot::last_known` with ties to the lower cell, and so which contributor ran first cannot reach a tactic.
`NoiseRunning` answers whether the plugin was added at all, asked of a marker only the plugin inserts rather than of the components or the messages, since `Hearing` brings a `Heard` with it whether or not anything will ever fill it and a view registers `NoiseHeard` for itself.

## Using it

A game's own noise is the same message the engine writes, so an alarm is a `MakeNoise { at, loudness, sound, maker }` written where its sounder stands, its `sound` one the game declared in `Sounds`, and whoever is in earshot hears it by the engine's rules and nothing else.

And what the engine's own actions cost is one constant, handed to the plugin as it is added: `NoisePlugin::new(NoiseRules { step: 0, strike: 10, door: 0, landing: 0, door_muffle: 3 })` is a place loud enough that only a blow or a shot carries, ten steps, and a shut door takes three off anything that passes it.

## The line

The engine decides how far a sound carries and who it reaches; a game decides what is worth making a sound about, and how loud.
A thing that makes a sound where it lands, a grenade or an alarm plate, says so in its content through `Noise` rather than in a system of the game's, so its loudness sits in the file beside what else it does and its thrower, as its maker, does not hear it.
A listener is told a place, and the engine tells it nothing about what happened there: `sound` and `maker` ride along for a game's own reactions and the engine reads neither, except to spare a listener its own.
Hearing and stealth are two levers that never read each other: `Stealth::quiet` is how hard you are to see and `Footfall` is how loud you are to walk, and all hearing does for noticing is bring a monster close, where the notice roll is likely to land.
What follows a sound is a tactic reading `last_known`: `SearchLastKnown` walks to the place, `Keep::enemies` keeps station on it once nothing is in sight and `Hover` holds while it is remembered, each of them only for a mind whose `Wits` hold `SEARCHES`, so a mind without the wit, or with none of those tactics in its brain, hears the sound and does nothing with it.
Nothing here persists between turns: a noise never outlives the pass it was made in, so there is no field to step and nothing to save, and `Heard` is lost on load the way awareness is, which is a monster on its way to look at a sound forgetting it.
A game that adds the plugin and authors no `Hearing` anywhere hears nothing at all, which is the right way round and the likely first report.
What the player reads off it is `Alert::Searching` on a nearby row: something on its way to a noise that has not seen you, named in the game's own words through `AlertWords`, and the same reading stealth gives something on its way to where it lost you to the unseen.
Per-weapon loudness and per-tile deadening are not here: a knife and a pistol are both `strike`, and a thick carpet is a `TileProps` field on the day a game asks for one.

## Where it lives

`atglyph-rules` is tier 1 and has no Bevy in it: `ai/hearing.rs` is five items and no world, the stats, what a cell costs a sound, what is left after a walk, whether that reaches a threshold and which of several a listener goes to, so the property the whole subsystem rests on, that a sound of loudness `n` reaches a threshold of zero at exactly `n` steps and not one more, is proved as arithmetic with no `App` under it.
`ai/awareness.rs` is where `Awareness` lives, shared with stealth, so a heard place and a lost subject go stale by one state machine and a panel reading either reads one type.
`atglyph-bevy` is tier 2 and owns where a sound goes: `noise.rs` is the plugin, the components, the messages, the flood and the two systems that reach into a mind's turn, and it asks `hearing::carries` what each cell costs rather than deciding that itself.
`plugin.rs` fixes `TurnSet::Listen` between the pass's reactions and its record, which is the one ordering decision that makes a game's noises and the engine's indistinguishable in a replay.
