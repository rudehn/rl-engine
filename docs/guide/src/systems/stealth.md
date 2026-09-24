<!-- documents:
     plugins: StealthPlugin
     files: crates/rl-rules/src/ai/awareness.rs
            crates/rl-rules/src/ai/tactics.rs
            crates/rl-bevy/src/stealth.rs
            crates/rl-bevy/src/minds.rs
            crates/rl-bevy/src/noise.rs
            crates/rl-bevy/src/combat.rs
            crates/rl-bevy/src/components.rs
            crates/rl-bevy/src/lighting.rs
            crates/rl-ui/src/view/nearby.rs
     fingerprint: 53d5e741 -->

# Stealth

Without this a mind acts on everything its own sight reaches the turn it first reaches it, which leaves a player nothing to break and a monster nothing to regain.
Stealth is the layer between "could be seen" and "has been seen": a roll to notice, and a memory that decays.
Noticing is two knobs rather than a radius, because a radius alone is a hard line the player learns to stand behind and a chance alone is a lottery with no readable edge.
What an observer knows is per observer and per subject, so a monster may be unaware of you and perfectly aware of the thief beside it, and one that loses you searches where it last saw you before it forgets.

## Turning it on

`StealthPlugin` adds three systems, and the message and the stream they need, and nothing else: `update_awareness` in `DecideSet::Notice`, before a mind decides; `filter_unnoticed` in `PerceiveSet::Filter`, after the roster stage put everyone in sight into a snapshot; and `wake_on_damage` in `TurnSet::React`, where a turn's consequences land.
It declares `depends_on::<MindsPlugin>`, since noticing is a thing minds act on, and the stream is `StealthRng`, so a tactic added to a brain or a blow struck elsewhere cannot shift which turn a guard spots you on.
Both sides have to be authored before anything changes: a `Notice` absent means the observer sees on sight, which is the behaviour before stealth existed, and a `Stealth` absent means the subject never hides.
That is the right way round, and it is why "I added the plugin and nothing happened" is the likely first report.
The plugin is opt-in per game and the components are opt-in per spawn, so a game may carry it and still have places where nothing hides.
Lighting is optional under it: with no `Lighting` resource every cell counts as lit, and with one, a subject standing in light widens the observer's certain radius by `lit_bonus`, which is zero unless a game says otherwise.
Combat is optional too: with no `CombatRules` nobody has a side, so everyone is at odds with everyone and every observer rolls against every hider.

## The model

`NoticeStats` is the observer's half: `certain`, the tiles inside which it spots you whatever the roll; `chance_pct`, its chance a turn beyond that; `lit_bonus`, added to `certain` while you stand in light; and `memory`, the turns it keeps looking after losing you, six when a content file leaves it out.
`StealthStats` is the subject's: `quiet` off the certain radius and `subtlety` off the chance, both defaulting to nothing.
`certain_radius` is `certain` plus the light bonus less `quiet`, floored at one, so no stack of gear hides you from somebody standing next to you; `notice_chance` is the chance less `subtlety`; `notices` is either of them answering yes.
`Notice(NoticeStats)` and `Stealth(StealthStats)` are the components, one name per tier so that globbing both crates into a prelude does not put two types called `Notice` in it.
`Notice` requires `Aware(BTreeMap<Entity, Awareness>)`, keyed only by the subjects the observer has an opinion of, which in most games is the player alone: a map for correctness and one entry in practice, and a `BTreeMap` because it is read on the decision path and walked in a fixed order.
`Awareness` is `Unaware` or `Alert { at, stale_turns }`, and the third state is derivable rather than stored: alert and in sight is hunting, alert and out of sight is searching.
`saw(at)` resets the staleness as well as the position, which is what stops a monster giving up on the turn it catches you; `lost(memory)` returns to `Unaware` once the count passes `memory`; `alerted_to(at)` is anything that tells it without its looking, and is the same call.
`update_awareness` runs for the actor holding the turn and no other, and never for the player, which carries no `Aware` worth filling and never rolls to notice whatever `Notice` is put on it, so noticing costs one roll per subject per monster-turn and nothing per frame.
It walks its subjects in spawn order rather than archetype order, since which subject gets which roll must not depend on how the world happens to be laid out.
A roll decides only whether an unaware observer becomes aware: one already alert keeps its subject for as long as it can perceive it, so a monster in plain view does not lose you to a bad number, and losing takes `memory` turns out of sight while noticing takes one.
What "could be seen" means is the observer's own `Viewshed` and `within_reach` of its `Perception`, `DEFAULT_PERCEPTION` without one, so the minds, the roll and the panels can never disagree about who could be seen.
`Noticed { observer, subject, at }` is written once, on the flip from unaware, and never again while the awareness holds.
`filter_unnoticed` takes the hiders the mind holding the turn has not noticed back out of its enemies and leaves a mind that keeps no `Aware` alone, then offers every subject it is alert to but cannot see as a trail through `Thinking::offer_trail`.
Noise offers its own to the same place, the freshest becomes `Snapshot::last_known`, and `SearchLastKnown` walks to it.
`wake_on_damage` wakes whoever takes a blow from something carrying `Stealth` and points it at the attacker's cell: a mend is not a blow, and a blow armor stopped at zero still wakes it.
`StealthRunning` answers whether the plugin was added, asked of its message rather than of the components, because `Notice` brings an `Aware` with it and a game that authored observers without the plugin would otherwise have monsters that notice nothing forever.
`Watchers` answers who is watching whom by the rule the minds act on: an observer that keeps an `Aware` watches what it knows about, one that does not watches whatever its own sight reaches, and neither watches anything it is not at odds with.

