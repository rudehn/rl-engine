# Undermere, part 3: monsters

Status: written 2026-10-07, at the end of part 2 on branch `undermere`; the research behind its engine slices is summarised under "What the research found".
The game design is `examples/undermere/DESIGN.md`, section "Monsters"; this is how that section is built.
The user asked for the design calls to be made without further questions, so every call below is the author's, made to be reviewed afterwards.

## Goal

Monsters are characters, not stat blocks.
Every kind has a tell, something the player sees before they understand it, and a trick; depth stacks tricks, so a rat has one, a necromancer three, the thing at the bottom many.
The dungeon is lived in: monsters asleep, on patrol, at a fire, feeding, held captive; a few places on a run belong to a named monster and its own; and the floor ten boss is drawn from three, so the last fight is not the same every run.
The look panel says what is known about a monster, in its traits' own words, and keeps a trick hidden until the player has seen it done.

## Engine work

Six slices, each a loop more than one game writes; the first, fourth and sixth also serve the town and the mercenaries of parts 5 and 6.

### 1. A mind uses what it carries

`docs/TODO.md`'s "A mind cannot use a thing from its bag".
A tactic, with a `Wits` capability (`uses`), that drinks or reads a carried thing whose `use` trigger mends when the mind is hurt past a threshold, and zaps a carried aimed thing at a foe the way `UseAbility` aims an ability, through `Aim::worth_aiming_at` and the item's own aim.
The resolver is the one the player's uses go through, so a monster's wand is refused for exactly what the player's would be.
Mercenaries in part 6 drink their own potions by this.

### 2. A status that sleeps

Part 2 gave statuses `holds`.
A status that `sleeps` holds its holder's turns until a noise reaches it above its hearing, a blow lands on it, or it notices a foe within a cell or two, and then ends, so a sleeping monster wakes the way a sleeping thing does and a sneak attack on one is the dagger's whole point.
Monsters are placed asleep by a share the game chooses per floor.

### 3. An actor's own triggers

Items and props answer moments (`use`, `land`, `fire`, `hit`, `pulse`, `entered`, `destroyed`); an actor answers none.
An actor may carry `Triggers` answering `hit` (its own blow lands: a venomous bite), `struck` (a blow lands on it: a fire aura, a jelly splitting) and `died` (it dies: a burst of spores where it fell), landed as its own doing through the effects pipeline, from content.
The trait model gains a `triggers` field, so "venomous" is one trait anywhere.

### 4. Actors that offer verbs

