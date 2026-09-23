# Remains

Status: built 2026-09-21, against `main` at `43660f9`.
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

The engine also never removes remains.
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
