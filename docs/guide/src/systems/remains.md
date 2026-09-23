<!-- documents:
     plugins: RemainsPlugin
     files: crates/rl-bevy/src/remains.rs
            crates/rl-bevy/src/combat.rs
            crates/rl-bevy/src/props.rs
            crates/rl-bevy/src/stealth.rs
            crates/rl-bevy/src/noise.rs
            crates/rl-rules/src/ai/snapshot.rs
            crates/rl-save/src/run.rs
     fingerprint: b267a80b -->

# Remains

What the dead leave lying where they fell, for a game to say what that means.
Nothing is spawned and nothing is copied: the remains are the dead entity itself, kept past the frame it died in, with whatever the game spawned it with still on it.
What the engine takes off is only what the engine put on, and only what means this is alive and acting.
What it adds is a `Remains` marking when it died and who got the credit, a `Prop` so a body is something standing in a cell like any other, and a name saying it is what is left of what it was.

## Turning it on

`RemainsPlugin` adds `leave_remains` to `CleanupSet::Remove`, after `process_deaths`, and declares `depends_on::<CombatPlugin>`, since without deaths there is nothing to leave.
It is opt-in twice: the plugin decides whether any death leaves anything, and `LeavesRemains` on the spawn decides whether this one does, so a game leaves wrecks behind its machines and nothing behind its summoned things without a second plugin.
A marker on an actor in a game that never added the plugin does nothing at all.
`RemainsPlugin::naming("{what} remains")` is the same plugin with the wording set as it is added; the default `RemainsNaming` says the same thing, and a game may replace the resource later instead.
A body is a `Prop`, so a game that wants minds to walk to bodies, verbs offered on them or panels listing them adds `PropsPlugin` beside this one.
Without `PropsPlugin` a body is something named lying on the floor and nothing reads it.

## The model

`LeavesRemains` is the per-actor half of the opt-in, and carries nothing.
`Remains` is what an actor becomes: `since`, what the clock read when it died, and `credit`, whoever killed it if anyone did and that one is still in the world.
Those are the two things the engine already knew at the moment of death, and neither is a claim about a world.
`leave_remains` reads `DeathEvent`, skips the player and skips anything unmarked, and on the rest removes `WasLiving` and inserts `Position`, `Prop` and `Remains`.
The position is put back because `process_deaths` took it off with the turn and the cell in the index, and remains lie where the actor fell rather than where a game would have to remember it fell.
`WasLiving` is the one list of what an actor stops being: `Dead`, `Actor`, `Blocks`, `Health`, `Mind`, `Perception`, `Viewshed`, `Notice`, `Aware`, `Hearing` and `Heard`.
`Health` comes off rather than being left at zero, because a body left with health answers the query the damage pass makes and could be killed a second time.
`Notice` and `Hearing` come off for a subtler reason: a watcher is anything carrying a `Mind` or a `Notice` that is not `Dead`, a listener anything carrying `Hearing` that is not `Dead`, and remains are not `Dead` by design, so a body that kept them would go on watching and hearing the player with every enemy on the level dead.
`Dead` is on that list too, and taking it off is the whole of keeping the entity, since `bury_the_dead` despawns whatever still carries it at the end of the frame.
`RemainsLeft { entity, at }` is sent for each one, and `entity` is the actor that died, so a game reacting in `TurnSet::React` reads whatever it spawned that actor with.
`RemainsNaming` is a template with `{what}` standing for whatever the actor was called, and `name_as_remains` applies it, once at the death and again when a save lays a body back down.
A body is seen by a mind as a `PropView` in `Snapshot::props`, filled by `perceive_props` in `PerceiveSet::Annotate`, carrying which entity it is, where it lies and whose it was, and nothing else.
`EntityState::remains` is how a save holds it: the whole of it is optional, so a save written before remains existed still loads, and the `SaveId` inside it is optional again, since a death nobody was credited with is still a death.

## Using it

A game answers `RemainsLeft` with whatever a body of its own is, on the entity the engine kept.

<!-- include: ../../../../examples/foundry/src/props.rs:wreck -->
```rust,no_run
/// Makes a droid's remains a wreck: something to go through.
///
/// The engine kept the dead droid and named it from the remains
/// template, so it is already a prop lying where it fell. What it cannot
/// know is what a Foundry wreck looks like or that it is worth opening,
/// which is one kind in `props.ron` and one component here.
pub fn wreck_the_dead(mut commands: Commands, mut left: MessageReader<RemainsLeft>, registries: Res<Registries>) {
    let Some(id) = registries.props.id("wreckage") else { return };
    for ev in left.read() {
        // The glyph comes off with it, so the renderer dresses the wreck
        // from `props.ron` rather than leaving it drawn as the droid that
        // walked: a `%` on the deck reads as something broken.
        commands.entity(ev.entity).remove::<Glyph>().insert(PropKind(id));
    }
}
```

## The line

The engine refuses to say what remains are.
There is no glyph, no rot timer, no loot table, and no answer to whether a body can be searched, stripped, rebuilt, eaten or raised: a game answers all of it from its own components, on the same entity, reacting to `RemainsLeft`.
The one word the engine puts on a body is the name, and that only through a template a game wrote.
Whose it was is the one thing the engine tells a mind, because sides are its own: it registered them and it holds the hostility matrix, so a mind asking whether that one is ours is asking a question answerable without a word of the game's vocabulary.
Anything finer than that travels as a sense the game pushes.
The engine never removes remains, because how long the dead linger is a rule about a world, and a timer here would be a default every game either accepted without meaning to or switched off.
The player's death is the game's alone, marked or not: a run ends on it, and the corpse a game may still want to draw is not taken out from under it.
The cost of keeping the entity is real and worth saying: a game querying its own monsters by a component it added now also matches its bodies, and those queries want `Without<Remains>`.
Queries on `Actor` or `Mind`, which is most of them, are unaffected.

## Where it lives

All of it is in `rl-bevy`, in one file, because there is no rule here to test without an `App`: the whole subsystem is which components come off an entity and which go on, and the only way to ask that is to kill something and look.
`combat.rs` owns the death it reacts to and the burial it prevents; `props.rs` owns what a body is once it is one, which is why nothing about bodies appears in a mind's snapshot beyond what any prop puts there.
`rl-save` keeps the fact of the death rather than the game doing so, because a game writes down what a thing is and never that it is dead, and on restore lays the body back down from the two numbers it kept.