A prop offers verbs (`open`, `search`, a game's own); an actor offers none, so nothing can be said to a person or done for a captive.
`OfferedHere` gains the actors beside the one holding the turn, each offering what its own `Offers` component names, under the same gate, refusal and `Interacted` answer as a prop's, and the bump into a non-hostile actor offering one thing comes to it.
Part 3 uses it to free captives; part 5 to talk and trade; part 6 to hire.

### 5. The look panel says it all

The inspect panel clips a long line; a monster with five traits needs them all.
It wraps a game's facets onto as many lines as its rectangle allows, with the `wrap` helper the scrollback already uses, and pushes the rest to a "more" line rather than cutting a word.

### 6. Following one leader

`Keep { on: Station::Allies }` keeps a mind near its nearest ally; a war band that follows its chief, and in part 6 a mercenary that follows the adventurer rather than the nearest of its fellows, need one named leader.
`Station::Leader(Entity)` keeps a mind within its distances of that one actor and falls back to its allies when the leader is gone, with the same no-undoing step logic `Keep` has.

## What the research found

`research-monsters.md` (part 2's workspace) is the evidence; in short:
the trigger machinery is carrier-agnostic already, so slice 3 is content and two new moments (`struck`, and `died` relayed across the frame through `Remnant` as a prop's `destroyed` is), with `hit` also fired on the attacking actor;
of sleep's three stimuli only noticing lands a `Cure` in its own pass, so a blow and a noise cure through `Statuses` directly, as `wake_on_damage` already writes `Aware`;
`OfferedHere`, the bump's way and the interaction resolver are filtered to props in three places, and `Interact` already keys by entity;
no `Snapshot` field lists carried usable things (only `missiles` and granted abilities), and `Wits` has a free bit for `USES`;
`revive` restores the twin's own faction, so a raised body becomes an ally after it, through a `befriend` generalised to any side.

## The game

### Armor absorbs at most half a blow

Part 2's soak found that armor, which subtracts flat and is enchanted a point a level, made floors one to eight nearly harmless to a delve that put its scrolls there.
Undermere's damage stages gain one of its own after `SubtractArmor`'s place: armor absorbs at most half of each blow (rounded down), so plate and scrolls still matter, and nothing is ever immune; the deeper monsters' heavier blows (two dice from the ogre down, part 2's tuning) do the rest.

### The bestiary

About thirty kinds over ten floors, in the tiers of `DESIGN.md`'s monsters section, each with a role, a tell and its tricks, written as traits; the skeleton's sixteen kinds are kept and given their tricks.
New traits, each one line in `traits.ron` over the engine seams above: venomous (a bite that poisons), webbing (a spit that holds), splitting (struck, it becomes two), spore-burst (died, a cloud of confusion), stealing (a blow that takes a carried thing, then flight), blinking (an ability), summoning (spectral blades, spawnlings), raising (revives a body nearby as its own), shrouded (unseen while it stands in the dark), regenerating, breathing fire, packing (stays near its own kind), sleeping light or deep.

### What is known about a monster

`Bestiary`, saved with the run, holds for each kind which of its traits the player has seen at work: a trait is seen when its effect lands in the player's sight (a bite poisons someone, a jelly splits, a necromancer raises a body), and plain traits (its look, its speed, whether it flies) are known on sight.
The look panel lists a monster's known traits in their own words and one "?" for each it has not shown yet, so the player knows there is more without knowing what.

### Mini-bosses

Five, a run drawing two between floors three and nine, each with a lair (a prefab of its own), an entourage drawn from roles, and an artifact it carries:
Grubnak the goblin king and his war band (the Crown of Grubnak: allies within sight strike harder; enemies notice you sooner),
the Widow in a web-choked cave of egg sacs that hatch when disturbed (the Widow's Fang: a dagger whose blow poisons and webs; it whispers, waking the floor),
the Bone Choir, a necromancer and its singing dead (the Choirmaster's Staff: raises the dead you kill as yours, for a while; you mend nothing while you hold it),
the Drowned Abbess in a flooded chapel (the Abbess's Chalice: a draught of mending that refills on every floor; the drowned hear you wherever you drink),
the Forgemaster at his anvil with his fire imps (the Forgemaster's Hammer: sweeps and burns; it is heavy, and loud).
Artifacts are part 2's items with a cost written in, named and never unknown.

### The floor ten boss

The run draws one of three, each with its own throne:
the Drowned King, who raises the drowned dead around his flooded hall;
the Mother Below, vast and slow, who spawns spawnlings from the walls while she lives;
the Ember Wyrm, who breathes fire across a hall of pillars and burning pools.
Killing it wins, as now.

### A lived-in dungeon

Monsters asleep at a share that falls with depth, patrols that walk a loop, goblin camps round a fire with their sleepers and a watch, feeders eating the dead, and captives: a monster of the floor, shackled and neutral, that the player may free (the verb, a few turns, under the floor's eyes) to follow as an ally, one more reason a run's party is unlike the last.

## Testing

Each engine slice in its crate; the game's include: a cave spider's bite poisons and the look panel then says so, not before; a sleeping goblin wakes when a noise reaches it and not when the adventurer creeps past unheard; a jelly struck becomes two, each with its share; a necromancer raises a body in its sight and it fights for it; a freed captive follows; every mini-boss's lair builds and its artifact drops; each of the three bosses can be drawn and killing it wins; a run's floors over a span of seeds hold sleepers, a patrol and a camp at the planned shares.

## Not in this part

Mercenaries (part 6), shops and townsfolk (part 5), new biomes and prefab pools (part 4).
