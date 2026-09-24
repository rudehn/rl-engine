# Remains

Status: built 2026-09-21, against `main` at `43660f9`; revival (§9) designed 2026-09-23, not built.
The reasoning is here; `docs/OVERVIEW.md` lists what exists.

## 0. Summary

A death used to be final in the most literal sense: `process_deaths` took the actor out of the world and `bury_the_dead` despawned it at the end of the frame.
A game that wanted a body on the floor had to spawn one, and every game that wanted one would have spawned it differently.
Foundry wants four things from a body at once: a repair drone rebuilding a droid wreck, a salvager stripping it, a scrap crab carrying it off, and the commando searching it.
None of those four are the engine's business, and all four need the same thing underneath: something left where the actor fell, that minds can see and walk to, and that survives a save.

This is that thing, and no more than that thing.

## 1. The remains are the actor, kept

The obvious build is to spawn a corpse: a fresh entity at the death position, carrying what the engine copied off the dead actor.
That build forces the engine to decide what "what died" means.
A name? A glyph? Its drop table? Its side? Every answer is a guess about a game the engine has not seen, and every guess is a field that some game will find wrong.

So nothing is spawned and nothing is copied.
The remains are the dead entity itself, kept past the frame it died in.
Whatever the game put on that actor when it spawned it is still there, on the same entity, under the same save kind the game already registered.
A game reads what died by reading its own components, which it wrote and understands.

What the engine takes off is only what the engine put on, and only what means "this is alive and acting", named once as `WasLiving`:

- `Actor` and the turn queue, so a body is never dealt a turn.
- `Blocks`, so a body never holds a doorway shut.
- `Health`, so a body is not a defender: left with health at zero it would still answer the damage pass's query and could be killed a second time.
- `Mind`, `Perception` and `Viewshed`, so a body never thinks and nothing recasts sight for it.
- `Dead`, which is what keeps `bury_the_dead` from despawning it.

The cost is real and worth stating: a game that queries its own monsters by a component it added now also matches its bodies, and those queries need `Without<Remains>`.
Queries on `Actor` or `Mind`, which is most of them, are unaffected.
That cost buys the engine never guessing at content, which is the trade this repo makes everywhere else too.

## 2. Opt-in twice

`RemainsPlugin` is opt-in, as every subsystem is.
Within it, each actor is opt-in too, through `LeavesRemains` on the spawn.

Two levels rather than one because "does anything stay behind" is not a property of a game, it is a property of a kind of thing in a game.
A foundry leaves wrecks behind its droids and nothing behind the vermin that get vaporised; a dungeon leaves bodies behind its orcs and nothing behind its summoned shades.
A game that wants bodies from everything puts the marker on every spawn, which is one line in the spawn it already writes.

Whether the plugin is added decides whether the system runs, and a marker on an actor in a game without the plugin does nothing at all, which is the rule the repo already holds everywhere.

## 3. What the engine refuses to know

There is no name, no glyph, no rot timer, no loot table, and no answer to whether remains can be searched, stripped, rebuilt, eaten or raised.
`Remains` carries two fields: what the clock read when it died, and who got the credit.
Both are things the engine already knew at the moment of death and neither is a claim about a world.

The engine also never removes remains of its own accord; revival (§9) happens only when a game asks for it.
How long the dead linger is a rule about a world, so a game that wants a body to fade despawns it on a clock of its own.
An engine-side decay timer would be a default every game would either accept without meaning to or have to switch off.

## 4. What a mind is told

`Snapshot::remains` is a `Vec<RemainsView>`, filled in `PerceiveSet::Annotate` by the plugin's own contributor, and sorted nearest first with the rest of the snapshot.
A view holds the entity, where it lies, and one thing more: whose it was.

The side is there because sides are the engine's own: it registered them, it holds the hostility matrix, and a mind asking "is that one of mine" is asking a question the engine can answer without learning a word of the game's vocabulary.
It is enough for the two tactics a body invites: a repair drone seeking its own side's wrecks, and a scavenger seeking anyone's.

Anything finer, a droid wreck against a rat's carcass, is the game's, and travels as a `Sense` the game pushes, exactly as the heist's relightable lamps do.

No wits are asked for.
Seeing a body takes no capability, and what a mind may do about one is its game's business rather than a capability the engine has a word for.

## 5. No tactic, yet

Walking to remains is `step_toward`, which exists.
What an actor does on arrival is always the game's, so there is nothing left for an engine tactic to own.
If a second game writes the same walk-and-act shape, that is the signal to lift it, which is the same rule `Facet` follows for panel fields.

## 6. Saving

A game writes down what a thing *is*, never that it is dead: that is the whole point of `Saveable`.
So the fact that this one is lying on the floor is the engine's to keep, as `EntityState` keeps position, health, bags and statuses.

`EntityState::remains` holds the clock reading and the killer's `SaveId`.
On restore the game's own `restore` spawns a living thing again, from its own record, and the engine then lays it back down exactly as the death did: `WasLiving` off, `Remains` on.
The killer comes back only if it was saved too, since credit for a blow is not worth keeping an entity alive for.

The field defaults, so a save written before this exists still loads.

## 7. Tests

- The opt-in, as one property: the marker decides, and a death without it is buried exactly as it always was.
- A body holds no turn, blocks nothing, and cannot be struck again.
- The game's own components come through untouched, which is the claim §1 rests on.
- Two deaths on one cell leave two bodies; the engine merges nothing.
- The player's death is left to the game, marked or not, since a run ends on it.
- A mind is told the remains it can see and no more.
- A body saved is a body continued, without the game's record of its kinds learning the word.

