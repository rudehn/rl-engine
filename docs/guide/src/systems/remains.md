<!-- documents:
     plugins: RemainsPlugin
     files: crates/atglyph-bevy/src/remains.rs
            crates/atglyph-bevy/src/combat.rs
            crates/atglyph-bevy/src/props.rs
            crates/atglyph-bevy/src/stealth.rs
            crates/atglyph-bevy/src/noise.rs
            crates/atglyph-rules/src/ai/snapshot.rs
            crates/atglyph-save/src/run.rs
     fingerprint: 85a8282c -->

# Remains

What the dead leave lying where they fell, for a game to say what that means, and to stand back up if it wants to.
The remains are the dead entity itself, kept past the frame it died in, with whatever the game spawned it with still on it.
What the engine takes off is only what the engine put on, and only what means this is alive and acting.
What it adds is a `Remains` marking when it died and who got the credit, a `Prop` so a body is something standing in a cell like any other, and a name saying it is what is left of what it was.
Beside the body it keeps a twin of the actor as it lived, which no ordinary query sees, so a body can be revived as the actor it was.

## Turning it on

`RemainsPlugin` keeps each marked actor's life in `ResolveSet::Damage`, after damage is applied, and lays the body down in `CleanupSet::Remove`, after deaths are processed, and declares `depends_on::<CombatPlugin>`, since without deaths there is nothing to leave.
It is opt-in twice: the plugin decides whether any death leaves anything, and `LeavesRemains` on the spawn decides whether this one does, so a game leaves wrecks behind its machines and nothing behind its summoned things without a second plugin.
A marker on an actor in a game that never added the plugin does nothing at all.
`RemainsPlugin::naming("{what} remains")` is the same plugin with the wording set as it is added; the default `RemainsNaming` says the same thing, and a game may replace the resource later instead.
`RevivalRules`, inserted before or after the plugin, sets how far from its body a revived actor may stand, two cells by default.
A body is a `Prop`, so a game that wants minds to walk to bodies, verbs offered on them or panels listing them adds `PropsPlugin` beside this one.
Without `PropsPlugin` a body is something named lying on the floor and nothing reads it.

## The model