## Using it

An observer is authored where it is meant to be fooled, so Corsair puts `Notice` on what walks its caves and deliberately not on what walks its islands in daylight.

<!-- include: ../../../../examples/corsair/src/monsters.rs:underground -->
```rust,no_run
    /// Spawns one `id` standing at `p` on the current map, underground, where
    /// whatever carries a lantern has it lit.
    pub fn spawn_underground(&self, commands: &mut Commands, id: rl_engine::rl_core::Id<MonsterDef>, p: Point) -> Entity {
        let e = self.spawn(commands, id, p);
        if let Some(lantern) = self.defs.get(id).lantern {
            commands.entity(e).insert(lantern);
        }
        // Only below: the caves are dark and have somewhere to hide, and
        // the islands in daylight deliberately do not.
        if let Some(notice) = self.defs.get(id).notice {
            commands.entity(e).insert(Notice(notice));
        }
        e
    }
```

The engine carries `Noticed` no further than writing it, and a game decides what being seen sets off.

<!-- include: ../../../../examples/heist/src/main.rs:alarm -->
```rust,no_run
/// A watchman who spots you shouts, and a hound bays: a noise of the
/// game's own at the watcher, which everyone in earshot comes to.
fn raise_alarm(
    mut noticed: MessageReader<Noticed>,
    mut noise: MessageWriter<MakeNoise>,
    player: Query<Entity, With<Player>>,
    watchers: Query<&Position>,
    sounds: Res<Sounds>,
) {
    let Ok(me) = player.single() else { return };
    let shout = sounds.get("shout").expect("declared in main");
    for ev in noticed.read() {
        if ev.subject != me {
            continue;
        }
        if let Ok(at) = watchers.get(ev.observer) {
            noise.write(MakeNoise { at: at.0, loudness: SHOUT, sound: shout, maker: Some(ev.observer) });
        }
    }
}
```

## The line

The engine decides who has noticed whom; a game decides what that is worth.
Propagation is deliberately absent: what a shout carries, how far it goes and who it reaches are content, so a game that wants a squad writes a dozen lines over `Noticed` rather than accepting the engine's idea of a squad.
Sneak damage is absent for the same reason, since a multiplier is balance.
Stealth hides a subject from minds and from nothing else: the drawing is untouched, so a monster is never hidden from the player, and two-way stealth would be a render change rather than another component.
A `Perception` is still the hard cap on how far an actor notices anything at all, and `Notice` is only the curve inside it, which is how a game gives a guard long sight and poor attention.
`notices` takes a `lit` flag rather than a `Lighting`, so it stays pure and a game is free to decide exposure means standing in water, or on open ground, or having shouted a moment ago.
Light is the one exposure term the engine ships, and it is one number, so a creature with `lit_bonus: 0` is one that hunts by something other than the eye without the engine learning a word for it.
Hearing is a separate lever that this never reads: it brings a monster close, and close is where the roll is likely to land.
What the player reads off it is `Alert::Hunting` on a nearby row, and hunting outranks searching, since something that has seen you is not still wondering about a noise.
`Aware` is not saved, so a monster that had noticed you has forgotten by the time a continued run begins.

## Where it lives

`rl-rules` is tier 1 and has no Bevy in it: `ai/awareness.rs` is the two stat blocks, the three functions over them and the state machine, with the caller doing the rolling and the caller deciding what lit means.
That is what lets the properties be proved rather than watched: that light widens the certain radius by exactly its bonus and by nothing else, that `quiet` narrows it and the floor of one holds against any stack of gear, and that `lost` returns to `Unaware` on exactly the turn the memory passes while a sighting in between resets the count.
Both stat blocks are serde-ready, so an observer's attention and a subject's quiet are written in a bestiary file rather than in Rust.
`rl-bevy` is tier 2 and owns the rolling: `stealth.rs` is the components, `Aware`, the stream, the three systems and the system parameters that answer whether stealth is running and who is watching.
`Watchers` lives there rather than in a panel because the vitals strip and the nearby rail must read the same answer the minds act on, and the bug that put it there was a strip reading hidden while a cutthroat cut the player down.