## 8. What is not here

- **Burning.** Remains burn if the game marked them `Flammable`, through the fire rules that already exist. The engine adds no flammability of its own, because whether a body burns is a fact about bodies in that world.
- **Remains of the player.** The player is left whole for the game, which may still want to draw it, read its health or say something about it on the screen the run ends on.
- **Bones files, or anything across runs.** Out of scope: nothing here outlives a save.

## 9. Revival

Status: designed 2026-09-23 with `docs/design/work.md`, not built.

Foundry's repair drone rebuilds a droid wreck into a droid, so remains must be able to stand up again.
The engine laid the body down, so standing it back up is the engine's too; a game that respawned a fresh droid in its place would lose what made it that droid, its bag, its statuses and every component the game put on it, and each game would lose a different part of it.

### 9.1 A copy of the living actor, taken at the moment of death

Revival is not built from a list of what death takes off.
A list goes stale the day someone adds to what death removes, and it cannot see what a game's own system removes when one of its actors dies.

Instead, the moment an actor that `LeavesRemains` dies, the engine copies the whole entity into a twin that is `Disabled`, which Bevy's default query filters skip, so no system and no panel ever sees it.
The copy is taken by a `RemainsPlugin` system in `ResolveSet::Damage` straight after `apply_damage`, which is where `DeathEvent` is written.
That is before `TurnSet::React`, where a game answers a death, and before `CleanupSet::Remove`, where the engine takes the actor out of the world, so the twin is the actor as it was when it died, whoever removes what afterwards.

Bevy copies a component only if it is `Clone`, and silently skips one that is not.
So the engine compares the twin's components with the living actor's as it copies, and reports any it could not copy by name, saying what to do: "`Patrol` cannot come back to life: derive `Clone`".
That is loud at runtime in every game, whether or not the game has a test that would notice.

Laying a body down again on load goes through the same function as a death does, which `leave_remains` and the save both call, so a body continued from a save has a twin too.
On that path the twin is the living thing the game's own record just spawned, which is all a save knows, so a droid revived after a load comes back without the statuses it died with.

### 9.2 The twin lives exactly as long as the body is remains

`Remains` gets an `on_remove` hook that despawns the twin.
Bevy runs it when the component is removed and when the entity is despawned, so every way a body stops being one takes its twin with it: consumed, destroyed, rotted on a game's clock, turned by a game into something else, or revived.
No path through the engine or a game can skip it, because the hook belongs to the component rather than to a system someone must remember to run.

### 9.3 What revival does

`commands.revive(body, health)` gives the body back the shape of its twin:

- Every component the twin has and the body lacks is put back, which is everything death took off and anything a game's death took off, named nowhere.
- Every component the body has and the twin lacks is taken off: `Remains`, whose hook then despawns the twin, `Prop`, and whatever the game added to the body, such as Foundry's `PropKind`.
- A component on both keeps the body's value, because that is what has happened since.
  The one exception is `Name`, which the engine itself changed when it named the body, and which comes back from the twin.
- `Health` is set to `health`, capped at the twin's maximum.

What it carries is the one place the twin is never trusted, because the twin's bag is a list of item entities that may since have gone anywhere.
`Inventory` and `Equipped` are never put back from the twin, even when the body has lost them:

- A looted body comes back without what was taken, because its bag was emptied in place and keeps the body's value like any other.
- A body whose bag a game took away altogether comes back carrying nothing, rather than holding a list of items that are now in the player's bag, which would be one item in two bags.
- What it still wears is what it still carries: taking a worn item from a body takes it out of `Equipped` too (`docs/design/work.md` §8, step 1), so no revived droid comes back wearing the player's armor.
- Revival never destroys an item either: a bag the body gained while it was a body, which the twin never had, is emptied onto the floor where it stands before it is taken off.

Putting `Actor` and `Blocks` back is what readmits it: `admit_new_actors` already admits anything that gains them, into the queue and the occupancy index.
It then writes `Revived { entity }`.

Rejected: making `Remains` a disabling marker and taking nothing off at death, so that revival would have nothing to put back.
Every system that should see a body, the renderer, the panels, props, fire and the minds, would have to opt back in to seeing it, and the turn queue and the occupancy index are not queries, so they would still be handled by hand in both directions.

### 9.4 The actor as it was, including what it was doing

The twin holds everything the actor had when it died, including what it was in the middle of, and revival brings all of it back.
Nothing is started fresh, and no plugin registers anything about revival or knows it exists.

That is the promise of §1 carried one step further: the remains are the actor, kept, and a revived actor is the same actor.
A drone that died mid-repair and stands up beside the same wreck carries on with it; if the wreck has gone, its work breaks as `OutOfReach` on the next pass, as anyone's would.
A droid that had noticed the player before it died still has.
Every system that points at another entity already copes with that entity disappearing, since living actors lose their targets all the time, so stale state from before a death is not a new case for any of them.
A game that wants a revived actor to forget something answers `Revived` in `TurnSet::React`.

### 9.5 Tests

- Kill an actor carrying every engine plugin's components and a game component of its own, revive it, and it has exactly the components it had when it died, less none and plus none.
- A component a game's own system removed at death comes back.
- A component that is not `Clone` is reported by name at the moment of death.
- No twin outlives its body, over every path: despawned, `Remains` removed, revived.
- A revived actor is dealt turns, blocks its cell, can be hurt and can die again, and leaves remains again.
- A body continued from a save can be revived.
- A looted body comes back without what was looted, worn or carried; one whose bag was taken away comes back with none; and no item is ever in two bags.
- Nothing carried or contained is lost to a revival.