`LeavesRemains` is the per-actor half of the opt-in, and carries nothing.
`Remains` is what an actor becomes: `since`, what the clock read when it died, and `credit`, whoever killed it if anyone did and that one is still in the world.
Those are the two things the engine already knew at the moment of death, and neither is a claim about a world.
The engine reads the same `DeathEvent` first, in the pass the actor dies and before anything answers it, and copies the whole actor onto a twin carrying Bevy's `Disabled`, linked from the body by `RevivalTwin`.
Every component is copied and none is named; one that is not `Clone` cannot be, and is named in an error at the moment of death.
`RevivalTwin` despawns the twin when it comes off the body or the body is despawned, and `Remains` takes `RevivalTwin` with it when it comes off, so no twin outlives its body.
Laying the body down reads `DeathEvent`, skips the player and skips anything unmarked, and on the rest puts `Position` back and calls `lay_down`, which takes off what made it living and inserts `Prop` and `Remains`.
The position is put back because the death pass took it off with the turn and the cell in the index, and remains lie where the actor fell rather than where a game would have to remember it fell.
One list says what an actor stops being: `Dead`, `Actor`, `Blocks`, `Health`, `Mind`, `Perception`, `Viewshed`, `Notice`, `Aware`, `Hearing`, `Heard`, `Post`, `Offers` and `Triggers`.
`Health` comes off rather than being left at zero, because a body left with health answers the query the damage pass makes and could be killed a second time.
`Notice` and `Hearing` come off for a subtler reason: a watcher is anything carrying a `Mind` or a `Notice` that is not `Dead`, a listener anything carrying `Hearing` that is not `Dead`, and remains are not `Dead` by design, so a body that kept them would go on watching and hearing the player with every enemy on the level dead.
`Post` comes off with them, since a body keeps no cell and walks back to none, and so do `Offers` and `Triggers`, so a captive's offer is not still made by its corpse; the twin keeps all of them, so a revival offers and answers what the living actor did.
`Dead` is on that list too, and taking it off is the whole of keeping the entity, since the engine despawns whatever still carries it at the end of the frame.
`RemainsLeft { entity, at }` is sent for each one, and `entity` is the actor that died, so a game reacting in `TurnSet::React` reads whatever it spawned that actor with.
`RemainsNaming` is a template with `{what}` standing for whatever the actor was called, with a second, set by `RemainsNaming::proper`, for someone with a proper name, and it is applied once at the death and again when a save lays a body back down.
A body is seen by a mind as a `PropView` in `Snapshot::props`, filled in `PerceiveSet::Annotate`, carrying which entity it is, where it lies and whose it was, and nothing else.
`EntityState::remains` is how a save holds it: the whole of it is optional, so a save written before remains existed still loads, and the `SaveId` inside it is optional again, since a death nobody was credited with is still a death.
`EntityState::remains_as` holds the prop kind a game dressed the body in afterwards, by name, and it is put back on the body as it is laid down, so a wreck comes back a wreck without the game's own record of what the thing was having to know it died.
A save lays a body down through the same `lay_down`, so a body from a save has a twin too, of the living thing the game's record respawned.
`revive(world, body, health)`, or `commands.revive(body, health)` through `ReviveCommands`, makes the body its twin again: every component the twin has comes back with the twin's value, everything the body gained as a body comes off, and only what the world did to the body stays the body's, where it lies and what it carries.
A looted body comes back without what was taken, a body whose bag was taken away comes back with none, and a bag it gained as a body is emptied onto the floor first.
In a game with items, death has already let fall everything the actor carried and wore, so it stands up with nothing and its gear at its feet, and what its gear lent is folded again from what it wears now.
An actor still dying is refused: it has a twin from the pass it died in, but it is not remains until it is laid down.
A body with something standing on it stands up on the nearest free cell within `RevivalRules::standing_room`, and `Revived { entity }` is sent.

## Using it

A game answers `RemainsLeft` with whatever a body of its own is, on the entity the engine kept: a machine's wreck to go through is that `entity` given a `PropKind` of the game's with its `Glyph` taken off, so the renderer draws it from the prop's definition rather than as the actor that walked.

## The line

The engine refuses to say what remains are.
There is no glyph, no rot timer, no loot table, and no answer to whether a body can be searched, stripped, rebuilt, eaten or raised: a game answers all of it from its own components, on the same entity, reacting to `RemainsLeft`.
Standing a body back up is the engine's, because the engine laid it down and knows what it took off; when, why and with how much health is the game's.
A revived actor comes back as it was the moment it died, including what it was in the middle of, and nothing is started fresh; a game that wants it to forget something answers `Revived`.
The one word the engine puts on a body is the name, and that only through a template a game wrote.
Whose it was is the one thing the engine tells a mind, because sides are its own: it registered them and it holds the hostility matrix, so a mind asking whether that one is ours is asking a question answerable without a word of the game's vocabulary.
Anything finer than that travels as a sense the game pushes.
The engine never removes remains of its own accord, because how long the dead linger is a rule about a world, and a timer here would be a default every game either accepted without meaning to or switched off.
The player's death is the game's alone, marked or not: a run ends on it, and the corpse a game may still want to draw is not taken out from under it.
The cost of keeping the entity is real and worth saying: a game querying its own monsters by a component it added now also matches its bodies, and those queries want `Without<Remains>`.
Queries on `Actor` or `Mind`, which is most of them, are unaffected.

## Where it lives

All of it is in `atglyph-bevy`, in one file, because there is no rule here to test without an `App`: the whole subsystem is which components come off an entity and which go on, and the only way to ask that is to kill something and look.
`combat.rs` owns the death it reacts to and the burial it prevents; `props.rs` owns what a body is once it is one, which is why nothing about bodies appears in a mind's snapshot beyond what any prop puts there.
`atglyph-save` keeps the fact of the death rather than the game doing so, because a game writes down what a thing is and never that it is dead, and on restore lays the body back down from the two numbers it kept.
